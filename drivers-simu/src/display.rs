use super::lpm013m1126c::{self, Buffer};

pub struct Display {
    buffer: lpm013m1126c::Buffer,
    window: crate::window::WindowHandle,
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
    pub(crate) fn new(window: crate::window::WindowHandle) -> Self {
        Self {
            buffer: Default::default(),
            window,
        }
    }
    pub fn on(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.display_on = true;
        window.update_window();
    }

    pub fn off(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.display_on = false;
        window.update_window();
    }

    pub async fn clear(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.clear_buffer();
    }

    pub async fn blink(&mut self, mode: lpm013m1126c::BlinkMode) {
        let mut window = self.window.lock().unwrap();
        window.blink_mode = mode;
        window.update_window();
    }

    pub async fn present(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.present(&mut self.buffer)
    }
}

pub struct Backlight {
    pub window: crate::window::WindowHandle,
}

impl Backlight {
    pub fn set_on(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.backlight_on = true;
        window.update_window();
    }

    pub fn set_off(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.backlight_on = false;
        window.update_window();
    }

    #[must_use]
    pub fn on<'a>(&'a mut self) -> BacklightOn<'a> {
        self.set_on();
        BacklightOn { bl: self }
    }
}

pub struct BacklightOn<'a> {
    bl: &'a mut Backlight,
}

impl Drop for BacklightOn<'_> {
    fn drop(&mut self) {
        self.bl.set_off();
    }
}
