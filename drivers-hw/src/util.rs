use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::{OutputPin, PinState};

pub struct SpiDeviceWrapper<'a, T: embassy_nrf::spim::Instance, CS> {
    pub spi: embassy_nrf::spim::Spim<'a, T>,
    pub cs: &'a mut CS,
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
                embedded_hal_async::spi::Operation::Write(buf) => self.spi.write(buf).await,
                embedded_hal_async::spi::Operation::Transfer(bin, bout) => {
                    self.spi.transfer(bin, bout).await
                }
                embedded_hal_async::spi::Operation::TransferInPlace(inout) => {
                    self.spi.transfer_in_place(inout).await
                }
                embedded_hal_async::spi::Operation::DelayNs(us) => {
                    Timer::after(Duration::from_nanos(*us as u64)).await;
                    Ok(())
                }
            }
            .map_err(|_e| embedded_hal_async::spi::ErrorKind::Other)?;
        }
        let _ = self.cs.set_state(!self.on);
        Ok(())
    }
}

//fn dump_peripheral_regs() {
//    let foo = unsafe { nrf52840_hal::pac::Peripherals::steal() };
//    defmt::println!("mrs: {:b}", foo.POWER.mainregstatus.read().bits());
//    defmt::println!("dcd: {:b}", foo.POWER.dcdcen.read().bits());
//    defmt::println!("dcd0: {:b}", foo.POWER.dcdcen0.read().bits());
//
//    defmt::println!("spi0: {:b}", foo.SPI0.enable.read().bits());
//    defmt::println!("spi1: {:b}", foo.SPI1.enable.read().bits());
//    defmt::println!("spi2: {:b}", foo.SPI2.enable.read().bits());
//    defmt::println!("spim0: {:b}", foo.SPIM0.enable.read().bits());
//    defmt::println!("spim1: {:b}", foo.SPIM1.enable.read().bits());
//    defmt::println!("spim2: {:b}", foo.SPIM2.enable.read().bits());
//    defmt::println!("spim3: {:b}", foo.SPIM3.enable.read().bits());
//    defmt::println!("spis0: {:b}", foo.SPIS0.enable.read().bits());
//    defmt::println!("spis1: {:b}", foo.SPIS1.enable.read().bits());
//    defmt::println!("spis2: {:b}", foo.SPIS2.enable.read().bits());
//
//    defmt::println!("twi0: {:b}", foo.TWI0.enable.read().bits());
//    defmt::println!("twi1: {:b}", foo.TWI1.enable.read().bits());
//    defmt::println!("twim0: {:b}", foo.TWIM0.enable.read().bits());
//    defmt::println!("twim1: {:b}", foo.TWIM1.enable.read().bits());
//    defmt::println!("twis0: {:b}", foo.TWIS0.enable.read().bits());
//    defmt::println!("twis1: {:b}", foo.TWIS1.enable.read().bits());
//
//    defmt::println!("uart0: {:b}", foo.UART0.enable.read().bits());
//    defmt::println!("uarte0: {:b}", foo.UARTE0.enable.read().bits());
//    defmt::println!("uarte1: {:b}", foo.UARTE1.enable.read().bits());
//
//    defmt::println!("radio: {:b}", foo.RADIO.power.read().bits());
//    defmt::println!("radio (state): {:b}", foo.RADIO.state.read().bits());
//
//    defmt::println!("i2s: {:b}", foo.I2S.enable.read().bits());
//    defmt::println!("qspi: {:b}", foo.QSPI.enable.read().bits());
//    defmt::println!("qdec: {:b}", foo.QDEC.enable.read().bits());
//    defmt::println!("qdec: {:b}", foo.QDEC.enable.read().bits());
//
//    defmt::println!("pwm0: {:b}", foo.PWM0.enable.read().bits());
//    defmt::println!("pwm1: {:b}", foo.PWM1.enable.read().bits());
//    defmt::println!("pwm2: {:b}", foo.PWM2.enable.read().bits());
//    defmt::println!("pwm3: {:b}", foo.PWM3.enable.read().bits());
//
//    defmt::println!("pdm: {:b}", foo.PDM.enable.read().bits());
//
//    defmt::println!("saadc: {:b}", foo.SAADC.enable.read().bits());
//    defmt::println!("usb: {:b}", foo.USBD.enable.read().bits());
//    defmt::println!("aar: {:b}", foo.AAR.enable.read().bits());
//    defmt::println!("ccm: {:b}", foo.CCM.enable.read().bits());
//    defmt::println!("crypto: {:b}", foo.CRYPTOCELL.enable.read().bits());
//
//    defmt::println!("nfct: {:b}", foo.NFCT.sleepstate.read().bits()); //NOT Sure if this is telling
//                                                                      //us anything
//
//    //TODO: look at gpio?
//}
