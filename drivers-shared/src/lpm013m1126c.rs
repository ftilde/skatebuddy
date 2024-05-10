use core::ops::Range;

use bitvec::prelude::*;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, primitives::Rectangle};

pub const WIDTH: usize = 176;
pub const HEIGHT: usize = 176;
pub const DISPLAY_AREA: Rectangle =
    Rectangle::new(Point::new(0, 0), Size::new(WIDTH as _, HEIGHT as _));

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
    values: [u8; NUM_BYTES_PER_ROW * HEIGHT + NUM_REQUIRED_SUFFIX_BYTES],
    changed: BitArray<[u32; 256 / 4]>,
}

impl Default for Buffer {
    fn default() -> Self {
        let mut ret = Self {
            values: core::array::from_fn(|_| 0),
            changed: BitArray::default(),
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

        self.changed.set(row, true);

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
    pub fn set_line(&mut self, row: i32, col_begin: i32, col_end: i32, val: Rgb111) {
        if row < 0 {
            return;
        }
        let row = row as usize;
        if row >= HEIGHT {
            return;
        }
        self.changed.set(row, true);

        let mut col_begin = (col_begin.max(0) as usize).min(WIDTH);
        let col_end = (col_end.max(0) as usize).min(WIDTH);

        if col_begin % NUM_PIXELS_PER_CELL == 1 {
            self.set(row as i32, col_begin as i32, val);
            col_begin = col_begin + 1;
        }
        if col_end % NUM_PIXELS_PER_CELL == 1 {
            let last = col_end - 1;
            self.set(row as i32, last as i32, val);
        }

        let fill_val = val.0 << NUM_BITS_PER_PIXEL | val.0;
        let row_idx = row * NUM_BYTES_PER_ROW + NUM_PREFIX_BYTES_PER_ROW;

        let fill_begin_idx = row_idx + col_begin / NUM_PIXELS_PER_CELL;
        let fill_end_idx = row_idx + col_end / NUM_PIXELS_PER_CELL;

        self.values[fill_begin_idx..fill_end_idx].fill(fill_val);
    }
    pub fn fill_lines(&mut self, val: Rgb111, lines: Range<usize>) {
        for line in lines.start..lines.end {
            self.changed.set(line, true);
        }

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

    pub fn lines_for_update<'a>(&'a mut self) -> impl Iterator<Item = &'a [u8]> + 'a {
        let mut row = 0;

        let values = &self.values;
        let changed = &mut self.changed;

        core::iter::from_fn(move || {
            while !changed[row] && row < HEIGHT {
                row += 1;
            }
            let begin = row;
            while changed[row] && row < HEIGHT {
                changed.set(row, false);
                row += 1;
            }
            let end = row;
            if begin != end {
                let begin = begin as usize * NUM_BYTES_PER_ROW;
                let end = end * NUM_BYTES_PER_ROW + NUM_REQUIRED_SUFFIX_BYTES;

                Some(&values[begin..end])
            } else {
                None
            }
        })
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

impl BWConfig {
    fn map(&self, col: BinaryColor) -> Rgb111 {
        match col {
            BinaryColor::Off => self.off,
            BinaryColor::On => self.on,
        }
    }
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
    //fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    //where
    //    I: IntoIterator<Item = Self::Color>,
    //{
    //    dbg!("Cont");
    //    Ok(())
    //}
    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        for yi in 0..area.size.height {
            let col_begin = area.top_left.x;
            let col_end = col_begin + area.size.width as i32;
            self.set_line(area.top_left.y + yi as i32, col_begin, col_end, color);
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
        self.inner.draw_iter(
            pixels
                .into_iter()
                .map(|Pixel(p, v)| Pixel(p, self.config.map(v))),
        )
    }
    //fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    //where
    //    I: IntoIterator<Item = Self::Color>,
    //{
    //    self.inner
    //        .fill_contiguous(area, colors.into_iter().map(|c| self.config.map(c)))
    //}
    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.inner.fill_solid(area, self.config.map(color))
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
