use crate::Context;

use drivers::futures::select;
use drivers::lpm013m1126c::{BlinkMode, Rgb111};
use drivers::touch::EventKind;
use embedded_graphics::prelude::*;

pub async fn touch_playground(ctx: &mut Context) {
    ctx.lcd.on().await;
    let _bl = ctx.backlight.on().await;

    ctx.lcd.fill(Rgb111::white());
    ctx.lcd.present().await;

    let mut touch = ctx.touch.enabled(&ctx.twi).await;

    //ctx.backlight.off();
    let mut prev_point = None;
    let next = loop {
        match select::select(ctx.button.wait_for_press(), touch.wait_for_event()).await {
            select::Either::First(_) => {
                break;
            }
            select::Either::Second(e) => {
                crate::println!("Touch: {:?}", e);

                let point = Point::new(e.x.into(), e.y.into());
                if let Some(pp) = prev_point {
                    embedded_graphics::primitives::Line::new(point, pp)
                        .into_styled(embedded_graphics::primitives::PrimitiveStyle::with_stroke(
                            Rgb111::black(),
                            3,
                        ))
                        .draw(&mut *ctx.lcd)
                        .unwrap();
                    crate::println!("Draw from {}:{} to {}:{}", point.x, point.y, pp.x, pp.y);
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

        crate::println!("we done presenting");
    };

    next
}
