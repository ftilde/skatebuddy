use embassy_nrf::{
    gpio::Pin,
    peripherals::{TWISPI0, TWISPI1},
    twim::{self, Config},
    Peripheral,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

pub type TWI0 = TWISPI0;
pub type TWI1 = TWISPI1;

pub struct TWI {
    pub(crate) twi0: Mutex<CriticalSectionRawMutex, TWI0>,
    pub(crate) twi1: Mutex<CriticalSectionRawMutex, TWI1>,
}

impl TWI {
    pub fn configure<
        'd,
        PinSDA: Pin,
        PinSCL: Pin,
        PerSDA: Peripheral<P = PinSDA> + 'd,
        PerSCL: Peripheral<P = PinSCL> + 'd,
    >(
        &'d self,
        sda: PerSDA,
        scl: PerSCL,
        config: Config,
    ) -> TwiHandle<'d, PerSDA, PerSCL> {
        TwiHandle {
            twi: self,
            config,
            sda,
            scl,
        }
    }
}

pub struct TwiHandle<'a, PerSDA, PerSCL> {
    twi: &'a TWI,
    config: Config,
    sda: PerSDA,
    scl: PerSCL,
}

// Why does this not impl Clone? ...
fn clone_config(c: &Config) -> Config {
    let mut out = Config::default();
    out.frequency = c.frequency;
    out.sda_high_drive = c.sda_high_drive;
    out.sda_pullup = c.sda_pullup;
    out.scl_high_drive = c.scl_high_drive;
    out.scl_pullup = c.scl_pullup;
    out
}

impl<
        'a,
        PinSDA: Pin,
        PinSCL: Pin,
        PerSDA: Peripheral<P = PinSDA> + 'a,
        PerSCL: Peripheral<P = PinSCL> + 'a,
    > TwiHandle<'a, PerSDA, PerSCL>
{
    pub async fn bind<'d>(&'d mut self) -> Twim<'d> {
        match embassy_futures::select::select(self.twi.twi0.lock(), self.twi.twi1.lock()).await {
            embassy_futures::select::Either::First(twi0) => {
                Twim::TWI0(embassy_nrf::twim::Twim::new(
                    twi0,
                    crate::Irqs,
                    &mut self.sda,
                    &mut self.scl,
                    clone_config(&self.config),
                ))
            }
            embassy_futures::select::Either::Second(twi1) => {
                Twim::TWI1(embassy_nrf::twim::Twim::new(
                    twi1,
                    crate::Irqs,
                    &mut self.sda,
                    &mut self.scl,
                    clone_config(&self.config),
                ))
            }
        }
    }

    pub fn bind_blocking<'d>(&'d mut self) -> Twim<'d> {
        // We should only ever get here if we have multiple executors running at different
        // priorities. TODO: what happens if we are waiting on lower priority? ...
        loop {
            if let Ok(twi0) = self.twi.twi0.try_lock() {
                return Twim::TWI0(embassy_nrf::twim::Twim::new(
                    twi0,
                    crate::Irqs,
                    &mut self.sda,
                    &mut self.scl,
                    clone_config(&self.config),
                ));
            }

            if let Ok(twi0) = self.twi.twi1.try_lock() {
                return Twim::TWI1(embassy_nrf::twim::Twim::new(
                    twi0,
                    crate::Irqs,
                    &mut self.sda,
                    &mut self.scl,
                    clone_config(&self.config),
                ));
            }
        }
    }
}

pub enum Twim<'d> {
    TWI0(embassy_nrf::twim::Twim<'d, TWI0>),
    TWI1(embassy_nrf::twim::Twim<'d, TWI1>),
}

//macro_rules! delegate_func {
//    ($func:ident) => {
//        fn $func
//    };
//}

impl<'d> Twim<'d> {
    pub async fn write_read(
        &mut self,
        address: u8,
        wr_buffer: &[u8],
        rd_buffer: &mut [u8],
    ) -> Result<(), twim::Error> {
        match self {
            Twim::TWI0(twim) => twim.write_read(address, wr_buffer, rd_buffer).await,
            Twim::TWI1(twim) => twim.write_read(address, wr_buffer, rd_buffer).await,
        }
    }

    pub async fn write(&mut self, address: u8, wr_buffer: &[u8]) -> Result<(), twim::Error> {
        match self {
            Twim::TWI0(twim) => twim.write(address, wr_buffer).await,
            Twim::TWI1(twim) => twim.write(address, wr_buffer).await,
        }
    }

    pub fn blocking_write(&mut self, address: u8, wr_buffer: &[u8]) -> Result<(), twim::Error> {
        match self {
            Twim::TWI0(twim) => twim.blocking_write(address, wr_buffer),
            Twim::TWI1(twim) => twim.blocking_write(address, wr_buffer),
        }
    }

    pub fn blocking_write_read(
        &mut self,
        address: u8,
        wr_buffer: &[u8],
        rd_buffer: &mut [u8],
    ) -> Result<(), twim::Error> {
        match self {
            Twim::TWI0(twim) => twim.blocking_write_read(address, wr_buffer, rd_buffer),
            Twim::TWI1(twim) => twim.blocking_write_read(address, wr_buffer, rd_buffer),
        }
    }
}
