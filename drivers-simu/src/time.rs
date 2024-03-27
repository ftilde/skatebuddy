pub use std::time::{Duration, Instant};

use once_cell::sync::Lazy;
use util::ClockScale;

pub static BOOT: Lazy<Instant> = Lazy::new(|| Instant::now());

pub fn time_since_last_sync() -> Duration {
    BOOT.elapsed()
}
pub fn last_sync_duration() -> Duration {
    Duration::from_secs(1)
}
pub fn num_sync_fails() -> u32 {
    0
}
pub fn last_drift_s() -> i32 {
    0
}

pub fn next_sync() -> Instant {
    Instant::now()
}

#[derive(Copy, Clone)]
pub struct ClockInfo {
    pub scale: ClockScale,
    pub offset_s: u64,
    pub last_sync: Instant,
    pub last_sync_time: chrono::DateTime<chrono::Utc>,
}

pub fn clock_info() -> ClockInfo {
    ClockInfo {
        scale: ClockScale::one(),
        offset_s: 0,
        last_sync: Instant::now(),
        last_sync_time: chrono::DateTime::UNIX_EPOCH,
    }
}

pub fn now_utc() -> Option<chrono::DateTime<chrono::Utc>> {
    Some(chrono::Utc::now())
}

pub fn now_local() -> Option<chrono::DateTime<chrono::FixedOffset>> {
    Some(chrono::Local::now().into())
}

pub fn to_instant<Tz: chrono::TimeZone>(t: chrono::DateTime<Tz>) -> Option<Instant> {
    let utc = t.to_utc();
    let now_utc = now_utc().unwrap();
    let now = Instant::now();
    let diff = utc - now_utc;

    let mut neg = false;
    let diff = diff.to_std().unwrap_or_else(|_| {
        neg = true;
        diff.abs().to_std().unwrap()
    });

    Some(if neg { now - diff } else { now + diff })
}

pub use smol::Timer;

pub struct Ticker {
    duration: Duration,
    next: Instant,
}

impl Ticker {
    pub fn every(d: Duration) -> Ticker {
        Self {
            duration: d,
            next: Instant::now() + d,
        }
    }

    pub async fn next(&mut self) {
        Timer::at(self.next).await;
        self.next += self.duration;
    }
}
