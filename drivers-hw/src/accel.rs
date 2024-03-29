use super::hardware::accel as hw;
use embassy_nrf::twim;

type I2CInstance = embassy_nrf::peripherals::TWISPI1;

pub use drivers_shared::accel::*;

const ADDR_CNTL1: u8 = 0x18;
//const ADDR_CNTL2: u8 = 0x19;

const ADDR_XHPL: u8 = 0x00;
const ADDR_XOUTL: u8 = 0x06;

pub struct AccelRessources {
    sda: hw::SDA,
    scl: hw::SCL,
}

impl AccelRessources {
    pub(crate) fn new(sda: hw::SDA, scl: hw::SCL) -> Self {
        Self { sda, scl }
    }

    pub async fn on<'a>(&'a mut self, instance: &'a mut I2CInstance, config: Config) -> Accel<'a> {
        Accel::new(self, instance, config).await
    }
}

pub struct Accel<'a> {
    i2c: twim::Twim<'a, I2CInstance>, //TODO: We don't want to hold on to this
    config: Config,
}

impl<'a> Accel<'a> {
    async fn read_registers(&mut self, r_addr: u8, r_buf: &mut [u8]) {
        let wbuf = [r_addr];

        self.i2c.write_read(hw::ADDR, &wbuf, r_buf).await.unwrap();
    }
    async fn write_register(&mut self, r_addr: u8, w: u8) {
        let wbuf = [r_addr, w];

        self.i2c.write(hw::ADDR, &wbuf).await.unwrap();
    }

    async fn new(
        hw: &'a mut AccelRessources,
        instance: &'a mut I2CInstance,
        config: Config,
    ) -> Accel<'a> {
        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        let i2c = twim::Twim::new(instance, crate::Irqs, &mut hw.sda, &mut hw.scl, i2c_conf);

        let v = config.cntl1.as_raw_slice()[0];
        let mut s = Self { i2c, config };
        s.write_register(ADDR_CNTL1, v).await;

        //Wait for startup
        //TODO: could be more specific basd on config... meh...
        embassy_time::Timer::after(embassy_time::Duration::from_millis(81)).await;

        s
    }

    async fn reading_from(&mut self, base_reg: u8) -> Reading {
        let mut r_buf = [0u8; 6];
        self.read_registers(base_reg, &mut r_buf).await;

        Reading {
            x: i16::from_le_bytes(r_buf[0..2].try_into().unwrap()),
            y: i16::from_le_bytes(r_buf[2..4].try_into().unwrap()),
            z: i16::from_le_bytes(r_buf[4..6].try_into().unwrap()),
        }
    }

    pub async fn reading_hf(&mut self) -> Reading {
        self.reading_from(ADDR_XHPL).await
    }

    pub async fn reading_nf(&mut self) -> Reading {
        self.reading_from(ADDR_XOUTL).await
    }
}

impl<'a> Drop for Accel<'a> {
    fn drop(&mut self) {
        let mut cntl1 = self.config.cntl1;

        cntl1.set(7, false); // standby bit

        let wbuf = [ADDR_CNTL1, cntl1.as_raw_slice()[0]];

        self.i2c.blocking_write(hw::ADDR, &wbuf).unwrap();
    }
}
