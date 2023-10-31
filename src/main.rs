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

mod accel;
mod battery;
mod display;
mod gps;
mod hardware;
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
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI1>;
    UARTE0_UART0 => embassy_nrf::buffered_uarte::InterruptHandler<gps::UartInstance>;
});

static TOUCH_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[embassy_executor::task]
async fn touch_task(
    twim: embassy_nrf::peripherals::TWISPI0,
    touch_sda: hardware::touch::SDA,
    touch_scl: hardware::touch::SCL,
    touch_reset: hardware::touch::RST,
    touch_int: hardware::touch::IRQ,
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

        let touch_addr = 0x15; //TODO: use config

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

#[embassy_executor::task]
async fn accel_task(
    twim: embassy_nrf::peripherals::TWISPI1,
    accel_sda: hardware::accel::SDA,
    accel_scl: hardware::accel::SCL,
) {
    let mut accel = crate::accel::AccelRessources::new(twim, accel_sda, accel_scl);

    let config = accel::Config::new();
    let mut accel = accel.on(config).await;

    let mut ticker = Ticker::every(Duration::from_secs(1));
    for _ in 0..10 {
        let reading = accel.reading_nf().await;
        let reading_hf = accel.reading_hf().await;

        defmt::println!(
            "Accel: x: {}, y: {}, z: {}, xh: {}, yh: {}, zh: {}",
            reading.x,
            reading.y,
            reading.z,
            reading_hf.x,
            reading_hf.y,
            reading_hf.z
        );
        ticker.next().await;
    }
}

async fn render_top_bar(
    lcd: &mut display::Display<'_>,
    battery: &mut battery::Battery<'_>,
    //battery: &mut battery::AccurateBatteryReader,
    charge_state: &battery::BatteryChargeState<'_>,
) {
    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };
    lcd.fill_lines(bw_config.off, 0..16);

    //let font = embedded_graphics::mono_font::ascii::FONT_9X18;
    //let style = embedded_graphics::mono_font::MonoTextStyle::new(
    //    &font,
    //    embedded_graphics::pixelcolor::BinaryColor::On,
    //);
    let font = bitmap_font::tamzen::FONT_8x16_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let (time_str, date_str) = if let Some(c) = time::now_local() {
        use chrono::{Datelike, Timelike};
        (
            arrform!(8, "{:0>2}:{:0>2}:{:0>2}", c.hour(), c.minute(), c.second()),
            arrform!(5, "{:0>2}.{:0>2}", c.day(), c.month(),),
        )
    } else {
        (arrform!(8, "??:??:??"), arrform!(5, "??.??"))
    };

    let v = battery.read().await;

    let bat = arrform!(
        4,
        "{}{:0>2}",
        match charge_state.read() {
            battery::ChargeState::Full => 'F',
            battery::ChargeState::Charging => 'C',
            battery::ChargeState::Draining => 'D',
        },
        v.percentage() as i32
    );
    let text = arrform!(
        { 8 + 5 + 4 + 6 },
        "{}   {}   {}",
        date_str.as_str(),
        time_str.as_str(),
        bat.as_str()
    );
    Text::new(text.as_str(), Point::new(0, 0), style)
        .draw(&mut lcd.binary(bw_config))
        .unwrap();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(conf);

    let mut battery = battery::Battery::new(p.SAADC, p.P0_03);
    let mut current_reader = { battery::CurrentEstimator::init(battery.read_accurate().await) };

    let _hrm_power = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);
    let _flash_cs = Output::new(p.P0_14, Level::High, OutputDrive::Standard);

    // Explicitly "disconnect" the following devices' i2c pins
    // Heartrate
    let _unused = Input::new(p.P0_24, Pull::None);
    let _unused = Input::new(p.P1_00, Pull::None);

    // Mag
    let _unused = Input::new(p.P1_12, Pull::None);
    let _unused = Input::new(p.P1_13, Pull::None);

    // pressure
    let _unused = Input::new(p.P1_15, Pull::None);
    let _unused = Input::new(p.P0_02, Pull::None);

    //TODO: figure out flash

    //let mut battery = battery::AccurateBatteryReader::new(&spawner, battery);
    let battery_charge_state = battery::BatteryChargeState::new(p.P0_23, p.P0_25);

    let mut button = Input::new(p.P0_17, Pull::Up);

    let mut lcd = display::setup(
        &spawner, p.SPI3, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27,
    );

    spawner
        .spawn(touch_task(p.TWISPI0, p.P1_01, p.P1_02, p.P1_03, p.P1_04))
        .unwrap();
    spawner
        .spawn(accel_task(p.TWISPI1, p.P1_06, p.P1_05))
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

    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };

    let mut ticker = Ticker::every(Duration::from_secs(60));

    loop {
        let v = battery.read_accurate().await;
        let mua = current_reader.next(v);
        let mdev = current_reader.deviation();

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

        render_top_bar(&mut lcd, &mut battery, &battery_charge_state).await;

        let now = begin.elapsed();
        let seconds = now.as_secs();

        let sec_clock = seconds % 60;
        let minutes = seconds / 60;
        let min_clock = minutes % 60;
        let hours = minutes / 60;

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", hours, min_clock, sec_clock);
        Text::new(text.as_str(), Point::new(0, 80), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        //let c = TOUCH_COUNTER.load(core::sync::atomic::Ordering::SeqCst);
        //let mua = battery.current();
        let text = arrform!(20, "c: {}muA", mua.micro_ampere());
        Text::new(text.as_str(), Point::new(0, 30), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();
        let text = arrform!(20, "s: {}muA", mdev.micro_ampere());
        Text::new(text.as_str(), Point::new(0, 50), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        let text = arrform!(
            20,
            "{} V: {}",
            match battery_charge_state.read() {
                battery::ChargeState::Full => 'F',
                battery::ChargeState::Charging => 'C',
                battery::ChargeState::Draining => 'D',
            },
            v.voltage()
        );
        Text::new(text.as_str(), Point::new(0, 100), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        lcd.present().await;

        embassy_futures::select::select(ticker.next(), button.wait_for_any_edge()).await;

        if button.is_low() {
            let _ = backlight.set_high();
            let v = battery.read_accurate().await;
            current_reader.reset(v)
        } else {
            let _ = backlight.set_low();
        }
    }
    //println!("Hello, world!");
}
