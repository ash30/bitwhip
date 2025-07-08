use anyhow::{anyhow, bail, Context, Result};
use ffmpeg::ffi::AVCodecContext;
use ffmpeg::{codec::Context as CodecContext, encoder::Video, Error, Packet};
use ffmpeg_next::{self as ffmpeg, frame};
use log::info;
use std::time::Instant;
use std::{
    collections::HashMap,
    ffi::{c_void, CString},
};

use crate::source::EncodedPacket;
use ffmpeg_sys_next::EAGAIN;

pub struct Encoder<T> {
    src: T,
    encoder: Video,
}

impl<T> Encoder<T> {
    pub fn new<F>(
        mut src: T,
        encoder: &str,
        encoder_options: Option<HashMap<String, String>>,
        setting_func: F,
    ) -> Result<Encoder<T>>
    where
        F: FnOnce(&mut ffmpeg::encoder::video::Video) -> Result<()>,
    {
        let codec = ffmpeg::encoder::find_by_name(encoder)
            .ok_or_else(|| anyhow!("Missing encoder {}", encoder))?;
        let codec_context = CodecContext::new_with_codec(codec);
        let mut encoder = codec_context.encoder().video()?;

        setting_func(&mut encoder)?;

        if let Some(encoder_options) = encoder_options {
            for (key, value) in encoder_options.iter() {
                info!("Setting option {key} {value}");
                unsafe { Self::set_option(encoder.as_mut_ptr(), &key, &value)? };
            }
        }

        Ok(Encoder {
            src,
            encoder: encoder.open()?,
        })
    }

    pub fn height(&self) -> u32 {
        self.encoder.height()
    }

    pub fn width(&self) -> u32 {
        self.encoder.width()
    }
    pub fn dimensions(&self) -> (u32, u32) {
        return (self.height(), self.width());
    }

    unsafe fn set_option(context: *mut AVCodecContext, name: &str, val: &str) -> Result<()> {
        let name_c = CString::new(name).context("Error in CString")?;
        let val_c = CString::new(val).context("Error in CString")?;
        let retval: i32 = ffmpeg::ffi::av_opt_set(
            context as *mut c_void,
            name_c.as_ptr(),
            val_c.as_ptr(),
            ffmpeg::ffi::AV_OPT_SEARCH_CHILDREN,
        );
        if retval != 0 {
            bail!("set_option failed: {retval}");
        }
        Ok(())
    }
}

impl<T> Iterator for Encoder<T>
where
    T: Iterator<Item = Result<frame::Video>>,
{
    type Item = Result<EncodedPacket>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut packet = Packet::empty();
        loop {
            match self.encoder.receive_packet(&mut packet) {
                Ok(_) => return Some(Ok(EncodedPacket(packet, Instant::now()))),
                Err(Error::Other { errno }) if errno == EAGAIN => {}
                Err(e) => return Some(Err(e.into())),
            };
            let next_frame = match self.src.next()? {
                Err(e) => return Some(Err(e)),
                Ok(f) => f,
            };

            if let Err(e) = self.encoder.send_frame(&next_frame) {
                return Some(Err(e.into()));
            }
        }
    }
}
