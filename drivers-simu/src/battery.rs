use std::{sync::atomic::AtomicU32, time::Duration};

pub use drivers_shared::battery::*;
use minifb::Key;
use smol::LocalExecutor;

use crate::{util::KeyState, window::WindowHandle};

pub struct BatteryChargeState {}

impl BatteryChargeState {
    pub fn read(&mut self) -> ChargeState {
        ChargeState::Full
    }
}

pub fn voltage_to_reading(voltage: f32) -> Reading {
    let raw = (voltage / (4.2 / 16384.0 / FULL_VOLTAGE_VAL / (1 << 16) as f32)) as u32;
    Reading { raw }
}
const V_100: f32 = 4.2;
const V_0: f32 = 3.3;

//#[derive(Copy, Clone)]
//enum Mode {
//    Draining,
//    Charging,
//}

pub struct AsyncBattery {
    _update_thread: smol::Task<()>,
}

static LATEST_READING: AtomicU32 = AtomicU32::new(0);
async fn update_reading(voltage: f32) {
    LATEST_READING.store(
        voltage_to_reading(voltage).raw,
        std::sync::atomic::Ordering::SeqCst,
    );
    crate::signal_display_event(crate::DisplayEvent::NewBatData).await;
}

impl AsyncBattery {
    pub fn new(executor: &LocalExecutor, window: WindowHandle) -> Self {
        Self {
            _update_thread: executor.spawn(async move {
                let mut key_drain = KeyState::new(Key::D);
                let mut key_charge = KeyState::new(Key::C);
                let mut voltage = (V_100 + V_0) * 0.5;
                update_reading(voltage).await;
                loop {
                    let mut changed = false;
                    {
                        let mut window = window.lock().unwrap();
                        window.window.update();
                        if key_drain.pressed(&mut window) {
                            voltage = (voltage - 0.1).max(V_0);
                            changed = true;
                        }
                        if key_charge.pressed(&mut window) {
                            voltage = (voltage + 0.1).min(V_100);
                            changed = true;
                        }
                    }
                    if changed {
                        update_reading(voltage).await;
                    }
                    crate::time::Timer::after(Duration::from_millis(20)).await;
                }
            }),
        }
    }
    pub async fn read(&self) -> Reading {
        Reading {
            raw: LATEST_READING.load(std::sync::atomic::Ordering::SeqCst),
        }
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
