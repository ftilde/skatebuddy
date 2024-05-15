pub struct HrmRessources;

use std::time::{Duration, Instant};

pub use drivers_shared::hrm::*;

type I2CInstance = crate::TWI1;

impl HrmRessources {
    pub async fn on<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Hrm<'a> {
        Hrm {
            _res: self,
            start: Instant::now(),
        }
    }
}

pub struct Hrm<'a> {
    _res: &'a HrmRessources,
    start: Instant,
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
        let ms = self.start.elapsed().as_millis() as f32;
        let beats_per_ms = 2.1 / 1000.0;
        let beat = ms * beats_per_ms;
        let norm_val = (beat * std::f32::consts::TAU).sin();
        let val = ((norm_val * 0.1 + 1.0) * 1024.0) as u16;
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
            Some(val),
        )
    }
}
