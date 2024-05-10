use core::future::Future;

use super::{hardware::lcd as hw, lpm013m1126c::Buffer};
use crate::util::SpiDeviceWrapper;
use drivers_shared::display::*;
use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::{PWM3, SPI2},
    pwm::SimplePwm,
    spim,
};
use embassy_time::{Duration, Timer};
use embedded_hal::digital::v2::PinState;

use super::lpm013m1126c;

enum ExcominCmd {
    Run,
    SetFreq(u64),
    Pause,
}

const DEFAULT_EXCOMIN_FREQ: u64 = 1;
const HIGH_EXCOMIN_FREQ: u64 = 120;
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

    let mut run = true;
    loop {
        let mut freq_hz = DEFAULT_EXCOMIN_FREQ;
        let v = EXTCOMIN_SIG.wait().await;
        EXTCOMIN_SIG.reset();
        match v {
            ExcominCmd::Run => {
                run = true;
            }
            ExcominCmd::SetFreq(f) => {
                freq_hz = f;
                if !run {
                    continue;
                }
            }
            ExcominCmd::Pause => {
                run = false;
                continue;
            }
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
    pub async fn on(&mut self) {
        self.disp.set_high();
        EXTCOMIN_SIG.signal(ExcominCmd::Run);
        crate::futures::yield_now().await;
    }

    pub async fn off(&mut self) {
        EXTCOMIN_SIG.signal(ExcominCmd::Pause);
        self.disp.set_low();
        crate::futures::yield_now().await;
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
        disp.on().await;
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
                    embedded_hal_async::spi::Operation::DelayNs(6_000),
                    embedded_hal_async::spi::Operation::Write(&buffer_to_present),
                    embedded_hal_async::spi::Operation::DelayNs(10_000),
                ])
                .await
                .unwrap();
            }
        })
        .await;
    }

    pub async fn present_and<R, F: Future<Output = R>>(&mut self, f: F) -> R {
        let ((), res) = embassy_futures::join::join(self.present(), f).await;
        res
    }
}

pub struct Backlight {
    _marker: (),
}

impl Backlight {
    pub(crate) fn new(spawner: &embassy_executor::Spawner, pin: hw::BL, pwm: PWM3) -> Self {
        spawner.spawn(drive_backlight(pin, pwm)).unwrap();
        Self { _marker: () }
    }

    async fn set_on(&mut self) {
        BACKLIGHT_SIG.signal(BacklightCmd::On);
        crate::futures::yield_now().await;
    }

    pub async fn set_off(&mut self) {
        BACKLIGHT_SIG.signal(BacklightCmd::Off);
        crate::futures::yield_now().await;
    }

    pub async fn active(&mut self) {
        BACKLIGHT_SIG.signal(BacklightCmd::ActiveFor {
            secs: DEFAULT_ACTIVE_DURATION,
        });
        crate::futures::yield_now().await;
    }

    #[must_use]
    pub async fn on<'a>(&'a mut self) -> BacklightOn<'a> {
        self.set_on().await;
        BacklightOn { bl: self }
    }
}

pub struct BacklightOn<'a> {
    bl: &'a mut Backlight,
}

impl Drop for BacklightOn<'_> {
    fn drop(&mut self) {
        crate::futures::block_on(self.bl.set_off());
    }
}

static BACKLIGHT_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    BacklightCmd,
> = embassy_sync::signal::Signal::new();

enum BacklightState<'a> {
    Off(Output<'a, hw::BL>),
    #[allow(dead_code)] // Not sure why this is required here. Annoying...
    On(SimplePwm<'a, PWM3>),
}

impl<'a> BacklightState<'a> {
    fn off(pin: &'a mut hw::BL) -> Self {
        BacklightState::Off(Output::new(pin, Level::Low, OutputDrive::Standard))
    }

    fn on(pwm: &'a mut PWM3, pin: &'a mut hw::BL) -> Self {
        let mut pwm = SimplePwm::new_1ch(pwm, pin);
        let max_duty = 10000;
        pwm.set_max_duty(max_duty);
        pwm.set_duty(0, max_duty - max_duty / 10);
        BacklightState::On(pwm)
    }
}

#[embassy_executor::task]
async fn drive_backlight(mut pin: hw::BL, mut pwm: PWM3) {
    let pin = &mut pin;
    let pwm = &mut pwm;
    let mut _state = BacklightState::off(pin);

    let mut turn_off_after = None;
    loop {
        let v = if let Some(turn_off_after) = turn_off_after.take() {
            match embassy_futures::select::select(
                BACKLIGHT_SIG.wait(),
                Timer::after_secs(turn_off_after as _),
            )
            .await
            {
                embassy_futures::select::Either::First(v) => v,
                embassy_futures::select::Either::Second(_) => {
                    EXTCOMIN_SIG.signal(ExcominCmd::SetFreq(DEFAULT_EXCOMIN_FREQ));
                    if !matches!(_state, BacklightState::Off(..)) {
                        core::mem::drop(_state);
                        _state = BacklightState::off(pin);
                    }
                    continue;
                }
            }
        } else {
            BACKLIGHT_SIG.wait().await
        };
        BACKLIGHT_SIG.reset();
        match v {
            BacklightCmd::ActiveFor { secs } => {
                turn_off_after = Some(secs);
                EXTCOMIN_SIG.signal(ExcominCmd::SetFreq(HIGH_EXCOMIN_FREQ));
                if !matches!(_state, BacklightState::On(..)) {
                    core::mem::drop(_state);
                    _state = BacklightState::on(pwm, pin);
                }
            }
            BacklightCmd::Off => {
                EXTCOMIN_SIG.signal(ExcominCmd::SetFreq(DEFAULT_EXCOMIN_FREQ));
                if !matches!(_state, BacklightState::Off(..)) {
                    core::mem::drop(_state);
                    _state = BacklightState::off(pin);
                }
            }
            BacklightCmd::On => {
                EXTCOMIN_SIG.signal(ExcominCmd::SetFreq(HIGH_EXCOMIN_FREQ));
                if !matches!(_state, BacklightState::On(..)) {
                    core::mem::drop(_state);
                    _state = BacklightState::on(pwm, pin);
                }
            }
        }
    }
}
