use crate::player::render_video;
use anyhow::{anyhow, Error, Result};
use axum::{response::Response, routing::post, Router};
use clap::{Args, Parser, Subcommand, ValueEnum};
use encoder::{EncodedPacket, EncodedPacketIter, EncoderBuilder};
use ffmpeg_next::{frame, Rational};
use log::LevelFilter;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use source::{AFScreenCapturer, DisplayDuplicator, Source};
use std::{sync::mpsc, time::Instant};
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};

mod client;
mod encoder;
mod player;
mod source;
mod whip;

#[no_mangle]
pub static NvOptimusEnablement: i32 = 1;
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: i32 = 1;

#[derive(Debug, Clone, Args)]
struct SourceConfig {
    /// Target frames per second for capture device
    #[arg(short, long, default_value_t = 60)]
    framerate: i32,
    /// Device(s) to capture, source specific
    #[arg(short, long)]
    device: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum CaptureMethod {
    AVFoundation,
    DXGI,
}

impl Default for CaptureMethod {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return CaptureMethod::DXGI;
        #[cfg(target_os = "macos")]
        return CaptureMethod::AVFoundation;
        panic!("unsupported platform")
    }
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

        /// Capture method
        #[clap(short, value_enum, default_value_t=CaptureMethod::default())]
        capture_method: CaptureMethod,

        #[command(flatten)]
        config: SourceConfig,

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
        Commands::Stream {
            url,
            token,
            capture_method,
            config,
        } => stream(url, token, capture_method, config).await?,
        Commands::PlayWHIP {} => play_whip().await,
        Commands::PlayWHEP { url, token } => play_whep(url, token).await?,
    }

    Ok(())
}

async fn stream(
    url: String,
    token: Option<String>,
    src: CaptureMethod,
    config: SourceConfig,
) -> Result<()> {
    let (handle, rx) = match src {
        #[cfg(target_os = "macos")]
        CaptureMethod::AVFoundation => _stream(AFScreenCapturer::new(&config)?, &config),
        #[cfg(target_os = "windows")]
        CaptureMethod::DXGI => _stream(DisplayDuplicator::new()?, &config),
        _ => Err(anyhow!("unsupported on this platform"))?,
    };

    tokio::select! {
        _ = whip::publish(&url, token, rx) => {},
        res = handle => {
            res??
        }
    }
    Ok(())
}

fn _stream<T>(
    mut source: T,
    config: &SourceConfig,
) -> (JoinHandle<Result<()>>, UnboundedReceiver<EncodedPacket>)
where
    T: Source + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let frame_rate = Rational::new(config.framerate, 1);
    let join_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let encoder = EncoderBuilder::new()
            .for_source(&mut source)
            .customise(move |encoder| {
                encoder.set_frame_rate(Some(frame_rate));
                encoder.set_time_base(frame_rate.invert());
                encoder.set_gop(120);
                encoder.set_max_b_frames(0);
            })
            .open()?;

        let mut iter = EncodedPacketIter::new(encoder, source);
        loop {
            match iter.next() {
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

    (join_handle, rx)
}

async fn whip_handler(
    tx: mpsc::Sender<ffmpeg_next::frame::Video>,
    offer: String,
) -> Response<String> {
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
