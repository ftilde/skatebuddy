use crate::hardware::lcd as hw;
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
            ExcominCmd::Pause => {
                defmt::println!("Extcomin pause");
                continue;
            }
        };
        defmt::println!("Extcomin run");

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

type DisplayInner<'a> = lpm013m1126c::Display<
    crate::util::SpiDeviceWrapper<'a, SPI3, Output<'a, hw::CS>>,
    Output<'a, hw::DISP>,
>;

pub struct Display<'a> {
    inner: DisplayInner<'a>,
}

impl<'a> core::ops::Deref for Display<'a> {
    type Target = DisplayInner<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<'a> core::ops::DerefMut for Display<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a> Display<'a> {
    pub fn on(&mut self) {
        self.inner.set_on();
        EXTCOMIN_SIG.signal(ExcominCmd::Run(DEFAULT_EXCOMIN_FREQ));
    }

    pub fn off(&mut self) {
        EXTCOMIN_SIG.signal(ExcominCmd::Pause);
        self.inner.set_off();
    }
}

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

    let spi = crate::util::SpiDeviceWrapper {
        spi: spim,
        cs,
        on: PinState::High,
    };

    let mut delay = embassy_time::Delay;
    let lcd = lpm013m1126c::Controller::new(spi, disp, &mut delay);

    spawner.spawn(drive_ext_com_in(extcomin)).unwrap();

    let mut disp = Display {
        inner: lpm013m1126c::Display::new(lcd),
    };
    disp.on();

    disp
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
