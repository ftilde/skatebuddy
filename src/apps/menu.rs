use crate::{drivers::lpm013m1126c::Rgb111, render_top_bar, ui::ButtonStyle, Context};
use embedded_graphics::prelude::{Point, Size};
use micromath::F32Ext;

pub async fn grid_menu<T: Copy, const N: usize>(
    ctx: &mut Context,
    options: [(&'static str, T); N],
    button: T,
) -> T {
    ctx.lcd.on();
    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font: &embedded_graphics::mono_font::ascii::FONT_10X20,
    };

    let mut i = 0i32;
    let cols = (N as f32).sqrt().ceil() as i32;
    let y_offset = 16;
    let x_offset = y_offset / 2;
    let s = (crate::drivers::lpm013m1126c::WIDTH as i32 - 2 * x_offset) / cols;
    let mut buttons = options.map(|(text, opt)| {
        let x = i % cols;
        let y = i / cols;
        let btn = crate::ui::Button::new(crate::ui::ButtonDefinition {
            position: Point::new(x * s + x_offset, y * s + y_offset),
            size: Size::new(s as _, s as _),
            style: &button_style,
            text,
        });
        i += 1;
        (btn, opt)
    });

    'outer: loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery, &mut ctx.bat_state).await;

        for (btn, _) in &buttons {
            btn.render(&mut *ctx.lcd);
        }

        let ((), evt) = embassy_futures::join::join(
            ctx.lcd.present(),
            embassy_futures::select::select(ctx.button.wait_for_press(), touch.wait_for_action()),
        )
        .await;

        match evt {
            embassy_futures::select::Either::First(_) => break 'outer button,
            embassy_futures::select::Either::Second(e) => {
                defmt::println!("BTN: {:?}", e);
                for (btn, app) in &mut buttons {
                    if btn.clicked(&e) {
                        break 'outer *app;
                    }
                }
            }
        }
    }
}
