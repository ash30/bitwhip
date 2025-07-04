use std::time::Instant;

use anyhow::Result;
use ffmpeg_next::{frame::video::Video, Packet};

#[cfg(target_os = "windows")]
mod dxdup;

#[cfg(target_os = "macos")]
mod avfoundation;

// ==========

pub struct EncodedPacket(pub Packet, pub Instant);

pub trait Source {
    fn get_frame(&mut self) -> Result<Video>;
}

#[cfg(target_os = "windows")]
pub fn init_source(c: &CaptureSourceConfig) -> Result<impl Source> {
    dxdup::DisplayDuplicator::new()
}

#[cfg(target_os = "macos")]
pub fn init_source(c: &CaptureSourceConfig) -> Result<impl Source> {
    todo!()
}
