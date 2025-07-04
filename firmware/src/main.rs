#![cfg_attr(target_arch = "arm", no_main, no_std)]
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
mod settings;
mod ui;
mod util;

mod log {
    #[cfg(not(target_arch = "arm"))]
    macro_rules! println {
        ($($arg:expr),*) => { std::println!($($arg),*) };
        //(debug, $($arg:expr),*) => { log::debug!($($arg),*) };
    }

    #[cfg(target_arch = "arm")]
    macro_rules! println {
        ($($arg:expr),*) => { defmt::println!($($arg),*) };
        //(debug, $($arg:expr),*) => { defmt::debug!($($arg),*) };
    }

    pub(crate) use println;
}

use apps::menu::Page;
use log::println;

use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use drivers::{time, Context};
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;

use drivers::lpm013m1126c::{BWConfig, Rgb111};

use drivers::futures::select;
use drivers::time::{Duration, Instant, Timer};

type Filesystem<'a, 'b> = littlefs2::fs::Filesystem<'a, drivers::flash::Flash<'b>>;

pub const BELOW_BAR_AREA: embedded_graphics::primitives::Rectangle =
    embedded_graphics::primitives::Rectangle::new(
        Point::new(0, 16),
        Size::new(
            drivers::lpm013m1126c::WIDTH as _,
            (drivers::lpm013m1126c::HEIGHT - 16) as _,
        ),
    );
async fn render_top_bar(lcd: &mut drivers::display::Display, bat: &drivers::battery::AsyncBattery) {
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

    let v = bat.read();

    let bat = arrform!(
        4,
        "{}{:0>2}",
        match bat.state() {
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

    ctx.lcd.on().await;
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

            let v = ctx.battery.read();
            let perc = v.percentage() as i32;

            let bat = arrform!(
                5,
                "{: >3}%{}",
                perc,
                match ctx.battery.state() {
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

        match select::select3(
            next_wakeup,
            ctx.button.wait_for_press(),
            drivers::wait_display_event(),
        )
        .await
        {
            select::Either3::First(_) => {}
            select::Either3::Second(_d) => {
                break;
            }
            select::Either3::Third(_event) => {}
        }
    }
}

async fn reset(ctx: &mut Context) {
    let options = [("Really Reset", true), ("Back", false)].into();

    if apps::menu::grid_menu(ctx, options, false).await {
        ctx.lcd.clear().await;
        drivers::sys_reset();
    }
}

async fn format_flash(ctx: &mut Context) {
    let options = [("Really Format", true), ("Back", false)].into();

    if apps::menu::grid_menu(ctx, options, false).await {
        let mut flash = ctx.flash.on().await;
        Filesystem::format(&mut flash).unwrap();
    }
}

async fn system_menu(ctx: &mut Context) {
    #[derive(Copy, Clone)]
    enum App {
        Reset,
        PanicMsg,
        Panic,
        FormatFlash,
        Files,
    }

    let options = [
        ("Files", App::Files),
        ("Panic Msg", App::PanicMsg),
        ("Reset", App::Reset),
        ("Panic", App::Panic),
        ("Fmt Flash", App::FormatFlash),
    ];

    let mut page = Page::zero();

    loop {
        if let apps::menu::MenuSelection::Item(last_page, (_, app)) =
            apps::menu::paginated_grid_menu::<4, _, _>(
                &mut ctx.touch,
                &ctx.twi,
                &mut ctx.button,
                &mut ctx.lcd,
                &mut ctx.battery,
                &mut ctx.backlight,
                options.as_slice(),
                page,
            )
            .await
        {
            page = last_page;
            match app {
                App::Files => apps::files::files(ctx).await,
                App::Reset => reset(ctx).await,
                App::PanicMsg => apps::panic_msg::panic_msg(ctx).await,
                App::Panic => panic!("as you choose"),
                App::FormatFlash => format_flash(ctx).await,
            }
        } else {
            break;
        }
    }
}
async fn app_menu(ctx: &mut Context) {
    #[derive(Copy, Clone)]
    enum App {
        Draw,
        Stopwatch,
        ClockInfo,
        BatInfo,
        Timer,
        Idle,
        Accel,
        Hrm,
        Track,
        Settings,
        System,
    }

    let options = [
        ("Hrm", App::Hrm),
        ("Track", App::Track),
        ("Stop\nwatch", App::Stopwatch),
        ("Timer", App::Timer),
        ("Clock", App::ClockInfo),
        ("Bat", App::BatInfo),
        ("Draw", App::Draw),
        ("Idle", App::Idle),
        ("Accel", App::Accel),
        ("Settings", App::Settings),
        ("System", App::System),
    ];

    let mut page = Page::zero();

    loop {
        if let apps::menu::MenuSelection::Item(last_page, (_, app)) =
            apps::menu::paginated_grid_menu::<4, _, _>(
                &mut ctx.touch,
                &ctx.twi,
                &mut ctx.button,
                &mut ctx.lcd,
                &mut ctx.battery,
                &mut ctx.backlight,
                options.as_slice(),
                page,
            )
            .await
        {
            page = last_page;
            match app {
                App::ClockInfo => apps::clockinfo::clock_info(ctx).await,
                App::BatInfo => apps::batinfo::battery_info(ctx).await,
                App::Draw => apps::draw::touch_playground(ctx).await,
                App::Stopwatch => apps::stopwatch::stopwatch(ctx).await,
                App::Timer => apps::timer::timer(ctx).await,
                App::Idle => apps::idle::idle(ctx).await,
                App::Accel => apps::accel::accel(ctx).await,
                App::Hrm => apps::hrm::hrm(ctx).await,
                App::Track => apps::track::track_app(ctx).await,
                App::Settings => settings::settings_ui(ctx).await,
                App::System => system_menu(ctx).await,
            }
        } else {
            break;
        }
    }
}

#[cfg_attr(target_arch = "arm", cortex_m_rt::entry)]
fn main() -> ! {
    drivers::run(|mut ctx: drivers::Context| async move {
        {
            let mut alloc = Filesystem::allocate();

            let mut flash = ctx.flash.on().await;
            let fs = match Filesystem::mount(&mut alloc, &mut flash) {
                Ok(fs) => {
                    crate::println!("Mounting existing fs");
                    fs
                }
                Err(_e) => {
                    crate::println!("Formatting fs because of mount error");
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
            crate::println!("This is boot nr {}", num_boots);

            // Ignore in case there are no settings, yet
            if let Ok(settings) = settings::Settings::load(&fs) {
                settings.apply();
            }
        }

        loop {
            clock(&mut ctx).await;
            app_menu(&mut ctx).await;
        }
    });
}

pub struct PerfTimer {
    start: Instant,
    what: &'static str,
}

impl PerfTimer {
    pub fn start(what: &'static str) -> Self {
        Self {
            start: Instant::now(),
            what,
        }
    }
    pub fn stop(self) {}
}
impl Drop for PerfTimer {
    fn drop(&mut self) {
        crate::println!("{}, {}ms", self.what, self.start.elapsed().as_millis());
    }
}
