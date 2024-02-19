use bitmap_font::TextStyle;
use core::fmt::Write;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{self, Duration, Ticker};

use crate::{render_top_bar, ui::TextWriter, util::hours_mins_secs, Context};

pub async fn clock_info(ctx: &mut Context) {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut ticker = Ticker::every(Duration::from_secs(60));

    ctx.lcd.on().await;
    //ctx.backlight.off();
    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let now = ctx.start_time.elapsed();

        let (h, min, s) = hours_mins_secs(Duration::from_secs(now.as_secs()));

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(20);

        let _ = writeln!(w, "R: {}:{:0>2}:{:0>2}", h, min, s);
        let _ = writeln!(w, "N_F: {}", time::num_sync_fails());

        let (h, min, s) = hours_mins_secs(time::time_since_last_sync());
        let _ = writeln!(w, "G: {}:{:0>2}:{:0>2}", h, min, s);

        let (_h, min, s) = hours_mins_secs(time::last_sync_duration());
        let _ = writeln!(w, "T_G: {:0>2}:{:0>2}", min, s);

        let _ = writeln!(w, "Drift: {}", time::last_drift_s());

        ctx.lcd.present().await;

        match select::select3(
            ticker.next(),
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
