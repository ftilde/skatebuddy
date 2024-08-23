#![no_std]

use util::RingBuffer;

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
    history: [i16; FILTER_SIZE],
    next_pos: usize,
}

fn scalar_product(v1: &[i8], v2: &[i16]) -> i32 {
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

    pub fn filter(&mut self, val: i16) -> i32 {
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
//
//pub struct HrmFilter {
//    inner: biquad::DirectForm2Transposed<f32>,
//}
//
//impl HrmFilter {
//    pub fn new() -> Self {
//        use biquad::*;
//        let fs = 25.hz();
//        let f0 = 2.hz();
//
//        let coefficients =
//            Coefficients::<f32>::from_params(biquad::Type::BandPass, fs, f0, Q_BUTTERWORTH_F32)
//                .unwrap();
//        Self {
//            inner: DirectForm2Transposed::<f32>::new(coefficients),
//        }
//    }
//
//    pub fn filter(&mut self, val: i16) -> f32 {
//        use biquad::*;
//        self.inner.run(val as f32)
//    }
//}
//

const SAMPLE_DELAY_MS: usize = 40;

pub struct FFTEstimator {
    samples: [f32; NUM_FFT_SAMPLES],
    next_sample_pos: u16,
}
impl Default for FFTEstimator {
    fn default() -> Self {
        Self {
            samples: core::array::from_fn(|_| 0.0),
            next_sample_pos: 0,
        }
    }
}

const NUM_FFT_SAMPLES: usize = 128;
const BASE_FREQ_INDEX: usize = bpm_to_index(45);
const SPECTRUM_SIZE: usize = bpm_to_index(230) - BASE_FREQ_INDEX;
const fn bpm_to_index(bpm: usize) -> usize {
    (bpm * NUM_FFT_SAMPLES * SAMPLE_DELAY_MS) / (60 * 1000)
}
const fn index_to_bpm(index: usize) -> f32 {
    (index * 60 * 1000 / (NUM_FFT_SAMPLES * SAMPLE_DELAY_MS)) as f32
}
pub type Spectrum = [f32; SPECTRUM_SIZE];

pub fn spectrum_freqs(s: Spectrum) -> [(f32, f32); SPECTRUM_SIZE] {
    core::array::from_fn(|i| (index_to_bpm(i + BASE_FREQ_INDEX), s[i]))
}
pub fn normalize_spectrum(s: Spectrum) -> Spectrum {
    let max = s.iter().max_by(|l, r| l.total_cmp(r)).unwrap();
    core::array::from_fn(|i| s[i] / max)
}

pub fn max_bpm(spectrum: Spectrum) -> BPM {
    let spectrum_index = spectrum
        .iter()
        .enumerate()
        .max_by(|l, r| l.1.total_cmp(&r.1))
        .unwrap()
        .0;

    let max_bpm = index_to_bpm(spectrum_index + BASE_FREQ_INDEX);
    BPM(max_bpm as _)
}

impl FFTEstimator {
    pub fn add_sample(&mut self, sample: f32) -> Option<Spectrum> {
        self.samples[self.next_sample_pos as usize] = sample;
        self.next_sample_pos += 1;
        if self.next_sample_pos as usize == self.samples.len() {
            self.next_sample_pos = 0;
            let spectrum = microfft::real::rfft_128(&mut self.samples);
            let spectrum: Spectrum =
                core::array::from_fn(|i| spectrum[i + BASE_FREQ_INDEX].l1_norm());
            Some(normalize_spectrum(spectrum))
        } else {
            None
        }
    }
}

pub struct SpectrumSmoother {
    spectrum_agg: Spectrum,
}
impl Default for SpectrumSmoother {
    fn default() -> Self {
        Self {
            spectrum_agg: core::array::from_fn(|_| 0.0),
        }
    }
}

impl SpectrumSmoother {
    pub fn add(&mut self, spectrum: Spectrum) -> Spectrum {
        self.spectrum_agg = normalize_spectrum(core::array::from_fn(|i| {
            let mut s = 2.0 * self.spectrum_agg[i];
            if i > 0 {
                s += self.spectrum_agg[i - 1];
            }
            if i < self.spectrum_agg.len() - 1 {
                s += self.spectrum_agg[i + 1];
            }
            s /= 4.0;

            let alpha = 0.5;
            s * alpha + spectrum[i] * (1.0 - alpha)
        }));
        self.spectrum_agg
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

pub struct HeartbeatDetector<E> {
    filter_state: HrmFilter,
    outlier_filter: OutlierFilter,
    region: BeatRegion,
    sample_count: usize,
    last_beat_sample: usize,
    min_since_cross: i32,
    min_sample: usize,
    sr_estimator: E,
}

impl<E> HeartbeatDetector<E> {
    pub fn new(sr_estimator: E) -> Self {
        HeartbeatDetector {
            filter_state: HrmFilter::new(),
            outlier_filter: Default::default(),
            region: BeatRegion::Below,
            sample_count: 0,
            last_beat_sample: 0,
            min_since_cross: i32::MAX,
            min_sample: 0,
            sr_estimator,
        }
    }
}

pub trait EstimateSampleRate {
    fn note_sample(&mut self);
    fn millis_per_sample(&self) -> f32;
}

pub struct UncalibratedEstimator;
impl EstimateSampleRate for UncalibratedEstimator {
    fn note_sample(&mut self) {}

    fn millis_per_sample(&self) -> f32 {
        40.0
    }
}

const MIN_BPM: u16 = 30;
const MAX_BPM: u16 = 230;

pub struct BPM(pub u16);

impl<E: EstimateSampleRate> HeartbeatDetector<E> {
    pub fn millis_per_sample(&mut self) -> f32 {
        self.sr_estimator.millis_per_sample()
    }
    pub fn add_sample(&mut self, s: i16) -> (f32, Option<BPM>) {
        let filtered = self.filter_state.filter(s);
        self.sample_count += 1;
        self.sr_estimator.note_sample();
        let bpm = {
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

                    let beat_duration_millis =
                        samples_since_last_beat as f32 * self.millis_per_sample();
                    let bpm = ((60.0 * 1000.0) / beat_duration_millis) as u16;

                    self.region = BeatRegion::Above;

                    if MIN_BPM <= bpm && bpm < MAX_BPM {
                        let bpm = self.outlier_filter.filter(bpm);

                        Some(BPM(bpm))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        (filtered as f32, bpm)
    }
}
