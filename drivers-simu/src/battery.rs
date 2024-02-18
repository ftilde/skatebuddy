use std::{
    sync::atomic::{AtomicU32, AtomicU8, Ordering},
    time::{Duration, Instant},
};

pub use drivers_shared::battery::*;
use minifb::Key;
use smol::LocalExecutor;

use crate::{util::KeyState, window::WindowHandle};

pub fn voltage_to_reading(voltage: f32) -> Reading {
    let raw = (voltage / (4.2 / 16384.0 / FULL_VOLTAGE_VAL / (1 << 16) as f32)) as u32;
    Reading { raw }
}
const V_100: f32 = 4.2;
const V_0: f32 = 3.3;

pub struct AsyncBattery {
    _update_thread: smol::Task<()>,
}

static LATEST_READING: AtomicU32 = AtomicU32::new(0);
static LAST_CHARGE_STATE: AtomicU8 = AtomicU8::new(ChargeState::Draining as u8);
static LAST_UPDATE_TIME: AtomicU32 = AtomicU32::new(0);

async fn update_reading(voltage: f32) {
    LATEST_READING.store(voltage_to_reading(voltage).raw, Ordering::SeqCst);
    LAST_UPDATE_TIME.store(
        crate::time::BOOT.elapsed().as_secs() as u32,
        Ordering::Relaxed,
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
                            LAST_CHARGE_STATE.store(ChargeState::Draining as u8, Ordering::SeqCst);
                            changed = true;
                        }
                        if key_charge.pressed(&mut window) {
                            voltage = (voltage + 0.1).min(V_100);
                            LAST_CHARGE_STATE.store(
                                if voltage == V_100 {
                                    ChargeState::Full
                                } else {
                                    ChargeState::Charging
                                } as u8,
                                Ordering::SeqCst,
                            );
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
            raw: LATEST_READING.load(Ordering::SeqCst),
        }
    }

    pub fn current(&self) -> CurrentReading {
        CurrentReading { micro_ampere: 0 }
    }

    pub fn current_std(&self) -> CurrentReading {
        CurrentReading { micro_ampere: 0 }
    }

    pub fn last_update(&self) -> Instant {
        *crate::time::BOOT + Duration::from_secs(LAST_UPDATE_TIME.load(Ordering::Relaxed) as u64)
    }

    pub fn state(&self) -> ChargeState {
        <ChargeState as drivers_shared::num_enum::TryFromPrimitive>::try_from_primitive(
            LAST_CHARGE_STATE.load(Ordering::Relaxed),
        )
        .unwrap()
    }

    pub async fn reset(&self) {
        println!("Simulated battery reset");
    }

    pub async fn force_update(&self) {
        println!("Simulated forced bat read");
        crate::signal_display_event(crate::DisplayEvent::NewBatData).await;
    }
}
