use embassy_nrf::{
    gpio::{Level, Output, OutputDrive},
    peripherals::{P0_29, P0_30, P0_31, UARTE0},
    uarte::{Config, Uarte},
};

pub type UartInstance = UARTE0;

pub struct GPSRessources {
    power: Output<'static, P0_29>,
    tx: P0_30,
    rx: P0_31,
    instance: UartInstance,
}

impl GPSRessources {
    pub fn new(power: P0_29, tx: P0_30, rx: P0_31, instance: UartInstance) -> Self {
        Self {
            power: Output::new(power, Level::Low, OutputDrive::Standard),
            tx,
            rx,
            instance,
        }
    }
    pub fn on<'a>(&'a mut self) -> GPS<'a> {
        GPS::new(self)
    }
}

pub struct GPS<'a> {
    power: &'a mut Output<'static, P0_29>,
    uart: Uarte<'a, UartInstance>,
}

impl<'a> GPS<'a> {
    fn new(ressources: &'a mut GPSRessources) -> Self {
        let mut config = Config::default();
        config.baudrate = embassy_nrf::uarte::Baudrate::BAUD9600;
        config.parity = embassy_nrf::uarte::Parity::EXCLUDED;
        let uart = Uarte::new(
            &mut ressources.instance,
            crate::Irqs,
            &mut ressources.rx,
            &mut ressources.tx,
            config,
        );
        ressources.power.set_high();
        GPS {
            power: &mut ressources.power,
            uart,
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) {
        self.uart.read(buf).await.unwrap();
    }
}

impl<'a> Drop for GPS<'a> {
    fn drop(&mut self) {
        self.power.set_low();
    }
}
