use bitmap_font::TextStyle;
use core::fmt::Write;
use embassy_time::{Duration, Ticker};

use crate::{
    drivers::lpm013m1126c::Rgb111, render_top_bar, ui::TextWriter, Context, DISPLAY_EVENT,
};

pub async fn battery_info(ctx: &mut Context) {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut ticker = Ticker::every(Duration::from_secs(60));

    ctx.lcd.on();
    loop {
        let mua = ctx.battery.current();
        let mdev = ctx.battery.current_std();

        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(20);

        let _ = writeln!(w, "c: {}muA", mua.micro_ampere());
        let _ = writeln!(w, "s: {}muA", mdev.micro_ampere());

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
            let _ = writeln!(w, "boot: {}", boot_count);
        } else {
            w.write("bootcount fail\n");
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
                    break;
                }
            }
            embassy_futures::select::Either3::Third(_event) => {
                DISPLAY_EVENT.reset();
            }
        }
    }
}
