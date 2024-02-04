use std::sync::Arc;

use bitvec::prelude::*;
use drivers_shared::lpm013m1126c::{self, Buffer};
use smol::lock::Mutex;

pub struct Window {
    pub window: minifb::Window,
    window_buffer: [u32; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
}

pub type WindowHandle = Arc<Mutex<Window>>;

impl Window {
    pub fn new() -> Self {
        let window = minifb::Window::new(
            "simu",
            lpm013m1126c::WIDTH,
            lpm013m1126c::HEIGHT,
            Default::default(),
        )
        .unwrap();

        Self {
            window,
            window_buffer: [0; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
        }
    }
    pub fn present(&mut self, buffer: &mut Buffer) {
        if let Some(buf) = buffer.lines_for_update() {
            let buf = &buf[..(buf.len() - lpm013m1126c::NUM_REQUIRED_SUFFIX_BYTES)];
            for line in buf.chunks(lpm013m1126c::NUM_BYTES_PER_ROW) {
                assert_eq!(line.len(), lpm013m1126c::NUM_BYTES_PER_ROW);

                assert_eq!(line[0], 0b100100_00);
                let line_num = (line[1] - 1) as usize;

                let line_in = &line[2..];
                let line_in = line_in.view_bits::<Lsb0>().chunks(4);
                let line_out = &mut self.window_buffer[lpm013m1126c::WIDTH * line_num..]
                    [..lpm013m1126c::WIDTH];
                for (in_, out_) in line_in.zip(line_out.iter_mut()) {
                    let r = (in_[1] as u32) * 0xff;
                    let g = (in_[2] as u32) * 0xff;
                    let b = (in_[3] as u32) * 0xff;
                    *out_ = r << 16 | g << 8 | b;
                }
            }

            self.window
                .update_with_buffer(
                    &self.window_buffer,
                    lpm013m1126c::WIDTH,
                    lpm013m1126c::HEIGHT,
                )
                .unwrap();
        }
    }
}
