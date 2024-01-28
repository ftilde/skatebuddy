use super::hardware::mag as hw;
use embassy_nrf::twim;
use embassy_time::{Duration, Timer};

pub struct MagRessources {
    scl: hw::SCL,
    sda: hw::SDA,
}

type I2CInstance = embassy_nrf::peripherals::TWISPI1;

impl MagRessources {
    pub fn new(sda: hw::SDA, scl: hw::SCL) -> Self {
        Self { sda, scl }
    }

    pub async fn on<'a>(&'a mut self, i2c: &'a mut I2CInstance) -> Mag<'a> {
        Mag::new(self, i2c).await
    }
}

pub struct Mag<'a> {
    i2c: twim::Twim<'a, I2CInstance>,
}

impl<'a> Mag<'a> {
    async fn new(hw: &'a mut MagRessources, instance: &'a mut I2CInstance) -> Mag<'a> {
        let mut i2c_conf = twim::Config::default();
        i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        Mag {
            i2c: twim::Twim::new(instance, crate::Irqs, &mut hw.sda, &mut hw.scl, i2c_conf),
        }
    }

    pub async fn read(&mut self) -> [u8; 7] {
        // Start measurement
        let cmd = [0x3e];
        let mut res = [0];
        self.i2c.write_read(hw::ADDR, &cmd, &mut res).await.unwrap();
        //defmt::println!("Status after start: {:b}", res[0]);
        Timer::after(Duration::from_millis(100)).await;

        let cmd = [0x4e];
        let mut res = [0; 7];
        self.i2c.write_read(hw::ADDR, &cmd, &mut res).await.unwrap();

        res
    }
}
