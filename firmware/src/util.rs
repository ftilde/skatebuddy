use drivers::time::{Duration, Instant};

pub fn hours_mins_secs(d: Duration) -> (u32, u32, u32) {
    let seconds = d.as_secs();

    let sec_clock = seconds % 60;
    let minutes = seconds / 60;
    let min_clock = minutes % 60;
    let hours = minutes / 60;

    (hours as _, min_clock as _, sec_clock as _)
}

pub struct SampleCountingEstimator {
    num_samples: usize,
    start: Instant,
}
impl SampleCountingEstimator {
    pub fn new() -> Self {
        Self {
            num_samples: 0,
            start: Instant::now(),
        }
    }
}
impl hrm::EstimateSampleRate for SampleCountingEstimator {
    fn note_sample(&mut self) {
        if self.num_samples == 0 {
            self.start = Instant::now();
        }
        self.num_samples += 1;
    }

    fn millis_per_sample(&self) -> f32 {
        self.start.elapsed().as_millis() as f32 / (self.num_samples - 1) as f32
    }
}
