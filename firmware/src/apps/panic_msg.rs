use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{Duration, Ticker};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::Drawable;

use crate::{render_top_bar, Context};

pub async fn panic_msg(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut ticker = Ticker::every(Duration::from_secs(60));

    let msg = ctx
        .last_panic_msg
        .unwrap_or("No recorded panic since last boot.");

    let area = crate::BELOW_BAR_AREA;

    ctx.lcd.on().await;

    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut text = embedded_text::TextBox::new(msg, area, sl);
        text.style.alignment = embedded_text::alignment::HorizontalAlignment::Left;
        text.style.vertical_alignment = embedded_text::alignment::VerticalAlignment::Middle;

        let _ = text.draw(&mut *ctx.lcd);

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
