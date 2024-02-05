use std::time::Duration;

pub use drivers_shared::touch::*;

pub struct TouchRessources {
    pub window: crate::window::WindowHandle,
}

type I2CInstance = crate::TWI0;

impl TouchRessources {
    pub async fn enabled<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Touch<'a> {
        Touch {
            hw: self,
            prev_down: false,
            prev_pos: (0.0, 0.0),
        }
    }
}

const POLL_PERIOD: Duration = Duration::from_millis(10);

pub struct Touch<'a> {
    #[allow(unused)]
    hw: &'a mut TouchRessources,
    prev_down: bool,
    prev_pos: (f32, f32),
}
impl<'a> Touch<'a> {
    pub async fn wait_for_event(&mut self) -> TouchEvent {
        loop {
            let mut window = self.hw.window.lock().await;
            window.window.update();
            let down = window.window.get_mouse_down(minifb::MouseButton::Left);

            if let Some(pos) = window.window.get_mouse_pos(minifb::MouseMode::Clamp) {
                let x = pos.0 as u8;
                let y = pos.1 as u8;
                let n_points = 1;
                let gesture = Gesture::SinglePress;
                let prev_down = self.prev_down;
                self.prev_down = down;
                let prev_pos = self.prev_pos;
                self.prev_pos = pos;
                match (prev_down, down) {
                    (false, true) => {
                        return TouchEvent {
                            gesture,
                            n_points,
                            kind: EventKind::Press,
                            x,
                            y,
                        }
                    }
                    (true, false) => {
                        return TouchEvent {
                            gesture,
                            n_points,
                            kind: EventKind::Release,
                            x,
                            y,
                        }
                    }
                    (true, true) => {
                        if prev_pos != pos {
                            return TouchEvent {
                                gesture,
                                n_points,
                                kind: EventKind::Hold,
                                x,
                                y,
                            };
                        }
                    }
                    (false, false) => {}
                }
            }

            smol::Timer::after(POLL_PERIOD).await;
        }
    }
    pub async fn wait_for_action(&mut self) -> TouchEvent {
        loop {
            let event = self.wait_for_event().await;
            if let EventKind::Release | EventKind::Press = event.kind {
                return event;
            }
        }
    }
}
