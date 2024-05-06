use arrform::*;
use drivers::{
    futures::select,
    lpm013m1126c::Rgb111,
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

use crate::{
    render_top_bar,
    ui::{Button, ButtonDefinition, ButtonStyle, EventHandler},
    Filesystem,
};

#[repr(C)]
#[derive(Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Settings {
    pub utc_offset_hours: i8,
    pub utc_offset_minutes: i8,
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
        drivers::time::set_utc_offset(
            (self.utc_offset_hours as i32 * 60 + self.utc_offset_minutes as i32) * 60,
        );
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

    enum Action {
        Continue,
        Stop,
    }

    let mut settings = ctx.flash.with_fs(|fs| Settings::load(fs)).await.unwrap();
    let orig_settings = settings;

    let w = 30;
    let h = sl.font.character_size.height;
    let size = Size::new(w, h);

    let plus_button = ButtonDefinition::new(&button_style, size, "+");
    let minus_button = ButtonDefinition::new(&button_style, size, "-");

    let mut plus_button_hours = Button::from(plus_button).on_click(|ctx: &mut Settings| {
        ctx.utc_offset_hours = (ctx.utc_offset_hours + 1).min(23);
        Action::Continue
    });

    let mut minus_button_hours = Button::from(minus_button).on_click(|ctx: &mut Settings| {
        ctx.utc_offset_hours = (ctx.utc_offset_hours - 1).max(-23);
        Action::Continue
    });

    let mut plus_button_minutes = Button::from(plus_button).on_click(|ctx: &mut Settings| {
        ctx.utc_offset_minutes = (ctx.utc_offset_minutes + 1).min(59);
        Action::Continue
    });

    let mut minus_button_minutes = Button::from(minus_button).on_click(|ctx: &mut Settings| {
        ctx.utc_offset_minutes = (ctx.utc_offset_minutes - 1).max(-59);
        Action::Continue
    });

    let mut save_button =
        Button::new(&button_style, Size::new(2 * w, 2 * h), "Save").on_click(|_ctx| Action::Stop);

    //let mut hours_label =
    //    crate::ui::Label::new(arrform!(3, "{:>3}", settings.utc_offset_hours), sl);

    let display_area = Rectangle::new(Point::new(0, 0), Size::new(176, 176));

    ctx.lcd.on().await;
    loop {
        ctx.lcd.fill(Rgb111::black());

        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let hours_text = arrform!(3, "{:>3}", settings.utc_offset_hours);
        let minutes_text = arrform!(3, "{:>3}", settings.utc_offset_minutes);
        let mut layout = LinearLayout::vertical(
            Chain::new(Text::new("UTC offset", Point::zero(), sl))
                .append(
                    LinearLayout::horizontal(
                        Chain::new(Text::new("Hours: ", Point::zero(), sl))
                            .append(&mut plus_button_hours)
                            .append(Text::new(hours_text.as_str(), Point::zero(), sl))
                            .append(&mut minus_button_hours),
                    )
                    .arrange(),
                )
                .append(
                    LinearLayout::horizontal(
                        Chain::new(Text::new("Mins: ", Point::zero(), sl))
                            .append(&mut plus_button_minutes)
                            .append(Text::new(minutes_text.as_str(), Point::zero(), sl))
                            .append(&mut minus_button_minutes),
                    )
                    .arrange(),
                )
                .append(&mut save_button),
        )
        .with_alignment(horizontal::Left)
        .with_spacing(embedded_layout::layout::linear::FixedMargin(5))
        .arrange()
        .align_to(&display_area, horizontal::Center, vertical::Center);

        layout.draw(&mut *ctx.lcd).unwrap();

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
            select::Either4::Second(_d) => {
                orig_settings.apply();
                break;
            }
            select::Either4::Third(_event) => {}
            select::Either4::Fourth(e) => match layout.touch(e, &mut settings) {
                crate::ui::TouchResult::Done(Action::Continue) => {
                    settings.apply();
                }
                crate::ui::TouchResult::Done(Action::Stop) => {
                    ctx.flash.with_fs(|fs| settings.save(fs)).await.unwrap();
                    break;
                }
                crate::ui::TouchResult::Continue => {}
            },
        }
    }
}
