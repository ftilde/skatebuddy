use super::lpm013m1126c::{self, Buffer};

use bitvec::prelude::*;

pub struct Display {
    buffer: lpm013m1126c::Buffer,
    window: minifb::Window,
    window_buffer: [u32; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
}

impl core::ops::Deref for Display {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl core::ops::DerefMut for Display {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Display {
    pub(crate) fn new(window: minifb::Window) -> Self {
        Self {
            buffer: Default::default(),
            window,
            window_buffer: [0u32; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
        }
    }
    pub fn on(&mut self) {}

    pub fn off(&mut self) {}

    pub async fn clear(&mut self) {}

    pub async fn blink(&mut self, _mode: lpm013m1126c::BlinkMode) {}

    pub async fn present(&mut self) {
        if let Some(buf) = self.buffer.lines_for_update() {
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

pub struct Backlight {}

impl Backlight {
    pub fn on(&mut self) {}

    pub fn off(&mut self) {}
}
