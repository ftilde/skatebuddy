use drivers::time::Instant;

use crate::util::RingBuffer;

pub struct HrmFilter {
    inner: biquad::DirectForm2Transposed<f32>,
}

impl HrmFilter {
    pub fn new() -> Self {
        use biquad::*;
        let fs = 25.hz();
        let f0 = 2.hz();

        let coefficients =
            Coefficients::<f32>::from_params(biquad::Type::BandPass, fs, f0, Q_BUTTERWORTH_F32)
                .unwrap();
        Self {
            inner: DirectForm2Transposed::<f32>::new(coefficients),
        }
    }

    pub fn filter(&mut self, val: u16) -> f32 {
        use biquad::*;
        self.inner.run(val as f32)
    }
}

#[derive(Copy, Clone)]
enum BeatRegion {
    Above,
    Below,
}

#[derive(Default)]
pub struct OutlierFilter {
    values: RingBuffer<7, u16>,
}

impl OutlierFilter {
    pub fn filter(&mut self, v: u16) -> u16 {
        self.values.add(v);
        let mut v = self.values.inner().clone();
        let v = &mut v[..self.values.num_valid()];
        let (_, median, _) = v.select_nth_unstable(v.len() / 2);
        *median
    }
}

pub struct HeartbeatDetector {
    filter_state: HrmFilter,
    outlier_filter: OutlierFilter,
    region: BeatRegion,
    sample_count: usize,
    last_beat_sample: usize,
    max_since_cross: f32,
    max_sample: usize,
    start: Instant,
}

impl Default for HeartbeatDetector {
    fn default() -> Self {
        HeartbeatDetector {
            filter_state: HrmFilter::new(),
            outlier_filter: Default::default(),
            region: BeatRegion::Below,
            sample_count: 0,
            last_beat_sample: 0,
            max_since_cross: f32::MIN,
            max_sample: 0,
            start: Instant::now(),
        }
    }
}

pub struct BPM(pub u16);

impl HeartbeatDetector {
    pub fn millis_per_sample(&self) -> f32 {
        self.start.elapsed().as_millis() as f32 / (self.sample_count - 1) as f32
    }
    pub fn add_sample(&mut self, s: u16) -> (f32, Option<BPM>) {
        let filtered = self.filter_state.filter(s);
        self.sample_count += 1;
        let bpm = if self.sample_count > 0 {
            if filtered > self.max_since_cross {
                self.max_since_cross = filtered;
                self.max_sample = self.sample_count;
            }

            match (self.region, filtered > 0.0) {
                (BeatRegion::Below, true) => {
                    self.region = BeatRegion::Above;
                    None
                }
                (BeatRegion::Above, false) => {
                    let samples_since_last_beat = self.max_sample - self.last_beat_sample;
                    self.last_beat_sample = self.max_sample;
                    self.max_since_cross = f32::MIN;

                    let beat_duration_millis =
                        samples_since_last_beat as f32 * self.millis_per_sample();
                    let bpm = ((60.0 * 1000.0) / beat_duration_millis) as u16;
                    let bpm = self.outlier_filter.filter(bpm);

                    self.region = BeatRegion::Below;
                    Some(BPM(bpm))
                }
                _ => None,
            }
        } else {
            self.start = Instant::now();
            None
        };

        (filtered as f32, bpm)
    }
}
