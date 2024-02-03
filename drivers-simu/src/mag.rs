pub struct MagRessources {}

type I2CInstance = crate::TWI1;

impl MagRessources {
    pub async fn on<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Mag<'a> {
        Mag { ressources: self }
    }
}

pub struct Mag<'a> {
    #[allow(unused)]
    ressources: &'a mut MagRessources,
}

impl<'a> Mag<'a> {
    pub async fn read(&mut self) -> [u8; 7] {
        [0, 1, 2, 3, 4, 5, 6]
    }
}
