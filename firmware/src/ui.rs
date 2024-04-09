use embedded_graphics::{
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::{renderer::TextRenderer, Text},
};
use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use drivers::{
    lpm013m1126c::{BWConfig, Rgb111},
    touch::{EventKind, Gesture, TouchEvent},
};

pub struct ButtonStyle<'a, C> {
    pub fill: C,
    pub highlight: C,
    pub font: &'a MonoFont<'a>,
}

pub struct ButtonDefinition<'a, 'b, C> {
    pub position: Point,
    pub size: Size,
    pub style: &'a ButtonStyle<'b, C>,
    pub text: &'a str,
}

impl<'a, 'b, C> ButtonDefinition<'a, 'b, C> {
    fn rect(&self) -> Rectangle {
        Rectangle::new(self.position, self.size)
    }
}

#[cfg_attr(target_arch = "arm", derive(defmt::Format))]
enum ButtonState {
    Down,
    Up,
}

pub struct Button<'a, 'b, C> {
    def: ButtonDefinition<'a, 'b, C>,
    state: ButtonState,
}

impl<'a, 'b, C: PixelColor + Default> Button<'a, 'b, C> {
    pub fn new(def: ButtonDefinition<'a, 'b, C>) -> Self {
        Self {
            def,
            state: ButtonState::Up,
        }
    }

    pub fn render<D, E>(&self, target: &mut D) -> Result<(), E>
    where
        D: DrawTarget<Color = C, Error = E>,
    {
        let (bg, fg) = match self.state {
            ButtonState::Down => (self.def.style.highlight, self.def.style.fill),
            ButtonState::Up => (self.def.style.fill, self.def.style.highlight),
        };

        let textbox_style = TextBoxStyleBuilder::new()
            .alignment(HorizontalAlignment::Center)
            .vertical_alignment(embedded_text::alignment::VerticalAlignment::Middle)
            .build();

        // Create the text box and apply styling options.
        let character_style = MonoTextStyle::new(self.def.style.font, fg);

        let bounds = self.def.rect();
        let text_box =
            TextBox::with_textbox_style(self.def.text, bounds, character_style, textbox_style);

        let style = embedded_graphics::primitives::PrimitiveStyleBuilder::new()
            .stroke_color(fg)
            .stroke_width(1)
            .fill_color(bg)
            .build();

        bounds.into_styled(style).draw(target)?;
        text_box.draw(target)?;
        Ok(())
    }

    pub fn clicked(&mut self, evt: &TouchEvent) -> bool {
        let bounds = self.def.rect();
        if bounds.contains(evt.point()) {
            match (evt.kind, evt.gesture) {
                (EventKind::Press | EventKind::Hold, _) => {
                    self.state = ButtonState::Down;
                }
                (EventKind::Release, Gesture::SinglePress) => {
                    if let ButtonState::Down = self.state {
                        self.state = ButtonState::Up;
                        return true;
                    }
                }
                (EventKind::Release, _) => {
                    self.state = ButtonState::Up;
                }
            }
        } else {
            self.state = ButtonState::Up;
        }
        false
    }
}

impl<'a, 'b, C> embedded_layout::View for Button<'a, 'b, C> {
    fn translate_impl(&mut self, by: Point) {
        self.def.position += by;
    }

    fn bounds(&self) -> Rectangle {
        self.def.rect()
    }
}
impl<'a, 'b, C: PixelColor + Default> embedded_graphics::Drawable for Button<'a, 'b, C> {
    type Color = C;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.render(target)
    }
}

pub struct TextWriter<'a, S> {
    pos: Point,
    line_start: i32,
    display: &'a mut drivers::display::Display,
    style: S,
}

impl<'a, S: TextRenderer<Color = BinaryColor> + Clone> TextWriter<'a, S> {
    pub fn new(display: &'a mut drivers::display::Display, style: S) -> Self {
        Self {
            pos: Point::new(0, 0),
            line_start: 0,
            display,
            style,
        }
    }

    //pub fn x(mut self, x: i32) -> Self {
    //    self.pos.x = x;
    //    self.line_start = x;
    //    self
    //}

    pub fn y(mut self, y: i32) -> Self {
        self.pos.y = y;
        self
    }

    pub fn write(&mut self, text: &str) {
        let bw_config = BWConfig {
            off: Rgb111::black(),
            on: Rgb111::white(),
        };

        let mut new_pos = Text::new(text, self.pos, self.style.clone())
            .draw(&mut self.display.binary(bw_config))
            .unwrap();

        if new_pos.y != self.pos.y {
            // We encountered a newline character
            new_pos.x = self.line_start;
        }
        self.pos = new_pos;
    }
}

impl<S: TextRenderer<Color = BinaryColor> + Clone> core::fmt::Write for TextWriter<'_, S> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s);
        Ok(())
    }
}
