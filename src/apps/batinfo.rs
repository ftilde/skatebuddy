use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use embassy_time::{Duration, Ticker};

use crate::{
    drivers::lpm013m1126c::Rgb111, render_top_bar, ui::TextWriter, App, Context, DISPLAY_EVENT,
};

pub async fn battery_info(ctx: &mut Context) -> App {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut ticker = Ticker::every(Duration::from_secs(60));

    ctx.lcd.on();
    loop {
        let mua = ctx.battery.current();
        let mdev = ctx.battery.current_std();

        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        let mut w = TextWriter {
            y: 20,
            display: &mut ctx.lcd,
        };

        w.writeln(sl, arrform!(20, "c: {}muA", mua.micro_ampere()).as_str());
        w.writeln(sl, arrform!(20, "s: {}muA", mdev.micro_ampere()).as_str());

        let boot_count = ctx
            .flash
            .with_fs(|fs| {
                fs.open_file_and_then(&littlefs2::path::PathBuf::from(b"bootcount.bin"), |file| {
                    let mut boot_count = 0;
                    file.read(bytemuck::bytes_of_mut(&mut boot_count))?;
                    Ok(boot_count)
                })
            })
            .await;

        if let Ok(boot_count) = boot_count {
            w.writeln(sl, arrform!(20, "boot: {}", boot_count).as_str());
        } else {
            w.writeln(sl, "bootcount fail");
        }

        ctx.lcd.present().await;

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
