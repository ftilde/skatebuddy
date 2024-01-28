#![no_std]

use embassy_nrf::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals::{TWISPI0, TWISPI1},
    saadc, spim, twim,
};
use embassy_time::Instant;
use littlefs2::fs::Filesystem;

pub mod accel;
pub mod battery;
pub mod button;
pub mod display;
pub mod flash;
pub mod gps;
pub mod hardware;
pub mod lpm013m1126c;
pub mod mag;
pub mod time;
pub mod touch;

mod util;

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

pub struct Context {
    pub flash: flash::FlashRessources,
    pub bat_state: battery::BatteryChargeState,
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
    pub twi0: TWISPI0,
    pub twi1: TWISPI1,
}

pub type Spawner = embassy_executor::Spawner;

pub async fn init(spawner: Spawner) -> Context {
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
    let battery = battery::AsyncBattery::new(&spawner, battery);

    // Keep hrm in reset to power it off
    let _hrm_power = Output::new(p.P0_21, Level::Low, OutputDrive::Standard);

    // Explicitly "disconnect" the following devices' i2c pins
    // Heartrate
    let _unused = Input::new(p.P0_24, Pull::None);
    let _unused = Input::new(p.P1_00, Pull::None);

    // pressure
    let _unused = Input::new(p.P1_15, Pull::None);
    let _unused = Input::new(p.P0_02, Pull::None);

    let _vibrate = Output::new(p.P0_19, Level::Low, OutputDrive::Standard);

    let mut flash =
        flash::FlashRessources::new(p.QSPI, p.P0_14, p.P0_16, p.P0_15, p.P0_13, p.P1_10, p.P1_11)
            .await;
    {
        let mut alloc = Filesystem::allocate();

        let mut flash = flash.on().await;
        let fs = match Filesystem::mount(&mut alloc, &mut flash) {
            Ok(fs) => {
                defmt::println!("Mounting existing fs",);
                fs
            }
            Err(_e) => {
                defmt::println!("Formatting fs because of mount error",);
                Filesystem::format(&mut flash).unwrap();
                Filesystem::mount(&mut alloc, &mut flash).unwrap()
            }
        };

        let num_boots = fs
            .open_file_with_options_and_then(
                |options| options.read(true).write(true).create(true),
                &littlefs2::path::PathBuf::from(b"bootcount.bin"),
                |file| {
                    let mut boot_num = 0u32;
                    if file.len().unwrap() >= 4 {
                        file.read(bytemuck::bytes_of_mut(&mut boot_num)).unwrap();
                    };
                    boot_num += 1;
                    file.seek(littlefs2::io::SeekFrom::Start(0)).unwrap();
                    file.write(bytemuck::bytes_of(&boot_num)).unwrap();
                    Ok(boot_num)
                },
            )
            .unwrap();
        defmt::println!("This is boot nr {}", num_boots);
    }

    //let mut battery = battery::AccurateBatteryReader::new(&spawner, battery);
    let bat_state = battery::BatteryChargeState::new(p.P0_23, p.P0_25);

    let button = button::Button::new(p.P0_17);

    let lcd = display::Display::setup(
        &spawner, p.SPI2, p.P0_05, p.P0_06, p.P0_07, p.P0_26, p.P0_27,
    )
    .await;

    let backlight = display::Backlight::new(p.P0_08);

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

    spawner.spawn(time::clock_sync_task(gps)).unwrap();

    let mag = mag::MagRessources::new(p.P1_12, p.P1_13);
    let accel = accel::AccelRessources::new(p.P1_06, p.P1_05);

    Context {
        backlight,
        button,
        bat_state,
        battery,
        //gps,
        flash,
        lcd,
        start_time: Instant::now(),
        mag,
        touch,
        accel,
        twi0: p.TWISPI0,
        twi1: p.TWISPI1,
    }
}
