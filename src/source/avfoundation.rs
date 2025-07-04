use std::{collections::HashMap, time::Instant};

use crate::encoder::Encoder;

use super::EncodedPacket;
use anyhow::{anyhow, Result};
use ffmpeg_next::{
    codec::Context,
    decoder::Video as VideoDecoder,
    device,
    format::{self, context::Input},
    frame,
    software::scaling::context::Context as Convertor,
    Dictionary, Error, Frame, Packet, Rational,
};
use ffmpeg_sys_next::EAGAIN;

pub struct AFScreenCapturer {
    device: Input,
    decoder: VideoDecoder,
    encoder: Encoder,
    convertor: Convertor,
}

impl AFScreenCapturer {
    pub fn new() -> Result<Self> {
        let input = device::input::video()
            .find(|d| d.name() == "avfoundation")
            .ok_or(anyhow!("missing device"))?;
        let framerate = Rational::new(30, 1);

        let mut opts = Dictionary::new();
        opts.set("pixel_format", "uyvy422");
        opts.set("frame_rate", "30/1");

        let device = format::open_with("2", &input, opts)?.input();
        let device_tbase = device
            .stream(0)
            .ok_or(anyhow!("missing stream"))?
            .time_base();

        let dec_ctx = Context::from_parameters(device.stream(0).unwrap().parameters())?;
        let decoder = dec_ctx.decoder().video()?;

        let encoder = Encoder::new("h264_videotoolbox", Some(HashMap::from([])), |encoder| {
            encoder.set_width(decoder.width());
            encoder.set_height(decoder.height());
            encoder.set_aspect_ratio(decoder.aspect_ratio());
            encoder.set_frame_rate(Some(framerate));
            encoder.set_time_base(device_tbase);
            encoder.set_format(format::Pixel::YUV420P);
            Ok(())
        })?;

        let convertor = ffmpeg_next::software::converter(
            (decoder.width(), decoder.height()),
            format::Pixel::UYVY422,
            format::Pixel::YUV420P,
        )?;

        Ok(Self {
            device,
            decoder,
            encoder,
            convertor,
        })
    }
}

impl Iterator for AFScreenCapturer {
    type Item = Result<EncodedPacket>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut frame_in = frame::Video::empty();
        let mut frame_out = frame::Video::empty();
        let mut packet = Packet::empty();

        loop {
            match self.encoder.encoder.receive_packet(&mut packet) {
                Ok(_) => return Some(Ok(EncodedPacket(packet, Instant::now()))),
                Err(Error::Other { errno }) if errno == EAGAIN => {}
                Err(e) => return Some(Err(e.into())),
            };

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
            if let Err(e) = self.encoder.encoder.send_frame(&frame_out) {
                return Some(Err(e.into()));
            }
        }
    }
}
