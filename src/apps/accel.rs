use bitmap_font::TextStyle;
use core::fmt::Write;
use embassy_time::{Duration, Ticker};

use crate::{drivers::lpm013m1126c::Rgb111, render_top_bar, ui::TextWriter, Context};

pub async fn accel(ctx: &mut Context) {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let config = crate::drivers::accel::Config::new();
    let mut accel = ctx.accel.on(&mut ctx.twi1, config).await;

    let mut ticker = Ticker::every(Duration::from_millis(100));

    ctx.lcd.on();
    //ctx.backlight.off();
    loop {
        let reading = accel.reading_nf().await;

        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(20);
        let _ = writeln!(w, "x: {}", reading.x);
        let _ = writeln!(w, "y: {}", reading.y);
        let _ = writeln!(w, "z: {}", reading.z);

        ctx.lcd.present().await;

        match embassy_futures::select::select(ticker.next(), ctx.button.wait_for_press()).await {
            embassy_futures::select::Either::First(_) => {}
            embassy_futures::select::Either::Second(_d) => {
                break;
            }
        }
    }
}
