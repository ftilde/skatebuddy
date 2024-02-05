pub use drivers_shared::battery::*;

pub struct BatteryChargeState {}

impl BatteryChargeState {
    pub fn read(&mut self) -> ChargeState {
        ChargeState::Full
    }
}

pub struct AsyncBattery;

const FULL: u32 = 1000; //TODO

impl AsyncBattery {
    pub async fn read(&self) -> Reading {
        Reading { raw: FULL }
    }

    pub fn current(&self) -> CurrentReading {
        CurrentReading { micro_ampere: 0 }
    }

    pub fn current_std(&self) -> CurrentReading {
        CurrentReading { micro_ampere: 0 }
    }

    pub async fn reset(&self) {
        println!("Simulated battery reset");
    }
}
