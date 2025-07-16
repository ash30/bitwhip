use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use std::io::Write;
use std::sync::mpsc;
use tracing::error;

pub fn render_video(rx: mpsc::Receiver<ffmpeg_next::frame::Video>) {
    match rx.recv() {
        Ok(first_frame) => {
            let sdl_context = sdl2::init().unwrap();
            let video_subsystem = sdl_context.video().unwrap();
            let window = video_subsystem
                .window("bitwhip", first_frame.width() / 2, first_frame.height() / 2)
                .position_centered()
                .allow_highdpi()
                .build()
                .unwrap();

            let mut canvas = window.into_canvas().build().unwrap();
            let mut event_pump = sdl_context.event_pump().unwrap();
            let texture_creator = canvas.texture_creator();
            let mut texture = texture_creator
                .create_texture_streaming(
                    PixelFormatEnum::IYUV,
                    first_frame.width(),
                    first_frame.height(),
                )
                .map_err(|e| e.to_string())
                .expect("No error");

            'running: loop {
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. }
                        | Event::KeyDown {
                            keycode: Some(Keycode::Escape),
                            ..
                        } => break 'running,
                        _ => {}
                    }
                }

                let res = texture
                    .with_lock(None, |mut buffer: &mut [u8], _pitch: usize| {
                        match rx.try_recv() {
                            Ok(frame) => unsafe {
                                let Some(desc) = frame.format().descriptor() else {
                                    return false;
                                };
                                let frame_ptr = *frame.as_ptr();

                                // Copy to buffer, trim padding
                                for p in 0..frame.planes() {
                                    frame
                                        .data(p)
                                        .chunks_exact(frame_ptr.linesize[p] as usize)
                                        .for_each(|row| {
                                            let scale = match p {
                                                0 => 0,
                                                _ => desc.log2_chroma_w(),
                                            };
                                            let (a, _) = row.split_at(
                                                ((frame.width() + (1 << scale) - 1) >> scale as u32)
                                                    as usize,
                                            );
                                            if let Err(e) = buffer.write(a) {
                                                error!("Error writing frame to texture: {}", e)
                                            }
                                        });
                                }
                                true
                            },
                            Err(_err) => false,
                        }
                    })
                    .expect("texture copy");

                if res {
                    canvas.clear();
                    canvas.copy(&texture, None, None).expect("No error");
                    canvas.present();
                }
            }
        }
        Err(_err) => {}
    }
}
