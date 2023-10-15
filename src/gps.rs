use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    gpio::{Level, Output, OutputDrive},
    peripherals::{P0_29, P0_30, P0_31, PPI_CH1, PPI_CH2, PPI_GROUP1, TIMER1, UARTE0},
    uarte::Config,
};

pub type UartInstance = UARTE0;
pub type TimerInstance = TIMER1;
pub type Channel1Instance = PPI_CH1;
pub type Channel2Instance = PPI_CH2;
pub type PPIGroupInstance = PPI_GROUP1;
pub type RXPin = P0_30;
pub type TXPin = P0_31;

pub struct GPSRessources {
    power: Output<'static, P0_29>,
    tx: TXPin,
    rx: RXPin,
    instance: UartInstance,
    timer: TimerInstance,
    ppi_ch1: Channel1Instance,
    ppi_ch2: Channel2Instance,
    ppi_group: PPIGroupInstance,
    r_buf: [u8; 128],
    w_buf: [u8; 128],
}

impl GPSRessources {
    pub fn new(
        power: P0_29,
        tx: TXPin,
        rx: RXPin,
        instance: UartInstance,
        timer: TimerInstance,
        ppi_ch1: Channel1Instance,
        ppi_ch2: Channel2Instance,
        ppi_group: PPIGroupInstance,
    ) -> Self {
        Self {
            power: Output::new(power, Level::Low, OutputDrive::Standard),
            tx,
            rx,
            instance,
            timer,
            ppi_ch1,
            ppi_ch2,
            ppi_group,
            r_buf: core::array::from_fn(|_| 0),
            w_buf: core::array::from_fn(|_| 0),
        }
    }
    pub fn on<'a>(&'a mut self) -> GPS<'a> {
        GPS::new(self)
    }
}

pub struct GPS<'a> {
    power: &'a mut Output<'static, P0_29>,
    uart: BufferedUarte<'a, UartInstance, TimerInstance>,
}

impl<'a> GPS<'a> {
    fn new(ressources: &'a mut GPSRessources) -> Self {
        let mut config = Config::default();
        config.baudrate = embassy_nrf::uarte::Baudrate::BAUD9600;
        config.parity = embassy_nrf::uarte::Parity::EXCLUDED;
        let uart = BufferedUarte::new(
            &mut ressources.instance,
            &mut ressources.timer,
            &mut ressources.ppi_ch1,
            &mut ressources.ppi_ch2,
            &mut ressources.ppi_group,
            crate::Irqs,
            &mut ressources.rx,
            &mut ressources.tx,
            config,
            &mut ressources.r_buf,
            &mut ressources.w_buf,
        );
        ressources.power.set_high();
        GPS {
            power: &mut ressources.power,
            uart,
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> usize {
        self.uart.read(buf).await.unwrap()
    }

    //pub async fn casic_cmd(&mut self, cmd: &str) {
    //    let check_sum = cmd.bytes().fold(0u8, |a, b| a ^ b);
    //    let mut check_sum_buf = [0u8; 3];

    //    use core2::io::Write;

    //    write!(check_sum_buf.as_mut_slice(), "*{:02X}", check_sum).unwrap();

    //    self.uart.write(cmd.as_bytes()).await.unwrap();
    //    self.uart.write(&check_sum_buf).await.unwrap();
    //    self.uart.flush().await.unwrap();
    //}
}

impl<'a> Drop for GPS<'a> {
    fn drop(&mut self) {
        self.power.set_low();
    }
}
