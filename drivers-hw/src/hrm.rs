use crate::twi::{TwiHandle, Twim, TWI};

use super::hardware::hrm as hw;

use arrayvec::ArrayVec;
pub use drivers_shared::hrm::*;

use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    twim,
};

const PS_WEARING_THRESHOLD: u8 = 6;

use embassy_time::{Duration, Timer};

pub struct HrmRessources {
    scl: hw::SCL,
    sda: hw::SDA,
    enabled: Output<'static, hw::EN>,
    irq: Input<'static, hw::IRQ>,
}

impl HrmRessources {
    pub(crate) fn new(sda: hw::SDA, scl: hw::SCL, enabled: hw::EN, irq: hw::IRQ) -> Self {
        Self {
            sda,
            scl,
            enabled: Output::new(enabled, Level::Low, OutputDrive::Standard),
            irq: Input::new(irq, Pull::None),
        }
    }

    pub async fn on<'a>(&'a mut self, i2c: &'a TWI) -> Hrm<'a> {
        self.enabled.set_high();
        // Wait for boot
        Timer::after(Duration::from_millis(1)).await;

        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        let reg_config = RegConfig::default();

        // Make reading start at zero in first batch by "faking" an adjust event. (Although
        // in a way we ARE adjusting by starting the device...)
        let adjust_event = Some(AdjustEvent { last_value: 0 });

        let state = HrmState {
            wearing: false,
            slots: 0,
            hrm_led_config: LedConfig::from_reg(reg_config.slot0_led_current),
            hrm_led_max_current: 0x6f,
            adjust_mode: AdjustMode::Stable,
            fifo_read_index: FIFO_RANGE_BEGIN as u8,
            hrm_res_config: PdResConfig::from_reg(reg_config.slot0_env_sensitivity),
            lo2_res_config: PdResConfig::from_reg(reg_config.slot1_env_sensitivity),
            env_res_config: PdResConfig::from_reg(reg_config.slot2_env_sensitivity),
            adjust_event,
            sample_offset: 0,
            sample_delay: 840,
        };

        let mut hrm = Hrm {
            enabled: &mut self.enabled,
            irq: &mut self.irq,
            i2c: i2c.configure(&mut self.sda, &mut self.scl, i2c_conf),
            state,
        };

        let model_number = hrm.model_number().await;
        if model_number != 33 {
            panic!("Only model number VC31B supported for now");
        }

        hrm
    }
}

#[derive(Copy, Clone)]
struct LedConfig {
    current: u8,
    ppg_gain: bool,
}

impl LedConfig {
    fn from_reg(reg: u8) -> Self {
        Self {
            current: reg & 0x7f,
            ppg_gain: (reg & 0x80) != 0,
        }
    }

    fn to_reg(&self) -> u8 {
        self.current & 0x7f | if self.ppg_gain { 0x80 } else { 0 }
    }
}

struct HrmState {
    wearing: bool,
    slots: u8,
    hrm_led_config: LedConfig,
    hrm_res_config: PdResConfig,
    lo2_res_config: PdResConfig,
    env_res_config: PdResConfig,
    hrm_led_max_current: u8,
    adjust_mode: AdjustMode,
    fifo_read_index: u8,
    adjust_event: Option<AdjustEvent>,
    sample_offset: i16,
    sample_delay: u16,
}

impl HrmState {
    fn update_slots(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(u8) -> u8) {
        self.slots = f(self.slots);
        i2c.blocking_write(hw::ADDR, &[REG_CONFIG_START_ADDR, self.slots])
            .unwrap();
    }

    fn update_hrm_led(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(&mut LedConfig)) {
        f(&mut self.hrm_led_config);
        i2c.blocking_write(hw::ADDR, &[LED_SLOT0_REG, self.hrm_led_config.to_reg()])
            .unwrap();
    }

    pub async fn update_hrm_res(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(&mut PdResConfig)) {
        f(&mut self.hrm_res_config);
        i2c.write(hw::ADDR, &[PD_RES_SLOT0_REG, self.hrm_res_config.to_reg()])
            .await
            .unwrap();
    }

    pub async fn update_lo2_res(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(&mut PdResConfig)) {
        f(&mut self.lo2_res_config);
        i2c.write(hw::ADDR, &[PD_RES_SLOT1_REG, self.lo2_res_config.to_reg()])
            .await
            .unwrap();
    }

    pub async fn update_env_res(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(&mut PdResConfig)) {
        f(&mut self.env_res_config);
        i2c.write(hw::ADDR, &[PD_RES_SLOT2_REG, self.env_res_config.to_reg()])
            .await
            .unwrap();
    }

    pub async fn update_sample_delay(&mut self, i2c: &mut Twim<'_>, f: impl FnOnce(&mut u16)) {
        f(&mut self.sample_delay);
        let sample_delay_high = (self.sample_delay >> 8) as u8;
        let sample_delay_low = (self.sample_delay & 0x00ff) as u8;
        i2c.write(
            hw::ADDR,
            &[SAMPLE_DELAY_REG, sample_delay_high, sample_delay_low],
        )
        .await
        .unwrap();
    }
}

pub struct Hrm<'a> {
    enabled: &'a mut Output<'static, hw::EN>,
    #[allow(unused)]
    irq: &'a mut Input<'static, hw::IRQ>,
    i2c: TwiHandle<'a, &'a mut hw::SDA, &'a mut hw::SCL>,
    state: HrmState,
}

const STATUS_START_REG: u8 = 0x01;
const FIFO_WRITE_ADDR_REG: u8 = 0x03;
const SLOT0_LED_CURRENT_REG: u8 = 0x17;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct RegConfig {
    slots: u8, // bit 0, 1, 2 -> slot 0, 1, 2
    irqs: u8,
    _idk: u8,
    fifo_int_len: u8,
    counter_prescaler: [u8; 2],
    slot2_env_sample_rate: u8,
    slot0_led_current: u8,     // slot0: actual hrm/ppg
    slot1_led_current: u8,     // slot1: vo2 sensor
    slot2_led_current: u8,     // slot2: i think env? but what do we need an led for in that case??
    slot0_env_sensitivity: u8, // these appear to be related to pd_res_value
    slot1_env_sensitivity: u8, // ...
    slot2_env_sensitivity: u8,
    _idk2: [u8; 4],
}
const INT_PS: u8 = 0x10;
const INT_LED_OVERLOAD: u8 = 0x08;
const INT_FIFO: u8 = 0x04;
const INT_ENV: u8 = 0x02;
//const INT_PPG: u8 = 0x01;
//
const OVERLOAD_MASK: u8 = 0b111;

const REG_CONFIG_START_ADDR: u8 = 0x10;
const SAMPLE_DELAY_REG: u8 = 0x14;
const LED_SLOT0_REG: u8 = 0x17;
const PD_RES_SLOT0_REG: u8 = 0x1A;
const PD_RES_SLOT1_REG: u8 = 0x1B;
const PD_RES_SLOT2_REG: u8 = 0x1C;
impl Default for RegConfig {
    fn default() -> Self {
        Self {
            slots: 0x01,
            irqs: INT_LED_OVERLOAD | INT_FIFO | INT_ENV | INT_PS,
            _idk: 0x8A,
            fifo_int_len: 0x40 | (FIFO_INT_LEN - 1) as u8,
            counter_prescaler: [0x03, 0x1F],
            slot2_env_sample_rate: 0x00,
            slot0_led_current: 0x00,
            slot1_led_current: 0x00,
            slot2_led_current: 0x00,
            slot0_env_sensitivity: 0x37,
            slot1_env_sensitivity: 0x37,
            slot2_env_sensitivity: 0x77,
            _idk2: [0x16, 0x56, 0x16, 0x00],
        }
    }
}

enum AdjustMode {
    Increasing,
    Decreasing,
    Stable,
}

struct AdjustEvent {
    last_value: i16,
}

const FIFO_INT_LEN: usize = 0x08;
const MAX_SAMPLES_PER_READ: usize = 2 * FIFO_INT_LEN;
const FIFO_RANGE_BEGIN: usize = 0x80;
const FIFO_RANGE_END: usize = 0x100;

fn read_fifo(i2c: &mut Twim<'_>, start: usize, end: usize, out: &mut [i16]) -> usize {
    let addr_diff = (end - start) as usize;
    assert_eq!(addr_diff % 2, 0);
    let samples_available = addr_diff / 2;
    let len = out.len().min(samples_available);
    //defmt::println!(
    //    "start {}, end {}, out.len {}, len {}",
    //    start,
    //    end,
    //    out.len(),
    //    len
    //);
    if len != 0 {
        let byte_buf: &mut [u8] = bytemuck::cast_slice_mut(&mut out[..len]);
        i2c.blocking_write_read(hw::ADDR, &[start.try_into().unwrap()], byte_buf)
            .unwrap();
        for val in out {
            *val = i16::from_be(*val);
        }
    }
    len
}

type Sample = i16;

impl<'a> Hrm<'a> {
    pub async fn model_number(&mut self) -> u8 {
        let reg = 0;
        let mut res = 0;
        let mut i2c = self.i2c.bind().await;
        i2c.write_read(hw::ADDR, &[reg], core::slice::from_mut(&mut res))
            .await
            .unwrap();
        res
    }

    pub async fn wait_event(
        &mut self,
    ) -> (ReadResult, Option<ArrayVec<i16, MAX_SAMPLES_PER_READ>>) {
        self.irq.wait_for_high().await;

        let mut buf1 = [0u8; 6];
        let mut buf2 = [0u8; 6];

        let mut i2c = self.i2c.bind().await;

        i2c.blocking_write_read(hw::ADDR, &[STATUS_START_REG], &mut buf1)
            .unwrap();

        i2c.blocking_write_read(hw::ADDR, &[SLOT0_LED_CURRENT_REG], &mut buf2)
            .unwrap();

        let read_result = ReadResult {
            status: buf1[0],
            irq_status: buf1[1],
            pre_value: [buf1[3] & 0x0F, buf1[4] & 0x0F],
            env_value: [buf1[3] >> 4, buf1[4] >> 4, buf1[5] >> 4],
            ps_value: buf1[5] & 0x0F,

            pd_res_value: [
                (buf2[3] >> 4) & 0x07,
                (buf2[4] >> 4) & 0x07,
                (buf2[5] >> 4) & 0x07,
            ],
            current_value: [buf2[0] & 0x7F, buf2[1] & 0x7F, buf2[2] & 0x7F],
        };

        // Update wear status, TODO: The interrupt does not seem to ever fire?
        if (read_result.irq_status & INT_PS) != 0 {
            // TODO: make more sophisticated
            let now_wearing = read_result.ps_value > PS_WEARING_THRESHOLD;
            match (self.state.wearing, now_wearing) {
                (true, false) => {
                    self.state
                        .update_slots(&mut i2c, |s| (s & 0xF8) | 0x04 /* only env sens */);
                }
                (false, true) => {
                    self.state
                        .update_slots(&mut i2c, |s| (s & 0xF8) | 0x05 /* env sens + hrm */);
                }
                _ => {}
            }

            self.state.wearing = now_wearing;
        }

        if (read_result.irq_status & INT_LED_OVERLOAD) != 0 {
            let overload = read_result.status & OVERLOAD_MASK;
            if (overload & 0b001) != 0 {
                defmt::println!("Overload slot 0!");
                self.state.hrm_led_max_current -= 1;
                let max_current = self.state.hrm_led_max_current;
                self.state
                    .update_hrm_led(&mut i2c, |l| l.current = l.current.min(max_current));
            }
            if (overload & 0b010) != 0 {
                defmt::println!("Overload slot 1!");
            }
            if (overload & 0b100) != 0 {
                defmt::println!("Overload slot 2!");
            }
        }

        // Read sample
        let sample = if (read_result.irq_status & INT_FIFO) != 0 {
            let mut fifo_write_index = 0u8;
            i2c.blocking_write_read(
                hw::ADDR,
                &[FIFO_WRITE_ADDR_REG],
                core::slice::from_mut(&mut fifo_write_index),
            )
            .unwrap();

            let mut fifo_read_index = self.state.fifo_read_index as usize;
            let fifo_write_index = fifo_write_index as usize;

            let mut samples = [0i16; MAX_SAMPLES_PER_READ];
            let mut samples_collected;

            if fifo_read_index <= fifo_write_index {
                // Contiguous region in fifo buffer
                let num_read = read_fifo(&mut i2c, fifo_read_index, fifo_write_index, &mut samples);
                fifo_read_index += num_read * core::mem::size_of::<Sample>();
                samples_collected = num_read;
            } else {
                // Wrapping over end
                let num_read = read_fifo(&mut i2c, fifo_read_index, FIFO_RANGE_END, &mut samples);
                fifo_read_index += num_read * core::mem::size_of::<Sample>();
                samples_collected = num_read;

                if fifo_read_index >= FIFO_RANGE_END {
                    fifo_read_index = FIFO_RANGE_BEGIN;
                    let num_read = read_fifo(
                        &mut i2c,
                        fifo_read_index,
                        fifo_write_index,
                        &mut samples[num_read..],
                    );
                    fifo_read_index += num_read * core::mem::size_of::<Sample>();
                    samples_collected += num_read;
                }
            }

            self.state.fifo_read_index = fifo_read_index.try_into().unwrap();
            let mut samples = ArrayVec::from(samples);
            samples.truncate(samples_collected);

            // All of the following is only relevant if we have at least one sample
            // TODO: Why would that ever happen though? Interrupt should only fire when there are
            // samples. Maybe we don't have enough compute time/waited too long after the interrupt
            // and the write-ptr caught up/overtook the read-ptr??
            if let Some(first_this_batch) = samples.first() {
                if let Some(AdjustEvent { last_value }) = self.state.adjust_event {
                    // Adjust offset so that
                    // <first sample of this batch> = <last sample of last batch>
                    self.state.sample_offset = last_value - first_this_batch;
                    self.state.adjust_event = None;
                }

                //let sample = u16::from_be_bytes(data);

                // Figure out adjust mode based on raw values
                let max_sample = *samples.iter().max().unwrap();
                let min_sample = *samples.iter().min().unwrap();
                let threshold = 10 * 32;
                let max_val = 4095;
                self.state.adjust_mode = match self.state.adjust_mode {
                    AdjustMode::Increasing => {
                        if max_sample < max_val / 2 {
                            // Reached center
                            AdjustMode::Stable
                        } else {
                            AdjustMode::Increasing
                        }
                    }
                    AdjustMode::Decreasing => {
                        if min_sample > max_val / 2 {
                            // Reached center
                            AdjustMode::Stable
                        } else {
                            AdjustMode::Decreasing
                        }
                    }
                    AdjustMode::Stable => {
                        if max_sample > max_val - threshold {
                            // Oversaturation
                            AdjustMode::Increasing
                        } else if min_sample < threshold {
                            // Undersaturation (? this seems the wrong way around...)
                            AdjustMode::Decreasing
                        } else {
                            AdjustMode::Stable
                        }
                    }
                };

                let max_current = self.state.hrm_led_max_current;

                // Act based on adjust mode
                match self.state.adjust_mode {
                    AdjustMode::Increasing => {
                        self.state.update_hrm_led(&mut i2c, |l| {
                            l.current = max_current.min(l.current + 1);
                            //defmt::println!("adjusting current up: {}", l.current);
                        });
                    }
                    AdjustMode::Decreasing => {
                        self.state.update_hrm_led(&mut i2c, |l| {
                            l.current = l.current.saturating_sub(1);
                            //defmt::println!("adjusting current down: {}", l.current);
                        });
                    }
                    AdjustMode::Stable => {}
                }

                // Apply offset to raw values to get a smoother transition when adjusting led current.
                for v in samples.iter_mut() {
                    *v += self.state.sample_offset;
                }
                // Slowly decrease offset
                self.state.sample_offset -= self.state.sample_offset.signum();

                // Register adjusts for next iteration
                if let AdjustMode::Increasing | AdjustMode::Decreasing = self.state.adjust_mode {
                    self.state.adjust_event = Some(AdjustEvent {
                        last_value: *samples.last().unwrap(),
                    });
                }
            }

            // now we need to adjust the PPG
            //if (vcInfo.wasAdjusted>0) vcInfo.wasAdjusted--;
            //if (vcInfo.allowGreenAdjust) {
            //  for (int slotNum=0;slotNum<3;slotNum++) {
            //    vc31b_slot_adjust(slotNum);
            //  }
            //}
            //defmt::println!("Samples: {:?}", samples.as_slice());
            Some(samples)
        } else {
            None
        };

        //defmt::println!("HRM read result: {}", read_result);
        (read_result, sample)
    }

    pub async fn update_hrm_res(&mut self, f: impl FnOnce(&mut PdResConfig)) {
        let mut i2c = self.i2c.bind().await;
        self.state.update_hrm_res(&mut i2c, f).await;
    }

    pub async fn update_lo2_res(&mut self, f: impl FnOnce(&mut PdResConfig)) {
        let mut i2c = self.i2c.bind().await;
        self.state.update_lo2_res(&mut i2c, f).await;
    }

    pub async fn update_env_res(&mut self, f: impl FnOnce(&mut PdResConfig)) {
        let mut i2c = self.i2c.bind().await;
        self.state.update_env_res(&mut i2c, f).await;
    }

    pub async fn update_sample_delay(&mut self, f: impl FnOnce(&mut u16)) {
        let mut i2c = self.i2c.bind().await;
        self.state.update_sample_delay(&mut i2c, f).await;
    }

    pub async fn enable(&mut self) {
        let mut cfg = RegConfig::default();

        let hrm_poll_interval = 40u16; // Hz
        let vc_hr02_sample_rate = (1000 / hrm_poll_interval) as u8;

        cfg.slot2_env_sample_rate = vc_hr02_sample_rate - 6; // VC31B_REG16 how often should ENV fire

        cfg.slot2_led_current = 0xE0; //CUR = 80mA//write Hs equal to 1 (SLOT2?)
        cfg.slot2_env_sensitivity = 0x67;
        cfg.slots = 0x45; // VC31B_REG11 heart rate calculation - SLOT2(env) and SLOT0(hr)

        // Set up HRM speed - from testing, 200=100hz/10ms, 400=50hz/20ms, 800=25hz/40ms
        let divisor: u16 = 840; //20 * (hrm_poll_interval as u16);
        cfg.counter_prescaler[0] = (divisor >> 8) as u8;
        cfg.counter_prescaler[1] = (divisor & 255) as u8;

        let mut buf = [0u8; 18];
        buf[0] = REG_CONFIG_START_ADDR;
        buf[1..].copy_from_slice(bytemuck::bytes_of(&cfg));

        let mut i2c = self.i2c.bind().await;
        i2c.write(hw::ADDR, &buf).await.unwrap();
        self.state.slots = cfg.slots;
        self.state.hrm_led_config = LedConfig::from_reg(cfg.slot0_led_current);

        self.state.update_slots(&mut i2c, |s| s | 0x80);
    }

    pub async fn disable(&mut self) {
        let mut i2c = self.i2c.bind().await;
        i2c.write(hw::ADDR, &[REG_CONFIG_START_ADDR, 0])
            .await
            .unwrap();
    }
}

impl Drop for Hrm<'_> {
    fn drop(&mut self) {
        self.enabled.set_low();
    }
}
