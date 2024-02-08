use std::sync::Arc;

use bitvec::prelude::*;
use drivers_shared::lpm013m1126c::{self, Buffer};
use std::sync::Mutex;

pub struct Window {
    pub window: minifb::Window,
    window_buffer: [u32; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
    pub backlight_on: bool,
    pub display_on: bool,
    pub blink_mode: lpm013m1126c::BlinkMode,
}

pub type WindowHandle = Arc<Mutex<Window>>;

impl Window {
    pub fn new() -> Self {
        let window = minifb::Window::new(
            "skatebuddy-simulator",
            lpm013m1126c::WIDTH,
            lpm013m1126c::HEIGHT,
            Default::default(),
        )
        .unwrap();

        Self {
            window,
            window_buffer: [0; lpm013m1126c::WIDTH * lpm013m1126c::HEIGHT],
            backlight_on: false,
            display_on: true,
            blink_mode: lpm013m1126c::BlinkMode::Normal,
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
                let line_in = line_in.view_bits::<Msb0>().chunks(4);
                let line_out = &mut self.window_buffer[lpm013m1126c::WIDTH * line_num..]
                    [..lpm013m1126c::WIDTH];
                for (in_, out_) in line_in.zip(line_out.iter_mut()) {
                    let r = (in_[0] as u32) * 0xff;
                    let g = (in_[1] as u32) * 0xff;
                    let b = (in_[2] as u32) * 0xff;
                    *out_ = r << 16 | g << 8 | b;
                }
            }

            self.update_window();
        }
    }

    pub fn clear_buffer(&mut self) {
        self.window_buffer.fill(0);
        self.update_window();
    }
    pub fn update_window(&mut self) {
        let mult = match (self.display_on, self.backlight_on) {
            (false, _) => 0,
            (true, false) => 0x7f,
            (true, true) => 0xff,
        };
        let display_buffer = self
            .window_buffer
            .iter()
            .map(|v| {
                let v = match self.blink_mode {
                    lpm013m1126c::BlinkMode::Black => 0,
                    lpm013m1126c::BlinkMode::White => 0xffffff,
                    lpm013m1126c::BlinkMode::Inverted => 0xffffff - v,
                    lpm013m1126c::BlinkMode::Normal => *v,
                };
                v * mult / 0xff
            })
            .collect::<Vec<_>>();

        self.window
            .update_with_buffer(&display_buffer, lpm013m1126c::WIDTH, lpm013m1126c::HEIGHT)
            .unwrap();
    }
}
