pub use drivers_shared::buzz::*;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::Timer;

pub struct Buzzer {
    _marker: (),
}

impl Buzzer {
    pub(crate) fn new(
        spawner: &embassy_executor::Spawner,
        pin: crate::hardware::vibrate::EN,
    ) -> Self {
        spawner.spawn(buzz_task(pin)).unwrap();
        Self { _marker: () }
    }

    pub fn on<'a>(&'a mut self) -> BuzzHandle<'a> {
        BUZZ_SIG.signal(BuzzCmd::On);
        BuzzHandle { _inner: self }
    }
}

pub struct BuzzHandle<'a> {
    _inner: &'a mut Buzzer,
}

impl Drop for BuzzHandle<'_> {
    fn drop(&mut self) {
        BUZZ_SIG.signal(BuzzCmd::Off);
    }
}

impl BuzzHandle<'_> {
    pub fn on(&mut self) {
        BUZZ_SIG.signal(BuzzCmd::On);
    }

    pub fn off(&mut self) {
        BUZZ_SIG.signal(BuzzCmd::On);
    }

    pub fn pattern(&mut self, pat: [u8; 7]) {
        BUZZ_SIG.signal(BuzzCmd::Pattern(pat));
    }
}

static BUZZ_SIG: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    BuzzCmd,
> = embassy_sync::signal::Signal::new();

#[embassy_executor::task]
async fn buzz_task(pin: crate::hardware::vibrate::EN) {
    let mut pin = Output::new(pin, Level::Low, OutputDrive::Standard);
    loop {
        let cmd = BUZZ_SIG.wait().await;
        BUZZ_SIG.reset();

        match cmd {
            BuzzCmd::On => pin.set_high(),
            BuzzCmd::Off => pin.set_low(),
            BuzzCmd::Pattern(pat) => {
                pin.set_high();
                for ms in pat {
                    if ms == 0 {
                        break;
                    }
                    match embassy_futures::select::select(
                        BUZZ_SIG.wait(),
                        Timer::after_millis(ms.into()),
                    )
                    .await
                    {
                        embassy_futures::select::Either::First(_) => break,
                        embassy_futures::select::Either::Second(_) => {}
                    }
                    pin.toggle();
                }
                pin.set_low();
            }
        }
    }
}
