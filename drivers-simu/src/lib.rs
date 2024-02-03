pub mod accel;
pub mod battery;
pub mod button;
pub mod display;
pub mod flash;
pub mod gps;
pub use drivers_shared::lpm013m1126c;
pub mod futures;
pub mod mag;
pub mod time;
pub mod touch;

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

pub async fn wait_display_event() -> DisplayEvent {
    //TODO
    smol::future::pending().await
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
    let context = Context {
        flash: flash::FlashRessources {},
        bat_state: battery::BatteryChargeState {},
        battery: battery::AsyncBattery,
        button: button::Button {},
        backlight: display::Backlight {},
        lcd: display::Display::new(),
        start_time: *time::BOOT,
        mag: mag::MagRessources {},
        touch: touch::TouchRessources {},
        accel: accel::AccelRessources {},
        twi0: TWI0,
        twi1: TWI1,
    };
    let _ = smol::block_on(main.build(context));
    panic!("Main should never return");
}
