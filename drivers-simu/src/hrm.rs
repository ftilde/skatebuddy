pub struct HrmRessources;

use std::time::Duration;

pub use drivers_shared::hrm::*;

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
    pub async fn enable(&mut self) {
        println!("Hrm enable");
    }
    pub async fn disable(&mut self) {
        println!("Hrm disable");
    }
    pub async fn wait_event(&mut self) -> (ReadResult, Option<u16>) {
        smol::Timer::after(Duration::from_millis(100)).await;
        (
            ReadResult {
                status: 0,
                irq_status: 0,
                env_value: [0; 3],
                pre_value: [0; 2],
                ps_value: 0,
                pd_res_value: [0; 3],
                current_value: [0; 3],
            },
            None,
        )
    }
}
