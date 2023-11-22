use crate::{hardware::lcd as hw, lpm013m1126c::Buffer};
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::SPI3,
    spim,
};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;

use crate::lpm013m1126c;

enum ExcominCmd {
    Run(u64),
    Pause,
}

const DEFAULT_EXCOMIN_FREQ: u64 = 2;
static EXTCOMIN_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    ExcominCmd,
> = embassy_sync::signal::Signal::new();

#[embassy_executor::task]
async fn drive_ext_com_in(pin: embassy_nrf::peripherals::P0_06) {
    let mut pin = Output::new(pin, Level::Low, OutputDrive::Standard);

    loop {
        let freq_hz;
        let v = EXTCOMIN_SIG.wait().await;
        EXTCOMIN_SIG.reset();
        match v {
            ExcominCmd::Run(f) => freq_hz = f,
            ExcominCmd::Pause => continue,
        };

        let period_us = 1_000_000 / freq_hz;
        let half_period_us = period_us / 2;
        let wait_period = Duration::from_micros(half_period_us);

        while !EXTCOMIN_SIG.signaled() {
            Timer::after(wait_period).await;
            pin.set_high();
            Timer::after(wait_period).await;
            pin.set_low();
        }
    }
}

pub struct Display {
    buffer: lpm013m1126c::Buffer,
    cs: Output<'static, hw::CS>,
    disp: Output<'static, hw::DISP>,
    spi: SPI3,
    sck: hw::SCK,
    mosi: hw::MOSI,
}

impl core::ops::Deref for Display {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}
impl core::ops::DerefMut for Display {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Display {
    pub fn on(&mut self) {
        self.disp.set_high();
        EXTCOMIN_SIG.signal(ExcominCmd::Run(DEFAULT_EXCOMIN_FREQ));
    }

    pub fn off(&mut self) {
        EXTCOMIN_SIG.signal(ExcominCmd::Pause);
        self.disp.set_low();
    }
}

impl Display {
    pub async fn setup(
        spawner: &embassy_executor::Spawner,
        spi: SPI3,
        cs: hw::CS,
        extcomin: hw::EXTCOMIN,
        disp: hw::DISP,
        sck: hw::SCK,
        mosi: hw::MOSI,
    ) -> Display {
        let disp = Output::new(disp, Level::Low, OutputDrive::Standard);

        spawner.spawn(drive_ext_com_in(extcomin)).unwrap();

        let cs = Output::new(cs, Level::Low, OutputDrive::Standard);

        let mut disp = Display {
            buffer: lpm013m1126c::Buffer::default(),
            cs,
            spi,
            sck,
            mosi,
            disp,
        };
        Timer::after(Duration::from_micros(1000)).await;
        disp.on();
        Timer::after(Duration::from_micros(200)).await;

        disp
    }

    pub async fn present<'a, 'b: 'a>(&'b mut self) {
        // TODO: even better: we only need to create they handle for flushing of the data!
        let mut config = spim::Config::default();
        config.frequency = spim::Frequency::M4;
        config.mode = lpm013m1126c::SPI_MODE;
        let spim = spim::Spim::new_txonly(
            &mut self.spi,
            crate::Irqs,
            &mut self.sck,
            &mut self.mosi,
            config,
        );

        let mut spi = crate::util::SpiDeviceWrapper {
            spi: spim,
            cs: &mut self.cs,
            on: PinState::High,
        };

        self.buffer.present(&mut spi).await;

        // Workaround? To make sure the transmission finishes ("stop") before dropping spi
        // handle and thus disabling the spim peripheral
        Timer::after(Duration::from_millis(1)).await;
    }
}

pub struct Backlight {
    level: Level,
    pin: Output<'static, hw::BL>,
}

impl Backlight {
    pub fn new(pin: hw::BL) -> Self {
        let level = Level::Low;
        Self {
            level,
            pin: Output::new(pin, Level::Low, OutputDrive::Standard),
        }
    }

    fn set(&mut self) {
        self.pin.set_level(self.level);
    }

    pub fn on(&mut self) {
        self.level = Level::High;
        self.set();
    }

    pub fn off(&mut self) {
        self.level = Level::Low;
        self.set();
    }

    pub fn toggle(&mut self) {
        self.level = match self.level {
            Level::Low => Level::High,
            Level::High => Level::Low,
        };
        self.set();
    }
}
