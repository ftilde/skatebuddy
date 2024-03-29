use bitmap_font::TextStyle;
use core::fmt::Write;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{Duration, Ticker};

use crate::{render_top_bar, ui::TextWriter, Context};

pub async fn hrm(ctx: &mut Context) {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut hrm = ctx.hrm.on(&mut ctx.twi1).await;

    let mut ticker = Ticker::every(Duration::from_millis(1000));

    ctx.lcd.on().await;
    //ctx.backlight.off();
    loop {
        let version = hrm.model_number().await;

        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(20);
        let _ = writeln!(w, "model: {}", version);

        ctx.lcd.present().await;

        match select::select(ticker.next(), ctx.button.wait_for_press()).await {
            select::Either::First(_) => {}
            select::Either::Second(_d) => {
                break;
            }
        }
    }
}
