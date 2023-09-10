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
mod lpm013m1126c;
mod util;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    saadc, spim,
};
use embassy_time::{Duration, Instant, Ticker};
use embedded_hal::digital::v2::{OutputPin, PinState};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
    SPIM3 => spim::InterruptHandler<embassy_nrf::peripherals::SPI3>;
});

struct SpiDeviceWrapper<'a, T: embassy_nrf::spim::Instance, CS> {
    spi: embassy_nrf::spim::Spim<'a, T>,
    cs: CS,
}

impl<'a, T: embassy_nrf::spim::Instance, CS: OutputPin> embedded_hal_async::spi::ErrorType
    for SpiDeviceWrapper<'a, T, CS>
{
    type Error = embedded_hal_async::spi::ErrorKind;
}
impl<'a, T: embassy_nrf::spim::Instance, CS: OutputPin> embedded_hal_async::spi::SpiDevice
    for SpiDeviceWrapper<'a, T, CS>
{
    async fn transaction(
        &mut self,
        operations: &mut [embedded_hal_async::spi::Operation<'_, u8>],
    ) -> Result<(), embedded_hal_async::spi::ErrorKind> {
        let _ = self.cs.set_high();
        for operation in operations {
            match operation {
                embedded_hal_async::spi::Operation::Read(_) => todo!(),
                embedded_hal_async::spi::Operation::Write(buf) => {
                    self.spi.write_from_ram(buf).await
                }
                embedded_hal_async::spi::Operation::Transfer(_, _) => todo!(),
                embedded_hal_async::spi::Operation::TransferInPlace(_) => todo!(),
                embedded_hal_async::spi::Operation::DelayUs(_) => todo!(),
            }
            .map_err(|_e| embedded_hal_async::spi::ErrorKind::Other)?;
        }
        let _ = self.cs.set_low();
        Ok(())
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(conf);

    let mut config = spim::Config::default();
    config.frequency = spim::Frequency::M4;
    config.mode = spim::Mode {
        polarity: spim::Polarity::IdleLow,
        phase: spim::Phase::CaptureOnFirstTransition,
    };

    let spim = spim::Spim::new_txonly(p.SPI3, Irqs, p.P0_26, p.P0_27, config);

    let mut battery = battery::Battery::new(p.SAADC, p.P0_03, p.P0_23, p.P0_25);

    let button = Input::new(p.P0_17, Pull::Up);

    let cs = Output::new(p.P0_05, Level::Low, OutputDrive::Standard);
    let extcomin = Output::new(p.P0_06, Level::Low, OutputDrive::Standard);
    let disp = Output::new(p.P0_07, Level::Low, OutputDrive::Standard);

    let spi = SpiDeviceWrapper { spi: spim, cs };

    //let sck = Output::new(p.P0_26, Level::Low, OutputDrive::Standard);
    //let mosi = Output::new(p.P0_27, Level::Low, OutputDrive::Standard);

    let mut backlight = Output::new(p.P0_08, Level::Low, OutputDrive::Standard);

    let mut delay = embassy_time::Delay;
    let lcd = lpm013m1126c::Controller::new(spi, extcomin, disp, &mut delay);

    let mut lcd = lpm013m1126c::Display::new(lcd);

    backlight.set_state(PinState::Low).unwrap();
    //let font = bitmap_font::tamzen::FONT_20x40.pixel_double();
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let begin = Instant::now();

    let mut ticker = Ticker::every(Duration::from_secs(1));

    let bw_config = lpm013m1126c::BWConfig {
        on: Rgb111::black(),
        off: Rgb111::white(),
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

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", hours, min_clock, sec_clock);
        Text::new(text.as_str(), Point::new(0, 0), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

        let text = arrform!(20, "c: {}", now.as_ticks());
        Text::new(text.as_str(), Point::new(0, 50), style)
            .draw(&mut lcd.binary(bw_config))
            .unwrap();

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
