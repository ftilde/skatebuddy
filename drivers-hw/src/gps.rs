use core::{ops::ControlFlow, sync::atomic::AtomicU32};

use arrayvec::ArrayVec;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, BufferedUarteRx, BufferedUarteTx},
    gpio::{Level, Output, OutputDrive},
    peripherals::{P0_29, PPI_CH1, PPI_CH2, PPI_GROUP1, TIMER1, UARTE0},
    uarte::Config,
};

use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
    pubsub::{PubSubChannel, Subscriber},
};
use embedded_io_async::Read;

use arrform::{arrform, ArrForm};
use embassy_time::{Duration, Timer};

use super::hardware::gps as hw;

pub use drivers_shared::gps::*;

pub(crate) type UartInstance = UARTE0;
pub(crate) type TimerInstance = TIMER1;
pub(crate) type Channel1Instance = PPI_CH1;
pub(crate) type Channel2Instance = PPI_CH2;
pub(crate) type PPIGroupInstance = PPI_GROUP1;

pub struct GPSRessources {
    power: Output<'static, hw::EN>,
    tx: hw::TX,
    rx: hw::RX,
    instance: UartInstance,
    timer: TimerInstance,
    ppi_ch1: Channel1Instance,
    ppi_ch2: Channel2Instance,
    ppi_group: PPIGroupInstance,
    r_buf: [u8; 256],
    w_buf: [u8; 128],
    line_buf: AsyncBufReader,
}

impl GPSRessources {
    pub(crate) async fn new(
        power: hw::EN,
        tx: hw::TX,
        rx: hw::RX,
        instance: UartInstance,
        timer: TimerInstance,
        ppi_ch1: Channel1Instance,
        ppi_ch2: Channel2Instance,
        ppi_group: PPIGroupInstance,
    ) -> Self {
        let mut ret = Self {
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
            line_buf: AsyncBufReader::new(),
        };

        {
            let mut gps = ret.on().await;
            let mut sender = gps.split().1;

            // Disable all nmea messages
            sender
                .set_nmea_msg_config(NMEAMsgConfig {
                    ..Default::default()
                })
                .await;

            // Only using gps gets us a quicker fix
            sender
                .set_active_satellites(SatelliteConfig {
                    gps: true,
                    bds: false,
                    glonass: false,
                })
                .await;

            // Debug: Print all messages that are enabled
            //gps.casic_msg(CFG_MSG, &[]).await;
            //let mut t = Instant::now();
            //gps.with_messages(|m| {
            //    match m {
            //        Message::Casic(c) => {
            //            defmt::println!("CASIC: {:?}, {:?}", c.id, c.payload);
            //        }
            //        Message::Nmea(c) => {
            //            defmt::println!("NMEA: {:?}", c);
            //        }
            //    }
            //    if t.elapsed() > Duration::from_millis(100) {
            //        t = Instant::now();
            //        ControlFlow::Break(())
            //    } else {
            //        ControlFlow::Continue(())
            //    }
            //})
            //.await;

            // Wait SOME time for the chip to process our requests...
            Timer::after(Duration::from_millis(100)).await;
        }

        ret
    }
    pub async fn on<'a>(&'a mut self) -> GPS<'a> {
        // Drop any leftover data from previous run
        self.line_buf.clear();

        let mut gps = GPS::new(self);
        gps.wait_for_init().await;
        gps
    }
}

pub struct GPS<'a> {
    power: &'a mut Output<'static, P0_29>,
    uart: BufferedUarte<'a, UartInstance, TimerInstance>,
    line_buf: &'a mut AsyncBufReader,
}

pub struct GpsUartReceiver<'g, 'a> {
    uart: BufferedUarteRx<'g, 'a, UartInstance, TimerInstance>,
    line_buf: &'g mut AsyncBufReader,
}
pub struct GpsUartTransmitter<'g, 'a> {
    uart: BufferedUarteTx<'g, 'a, UartInstance, TimerInstance>,
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
            line_buf: &mut ressources.line_buf,
        }
    }

    pub fn split(&mut self) -> (GpsUartReceiver, GpsUartTransmitter) {
        let (rx, tx) = self.uart.split();

        (
            GpsUartReceiver {
                uart: rx,
                line_buf: self.line_buf,
            },
            GpsUartTransmitter { uart: tx },
        )
    }

    async fn wait_for_init(&mut self) {
        // First throw away everything until (and including) the first 0xff which signals the start
        // of the transmission
        loop {
            let n_new = self.line_buf.fill(&mut self.uart).await;
            let current_buf = self.line_buf.buf();
            if n_new == 0 {
                defmt::println!("wait bc unchanged len");
                Timer::after(Duration::from_millis(1)).await;
                continue;
            }

            if let Some(pos) = current_buf.iter().position(|b| *b == 0xff) {
                self.line_buf.consume(pos + 1);
                break;
            } else {
                self.line_buf.consume(current_buf.len());
            }
        }

        // Then wait and drop the first 5 GPTXT messages which include chip information
        let mut n = 0;
        self.split()
            .0
            .with_messages(|m| {
                match m {
                    Message::Casic(_c) => {}
                    Message::Nmea(c) => {
                        if &c[..6] == b"$GPTXT" {
                            n += 1;
                            if n == 5 {
                                return ControlFlow::Break(());
                            }
                        }
                    }
                }
                ControlFlow::Continue(())
            })
            .await;
    }
}

impl<'g, 'a> GpsUartTransmitter<'g, 'a> {
    async fn casic_msg(&mut self, msg_id: CASICMessageIdentifier, payload: &[u8]) {
        let len = payload.len() as u32;
        assert!(len < 2000);
        assert!(len % 4 == 0);
        let header = CASICPacketHeader {
            msg_id,
            len: len as u16,
        };

        let mut checksum = ((msg_id[1] as u32) << 24) + ((msg_id[0] as u32) << 16) + len;

        for bytes in payload.chunks_exact(4) {
            let bytes: &[u8; 4] = bytes.try_into().unwrap();
            let val = u32::from_le_bytes(*bytes);
            checksum = checksum.wrapping_add(val);
        }

        self.uart.write(&CASIC_MAGIC_HEADER).await.unwrap();
        self.uart.write(bytemuck::bytes_of(&header)).await.unwrap();
        self.uart.write(payload).await.unwrap();
        self.uart.write(&checksum.to_le_bytes()).await.unwrap();
        self.uart.flush().await.unwrap();
    }

    async fn nmea_cmd(&mut self, cmd: &[u8]) {
        let check_sum = cmd.iter().fold(0u8, |a, b| a ^ b);
        let prefix = b"$";
        let mut check_sum_buf = [0u8; 5];

        use core2::io::Write;

        write!(check_sum_buf.as_mut_slice(), "*{:02X}\r\n", check_sum).unwrap();

        defmt::println!(
            "MSG: {}{}{}",
            core::str::from_utf8(prefix).unwrap(),
            core::str::from_utf8(cmd).unwrap(),
            core::str::from_utf8(&check_sum_buf).unwrap()
        );

        self.uart.write(prefix).await.unwrap();
        self.uart.write(cmd).await.unwrap();
        self.uart.write(&check_sum_buf).await.unwrap();
        self.uart.flush().await.unwrap();
    }

    async fn set_active_satellites(&mut self, cfg: SatelliteConfig) {
        let mut cfg_num = 0;
        cfg_num += (cfg.gps as usize) << 0;
        cfg_num += (cfg.bds as usize) << 1;
        cfg_num += (cfg.glonass as usize) << 2;
        let cmd = arrform!(10, "PCAS04,{}", cfg_num);
        self.nmea_cmd(cmd.as_bytes()).await;
    }

    async fn set_msg_freq(&mut self, msg_id: CASICMessageIdentifier, rate: u16) {
        #[repr(C, packed)]
        #[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
        struct Payload {
            msg_id: CASICMessageIdentifier,
            rate: u16,
        }

        let payload = Payload { msg_id, rate };

        self.casic_msg(CFG_MSG, bytemuck::bytes_of(&payload)).await
    }
    async fn set_casic_msg_config(&mut self, config: CasicMsgConfig) {
        self.set_msg_freq(NAV_TIME_UTC, config.nav_time).await;
        self.set_msg_freq(NAV_PV, config.nav_pv).await;
        self.set_msg_freq(NAV_GPS_INFO, config.nav_gps_info).await;
    }

    async fn set_nmea_msg_config(&mut self, cfg: NMEAMsgConfig) {
        fn p(i: u8) -> u8 {
            i.min(9)
        }

        let cmd = arrform!(
            37,
            "PCAS03,{},{},{},{},{},{},{},{},{},{},,,{},{},,,,{}",
            p(cfg.gga),
            p(cfg.gll),
            p(cfg.gsa),
            p(cfg.gsv),
            p(cfg.rmc),
            p(cfg.vtg),
            p(cfg.zda),
            p(cfg.ant),
            p(cfg.dhv),
            p(cfg.lps),
            p(cfg.utc),
            p(cfg.gst),
            p(cfg.tim),
        );
        self.nmea_cmd(cmd.as_bytes()).await;
    }
}

impl<'g, 'a> GpsUartReceiver<'g, 'a> {
    pub(crate) async fn with_messages<R>(
        &mut self,
        mut f: impl FnMut(Message) -> ControlFlow<R>,
    ) -> R {
        enum State {
            Casic,
            Nmea,
            Free,
        }
        let mut state = State::Free;
        loop {
            let n_new = self.line_buf.fill(&mut self.uart).await;
            let current_buf = self.line_buf.buf();
            if n_new == 0 {
                defmt::println!("wait bc unchanged len");
                Timer::after(Duration::from_millis(1)).await;
                continue;
            }
            //defmt::println!("buffer is now: {:?}", current_buf);

            match state {
                State::Casic => {
                    let header_len = core::mem::size_of::<CASICPacketHeader>();
                    if current_buf.len() >= header_len {
                        let header: &CASICPacketHeader =
                            bytemuck::from_bytes(&current_buf[..header_len]);

                        let magic_len = 4;
                        let payload_len = header.len as usize;
                        let packet_len = header_len + payload_len + magic_len;
                        if current_buf.len() >= packet_len {
                            //TODO: we could also check the checksum... meh...

                            let payload_buf = &current_buf[header_len..][..payload_len];
                            let res = f(Message::Casic(RawCasicMsg {
                                id: header.msg_id,
                                payload: payload_buf,
                            }));

                            self.line_buf.consume(packet_len);

                            if let ControlFlow::Break(res) = res {
                                return res;
                            }

                            state = State::Free;
                        }
                    }
                }
                State::Nmea => {
                    if let Some(newline_pos) = current_buf.iter().position(|b| *b == b'\n') {
                        let after_newline = newline_pos + 1;
                        let line = &current_buf[..after_newline];
                        let res = f(Message::Nmea(line));

                        self.line_buf.consume(after_newline);

                        if let ControlFlow::Break(res) = res {
                            return res;
                        }

                        state = State::Free;
                    }
                }

                State::Free => {
                    let mut to_consume = 0;
                    for (i, w) in current_buf.windows(2).enumerate() {
                        let w: [u8; 2] = w.try_into().unwrap();
                        match w {
                            [b'$', _] => {
                                state = State::Nmea;
                                to_consume = i;
                                break;
                            }
                            CASIC_MAGIC_HEADER => {
                                state = State::Casic;
                                to_consume = i + CASIC_MAGIC_HEADER.len();
                                break;
                            }
                            _ => {
                                to_consume = i + 1;
                            }
                        };
                    }

                    //if to_consume > 0 {
                    //    defmt::println!("trashing {}", &current_buf[..to_consume]);
                    //}
                    self.line_buf.consume(to_consume);
                }
            }
        }
    }
}

struct AsyncBufReader {
    inner: [u8; 1024],
    end: usize,
}

impl AsyncBufReader {
    fn new() -> Self {
        Self {
            inner: core::array::from_fn(|_| 0),
            end: 0,
        }
    }

    async fn fill<B: Read>(&mut self, r: &mut B) -> usize {
        let n_new = r.read(&mut self.inner[self.end..]).await.unwrap();
        self.end += n_new;
        n_new
    }

    fn buf(&self) -> &[u8] {
        &self.inner[..self.end]
    }

    fn consume(&mut self, n: usize) {
        self.inner.copy_within(n..self.end, 0);
        self.end -= n;
    }

    fn clear(&mut self) {
        self.consume(self.end);
    }
}

#[derive(defmt::Format)]
pub enum Message<'a> {
    Casic(RawCasicMsg<'a>),
    Nmea(&'a [u8]),
}

pub struct SatelliteConfig {
    pub gps: bool,
    pub bds: bool,
    pub glonass: bool,
}

#[derive(Default)]
pub struct NMEAMsgConfig {
    pub gga: u8,
    pub gll: u8,
    pub gsa: u8,
    pub gsv: u8,
    pub rmc: u8,
    pub vtg: u8,
    pub zda: u8,
    pub ant: u8,
    pub dhv: u8,
    pub lps: u8,
    pub utc: u8,
    pub gst: u8,
    pub tim: u8,
}

impl<'a> Drop for GPS<'a> {
    fn drop(&mut self) {
        self.power.set_low();
    }
}

const CASIC_MAGIC_HEADER_0: u8 = 0xba;
const CASIC_MAGIC_HEADER_1: u8 = 0xce;
const CASIC_MAGIC_HEADER: [u8; 2] = [CASIC_MAGIC_HEADER_0, CASIC_MAGIC_HEADER_1];

//static CLOCK_INFO: Mutex<CriticalSectionRawMutex, RefCell<ClockInfo>> =
const MAX_SUBSCRIBERS: usize = 3;
const MAX_MSGS: usize = 4;

type GpsPubSubChannel =
    PubSubChannel<CriticalSectionRawMutex, CasicMsg, MAX_MSGS, MAX_SUBSCRIBERS, 1>;
static GPS_PUB_SUB_CHANNEL: GpsPubSubChannel = GpsPubSubChannel::new();

type GpsControlChannel = Channel<CriticalSectionRawMutex, GPSControlMsg, 1>;
static GPS_CONTROL_CHANNEL: GpsControlChannel = GpsControlChannel::new();

#[derive(Clone, Copy)]
struct GpsSubscriber {
    id: u32,
    config: CasicMsgConfig,
}

fn compute_merged_config(subscribers: &[GpsSubscriber]) -> CasicMsgConfig {
    let mut out = CasicMsgConfig::default();
    for s in subscribers {
        out = out.merge(&s.config);
    }
    out
}

#[derive(Default)]
struct GpsState {
    subscribers: ArrayVec<GpsSubscriber, MAX_SUBSCRIBERS>,
    merged_config: CasicMsgConfig,
}

impl GpsState {
    async fn handle_subscribers(&mut self) {
        let msg = GPS_CONTROL_CHANNEL.receive().await;
        match msg {
            GPSControlMsg::Subscribe(config, id) => {
                self.subscribers.push(GpsSubscriber { id, config });
                self.merged_config = compute_merged_config(&self.subscribers);
            }
            GPSControlMsg::UpdateConfig(config, id) => {
                self.subscribers
                    .iter_mut()
                    .find(|s| s.id == id)
                    .unwrap()
                    .config = config;
                self.merged_config = compute_merged_config(&self.subscribers);
            }
            GPSControlMsg::Unsubscribe(id) => {
                self.subscribers = self
                    .subscribers
                    .clone()
                    .into_iter()
                    .filter(|s| s.id != id)
                    .collect();

                self.merged_config = compute_merged_config(&self.subscribers);
            }
        }
    }
}

#[embassy_executor::task]
pub(crate) async fn gps_task(mut ressources: GPSRessources) -> ! {
    let mut state = GpsState::default();

    let publisher = GPS_PUB_SUB_CHANNEL.publisher().unwrap();

    loop {
        // GPS inactive
        defmt::println!("GPS off");
        while state.subscribers.is_empty() {
            state.handle_subscribers().await;
        }

        // GPS active
        defmt::println!("GPS on");
        let mut gps = ressources.on().await;
        let (mut rx, mut tx) = gps.split();

        let mut handle_messages = rx.with_messages(|msg| {
            match msg {
                Message::Casic(msg) => {
                    publisher.publish_immediate(msg.parse());
                }
                Message::Nmea(s) => {
                    let s = core::str::from_utf8(s).unwrap();
                    defmt::println!("NMEA: {}", s);
                }
            }
            ControlFlow::Continue::<()>(())
        });
        let mut handle_messages = core::pin::pin!(handle_messages);

        while !state.subscribers.is_empty() {
            tx.set_casic_msg_config(state.merged_config).await;
            embassy_futures::select::select(state.handle_subscribers(), &mut handle_messages).await;
        }
    }
}

static RECEIVER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
enum GPSControlMsg {
    Subscribe(CasicMsgConfig, u32),
    UpdateConfig(CasicMsgConfig, u32),
    Unsubscribe(u32),
}

pub struct GPSReceiver<'a> {
    msgs: Subscriber<'a, CriticalSectionRawMutex, CasicMsg, MAX_MSGS, MAX_SUBSCRIBERS, 1>,
    id: u32,
}

impl Drop for GPSReceiver<'_> {
    fn drop(&mut self) {
        GPS_CONTROL_CHANNEL
            .try_send(GPSControlMsg::Unsubscribe(self.id))
            .unwrap();
    }
}

impl<'a> GPSReceiver<'a> {
    pub async fn new(config: CasicMsgConfig) -> GPSReceiver<'a> {
        let id = RECEIVER_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        GPS_CONTROL_CHANNEL
            .send(GPSControlMsg::Subscribe(config, id))
            .await;
        GPSReceiver {
            msgs: GPS_PUB_SUB_CHANNEL.subscriber().unwrap(),
            id,
        }
    }

    pub async fn update_config(&mut self, config: CasicMsgConfig) {
        GPS_CONTROL_CHANNEL
            .send(GPSControlMsg::UpdateConfig(config, self.id))
            .await;
    }

    pub async fn receive(&mut self) -> CasicMsg {
        loop {
            match self.msgs.next_message().await {
                embassy_sync::pubsub::WaitResult::Lagged(_) => {}
                embassy_sync::pubsub::WaitResult::Message(r) => return r,
            }
        }
    }
}
