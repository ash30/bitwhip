use std::time::{Duration, Instant};

use ffmpeg_next::{frame, Rational};

mod avfoundation;
mod dxdup;

pub use avfoundation::AFScreenCapturer;
pub use dxdup::DisplayDuplicator;

pub use ffmpeg_next::util::error::{Error, EAGAIN};

pub trait Source {
    fn next_frame(&mut self, out: &mut frame::Video) -> Result<(), Error>;
    fn hw_support(&self) -> bool {
        false
    }
}

pub struct PollSource<T> {
    start: Instant,
    next: Option<Instant>,
    source: T,
    target_fps: Rational,
}

pub type Delta = Duration;
pub enum Output<T> {
    Pending(Duration),
    Item(T, Delta),
    Complete,
}

impl<T> PollSource<T> {
    pub fn new(source: T, target_fps: Rational, start: Instant) -> Self {
        Self {
            start,
            source,
            next: None,
            target_fps,
        }
    }
}

impl<T> PollSource<T>
where
    T: Source,
{
    pub fn next(
        &mut self,
        now: Instant,
        frame: &mut frame::Video,
    ) -> Output<Result<(), ffmpeg_next::Error>> {
        match self.source.next_frame(frame) {
            Err(Error::Eof) => Output::Complete,
            Err(Error::Other { errno }) if errno == EAGAIN => Output::Pending(
                self.next
                    .map(|i| (i - now).max(Duration::ZERO))
                    .unwrap_or(Duration::from_millis(5)),
            ),
            Err(e) => Output::Item(Err(e), now - self.start),
            Ok(_) => {
                unsafe {
                    let d = (*frame.as_ptr()).duration * self.target_fps.numerator() as i64;
                    self.next
                        .replace(now + Duration::from_millis(d.try_into().unwrap()));
                }
                Output::Item(Ok(()), now - self.start)
            }
        }
    }
}
