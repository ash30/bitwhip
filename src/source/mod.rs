use std::time::Instant;

use anyhow::Result;
use ffmpeg_next::{frame::video::Video, Packet, Rational};

#[cfg(target_os = "windows")]
mod dxdup;

#[cfg(target_os = "macos")]
mod avfoundation;

// ==========

#[derive(Debug, Clone)]
pub enum CaptureSource {
    AVFoundation,
}

pub struct EncodedPacket(pub Packet, pub Instant);

trait Source {
    type Output: Iterator<Item = Result<EncodedPacket>>;
    fn init_source() -> Result<Self::Output>;
}

pub fn init_capture_source(
    src: CaptureSource,
) -> Result<impl Iterator<Item = Result<EncodedPacket>>> {
    #[cfg(target_os = "macos")]
    {
        avfoundation::AFScreenCapturer::init_source()
    }
    #[cfg(target_os = "windows")]
    return dxdup::DisplayDuplicator::new();
}
