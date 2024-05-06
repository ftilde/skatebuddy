use arrform::ArrForm;
use embedded_graphics::{
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
    text::{renderer::TextRenderer, Text},
};
use embedded_layout::layout::linear::LinearLayout;
use embedded_text::{alignment::HorizontalAlignment, style::TextBoxStyleBuilder, TextBox};

use drivers::{
    lpm013m1126c::{BWConfig, Rgb111},
    touch::{EventKind, Gesture, TouchEvent},
};

pub enum TouchResult<R> {
    Done(R),
    Continue,
}

pub trait EventHandler<Context, R> {
    fn touch(&mut self, e: TouchEvent, ctx: &mut Context) -> TouchResult<R>;
}

impl<C, Context, R> EventHandler<Context, R> for Text<'_, C> {
    fn touch(&mut self, _e: TouchEvent, _ctx: &mut Context) -> TouchResult<R> {
        TouchResult::Continue
    }
}
impl<Context, R, I: EventHandler<Context, R>> EventHandler<Context, R>
    for embedded_layout::object_chain::Chain<I>
{
    fn touch(&mut self, e: TouchEvent, ctx: &mut Context) -> TouchResult<R> {
        self.object.touch(e, ctx)
    }
}

impl<
        Context,
        R,
        I: EventHandler<Context, R>,
        IC: EventHandler<Context, R> + embedded_layout::object_chain::ChainElement,
    > EventHandler<Context, R> for embedded_layout::object_chain::Link<I, IC>
{
    fn touch(&mut self, e: TouchEvent, ctx: &mut Context) -> TouchResult<R> {
        if let TouchResult::Done(r) = self.object.touch(e, ctx) {
            TouchResult::Done(r)
        } else {
            self.parent.touch(e, ctx)
        }
    }
}

impl<
        Context,
        R,
        LD: embedded_layout::layout::linear::Orientation,
        VG: embedded_layout::view_group::ViewGroup + EventHandler<Context, R>,
    > EventHandler<Context, R> for LinearLayout<LD, VG>
{
    fn touch(&mut self, e: TouchEvent, ctx: &mut Context) -> TouchResult<R> {
        self.inner_mut().touch(e, ctx)
    }
}

pub struct ButtonStyle<'a, C> {
    pub fill: C,
    pub highlight: C,
    pub font: &'a MonoFont<'a>,
}

#[derive(Copy, Clone)]
pub struct ButtonDefinition<'a, 'b, C> {
    pub position: Point,
    pub size: Size,
    pub style: &'a ButtonStyle<'b, C>,
    pub text: &'a str,
}

impl<'a, 'b, C> ButtonDefinition<'a, 'b, C> {
    pub fn new(style: &'a ButtonStyle<'b, C>, size: Size, text: &'a str) -> Self {
        Self {
            style,
            size,
            text,
            position: Point::new(0, 0),
        }
    }
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

impl<'a, 'b, C: PixelColor> From<ButtonDefinition<'a, 'b, C>> for Button<'a, 'b, C> {
    fn from(def: ButtonDefinition<'a, 'b, C>) -> Self {
        Self {
            def,
            state: ButtonState::Up,
        }
    }
}

impl<'a, 'b, C: PixelColor> Button<'a, 'b, C> {
    pub fn new(style: &'a ButtonStyle<'b, C>, size: Size, text: &'a str) -> Self {
        Self {
            def: ButtonDefinition::new(style, size, text),
            state: ButtonState::Up,
        }
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

    pub fn on_click<Context, R>(
        self,
        action: fn(&mut Context) -> R,
    ) -> ActionButton<'a, 'b, C, Context, R> {
        ActionButton {
            inner: self,
            action,
        }
    }
}

impl<'a, 'b, C: PixelColor + Default> Button<'a, 'b, C> {
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
}

impl<'r, 'a, 'b, C> embedded_layout::View for &'r mut Button<'a, 'b, C> {
    fn translate_impl(&mut self, by: Point) {
        self.def.position += by;
    }

    fn bounds(&self) -> Rectangle {
        self.def.rect()
    }
}
impl<'r, 'a, 'b, C: PixelColor + Default> embedded_graphics::Drawable
    for &'r mut Button<'a, 'b, C>
{
    type Color = C;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.render(target)
    }
}

pub struct ActionButton<'a, 'b, C, Context, R> {
    inner: Button<'a, 'b, C>,
    action: fn(&mut Context) -> R,
}

impl<'r, 'a, 'b, C: PixelColor + Default, Context, R> embedded_layout::View
    for &'r mut ActionButton<'a, 'b, C, Context, R>
{
    fn translate_impl(&mut self, by: Point) {
        self.inner.def.position += by;
    }

    fn bounds(&self) -> Rectangle {
        self.inner.def.rect()
    }
}
impl<'r, 'a, 'b, C: PixelColor + Default, Context, R> embedded_graphics::Drawable
    for &'r mut ActionButton<'a, 'b, C, Context, R>
{
    type Color = C;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.inner.render(target)
    }
}

impl<'r, 'a, 'b, C: PixelColor, Context, R> EventHandler<Context, R>
    for &'r mut ActionButton<'a, 'b, C, Context, R>
{
    fn touch(&mut self, e: TouchEvent, ctx: &mut Context) -> TouchResult<R> {
        if self.inner.clicked(&e) {
            TouchResult::Done((self.action)(ctx))
        } else {
            TouchResult::Continue
        }
    }
}

pub struct Label<'a, const SIZE: usize, C> {
    pub style: MonoTextStyle<'a, C>,
    buffer: ArrForm<SIZE>,
    pub position: Point,
}

impl<'a, const SIZE: usize, C: PixelColor> Label<'a, SIZE, C> {
    pub fn new(buffer: ArrForm<SIZE>, style: MonoTextStyle<'a, C>) -> Self {
        Self {
            style,
            buffer,
            position: Point::zero(),
        }
    }
    pub fn text<'r>(&'r self) -> Text<'r, MonoTextStyle<'a, C>> {
        Text::new(self.buffer.as_str(), self.position, self.style)
    }
    pub fn set<'r>(&'r mut self) -> LabelWrite<'r, 'a, SIZE, C> {
        self.buffer = arrform::ArrForm::new();
        LabelWrite { inner: self }
    }
    pub fn render<D: DrawTarget<Color = C>>(&self, target: &mut D) -> Result<Point, D::Error> {
        self.text().draw(target)
    }
}

pub struct LabelWrite<'r, 'a, const SIZE: usize, C> {
    inner: &'r mut Label<'a, SIZE, C>,
}

impl<'a, const SIZE: usize, C: PixelColor> core::fmt::Write for LabelWrite<'_, '_, SIZE, C> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.inner.buffer.write_str(s)
    }
}

impl<'r, 'a, const SIZE: usize, C: PixelColor> embedded_graphics::Drawable
    for &'r mut Label<'a, SIZE, C>
{
    type Color = C;

    type Output = Point;

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.render(target)
    }
}

impl<'r, 'a, const SIZE: usize, C: PixelColor> embedded_layout::View
    for &'r mut Label<'a, SIZE, C>
{
    fn translate_impl(&mut self, by: Point) {
        self.position += by;
    }

    fn bounds(&self) -> Rectangle {
        self.text().bounds()
    }
}
impl<'r, 'a, const SIZE: usize, C: PixelColor, Context, R> EventHandler<Context, R>
    for &'r mut Label<'a, SIZE, C>
{
    fn touch(&mut self, _e: TouchEvent, _ctx: &mut Context) -> TouchResult<R> {
        TouchResult::Continue
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
