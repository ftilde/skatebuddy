#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

// Potentially useful in the future:
//
// Layout
// https://crates.io/crates/embedded-layout
//
// Text boxes
// https://crates.io/crates/embedded-text
//
// Plots
// https://crates.io/crates/embedded-plots

use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;

use defmt_rtt as _; //logger
use nrf52840_hal as _; // memory layout
use panic_probe as _; //panic handler

//use nrf52840_hal::{gpio::Level, prelude::*};

use crate::lpm013m1126c::Rgb111;

mod lpm013m1126c;
mod util;

use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::v2::{OutputPin, PinState};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let cs = Output::new(p.P0_05, Level::Low, OutputDrive::Standard);
    let extcomin = Output::new(p.P0_06, Level::Low, OutputDrive::Standard);
    let disp = Output::new(p.P0_07, Level::Low, OutputDrive::Standard);
    let sck = Output::new(p.P0_26, Level::Low, OutputDrive::Standard);
    let mosi = Output::new(p.P0_27, Level::Low, OutputDrive::Standard);

    let mut backlight = Output::new(p.P0_08, Level::Low, OutputDrive::Standard);

    let mut delay = embassy_time::Delay;
    let lcd = lpm013m1126c::Controller::new(cs, extcomin, disp, sck, mosi, &mut delay);

    let mut lcd = lpm013m1126c::Display::new(lcd);

    let mut s = PinState::Low;
    backlight.set_state(PinState::Low).unwrap();
    //let font = bitmap_font::tamzen::FONT_20x40.pixel_double();
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let begin = Instant::now();
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

        let now = begin.elapsed();
        let seconds = now.as_secs();

        let sec_clock = seconds % 60;
        let minutes = seconds / 60;
        let min_clock = minutes % 60;
        let hours = minutes / 60;

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", hours, min_clock, sec_clock);
        Text::new(text.as_str(), Point::new(0, 0), style)
            .draw(&mut lcd.binary())
            .unwrap();

        let text = arrform!(20, "c: {}", now.as_ticks());
        Text::new(text.as_str(), Point::new(0, 50), style)
            .draw(&mut lcd.binary())
            .unwrap();
        lcd.present();

        let interval = Duration::from_millis(10_000);
        Timer::after(interval).await;
        s = util::flip(s);
        //backlight.set_state(s).unwrap();
    }
    //println!("Hello, world!");
}
