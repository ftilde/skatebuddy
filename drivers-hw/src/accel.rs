use crate::twi::{TwiHandle, TWI};

use super::hardware::accel as hw;
use embassy_nrf::twim;

pub use drivers_shared::accel::*;

const ADDR_CNTL1: u8 = 0x18;
//const ADDR_CNTL2: u8 = 0x19;
//const ADDR_CNTL3: u8 = 0x1a;
const ADDR_ODCNTL: u8 = 0x1b;

const ADDR_XHPL: u8 = 0x00;
const ADDR_XOUTL: u8 = 0x06;
const ADDR_BUF_STATUS_1: u8 = 0x3c;
const ADDR_BUF_READ: u8 = 0x3f;
const ADDR_BUF_CNTL2: u8 = 0x3b;

pub struct AccelRessources {
    sda: hw::SDA,
    scl: hw::SCL,
}

impl AccelRessources {
    pub(crate) fn new(sda: hw::SDA, scl: hw::SCL) -> Self {
        Self { sda, scl }
    }

    pub async fn on<'a>(&'a mut self, instance: &'a TWI, config: Config) -> Accel<'a> {
        Accel::new(self, instance, config).await
    }
}

pub struct Accel<'a> {
    i2c: TwiHandle<'a, &'a mut hw::SDA, &'a mut hw::SCL>,
    config: Config,
}

impl<'a> Accel<'a> {
    async fn read_registers(&mut self, r_addr: u8, r_buf: &mut [u8]) {
        let wbuf = [r_addr];

        let mut i2c = self.i2c.bind().await;
        i2c.write_read(hw::ADDR, &wbuf, r_buf).await.unwrap();
    }
    async fn write_register(&mut self, r_addr: u8, w: u8) {
        let wbuf = [r_addr, w];

        let mut i2c = self.i2c.bind().await;
        i2c.write(hw::ADDR, &wbuf).await.unwrap();
    }

    async fn new(hw: &'a mut AccelRessources, instance: &'a TWI, config: Config) -> Accel<'a> {
        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        //let i2c = instance.configure(&mut hw.sda, &mut hw.scl, i2c_conf);

        let mut s = Self {
            i2c: instance.configure(&mut hw.sda, &mut hw.scl, i2c_conf),
            config,
        };
        s.write_register(ADDR_CNTL1, config.cntl1.into_bytes()[0])
            .await;
        s.write_register(ADDR_ODCNTL, config.odcntl.into_bytes()[0])
            .await;
        s.write_register(ADDR_BUF_CNTL2, config.buf_cntl2.into_bytes()[0])
            .await;

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

    pub async fn read_buffer<'b>(&mut self, out: &'b mut [Reading]) -> &'b mut [Reading] {
        let mut i2c = self.i2c.bind().await;

        let bytes_per_reading = match self.config.buf_cntl2.resolution() {
            BufRes::Bit8 => 3,
            BufRes::Bit16 => 6,
        };

        let mut total_readings = 0;

        while total_readings < out.len() {
            let mut num_bytes = 0u8;
            i2c.write_read(
                hw::ADDR,
                &[ADDR_BUF_STATUS_1],
                core::slice::from_mut(&mut num_bytes),
            )
            .await
            .unwrap();

            let num_readings = (num_bytes / bytes_per_reading) as usize;
            let num_readings = num_readings.min(out.len() - total_readings);

            if num_readings == 0 {
                break;
            }

            let mut buf = [0u8; 6];
            for _ in 0..num_readings {
                for o in &mut buf[..bytes_per_reading as usize] {
                    i2c.write_read(hw::ADDR, &[ADDR_BUF_READ], core::slice::from_mut(o))
                        .await
                        .unwrap();
                }

                let r = &mut out[total_readings];
                total_readings += 1;

                match self.config.buf_cntl2.resolution() {
                    BufRes::Bit8 => {
                        r.x = buf[0] as i8 as i16;
                        r.y = buf[1] as i8 as i16;
                        r.z = buf[2] as i8 as i16;
                    }
                    BufRes::Bit16 => {
                        r.x = i16::from_le_bytes(buf[0..2].try_into().unwrap());
                        r.y = i16::from_le_bytes(buf[2..4].try_into().unwrap());
                        r.z = i16::from_le_bytes(buf[4..6].try_into().unwrap());
                    }
                }
            }
        }
        &mut out[..total_readings]
    }
}

impl<'a> Drop for Accel<'a> {
    fn drop(&mut self) {
        let mut cntl1 = self.config.cntl1;

        cntl1.set_pc1(0); // standby bit

        let wbuf = [ADDR_CNTL1, cntl1.into_bytes()[0]];

        let mut i2c = self.i2c.bind_blocking();
        i2c.blocking_write(hw::ADDR, &wbuf).unwrap();
    }
}
