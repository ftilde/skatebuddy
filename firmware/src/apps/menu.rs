use crate::{render_top_bar, ui::ButtonStyle, Context};
use arrayvec::ArrayVec;
use drivers::futures::select;
use drivers::lpm013m1126c::Rgb111;
use drivers::touch::{EventKind, Gesture};
use embedded_graphics::prelude::{Point, Size};

#[cfg(target_arch = "arm")]
use micromath::F32Ext;

pub async fn grid_menu<T: Clone, const N: usize>(
    ctx: &mut Context,
    options: ArrayVec<(&str, T), N>,
    button: T,
) -> T {
    ctx.lcd.on().await;
    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;
    ctx.backlight.active().await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font: &embedded_graphics::mono_font::ascii::FONT_10X20,
    };

    let mut i = 0i32;
    let cols = (N as f32).sqrt().ceil() as i32;
    let y_offset = 16;
    let x_offset = y_offset / 2;
    let s = (drivers::lpm013m1126c::WIDTH as i32 - 2 * x_offset) / cols;
    let mut buttons = options
        .into_iter()
        .map(|(text, opt)| {
            let x = i % cols;
            let y = i / cols;
            let btn = crate::ui::Button::from(crate::ui::ButtonDefinition {
                position: Point::new(x * s + x_offset, y * s + y_offset),
                size: Size::new(s as _, s as _),
                style: &button_style,
                text,
            });
            i += 1;
            (btn, opt)
        })
        .collect::<ArrayVec<_, N>>();

    'outer: loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        for (btn, _) in &buttons {
            btn.render(&mut *ctx.lcd).unwrap();
        }

        let evt = ctx
            .lcd
            .present_and(select::select(
                ctx.button.wait_for_press(),
                touch.wait_for_action(),
            ))
            .await;

        match evt {
            select::Either::First(_) => break 'outer button,
            select::Either::Second(e) => {
                ctx.backlight.active().await;
                crate::println!("BTN: {:?}", e);
                for (btn, app) in &mut buttons {
                    if btn.clicked(&e) {
                        break 'outer app.clone();
                    }
                }
            }
        }
    }
}

pub trait Paginated<const N: usize> {
    type Item;

    async fn access(&mut self, i: usize) -> ArrayVec<Self::Item, N>;
    async fn num_pages(&mut self) -> usize;
}

pub trait MenuItem {
    fn button_text(&self) -> &str;
}

impl<T> MenuItem for (&str, T) {
    fn button_text(&self) -> &str {
        self.0
    }
}

impl<const N: usize, T: Clone> Paginated<N> for &[T] {
    type Item = T;

    async fn access(&mut self, i: usize) -> ArrayVec<Self::Item, N> {
        self[(N * i)..(N * (i + 1)).min(self.len())]
            .iter()
            .cloned()
            .collect()
    }

    async fn num_pages(&mut self) -> usize {
        self.len().div_ceil(N)
    }
}

pub enum MenuSelection<T> {
    HardwareButton,
    Item(T),
}

pub async fn paginated_grid_menu<const N: usize, T: Clone + MenuItem, P: Paginated<N, Item = T>>(
    touch: &mut drivers::touch::TouchRessources,
    twi0: &mut drivers::TWI0,
    button: &mut drivers::button::Button,
    lcd: &mut drivers::display::Display,
    battery: &mut drivers::battery::AsyncBattery,
    backlight: &mut drivers::display::Backlight,
    mut options: P,
) -> MenuSelection<T> {
    backlight.active().await;
    lcd.on().await;
    let mut touch = touch.enabled(twi0).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font: &embedded_graphics::mono_font::ascii::FONT_10X20,
    };

    assert!(options.num_pages().await > 0);
    let mut page = 0;

    'outer: loop {
        let mut i = 0i32;
        let cols = (N as f32).sqrt().ceil() as i32;
        let y_offset = 16;
        let x_offset = y_offset / 2;
        let s = (drivers::lpm013m1126c::WIDTH as i32 - 2 * x_offset) / cols;
        let page_data = options.access(page).await;

        let mut buttons = page_data
            .iter()
            .map(|opt| {
                let x = i % cols;
                let y = i / cols;
                let btn = crate::ui::Button::from(crate::ui::ButtonDefinition {
                    position: Point::new(x * s + x_offset, y * s + y_offset),
                    size: Size::new(s as _, s as _),
                    style: &button_style,
                    text: opt.button_text(),
                });
                i += 1;
                (btn, opt)
            })
            .collect::<ArrayVec<_, N>>();

        'newpage: loop {
            lcd.fill(Rgb111::black());
            render_top_bar(lcd, &battery).await;

            for (btn, _) in &buttons {
                btn.render(&mut **lcd).unwrap();
            }

            let evt = lcd
                .present_and(select::select(
                    button.wait_for_press(),
                    touch.wait_for_action(),
                ))
                .await;

            match evt {
                select::Either::First(_) => break 'outer MenuSelection::HardwareButton,
                select::Either::Second(e) => {
                    crate::println!("BTN: {:?}", e);
                    backlight.active().await;
                    for (btn, app) in &mut buttons {
                        if btn.clicked(&e) {
                            break 'outer MenuSelection::Item(app.clone());
                        }
                    }

                    if let EventKind::Release = e.kind {
                        match e.gesture {
                            Gesture::SwipeDown | Gesture::SwipeRight => {
                                page = page.saturating_sub(1);
                                break 'newpage;
                            }
                            Gesture::SwipeUp | Gesture::SwipeLeft => {
                                page = (page + 1).min(options.num_pages().await - 1);
                                break 'newpage;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
