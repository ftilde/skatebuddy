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
mod ui;

use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use drivers_hw::{time, Context};
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;

use drivers_hw::lpm013m1126c::{BWConfig, Rgb111};

use drivers_hw::futures::select;
use drivers_hw::time::{Duration, Timer};

async fn render_top_bar(
    lcd: &mut drivers_hw::display::Display,
    bat: &drivers_hw::battery::AsyncBattery,
    bat_state: &mut drivers_hw::battery::BatteryChargeState,
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
            drivers_hw::battery::ChargeState::Full => 'F',
            drivers_hw::battery::ChargeState::Charging => 'C',
            drivers_hw::battery::ChargeState::Draining => 'D',
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

    let x = (drivers_hw::lpm013m1126c::WIDTH - text_len * font.width() as usize) / 2;
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
                    drivers_hw::battery::ChargeState::Full => 'F',
                    drivers_hw::battery::ChargeState::Charging => 'C',
                    drivers_hw::battery::ChargeState::Draining => 'D',
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

            let x =
                drivers_hw::lpm013m1126c::WIDTH - bat.as_str().len() * tiny_font.width() as usize;
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

        match select::select3(
            next_wakeup,
            ctx.button.wait_for_press(),
            drivers_hw::wait_display_event(),
        )
        .await
        {
            select::Either3::First(_) => {}
            select::Either3::Second(d) => {
                if d > Duration::from_secs(1) {
                    ctx.battery.reset().await;
                } else {
                    break;
                }
            }
            select::Either3::Third(_event) => {}
        }
    }
}

async fn reset(ctx: &mut Context) {
    let options = [("Really Reset", true), ("Back", false)];

    if apps::menu::grid_menu(ctx, options, false).await {
        ctx.lcd.clear().await;
        drivers_hw::sys_reset();
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

#[cortex_m_rt::entry]
fn main() -> ! {
    drivers_hw::run(|mut ctx| async move {
        loop {
            clock(&mut ctx).await;
            app_menu(&mut ctx).await;
        }
    });
}
