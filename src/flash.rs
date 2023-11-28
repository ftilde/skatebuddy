use crate::hardware::flash as hw;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    spim,
};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;
use embedded_hal_async::spi::{Operation, SpiDevice};

type SPIInstance = embassy_nrf::peripherals::SPI2;

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
    instance: SPIInstance,
    cs: Output<'static, hw::CS>,
    sck: hw::SCK,
    mosi: hw::MOSI,
    miso: hw::MISO,
}

impl FlashRessources {
    pub async fn new(
        instance: SPIInstance,
        cs: hw::CS,
        sck: hw::SCK,
        mosi: hw::MOSI,
        miso: hw::MISO,
    ) -> Self {
        let cs = Output::new(cs, Level::High, OutputDrive::Standard);

        let mut s = Self {
            instance,
            cs,
            sck,
            mosi,
            miso,
        };
        {
            let _ = s.on().await;
            // Drop the handle and thus enter deep sleep
        }
        s
    }

    pub async fn on<'a>(&'a mut self) -> Flash<'a> {
        let mut config = spim::Config::default();
        config.frequency = spim::Frequency::M8; //TODO: Maybe we can make this faster
        config.mode = spim::MODE_0;

        let spim = spim::Spim::new(
            &mut self.instance,
            crate::Irqs,
            &mut self.sck,
            &mut self.miso,
            &mut self.mosi,
            config,
        );

        let spim = crate::util::SpiDeviceWrapper {
            spi: spim,
            cs: &mut self.cs,
            on: PinState::Low,
        };

        let mut s = Flash { spim };
        s.wake_up_from_deep_sleep().await;
        s
    }
}

pub struct Flash<'a> {
    spim: crate::util::SpiDeviceWrapper<'a, SPIInstance, Output<'static, hw::CS>>,
}

impl<'a> Flash<'a> {
    //ONLY used internally
    async fn enter_deep_sleep(&mut self) {
        self.wait_idle().await;

        // Deep Power-Down command
        self.spim.write(&[0xb9]).await.unwrap();
    }

    //ONLY used internally
    async fn wake_up_from_deep_sleep(&mut self) {
        self.wait_idle().await;

        // Release from Deep Power-Down and Read Device ID command
        self.spim.write(&[0xab]).await.unwrap();
        // Wait for t_RES1, i.e. the wake up
        Timer::after(Duration::from_micros(20)).await;
    }

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

    pub async fn read(&mut self, addr: u32 /*actually 24 bit*/, out: &mut [u8]) {
        let mut cmd = addr.to_be_bytes();
        cmd[0] = 0x03;
        let mut operations = [Operation::Write(&cmd), Operation::Read(out)];
        self.spim.transaction(&mut operations).await.unwrap();
    }

    pub async fn wait_idle(&mut self) -> Reg1 {
        loop {
            let reg = self.read_status_reg().await;
            if !reg.wip() {
                return reg;
            }
            //TODO: sleep for a bit?
        }
    }

    pub async fn write(&mut self, addr: u32 /*actually 24 bit*/, buf: &[u8]) {
        assert!(buf.len() <= 256);

        let reg = self.wait_idle().await;
        if !reg.wel() {
            self.write_enable().await;
        }

        let mut cmd = addr.to_be_bytes();
        cmd[0] = 0x02;
        let mut operations = [Operation::Write(&cmd), Operation::Write(&buf)];
        self.spim.transaction(&mut operations).await.unwrap();

        self.wait_idle().await;
    }
}

impl<'a> Drop for Flash<'a> {
    fn drop(&mut self) {
        // Not super nice to block here, but we don't have another option. it also does not make
        // sense to implement a separate blocking procedure here. We don't even expect this to take
        // very long.
        embassy_futures::block_on(self.enter_deep_sleep());
    }
}
