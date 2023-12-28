use core::sync::atomic::{AtomicU32, Ordering};

use crate::hardware::bat as hw;
use embassy_nrf::{
    gpio::{Input, Pull},
    peripherals::SAADC,
    saadc::{self},
};
use embassy_time::{Duration, Instant};

pub struct Battery {
    saadc: SAADC,
    bat_val_pin: hw::VOLTAGE,
}

pub struct BatteryChargeState {
    charge_port_pin: Input<'static, hw::CHARGING>,
    charge_complete_pin: Input<'static, hw::FULL>,
}

pub enum ChargeState {
    Full,
    Charging,
    Draining,
}

impl BatteryChargeState {
    pub fn new(charge_port_pin: hw::CHARGING, charge_complete_pin: hw::FULL) -> Self {
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
}

impl Battery {
    pub fn new(saadc: SAADC, bat_val_pin: hw::VOLTAGE) -> Self {
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

    pub async fn read_specific_median(&mut self, vals: &mut [u32], duration: Duration) -> Reading {
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

    pub async fn read_accurate(&mut self) -> Reading {
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

#[derive(Copy, Clone)]
pub struct Reading {
    raw: u32,
}

const FULL_VOLTAGE_VAL: f32 = 0.3144;

impl Reading {
    pub fn voltage(&self) -> f32 {
        self.raw as f32 * (4.2 / 16384.0 / FULL_VOLTAGE_VAL / (1 << 16) as f32)
    }

    pub fn percentage(&self) -> f32 {
        let voltage = self.voltage();
        let v_100 = 4.2;
        let v_80 = 3.95;
        let v_10 = 3.70;
        let v_0 = 3.3;

        // Piecewise linear approximation as done in espruino
        let percentage = if voltage > v_80 {
            (voltage - v_80) * 20.0 / (v_100 - v_80) + 80.0
        } else if voltage > v_10 {
            (voltage - v_10) * 70.0 / (v_80 - v_10) + 10.0
        } else {
            (voltage - v_0) * 10.0 / (v_10 - v_0)
        };

        percentage
    }
}

static LAST_ASYNC_READING: AtomicU32 = AtomicU32::new(0);
static LAST_ASYNC_CURRENT: AtomicU32 = AtomicU32::new(0);
static LAST_ASYNC_CURRENT_STD: AtomicU32 = AtomicU32::new(0);

static ASYNC_BATTERY_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    BatteryCommand,
> = embassy_sync::signal::Signal::new();

enum BatteryCommand {
    Reset,
}

const ASYNC_BATTERY_PERIOD: Duration = Duration::from_secs(20 * 60);

#[embassy_executor::task]
async fn accurate_battery_task(mut battery: Battery) {
    loop {
        LAST_ASYNC_CURRENT.store(0, Ordering::Relaxed);
        LAST_ASYNC_CURRENT_STD.store(u32::MAX, Ordering::Relaxed);
        let reading = battery.read_accurate().await;
        LAST_ASYNC_READING.store(reading.raw, Ordering::Relaxed);
        crate::signal_display_event(crate::DisplayEvent::NewBatData);

        let current_estimator = CurrentEstimator::init(reading);

        let mut ticker = embassy_time::Ticker::every(ASYNC_BATTERY_PERIOD);
        loop {
            if let embassy_futures::select::Either::Second(cmd) =
                embassy_futures::select::select(ticker.next(), ASYNC_BATTERY_SIG.wait()).await
            {
                ASYNC_BATTERY_SIG.reset();
                let BatteryCommand::Reset = cmd;
                break;
            }

            let reading = battery.read_accurate().await;
            LAST_ASYNC_READING.store(reading.raw, Ordering::Relaxed);
            let current = current_estimator.next(reading);
            let std = current_estimator.deviation();
            LAST_ASYNC_CURRENT.store(current.micro_ampere, Ordering::Relaxed);
            LAST_ASYNC_CURRENT_STD.store(std.micro_ampere, Ordering::Relaxed);
            crate::signal_display_event(crate::DisplayEvent::NewBatData);
        }
    }
}

pub struct AsyncBattery;

impl AsyncBattery {
    pub fn new(spawner: &embassy_executor::Spawner, battery: Battery) -> Self {
        spawner.spawn(accurate_battery_task(battery)).unwrap();
        Self
    }

    pub async fn read(&self) -> Reading {
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

    pub async fn reset(&self) {
        ASYNC_BATTERY_SIG.signal(BatteryCommand::Reset);
        embassy_futures::yield_now().await;
    }
}

#[allow(unused)]
pub struct CurrentEstimator {
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

#[allow(unused)]
#[derive(Copy, Clone)]
pub struct CurrentReading {
    micro_ampere: u32,
}

#[allow(unused)]
impl CurrentReading {
    pub fn micro_ampere(self) -> u32 {
        self.micro_ampere
    }
}
