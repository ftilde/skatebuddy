use embedded_graphics::{
    mono_font::{MonoFont, MonoTextStyle},
    prelude::*,
    primitives::Rectangle,
};
use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use crate::touch::{EventKind, TouchEvent};

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

#[derive(defmt::Format)]
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

    pub fn render<D, E>(&self, target: &mut D)
    where
        E: core::fmt::Debug,
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

        bounds.into_styled(style).draw(target).unwrap();
        text_box.draw(target).unwrap();
    }

    pub fn clicked(&mut self, evt: &TouchEvent) -> bool {
        let bounds = self.def.rect();
        if bounds.contains(evt.point()) {
            match evt.kind {
                EventKind::Press | EventKind::Hold => {
                    self.state = ButtonState::Down;
                }
                EventKind::Release => {
                    if let ButtonState::Down = self.state {
                        self.state = ButtonState::Up;
                        return true;
                    }
                }
            }
        } else {
            self.state = ButtonState::Up;
        }
        false
    }
}
