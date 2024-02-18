use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use super::hardware::bat as hw;
use embassy_nrf::{
    gpio::{Input, Pull},
    peripherals::SAADC,
    saadc::{self},
};
use embassy_time::{Duration, Instant};

pub use drivers_shared::battery::*;

pub(crate) struct Battery {
    saadc: SAADC,
    bat_val_pin: hw::VOLTAGE,
}

pub struct BatteryChargeState {
    charge_port_pin: Input<'static, hw::CHARGING>,
    charge_complete_pin: Input<'static, hw::FULL>,
}

impl BatteryChargeState {
    pub(crate) fn new(charge_port_pin: hw::CHARGING, charge_complete_pin: hw::FULL) -> Self {
        let charge_port_pin = Input::new(charge_port_pin, Pull::None);
        let charge_complete_pin = Input::new(charge_complete_pin, Pull::None);

        Self {
            charge_port_pin,
            charge_complete_pin,
        }
    }
    pub fn read(&mut self) -> ChargeState {
        if self.charge_port_pin.is_low() {
            if self.charge_complete_pin.is_low() {
                ChargeState::Charging
            } else {
                ChargeState::Full
            }
        } else {
            ChargeState::Draining
        }
    }

    pub async fn wait_change(&mut self) {
        embassy_futures::select::select(
            self.charge_port_pin.wait_for_any_edge(),
            self.charge_complete_pin.wait_for_any_edge(),
        )
        .await;
    }
}

impl Battery {
    pub(crate) fn new(saadc: SAADC, bat_val_pin: hw::VOLTAGE) -> Self {
        Self { saadc, bat_val_pin }
    }

    async fn read_sample(&mut self) -> i16 {
        let mut config = saadc::Config::default();
        config.resolution = saadc::Resolution::_14BIT;
        config.oversample = saadc::Oversample::OVER256X;

        let mut channel_config = saadc::ChannelConfig::single_ended(&mut self.bat_val_pin);
        channel_config.reference = saadc::Reference::VDD1_4;
        channel_config.gain = saadc::Gain::GAIN1_4;
        channel_config.time = saadc::Time::_3US;

        let mut saadc = saadc::Saadc::new(&mut self.saadc, crate::Irqs, config, [channel_config]);

        let mut bat_buf = [0; 1];
        saadc.sample(&mut bat_buf).await;
        bat_buf[0]
    }

    //pub async fn read_specific_mean(&mut self, n: u32, duration: Duration) -> Reading {
    //    assert!(n > 0);
    //    let mut sum = 0;
    //    for _ in 0..n {
    //        embassy_time::Timer::after(duration).await;
    //        let sample = self.read_sample().await;
    //        sum += (sample as u32) << 8;
    //    }
    //    let mean = sum / n;
    //    Reading { raw: mean << 8 }
    //}

    async fn read_specific_median(&mut self, vals: &mut [u32], duration: Duration) -> Reading {
        assert!(vals.len() > 0);
        for v in vals.iter_mut() {
            embassy_time::Timer::after(duration).await;
            let sample = self.read_sample().await;
            *v = sample as u32;
        }
        //defmt::println!("vals: {:?}", vals);
        let (_, med, _) = vals.select_nth_unstable(vals.len() / 2);
        //defmt::println!("med: {:?}", med);
        Reading { raw: *med << 16 }
    }

    async fn read_accurate(&mut self) -> Reading {
        let mut vals = [0; 17];
        self.read_specific_median(&mut vals, Duration::from_millis(100))
            .await
        //self.read_specific_mean(16, Duration::from_millis(100))
        //    .await
    }
    //pub async fn read(&mut self) -> Reading {
    //    //let mut vals = [0; 17];
    //    //self.read_specific_median(&mut vals, Duration::from_millis(100))
    //    //    .await
    //    self.read_specific_mean(1, Duration::from_millis(0)).await
    //}
}

static LAST_ASYNC_READING: AtomicU32 = AtomicU32::new(0);
static LAST_ASYNC_CURRENT: AtomicU32 = AtomicU32::new(0);
static LAST_ASYNC_CURRENT_STD: AtomicU32 = AtomicU32::new(0);
static LAST_CHARGE_STATE: AtomicU8 = AtomicU8::new(ChargeState::Draining as u8);
static LAST_UPDATE_TIME: AtomicU32 = AtomicU32::new(0);

static ASYNC_BATTERY_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    BatteryCommand,
> = embassy_sync::signal::Signal::new();

enum BatteryCommand {
    Reset,
    Update,
}

fn async_bat_wait_period(state: ChargeState) -> Duration {
    match state {
        ChargeState::Full => Duration::from_secs(60 * 60),
        ChargeState::Charging => Duration::from_secs(60),
        ChargeState::Draining => Duration::from_secs(10 * 60),
    }
}

#[embassy_executor::task]
async fn accurate_battery_task(mut battery: Battery, mut charge_state: BatteryChargeState) {
    loop {
        LAST_ASYNC_CURRENT.store(0, Ordering::Relaxed);
        LAST_ASYNC_CURRENT_STD.store(u32::MAX, Ordering::Relaxed);
        let reading = battery.read_accurate().await;
        LAST_ASYNC_READING.store(reading.raw, Ordering::Relaxed);
        crate::signal_display_event(crate::DisplayEvent::NewBatData);
        let state = charge_state.read();
        LAST_CHARGE_STATE.store(state as u8, Ordering::Relaxed);

        let current_estimator = CurrentEstimator::init(reading);

        let mut wait_duration = async_bat_wait_period(state);

        loop {
            let timer = embassy_time::Timer::after(wait_duration);
            if let embassy_futures::select::Either3::Second(cmd) = embassy_futures::select::select3(
                timer,
                ASYNC_BATTERY_SIG.wait(),
                charge_state.wait_change(),
            )
            .await
            {
                ASYNC_BATTERY_SIG.reset();
                match cmd {
                    BatteryCommand::Reset => {
                        break;
                    }
                    BatteryCommand::Update => {
                        // Nothing, just take a reading now
                    }
                }
            }

            let reading = battery.read_accurate().await;
            LAST_ASYNC_READING.store(reading.raw, Ordering::Relaxed);
            let current = current_estimator.next(reading);
            let std = current_estimator.deviation();
            LAST_ASYNC_CURRENT.store(current.micro_ampere, Ordering::Relaxed);
            LAST_ASYNC_CURRENT_STD.store(std.micro_ampere, Ordering::Relaxed);
            let state = charge_state.read();
            LAST_CHARGE_STATE.store(state as u8, Ordering::Relaxed);
            LAST_UPDATE_TIME.store(Instant::now().as_secs() as u32, Ordering::Relaxed);
            crate::signal_display_event(crate::DisplayEvent::NewBatData);

            wait_duration = async_bat_wait_period(state);
        }
    }
}

pub struct AsyncBattery;

impl AsyncBattery {
    pub(crate) fn new(
        spawner: &embassy_executor::Spawner,
        battery: Battery,
        charge_state: BatteryChargeState,
    ) -> Self {
        spawner
            .spawn(accurate_battery_task(battery, charge_state))
            .unwrap();
        Self
    }

    pub fn read(&self) -> Reading {
        Reading {
            raw: LAST_ASYNC_READING.load(Ordering::Relaxed),
        }
    }

    pub fn current(&self) -> CurrentReading {
        CurrentReading {
            micro_ampere: LAST_ASYNC_CURRENT.load(Ordering::Relaxed),
        }
    }

    pub fn current_std(&self) -> CurrentReading {
        CurrentReading {
            micro_ampere: LAST_ASYNC_CURRENT_STD.load(Ordering::Relaxed),
        }
    }

    pub fn last_update(&self) -> Instant {
        Instant::from_secs(LAST_UPDATE_TIME.load(Ordering::Relaxed) as u64)
    }

    pub fn state(&self) -> ChargeState {
        <ChargeState as drivers_shared::num_enum::TryFromPrimitive>::try_from_primitive(
            LAST_CHARGE_STATE.load(Ordering::Relaxed),
        )
        .unwrap()
    }

    pub async fn reset(&self) {
        ASYNC_BATTERY_SIG.signal(BatteryCommand::Reset);
        embassy_futures::yield_now().await;
    }

    pub async fn force_update(&self) {
        ASYNC_BATTERY_SIG.signal(BatteryCommand::Update);
        embassy_futures::yield_now().await;
    }
}

struct CurrentEstimator {
    last_percentage: f32,
    last_time: Instant,
}

#[allow(unused)]
const TOTAL_CAPACITY_MAH: f32 = 175.0;

fn calc_current(duration: Duration, percentage_now: f32, percentage_prev: f32) -> CurrentReading {
    let dms = duration.as_millis();

    let dp = (percentage_prev - percentage_now).max(0.0);

    let dmah = dp * 0.01 * TOTAL_CAPACITY_MAH;
    const MS_PER_HOUR: f32 = 60.0 * 60.0 * 1000.0;
    let dmams = dmah * MS_PER_HOUR;
    let ma = dmams / dms as f32;
    let mua = (ma * 1000.0) as u32;

    //defmt::println!("prev: {}", percentage_prev);
    //defmt::println!("now : {}", percentage_now);

    CurrentReading { micro_ampere: mua }
}

#[allow(unused)]
impl CurrentEstimator {
    pub fn init(reading: Reading) -> Self {
        Self {
            last_percentage: reading.percentage(),
            last_time: Instant::now(),
        }
    }

    pub fn reset(&mut self, reading: Reading) {
        self.last_percentage = reading.percentage();
        self.last_time = Instant::now();
    }

    pub fn next(&self, reading: Reading) -> CurrentReading {
        let now = Instant::now();
        let dt = now - self.last_time;

        calc_current(dt, reading.percentage(), self.last_percentage)
    }

    pub fn deviation(&self) -> CurrentReading {
        let now = Instant::now();
        let dt = now - self.last_time;

        let assumed_reading_std = 1;
        let base = 4700;
        let r1 = base << 16;
        let r2 = (base + assumed_reading_std) << 16;
        let r1 = Reading { raw: r1 }.percentage();
        let r2 = Reading { raw: r2 }.percentage();

        calc_current(dt, r1, r2)
    }
}
