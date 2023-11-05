use crate::hardware::flash as hw;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    spim,
};
use embedded_hal::digital::v2::PinState;
use embedded_hal_async::spi::{Operation, SpiDevice};

type SPIInstance = embassy_nrf::peripherals::SPI2;

struct FlashHardware {
    spim: crate::util::SpiDeviceWrapper<'static, SPIInstance, Output<'static, hw::CS>>,
}

impl FlashHardware {
    async fn read_status_reg(&mut self) -> Reg1 {
        let cmd = StatusReg::Reg1 as u8;
        let mut out = 0;
        let mut operations = [
            Operation::Write(core::slice::from_ref(&cmd)),
            Operation::Read(core::slice::from_mut(&mut out)),
        ];
        self.spim.transaction(&mut operations).await.unwrap();
        Reg1(out)
    }
    async fn write_enable(&mut self) {
        let cmd = [0x06];
        self.spim.write(&cmd).await.unwrap();
    }
}

struct Reg1(u8);
impl Reg1 {
    fn wel(&self) -> bool {
        (self.0 & 0b10) != 0
    }
    fn wip(&self) -> bool {
        (self.0 & 0b01) != 0
    }
}

#[repr(u8)]
enum StatusReg {
    Reg1 = 0x05,
    //Reg2 = 0x35,
    //Reg3 = 0x15,
}

pub struct FlashRessources {
    hw: FlashHardware,
}

impl FlashRessources {
    pub fn new(spi: SPIInstance, cs: hw::CS, sck: hw::SCK, mosi: hw::MOSI, miso: hw::MISO) -> Self {
        let mut config = spim::Config::default();
        config.frequency = spim::Frequency::M8; //TODO: Maybe we can make this faster
        config.mode = spim::MODE_0;

        let cs = Output::new(cs, Level::High, OutputDrive::Standard);
        let spim = spim::Spim::new(spi, crate::Irqs, sck, miso, mosi, config);

        let spim = crate::util::SpiDeviceWrapper {
            spi: spim,
            cs,
            on: PinState::Low,
        };

        //TODO: enter sleep mode

        Self {
            hw: FlashHardware { spim },
        }
    }

    pub fn on<'a>(&'a mut self) -> Flash<'a> {
        //TODO: leave sleep mode
        Flash { hw: &mut self.hw }
    }
}

pub struct Flash<'a> {
    hw: &'a mut FlashHardware,
}

impl<'a> Flash<'a> {
    pub async fn read(&mut self, addr: u32 /*actually 24 bit*/, out: &mut [u8]) {
        let mut cmd = addr.to_be_bytes();
        cmd[0] = 0x03;
        let mut operations = [Operation::Write(&cmd), Operation::Read(out)];
        self.hw.spim.transaction(&mut operations).await.unwrap();
    }

    pub async fn write(&mut self, addr: u32 /*actually 24 bit*/, buf: &[u8]) {
        assert!(buf.len() <= 256);

        loop {
            let reg = self.hw.read_status_reg().await;
            if reg.wip() {
                continue; //TODO: sleep here for a bit?
            }
            if !reg.wel() {
                self.hw.write_enable().await;
            }
            break;
        }

        let mut cmd = addr.to_be_bytes();
        cmd[0] = 0x02;
        let mut operations = [Operation::Write(&cmd), Operation::Write(&buf)];
        self.hw.spim.transaction(&mut operations).await.unwrap();

        loop {
            let reg = self.hw.read_status_reg().await;
            if reg.wip() {
                defmt::println!("still wip");
                continue;
            }
            break;
        }
    }
}

impl<'a> Drop for Flash<'a> {
    fn drop(&mut self) {
        //TODO: enter sleep mode
    }
}
