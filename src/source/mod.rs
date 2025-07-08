use anyhow::{anyhow, Result};
use ffmpeg_next::Packet;
use std::time::Instant;

use crate::{CaptureMethod, SourceConfig};

mod avfoundation;
mod dxdup;

pub struct EncodedPacket(pub Packet, pub Instant);

trait Source {
    type Output: Iterator<Item = Result<EncodedPacket>>;
    fn init_source(config: &SourceConfig) -> Result<Self::Output>;
}

// Factory method to intialise source
pub fn init_capture_source(
    src: CaptureMethod,
    config: SourceConfig,
) -> Result<Box<dyn Iterator<Item = Result<EncodedPacket>>>> {
    Ok(match src {
        #[cfg(target_os = "macos")]
        CaptureMethod::AVFoundation => {
            Box::new(avfoundation::AFScreenCapturer::init_source(&config)?)
        }
        #[cfg(target_os = "windows")]
        CaptureMethod::DXGI => Box::new(dxdup::DisplayDuplicator::init_source(&config)?),
        _ => Err(anyhow!("unsupported on this platform"))?,
    })
}
