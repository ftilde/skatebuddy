use embassy_futures::join;
use futures::Future;
use smol::{
    channel::{Receiver, Sender},
    Timer,
};

use crate::window::WindowHandle;

use super::lpm013m1126c::{self, Buffer};
use drivers_shared::display::*;

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
    pub async fn on(&mut self) {
        let mut window = self.window.lock().unwrap();
        window.display_on = true;
        window.update_window();
    }

    pub async fn off(&mut self) {
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

    pub async fn present_and<R, F: Future<Output = R>>(&mut self, f: F) -> R {
        let ((), res) = join::join(self.present(), f).await;
        res
    }
}

pub struct Backlight {
    sender: Sender<BacklightCmd>,
    _task: smol::Task<()>,
}

impl Backlight {
    pub fn new(executor: &smol::LocalExecutor, window: crate::window::WindowHandle) -> Self {
        let (sender, receiver) = smol::channel::bounded(1);
        let _task = executor.spawn(async move {
            drive_backlight(receiver, window).await;
        });
        Self { sender, _task }
    }
    async fn set_on(&mut self) {
        self.sender.send(BacklightCmd::On).await.unwrap();
    }

    pub async fn set_off(&mut self) {
        self.sender.send(BacklightCmd::Off).await.unwrap();
    }

    pub async fn active(&mut self) {
        self.sender
            .send(BacklightCmd::ActiveFor {
                secs: DEFAULT_ACTIVE_DURATION,
            })
            .await
            .unwrap();
    }

    #[must_use]
    pub async fn on<'a>(&'a mut self) -> BacklightOn<'a> {
        self.set_on().await;
        BacklightOn { bl: self }
    }
}

pub struct BacklightOn<'a> {
    bl: &'a mut Backlight,
}

impl Drop for BacklightOn<'_> {
    fn drop(&mut self) {
        crate::futures::block_on(self.bl.set_off());
    }
}

async fn drive_backlight(receiver: Receiver<BacklightCmd>, window: WindowHandle) {
    let mut turn_off_after = None;
    loop {
        let v = if let Some(turn_off_after) = turn_off_after.take() {
            match embassy_futures::select::select(
                receiver.recv(),
                Timer::after(std::time::Duration::from_secs(turn_off_after as _)),
            )
            .await
            {
                embassy_futures::select::Either::First(v) => v,
                embassy_futures::select::Either::Second(_) => {
                    let mut window = window.lock().unwrap();
                    window.backlight_on = false;
                    window.update_window();
                    continue;
                }
            }
        } else {
            receiver.recv().await
        };
        match v.unwrap() {
            BacklightCmd::ActiveFor { secs } => {
                turn_off_after = Some(secs);
                let mut window = window.lock().unwrap();
                window.backlight_on = true;
                window.update_window();
            }
            BacklightCmd::Off => {
                let mut window = window.lock().unwrap();
                window.backlight_on = false;
                window.update_window();
            }
            BacklightCmd::On => {
                let mut window = window.lock().unwrap();
                window.backlight_on = true;
                window.update_window();
            }
        }
    }
}
