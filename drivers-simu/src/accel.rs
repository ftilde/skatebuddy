pub use drivers_shared::accel::*;

pub struct AccelRessources {}

type I2CInstance = crate::TWI1;

impl AccelRessources {
    pub async fn on<'a>(&'a mut self, _instance: &'a mut I2CInstance, config: Config) -> Accel<'a> {
        Accel {
            ressources: self,
            config,
        }
    }
}

#[allow(unused)]
pub struct Accel<'a> {
    ressources: &'a mut AccelRessources,
    config: Config,
}

impl<'a> Accel<'a> {
    pub async fn reading_hf(&mut self) -> Reading {
        Reading { x: 0, y: 0, z: 0 }
    }

    pub async fn reading_nf(&mut self) -> Reading {
        Reading { x: 0, y: 0, z: 0 }
    }
}
