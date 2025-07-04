use crate::player::render_video;
use anyhow::{Error, Result};
use axum::{response::Response, routing::post, Router};
use clap::{Parser, Subcommand};
use encoder::Encoder;
use ffmpeg_next::ffi::{av_buffer_ref, AVBufferRef};
use log::LevelFilter;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use source::{init_capture_source, CaptureSource};
use std::{collections::HashMap, sync::mpsc, time::Instant};

mod client;
mod encoder;
mod player;
mod source;
mod whip;

//struct EncodedPacket(Packet, Instant);
pub use source::EncodedPacket;

#[no_mangle]
pub static NvOptimusEnablement: i32 = 1;
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: i32 = 1;

#[cfg(target_os = "windows")]
fn create_encoder(width: u32, height: u32, hw_frames: *mut AVBufferRef) -> Result<Encoder> {
    let encoder = Encoder::new(
        "h264_nvenc",
        Some(HashMap::from([
            ("preset".into(), "p6".into()),
            ("tune".into(), "ull".into()),
        ])),
        |encoder| {
            let frame_rate = Rational::new(60, 1);
            encoder.set_bit_rate(5000 * 1000);
            encoder.set_width(width);
            encoder.set_height(height);
            encoder.set_time_base(frame_rate.invert());
            encoder.set_frame_rate(Some(frame_rate));
            encoder.set_gop(120);
            encoder.set_max_b_frames(0);
            encoder.set_format(Pixel::D3D11);
            unsafe {
                let encoder = &mut *encoder.as_mut_ptr();
                encoder.hw_frames_ctx = av_buffer_ref(hw_frames);
            }

            Ok(())
        },
    )?;

    Ok(encoder)
}

#[derive(Parser)]
#[command(name = "bitwhip")]
#[command(bin_name = "bitwhip")]
struct Cli {
    #[command(subcommand)]
    commands: Commands,

    /// Increase log verbosity, multiple occurrences (-vvv) further increase
    #[clap(short, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Stream to a WHIP destination
    #[command(arg_required_else_help = true)]
    Stream {
        /// The WHIP URL
        url: String,

        /// The WHIP bearer token
        token: Option<String>,
    },

    /// Start a WHIP server that accepts incoming requests
    PlayWHIP {},

    /// Play from a WHEP destination
    #[command(arg_required_else_help = true)]
    PlayWHEP {
        /// The WHEP URL
        url: String,

        /// The WHEP bearer token
        token: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    ffmpeg_next::init()?;

    let args = Cli::parse();
    let level_filter = match args.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        3.. => LevelFilter::Trace,
    };

    TermLogger::init(
        level_filter,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )?;

    match args.commands {
        Commands::Stream { url, token } => stream(url, token).await?,
        Commands::PlayWHIP {} => play_whip().await,
        Commands::PlayWHEP { url, token } => play_whep(url, token).await?,
    }

    Ok(())
}

async fn stream(url: String, token: Option<String>) -> Result<()> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let join_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut source = init_capture_source(source::CaptureSource::AVFoundation)?;
        loop {
            match source.next() {
                Some(Err(e)) => {
                    return Err(e);
                }
                Some(Ok(packet)) => {
                    tx.send(packet).unwrap();
                }
                None => break,
            }
        }
        Ok(())
    });

    tokio::select! {
        _ = whip::publish(&url, token, rx) => {},
        res = join_handle => {
            res??
        }
    }

    Ok(())
}

async fn whip_handler(tx: mpsc::Sender<ffmpeg_next::frame::Video>, offer: String) -> Response<String> {
    let answer = whip::subscribe_as_server(tx, offer);
    Response::builder()
        .status(201)
        .header("Location", "/")
        .body(answer)
        .unwrap()
}

async fn play_whip() {
    println!("Listening for WHIP Requests on 0.0.0.0:1337");
    let (tx, rx): (mpsc::Sender<ffmpeg_next::frame::Video>, mpsc::Receiver<ffmpeg_next::frame::Video>) = mpsc::channel();

    tokio::task::spawn(async move {
        axum::serve(
            tokio::net::TcpListener::bind("0.0.0.0:1337").await.unwrap(),
            Router::new().route("/", post(move |offer: String| whip_handler(tx, offer))),
        )
        .await
        .unwrap();
    });

    render_video(rx);
}

async fn play_whep(url: String, token: Option<String>) -> Result<()> {
    let (tx, rx): (mpsc::Sender<ffmpeg_next::frame::Video>, mpsc::Receiver<ffmpeg_next::frame::Video>) = mpsc::channel();

    whip::subscribe_as_client(tx, &url, token).await;
    render_video(rx);

    Ok(())
}
