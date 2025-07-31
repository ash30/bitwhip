use super::Source;
use crate::SourceConfig;

use anyhow::{anyhow, Result};
use ffmpeg_next::{
    codec::Context,
    decoder::Video as VideoDecoder,
    device,
    format::{self, context::Input},
    frame, Dictionary, Error, Packet,
};

// TODO: Could generalise this to any device in future
pub struct AFScreenCapturer {
    device: Input,
    decoder: VideoDecoder,
}

impl AFScreenCapturer {
    pub fn new(config: &SourceConfig) -> Result<Self> {
        let input = device::input::video()
            .find(|d| d.name() == "avfoundation")
            .ok_or(anyhow!("missing device"))?;

        let framerate = format!("{}/1", config.framerate);
        let mut opts = Dictionary::new();
        //opts.set("pixel_format", "uyvy422");
        opts.set("pixel_format", "nv12");
        opts.set("framerate", &framerate);

        let device_index = config.device.clone().unwrap_or("1".to_string());
        let device = format::open_with(&device_index, &input, opts)?.input();

        let dec_ctx = Context::from_parameters(device.stream(0).unwrap().parameters())?;
        let decoder = dec_ctx.decoder().video()?;

        Ok(Self { device, decoder })
    }
}
impl Source for AFScreenCapturer {
    fn next_frame(&mut self, out: &mut frame::Video) -> Result<(), Error> {
        let mut p = Packet::empty();
        // EAGAIN may be returned to caller or EOF
        p.read(&mut self.device)?;
        self.decoder.send_packet(&p)?;
        // EAGAIN shouldn't happen here, no need to loop
        self.decoder.receive_frame(out)?;
        Ok(())
    }
}
