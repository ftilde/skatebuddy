use crate::{App, Context};

use crate::drivers::lpm013m1126c::{BlinkMode, Rgb111};
use crate::drivers::touch::EventKind;
use embedded_graphics::prelude::*;

pub async fn touch_playground(ctx: &mut Context) -> App {
    ctx.lcd.on();
    ctx.backlight.on();

    ctx.lcd.fill(Rgb111::white());
    ctx.lcd.present().await;

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    //ctx.backlight.off();
    let mut prev_point = None;
    let next = loop {
        match embassy_futures::select::select(ctx.button.wait_for_press(), touch.wait_for_event())
            .await
        {
            embassy_futures::select::Either::First(_) => {
                break App::Menu;
            }
            embassy_futures::select::Either::Second(e) => {
                defmt::println!("Touch: {:?}", e);

                let point = Point::new(e.x.into(), e.y.into());
                if let Some(pp) = prev_point {
                    embedded_graphics::primitives::Line::new(point, pp)
                        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(
                            Rgb111::black(),
                            3,
                        ))
                        .draw(&mut *ctx.lcd)
                        .unwrap();
                    defmt::println!("Draw from {}:{} to {}:{}", point.x, point.y, pp.x, pp.y);
                }

                prev_point = match e.kind {
                    EventKind::Press => {
                        ctx.lcd.blink(BlinkMode::Inverted).await;
                        Some(point)
                    }
                    EventKind::Release => {
                        ctx.lcd.blink(BlinkMode::Normal).await;
                        None
                    }
                    EventKind::Hold => Some(point),
                };
            }
        }
        ctx.lcd.present().await;

        defmt::println!("we done presenting");
    };

    ctx.backlight.off();

    next
}
