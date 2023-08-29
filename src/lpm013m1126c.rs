use cortex_m::prelude::*;
use embedded_graphics::prelude::*;
use nrf52840_hal::{
    gpio::{p0::*, Disconnected, Level, Output, PushPull},
    prelude::*,
    Delay,
};

const WIDTH: usize = 176;
const HEIGHT: usize = 176;

pub struct Controller {
    cs: P0_05<Output<PushPull>>,
    extcomin: P0_06<Output<PushPull>>,
    disp: P0_07<Output<PushPull>>,
    sck: P0_26<Output<PushPull>>,
    mosi: P0_27<Output<PushPull>>,
    ext_com_val: PinState,
}

#[inline]
fn spi_delay() {
    cortex_m::asm::nop();
    //cortex_m::asm::nop();
    //cortex_m::asm::nop();
    //cortex_m::asm::nop();
}

impl Controller {
    pub fn new(
        cs: P0_05<Disconnected>,
        extcomin: P0_06<Disconnected>,
        disp: P0_07<Disconnected>,
        sck: P0_26<Disconnected>,
        mosi: P0_27<Disconnected>,
        delay: &mut Delay,
    ) -> Self {
        let mut s = Self {
            cs: cs.into_push_pull_output(Level::Low),
            extcomin: extcomin.into_push_pull_output(Level::Low),
            disp: disp.into_push_pull_output(Level::Low),
            sck: sck.into_push_pull_output(Level::Low),
            mosi: mosi.into_push_pull_output(Level::Low),
            ext_com_val: PinState::High,
        };
        delay.delay_ms(1u32);
        s.extcomin.set_state(s.ext_com_val).unwrap();
        s.disp.set_high().unwrap();
        delay.delay_us(200u32);
        s
    }

    fn write_bits(&mut self, v: u32, len: u32) {
        for i in (0..len).rev() {
            let _ = self.sck.set_low();
            spi_delay();
            let _ = if (v >> i) & 1 == 0 {
                self.mosi.set_low()
            } else {
                self.mosi.set_high()
            };
            let _ = self.sck.set_high();
            spi_delay();
        }
    }

    fn write_epilog(&mut self) {
        for _ in 0..16 {
            let _ = self.sck.set_low();
            spi_delay();
            let _ = self.sck.set_high();
            spi_delay();
        }
    }

    pub fn fill(&mut self, vals: u8) {
        let _ = self.cs.set_high();
        crate::util::delay_micros(6); //CS settling time

        let v = 0b_001_001_001_001_001_001_001_001 * (vals & 0b111) as u32;

        self.write_bits(0b_100000, 6);
        self.write_bits(1, 10);
        for i in 0..176 {
            for _ in 0..22 {
                self.write_bits(v, 24);
            }
            self.write_bits(i + 2, 16);
        }

        let _ = self.sck.set_low();
        let _ = self.mosi.set_low();
        let _ = self.cs.set_low();

        self.ext_com_flip();
    }

    pub fn ext_com_flip(&mut self) {
        self.ext_com_val = crate::util::flip(self.ext_com_val);
        let _ = self.extcomin.set_state(self.ext_com_val);
    }

    //pub fn clear(&mut self) {
    //    let _ = self.cs.set_high();

    //    self.write_bits(0b_0_0_1000, 6); /*what about COM here????*/
    //    let _ = self.cs.set_low();
    //}
}

const NUM_PIXELS_PER_CELL: usize = 2;
const NUM_BITS_PER_PIXEL: usize = 4;
const PIXEL_MASK: u8 = (1 << NUM_BITS_PER_PIXEL) - 1 as u8;

struct Buffer {
    values: [u8; WIDTH * HEIGHT / NUM_PIXELS_PER_CELL],
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            values: core::array::from_fn(|_| 0),
        }
    }
}

impl Buffer {
    fn set(&mut self, row: i32, col: i32, val: Rgb111) {
        if row < 0 || col < 0 {
            return;
        }
        let row = row as usize;
        let col = col as usize;

        if row >= HEIGHT || col >= WIDTH {
            return;
        }

        let cell_idx = (row * WIDTH + col) / NUM_PIXELS_PER_CELL;
        let in_cell_idx = col % NUM_PIXELS_PER_CELL;

        let v = &mut self.values[cell_idx];
        let shift_amt = NUM_PIXELS_PER_CELL * in_cell_idx;
        let mask = PIXEL_MASK << shift_amt;
        *v = (*v & !mask) | (val.0 << shift_amt);
    }
    fn fill(&mut self, val: Rgb111) {
        let nv = val.0 | val.0 << NUM_BITS_PER_PIXEL;
        for v in &mut self.values {
            *v = nv;
        }
    }
}

impl Display {
    pub fn new(c: Controller) -> Self {
        Self {
            c,
            buffer: Default::default(),
        }
    }
    pub fn fill(&mut self, val: Rgb111) {
        self.buffer.fill(val);
    }
    pub fn present(&mut self) {
        let _ = self.c.cs.set_high();
        crate::util::delay_micros(6); //CS settling time

        self.c.write_bits(0b_100000, 6);
        self.c.write_bits(1, 10);
        for (i, row) in self
            .buffer
            .values
            .chunks(WIDTH / NUM_PIXELS_PER_CELL)
            .enumerate()
        {
            for cell in row {
                for bit_index in [2, 1, 0, 6, 5, 4] {
                    let _ = self.c.sck.set_low();
                    let _ = if (cell >> bit_index) & 1 == 0 {
                        self.c.mosi.set_low()
                    } else {
                        self.c.mosi.set_high()
                    };
                    let _ = self.c.sck.set_high();
                    //Loop is probably unrolled so we need a break before clearing sck again
                    cortex_m::asm::nop();
                }
            }
            self.c.write_bits(i as u32 + 2, 16);
        }

        let _ = self.c.sck.set_low();
        let _ = self.c.mosi.set_low();
        let _ = self.c.cs.set_low();

        self.c.ext_com_flip();
    }
}

pub struct Display {
    buffer: Buffer,
    c: Controller,
}

impl OriginDimensions for Display {
    fn size(&self) -> Size {
        Size {
            width: WIDTH as _,
            height: HEIGHT as _,
        }
    }
}

impl DrawTarget for Display {
    type Color = Rgb111;

    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels {
            self.buffer.set(pos.x, pos.y, color);
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct Rgb111(u8);

impl PixelColor for Rgb111 {
    type Raw = embedded_graphics::pixelcolor::raw::RawU4;
}

#[allow(unused)]
impl Rgb111 {
    pub fn raw(v: u8) -> Self {
        Self(v)
    }
    pub fn black() -> Self {
        Self(0b000)
    }
    pub fn red() -> Self {
        Self(0b100)
    }
    pub fn green() -> Self {
        Self(0b010)
    }
    pub fn blue() -> Self {
        Self(0b001)
    }
    pub fn yellow() -> Self {
        Self(0b110)
    }
    pub fn purple() -> Self {
        Self(0b101)
    }
    pub fn cyan() -> Self {
        Self(0b011)
    }
    pub fn white() -> Self {
        Self(0b111)
    }
}
