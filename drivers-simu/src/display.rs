use super::lpm013m1126c::{self, Buffer};

pub struct Display {
    buffer: lpm013m1126c::Buffer,
}

impl core::ops::Deref for Display {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl core::ops::DerefMut for Display {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Display {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Default::default(),
        }
    }
    pub fn on(&mut self) {}

    pub fn off(&mut self) {}

    pub async fn clear(&mut self) {}

    pub async fn blink(&mut self, _mode: lpm013m1126c::BlinkMode) {}

    pub async fn present(&mut self) {
        todo!();
    }
}

pub struct Backlight {}

impl Backlight {
    pub fn on(&mut self) {}

    pub fn off(&mut self) {}
}
