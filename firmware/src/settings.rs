use core::fmt::Write;

use arrform::*;
use drivers::{
    futures::select,
    lpm013m1126c::{BWConfig, Rgb111},
    time::{Duration, Ticker},
    Context,
};
use embedded_graphics::{
    geometry::{Point, Size},
    mono_font::MonoTextStyle,
    primitives::Rectangle,
    text::Text,
    Drawable,
};
use embedded_layout::{
    align::{horizontal, vertical, Align},
    layout::linear::LinearLayout,
    object_chain::Chain,
};
use littlefs2::path::Path;

use crate::{render_top_bar, ui::ButtonStyle, Filesystem};

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Settings {
    pub utc_offset: i8,
}

const SETTINGS_FILE: &Path = &Path::from_str_with_nul("settings.bin\0");

impl Settings {
    pub fn load(fs: &Filesystem) -> littlefs2::io::Result<Self> {
        fs.open_file_with_options_and_then(
            |options| options.read(true).create(true),
            SETTINGS_FILE,
            |file| {
                let mut s = Self::default();
                file.read(bytemuck::bytes_of_mut(&mut s))?;
                Ok(s)
            },
        )
    }

    pub fn save(&self, fs: &Filesystem) -> littlefs2::io::Result<()> {
        fs.open_file_with_options_and_then(
            |options| options.write(true).create(true),
            SETTINGS_FILE,
            |file| {
                let s = bytemuck::bytes_of(self);
                let written = file.write(s)?;
                assert_eq!(written, s.len());
                Ok(())
            },
        )
    }

    pub fn apply(&self) {
        drivers::time::set_utc_offset(self.utc_offset as i32 /*hours*/ * 60 *60);
    }
}

pub async fn settings_ui(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, Rgb111::white());

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let mut ticker = Ticker::every(Duration::from_secs(60));

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let mut settings = ctx.flash.with_fs(|fs| Settings::load(fs)).await.unwrap();

    let w = 30;
    let h = sl.font.character_size.height;
    let mut plus_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 0),
        size: Size::new(w, h),
        style: &button_style,
        text: "+",
    });

    let mut minus_button = crate::ui::Button::new(crate::ui::ButtonDefinition {
        position: Point::new(0, 0),
        size: Size::new(w, h),
        style: &button_style,
        text: "-",
    });

    let mut text_label = crate::ui::Label::new(arrform!(10, "UTC offset"), sl);
    let mut hours_label = crate::ui::Label::new(arrform!(3, "{:>3}", settings.utc_offset), sl);

    let display_area = Rectangle::new(Point::new(0, 0), Size::new(176, 176));

    LinearLayout::horizontal(
        Chain::new(&mut text_label)
            .append(&mut plus_button)
            .append(&mut hours_label)
            .append(&mut minus_button),
    )
    .with_alignment(vertical::Center)
    .arrange()
    .align_to(&display_area, horizontal::Center, vertical::Center);

    ctx.lcd.on().await;
    loop {
        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        text_label.draw(&mut *ctx.lcd).unwrap();
        hours_label.draw(&mut *ctx.lcd).unwrap();
        plus_button.render(&mut *ctx.lcd).unwrap();
        minus_button.render(&mut *ctx.lcd).unwrap();
        //let _ = layout.draw(&mut *ctx.lcd);

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
            select::Either4::Second(d) => {
                if d > Duration::from_secs(1) {
                    ctx.battery.reset().await;
                } else {
                    break;
                }
            }
            select::Either4::Third(_event) => {}
            select::Either4::Fourth(e) => {
                let mut changed = false;
                if plus_button.clicked(&e) {
                    settings.utc_offset += 1;
                    changed = true;
                }
                if minus_button.clicked(&e) {
                    settings.utc_offset -= 1;
                    changed = true;
                }
                settings.utc_offset = settings.utc_offset.clamp(-23, 23);
                if changed {
                    write!(hours_label.set(), "{:>3}", settings.utc_offset).unwrap();
                }

                settings.apply();
            }
        }
    }

    ctx.flash.with_fs(|fs| settings.save(fs)).await.unwrap();
}
