pub struct HrmRessources;

use std::time::Duration;

pub use drivers_shared::hrm::*;

type I2CInstance = crate::TWI1;

impl HrmRessources {
    pub async fn on<'a>(&'a mut self, _i2c: &'a mut I2CInstance) -> Hrm<'a> {
        Hrm {
            _res: self,
            elapsed_millis: 0,
        }
    }
}

pub struct Hrm<'a> {
    _res: &'a HrmRessources,
    elapsed_millis: u64,
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
    pub async fn wait_event(&mut self) -> (ReadResult, Option<Vec<u16>>) {
        let num_samples = 8;
        let delay_per_sample_ms = 40;
        smol::Timer::after(Duration::from_millis(delay_per_sample_ms * 8)).await;
        let mut vals = Vec::new();
        for _ in 0..num_samples {
            let ms = self.elapsed_millis as f32;
            self.elapsed_millis += delay_per_sample_ms;
            let beats_per_ms = 2.1 / 1000.0;
            let beat = ms * beats_per_ms;
            let norm_val = (beat * std::f32::consts::TAU).sin();
            let val = ((norm_val * 0.1 + 1.0) * 1024.0) as u16;
            vals.push(val);
        }
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
            Some(vals),
        )
    }
}
