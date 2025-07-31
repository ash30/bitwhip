use ffmpeg_next::frame;

mod avfoundation;
mod dxdup;

pub use avfoundation::AFScreenCapturer;
pub use dxdup::DisplayDuplicator;

pub use ffmpeg_next::util::error::Error;

pub trait Source {
    fn next_frame(&mut self, out: &mut frame::Video) -> Result<(), Error>;
    fn hw_support(&self) -> bool {
        false
    }
}
