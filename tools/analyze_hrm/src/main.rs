//use goertzel_algorithm::OptimizedGoertzel;
use biquad::*;
//use goertzel_nostd::Parameters;
use plotpy::{Curve, Plot};
use realfft::RealFftPlanner;
use std::error::Error;

#[derive(serde::Deserialize)]
struct Row {
    val: i16,
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

    if let Err(e) = plot
        .legend()
        .grid_and_labels("x", "y")
        .save_and_show("out.svg")
    {
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

    if let Err(e) = plot.save_and_show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let file = std::fs::File::open(&args[1]).unwrap();
    let mut rdr = csv::Reader::from_reader(file);

    let fs = 25.hz();
    let f0 = 1.0.hz();

    let coefficients =
        Coefficients::<f32>::from_params(biquad::Type::HighPass, fs, f0, Q_BUTTERWORTH_F32)
            .unwrap();
    let mut filter = DirectForm2Transposed::<f32>::new(coefficients);

    let mut gradient_clip = hrm::GradientClip::default();
    let mut ms = 0;
    let mut vals = Vec::new();
    let mut raw_vals = Vec::new();
    let sample_delay = 40;
    //let sample_rate = 1000 / sample_delay;
    for result in rdr.records() {
        let record = result?;
        let mut row: Row = record.deserialize(None)?;

        let val = gradient_clip.add_value(row.val);
        //let val = row.val;
        vals.push((ms as f32, filter.run(val as f32)));
        raw_vals.push((ms as f32, val));
        ms += sample_delay;
    }

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

    let mut hrm_detector = hrm::HeartbeatDetector::new(hrm::UncalibratedEstimator);
    let mut freq_detector = hrm::FFTEstimator::default();
    let mut spec_smoother = hrm::SpectrumSmoother::default();
    let mut bpm_vals = Vec::new();
    let mut bpm_vals_freq = Vec::new();
    let mut bpm_vals_freq_smooth = Vec::new();
    for (i, v) in raw_vals.iter() {
        if let Some(bpm) = hrm_detector.add_sample(*v).1 {
            bpm_vals.push((*i, bpm.0 as f32));
        }
    }
    let mut window = std::collections::VecDeque::new();
    let mut prev = 0.0;
    let mut biased_filter = hrm::BiasedSampleFilter::new();
    for (j, ((i, v), (ir, vr))) in vals.iter().zip(raw_vals.iter()).enumerate() {
        //window.push_back((*ir, (*vr as f32 - prev).abs()));
        let filtered = biased_filter.filter(*vr);
        window.push_back((*i, filtered));
        prev = *vr as f32;
        if window.len() == 512 {
            window.pop_front();
        }
        if let Some(spectrum_orig) = freq_detector.add_sample(*v as _) {
            let spectrum = spec_smoother.add(spectrum_orig);

            let bpm = hrm::max_bpm(spectrum_orig);
            let bpm_smooth = hrm::max_bpm(spectrum);
            biased_filter.tune(bpm_smooth);

            let spectrum = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum));
            let spectrum_orig = hrm::spectrum_freqs(hrm::normalize_spectrum_max(spectrum_orig));
            if j % 512 == 0 {
                plot_values_multiple(&[&spectrum_orig, &spectrum])?;
                plot_values(&window.iter().cloned().collect::<Vec<_>>())?;
            }

            bpm_vals_freq.push((*i, bpm.0 as f32));
            bpm_vals_freq_smooth.push((*i, bpm_smooth.0 as f32));
        }
    }
    plot_values_multiple(&[&bpm_vals, &bpm_vals_freq, &bpm_vals_freq_smooth])?;

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
