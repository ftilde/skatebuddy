use super::hardware::hrm as hw;

use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    twim,
};

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
        Timer::after(Duration::from_millis(1)).await; //TODO
                                                      //
        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        Hrm {
            enabled: &mut self.enabled,
            irq: &mut self.irq,
            i2c: twim::Twim::new(i2c, crate::Irqs, &mut self.sda, &mut self.scl, i2c_conf),
        }
    }
}

pub struct Hrm<'a> {
    enabled: &'a mut Output<'static, hw::EN>,
    #[allow(unused)]
    irq: &'a mut Input<'static, hw::IRQ>,
    i2c: twim::Twim<'a, I2CInstance>,
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
}

impl Drop for Hrm<'_> {
    fn drop(&mut self) {
        self.enabled.set_low();
    }
}
