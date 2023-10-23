use crate::hardware::bat as hw;
use embassy_nrf::{
    gpio::{Input, Pull},
    peripherals::SAADC,
    saadc::{self, Saadc},
};
use embassy_time::{Duration, Instant};

pub struct Battery<'a> {
    saadc: Saadc<'a, 1>,
}

pub struct BatteryChargeState<'a> {
    charge_port_pin: Input<'a, hw::CHARGING>,
    charge_complete_pin: Input<'a, hw::FULL>,
}

pub enum ChargeState {
    Full,
    Charging,
    Draining,
}

impl<'a> BatteryChargeState<'a> {
    pub fn new(charge_port_pin: hw::CHARGING, charge_complete_pin: hw::FULL) -> Self {
        let charge_port_pin = Input::new(charge_port_pin, Pull::None);
        let charge_complete_pin = Input::new(charge_complete_pin, Pull::None);

        Self {
            charge_port_pin,
            charge_complete_pin,
        }
    }
    pub fn read(&self) -> ChargeState {
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

impl<'a> Battery<'a> {
    pub fn new(saadc: SAADC, bat_val_pin: hw::VOLTAGE) -> Self {
        let mut config = saadc::Config::default();
        config.resolution = saadc::Resolution::_14BIT;
        config.oversample = saadc::Oversample::OVER256X;

        let mut channel_config = saadc::ChannelConfig::single_ended(bat_val_pin);
        channel_config.reference = saadc::Reference::VDD1_4;
        channel_config.gain = saadc::Gain::GAIN1_4;
        channel_config.time = saadc::Time::_3US;

        let saadc = saadc::Saadc::new(
            saadc,
            crate::Irqs, /*TODO: not sure if this is correct */
            config,
            [channel_config],
        );
        Self { saadc }
    }

    pub async fn read_specific_mean(&mut self, n: u32, duration: Duration) -> Reading {
        assert!(n > 0);
        let mut sum = 0;
        for _ in 0..n {
            let mut bat_buf = [0; 1];
            embassy_time::Timer::after(duration).await;
            self.saadc.sample(&mut bat_buf).await;
            sum += (bat_buf[0] as u32) << 8;
        }
        let mean = sum / n;
        Reading { raw: mean << 8 }
    }

    pub async fn read_specific_median(&mut self, vals: &mut [u32], duration: Duration) -> Reading {
        assert!(vals.len() > 0);
        for v in vals.iter_mut() {
            let mut bat_buf = [0; 1];
            embassy_time::Timer::after(duration).await;
            self.saadc.sample(&mut bat_buf).await;
            *v = bat_buf[0] as u32;
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
    pub async fn read(&mut self) -> Reading {
        //let mut vals = [0; 17];
        //self.read_specific_median(&mut vals, Duration::from_millis(100))
        //    .await
        self.read_specific_mean(1, Duration::from_millis(0)).await
    }
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

//static LAST_ASYNC_READING: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
//
//#[embassy_executor::task]
//async fn accurate_battery_task(mut battery: Battery<'static>) {
//    let wait_time = Duration::from_secs(60);
//
//    let reading = battery.read_accurate().await;
//    LAST_ASYNC_READING.store(reading.raw, core::sync::atomic::Ordering::Relaxed);
//
//    let mut ticker = embassy_time::Ticker::every(wait_time);
//    loop {
//        ticker.next().await;
//
//        let reading = battery.read_accurate().await;
//        LAST_ASYNC_READING.store(reading.raw, core::sync::atomic::Ordering::Relaxed);
//    }
//}
//
//pub struct AsyncBattery;
//
//impl AsyncBattery {
//    pub fn new(spawner: &embassy_executor::Spawner, battery: Battery<'static>) -> Self {
//        spawner.spawn(accurate_battery_task(battery)).unwrap();
//        Self
//    }
//
//    pub async fn read(&mut self) -> Reading {
//        Reading {
//            raw: LAST_ASYNC_READING.load(core::sync::atomic::Ordering::Relaxed),
//        }
//    }
//}

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
