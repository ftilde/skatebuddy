pub struct HrmRessources;

type I2CInstance = crate::TWI1;

impl HrmRessources {
    pub async fn on<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Hrm<'a> {
        Hrm { _res: self }
    }
}

pub struct Hrm<'a> {
    _res: &'a HrmRessources,
}

impl<'a> Hrm<'a> {
    pub async fn model_number(&mut self) -> u8 {
        33
    }
}
