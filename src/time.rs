use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use embassy_time::{Duration, Instant, Timer};

use crate::gps;

#[embassy_executor::task]
pub async fn clock_sync_task(mut gps: gps::GPSRessources) {
    const INITIAL_SYNC_TIME: Duration = Duration::from_secs(15 * 60);
    const INCREMENTAL_SYNC_TIME: Duration = Duration::from_secs(2 * 60);

    let mut sync_time = INITIAL_SYNC_TIME;

    loop {
        let before_sync = Instant::now();
        let res = {
            let mut gps = gps.on();
            sync_clock(&mut gps, sync_time).await
        };
        let next_sync = if res.is_ok() {
            let sync_duration = before_sync.elapsed();

            LAST_SYNC_DURATION_S.store(sync_duration.as_secs() as _, Ordering::Relaxed);
            LAST_SYNC_TS_S.store(Instant::now().as_secs() as _, Ordering::Relaxed);

            sync_time = INCREMENTAL_SYNC_TIME;

            Duration::from_secs(60 * 60 * 8)
        } else {
            NUM_SYNC_FAILS.fetch_add(1, Ordering::Relaxed);

            Duration::from_secs(60 * 60 * 1)
        };
        Timer::after(next_sync).await;
    }
}

async fn sync_clock(gps: &mut gps::GPS<'_>, timeout: Duration) -> Result<(), ()> {
    let mut buf = [0u8; 128];
    let mut end = 0;
    let mut done = false;
    let give_up = Instant::now() + timeout;
    while !done {
        if give_up < Instant::now() {
            return Err(());
        }
        let n_read = gps.read(&mut buf[end..]).await;
        if n_read == 1 && buf[end] == 0xff {
            continue;
        }
        let mut read_end = end + n_read;
        while let Some(newline) = buf[end..read_end].iter().position(|b| *b == b'\n') {
            let after_newline = end + newline + 1;
            let line = &buf[..after_newline];
            let s = core::str::from_utf8(line).unwrap();
            defmt::println!("GPS: {}", s);
            //state.parse(s).unwrap();
            let nmea = nmea::parse_bytes(line);
            match nmea {
                Ok(nmea::ParseResult::ZDA(d)) => {
                    if set_utc_offset(d).is_ok() {
                        done = true;
                    }
                }
                _ => {}
            }

            buf.copy_within(after_newline..read_end, 0);
            end = 0;
            read_end = read_end - after_newline;
        }
        end = read_end;
    }
    Ok(())
}

fn set_utc_offset(data: nmea::sentences::ZdaData) -> Result<(), ()> {
    if let Some(d) = data.utc_date_time() {
        //let d = chrono::DateTime::from_naive_utc_and_offset(d, chrono::Utc);
        let unix_seconds = d.timestamp() as u64;

        let boot_time = Instant::now();
        let boot_seconds = boot_time.as_secs();

        let offset = unix_seconds - CONST_UTC_OFFSET_S - boot_seconds;
        OFFSET_UTC_S.store(offset.try_into().unwrap(), Ordering::Relaxed);
        Ok(())
    } else {
        Err(())
    }
}

static OFFSET_UTC_S: AtomicU32 = AtomicU32::new(0);
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

pub fn now_utc() -> Option<chrono::DateTime<chrono::Utc>> {
    let offset = OFFSET_UTC_S.load(Ordering::Relaxed);
    if offset == 0 {
        return None;
    }

    let boot_time = Instant::now();
    let boot_seconds = boot_time.as_secs();

    let unix_seconds = CONST_UTC_OFFSET_S + offset as u64 + boot_seconds;
    chrono::DateTime::from_timestamp(unix_seconds as i64, 0)
}

pub fn now_local() -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let now = now_utc()?;
    let offset = chrono::FixedOffset::east_opt(TZ_SECONDS_EAST.load(Ordering::Relaxed)).unwrap();
    Some(now.with_timezone(&offset))
}
