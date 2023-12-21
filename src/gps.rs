use core::ops::ControlFlow;

use embassy_nrf::{
    buffered_uarte::BufferedUarte,
    gpio::{Level, Output, OutputDrive},
    peripherals::{P0_29, PPI_CH1, PPI_CH2, PPI_GROUP1, TIMER1, UARTE0},
    uarte::Config,
};

use arrform::{arrform, ArrForm};
use embassy_time::{Duration, Instant, Timer};

use crate::hardware::gps as hw;

pub type UartInstance = UARTE0;
pub type TimerInstance = TIMER1;
pub type Channel1Instance = PPI_CH1;
pub type Channel2Instance = PPI_CH2;
pub type PPIGroupInstance = PPI_GROUP1;

pub struct GPSRessources {
    power: Output<'static, hw::EN>,
    tx: hw::TX,
    rx: hw::RX,
    instance: UartInstance,
    timer: TimerInstance,
    ppi_ch1: Channel1Instance,
    ppi_ch2: Channel2Instance,
    ppi_group: PPIGroupInstance,
    r_buf: [u8; 1024],
    w_buf: [u8; 128],
}

impl GPSRessources {
    pub async fn new(
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
        };

        {
            let mut gps = ret.on().await;

            gps.casic_msg(
                CASICMessageIdentifier {
                    class: 0x06,
                    number: 0x01,
                },
                &[],
            )
            .await;

            let mut t = Instant::now();
            gps.with_messages(|m| {
                match m {
                    Message::Casic(c) => {
                        defmt::println!("CASIC: {}, {}, {:?}", c.id.class, c.id.number, c.payload);
                    }
                    Message::Nmea(c) => {
                        defmt::println!("NMEA: {:?}", c);
                    }
                }
                if t.elapsed() > Duration::from_millis(100) {
                    t = Instant::now();
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .await;

            gps.set_active_satellites(SatelliteConfig {
                gps: true,
                bds: false,
                glonass: false,
            })
            .await;

            gps.set_msg_config(NMEAMsgConfig {
                zda: 1,
                tim: 1,
                ..Default::default()
            })
            .await;

            // Wait SOME time for the chip to process our requests...
            Timer::after(Duration::from_millis(100)).await;
        }

        ret
    }
    pub async fn on<'a>(&'a mut self) -> GPS<'a> {
        let mut gps = GPS::new(self);
        gps.wait_for_init().await;
        gps
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

    pub async fn casic_msg(&mut self, msg_id: CASICMessageIdentifier, payload: &[u8]) {
        let len = payload.len() as u32;
        assert!(len < 2000);
        assert!(len % 4 == 0);
        let header = CASICPacketHeader {
            msg_id,
            len: len as u16,
        };

        let mut checksum = ((msg_id.number as u32) << 24) + ((msg_id.class as u32) << 16) + len;

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

    pub async fn nmea_cmd(&mut self, cmd: &[u8]) {
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

    pub async fn set_active_satellites(&mut self, cfg: SatelliteConfig) {
        let mut cfg_num = 0;
        cfg_num += (cfg.gps as usize) << 0;
        cfg_num += (cfg.bds as usize) << 1;
        cfg_num += (cfg.glonass as usize) << 2;
        let cmd = arrform!(10, "PCAS04,{}", cfg_num);
        self.nmea_cmd(cmd.as_bytes()).await;
    }

    pub async fn set_msg_config(&mut self, cfg: NMEAMsgConfig) {
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

    pub async fn wait_for_init(&mut self) {
        let mut txt_count = 0;
        const NUM_TXT_MSGS_IN_INIT: usize = 4;

        self.with_lines(|line| {
            if &line[..6] != &*b"$GPTXT" {
                txt_count += 1;
            }
            if txt_count >= NUM_TXT_MSGS_IN_INIT {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .await;
    }

    pub async fn with_lines<R>(&mut self, mut f: impl FnMut(&[u8]) -> ControlFlow<R>) -> R {
        let mut buf = [0u8; 128];
        let mut end = 0;
        loop {
            let n_read = self.read(&mut buf[end..]).await;
            if n_read == 1 && buf[end] == 0xff {
                continue;
            }
            let mut read_end = end + n_read;
            while let Some(newline) = buf[end..read_end].iter().position(|b| *b == b'\n') {
                let after_newline = end + newline + 1;
                let line = &buf[..after_newline];

                if let ControlFlow::Break(res) = f(line) {
                    return res;
                }

                buf.copy_within(after_newline..read_end, 0);
                end = 0;
                read_end = read_end - after_newline;
            }
            end = read_end;
        }
    }

    pub async fn with_messages<R>(&mut self, mut f: impl FnMut(Message) -> ControlFlow<R>) -> R {
        enum State {
            Casic,
            Nmea,
            Free,
        }
        let mut state = State::Free;
        let mut old_len = 0;
        loop {
            let current_buf = self.uart.fill_buf().await.unwrap();
            let new_len = current_buf.len();
            if old_len == new_len {
                Timer::after(Duration::from_millis(1)).await;
                continue;
            }
            old_len = new_len;

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

                            self.uart.consume(packet_len);

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

                        self.uart.consume(after_newline);

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
                                to_consume = i;
                            }
                        };
                    }

                    self.uart.consume(to_consume);
                }
            }
        }
    }
}

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

#[repr(C, packed)]
#[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CASICPacketHeader {
    len: u16,
    msg_id: CASICMessageIdentifier,
}

#[repr(C, packed)]
#[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CASICMessageIdentifier {
    class: u8,
    number: u8,
}

pub struct RawCasicMsg<'a> {
    id: CASICMessageIdentifier,
    payload: &'a [u8],
}
