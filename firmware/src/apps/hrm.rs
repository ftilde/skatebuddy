use core::fmt::Write;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use embedded_graphics::mono_font::MonoTextStyle;

use crate::{render_top_bar, ui::TextWriter, Context};

pub async fn hrm(ctx: &mut Context) {
    //let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    //let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut hrm = ctx.hrm.on(&mut ctx.twi1).await;
    hrm.enable().await;

    ctx.lcd.on().await;
    loop {
        match select::select(hrm.wait_event(), ctx.button.wait_for_press()).await {
            select::Either::First((r, s)) => {
                ctx.lcd.fill(Rgb111::black());
                render_top_bar(&mut ctx.lcd, &ctx.battery).await;

                let mut w =
                    TextWriter::new(&mut ctx.lcd, sl).y(20 + font.character_size.height as i32);
                let _ = writeln!(w, "status: {}", r.status);
                let _ = writeln!(w, "irq_status: {}", r.irq_status);
                let _ = writeln!(w, "env: {:?}", r.env_value);
                let _ = writeln!(w, "pre: {:?}", r.pre_value);
                let _ = writeln!(w, "ps: {}", r.ps_value);
                let _ = writeln!(w, "pd: {:?}", r.pd_res_value);
                let _ = writeln!(w, "cur: {:?}", r.current_value);

                if let Some(sample) = s {
                    let _ = writeln!(w, "s: {:?}", sample);
                }

                ctx.lcd.present().await;
            }
            select::Either::Second(_d) => {
                break;
            }
        }
    }
    hrm.disable().await;
}
