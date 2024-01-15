use arrform::{arrform, ArrForm};
use bitmap_font::TextStyle;
use embassy_time::{Duration, Ticker};

use crate::{
    drivers::lpm013m1126c::Rgb111,
    render_top_bar,
    time::{self, hours_mins_secs},
    ui::TextWriter,
    App, Context, DISPLAY_EVENT,
};

pub async fn clock_info(ctx: &mut Context) -> App {
    let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut ticker = Ticker::every(Duration::from_secs(60));

    ctx.lcd.on();
    //ctx.backlight.off();
    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        let now = ctx.start_time.elapsed();

        let (h, min, s) = hours_mins_secs(Duration::from_secs(now.as_secs()));

        let mut w = TextWriter {
            y: 20,
            display: &mut ctx.lcd,
        };
        w.writeln(sl, arrform!(20, "R: {}:{:0>2}:{:0>2}", h, min, s).as_str());

        w.writeln(sl, arrform!(36, "N_F: {}", time::num_sync_fails()).as_str());

        let (h, min, s) = hours_mins_secs(time::time_since_last_sync());
        w.writeln(sl, arrform!(36, "G: {}:{:0>2}:{:0>2}", h, min, s).as_str());

        let (_h, min, s) = hours_mins_secs(time::last_sync_duration());
        w.writeln(sl, arrform!(16, "T_G: {:0>2}:{:0>2}", min, s).as_str());

        w.writeln(sl, arrform!(16, "Drift: {}", time::last_drift_s()).as_str());

        ctx.lcd.present().await;

        match embassy_futures::select::select3(
            ticker.next(),
            ctx.button.wait_for_press(),
            DISPLAY_EVENT.wait(),
        )
        .await
        {
            embassy_futures::select::Either3::First(_) => {}
            embassy_futures::select::Either3::Second(_d) => {
                break App::Menu;
            }
            embassy_futures::select::Either3::Third(_event) => {
                DISPLAY_EVENT.reset();
            }
        }
    }
}
