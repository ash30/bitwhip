use std::time::Instant;

use anyhow::Result;
use ffmpeg_next::{frame::video::Video, Packet};

#[cfg(target_os = "windows")]
mod dxdup;

#[cfg(target_os = "macos")]
mod avfoundation;

// ==========

pub struct EncodedPacket(pub Packet, pub Instant);

#[derive(Debug, Clone)]
pub enum CaptureSource {
    AVFoundation,
}

pub fn init_capture_source(
    src: CaptureSource,
) -> Result<impl Iterator<Item = Result<EncodedPacket>>> {
    #[cfg(target_os = "macos")]
    return avfoundation::AFScreenCapturer::new();
    #[cfg(target_os = "windows")]
    return dxdup::DisplayDuplicator::new();
}
