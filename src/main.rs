#![no_main]
#![no_std]

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

use nrf52840_hal::{gpio::Level, prelude::*};

use crate::lpm013m1126c::Rgb111;

mod lpm013m1126c;
mod util;

const RTC_RANGE: u32 = 1 << 24;

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

    let clocks = nrf52840_hal::clocks::Clocks::new(peripherals.CLOCK);
    clocks.start_lfclk();

    let mut rtc = nrf52840_hal::Rtc::new(peripherals.RTC0, (1 << 12) - 1).unwrap();
    let mut overflow_counter = 0;

    let rtc_freq = 8;
    let seconds_per_overflow = RTC_RANGE / 8;
    rtc.enable_event(nrf52840_hal::rtc::RtcInterrupt::Overflow);
    rtc.enable_counter();

    let mut lcd = lpm013m1126c::Display::new(lcd);

    let mut s = PinState::Low;
    backlight.set_state(PinState::High).unwrap();
    //let font = bitmap_font::tamzen::FONT_20x40.pixel_double();
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
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

        if rtc.is_event_triggered(nrf52840_hal::rtc::RtcInterrupt::Overflow) {
            overflow_counter += 1;
        }
        let seconds = rtc.get_counter() / rtc_freq + overflow_counter * seconds_per_overflow;

        let sec_clock = seconds % 60;
        let minutes = seconds / 60;
        let min_clock = minutes % 60;
        let hours = minutes / 60;

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", hours, min_clock, sec_clock);
        Text::new(text.as_str(), Point::new(0, 0), style)
            .draw(&mut lcd.binary())
            .unwrap();

        let text = arrform!(20, "c: {}", rtc.get_counter());
        Text::new(text.as_str(), Point::new(0, 50), style)
            .draw(&mut lcd.binary())
            .unwrap();
        let text = arrform!(20, "o: {}", overflow_counter);
        Text::new(text.as_str(), Point::new(0, 100), style)
            .draw(&mut lcd.binary())
            .unwrap();
        lcd.present();

        //util::delay_micros(10_000);
        delay.delay_ms(10_000u32);
        s = util::flip(s);
        //backlight.set_state(s).unwrap();
    }
    //println!("Hello, world!");
}
