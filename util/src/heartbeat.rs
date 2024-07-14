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

pub struct HeartbeatDetector {
    filter_state: HrmFilter,
    region: BeatRegion,
    sample_count: usize,
    last_beat_sample: usize,
    min_since_cross: f32,
    min_sample: usize,
}

impl Default for HeartbeatDetector {
    fn default() -> Self {
        HeartbeatDetector {
            filter_state: HrmFilter::new(),
            region: BeatRegion::Below,
            sample_count: 0,
            last_beat_sample: 0,
            min_since_cross: f32::MAX,
            min_sample: 0,
        }
    }
}

pub struct BPM(pub u16);

impl HeartbeatDetector {
    pub fn num_samples(&self) -> usize {
        self.sample_count
    }
    pub fn add_sample(&mut self, s: u16) -> (f32, Option<BPM>) {
        let filtered = self.filter_state.filter(s);
        self.sample_count += 1;
        let bpm = if self.sample_count >= 0 {
            if filtered < self.min_since_cross {
                self.min_since_cross = filtered;
                self.min_sample = self.sample_count;
            }

            match (self.region, filtered > 0.0) {
                (BeatRegion::Above, false) => {
                    self.region = BeatRegion::Below;
                    None
                }
                (BeatRegion::Below, true) => {
                    let samples_since_last_beat = self.min_sample - self.last_beat_sample;
                    self.last_beat_sample = self.min_sample;
                    self.min_since_cross = f32::MAX;

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
