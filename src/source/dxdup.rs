use super::Source;
use anyhow::{anyhow, Result};
use ffmpeg_next::{
    filter::{self, Graph},
    frame, Error,
};

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
    fn hw_support(&self) -> bool {
        true
    }
    fn next_frame(&mut self, out: &mut frame::Video) -> std::result::Result<(), Error> {
        self.graph.get("out").unwrap().sink().frame(out)?;
        Ok(())
    }
}
