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
    primitives::Rectangle,
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
    ui::{ButtonStyle, EventHandler as _},
};

pub async fn stopwatch(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_millis(100));

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

    let mut start_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 0),
        size: Size::new(w, h),
        style: &button_style,
        text: "Start",
    })
    .on_click(|s: &mut State| {
        let duration_so_far = match *s {
            State::Stopped => Duration::from_secs(0),
            State::Running { .. } => panic!("Invalid state"),
            State::Paused { so_far } => so_far,
        };
        *s = State::Running {
            since: Instant::now() - duration_so_far,
        };
    });

    let mut stop_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 0),
        size: Size::new(w, h),
        style: &button_style,
        text: "Stop",
    })
    .on_click(|s: &mut State| {
        let State::Running { since } = *s else {
            panic!("Invalid state to stop");
        };

        *s = State::Paused {
            so_far: since.elapsed(),
        }
    });

    let mut reset_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 0),
        size: Size::new(w, h),
        style: &button_style,
        text: "Reset",
    })
    .on_click(|s: &mut State| {
        *s = State::Stopped;
    });

    let display_area = Rectangle::new(Point::new(0, 0), Size::new(176, 176));

    ctx.lcd.on().await;
    loop {
        ctx.lcd.fill(Rgb111::black());

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

        let left_button = match state {
            State::Stopped => &mut start_button,
            State::Running { .. } => &mut stop_button,
            State::Paused { .. } => &mut start_button,
        };
        let mut layout = LinearLayout::vertical(
            Chain::new(Text::new(time_text.as_str(), Point::zero(), sl)).append(
                LinearLayout::horizontal(Chain::new(left_button).append(&mut reset_button))
                    .arrange(),
            ),
        )
        .with_alignment(horizontal::Center)
        .with_spacing(embedded_layout::layout::linear::FixedMargin(5))
        .arrange()
        .align_to(&display_area, horizontal::Center, vertical::Center);

        layout.draw(&mut *ctx.lcd).unwrap();

        ctx.lcd.present().await;

        match select::select3(
            ticker.next(),
            ctx.button.wait_for_press(),
            touch.wait_for_action(),
        )
        .await
        {
            select::Either3::First(_) => {}
            select::Either3::Second(_d) => {
                break;
            }
            select::Either3::Third(e) => {
                let _ = layout.touch(e, &mut state);
            }
        }
    }
}
