use core::{
    cell::RefCell,
    ops::ControlFlow,
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
};

use crate::gps;

use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
pub use embassy_time::*;
use util::ClockScale;

const DRIFT_THRESHOLD_LONGER: i32 = 5;
const DRIFT_THRESHOLD_SHORTER: i32 = 10;

#[embassy_executor::task]
pub(crate) async fn clock_sync_task(mut gps: gps::GPSRessources) {
    const INITIAL_SYNC_TIME: Duration = Duration::from_secs(2 * 60);
    const INCREMENTAL_SYNC_TIME: Duration = Duration::from_secs(1 * 60);

    let mut success_wait_time = Duration::from_secs(60 * 60 * 4);

    let mut sync_time = INITIAL_SYNC_TIME;

    Timer::after(Duration::from_secs(1 * 60)).await;
    loop {
        let before_sync = Instant::now();
        let res = {
            let mut gps = gps.on().await;
            sync_clock(&mut gps, sync_time).await
        };
        let next_sync = if res.is_ok() {
            let sync_duration = before_sync.elapsed();

            LAST_SYNC_DURATION_S.store(sync_duration.as_secs() as _, Ordering::Relaxed);
            LAST_SYNC_TS_S.store(Instant::now().as_secs() as _, Ordering::Relaxed);

            sync_time = INCREMENTAL_SYNC_TIME;

            let drift = last_drift_s().abs();

            if drift < DRIFT_THRESHOLD_LONGER {
                success_wait_time *= 2;
            } else if drift > DRIFT_THRESHOLD_SHORTER {
                success_wait_time /= 2;
            }

            success_wait_time
        } else {
            NUM_SYNC_FAILS.fetch_add(1, Ordering::Relaxed);

            Duration::from_secs(60 * 60 * 1)
        };

        NEXT_SYNC_S.store(
            (Instant::now() + next_sync).as_secs().try_into().unwrap(),
            Ordering::Release,
        );

        Timer::after(next_sync).await;
    }
}

async fn sync_clock(gps: &mut gps::GPS<'_>, timeout: Duration) -> Result<(), ()> {
    let give_up = Instant::now() + timeout;

    let res = gps
        .with_messages(|msg| {
            if give_up < Instant::now() {
                return ControlFlow::Break(Err(()));
            }
            match msg {
                gps::Message::Casic(c) => match c.parse() {
                    gps::CasicMsg::NavTimeUTC(c) => {
                        defmt::println!("GPS nav: {:?}", c);
                        if let Ok(time) = reconstruct_time(c) {
                            let now = Instant::now();
                            return ControlFlow::Break(Ok((now, time)));
                        }
                    }
                    gps::CasicMsg::Unknown(c) => {
                        defmt::println!("GPS CASIC: {:?}, {:?}", c.id, c.payload);
                    }
                },
                gps::Message::Nmea(s) => {
                    let s = core::str::from_utf8(s).unwrap();
                    defmt::println!("NMEA: {}", s);
                }
            }

            ControlFlow::Continue(())
        })
        .await;
    if let Ok((now, time)) = res {
        update_clock_info(now, time);
        Ok(())
    } else {
        Err(())
    }
}

fn reconstruct_time(data: gps::NavTimeUTC) -> Result<chrono::DateTime<chrono::Utc>, ()> {
    if data.valid == 0 {
        return Err(());
    }

    let time = chrono::NaiveTime::from_hms_opt(data.hour.into(), data.min.into(), data.sec.into())
        .unwrap();

    if data.date_valid != 0 {
        let date =
            chrono::NaiveDate::from_ymd_opt(data.year.into(), data.month.into(), data.day.into())
                .unwrap();

        Ok(date.and_time(time).and_utc())
    } else if let Some(current_dt) = now_utc() {
        util::resync_time(current_dt, time)
    } else {
        Err(())
    }
}

#[derive(Copy, Clone)]
pub struct ClockInfo {
    pub scale: ClockScale,
    pub offset_s: u64,
    pub last_sync: Instant,
    pub last_sync_time: chrono::DateTime<chrono::Utc>,
}

impl ClockInfo {
    fn valid_last_sync(&self) -> bool {
        self.last_sync != Instant::from_secs(0)
    }

    fn to_utc(&self, instant: Instant) -> Option<chrono::DateTime<chrono::Utc>> {
        let offset = self.offset_s;
        if offset == 0 {
            return None;
        }

        let boot_seconds = instant.as_secs();
        let boot_seconds = self.scale.apply(boot_seconds as i32) as u64;

        let unix_seconds = CONST_UTC_OFFSET_S + offset as u64 + boot_seconds;
        chrono::DateTime::from_timestamp(unix_seconds as i64, 0)
    }

    fn to_instant<Tz: chrono::TimeZone>(&self, t: chrono::DateTime<Tz>) -> Option<Instant> {
        let utc = t.naive_utc();
        let unix_seconds = utc.timestamp();

        let offset = self.offset_s;
        if offset == 0 {
            return None;
        }

        let boot_seconds_raw = unix_seconds - CONST_UTC_OFFSET_S as i64 - offset as i64;
        let boot_seconds = self.scale.inverse().apply(boot_seconds_raw as i32);

        Some(Instant::from_secs(boot_seconds as u64))
    }
}

static CLOCK_INFO: Mutex<CriticalSectionRawMutex, RefCell<ClockInfo>> =
    Mutex::new(RefCell::new(ClockInfo {
        scale: ClockScale::one(),
        offset_s: 0,
        last_sync: Instant::from_secs(0),
        last_sync_time: chrono::DateTime::UNIX_EPOCH,
    }));

fn update_clock_info(boot_time_now: Instant, datetime: chrono::DateTime<chrono::Utc>) {
    let unix_seconds = datetime.timestamp() as u64;

    CLOCK_INFO.lock(|info| {
        let mut info = info.borrow_mut();

        if info.valid_last_sync() {
            LAST_DRIFT_S.store(
                (datetime.timestamp() - info.to_utc(boot_time_now).unwrap().timestamp()) as i32,
                Ordering::Release,
            );

            let clock_time = (boot_time_now - info.last_sync).as_secs();
            let real_time = (datetime - info.last_sync_time).num_seconds();

            info.scale = ClockScale::new(real_time as i32, clock_time as i32);
        }

        let boot_seconds = boot_time_now.as_secs();
        let boot_seconds = info.scale.apply(boot_seconds as i32) as u64;

        let offset = unix_seconds - CONST_UTC_OFFSET_S - boot_seconds;
        info.offset_s = offset;

        info.last_sync_time = datetime;
        info.last_sync = boot_time_now;
    });
}

static TZ_SECONDS_EAST: AtomicI32 = AtomicI32::new(1 * 60 * 60); //TODO: actually sync this
const CONST_UTC_OFFSET_S: u64 = 1u64 << 30;

static LAST_SYNC_TS_S: AtomicU32 = AtomicU32::new(0);
pub fn time_since_last_sync() -> Duration {
    let last_sync_ts = Instant::from_secs(LAST_SYNC_TS_S.load(Ordering::Relaxed) as _);
    last_sync_ts.elapsed()
}
static LAST_SYNC_DURATION_S: AtomicU32 = AtomicU32::new(0);
pub fn last_sync_duration() -> Duration {
    Duration::from_secs(LAST_SYNC_DURATION_S.load(Ordering::Relaxed) as _)
}
static NUM_SYNC_FAILS: AtomicU32 = AtomicU32::new(0);
pub fn num_sync_fails() -> u32 {
    NUM_SYNC_FAILS.load(Ordering::Relaxed)
}
static LAST_DRIFT_S: AtomicI32 = AtomicI32::new(0);
pub fn last_drift_s() -> i32 {
    LAST_DRIFT_S.load(Ordering::Relaxed)
}
static NEXT_SYNC_S: AtomicU32 = AtomicU32::new(0);
pub fn next_sync() -> Instant {
    Instant::from_secs(NEXT_SYNC_S.load(Ordering::Relaxed) as _)
}

pub fn clock_info() -> ClockInfo {
    CLOCK_INFO.lock(|i| i.borrow().clone())
}

pub fn now_utc() -> Option<chrono::DateTime<chrono::Utc>> {
    CLOCK_INFO.lock(|info| info.borrow().to_utc(Instant::now()))
}

pub fn now_local() -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let now = now_utc()?;
    let offset = chrono::FixedOffset::east_opt(TZ_SECONDS_EAST.load(Ordering::Relaxed)).unwrap();
    Some(now.with_timezone(&offset))
}

pub fn to_instant<Tz: chrono::TimeZone>(t: chrono::DateTime<Tz>) -> Option<Instant> {
    CLOCK_INFO.lock(|info| info.borrow().to_instant(t))
}
