use std::collections::HashMap;

use super::Source;
use crate::encoder::Encoder;

use anyhow::{anyhow, Result};
use ffmpeg_next::{
    codec::Context,
    decoder::Video as VideoDecoder,
    device,
    format::{self, context::Input},
    frame,
    software::scaling::context::Context as Convertor,
    Dictionary, Error, Rational,
};

use ffmpeg_sys_next::EAGAIN;

impl Source for AFScreenCapturer {
    type Output = Encoder<Self>;

    fn init_source() -> Result<Self::Output> {
        let src = AFScreenCapturer::new()?;
        let width = src.decoder.width();
        let height = src.decoder.width();
        let aspect = src.decoder.aspect_ratio();
        let framerate = Rational::new(30, 1);
        let device_tbase = src
            .device
            .stream(0)
            .ok_or(anyhow!("missing stream"))?
            .time_base();

        Encoder::new(
            src,
            "h264_videotoolbox",
            Some(HashMap::from([])),
            |encoder| {
                encoder.set_width(width);
                encoder.set_height(height);
                encoder.set_aspect_ratio(aspect);
                encoder.set_frame_rate(Some(framerate));
                encoder.set_time_base(device_tbase);
                encoder.set_format(format::Pixel::YUV420P);
                Ok(())
            },
        )
    }
}

pub struct AFScreenCapturer {
    device: Input,
    decoder: VideoDecoder,
    convertor: Convertor,
}

impl AFScreenCapturer {
    pub fn new() -> Result<Self> {
        let input = device::input::video()
            .find(|d| d.name() == "avfoundation")
            .ok_or(anyhow!("missing device"))?;

        let mut opts = Dictionary::new();
        opts.set("pixel_format", "uyvy422");
        opts.set("frame_rate", "30/1");

        let device = format::open_with("2", &input, opts)?.input();

        let dec_ctx = Context::from_parameters(device.stream(0).unwrap().parameters())?;
        let decoder = dec_ctx.decoder().video()?;

        // Todo: should probably move this towards encoder?
        let convertor = ffmpeg_next::software::converter(
            (decoder.width(), decoder.height()),
            format::Pixel::UYVY422,
            format::Pixel::YUV420P,
        )?;

        Ok(Self {
            device,
            decoder,
            convertor,
        })
    }
}

impl Iterator for AFScreenCapturer {
    type Item = Result<frame::Video>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut frame_in = frame::Video::empty();
        let mut frame_out = frame::Video::empty();

        loop {
            let (_, p) = self.device.packets().next()?;

            if let Err(e) = self.decoder.send_packet(&p) {
                return Some(Err(e.into()));
            }

            match self.decoder.receive_frame(&mut frame_in) {
                Err(Error::Other { errno }) if errno == EAGAIN => continue,
                Err(e) => return Some(Err(e.into())),
                _ => break,
            };
        }
        // Resample due to format change
        if let Err(e) = self.convertor.run(&frame_in, &mut frame_out) {
            return Some(Err(e.into()));
        }
        frame_out.set_pts(frame_in.pts());
        return Some(Ok(frame_out));
    }
}
