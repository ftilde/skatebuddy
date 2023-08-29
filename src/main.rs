#![no_main]
#![no_std]

use bitmap_font::TextStyle;
use defmt_rtt as _;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
// global logger
use nrf52840_hal as _; // memory layout
use nrf52840_hal::{gpio::Level, prelude::*};

use crate::lpm013m1126c::Rgb111;

mod lpm013m1126c;
mod util;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let loc = info.location().unwrap();
    defmt::error!("panicked {}:{}", loc.file(), loc.line());
    util::exit()
}

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::info!("we start, then loop");

    let core_peripherals = nrf52840_hal::pac::CorePeripherals::take().unwrap();
    let peripherals = nrf52840_hal::pac::Peripherals::take().unwrap();
    let p0 = nrf52840_hal::gpio::p0::Parts::new(peripherals.P0);
    let _button = p0.p0_17.into_pullup_input();
    let mut backlight = p0.p0_08.into_push_pull_output(Level::Low);
    let mut delay = nrf52840_hal::Delay::new(core_peripherals.SYST);

    let lcd = lpm013m1126c::Controller::new(
        p0.p0_05.into(),
        p0.p0_06.into(),
        p0.p0_07.into(),
        p0.p0_26.into(),
        p0.p0_27.into(),
        &mut delay,
    );

    let mut lcd = lpm013m1126c::Display::new(lcd);

    let mut s = PinState::Low;
    backlight.set_state(PinState::High).unwrap();
    //let font = bitmap_font::tamzen::FONT_20x40.pixel_double();
    let font = bitmap_font::tamzen::FONT_20x40_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);
    loop {
        lcd.fill(Rgb111::black());
        //Circle::new(Point::new(5, i), 40)
        //    .into_styled(
        //        PrimitiveStyleBuilder::new()
        //            .stroke_color(Rgb111::white())
        //            .stroke_width(1)
        //            .fill_color(Rgb111::blue())
        //            .build(),
        //    )
        //    .draw(&mut lcd)
        //    .unwrap();
        Text::new("Hello Rust!", Point::new(0, 0), style)
            .draw(&mut lcd.binary())
            .unwrap();
        lcd.present();

        util::delay_micros(10_000);
        //delay.delay_us(100_000u32);
        s = util::flip(s);
        //backlight.set_state(s).unwrap();
    }
    //println!("Hello, world!");
}
