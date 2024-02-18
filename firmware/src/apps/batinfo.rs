use core::fmt::Write;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{Duration, Ticker};
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::MonoTextStyle;

use crate::ui::ButtonStyle;
use crate::{render_top_bar, ui::TextWriter, Context};

pub async fn battery_info(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_secs(60));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let mut upt_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 120),
        size: Size::new(90, 50),
        style: &button_style,
        text: "Update",
    });

    ctx.lcd.on();
    loop {
        let mua = ctx.battery.current();
        let mdev = ctx.battery.current_std();

        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(20 + font.character_size.height as i32);

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
        let _ = writeln!(w, "UPD: {}s", ctx.battery.last_update().elapsed().as_secs());

        upt_button.render(&mut *ctx.lcd);

        ctx.lcd.present().await;

        match select::select4(
            ticker.next(),
            ctx.button.wait_for_press(),
            drivers::wait_display_event(),
            touch.wait_for_action(),
        )
        .await
        {
            select::Either4::First(_) => {}
            select::Either4::Second(d) => {
                if d > Duration::from_secs(1) {
                    ctx.battery.reset().await;
                } else {
                    break;
                }
            }
            select::Either4::Third(_event) => {}
            select::Either4::Fourth(e) => {
                if upt_button.clicked(&e) {
                    ctx.battery.force_update().await;
                }
            }
        }
    }
}
