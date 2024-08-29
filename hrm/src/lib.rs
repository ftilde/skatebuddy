#![no_std]

use micromath::F32Ext;
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

pub struct ExpMean {
    acc: f32,
    alpha: f32,
}
impl ExpMean {
    pub fn new(alpha: f32) -> Self {
        Self { acc: 0.0, alpha }
    }
    pub fn add(&mut self, v: f32) -> f32 {
        let res = self.acc * self.alpha + v * (1.0 - self.alpha);
        self.acc = res;
        res
    }
    pub fn get(&self) -> f32 {
        self.acc
    }
}

//const GRADIENT_CLIP_MIN_VALUES: usize = 128;
const GRADIENT_CLIP_CLIP_MULT: i16 = 3;
pub struct GradientClip {
    //ring_buffer: RingBuffer<GRADIENT_CLIP_MIN_VALUES, i16>,
    //sum: i32,
    mean: ExpMean,
    prev_in: i16,
    prev_out: i16,
}

impl Default for GradientClip {
    fn default() -> Self {
        Self {
            mean: ExpMean::new(0.99),
            prev_in: 0,
            prev_out: 0,
        }
    }
}

impl GradientClip {
    pub fn add_value(&mut self, v: i16) -> i16 {
        //let mean = (self.sum / GRADIENT_CLIP_MIN_VALUES as i32).max(1) as i16;
        let mean = self.mean.get() as i16;
        let diff = self.prev_in - v;
        self.prev_in = v;
        let diff_abs_orig = diff.abs();

        //let old = self.ring_buffer.add(diff_abs_orig);
        //self.sum -= old as i32;
        //self.sum += diff_abs_orig as i32;
        self.mean.add(diff_abs_orig as f32);

        let abs_max = mean * GRADIENT_CLIP_CLIP_MULT;
        let diff_abs = diff_abs_orig.min(abs_max);
        let diff = diff.signum() * diff_abs;

        let out = self.prev_out - diff;
        self.prev_out = out;
        out
    }
}

const SAMPLE_DELAY_MS: usize = 40;

pub struct FFTEstimator {
    samples: RingBuffer<NUM_FFT_SAMPLES, f32>,
}
impl Default for FFTEstimator {
    fn default() -> Self {
        Self {
            samples: Default::default(),
        }
    }
}

const NUM_FFT_SAMPLES: usize = 256;
const BASE_FREQ_INDEX: usize = bpm_to_fft_index(45);
const SPECTRUM_SIZE: usize = bpm_to_fft_index(230) - BASE_FREQ_INDEX;
const fn bpm_to_fft_index(bpm: usize) -> usize {
    (bpm * NUM_FFT_SAMPLES * SAMPLE_DELAY_MS) / (60 * 1000)
}
//fn bpm_to_index(bpm: f32) -> usize {
//    ((bpm * NUM_FFT_SAMPLES as f32 * SAMPLE_DELAY_MS as f32) / (60.0 * 1000.0)).round() as usize
//}
const fn index_to_bpm(index: usize) -> f32 {
    ((index + BASE_FREQ_INDEX) * 60 * 1000 / (NUM_FFT_SAMPLES * SAMPLE_DELAY_MS)) as f32
}
pub type Spectrum = [f32; SPECTRUM_SIZE];

pub fn spectrum_freqs(s: Spectrum) -> [(f32, f32); SPECTRUM_SIZE] {
    core::array::from_fn(|i| (index_to_bpm(i), s[i]))
}
pub fn normalize_spectrum_max(s: Spectrum) -> Spectrum {
    let max = s.iter().max_by(|l, r| l.total_cmp(r)).unwrap();
    if *max > 0.0 {
        core::array::from_fn(|i| s[i] / max)
    } else {
        s
    }
}
pub fn normalize_spectrum_sum(s: Spectrum) -> Spectrum {
    //let sum_sq: f32 = s.iter().map(|v| if *v < 0.0 { -*v } else { *v }).sum();
    let sum_sq: f32 = s.iter().map(|v| v * v).sum();
    if sum_sq > 0.0 {
        let n = 1.0 / sum_sq.sqrt();
        core::array::from_fn(|i| s[i] * n)
    } else {
        s
    }
}

pub fn max_bpm(spectrum: Spectrum) -> BPM {
    let spectrum_index = spectrum
        .iter()
        .enumerate()
        .max_by(|l, r| l.1.total_cmp(&r.1))
        .unwrap()
        .0;

    let max_bpm = index_to_bpm(spectrum_index);
    BPM(max_bpm as _)
}

impl FFTEstimator {
    pub fn add_sample(&mut self, sample: f32) -> Option<Spectrum> {
        self.samples.add(sample);
        if self.samples.num_valid() == NUM_FFT_SAMPLES {
            let mut samples = core::array::from_fn(|i| self.samples.inner()[i]);
            let spectrum = microfft::real::rfft_256(&mut samples);
            let spectrum: Spectrum =
                core::array::from_fn(|i| spectrum[i + BASE_FREQ_INDEX].l1_norm());
            Some(spectrum)
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

fn kernel_smooth(spectrum: Spectrum) -> Spectrum {
    core::array::from_fn(|i| {
        let mut s = 100.0 * spectrum[i];
        if i > 0 {
            s += spectrum[i - 1];
        }
        if i < spectrum.len() - 1 {
            s += spectrum[i + 1];
        }
        s /= 120.0;
        s
    })
}

impl SpectrumSmoother {
    pub fn add(&mut self, spectrum: Spectrum) -> Spectrum {
        let l = self.spectrum_agg;
        let r = normalize_spectrum_sum(spectrum);
        self.spectrum_agg = core::array::from_fn(|i| {
            let alpha = 0.99;
            l[i] * alpha + r[i] * (1.0 - alpha)
        });
        //self.spectrum_agg
        kernel_smooth(self.spectrum_agg)
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
