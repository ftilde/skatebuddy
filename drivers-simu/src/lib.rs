pub mod accel;
pub mod battery;
pub mod button;
pub mod display;
pub mod flash;
pub mod gps;
mod util;
use std::sync::Arc;

pub use drivers_shared::lpm013m1126c;
use once_cell::sync::Lazy;
use smol::{
    channel::{Receiver, Sender},
    LocalExecutor,
};
pub mod futures;
pub mod mag;
pub mod time;
pub mod touch;
mod window;

//TODO: Move
pub enum DisplayEvent {
    NewBatData,
}

//static DISPLAY_EVENT: embassy_sync::signal::Signal<
//    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
//    DisplayEvent,
//> = embassy_sync::signal::Signal::new();
//
//fn signal_display_event(event: DisplayEvent) {
//    DISPLAY_EVENT.signal(event);
//}

static DISPLAY_EVENT_PIPE: Lazy<(Sender<DisplayEvent>, Receiver<DisplayEvent>)> =
    Lazy::new(|| smol::channel::bounded(1));

async fn signal_display_event(evt: DisplayEvent) {
    DISPLAY_EVENT_PIPE.0.send(evt).await.unwrap()
}
pub async fn wait_display_event() -> DisplayEvent {
    DISPLAY_EVENT_PIPE.1.recv().await.unwrap()
}

pub struct TWI0;
pub struct TWI1;

pub struct Context {
    pub flash: flash::FlashRessources,
    pub bat_state: battery::BatteryChargeState,
    pub battery: battery::AsyncBattery,
    pub button: button::Button,
    pub backlight: display::Backlight,
    #[allow(unused)]
    //pub gps: gps::GPSRessources,
    pub lcd: display::Display,
    pub start_time: time::Instant,
    pub mag: mag::MagRessources,
    pub touch: touch::TouchRessources,
    pub accel: accel::AccelRessources,
    pub twi0: TWI0,
    pub twi1: TWI1,
}

pub fn sys_reset() -> ! {
    println!("Simulated reset. Exiting Simulator.");
    std::process::exit(0);
}

pub enum Never {}

pub trait Main: 'static {
    fn build(self, context: Context) -> impl core::future::Future<Output = Never> + 'static;
}

impl<F: core::future::Future<Output = Never> + 'static, C: FnOnce(Context) -> F + 'static> Main
    for C
{
    fn build(self, context: Context) -> impl core::future::Future<Output = Never> + 'static {
        self(context)
    }
}

pub fn run(main: impl Main) -> ! {
    let executor = LocalExecutor::new();
    let window = Arc::new(std::sync::Mutex::new(window::Window::new()));

    let context = Context {
        flash: flash::FlashRessources::new(),
        bat_state: battery::BatteryChargeState {},
        battery: battery::AsyncBattery::new(&executor, window.clone()),
        button: button::Button::new(window.clone()),
        backlight: display::Backlight {
            window: window.clone(),
        },
        lcd: display::Display::new(window.clone()),
        start_time: *time::BOOT,
        mag: mag::MagRessources {},
        touch: touch::TouchRessources {
            window: window.clone(),
        },
        accel: accel::AccelRessources {},
        twi0: TWI0,
        twi1: TWI1,
    };
    let _ = smol::block_on(executor.run(main.build(context)));
    panic!("Main should never return");
}
