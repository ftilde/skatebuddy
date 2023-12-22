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
mod button;
mod display;
mod flash;
mod gps;
mod hardware;
mod lpm013m1126c;
mod mag;
mod time;
mod util;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals::TWISPI1,
    saadc, spim, twim,
};
use embassy_time::{Duration, Instant, Ticker, Timer};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
    SPIM2_SPIS2_SPI2 => spim::InterruptHandler<embassy_nrf::peripherals::SPI2>;
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI1>;
    UARTE0_UART0 => embassy_nrf::buffered_uarte::InterruptHandler<gps::UartInstance>;
});

static TOUCH_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[embassy_executor::task]
async fn touch_task(
    mut twim: embassy_nrf::peripherals::TWISPI0,
    mut touch_sda: hardware::touch::SDA,
    mut touch_scl: hardware::touch::SCL,
    touch_reset: hardware::touch::RST,
    touch_int: hardware::touch::IRQ,
) {
    let mut touch_reset = Output::new(touch_reset, Level::Low, OutputDrive::Standard);
    let mut touch_int = Input::new(touch_int, Pull::None);

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

        {
            let config = twim::Config::default();
            let mut i2c = twim::Twim::new(&mut twim, Irqs, &mut touch_sda, &mut touch_scl, config);

            let reg_val = 0x03;
            let buf = [reg_addr, reg_val];
            i2c.write(touch_addr, &buf).await.unwrap();
        }

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
    for _ in 0..1 {
        let _reading = accel.reading_nf().await;
        let _reading_hf = accel.reading_hf().await;

        //defmt::println!(
        //    "Accel: x: {}, y: {}, z: {}, xh: {}, yh: {}, zh: {}",
        //    reading.x,
        //    reading.y,
        //    reading.z,
        //    reading_hf.x,
        //    reading_hf.y,
        //    reading_hf.z
        //);
        ticker.next().await;
    }
}

async fn render_top_bar(ctx: &mut Context) {
    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };
    ctx.lcd.fill_lines(bw_config.off, 0..16);

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

    let v = ctx.battery.read().await;

    let bat = arrform!(
        4,
        "{}{:0>2}",
        match ctx.bat_state.read() {
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
        .draw(&mut ctx.lcd.binary(bw_config))
        .unwrap();
}

struct Context {
    #[allow(unused)]
    flash: flash::FlashRessources,
    bat_state: battery::BatteryChargeState<'static>,
    battery: battery::AsyncBattery,
    button: button::Button,
    backlight: display::Backlight,
    #[allow(unused)]
    //gps: gps::GPSRessources,
    lcd: display::Display,
    start_time: Instant,
    spi: embassy_nrf::peripherals::SPI2,
    mag: mag::MagRessources,
    twi1: TWISPI1,
    //dcb: cortex_m::peripheral::DCB,
}

async fn idle(ctx: &mut Context) {
    ctx.lcd.off();
    ctx.backlight.off();

    ctx.button.wait_for_press().await;
}

pub enum DisplayEvent {
    NewBatData,
}

static DISPLAY_EVENT: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    DisplayEvent,
> = embassy_sync::signal::Signal::new();

pub fn signal_display_event(event: DisplayEvent) {
    DISPLAY_EVENT.signal(event);
}

fn hours_mins_secs(d: Duration) -> (u32, u32, u32) {
    let seconds = d.as_secs();

    let sec_clock = seconds % 60;
    let minutes = seconds / 60;
    let min_clock = minutes % 60;
    let hours = minutes / 60;

    (hours as _, min_clock as _, sec_clock as _)
}

async fn display_stuff(ctx: &mut Context) {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };

    let mut ticker = Ticker::every(Duration::from_secs(60));

    ctx.lcd.on();
    //ctx.backlight.off();
    loop {
        let mua = ctx.battery.current();
        let mdev = ctx.battery.current_std();

        ctx.lcd.fill(bw_config.off);
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

        render_top_bar(ctx).await;

        let now = ctx.start_time.elapsed();

        let (h, min, s) = hours_mins_secs(Duration::from_secs(now.as_secs()));

        //let c = TOUCH_COUNTER.load(core::sync::atomic::Ordering::SeqCst);
        //let mua = battery.current();
        let text = arrform!(20, "c: {}muA", mua.micro_ampere());
        Text::new(text.as_str(), Point::new(0, 20), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();
        let text = arrform!(20, "s: {}muA", mdev.micro_ampere());
        Text::new(text.as_str(), Point::new(0, 40), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();

        let text = arrform!(20, "R: {}:{:0>2}:{:0>2}", h, min, s);
        Text::new(text.as_str(), Point::new(0, 70), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();

        //let text = arrform!(
        //    20,
        //    "{} V: {}",
        //    match ctx.bat_state.read() {
        //        battery::ChargeState::Full => 'F',
        //        battery::ChargeState::Charging => 'C',
        //        battery::ChargeState::Draining => 'D',
        //    },
        //    v.voltage()
        //);
        //Text::new(text.as_str(), Point::new(0, 90), style)
        //    .draw(&mut ctx.lcd.binary(bw_config))
        //    .unwrap();
        let text = arrform!(36, "N_F: {}", time::num_sync_fails());
        Text::new(text.as_str(), Point::new(0, 105), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();

        let (h, min, s) = hours_mins_secs(time::time_since_last_sync());
        let text = arrform!(36, "G: {}:{:0>2}:{:0>2}", h, min, s);
        Text::new(text.as_str(), Point::new(0, 125), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();

        let (_h, min, s) = hours_mins_secs(time::last_sync_duration());
        let text = arrform!(16, "T_G: {:0>2}:{:0>2}", min, s);
        Text::new(text.as_str(), Point::new(0, 145), style)
            .draw(&mut ctx.lcd.binary(bw_config))
            .unwrap();

        ctx.lcd.present(&mut ctx.spi).await;

        match embassy_futures::select::select3(
            ticker.next(),
            ctx.button.wait_for_press(),
            DISPLAY_EVENT.wait(),
        )
        .await
        {
            embassy_futures::select::Either3::First(_) => {}
            embassy_futures::select::Either3::Second(d) => {
                if d > Duration::from_secs(1) {
                    ctx.battery.reset().await;
                } else {
                    break;
                }
            }
            embassy_futures::select::Either3::Third(_event) => {
                DISPLAY_EVENT.reset();
            }
        }
    }
}

fn dump_peripheral_regs() {
    let foo = unsafe { nrf52840_hal::pac::Peripherals::steal() };
    defmt::println!("mrs: {:b}", foo.POWER.mainregstatus.read().bits());
    defmt::println!("dcd: {:b}", foo.POWER.dcdcen.read().bits());
    defmt::println!("dcd0: {:b}", foo.POWER.dcdcen0.read().bits());

    defmt::println!("spi0: {:b}", foo.SPI0.enable.read().bits());
    defmt::println!("spi1: {:b}", foo.SPI1.enable.read().bits());
    defmt::println!("spi2: {:b}", foo.SPI2.enable.read().bits());
    defmt::println!("spim0: {:b}", foo.SPIM0.enable.read().bits());
    defmt::println!("spim1: {:b}", foo.SPIM1.enable.read().bits());
    defmt::println!("spim2: {:b}", foo.SPIM2.enable.read().bits());
    defmt::println!("spim3: {:b}", foo.SPIM3.enable.read().bits());
    defmt::println!("spis0: {:b}", foo.SPIS0.enable.read().bits());
    defmt::println!("spis1: {:b}", foo.SPIS1.enable.read().bits());
    defmt::println!("spis2: {:b}", foo.SPIS2.enable.read().bits());

    defmt::println!("twi0: {:b}", foo.TWI0.enable.read().bits());
    defmt::println!("twi1: {:b}", foo.TWI1.enable.read().bits());
    defmt::println!("twim0: {:b}", foo.TWIM0.enable.read().bits());
    defmt::println!("twim1: {:b}", foo.TWIM1.enable.read().bits());
    defmt::println!("twis0: {:b}", foo.TWIS0.enable.read().bits());
    defmt::println!("twis1: {:b}", foo.TWIS1.enable.read().bits());

    defmt::println!("uart0: {:b}", foo.UART0.enable.read().bits());
    defmt::println!("uarte0: {:b}", foo.UARTE0.enable.read().bits());
    defmt::println!("uarte1: {:b}", foo.UARTE1.enable.read().bits());

    defmt::println!("radio: {:b}", foo.RADIO.power.read().bits());
    defmt::println!("radio (state): {:b}", foo.RADIO.state.read().bits());

    defmt::println!("i2s: {:b}", foo.I2S.enable.read().bits());
    defmt::println!("qspi: {:b}", foo.QSPI.enable.read().bits());
    defmt::println!("qdec: {:b}", foo.QDEC.enable.read().bits());
    defmt::println!("qdec: {:b}", foo.QDEC.enable.read().bits());

    defmt::println!("pwm0: {:b}", foo.PWM0.enable.read().bits());
    defmt::println!("pwm1: {:b}", foo.PWM1.enable.read().bits());
    defmt::println!("pwm2: {:b}", foo.PWM2.enable.read().bits());
    defmt::println!("pwm3: {:b}", foo.PWM3.enable.read().bits());

    defmt::println!("pdm: {:b}", foo.PDM.enable.read().bits());

    defmt::println!("saadc: {:b}", foo.SAADC.enable.read().bits());
    defmt::println!("usb: {:b}", foo.USBD.enable.read().bits());
    defmt::println!("aar: {:b}", foo.AAR.enable.read().bits());
    defmt::println!("ccm: {:b}", foo.CCM.enable.read().bits());
    defmt::println!("crypto: {:b}", foo.CRYPTOCELL.enable.read().bits());

    defmt::println!("nfct: {:b}", foo.NFCT.sleepstate.read().bits()); //NOT Sure if this is telling
                                                                      //us anything

    //TODO: look at gpio?
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut conf = embassy_nrf::config::Config::default();
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    conf.dcdc.reg1 = true;

    //let core_p = cortex_m::Peripherals::take().unwrap();
    let mut p = embassy_nrf::init(conf);

    // DO NOT USE! See
    // https://infocenter.nordicsemi.com/index.jsp?topic=%2Ferrata_nRF52840_Rev3%2FERR%2FnRF52840%2FRev3%2Flatest%2Fanomaly_840_195.html
    let _ = p.SPI3;

    //dump_peripheral_regs();

    let battery = battery::Battery::new(p.SAADC, p.P0_03);
    let battery = battery::AsyncBattery::new(&spawner, battery);

    // Keep hrm in reset to power it off
    let _hrm_power = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);

    // Explicitly "disconnect" the following devices' i2c pins
    // Heartrate
    let _unused = Input::new(p.P0_24, Pull::None);
    let _unused = Input::new(p.P1_00, Pull::None);

    // pressure
    let _unused = Input::new(p.P1_15, Pull::None);
    let _unused = Input::new(p.P0_02, Pull::None);

    //let _flash_cs = Output::new(p.P0_14, Level::High, OutputDrive::Standard);
    let _vibrate = Output::new(p.P0_19, Level::Low, OutputDrive::Standard);

    let flash = flash::FlashRessources::new(&mut p.SPI2, p.P0_14, p.P0_16, p.P0_15, p.P0_13).await;
    {
        //let addr = 0;
        //let mut f = flash.on(&mut p.SPI2).await;

        //let mut buf = [0xab; 4];
        //f.read(addr, &mut buf).await;
        //defmt::println!("Got flash num: {:?}", buf);
        //let u = u32::from_le_bytes(buf) + 1;
        //let buf = u.to_le_bytes();
        //defmt::println!("Trying to write: {:?}", buf);

        //TODO: well, we will need to reset the page first...
        //f.write(addr, &buf).await;
        //defmt::println!("Done");
    }

    //let mut battery = battery::AccurateBatteryReader::new(&spawner, battery);
    let bat_state = battery::BatteryChargeState::new(p.P0_23, p.P0_25);

    let button = button::Button::new(p.P0_17);

    let lcd = display::Display::setup(&spawner, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27).await;

    let backlight = display::Backlight::new(p.P0_08);

    let gps = gps::GPSRessources::new(
        p.P0_29,
        p.P0_31,
        p.P0_30,
        p.UARTE0,
        p.TIMER1,
        p.PPI_CH1,
        p.PPI_CH2,
        p.PPI_GROUP1,
    )
    .await;

    let mag = mag::MagRessources::new(p.P1_12, p.P1_13);

    let mut ctx = Context {
        backlight,
        button,
        bat_state,
        battery,
        //gps,
        flash,
        lcd,
        spi: p.SPI2,
        start_time: Instant::now(),
        mag,
        twi1: p.TWISPI1,
    };

    spawner
        .spawn(touch_task(p.TWISPI0, p.P1_01, p.P1_02, p.P1_03, p.P1_04))
        .unwrap();

    {
        let mut mag = ctx.mag.on(&mut ctx.twi1).await;
        let r = mag.read().await;
        defmt::println!("mag: {:?}", r);
    }

    //spawner
    //    .spawn(accel_task(p.TWISPI1, p.P1_06, p.P1_05))
    //    .unwrap();

    spawner.spawn(time::clock_sync_task(gps)).unwrap();

    loop {
        display_stuff(&mut ctx).await;
        idle(&mut ctx).await;
    }
}
