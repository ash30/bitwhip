use super::Source;
use crate::{encoder::Encoder, SourceConfig};
use anyhow::{anyhow, Result};
use ffmpeg_next::{
    filter::{self, Graph},
    format::Pixel,
    frame, Rational,
};
use ffmpeg_sys_next::av_buffer_ref;
use std::collections::HashMap;

pub struct DisplayDuplicator {
    graph: Graph,
}

impl DisplayDuplicator {
    pub fn new() -> Result<Self> {
        let mut graph = filter::Graph::new();

        let buffer_sink = filter::find("buffersink")
            .ok_or_else(|| anyhow!("Failed to find buffersink filter"))?;

        graph.add(&buffer_sink, "out", "")?;
        graph.input("out", 0)?.parse("ddagrab=0:framerate=60")?;
        graph.validate()?;

        Ok(Self { graph })
    }
}

impl Source for DisplayDuplicator {
    type Output = Encoder<DisplayDuplicator>;

    fn init_source(config: &SourceConfig) -> Result<Self::Output> {
        let mut dd = DisplayDuplicator::new()?;
        let Some(Ok(frame)) = dd.next() else {
            return Err(anyhow!(""));
        };

        let encoder = Encoder::new(
            dd,
            "h264_nvenc",
            Some(HashMap::from([
                ("preset".into(), "p6".into()),
                ("tune".into(), "ull".into()),
            ])),
            |encoder| {
                let frame_rate = Rational::new(config.framerate, 1);
                encoder.set_width(frame.width());
                encoder.set_height(frame.height());
                encoder.set_time_base(frame_rate.invert());
                encoder.set_frame_rate(Some(frame_rate));
                encoder.set_bit_rate(5000 * 1000);
                encoder.set_gop(120);
                encoder.set_max_b_frames(0);
                encoder.set_format(Pixel::D3D11);
                unsafe {
                    let encoder = &mut *encoder.as_mut_ptr();
                    let hw_frames = (*frame.as_ptr()).hw_frames_ctx;
                    encoder.hw_frames_ctx = av_buffer_ref(hw_frames);
                }
                Ok(())
            },
        )?;

        Ok(encoder)
    }
}

impl Iterator for DisplayDuplicator {
    type Item = Result<frame::Video>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut frame = frame::Video::empty();
        if let Err(e) = self.graph.get("out").unwrap().sink().frame(&mut frame) {
            return Some(Err(e.into()));
        };
        Some(Ok(frame))
    }
}
