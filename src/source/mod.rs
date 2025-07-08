use std::time::Instant;

use anyhow::Result;
use ffmpeg_next::Packet;

mod avfoundation;
mod dxdup;

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
    {
        dxdup::DisplayDuplicator::init_source()
    }
}
