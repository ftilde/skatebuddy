/*

FIR filter designed with
http://t-filter.appspot.com

sampling frequency: 25 Hz

fixed point precision: 10 bits

* 0.1 Hz - 0.6 Hz
  gain = 0
  desired attenuation = -32.07 dB
  actual attenuation = n/a

* 0.9 Hz - 3.5 Hz
  gain = 1
  desired ripple = 5 dB
  actual ripple = n/a

* 4 Hz - 12.5 Hz
  gain = 0
  desired attenuation = -20 dB
  actual attenuation = n/a

*/

const FILTER_SIZE: usize = 67;
const FILTER_VALS: [i8; FILTER_SIZE] = [
    -30, -12, -5, 5, 12, 13, 9, 3, 0, 4, 10, 14, 12, 5, -3, -5, 0, 7, 9, 1, -12, -22, -23, -13, -2,
    -1, -17, -42, -60, -51, -12, 47, 99, 120, 99, 47, -12, -51, -60, -42, -17, -1, -2, -13, -23,
    -22, -12, 1, 9, 7, 0, -5, -3, 5, 12, 14, 10, 4, 0, 3, 9, 13, 12, 5, -5, -12, -30,
];

pub struct HrmFilter {
    history: [u16; FILTER_SIZE],
    next_pos: usize,
}

fn scalar_product(v1: &[i8], v2: &[u16]) -> i32 {
    assert_eq!(v1.len(), v2.len());

    let mut sum = 0;
    for (l, r) in v1.iter().zip(v2.iter()) {
        sum += (*l as i32) * (*r as i32);
    }
    sum
}

impl HrmFilter {
    pub fn new() -> Self {
        Self {
            history: [0; FILTER_SIZE],
            next_pos: 0,
        }
    }

    pub fn filter(&mut self, val: u16) -> i32 {
        let newest_pos = self.next_pos;
        let oldest_pos = newest_pos + 1;
        self.history[newest_pos] = val;

        let begin_sum = scalar_product(
            &FILTER_VALS[..self.history.len() - oldest_pos],
            &self.history[oldest_pos..],
        );
        let end_sum = scalar_product(
            &FILTER_VALS[self.history.len() - oldest_pos..],
            &self.history[..oldest_pos],
        );
        let out = begin_sum + end_sum;

        self.next_pos = oldest_pos % self.history.len();
        out
    }
}

#[derive(Copy, Clone)]
enum BeatRegion {
    Above,
    Below,
}

pub struct HeartbeatDetector {
    filter_state: HrmFilter,
    region: BeatRegion,
    sample_count: usize,
    last_beat_sample: usize,
    min_since_cross: i32,
    min_sample: usize,
}

impl Default for HeartbeatDetector {
    fn default() -> Self {
        HeartbeatDetector {
            filter_state: HrmFilter::new(),
            region: BeatRegion::Below,
            sample_count: 0,
            last_beat_sample: 0,
            min_since_cross: i32::MAX,
            min_sample: 0,
        }
    }
}

pub struct BPM(pub u16);

impl HeartbeatDetector {
    pub fn add_sample(&mut self, s: u16) -> (f32, Option<BPM>) {
        let filtered = self.filter_state.filter(s);
        self.sample_count += 1;
        let bpm = if self.sample_count > FILTER_SIZE {
            if filtered < self.min_since_cross {
                self.min_since_cross = filtered;
                self.min_sample = self.sample_count;
            }

            match (self.region, filtered > 0) {
                (BeatRegion::Above, false) => {
                    self.region = BeatRegion::Below;
                    None
                }
                (BeatRegion::Below, true) => {
                    let samples_since_last_beat = self.min_sample - self.last_beat_sample;
                    self.last_beat_sample = self.min_sample;
                    self.min_since_cross = i32::MAX;

                    let beat_duration_millis = samples_since_last_beat * 40 /* 40ms = 1/25Hz */;
                    let bpm = ((60 * 1000) / beat_duration_millis) as u16;

                    self.region = BeatRegion::Above;
                    Some(BPM(bpm))
                }
                _ => None,
            }
        } else {
            None
        };

        (filtered as f32, bpm)
    }
}
