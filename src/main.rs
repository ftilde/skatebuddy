#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]

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

mod battery;
mod display;
mod gps;
mod lpm013m1126c;
mod time;
mod util;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    saadc, spim, twim,
};
use embassy_time::{Duration, Instant, Ticker, Timer};
use embedded_hal::digital::v2::{OutputPin, PinState};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
    SPIM3 => spim::InterruptHandler<embassy_nrf::peripherals::SPI3>;
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
    UARTE0_UART0 => embassy_nrf::buffered_uarte::InterruptHandler<gps::UartInstance>;
});

static TOUCH_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[embassy_executor::task]
async fn touch_task(
    twim: embassy_nrf::peripherals::TWISPI0,
    touch_sda: embassy_nrf::peripherals::P1_01,
    touch_scl: embassy_nrf::peripherals::P1_02,
    touch_reset: embassy_nrf::peripherals::P1_03,
    touch_int: embassy_nrf::peripherals::P1_04,
) {
    let mut touch_reset = Output::new(touch_reset, Level::Low, OutputDrive::Standard);
    let mut touch_int = Input::new(touch_int, Pull::None);

    let config = twim::Config::default();
    let mut i2c = twim::Twim::new(twim, Irqs, touch_sda, touch_scl, config);

    loop {
        touch_reset.set_low();
        Timer::after(Duration::from_millis(20)).await;
        touch_reset.set_high();
        Timer::after(Duration::from_millis(200)).await;

        let touch_addr = 0x15;

        //This is something else, but used in espruino.
        //Mabye this is actual sleep?
        let reg_addr = 0xE5;

        ////This is sleep mode according to official example code (but looks like standby???
        //let reg_addr = 0xA5;

        let reg_val = 0x03;
        let buf = [reg_addr, reg_val];
        i2c.write(touch_addr, &buf).await.unwrap();

        touch_int.wait_for_low().await;

        let prev = TOUCH_COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        defmt::println!("Got a touch event! {}", prev);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(conf);

    let mut config = spim::Config::default();
    config.frequency = spim::Frequency::M4;
    config.mode = spim::Mode {
        polarity: spim::Polarity::IdleLow,
        phase: spim::Phase::CaptureOnFirstTransition,
    };

    let _hrm_power = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);
    //let _gps_power = Output::new(p.P0_29, Level::Low, OutputDrive::Standard);
    let _flash_cs = Output::new(p.P0_14, Level::High, OutputDrive::Standard);

    //TODO: figure out accelerometer (maybe some power draw?)
    //TODO: figure out flash

    let mut battery = battery::Battery::new(p.SAADC, p.P0_03, p.P0_23, p.P0_25);

    let button = Input::new(p.P0_17, Pull::Up);

    let mut lcd = display::setup(
        &spawner, p.SPI3, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27,
    );

    spawner
        .spawn(touch_task(p.TWISPI0, p.P1_01, p.P1_02, p.P1_03, p.P1_04))
        .unwrap();

    let _gps = gps::GPSRessources::new(
        p.P0_29,
        p.P0_31,
        p.P0_30,
        p.UARTE0,
        p.TIMER1,
        p.PPI_CH1,
        p.PPI_CH2,
        p.PPI_GROUP1,
    );
    spawner.spawn(time::clock_sync_task(_gps)).unwrap();

    let mut backlight = Output::new(p.P0_08, Level::Low, OutputDrive::Standard);

    backlight.set_state(PinState::Low).unwrap();
    //let font = bitmap_font::tamzen::FONT_20x40.pixel_double();
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let begin = Instant::now();

    let mut ticker = Ticker::every(Duration::from_secs(60));

    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };

    loop {
        let v = battery.read().await;

        lcd.fill(bw_config.off);
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

        if let Some(c) = time::now_local() {
            use chrono::{Datelike, Timelike};
            let text = arrform!(
                20,
                "{:0>2}.{:0>2}.{:0>4}\n{:0>2}:{:0>2}:{:0>2}",
                c.day(),
                c.month(),
                c.year(),
                c.hour(),
                c.minute(),
                c.second()
            );
            Text::new(text.as_str(), Point::new(0, 0), style)
                .draw(&mut lcd.binary(bw_config))
                .unwrap();
        } else {
            Text::new("Time not synced, yet", Point::new(0, 0), style)
                .draw(&mut lcd.binary(bw_config))
                .unwrap();
        }

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", hours, min_clock, sec_clock);
        Text::new(text.as_str(), Point::new(0, 50), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        //let c = TOUCH_COUNTER.load(core::sync::atomic::Ordering::SeqCst);
        //let text = arrform!(20, "c: {}", c);
        //Text::new(text.as_str(), Point::new(0, 50), style)
        //    .draw(&mut lcd.binary(bw_config))
        //    .unwrap();

        let text = arrform!(
            20,
            "{} V: {}",
            match battery.charge_state() {
                battery::ChargeState::Full => 'F',
                battery::ChargeState::Charging => 'C',
                battery::ChargeState::Draining => 'D',
            },
            v.voltage()
        );
        Text::new(text.as_str(), Point::new(0, 100), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        let text = arrform!(20, "%: {}", v.percentage());
        Text::new(text.as_str(), Point::new(0, 130), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        lcd.present().await;

        if button.is_low() {
            let _ = backlight.set_high();
        } else {
            let _ = backlight.set_low();
        }

        ticker.next().await;
    }
    //println!("Hello, world!");
}
