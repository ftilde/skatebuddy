#![cfg_attr(not(test), no_std)]

pub mod gps;

pub fn resync_time(
    base: chrono::DateTime<chrono::Utc>,
    time: chrono::NaiveTime,
) -> Result<chrono::DateTime<chrono::Utc>, ()> {
    let mut n_fixed = base.date_naive().and_time(time).and_utc();

    let an_hour = chrono::Duration::hours(1);
    let a_day = chrono::Duration::days(1);

    let ahead = n_fixed.signed_duration_since(base);
    if a_day - an_hour < ahead && ahead < a_day + an_hour {
        // Roughly a day ahead
        n_fixed -= a_day;
    }

    if a_day - an_hour < -ahead && -ahead < a_day + an_hour {
        // Roughly a day behind
        n_fixed += a_day;
    }

    let diff = n_fixed.signed_duration_since(base).abs();
    if diff < an_hour {
        Ok(n_fixed)
    } else {
        Err(())
    }
}

#[derive(Copy, Clone)]
pub struct ClockScale {
    pub numerator: i32,
    pub denominator: i32,
}

impl ClockScale {
    pub const fn one() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }
    pub const fn new(real_time: i32, clock_time: i32) -> Self {
        Self {
            numerator: real_time,
            denominator: clock_time,
        }
    }

    pub fn apply(&self, time: i32) -> i32 {
        let dt = self.numerator - self.denominator;
        time + time * dt / self.denominator
    }

    pub fn inverse(&self) -> Self {
        Self {
            numerator: self.denominator,
            denominator: self.numerator,
        }
    }
}

pub struct RingBuffer<const N: usize, T> {
    ring_buffer: [T; N],
    next: usize,
    num_total: usize,
}

impl<const N: usize, T: Default> Default for RingBuffer<N, T> {
    fn default() -> Self {
        Self {
            ring_buffer: core::array::from_fn(|_| Default::default()),
            next: 0,
            num_total: 0,
        }
    }
}
impl<const N: usize, T> RingBuffer<N, T> {
    pub fn add(&mut self, mut v: T) -> T {
        self.num_total += 1;

        core::mem::swap(&mut self.ring_buffer[self.next], &mut v);
        self.next = (self.next + 1) % N;

        v
    }
    pub fn past_value(&self, diff: usize) -> &T {
        let i = (self.next + N).checked_sub(diff).unwrap() % N;
        &self.ring_buffer[i]
    }

    pub fn inner(&self) -> &[T; N] {
        &self.ring_buffer
    }
    pub fn num_valid(&self) -> usize {
        self.num_total.min(N)
    }
    pub fn valid_values(&self) -> &[T] {
        &self.ring_buffer[..self.num_total.min(N)]
    }

    pub fn is_full(&self) -> bool {
        self.num_valid() == N
    }

    pub fn next(&self) -> usize {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn run_test_resync_time(i: &str, h: u32, m: u32, s: u32, o: Result<&str, ()>) {
        assert_eq!(
            resync_time(
                chrono::DateTime::parse_from_rfc3339(i).unwrap().into(),
                chrono::NaiveTime::from_hms_opt(h, m, s).unwrap(),
            ),
            o.map(|o| chrono::DateTime::parse_from_rfc3339(o).unwrap().into())
        );
    }
    #[test]
    fn test_resync_time() {
        // No wrap
        run_test_resync_time("2023-01-10T00:00:01Z", 0, 0, 0, Ok("2023-01-10T00:00:00Z"));

        // Wrap backwards
        run_test_resync_time("2023-01-10T00:00:00Z", 23, 4, 2, Ok("2023-01-09T23:04:02Z"));

        // Wrap forwards
        run_test_resync_time("2023-01-10T23:55:02Z", 0, 4, 5, Ok("2023-01-11T00:04:05Z"));

        // Too far
        run_test_resync_time("2023-01-10T23:55:02Z", 1, 4, 5, Err(()));
        run_test_resync_time("2023-01-10T23:55:02Z", 20, 4, 5, Err(()));
        run_test_resync_time("2023-01-10T00:59:02Z", 23, 55, 17, Err(()));
    }
}
