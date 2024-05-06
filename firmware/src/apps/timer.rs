use arrform::*;
use drivers::{
    futures::select,
    lpm013m1126c::Rgb111,
    time::{Duration, Instant, Ticker},
    Context,
};
use embedded_graphics::{
    geometry::{Point, Size},
    mono_font::MonoTextStyle,
    text::Text,
    Drawable as _,
};
use embedded_layout::{
    align::{horizontal, vertical, Align as _},
    layout::linear::LinearLayout,
    object_chain::Chain,
};

use crate::{
    render_top_bar,
    ui::{ButtonStyle, EventHandler as _, TouchResult},
};

#[derive(Copy, Clone)]
enum Mode {
    Running { end: Instant },
    Paused { time_left: Duration },
}

enum Flow {
    Stop,
    Continue,
}

pub async fn timer(ctx: &mut Context) {
    let mut timer_duration = Duration::from_secs(5 * 60);
    loop {
        if let Flow::Stop = configure_timer(ctx, &mut timer_duration).await {
            break;
        }
        if let TimerResult::Finished = run_timer(ctx, timer_duration).await {
            buzz_msg(ctx, "Timer elapsed!").await
        }
    }
}

pub async fn buzz_msg(ctx: &mut Context, msg: &str) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_secs(1));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let s = Size::new(150, 100);

    let mut dismiss_btn = crate::ui::Button::new(&button_style, s, "Dismiss");

    ctx.lcd.on().await;

    loop {
        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;
        let message = Text::new(msg, Point::zero(), sl);

        let layout = LinearLayout::vertical(Chain::new(message).append(&mut dismiss_btn))
            .with_alignment(horizontal::Center)
            .arrange()
            .align_to(
                &drivers::lpm013m1126c::DISPLAY_AREA,
                horizontal::Center,
                vertical::Center,
            );
        layout.draw(&mut *ctx.lcd).unwrap();
        match ctx
            .lcd
            .present_and(select::select3(
                ctx.button.wait_for_press(),
                touch.wait_for_action(),
                ticker.next(),
            ))
            .await
        {
            select::Either3::First(_d) => {
                break;
            }
            select::Either3::Second(e) => {
                if dismiss_btn.clicked(&e) {
                    break;
                }
            }
            select::Either3::Third(_) => {}
        }
    }
}

async fn configure_timer(ctx: &mut Context, timer_duration: &mut Duration) -> Flow {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_secs(1));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let s = Size::new(50, 50);

    let mut plus_5min =
        crate::ui::Button::new(&button_style, s, "+5 min").on_click(|s: &mut Duration| {
            *s += Duration::from_secs(5 * 60);
            Flow::Continue
        });
    let mut minus_5min =
        crate::ui::Button::new(&button_style, s, "-5 min").on_click(|s: &mut Duration| {
            *s = s
                .checked_sub(Duration::from_secs(5 * 60))
                .unwrap_or(Duration::from_secs(0));
            Flow::Continue
        });
    let mut plus_1min =
        crate::ui::Button::new(&button_style, s, "+1 min").on_click(|s: &mut Duration| {
            *s += Duration::from_secs(60);
            Flow::Continue
        });
    let mut minus_1min =
        crate::ui::Button::new(&button_style, s, "-1 min").on_click(|s: &mut Duration| {
            *s = s
                .checked_sub(Duration::from_secs(60))
                .unwrap_or(Duration::from_secs(0));
            Flow::Continue
        });
    let mut plus_1s =
        crate::ui::Button::new(&button_style, s, "+1 min").on_click(|s: &mut Duration| {
            *s += Duration::from_secs(1);
            Flow::Continue
        });
    let mut minus_1s =
        crate::ui::Button::new(&button_style, s, "-1 min").on_click(|s: &mut Duration| {
            *s = s
                .checked_sub(Duration::from_secs(1))
                .unwrap_or(Duration::from_secs(0));
            Flow::Continue
        });

    let mut start_button =
        crate::ui::Button::new(&button_style, s, "Start").on_click(|_s: &mut Duration| Flow::Stop);

    ctx.lcd.on().await;

    loop {
        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let secs = timer_duration.as_secs();
        let time_text = arrform!(10, "{}:{:0>2}", secs / 60, secs % 60,);

        let time_text = Text::new(time_text.as_str(), Point::zero(), sl);
        let mut layout = LinearLayout::vertical(
            Chain::new(
                LinearLayout::horizontal(Chain::new(time_text).append(&mut start_button))
                    .with_spacing(embedded_layout::layout::linear::spacing::DistributeFill(
                        150,
                    ))
                    .arrange(),
            )
            .append(
                LinearLayout::horizontal(
                    Chain::new(&mut plus_5min)
                        .append(&mut plus_1min)
                        .append(&mut plus_1s),
                )
                .arrange(),
            )
            .append(
                LinearLayout::horizontal(
                    Chain::new(&mut minus_5min)
                        .append(&mut minus_1min)
                        .append(&mut minus_1s),
                )
                .arrange(),
            ),
        )
        .with_alignment(horizontal::Center)
        .arrange()
        .align_to(
            &drivers::lpm013m1126c::DISPLAY_AREA,
            horizontal::Center,
            vertical::Center,
        );
        layout.draw(&mut *ctx.lcd).unwrap();
        match ctx
            .lcd
            .present_and(select::select3(
                ctx.button.wait_for_press(),
                touch.wait_for_action(),
                ticker.next(),
            ))
            .await
        {
            select::Either3::First(_d) => {
                break Flow::Stop;
            }
            select::Either3::Second(e) => {
                if let TouchResult::Done(Flow::Stop) = layout.touch(e, timer_duration) {
                    break Flow::Continue;
                }
            }
            select::Either3::Third(_) => {}
        }
    }
}

enum TimerResult {
    Interrupted,
    Finished,
}

async fn run_timer(ctx: &mut Context, timer_duration: Duration) -> TimerResult {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_secs(1));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let mut state = Mode::Running {
        end: Instant::now() + timer_duration,
    };

    let s = Size::new(80, 80);

    let mut resume_button =
        crate::ui::Button::new(&button_style, s, "Resume").on_click(|s: &mut Mode| {
            let time_left = match *s {
                Mode::Running { .. } => panic!("Invalid state"),
                Mode::Paused { time_left } => time_left,
            };
            *s = Mode::Running {
                end: Instant::now() + time_left,
            };
            Flow::Continue
        });

    let mut stop_button =
        crate::ui::Button::new(&button_style, s, "Pause").on_click(|s: &mut Mode| {
            let Mode::Running { end } = *s else {
                panic!("Invalid state to stop");
            };

            *s = Mode::Paused {
                time_left: end
                    .checked_duration_since(Instant::now())
                    .unwrap_or(Duration::from_secs(0)),
            };
            Flow::Continue
        });

    let mut reset_button =
        crate::ui::Button::new(&button_style, s, "Reset").on_click(|_s: &mut Mode| Flow::Stop);

    ctx.lcd.on().await;

    loop {
        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let time_left = match state {
            Mode::Running { end } => end
                .checked_duration_since(Instant::now())
                .unwrap_or(Duration::from_secs(0)),
            Mode::Paused { time_left } => time_left,
        };
        let secs = time_left.as_secs();
        let time_text = arrform!(10, "{}:{:0>2}", secs / 60, secs % 60,);

        let left_button = match state {
            Mode::Running { .. } => &mut stop_button,
            Mode::Paused { .. } => &mut resume_button,
        };
        let time_text = Text::new(time_text.as_str(), Point::zero(), sl);
        if time_left == Duration::from_secs(0) {
            break TimerResult::Finished;
        }
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
        layout.draw(&mut *ctx.lcd).unwrap();
        match ctx
            .lcd
            .present_and(select::select3(
                ctx.button.wait_for_press(),
                touch.wait_for_action(),
                ticker.next(),
            ))
            .await
        {
            select::Either3::First(_d) => {
                break TimerResult::Interrupted;
            }
            select::Either3::Second(e) => {
                if let TouchResult::Done(Flow::Stop) = layout.touch(e, &mut state) {
                    break TimerResult::Interrupted;
                }
            }
            select::Either3::Third(_) => {}
        }
    }
}
