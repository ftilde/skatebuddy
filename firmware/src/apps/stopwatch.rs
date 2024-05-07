use arrform::*;
use drivers::{
    futures::select,
    lpm013m1126c::Rgb111,
    time::{Duration, Instant, Ticker},
    Context,
};
use embedded_graphics::{geometry::Size, mono_font::MonoTextStyle, Drawable as _};
use embedded_layout::{
    align::{horizontal, vertical, Align as _},
    layout::linear::LinearLayout,
    object_chain::Chain,
};

use crate::{
    render_top_bar,
    ui::{textbox, ButtonStyle, EventHandler as _},
};

pub async fn stopwatch(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;
    ctx.backlight.active().await;

    let mut ticker_on = Ticker::every(Duration::from_millis(10));
    let mut ticker_off = Ticker::every(Duration::from_millis(1000));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    #[derive(Copy, Clone)]
    enum State {
        Stopped,
        Running { since: Instant },
        Paused { so_far: Duration },
    }

    let mut state = State::Stopped;

    let w = 80;
    let h = 80;
    let s = Size::new(w, h);

    let mut start_button =
        crate::ui::Button::lazy(&button_style, s, "Start").on_click(|s: &mut State| {
            let duration_so_far = match *s {
                State::Stopped => Duration::from_secs(0),
                State::Running { .. } => panic!("Invalid state"),
                State::Paused { so_far } => so_far,
            };
            *s = State::Running {
                since: Instant::now() - duration_so_far,
            };
        });

    let mut stop_button =
        crate::ui::Button::lazy(&button_style, s, "Stop").on_click(|s: &mut State| {
            let State::Running { since } = *s else {
                panic!("Invalid state to stop");
            };

            *s = State::Paused {
                so_far: since.elapsed(),
            }
        });

    let mut reset_button =
        crate::ui::Button::lazy(&button_style, s, "Reset").on_click(|s: &mut State| {
            *s = State::Stopped;
        });

    let bg = Rgb111::black();
    ctx.lcd.fill(bg);
    ctx.lcd.on().await;
    loop {
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let duration = match state {
            State::Stopped => Duration::from_secs(0),
            State::Running { since } => since.elapsed(),
            State::Paused { so_far } => so_far,
        };
        let secs = duration.as_secs();
        let time_text = arrform!(
            10,
            "{}:{:0>2}:{:0>2}",
            secs / 60,
            secs % 60,
            (duration.as_millis() / 10) % 100,
        );
        let time_text = textbox(time_text.as_str(), Size::new(80, 40), bg, sl);

        let (left_button, ticker) = match state {
            State::Stopped => (&mut start_button, &mut ticker_off),
            State::Running { .. } => (&mut stop_button, &mut ticker_on),
            State::Paused { .. } => (&mut start_button, &mut ticker_off),
        };
        let mut layout = LinearLayout::vertical(Chain::new(time_text).append(
            LinearLayout::horizontal(Chain::new(left_button).append(&mut reset_button)).arrange(),
        ))
        .with_alignment(horizontal::Center)
        .with_spacing(embedded_layout::layout::linear::FixedMargin(5))
        .arrange()
        .align_to(
            &drivers::lpm013m1126c::DISPLAY_AREA,
            horizontal::Center,
            vertical::Center,
        );

        let t = crate::PerfTimer::start("draw");
        layout.draw(&mut *ctx.lcd).unwrap();
        t.stop();

        let t = crate::PerfTimer::start("present");
        let t2 = crate::PerfTimer::start("touch");
        match ctx
            .lcd
            .present_and(async {
                let res = select::select3(
                    ctx.button.wait_for_press(),
                    touch.wait_for_action(),
                    ticker.next(),
                )
                .await;
                t2.stop();
                res
            })
            .await
        {
            select::Either3::First(_d) => {
                break;
            }
            select::Either3::Second(e) => {
                let _ = layout.touch(e, &mut state);
                ctx.backlight.active().await;
            }
            select::Either3::Third(_) => {}
        }
        t.stop();
    }
}
