pub use drivers_shared::touch::*;

pub struct TouchRessources {}

type I2CInstance = crate::TWI0;

impl TouchRessources {
    pub async fn enabled<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Touch<'a> {
        Touch { hw: self }
    }
}

pub struct Touch<'a> {
    #[allow(unused)]
    hw: &'a mut TouchRessources,
}
impl<'a> Touch<'a> {
    pub async fn wait_for_event(&mut self) -> TouchEvent {
        smol::future::pending().await
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
