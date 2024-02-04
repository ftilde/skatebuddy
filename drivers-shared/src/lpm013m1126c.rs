use core::ops::Range;

use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

pub const WIDTH: usize = 176;
pub const HEIGHT: usize = 176;

pub const SPI_MODE: embedded_hal::spi::Mode = embedded_hal::spi::Mode {
    polarity: embedded_hal::spi::Polarity::IdleLow,
    phase: embedded_hal::spi::Phase::CaptureOnFirstTransition,
};

const NUM_PIXELS_PER_CELL: usize = 2;
const NUM_BITS_PER_PIXEL: usize = 4;
const PIXEL_MASK: u8 = (1 << NUM_BITS_PER_PIXEL) - 1 as u8;

const NUM_PREFIX_BYTES_PER_ROW: usize = 2;
pub const NUM_BYTES_PER_ROW: usize = WIDTH / NUM_PIXELS_PER_CELL + NUM_PREFIX_BYTES_PER_ROW;
pub const NUM_REQUIRED_SUFFIX_BYTES: usize = 2; // We need 16 more clock cycles after the last row.

pub struct Buffer {
    min_row: u8,
    max_row: u8,
    values: [u8; NUM_BYTES_PER_ROW * HEIGHT + NUM_REQUIRED_SUFFIX_BYTES],
}

impl Default for Buffer {
    fn default() -> Self {
        let mut ret = Self {
            min_row: u8::MAX,
            max_row: 0,
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
    pub fn set(&mut self, row: i32, col: i32, val: Rgb111) {
        if row < 0 || col < 0 {
            return;
        }
        let row = row as usize;
        let col = col as usize;

        if row >= HEIGHT || col >= WIDTH {
            return;
        }

        self.min_row = self.min_row.min(row as u8);
        self.max_row = self.max_row.max(row as u8);

        let col_cell = col / NUM_PIXELS_PER_CELL;

        let cell_idx = row * NUM_BYTES_PER_ROW + col_cell + NUM_PREFIX_BYTES_PER_ROW;

        // The cell with lower column is actually in the high nibble since the spi device (is
        // assumed to) send(s) bytes in msb order.
        let in_cell_idx = 1 - (col % NUM_PIXELS_PER_CELL);

        let v = &mut self.values[cell_idx];
        let shift_amt = NUM_BITS_PER_PIXEL * in_cell_idx;
        let mask = PIXEL_MASK << shift_amt;
        *v = (*v & !mask) | (val.0 << shift_amt);
    }
    pub fn fill_lines(&mut self, val: Rgb111, lines: Range<usize>) {
        self.min_row = self.min_row.min(lines.start as u8);
        self.max_row = self.max_row.max((lines.end - 1) as u8);

        let nv = val.0 | val.0 << NUM_BITS_PER_PIXEL;
        assert!(lines.end <= HEIGHT);
        for r in lines {
            let row_begin = r * NUM_BYTES_PER_ROW + NUM_PREFIX_BYTES_PER_ROW;
            let row_end = (r + 1) * NUM_BYTES_PER_ROW;
            let row = &mut self.values[row_begin..row_end];
            for v in row {
                *v = nv;
            }
        }
    }
    pub fn fill(&mut self, val: Rgb111) {
        self.fill_lines(val, 0..HEIGHT);
    }

    pub fn binary<'b>(&'b mut self, config: BWConfig) -> BufferBW<'b> {
        BufferBW {
            inner: self,
            config,
        }
    }

    pub fn lines_for_update(&mut self) -> Option<&[u8]> {
        if self.max_row < self.min_row {
            return None;
        }

        let begin = self.min_row as usize * NUM_BYTES_PER_ROW;
        let end = (self.max_row as usize + 1) * NUM_BYTES_PER_ROW + NUM_REQUIRED_SUFFIX_BYTES;

        self.min_row = u8::MAX;
        self.max_row = 0;

        Some(&self.values[begin..end])
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
#[allow(unused)]
pub enum BlinkMode {
    Black = 0b000100_00,
    White = 0b000110_00,
    Inverted = 0b000101_00,
    Normal = 0b000000_00,
}

pub async fn blink<'a, SPI: embedded_hal_async::spi::SpiDevice>(spi: &mut SPI, mode: BlinkMode) {
    let buffer = [mode as u8, 0]; // Dummy byte to send for at least 16 cycles

    spi.transaction(&mut [
        embedded_hal_async::spi::Operation::Write(&buffer),
        embedded_hal_async::spi::Operation::DelayNs(10_000),
    ])
    .await
    .unwrap();
}

pub async fn clear<'a, SPI: embedded_hal_async::spi::SpiDevice>(spi: &mut SPI) {
    let cmd = 0b001000_00;
    let buffer = [cmd, 0]; // Dummy byte to send for at least 16 cycles

    spi.transaction(&mut [
        embedded_hal_async::spi::Operation::Write(&buffer),
        embedded_hal_async::spi::Operation::DelayNs(10_000),
    ])
    .await
    .unwrap();
}

#[derive(Copy, Clone)]
pub struct BWConfig {
    pub on: Rgb111,
    pub off: Rgb111,
}

pub struct BufferBW<'a> {
    inner: &'a mut Buffer,
    config: BWConfig,
}

impl OriginDimensions for Buffer {
    fn size(&self) -> Size {
        Size {
            width: WIDTH as _,
            height: HEIGHT as _,
        }
    }
}

impl DrawTarget for Buffer {
    type Color = Rgb111;

    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for Pixel(pos, color) in pixels {
            self.set(pos.y, pos.x, color);
        }
        Ok(())
    }
}

impl<'a> OriginDimensions for BufferBW<'a> {
    fn size(&self) -> Size {
        self.inner.size()
    }
}

impl<'a> DrawTarget for BufferBW<'a> {
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

impl Default for Rgb111 {
    fn default() -> Self {
        Self::black()
    }
}

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
