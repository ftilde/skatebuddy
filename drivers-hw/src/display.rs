use core::future::Future;

use super::{hardware::lcd as hw, lpm013m1126c::Buffer};
use crate::util::SpiDeviceWrapper;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::SPI2,
    spim,
};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;

use super::lpm013m1126c;

enum ExcominCmd {
    Run(u64),
    Pause,
}

const DEFAULT_EXCOMIN_FREQ: u64 = 1;
static EXTCOMIN_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    ExcominCmd,
> = embassy_sync::signal::Signal::new();

#[embassy_executor::task]
async fn drive_ext_com_in(pin: embassy_nrf::peripherals::P0_06) {
    let mut pin = Output::new(pin, Level::Low, OutputDrive::Standard);

    // Reserve some time to do actual calculation, wake up, scheduling, etc.
    let calc_period_estimated = Duration::from_micros(10);

    //Minimum time according to data sheet
    let wait_period_high = Duration::from_micros(2);

    loop {
        let freq_hz;
        let v = EXTCOMIN_SIG.wait().await;
        EXTCOMIN_SIG.reset();
        match v {
            ExcominCmd::Run(f) => freq_hz = f,
            ExcominCmd::Pause => continue,
        };

        let period_us = 1_000_000 / freq_hz;
        let period = Duration::from_micros(period_us) - calc_period_estimated;

        let wait_period_low = period - wait_period_high;

        while !EXTCOMIN_SIG.signaled() {
            Timer::after(wait_period_low).await;
            pin.set_high();
            Timer::after(wait_period_high).await;
            pin.set_low();
        }
    }
}

pub struct Display {
    buffer: lpm013m1126c::Buffer,
    spi: SPI2,
    cs: Output<'static, hw::CS>,
    disp: Output<'static, hw::DISP>,
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

    pub(crate) async fn setup(
        spawner: &embassy_executor::Spawner,
        spi: SPI2,
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
            spi,
            cs,
            sck,
            mosi,
            disp,
        };
        Timer::after(Duration::from_micros(1000)).await;
        disp.on();
        Timer::after(Duration::from_micros(200)).await;

        disp
    }

    async fn with_spi<'a, F: Future<Output = ()> + 'a>(
        &'a mut self,
        f: impl FnOnce(&'a mut Buffer, SpiDeviceWrapper<'a, SPI2, Output<'static, hw::CS>>) -> F,
    ) {
        let mut config = spim::Config::default();
        config.frequency = spim::Frequency::M2;
        config.mode = lpm013m1126c::SPI_MODE;
        let spim = spim::Spim::new_txonly(
            &mut self.spi,
            crate::Irqs,
            &mut self.sck,
            &mut self.mosi,
            config,
        );

        let spi: SpiDeviceWrapper<'a, SPI2, Output<hw::CS>> = SpiDeviceWrapper {
            spi: spim,
            cs: &mut self.cs,
            on: PinState::High,
        };

        f(&mut self.buffer, spi).await
    }

    pub async fn clear(&mut self) {
        self.with_spi(|_buffer, mut spi| async move {
            lpm013m1126c::clear(&mut spi).await;
        })
        .await;
    }

    pub async fn blink(&mut self, mode: lpm013m1126c::BlinkMode) {
        self.with_spi(|_buffer, mut spi| async move {
            lpm013m1126c::blink(&mut spi, mode).await;
        })
        .await;
    }

    pub async fn present(&mut self) {
        self.with_spi(|buffer, mut spi| async move {
            if let Some(buffer_to_present) = buffer.lines_for_update() {
                use embedded_hal_async::spi::SpiDevice;
                spi.transaction(&mut [
                    embedded_hal_async::spi::Operation::Write(&buffer_to_present),
                    embedded_hal_async::spi::Operation::DelayNs(10_000),
                ])
                .await
                .unwrap();
            }
        })
        .await;
    }
}

pub struct Backlight {
    level: Level,
    pin: Output<'static, hw::BL>,
}

impl Backlight {
    pub(crate) fn new(pin: hw::BL) -> Self {
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

    //pub fn toggle(&mut self) {
    //    self.level = match self.level {
    //        Level::Low => Level::High,
    //        Level::High => Level::Low,
    //    };
    //    self.set();
    //}
}
