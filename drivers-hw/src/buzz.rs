pub use drivers_shared::buzz::*;
use embassy_nrf::gpio::{Level, Output, OutputDrive};

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

    pub fn on<'a>(&'a mut self) -> BuzzGuard<'a> {
        BUZZ_SIG.signal(BuzzCmd::On);
        BuzzGuard { _inner: self }
    }
}

pub struct BuzzGuard<'a> {
    _inner: &'a mut Buzzer,
}

impl Drop for BuzzGuard<'_> {
    fn drop(&mut self) {
        BUZZ_SIG.signal(BuzzCmd::Off);
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

        match cmd {
            BuzzCmd::On => pin.set_high(),
            BuzzCmd::Off => pin.set_low(),
        }
    }
}
