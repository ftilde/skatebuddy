use crate::twi::{TwiHandle, TWI};

use super::hardware::mag as hw;
use embassy_nrf::twim;
use embassy_time::{Duration, Timer};

pub struct MagRessources {
    scl: hw::SCL,
    sda: hw::SDA,
}

impl MagRessources {
    pub(crate) fn new(sda: hw::SDA, scl: hw::SCL) -> Self {
        Self { sda, scl }
    }

    pub async fn on<'a>(&'a mut self, i2c: &'a TWI) -> Mag<'a> {
        Mag::new(self, i2c).await
    }
}

pub struct Mag<'a> {
    i2c: TwiHandle<'a, &'a mut hw::SDA, &'a mut hw::SCL>,
}

impl<'a> Mag<'a> {
    async fn new(hw: &'a mut MagRessources, instance: &'a TWI) -> Mag<'a> {
        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        Mag {
            i2c: instance.configure(&mut hw.sda, &mut hw.scl, i2c_conf),
        }
    }

    pub async fn read(&mut self) -> [u8; 7] {
        // Start measurement
        let cmd = [0x3e];
        let mut res = [0];
        let mut i2c = self.i2c.bind().await;
        i2c.write_read(hw::ADDR, &cmd, &mut res).await.unwrap();
        //defmt::println!("Status after start: {:b}", res[0]);
        Timer::after(Duration::from_millis(100)).await;

        let cmd = [0x4e];
        let mut res = [0; 7];
        i2c.write_read(hw::ADDR, &cmd, &mut res).await.unwrap();

        res
    }
}
