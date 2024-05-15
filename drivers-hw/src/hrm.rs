use super::hardware::hrm as hw;

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

type I2CInstance = embassy_nrf::peripherals::TWISPI1;

impl HrmRessources {
    pub(crate) fn new(sda: hw::SDA, scl: hw::SCL, enabled: hw::EN, irq: hw::IRQ) -> Self {
        Self {
            sda,
            scl,
            enabled: Output::new(enabled, Level::Low, OutputDrive::Standard),
            irq: Input::new(irq, Pull::None),
        }
    }

    pub async fn on<'a>(&'a mut self, i2c: &'a mut I2CInstance) -> Hrm<'a> {
        self.enabled.set_high();
        // Wait for boot
        Timer::after(Duration::from_millis(1)).await;

        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        let state = HrmState {
            wearing: false,
            slots: 0,
            hrm_led_config: LedConfig::from_reg(0),
            hrm_led_max_current: 0x6f,
            adjust_mode: AdjustMode::Stable,
        };

        let mut hrm = Hrm {
            enabled: &mut self.enabled,
            irq: &mut self.irq,
            i2c: twim::Twim::new(i2c, crate::Irqs, &mut self.sda, &mut self.scl, i2c_conf),
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
    hrm_led_max_current: u8,
    adjust_mode: AdjustMode,
}

pub struct Hrm<'a> {
    enabled: &'a mut Output<'static, hw::EN>,
    #[allow(unused)]
    irq: &'a mut Input<'static, hw::IRQ>,
    i2c: twim::Twim<'a, I2CInstance>,
    state: HrmState,
}

const STATUS_START_REG: u8 = 0x01;
//const FIFO_WRITE_ADDR_REG: u8 = 0x03;
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
    mode_or_something: u8,
    _idk2: [u8; 4],
}
const INT_PS: u8 = 0x10;
const INT_LED_OVERLOAD: u8 = 0x08;
const INT_FIFO: u8 = 0x04;
const INT_ENV: u8 = 0x02;
//const INT_PPG: u8 = 0x01;

const REG_CONFIG_START_ADDR: u8 = 0x10;
const LED_SLOT0_REG: u8 = 0x17;
impl Default for RegConfig {
    fn default() -> Self {
        Self {
            slots: 0x01,
            irqs: INT_LED_OVERLOAD | INT_FIFO | INT_ENV | INT_PS,
            _idk: 0x8A,
            fifo_int_len: 0x40,
            counter_prescaler: [0x03, 0x1F],
            slot2_env_sample_rate: 0x00,
            slot0_led_current: 0x00,
            slot1_led_current: 0x80,
            slot2_led_current: 0x00,
            slot0_env_sensitivity: 0x57,
            slot1_env_sensitivity: 0x37,
            mode_or_something: 0x07,
            _idk2: [0x16, 0x56, 0x16, 0x00],
        }
    }
}

enum AdjustMode {
    Increasing,
    Decreasing,
    Stable,
}

impl<'a> Hrm<'a> {
    pub async fn model_number(&mut self) -> u8 {
        let reg = 0;
        let mut res = 0;
        self.i2c
            .write_read(hw::ADDR, &[reg], core::slice::from_mut(&mut res))
            .await
            .unwrap();
        res
    }

    pub async fn wait_event(&mut self) -> (ReadResult, Option<u16>) {
        self.irq.wait_for_high().await;

        let mut buf1 = [0u8; 6];
        let mut buf2 = [0u8; 6];

        self.i2c
            .write_read(hw::ADDR, &[STATUS_START_REG], &mut buf1)
            .await
            .unwrap();

        self.i2c
            .write_read(hw::ADDR, &[SLOT0_LED_CURRENT_REG], &mut buf2)
            .await
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
                    self.update_slots(|s| (s & 0xF8) | 0x04 /* only env sens */)
                        .await;
                }
                (false, true) => {
                    self.update_slots(|s| (s & 0xF8) | 0x05 /* env sens + hrm */)
                        .await;
                }
                _ => {}
            }

            self.state.wearing = now_wearing;
        }

        if (read_result.irq_status & INT_LED_OVERLOAD) != 0 {
            self.state.hrm_led_max_current -= 1;
            let max_current = self.state.hrm_led_max_current;
            self.update_hrm_led(|l| l.current = max_current).await;
        }

        // Read sample
        let sample = if (read_result.irq_status & INT_FIFO) != 0 {
            //let mut fifo_write_index;
            //self.i2c.write_read(hw::ADDR, &[FIFO_WRITE_ADDR_REG], core::slice::from_mut(&mut fifo_write_index)).await;

            let fifo_read_index = 0x80;
            let mut data = [0; 2];
            self.i2c
                .write_read(hw::ADDR, &[fifo_read_index], &mut data)
                .await
                .unwrap();

            let sample = u16::from_be_bytes(data);

            let threshold = 10 * 32;
            let max_current = self.state.hrm_led_max_current;
            let max_val = 4095;
            self.state.adjust_mode = match self.state.adjust_mode {
                AdjustMode::Increasing => {
                    if sample < max_val / 2 {
                        // Reached center
                        AdjustMode::Stable
                    } else {
                        AdjustMode::Increasing
                    }
                }
                AdjustMode::Decreasing => {
                    if sample > max_val / 2 {
                        // Reached center
                        AdjustMode::Stable
                    } else {
                        AdjustMode::Decreasing
                    }
                }
                AdjustMode::Stable => {
                    if sample > max_val - threshold {
                        // Oversaturation
                        AdjustMode::Increasing
                    } else if sample < threshold {
                        // Undersaturation (? this seems the wrong way around...)
                        AdjustMode::Decreasing
                    } else {
                        AdjustMode::Stable
                    }
                }
            };
            match self.state.adjust_mode {
                AdjustMode::Increasing => {
                    self.update_hrm_led(|l| {
                        l.current = max_current.min(l.current + 1);
                        defmt::println!("adjusting current up: {}", l.current);
                    })
                    .await;
                }
                AdjustMode::Decreasing => {
                    self.update_hrm_led(|l| {
                        l.current = l.current.saturating_sub(1);
                        defmt::println!("adjusting current down: {}", l.current);
                    })
                    .await;
                }
                AdjustMode::Stable => {}
            }

            // now we need to adjust the PPG
            //if (vcInfo.wasAdjusted>0) vcInfo.wasAdjusted--;
            //if (vcInfo.allowGreenAdjust) {
            //  for (int slotNum=0;slotNum<3;slotNum++) {
            //    vc31b_slot_adjust(slotNum);
            //  }
            //}
            Some(sample)
        } else {
            None
        };

        defmt::println!("HRM read result: {}", read_result);
        (read_result, sample)
    }

    async fn update_slots(&mut self, f: impl FnOnce(u8) -> u8) {
        self.state.slots = f(self.state.slots);
        self.i2c
            .write(hw::ADDR, &[REG_CONFIG_START_ADDR, self.state.slots])
            .await
            .unwrap();
    }

    async fn update_hrm_led(&mut self, f: impl FnOnce(&mut LedConfig)) {
        f(&mut self.state.hrm_led_config);
        self.i2c
            .write(
                hw::ADDR,
                &[LED_SLOT0_REG, self.state.hrm_led_config.to_reg()],
            )
            .await
            .unwrap();
    }

    pub async fn enable(&mut self) {
        let mut cfg = RegConfig::default();

        let hrm_poll_interval = 40u16; // Hz
        let vc_hr02_sample_rate = (1000 / hrm_poll_interval) as u8;

        cfg.slot2_env_sample_rate = vc_hr02_sample_rate - 6; // VC31B_REG16 how often should ENV fire

        cfg.slot2_led_current = 0xE0; //CUR = 80mA//write Hs equal to 1 (SLOT2?)
        cfg.mode_or_something = 0x67;
        cfg.slots = 0x45; // VC31B_REG11 heart rate calculation - SLOT2(env) and SLOT0(hr)

        // Set up HRM speed - from testing, 200=100hz/10ms, 400=50hz/20ms, 800=25hz/40ms
        let divisor: u16 = 20 * (hrm_poll_interval as u16);
        cfg.counter_prescaler[0] = (divisor >> 8) as u8;
        cfg.counter_prescaler[1] = (divisor & 255) as u8;

        let mut buf = [0u8; 18];
        buf[0] = REG_CONFIG_START_ADDR;
        buf[1..].copy_from_slice(bytemuck::bytes_of(&cfg));
        self.i2c.write(hw::ADDR, &buf).await.unwrap();
        self.state.slots = cfg.slots;
        self.state.hrm_led_config = LedConfig::from_reg(cfg.slot0_led_current);

        self.update_slots(|s| s | 0x80).await;
    }

    pub async fn disable(&mut self) {
        self.i2c
            .write(hw::ADDR, &[REG_CONFIG_START_ADDR, 0])
            .await
            .unwrap();
    }
}

impl Drop for Hrm<'_> {
    fn drop(&mut self) {
        self.enabled.set_low();
    }
}
