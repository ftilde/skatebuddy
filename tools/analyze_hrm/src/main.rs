//use goertzel_algorithm::OptimizedGoertzel;
use biquad::*;
use hrm::Spectrum;
//use goertzel_nostd::Parameters;
use plotpy::{Curve, Plot};
use realfft::RealFftPlanner;
use std::error::Error;

#[derive(serde::Deserialize)]
struct Row {
    val: i16,
}
#[derive(serde::Deserialize)]
struct AccelRow {
    x: i16,
    y: i16,
    z: i16,
}

fn plot_values_multiple(vals: &[&[(f32, f32)]]) -> Result<(), Box<dyn Error>> {
    let mut plot = Plot::new();
    for (i, vals) in vals.iter().enumerate() {
        let mut curve = Curve::new();
        curve.set_line_width(2.0);

        curve.points_begin();
        for (x, y) in *vals {
            curve.points_add(x, y);
        }
        curve.points_end();
        curve.set_label(&i.to_string());

        plot.add(&curve);
    }

    if let Err(e) = plot.legend().grid_and_labels("x", "y").show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}
fn plot_values(vals: &[(f32, f32)]) -> Result<(), Box<dyn Error>> {
    let mut curve = Curve::new();
    curve.set_line_width(2.0);

    curve.points_begin();
    for (x, y) in vals {
        curve.points_add(x, y);
    }
    curve.points_end();

    let mut plot = Plot::new();
    plot.add(&curve).grid_and_labels("x", "y");

    if let Err(e) = plot.show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}
//fn plot_phase_diffs(vals: Vec<Spectrum>) -> Result<(), Box<dyn Error>> {
//    let mut data = vec![Vec::new(); hrm::SPECTRUM_SIZE];
//    for spec in vals {
//        for (i, v) in spec.iter().enumerate() {
//            data[i].push(*v);
//        }
//    }
//
//    let ticks: Vec<_> = (1..(data.len() + 1)).into_iter().collect();
//    let labels = (0..hrm::SPECTRUM_SIZE)
//        .into_iter()
//        .map(hrm::index_to_bpm)
//        .collect::<Vec<_>>();
//    let mut boxes = plotpy::Boxplot::new();
//    boxes.draw(&data);
//
//    // save figure
//    let mut plot = Plot::new();
//    plot.add(&boxes).set_ticks_x_labels(&ticks, &labels);
//    //.set_title("boxplot documentation test")
//
//    if let Err(e) = plot.show("out.svg") {
//        println!("{}", e);
//    }
//
//    Ok(())
//}
fn plot_phase_diffs(vals: Vec<Spectrum>) -> Result<(), Box<dyn Error>> {
    let mut curve = Curve::new();
    curve.set_line_width(2.0);
    curve.set_line_style("None");
    curve.set_marker_style("o");
    curve.set_marker_size(1.0);

    curve.points_begin();
    for spec in vals {
        for (bpm, d) in hrm::spectrum_freqs(spec) {
            curve.points_add(bpm, d);
        }
    }
    curve.points_end();

    let mut plot = Plot::new();
    plot.add(&curve).grid_and_labels("x", "y");
    plot.set_xmin(85.0);

    if let Err(e) = plot.show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let file = std::fs::File::open(&args[1]).unwrap();
    let accel_file = std::fs::File::open(&args[2]).unwrap();
    let mut rdr = csv::Reader::from_reader(file);
    let mut accel_rdr = csv::Reader::from_reader(accel_file);

    let fs = 25.hz();
    let f0 = 1.0.hz();

    let coefficients =
        Coefficients::<f32>::from_params(biquad::Type::HighPass, fs, f0, Q_BUTTERWORTH_F32)
            .unwrap();
    //let mut filter = DirectForm2Transposed::<f32>::new(coefficients);
    let mut filter = hrm::UnbiasedBiquadHighPass::new();
    //let mut filter = hrm::KernelSampleFilter::new();
    //let mut filter = hrm::UnbiasedBiquadSampleFilter::new();

    let mut gradient_clip = hrm::GradientClip::default();
    let mut ms = 0;
    let mut vals = Vec::new();
    let mut raw_vals = Vec::new();
    let sample_delay = 40;
    //let sample_rate = 1000 / sample_delay;
    for result in rdr.records() {
        let record = result?;
        let row: Row = record.deserialize(None)?;

        let val = gradient_clip.add_value(row.val);
        //let val = row.val;
        //vals.push((ms as f32, filter.run(val as f32)));
        vals.push((ms as f32, filter.filter(val) as f32));
        raw_vals.push((ms as f32, row.val));
        ms += sample_delay;
    }

    let mut ms = 0;
    let mut accel_vals = Vec::new();
    for result in accel_rdr.records() {
        let record = result?;
        let row: AccelRow = record.deserialize(None)?;
        accel_vals.push((ms as f32, row));
        ms += sample_delay;
    }

    fn square(v: i16) -> f32 {
        v as f32 * v as f32
    }
    let accel_vals = accel_vals
        .into_iter()
        //.map(|(ms, r)| (ms, (square(r.x) + square(r.y) + square(r.z)).sqrt()))
        //.map(|(ms, r)| (ms, [r.x as f32, r.y as f32, r.z as f32]))
        .map(|(ms, r)| (ms, [r.x as f32, r.y as f32, r.z as f32]))
        .collect::<Vec<_>>();
    //plot_values(&accel_vals[..])?;

    //plot_values(&vals[50..])?;

    let step = 10;
    let low = 30;
    let high = 230;

    let freqs = (low / step..high / step)
        .map(|v| (v * step) as f32 / 60.0)
        .collect::<Vec<_>>();

    let length = 150;

    // make a planner
    let mut real_planner = RealFftPlanner::<f32>::new();

    // create a FFT
    let r2c = real_planner.plan_fft_forward(length);
    // make a dummy real-valued signal (filled with zeros)
    let mut indata = r2c.make_input_vec();
    // make a vector for storing the spectrum
    let mut spectrum = r2c.make_output_vec();

    //let mut spectrum = Vec::new();
    //for freq in &freqs {
    //let block_size = (sample_rate as f32 / freq).round() as u32;
    //let block_size = vals.len();
    //let goertzel = Parameters::new(*freq, sample_rate, block_size as _);
    //let mut p = goertzel.start();

    //for (i, v) in vals.iter().enumerate() {
    //    p = p.add_samples(std::slice::from_ref(&v.1).iter());
    //    if (i + 1) == block_size as usize {
    //        spectrum.push((*freq * 60.0, p.finish_mag() * *freq));
    //        break;
    //    }
    //}
    //let mut goertzel = OptimizedGoertzel::new();
    //goertzel.prepare(sample_rate, *freq, block_size as _);

    //for v in &vals {
    //    if let Some(v) = goertzel.process_sample(&v.1) {
    //        spectrum.push((*freq * 60.0, v));
    //        break;
    //    }
    //}

    //}

    let mut hrm_detector = hrm::ZeroCrossHeartbeatDetector::new(hrm::UncalibratedEstimator);
    //let mut freq_detector = hrm::FFTEstimator::default();
    let mut freq_detector = hrm::SparseFFTEstimator::default();
    let mut accel_freq_detectors: [_; 3] =
        std::array::from_fn(|_| hrm::SparseFFTEstimator::default());
    let mut spec_smoother = hrm::SpectrumSmoother::default();
    let mut spec_smoother_supressed = hrm::SpectrumSmoother::default();
    let mut accel_spec_smoother = hrm::SpectrumSmoother::default();
    let mut bpm_vals = Vec::new();
    let mut bpm_vals_freq = Vec::new();
    let mut bpm_vals_freq_smooth = Vec::new();
    let mut bpm_vals_baseline = Vec::new();
    let mut bpm_vals_accel_supressed = Vec::new();
    let mut bpm_vals_accel_supressed2 = Vec::new();
    let mut accel_vals_freq = Vec::new();
    let mut accel_vals_freq_smooth = Vec::new();

    let mut baseline_filter = hrm::HeartbeatDetector::new(hrm::UncalibratedEstimator);

    let mut window = std::collections::VecDeque::new();
    let mut prev = 0.0;
    let mut biased_filter = hrm::BiasedSampleFilter::new();

    let mut accel_spectrum = None;
    let mut accel_spectrum_smooth = None;

    let mut phase_diffs = Vec::new();
    for (j, (((i, v), (ir, vr)), (act, acv))) in vals
        .iter()
        .zip(raw_vals.iter())
        .zip(accel_vals.iter())
        .enumerate()
    {
        window.push_back((*ir, *vr as f32));
        //window.push_back((*ir, (*vr as f32 - prev).abs()));
        let filtered = biased_filter.filter(*vr);

        if let Some(bpm) = hrm_detector.add_sample(filtered) {
            bpm_vals.push((*i, bpm.0 as f32));
        }

        //window.push_back((*i, filtered));
        prev = *vr as f32;
        if window.len() == 512 {
            window.pop_front();
        }
        if let [Some(spec_x), Some(spec_y), Some(spec_z)] = std::array::from_fn(|i| {
            accel_freq_detectors[i]
                .add_sample(acv[i])
                .map(hrm::spectrum_norm)
        }) {
            let spectrum_accel: Spectrum =
                //std::array::from_fn(|i| spec_x[i].max(spec_y[i]).max(spec_z[i]));
                //std::array::from_fn(|i| spec_y[i].max(spec_z[i]));
                std::array::from_fn(|i| spec_y[i] + spec_z[i]);
            //std::array::from_fn(|i| spec_z[i]);

            accel_spectrum = Some(spectrum_accel);
            let spectrum = accel_spec_smoother.add(spectrum_accel);
            //dbg!(spectrum_accel.iter().cloned().fold(0.0, f32::max));
            accel_spectrum_smooth = Some(spectrum);

            let bpm = hrm::max_bpm(spectrum_accel);
            let bpm_smooth = hrm::max_bpm(spectrum);

            accel_vals_freq.push((*act, bpm.0 as f32));
            accel_vals_freq_smooth.push((*act, bpm_smooth.0 as f32));

            let spectrum = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum));
            let spectrum_orig = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum_accel));

            if j % 512 == 0 {
                //plot_values_multiple(&[&spectrum_orig, &spectrum])?;
                //plot_values(&window.iter().cloned().collect::<Vec<_>>())?;
            }
        }
        if let Some(spectrum_complex) = freq_detector.add_sample(*v as _) {
            //phase_diffs.clear();
            phase_diffs.push(hrm::harmonic_phase_diff(spectrum_complex));
            //let spectrum_orig = hrm::hrm_enhance(spectrum_orig);
            let spectrum_orig = hrm::spectrum_norm(spectrum_complex);
            let spectrum = spec_smoother.add(spectrum_orig);

            let bpm = hrm::max_bpm(spectrum_orig);
            let bpm_smooth = hrm::max_bpm(spectrum);
            let bpm_suppressed = if let Some(accel_spectrum) = accel_spectrum_smooth {
                let suppressed_spectrum = hrm::suppress_from(spectrum, accel_spectrum);

                if j % 512 == 0 {
                    //plot_values_multiple(&[
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum)),
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(accel_spectrum)),
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(suppressed_spectrum)),
                    //])?;
                    //plot_values(&window.iter().cloned().collect::<Vec<_>>())?;
                }

                hrm::max_bpm(suppressed_spectrum)
            } else {
                bpm_smooth
            };
            let bpm_suppressed2 = if let Some(accel_spectrum) = accel_spectrum {
                let suppressed_spectrum = hrm::suppress_complex(spectrum_complex, accel_spectrum);
                let spectrum = hrm::hrm_enhance(suppressed_spectrum);
                let spectrum_smooth = spec_smoother_supressed.add(spectrum);

                if j % 150 == 0 {
                    //plot_values_multiple(&[
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum)),
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(accel_spectrum)),
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum_smooth)),
                    //    &hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum)),
                    //])?;
                    //plot_values(&window.iter().cloned().collect::<Vec<_>>())?;
                }

                if j % 50 == 0 {
                    TODO: proper amplitude based accel filter
                    plot_values_multiple(&[
                        &hrm::spectrum_freqs(hrm::normalize_spectrum_max(hrm::spectrum_norm(
                            suppressed_spectrum,
                        ))),
                        &hrm::spectrum_freqs(hrm::normalize_spectrum_max(accel_spectrum)),
                        &hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum)),
                        //&hrm::spectrum_freqs(hrm::normalize_spectrum_max(suppressed_spectrum)),
                    ])?;
                    //plot_phase_diffs(phase_diffs.clone())?;
                    //phase_diffs.clear();
                }

                hrm::max_bpm(spectrum_smooth)
            } else {
                bpm_smooth
            };
            biased_filter.tune(bpm_smooth);

            let spectrum = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum));
            let spectrum_orig = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum_orig));
            if j % 512 == 0 {
                //plot_values_multiple(&[&spectrum_orig, &spectrum])?;
                //plot_values(&window.iter().cloned().collect::<Vec<_>>())?;
            }

            bpm_vals_freq.push((*i, bpm.0 as f32));
            bpm_vals_freq_smooth.push((*i, bpm_smooth.0 as f32));
            bpm_vals_accel_supressed.push((*i, bpm_suppressed.0 as f32));
            bpm_vals_accel_supressed2.push((*i, bpm_suppressed2.0 as f32));
        }

        if let Some(bpm) = baseline_filter.add_sample(*vr).1 {
            bpm_vals_baseline.push((*i, bpm.0 as f32));
        }
    }
    //plot_phase_diffs(phase_diffs)?;
    plot_values_multiple(&[
        &bpm_vals,
        &bpm_vals_freq,
        &bpm_vals_freq_smooth,
        &bpm_vals_baseline,
        //&accel_vals_freq,
        //&accel_vals_freq_smooth,
        &bpm_vals_accel_supressed,
        &bpm_vals_accel_supressed2,
    ])?;

    for (i, o) in vals[50..].iter().zip(indata.iter_mut()) {
        *o = i.1;
    }

    r2c.process(&mut indata, &mut spectrum).unwrap();

    let spectrum = spectrum[3..23]
        .iter()
        .enumerate()
        .map(|(i, v)| ((10 * (i + 3)) as f32, v.norm()))
        .collect::<Vec<_>>();

    //assert_eq!(spectrum.len(), freqs.len());
    //plot_values(&spectrum)?;

    Ok(())
}
