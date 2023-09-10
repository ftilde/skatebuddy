use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use embedded_hal::{
    blocking::delay::DelayUs,
    digital::v2::{OutputPin, PinState},
};

const WIDTH: usize = 176;
const HEIGHT: usize = 176;

pub struct Controller<SPI, EXTCOMIN, DISP> {
    spi: SPI,
    extcomin: EXTCOMIN,
    disp: DISP,
    ext_com_val: PinState,
}

impl<SPI: embedded_hal_async::spi::SpiDevice, EXTCOMIN: OutputPin, DISP: OutputPin>
    Controller<SPI, EXTCOMIN, DISP>
{
    pub fn new<D: DelayUs<u32>>(
        spi: SPI,
        mut extcomin: EXTCOMIN,
        mut disp: DISP,
        delay: &mut D,
    ) -> Self {
        let _ = extcomin.set_low();
        let _ = disp.set_low();
        let mut s = Self {
            spi,
            extcomin,
            disp,
            ext_com_val: PinState::High,
        };

        delay.delay_us(1_000u32);
        let _ = s.extcomin.set_state(s.ext_com_val);
        let _ = s.disp.set_high();
        delay.delay_us(200u32);
        s
    }
    pub fn ext_com_flip(&mut self) {
        self.ext_com_val = !self.ext_com_val;
        let _ = self.extcomin.set_state(self.ext_com_val);
    }
}

const NUM_PIXELS_PER_CELL: usize = 2;
const NUM_BITS_PER_PIXEL: usize = 4;
const PIXEL_MASK: u8 = (1 << NUM_BITS_PER_PIXEL) - 1 as u8;

const NUM_PREFIX_BYTES_PER_ROW: usize = 2;
const NUM_BYTES_PER_ROW: usize = WIDTH / NUM_PIXELS_PER_CELL + NUM_PREFIX_BYTES_PER_ROW;

struct Buffer {
    values: [u8; NUM_BYTES_PER_ROW * HEIGHT],
}

impl Default for Buffer {
    fn default() -> Self {
        let mut ret = Self {
            values: core::array::from_fn(|_| 0),
        };

        // Init the first two bytes of every row in the buffer with:
        // - the mode
        // - the addr
        //
        // By setting the mode in every line, we can actually select a slice of rows for the update
        // via the spi device. All are valid start points for the multiple line update mode!
        //
        // In the following we assume that the spi device is set to MSB mode.
        let mode_select = 0b100100_00;
        for r in 0..HEIGHT {
            let addr = (r + 1) as u8;
            let offset = r * NUM_BYTES_PER_ROW;
            ret.values[offset] = mode_select;
            ret.values[offset + 1] = addr;
        }
        ret
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

        let col_cell = col / NUM_PIXELS_PER_CELL;

        let cell_idx = row * NUM_BYTES_PER_ROW + col_cell + NUM_PREFIX_BYTES_PER_ROW;

        // The cell with lower column is actually in the high nibble since the spi device (is
        // assumed to) send(s) bytes in msb order.
        let in_cell_idx = 1 - (col % NUM_PIXELS_PER_CELL);

        let v = &mut self.values[cell_idx];
        let shift_amt = NUM_PIXELS_PER_CELL * in_cell_idx;
        let mask = PIXEL_MASK << shift_amt;
        *v = (*v & !mask) | (val.0 << shift_amt);
    }
    fn fill(&mut self, val: Rgb111) {
        let nv = val.0 | val.0 << NUM_BITS_PER_PIXEL;
        for r in 0..HEIGHT {
            let row_begin = r * NUM_BYTES_PER_ROW + NUM_PREFIX_BYTES_PER_ROW;
            let row_end = (r + 1) * NUM_BYTES_PER_ROW;
            let row = &mut self.values[row_begin..row_end];
            for v in row {
                *v = nv;
            }
        }
    }
}

pub struct Display<SPI, EXTCOMIN, DISP> {
    buffer: Buffer,
    c: Controller<SPI, EXTCOMIN, DISP>,
}

impl<SPI: embedded_hal_async::spi::SpiDevice, EXTCOMIN: OutputPin, DISP: OutputPin>
    Display<SPI, EXTCOMIN, DISP>
{
    pub fn new(c: Controller<SPI, EXTCOMIN, DISP>) -> Self {
        Self {
            c,
            buffer: Default::default(),
        }
    }
    pub fn fill(&mut self, val: Rgb111) {
        self.buffer.fill(val);
    }
    pub async fn present(&mut self) {
        self.c.spi.write(&self.buffer.values).await.unwrap();
        self.c.ext_com_flip();
    }

    pub fn binary(&mut self, config: BWConfig) -> DisplayBW<SPI, EXTCOMIN, DISP> {
        DisplayBW {
            inner: self,
            config,
        }
    }
}

#[derive(Copy, Clone)]
pub struct BWConfig {
    pub on: Rgb111,
    pub off: Rgb111,
}

pub struct DisplayBW<'a, SPI, EXTCOMIN, DISP> {
    inner: &'a mut Display<SPI, EXTCOMIN, DISP>,
    config: BWConfig,
}

impl<SPI, EXTCOMIN, DISP> OriginDimensions for Display<SPI, EXTCOMIN, DISP> {
    fn size(&self) -> Size {
        Size {
            width: WIDTH as _,
            height: HEIGHT as _,
        }
    }
}

impl<SPI, EXTCOMIN, DISP> DrawTarget for Display<SPI, EXTCOMIN, DISP> {
    type Color = Rgb111;

    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels {
            self.buffer.set(pos.y, pos.x, color);
        }
        Ok(())
    }
}

impl<SPI, EXTCOMIN, DISP> OriginDimensions for DisplayBW<'_, SPI, EXTCOMIN, DISP> {
    fn size(&self) -> Size {
        self.inner.size()
    }
}

impl<SPI, EXTCOMIN, DISP> DrawTarget for DisplayBW<'_, SPI, EXTCOMIN, DISP> {
    type Color = BinaryColor;

    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        self.inner.draw_iter(pixels.into_iter().map(|Pixel(p, v)| {
            Pixel(
                p,
                match v {
                    BinaryColor::Off => self.config.off,
                    BinaryColor::On => self.config.on,
                },
            )
        }))
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
        Self(0b0000)
    }
    pub fn red() -> Self {
        Self(0b1000)
    }
    pub fn green() -> Self {
        Self(0b0100)
    }
    pub fn blue() -> Self {
        Self(0b0010)
    }
    pub fn yellow() -> Self {
        Self(0b1100)
    }
    pub fn purple() -> Self {
        Self(0b1010)
    }
    pub fn cyan() -> Self {
        Self(0b0110)
    }
    pub fn white() -> Self {
        Self(0b1110)
    }
}
