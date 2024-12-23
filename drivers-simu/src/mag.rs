use crate::time::{Duration, Timer};

pub struct MagRessources {}

impl MagRessources {
    pub async fn on<'a>(&'a mut self, _i2c: &'a crate::TWI) -> Mag<'a> {
        Mag { ressources: self }
    }
}

pub struct Mag<'a> {
    #[allow(unused)]
    ressources: &'a mut MagRessources,
}

impl<'a> Mag<'a> {
    pub async fn read(&mut self) -> [u8; 7] {
        Timer::after(Duration::from_millis(100)).await;
        [0, 1, 2, 3, 4, 5, 6]
    }
}
