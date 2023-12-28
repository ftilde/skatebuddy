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

use defmt_rtt as _;
use lpm013m1126c::BWConfig;
//logger
use nrf52840_hal as _; // memory layout
use panic_probe as _;
use ui::ButtonStyle; //panic handler

//use nrf52840_hal::{gpio::Level, prelude::*};

use crate::lpm013m1126c::{BlinkMode, Rgb111};

mod accel;
mod battery;
mod button;
mod display;
#[allow(unused)]
mod flash;
mod gps;
mod hardware;
mod lpm013m1126c;
mod mag;
mod time;
mod touch;
mod ui;
mod util;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals::{TWISPI0, TWISPI1},
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

async fn render_top_bar(
    lcd: &mut display::Display,
    bat: &battery::AsyncBattery,
    bat_state: &mut battery::BatteryChargeState,
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

    let v = bat.read().await;

    let bat = arrform!(
        4,
        "{}{:0>2}",
        match bat_state.read() {
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

struct Context {
    #[allow(unused)]
    flash: flash::FlashRessources,
    bat_state: battery::BatteryChargeState,
    battery: battery::AsyncBattery,
    button: button::Button,
    backlight: display::Backlight,
    #[allow(unused)]
    //gps: gps::GPSRessources,
    lcd: display::Display,
    start_time: Instant,
    spi: embassy_nrf::peripherals::SPI2,
    mag: mag::MagRessources,
    touch: touch::TouchRessources,
    twi0: TWISPI0,
    twi1: TWISPI1,
}

async fn idle(ctx: &mut Context) -> App {
    ctx.lcd.clear(&mut ctx.spi).await;
    ctx.lcd.off();
    ctx.backlight.off();

    ctx.button.wait_for_press().await;

    App::Menu
}

async fn touch_playground(ctx: &mut Context) -> App {
    ctx.lcd.on();
    ctx.backlight.on();

    ctx.lcd.fill(Rgb111::white());
    ctx.lcd.present(&mut ctx.spi).await;

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    //ctx.backlight.off();
    let mut prev_point = None;
    let next = loop {
        match embassy_futures::select::select(ctx.button.wait_for_press(), touch.wait_for_event())
            .await
        {
            embassy_futures::select::Either::First(_) => {
                break App::Menu;
            }
            embassy_futures::select::Either::Second(e) => {
                defmt::println!("Touch: {:?}", e);

                let point = Point::new(e.x.into(), e.y.into());
                if let Some(pp) = prev_point {
                    embedded_graphics::primitives::Line::new(point, pp)
                        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(
                            Rgb111::black(),
                            3,
                        ))
                        .draw(&mut *ctx.lcd)
                        .unwrap();
                    defmt::println!("Draw from {}:{} to {}:{}", point.x, point.y, pp.x, pp.y);
                }

                prev_point = match e.kind {
                    touch::EventKind::Press => {
                        ctx.lcd.blink(&mut ctx.spi, BlinkMode::Inverted).await;
                        Some(point)
                    }
                    touch::EventKind::Release => {
                        ctx.lcd.blink(&mut ctx.spi, BlinkMode::Normal).await;
                        None
                    }
                    touch::EventKind::Hold => Some(point),
                };
            }
        }
        ctx.lcd.present(&mut ctx.spi).await;

        defmt::println!("we done presenting");
    };

    ctx.backlight.off();

    next
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

async fn display_stuff(ctx: &mut Context) -> App {
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

        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

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
                    break App::Menu;
                }
            }
            embassy_futures::select::Either3::Third(_event) => {
                DISPLAY_EVENT.reset();
            }
        }
    }
}

#[derive(Copy, Clone)]
enum App {
    Draw,
    Info,
    Idle,
    Menu,
    Clock,
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

    let x = (lpm013m1126c::WIDTH - text_len * font.width() as usize) / 2;
    Text::new(text, Point::new(x as i32, y), style)
        .draw(&mut ctx.lcd.binary(bw_config))
        .unwrap();
}

async fn clock(ctx: &mut Context) -> App {
    let large_font = bitmap_font::tamzen::FONT_16x32_BOLD.pixel_double();
    let small_font = bitmap_font::tamzen::FONT_16x32_BOLD;

    let bw_config = lpm013m1126c::BWConfig {
        off: Rgb111::black(),
        on: Rgb111::white(),
    };

    ctx.lcd.on();
    //ctx.backlight.off();
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
                    battery::ChargeState::Full => 'F',
                    battery::ChargeState::Charging => 'C',
                    battery::ChargeState::Draining => 'D',
                },
            );
            let col = if perc > 95 {
                Rgb111::green()
            } else if perc > 10 {
                Rgb111::white()
            } else {
                Rgb111::red()
            };

            let bw_config = lpm013m1126c::BWConfig {
                off: Rgb111::black(),
                on: col,
            };

            let x = lpm013m1126c::WIDTH - bat.as_str().len() * tiny_font.width() as usize;
            Text::new(bat.as_str(), Point::new(x as _, 0), tiny_style)
                .draw(&mut ctx.lcd.binary(bw_config))
                .unwrap();
        }

        ctx.lcd.present(&mut ctx.spi).await;

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
                    break App::Menu;
                }
            }
            embassy_futures::select::Either3::Third(_event) => {
                DISPLAY_EVENT.reset();
            }
        }
    }
}

async fn menu(ctx: &mut Context) -> App {
    ctx.lcd.on();
    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font: &embedded_graphics::mono_font::ascii::FONT_10X20,
    };

    let options = [
        ("Draw", App::Draw),
        ("Info", App::Info),
        ("Idle", App::Idle),
    ];

    let mut i = 0i32;
    let cols = 2i32;
    let y_offset = 16;
    let x_offset = y_offset / 2;
    let s = (crate::lpm013m1126c::WIDTH as i32 - 2 * x_offset) / cols;
    let mut buttons = options.map(|(text, app)| {
        let x = i % cols;
        let y = i / cols;
        let btn = ui::Button::new(ui::ButtonDefinition {
            position: Point::new(x * s + x_offset, y * s + y_offset),
            size: Size::new(s as _, s as _),
            style: &button_style,
            text,
        });
        i += 1;
        (btn, app)
    });

    'outer: loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        for (btn, _) in &buttons {
            btn.render(&mut *ctx.lcd);
        }

        let ((), evt) = embassy_futures::join::join(
            ctx.lcd.present(&mut ctx.spi),
            embassy_futures::select::select(ctx.button.wait_for_press(), touch.wait_for_action()),
        )
        .await;

        match evt {
            embassy_futures::select::Either::First(_) => break 'outer App::Clock,
            embassy_futures::select::Either::Second(e) => {
                defmt::println!("BTN: {:?}", e);
                for (btn, app) in &mut buttons {
                    if btn.clicked(&e) {
                        break 'outer *app;
                    }
                }
            }
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

    let touch =
        touch::TouchRessources::new(p.P1_01, p.P1_02, p.P1_03, p.P1_04, &mut p.TWISPI0).await;

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
        touch,
        twi0: p.TWISPI0,
        twi1: p.TWISPI1,
    };

    {
        let mut mag = ctx.mag.on(&mut ctx.twi1).await;
        let r = mag.read().await;
        defmt::println!("mag: {:?}", r);
    }

    {
        let mut accel = crate::accel::AccelRessources::new(p.P1_06, p.P1_05);

        let config = accel::Config::new();
        let mut accel = accel.on(&mut ctx.twi1, config).await;

        let mut ticker = Ticker::every(Duration::from_secs(1));
        for _ in 0..1 {
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

    spawner.spawn(time::clock_sync_task(gps)).unwrap();

    let mut state = App::Clock;
    loop {
        state = match state {
            App::Draw => touch_playground(&mut ctx).await,
            App::Info => display_stuff(&mut ctx).await,
            App::Idle => idle(&mut ctx).await,
            App::Menu => menu(&mut ctx).await,
            App::Clock => clock(&mut ctx).await,
        }
    }
}
