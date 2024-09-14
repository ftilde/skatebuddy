#![no_std]

use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Pull},
    peripherals::{TWISPI0, TWISPI1},
    saadc, spim, twim,
};
use embassy_time::Instant;
use static_cell::StaticCell;

use defmt_rtt as _; //logger

use panic_persist as _;
//use panic_probe as _; //panic handler //panic handler

pub mod accel;
pub mod battery;
pub mod button;
pub mod display;
pub mod flash;
pub mod gps;
pub mod hardware;
pub use drivers_shared::lpm013m1126c;
pub mod buzz;
pub mod hrm;
pub mod mag;
pub mod time;
pub mod touch;

mod util;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    panic_persist::report_panic_info(info);
    defmt::error!("{}", defmt::Display2Format(info));

    sys_reset();
}

pub mod futures {
    pub use embassy_futures::*;
}

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
    SPIM2_SPIS2_SPI2 => spim::InterruptHandler<embassy_nrf::peripherals::SPI2>;
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<embassy_nrf::peripherals::TWISPI1>;
    UARTE0_UART0 => embassy_nrf::buffered_uarte::InterruptHandler<gps::UartInstance>;
    QSPI => embassy_nrf::qspi::InterruptHandler<embassy_nrf::peripherals::QSPI>;
});

pub enum DisplayEvent {
    NewBatData,
}

static DISPLAY_EVENT: embassy_sync::signal::Signal<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    DisplayEvent,
> = embassy_sync::signal::Signal::new();

fn signal_display_event(event: DisplayEvent) {
    DISPLAY_EVENT.signal(event);
}

pub async fn wait_display_event() -> DisplayEvent {
    let e = DISPLAY_EVENT.wait().await;
    DISPLAY_EVENT.reset();
    e
}

pub type TWI0 = TWISPI0;
pub type TWI1 = TWISPI1;

pub struct Context {
    pub flash: flash::FlashRessources,
    pub battery: battery::AsyncBattery,
    pub button: button::Button,
    pub backlight: display::Backlight,
    #[allow(unused)]
    //pub gps: gps::GPSRessources,
    pub lcd: display::Display,
    pub start_time: Instant,
    pub mag: mag::MagRessources,
    pub touch: touch::TouchRessources,
    pub accel: accel::AccelRessources,
    pub hrm: hrm::HrmRessources,
    pub buzzer: buzz::Buzzer,
    pub twi0: TWI0,
    pub twi1: TWI1,
    pub last_panic_msg: Option<&'static str>,
}

async fn init(spawner: embassy_executor::Spawner) -> Context {
    let mut conf = embassy_nrf::config::Config::default();
    conf.lfclk_source = embassy_nrf::config::LfclkSource::ExternalXtal;
    conf.dcdc.reg1 = true;

    //let core_p = cortex_m::Peripherals::take().unwrap();
    let mut p = embassy_nrf::init(conf);

    // DO NOT USE! See
    // https://infocenter.nordicsemi.com/index.jsp?topic=%2Ferrata_nRF52840_Rev3%2FERR%2FnRF52840%2FRev3%2Flatest%2Fanomaly_840_195.html
    let _ = p.SPI3;

    //dump_peripheral_regs();

    let battery = battery::Battery::new(p.SAADC, p.P0_03);
    let bat_state = battery::BatteryChargeState::new(p.P0_23, p.P0_25);
    let battery = battery::AsyncBattery::new(&spawner, battery, bat_state);

    let hrm = hrm::HrmRessources::new(p.P0_24, p.P1_00, p.P0_21, p.P0_22);

    // Explicitly "disconnect" the following devices' i2c pins
    // pressure
    let _unused = Input::new(p.P1_15, Pull::None);
    let _unused = Input::new(p.P0_02, Pull::None);

    let buzzer = buzz::Buzzer::new(&spawner, p.P0_19);

    let flash =
        flash::FlashRessources::new(p.QSPI, p.P0_14, p.P0_16, p.P0_15, p.P0_13, p.P1_10, p.P1_11)
            .await;

    let button = button::Button::new(p.P0_17);

    let lcd = display::Display::setup(
        &spawner, p.SPI2, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27,
    )
    .await;

    let backlight = display::Backlight::new(&spawner, p.P0_08, p.PWM3);

    let touch =
        touch::TouchRessources::new(p.P1_01, p.P1_02, p.P1_03, p.P1_04, &mut p.TWISPI0).await;

    let gps = gps::GPSRessources::new(
        p.P0_29,
        p.P0_31,
        p.P0_30,
        p.UARTE0,
        p.TIMER1,
        p.PPI_CH1,
        p.PPI_CH2,
        p.PPI_GROUP1,
    )
    .await;

    spawner.spawn(gps::gps_task(gps)).unwrap();
    spawner.spawn(time::clock_sync_task()).unwrap();

    let mag = mag::MagRessources::new(p.P1_12, p.P1_13);
    let accel = accel::AccelRessources::new(p.P1_06, p.P1_05);

    Context {
        backlight,
        button,
        battery,
        //gps,
        flash,
        lcd,
        start_time: Instant::now(),
        mag,
        touch,
        accel,
        hrm,
        buzzer,
        twi0: p.TWISPI0,
        twi1: p.TWISPI1,
        last_panic_msg: panic_persist::get_panic_message_utf8(),
    }
}

pub fn sys_reset() -> ! {
    cortex_m::peripheral::SCB::sys_reset()
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

#[embassy_executor::task]
async fn main_task(main: impl Main) {
    let spawner = embassy_executor::Spawner::for_current_executor().await;
    let ctx = init(spawner).await;
    let future = main.build(ctx);
    let _ = future.await;
}

static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();

pub fn run(main: impl Main) -> ! {
    let executor = EXECUTOR.init(embassy_executor::Executor::new());

    executor.run(|spawner| {
        // Here we get access to a spawner to spawn the initial tasks.
        defmt::unwrap!(spawner.spawn(main_task(main)));
    });
}
