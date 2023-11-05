use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;
use nrf52840_hal::prelude::OutputPin;

pub struct SpiDeviceWrapper<'a, T: embassy_nrf::spim::Instance, CS> {
    pub spi: embassy_nrf::spim::Spim<'a, T>,
    pub cs: CS,
    pub on: PinState,
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
        let _ = self.cs.set_state(self.on);
        for operation in operations {
            match operation {
                embedded_hal_async::spi::Operation::Read(buf) => self.spi.read(buf).await,
                embedded_hal_async::spi::Operation::Write(buf) => {
                    self.spi.write_from_ram(buf).await
                }
                embedded_hal_async::spi::Operation::Transfer(bin, bout) => {
                    self.spi.transfer(bin, bout).await
                }
                embedded_hal_async::spi::Operation::TransferInPlace(inout) => {
                    self.spi.transfer_in_place(inout).await
                }
                embedded_hal_async::spi::Operation::DelayUs(us) => {
                    Timer::after(Duration::from_micros(*us as u64)).await;
                    Ok(())
                }
            }
            .map_err(|_e| embedded_hal_async::spi::ErrorKind::Other)?;
        }
        let _ = self.cs.set_state(!self.on);
        Ok(())
    }
}
