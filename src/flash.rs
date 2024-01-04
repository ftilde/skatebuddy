use crate::hardware::flash as hw;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    qspi,
};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;
use embedded_hal_async::spi::{Operation, SpiDevice};

type SPIInstance = embassy_nrf::peripherals::QSPI;

pub struct Reg1(u8);
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
    spi: SPIInstance,
    cs: hw::CS,
    sck: hw::SCK,
    mosi: hw::MOSI,
    miso: hw::MISO,
    unused0: hw::UNUSED0,
    unused1: hw::UNUSED1,
}

impl FlashRessources {
    pub async fn new(
        spi: SPIInstance,
        cs: hw::CS,
        sck: hw::SCK,
        mosi: hw::MOSI,
        miso: hw::MISO,
        unused0: hw::UNUSED0,
        unused1: hw::UNUSED1,
    ) -> Self {
        let mut s = Self {
            spi,
            cs,
            sck,
            mosi,
            miso,
            unused0,
            unused1,
        };
        {
            let _ = s.on().await;
            // Drop the handle and thus enter deep sleep
        }
        s
    }

    pub async fn on<'a>(&'a mut self) -> Flash<'a> {
        let mut config = qspi::Config::default();
        config.xip_offset = 0;
        config.read_opcode = qspi::ReadOpcode::READ2IO;
        config.write_opcode = qspi::WriteOpcode::PP;
        config.write_page_size = qspi::WritePageSize::_256BYTES;
        config.deep_power_down = None; //TODO
                                       //
        config.frequency = qspi::Frequency::M32;
        //When running at 32MHz we also have to adjust the read-relay from 1/32Hz to 1/64Hz or
        //things become glitchy...
        config.rx_delay = 1;
        config.sck_delay = 1; // T_SHSL = min 20ns -> we choose 1 * 62.5ns here

        config.spi_mode = qspi::SpiMode::MODE0;
        config.address_mode = qspi::AddressMode::_24BIT;
        config.capacity = hw::SIZE as _;

        let mut qspi = qspi::Qspi::new(
            &mut self.spi,
            crate::Irqs,
            &mut self.sck,
            &mut self.cs,
            &mut self.mosi,
            &mut self.miso,
            &mut self.unused0,
            &mut self.unused1,
            config,
        );

        let mut s = Flash { qspi };
        s.wake_up_from_deep_sleep().await;
        s
    }
}

pub struct Flash<'a> {
    qspi: qspi::Qspi<'a, SPIInstance>,
}

const WRITE_BLOCK_SIZE: usize = 256;
const SECTOR_SIZE: usize = 4096;

impl<'a> Flash<'a> {
    //ONLY used internally
    async fn enter_deep_sleep(&mut self) {
        self.wait_idle().await;

        // Deep Power-Down command
        self.qspi
            .custom_instruction(0xb9, &[], &mut [])
            .await
            .unwrap();
    }

    //ONLY used internally
    async fn wake_up_from_deep_sleep(&mut self) {
        self.wait_idle().await;

        // Release from Deep Power-Down and Read Device ID command
        self.qspi
            .custom_instruction(0xab, &[], &mut [])
            .await
            .unwrap();
        // Wait for t_RES1, i.e. the wake up
        Timer::after(Duration::from_micros(20)).await;
    }

    async fn read_status_reg(&mut self) -> Reg1 {
        let cmd = StatusReg::Reg1 as u8;
        let mut out = 0;
        self.qspi
            .custom_instruction(cmd, &[], core::slice::from_mut(&mut out))
            .await;
        Reg1(out)
    }
    async fn write_enable(&mut self) {
        let cmd = 0x06;
        self.qspi
            .custom_instruction(cmd, &[], &mut [])
            .await
            .unwrap();
    }

    pub async fn read(&mut self, addr: u32 /*actually 24 bit*/, out: &mut [u8]) {
        self.wait_idle().await;
        self.qspi.read(addr, out).await;
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
        let initial_end = (WRITE_BLOCK_SIZE - (addr as usize % WRITE_BLOCK_SIZE)).min(buf.len());
        let initial = &buf[..initial_end];

        self.write_inner(addr, initial).await;
        let rest = &buf[initial_end..];
        let rest_begin = addr + initial_end as u32;
        for (i, block) in rest.windows(WRITE_BLOCK_SIZE).enumerate() {
            self.write_inner(rest_begin + (i * WRITE_BLOCK_SIZE) as u32, block)
                .await;
        }
    }

    async fn write_inner(&mut self, addr: u32 /*actually 24 bit*/, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        assert!(buf.len() <= WRITE_BLOCK_SIZE);
        let begin = addr as usize;
        let end = begin + buf.len();
        let begin_block = begin / WRITE_BLOCK_SIZE;
        let end_block = end / WRITE_BLOCK_SIZE;
        assert_eq!(begin_block, end_block);

        let reg = self.wait_idle().await;
        if !reg.wel() {
            self.write_enable().await;
        }

        let mut cmd = addr.to_be_bytes();
        self.qspi.write(addr, buf).await;

        self.wait_idle().await;
    }

    pub async fn erase(&mut self, addr: u32 /*actually 24 bit*/) {
        let reg = self.wait_idle().await;
        if !reg.wel() {
            self.write_enable().await;
        }

        self.qspi.erase(addr).await;

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
