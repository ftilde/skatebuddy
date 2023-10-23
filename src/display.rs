use crate::hardware::lcd as hw;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::SPI3,
    spim,
};
use embassy_time::{Duration, Timer};
use nrf52840_hal::prelude::OutputPin;

use crate::lpm013m1126c;

pub struct SpiDeviceWrapper<'a, T: embassy_nrf::spim::Instance, CS> {
    spi: embassy_nrf::spim::Spim<'a, T>,
    cs: CS,
}

impl<'a, T: embassy_nrf::spim::Instance, CS: OutputPin> embedded_hal_async::spi::ErrorType
    for SpiDeviceWrapper<'a, T, CS>
{
    type Error = embedded_hal_async::spi::ErrorKind;
}
impl<'a, T: embassy_nrf::spim::Instance, CS: OutputPin> embedded_hal_async::spi::SpiDevice
    for SpiDeviceWrapper<'a, T, CS>
{
    async fn transaction(
        &mut self,
        operations: &mut [embedded_hal_async::spi::Operation<'_, u8>],
    ) -> Result<(), embedded_hal_async::spi::ErrorKind> {
        let _ = self.cs.set_high();
        for operation in operations {
            match operation {
                embedded_hal_async::spi::Operation::Read(_) => todo!(),
                embedded_hal_async::spi::Operation::Write(buf) => {
                    self.spi.write_from_ram(buf).await
                }
                embedded_hal_async::spi::Operation::Transfer(_, _) => todo!(),
                embedded_hal_async::spi::Operation::TransferInPlace(_) => todo!(),
                embedded_hal_async::spi::Operation::DelayUs(_) => todo!(),
            }
            .map_err(|_e| embedded_hal_async::spi::ErrorKind::Other)?;
        }
        let _ = self.cs.set_low();
        Ok(())
    }
}

#[embassy_executor::task]
async fn drive_ext_com_in(pin: embassy_nrf::peripherals::P0_06) {
    let mut pin = Output::new(pin, Level::Low, OutputDrive::Standard);

    let freq_hz = 2;

    let period_us = 1_000_000 / freq_hz;
    let half_period_us = period_us / 2;
    let wait_period = Duration::from_micros(half_period_us);
    loop {
        Timer::after(wait_period).await;
        pin.set_high();
        Timer::after(wait_period).await;
        pin.set_low();
    }
}

pub type Display<'a> =
    lpm013m1126c::Display<SpiDeviceWrapper<'a, SPI3, Output<'a, hw::CS>>, Output<'a, hw::DISP>>;

pub fn setup(
    spawner: &embassy_executor::Spawner,
    spi: SPI3,
    cs: hw::CS,
    extcomin: hw::EXTCOMIN,
    disp: hw::DISP,
    sck: hw::SCK,
    mosi: hw::MOSI,
) -> Display<'static> {
    let mut config = spim::Config::default();
    config.frequency = spim::Frequency::M4;
    config.mode = lpm013m1126c::SPI_MODE;

    let cs = Output::new(cs, Level::Low, OutputDrive::Standard);
    let disp = Output::new(disp, Level::Low, OutputDrive::Standard);
    let spim = spim::Spim::new_txonly(spi, crate::Irqs, sck, mosi, config);

    let spi = SpiDeviceWrapper { spi: spim, cs };

    let mut delay = embassy_time::Delay;
    let lcd = lpm013m1126c::Controller::new(spi, disp, &mut delay);

    spawner.spawn(drive_ext_com_in(extcomin)).unwrap();

    lpm013m1126c::Display::new(lcd)
}
