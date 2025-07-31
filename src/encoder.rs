use anyhow::{anyhow, bail, Context, Result};
use ffmpeg::ffi::AVCodecContext;
use ffmpeg::{
    codec::Context as CodecContext, encoder::video::Encoder as VideoEncoderOpened,
    encoder::video::Video as VideoEncoder, frame, Error, Packet,
};
use ffmpeg_next::{self as ffmpeg};
use ffmpeg_sys_next::{av_buffer_ref, AVBufferRef, EAGAIN};
use log::info;
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{
    collections::HashMap,
    ffi::{c_void, CString},
};

use crate::source::{self, Output, PollSource, Source};

pub struct EncodedPacket(pub Packet, pub source::Delta);

// Simple Type Wrapper to organise enc preset and name
pub struct Codec(String);

impl Default for Codec {
    fn default() -> Self {
        #[cfg(target_os = "macos")]
        return Self("h264_videotoolbox".to_string());
        #[cfg(target_os = "windows")]
        return Self("h264_nvenc".to_string());
    }
}

impl Into<String> for Codec {
    fn into(self) -> String {
        self.0
    }
}

impl Codec {
    fn default_settings(&self) -> HashMap<String, String> {
        match self.0.as_str() {
            "h264_nvenc" => HashMap::from([
                ("preset".into(), "p6".into()),
                ("tune".into(), "ull".into()),
            ]),
            _ => HashMap::from([]),
        }
    }
}

pub fn encode(encoder: &mut VideoEncoderOpened, frame: &frame::Video) -> Result<Option<Packet>> {
    encoder.send_frame(frame)?;

    let mut packet = Packet::empty();
    if encoder.receive_packet(&mut packet).is_ok() {
        return Ok(Some(packet));
    }

    Ok(None)
}

pub struct EncoderBuilder {
    codec: Codec,
    hw_ctx: Option<*mut AVBufferRef>,
    example_frame: frame::Video,
    customise: Option<Box<dyn FnOnce(&mut VideoEncoder)>>,
}

impl EncoderBuilder {
    pub fn new() -> Self {
        Self {
            codec: Codec::default(),
            hw_ctx: None,
            example_frame: frame::Video::empty(),
            customise: None,
        }
    }

    pub fn set_encoder(mut self, c: Codec) -> Self {
        self.codec = c;
        self
    }

    pub fn for_source<T: Source>(mut self, src: &mut T) -> Self {
        src.next_frame(&mut self.example_frame).expect("");
        if src.hw_support() {
            unsafe { self.hw_ctx = Some((*self.example_frame.as_ptr()).hw_frames_ctx) }
        }
        self
    }

    pub fn customise(mut self, f: impl FnOnce(&mut VideoEncoder) + 'static) -> Self {
        self.customise = Some(Box::new(f));
        self
    }

    pub fn open(mut self) -> Result<VideoEncoderOpened> {
        let settings = self.codec.default_settings();
        let name: String = self.codec.into();

        let codec = ffmpeg::encoder::find_by_name(name.as_str())
            .ok_or_else(|| anyhow!("Missing encoder {}", name))?;
        let codec_context = CodecContext::new_with_codec(codec);

        let mut enc = codec_context.encoder().video()?;
        // general encoder options
        enc.set_width(self.example_frame.width());
        enc.set_height(self.example_frame.height());
        enc.set_aspect_ratio(self.example_frame.aspect_ratio());
        // TODO: will this work?
        enc.set_format(self.example_frame.format());

        // set options based on cli args
        if let Some(f) = self.customise.take() {
            (f)(&mut enc)
        }

        if let Some(buf) = self.hw_ctx {
            unsafe {
                let encoder = &mut *enc.as_mut_ptr();
                encoder.hw_frames_ctx = av_buffer_ref(buf);
            }
        }

        // set any encoder specific options from defaults
        // in future, we could merge cli args in as well
        for (key, value) in settings.iter() {
            info!("Setting option {key} {value}");
            unsafe { Self::set_option(enc.as_mut_ptr(), key, value)? };
        }

        Ok(enc.open()?)
    }

    // helpers

    unsafe fn set_option(context: *mut AVCodecContext, name: &str, val: &str) -> Result<()> {
        let name_c = CString::new(name).context("Error in CString")?;
        let val_c = CString::new(val).context("Error in CString")?;
        let retval: i32 = ffmpeg::ffi::av_opt_set(
            context as *mut c_void,
            name_c.as_ptr(),
            val_c.as_ptr(),
            ffmpeg::ffi::AV_OPT_SEARCH_CHILDREN,
        );
        if retval != 0 {
            bail!("set_option failed: {retval}");
        }
        Ok(())
    }
}

// An iterator to simplify integration between source and encoder
pub struct EncodedPacketIter<T> {
    source: PollSource<T>,
    encoder: VideoEncoderOpened,
    frame_next: frame::Video,
    frame_timestamp: Duration,
}

impl<T> EncodedPacketIter<T> {
    pub fn new(encoder: VideoEncoderOpened, source: T) -> Self {
        let target_fps = encoder.frame_rate();
        Self {
            encoder,
            source: PollSource::new(source, target_fps, Instant::now()),
            frame_next: frame::Video::empty(),
            frame_timestamp: Duration::new(0, 0),
        }
    }
}

impl<T> Iterator for EncodedPacketIter<T>
where
    T: Source,
{
    type Item = Result<EncodedPacket>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut p = Packet::empty();
        loop {
            // drain packets from encoder
            match self.encoder.receive_packet(&mut p) {
                // Reuse timestamp for all frame packets
                Ok(_) => return Some(Ok(EncodedPacket(p, self.frame_timestamp))),
                Err(Error::Other { errno }) if errno == EAGAIN => {}
                Err(e) => return Some(Err(e.into())),
            }

            // get next frame since encoder is empty
            loop {
                let now = Instant::now();
                match self.source.next(now, &mut self.frame_next) {
                    Output::Complete => return None,
                    Output::Item(Ok(_), time_delta) => {
                        // We will use simple sys time delta for rtp timestamp
                        self.frame_timestamp = time_delta;
                        break;
                    }
                    Output::Item(Err(e), _) => return Some(Err(e.into())),
                    Output::Pending(d) => {
                        sleep(d);
                        continue;
                    }
                };
            }

            if let Err(e) = self.encoder.send_frame(&self.frame_next) {
                return Some(Err(e.into()));
            }
        }
    }
}
