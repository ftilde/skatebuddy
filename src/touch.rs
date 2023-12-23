use crate::hardware::touch as hw;
use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    twim,
};
use embassy_time::{Duration, Timer};

pub struct TouchRessources {
    scl: hw::SCL,
    sda: hw::SDA,
    reset: Output<'static, hw::RST>,
    irq: Input<'static, hw::IRQ>,
}

type I2CInstance = embassy_nrf::peripherals::TWISPI0;

// From espruino. not documented anywhere else, afaik, though...
const REG_ADDR_SLEEP: u8 = 0xE5;

// This is sleep mode according to official example code (but looks like standby???
const REG_ADDR_STANDBY: u8 = 0xA5;

impl TouchRessources {
    pub async fn new(
        sda: hw::SDA,
        scl: hw::SCL,
        reset: hw::RST,
        irq: hw::IRQ,
        i2c: &mut I2CInstance,
    ) -> Self {
        let mut ret = Self {
            sda,
            scl,
            reset: Output::new(reset, Level::Low, OutputDrive::Standard),
            irq: Input::new(irq, Pull::None),
        };

        {
            // Turn on once to activate sleep mode
            let _ = ret.enabled(i2c);
        }

        ret
    }

    pub async fn enabled<'a>(&'a mut self, i2c: &'a mut I2CInstance) -> Touch<'a> {
        self.reset.set_low();
        Timer::after(Duration::from_millis(20)).await;
        self.reset.set_high();
        Timer::after(Duration::from_millis(200)).await;

        Touch {
            hw: self,
            instance: i2c,
        }
    }
}

pub struct Touch<'a> {
    hw: &'a mut TouchRessources,
    instance: &'a mut I2CInstance,
}

fn build_i2c<'a>(
    sda: &'a mut hw::SDA,
    scl: &'a mut hw::SCL,
    instance: &'a mut I2CInstance,
) -> twim::Twim<'a, I2CInstance> {
    let i2c_conf = twim::Config::default();
    // TODO: check if this is allowed
    //i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

    twim::Twim::new(instance, crate::Irqs, sda, scl, i2c_conf)
}

impl<'a> Drop for Touch<'a> {
    fn drop(&mut self) {
        let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);
        let reg_val = 0x03;
        let buf = [REG_ADDR_SLEEP, reg_val];
        i2c.blocking_write(hw::ADDR, &buf).unwrap();
    }
}

impl<'a> Touch<'a> {
    pub async fn wait_for_event(&mut self) {
        self.hw.irq.wait_for_low().await;

        let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);
        let reg_val = 0x03;
        let buf = [REG_ADDR_STANDBY, reg_val];
        i2c.write(hw::ADDR, &buf).await.unwrap();
    }
}
