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

mod apps;
mod drivers;
mod time;
mod ui;
mod util;

use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use cortex_m::peripheral::SCB;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;

use defmt_rtt as _;
use drivers::lpm013m1126c::BWConfig;
use littlefs2::fs::Filesystem;
//logger
use nrf52840_hal as _; // memory layout
use panic_probe as _;

//use nrf52840_hal::{gpio::Level, prelude::*};

use drivers::lpm013m1126c::Rgb111;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals::{TWISPI0, TWISPI1},
    saadc, spim, twim,
};
use embassy_time::{Duration, Instant, Timer};

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
    SPIM2_SPIS2_SPI2 => spim::InterruptHandler<embassy_nrf::peripherals::SPI2>;
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI1>;
    UARTE0_UART0 => embassy_nrf::buffered_uarte::InterruptHandler<drivers::gps::UartInstance>;
    QSPI => embassy_nrf::qspi::InterruptHandler<embassy_nrf::peripherals::QSPI>;
});

async fn render_top_bar(
    lcd: &mut drivers::display::Display,
    bat: &drivers::battery::AsyncBattery,
    bat_state: &mut drivers::battery::BatteryChargeState,
) {
    let bw_config = BWConfig {
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

    let v = bat.read().await;

    let bat = arrform!(
        4,
        "{}{:0>2}",
        match bat_state.read() {
            drivers::battery::ChargeState::Full => 'F',
            drivers::battery::ChargeState::Charging => 'C',
            drivers::battery::ChargeState::Draining => 'D',
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

struct Context {
    #[allow(unused)]
    flash: drivers::flash::FlashRessources,
    bat_state: drivers::battery::BatteryChargeState,
    battery: drivers::battery::AsyncBattery,
    button: drivers::button::Button,
    backlight: drivers::display::Backlight,
    #[allow(unused)]
    //gps: gps::GPSRessources,
    lcd: drivers::display::Display,
    start_time: Instant,
    mag: drivers::mag::MagRessources,
    touch: drivers::touch::TouchRessources,
    accel: drivers::accel::AccelRessources,
    twi0: TWISPI0,
    twi1: TWISPI1,
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

fn draw_centered(
    ctx: &mut Context,
    text: &str,
    font: &bitmap_font::BitmapFont,
    y: i32,
    bw_config: BWConfig,
) {
    // TODO: Remove this terrible sin. I'm so sorry.
    let text_len = text.len();

    let style = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let x = (drivers::lpm013m1126c::WIDTH - text_len * font.width() as usize) / 2;
    Text::new(text, Point::new(x as i32, y), style)
        .draw(&mut ctx.lcd.binary(bw_config))
        .unwrap();
}

async fn clock(ctx: &mut Context) {
    let large_font = bitmap_font::tamzen::FONT_16x32_BOLD.pixel_double();
    let small_font = bitmap_font::tamzen::FONT_16x32_BOLD;

    let bw_config = BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };

    ctx.lcd.on();
    loop {
        ctx.lcd.fill(bw_config.off);

        if let Some(c) = time::now_local() {
            use chrono::{Datelike, Timelike};
            let time = arrform!(5, "{:0>2}:{:0>2}", c.hour(), c.minute());
            let date = arrform!(10, "{:0>2}.{:0>2}.{:0>4}", c.day(), c.month(), c.year());

            draw_centered(ctx, time.as_str(), &large_font, 40, bw_config);
            draw_centered(ctx, date.as_str(), &small_font, 100, bw_config);
        } else {
            draw_centered(ctx, "SYNC", &large_font, 40, bw_config);
        };

        {
            let tiny_font = bitmap_font::tamzen::FONT_12x24;
            let tiny_style =
                TextStyle::new(&tiny_font, embedded_graphics::pixelcolor::BinaryColor::On);

            let v = ctx.battery.read().await;
            let perc = v.percentage() as i32;

            let bat = arrform!(
                5,
                "{: >3}%{}",
                perc,
                match ctx.bat_state.read() {
                    drivers::battery::ChargeState::Full => 'F',
                    drivers::battery::ChargeState::Charging => 'C',
                    drivers::battery::ChargeState::Draining => 'D',
                },
            );
            let col = if perc > 95 {
                Rgb111::green()
            } else if perc > 10 {
                Rgb111::white()
            } else {
                Rgb111::red()
            };

            let bw_config = BWConfig {
                off: Rgb111::black(),
                on: col,
            };

            let x = drivers::lpm013m1126c::WIDTH - bat.as_str().len() * tiny_font.width() as usize;
            Text::new(bat.as_str(), Point::new(x as _, 0), tiny_style)
                .draw(&mut ctx.lcd.binary(bw_config))
                .unwrap();
        }

        ctx.lcd.present().await;

        let next_wakeup = if let Some(now) = time::now_local() {
            use chrono::Timelike;
            let wakeup =
                time::to_instant(now.with_second(0).unwrap() + chrono::Duration::minutes(1))
                    .unwrap();
            Timer::at(wakeup)
        } else {
            Timer::after(Duration::from_secs(60))
        };

        match embassy_futures::select::select3(
            next_wakeup,
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

async fn reset(ctx: &mut Context) {
    let options = [("Really Reset", true), ("Back", false)];

    if apps::menu::grid_menu(ctx, options, false).await {
        ctx.lcd.clear().await;
        SCB::sys_reset()
    }
}

async fn app_menu(ctx: &mut Context) {
    #[derive(Copy, Clone)]
    enum App {
        Draw,
        ClockInfo,
        BatInfo,
        Idle,
        Reset,
        Accel,
    }

    let options = [
        ("Draw", Some(App::Draw)),
        ("Clock", Some(App::ClockInfo)),
        ("Bat", Some(App::BatInfo)),
        ("Idle", Some(App::Idle)),
        ("Reset", Some(App::Reset)),
        ("Accel", Some(App::Accel)),
    ];

    loop {
        if let Some(app) = apps::menu::grid_menu(ctx, options, None).await {
            match app {
                App::Draw => apps::draw::touch_playground(ctx).await,
                App::ClockInfo => apps::clockinfo::clock_info(ctx).await,
                App::BatInfo => apps::batinfo::battery_info(ctx).await,
                App::Idle => apps::idle::idle(ctx).await,
                App::Accel => apps::accel::accel(ctx).await,
                App::Reset => reset(ctx).await,
            }
        } else {
            break;
        }
    }
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

    let battery = drivers::battery::Battery::new(p.SAADC, p.P0_03);
    let battery = drivers::battery::AsyncBattery::new(&spawner, battery);

    // Keep hrm in reset to power it off
    let _hrm_power = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);

    // Explicitly "disconnect" the following devices' i2c pins
    // Heartrate
    let _unused = Input::new(p.P0_24, Pull::None);
    let _unused = Input::new(p.P1_00, Pull::None);

    // pressure
    let _unused = Input::new(p.P1_15, Pull::None);
    let _unused = Input::new(p.P0_02, Pull::None);

    let _vibrate = Output::new(p.P0_19, Level::Low, OutputDrive::Standard);

    let mut flash = drivers::flash::FlashRessources::new(
        p.QSPI, p.P0_14, p.P0_16, p.P0_15, p.P0_13, p.P1_10, p.P1_11,
    )
    .await;
    {
        let mut alloc = Filesystem::allocate();

        let mut flash = flash.on().await;
        let fs = match Filesystem::mount(&mut alloc, &mut flash) {
            Ok(fs) => {
                defmt::println!("Mounting existing fs",);
                fs
            }
            Err(_e) => {
                defmt::println!("Formatting fs because of mount error",);
                Filesystem::format(&mut flash).unwrap();
                Filesystem::mount(&mut alloc, &mut flash).unwrap()
            }
        };

        let num_boots = fs
            .open_file_with_options_and_then(
                |options| options.read(true).write(true).create(true),
                &littlefs2::path::PathBuf::from(b"bootcount.bin"),
                |file| {
                    let mut boot_num = 0u32;
                    if file.len().unwrap() >= 4 {
                        file.read(bytemuck::bytes_of_mut(&mut boot_num)).unwrap();
                    };
                    boot_num += 1;
                    file.seek(littlefs2::io::SeekFrom::Start(0)).unwrap();
                    file.write(bytemuck::bytes_of(&boot_num)).unwrap();
                    Ok(boot_num)
                },
            )
            .unwrap();
        defmt::println!("This is boot nr {}", num_boots);
    }

    //let mut battery = battery::AccurateBatteryReader::new(&spawner, battery);
    let bat_state = drivers::battery::BatteryChargeState::new(p.P0_23, p.P0_25);

    let button = drivers::button::Button::new(p.P0_17);

    let lcd = drivers::display::Display::setup(
        &spawner, p.SPI2, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27,
    )
    .await;

    let backlight = drivers::display::Backlight::new(p.P0_08);

    let touch =
        drivers::touch::TouchRessources::new(p.P1_01, p.P1_02, p.P1_03, p.P1_04, &mut p.TWISPI0)
            .await;

    let gps = drivers::gps::GPSRessources::new(
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

    let mag = drivers::mag::MagRessources::new(p.P1_12, p.P1_13);
    let accel = drivers::accel::AccelRessources::new(p.P1_06, p.P1_05);

    let mut ctx = Context {
        backlight,
        button,
        bat_state,
        battery,
        //gps,
        flash,
        lcd,
        start_time: Instant::now(),
        mag,
        touch,
        accel,
        twi0: p.TWISPI0,
        twi1: p.TWISPI1,
    };

    {
        let mut mag = ctx.mag.on(&mut ctx.twi1).await;
        let r = mag.read().await;
        defmt::println!("mag: {:?}", r);
    }

    spawner.spawn(time::clock_sync_task(gps)).unwrap();

    loop {
        clock(&mut ctx).await;
        app_menu(&mut ctx).await;
    }
}
