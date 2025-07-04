use core::fmt::Write;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{self, Duration, Instant, Ticker};
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::MonoTextStyle;

use crate::ui::ButtonStyle;
use crate::{render_top_bar, ui::TextWriter, util::hours_mins_secs, Context};

pub async fn clock_info(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut ticker = Ticker::every(Duration::from_secs(60));

    let mut touch = ctx.touch.enabled(&ctx.twi).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };
    let s = 50;
    let mut force_sync_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(170 - s, 170 - s),
        size: Size::new(s as _, s as _),
        style: &button_style,
        text: "Force Sync",
    });

    ctx.lcd.on().await;
    //ctx.backlight.off();
    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let now = ctx.start_time.elapsed();

        let (h, min, s) = hours_mins_secs(Duration::from_secs(now.as_secs()));

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);

        let _ = writeln!(w, "R: {}:{:0>2}:{:0>2}", h, min, s);
        let _ = writeln!(w, "N_F: {}", time::num_sync_fails());

        let (h, min, s) = hours_mins_secs(time::time_since_last_sync());
        let _ = writeln!(w, "G: {}:{:0>2}:{:0>2}", h, min, s);
        let next = time::next_sync();
        let now = Instant::now();
        let next = if next > now {
            next - now
        } else {
            Duration::from_secs(0)
        };
        let (h, min, s) = hours_mins_secs(next);
        let _ = writeln!(w, "N: {}:{:0>2}:{:0>2}", h, min, s);

        let (_h, min, s) = hours_mins_secs(time::last_sync_duration());
        let _ = writeln!(w, "T_G: {:0>2}:{:0>2}", min, s);

        let _ = writeln!(w, "Drift: {}", time::last_drift_s());

        let info = time::clock_info();
        let _ = writeln!(w, "S_N: {}", info.scale.numerator);
        let _ = writeln!(w, "S_D: {}", info.scale.denominator);

        force_sync_button.render(&mut *ctx.lcd).unwrap();

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
            select::Either4::Second(_d) => {
                break;
            }
            select::Either4::Third(_event) => {}
            select::Either4::Fourth(event) => {
                if force_sync_button.clicked(&event) {
                    time::force_sync()
                }
            }
        }
    }
}
