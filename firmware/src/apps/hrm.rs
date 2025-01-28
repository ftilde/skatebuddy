use arrform::*;
use core::fmt::Write;
use drivers::accel::Reading;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::Duration;
use drivers::{futures::select, time::Instant};
use embedded_graphics::{
    geometry::{Point, Size},
    mono_font::MonoTextStyle,
};
use hrm::HeartbeatDetector;
use littlefs2::path::PathBuf;
use util::RingBuffer;

use crate::util::SampleCountingEstimator;
use crate::{
    render_top_bar,
    ui::{ButtonStyle, TextWriter},
    Context,
};

struct DrawState {
    filtered: RingBuffer<176, f32>,
    bpm_detector: HeartbeatDetector<SampleCountingEstimator>,
}

impl Default for DrawState {
    fn default() -> Self {
        DrawState {
            filtered: Default::default(),
            bpm_detector: hrm::HeartbeatDetector::new(SampleCountingEstimator::new()),
        }
    }
}

pub async fn hrm(ctx: &mut Context) {
    //let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    //let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);
    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut hrm = ctx.hrm.on(&ctx.twi).await;
    hrm.enable().await;

    let mut last_bpm = None;

    let mut touch = ctx.touch.enabled(&ctx.twi).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let mut record_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(0, 126),
        size: Size::new(50, 50),
        style: &button_style,
        text: "Record",
    });
    let mut plus_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(50, 126),
        size: Size::new(50, 50),
        style: &button_style,
        text: "+",
    });
    let mut minus_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(100, 126),
        size: Size::new(50, 50),
        style: &button_style,
        text: "-",
    });

    enum State {
        Idle,
        Recording {
            since: Instant,
            path: PathBuf,
            samples: [i16; 2000],
            sample: usize,
            accel_path: PathBuf,
            accel_samples: [Reading; 2000],
            accel_sample: usize,
        },
    }

    let mut state = State::Idle;

    let mut draw_state = DrawState::default();
    let mut last_res = 0u8;

    let mut config = drivers::accel::Config::new();
    config
        .odcntl
        .set_output_data_rate(drivers::accel::DataRate::Hz25);
    config.buf_cntl2.set_mode(drivers::accel::BufMode::Fifo);
    config.buf_cntl2.set_enabled(1);
    config
        .buf_cntl2
        .set_resolution(drivers::accel::BufRes::Bit16);
    let mut accel = ctx.accel.on(&ctx.twi, config).await;

    ctx.lcd.on().await;
    loop {
        match select::select3(
            hrm.wait_event(),
            ctx.button.wait_for_press(),
            touch.wait_for_action(),
        )
        .await
        {
            select::Either3::First((r, s)) => {
                ctx.lcd.fill(Rgb111::black());
                render_top_bar(&mut ctx.lcd, &ctx.battery).await;

                if r.pd_res_value[0] != last_res {
                    last_res = r.pd_res_value[0];
                    draw_state = Default::default();
                }

                if let Some(sample_vals) = s {
                    //let _ = writeln!(w, "s: {:?}", sample_vals);
                    let mut accel_buf = [Reading::default(); 40];

                    let accel_read = accel.read_buffer(&mut accel_buf).await;
                    //for r in accel_read {
                    //    crate::println!("{}", r);
                    //}

                    for sample in &sample_vals {
                        let (filtered, bpm) = draw_state.bpm_detector.add_sample(*sample);
                        if let Some(bpm) = bpm {
                            last_bpm = Some(bpm);
                        }

                        draw_state.filtered.add(filtered);
                    }
                    if let State::Recording {
                        since,
                        path,
                        sample,
                        samples,
                        accel_path,
                        accel_samples,
                        accel_sample,
                    } = &mut state
                    {
                        crate::println!("HRM: {}, Accel: {}", sample_vals.len(), accel_read.len());
                        for val in sample_vals {
                            if *sample == samples.len() {
                                break;
                            }
                            samples[*sample] = val;
                            *sample += 1;
                        }

                        for val in accel_read {
                            if *accel_sample == accel_samples.len() {
                                break;
                            }
                            accel_samples[*accel_sample] = *val;
                            *accel_sample += 1;
                        }
                        if since.elapsed() > Duration::from_secs(1000)
                            || *sample == samples.len()
                            || *accel_sample == accel_samples.len()
                        {
                            ctx.flash
                                .with_fs(|fs| {
                                    fs.open_file_with_options_and_then(
                                        |o| o.write(true).create(true).append(true),
                                        &path,
                                        |file| {
                                            use littlefs2::io::Write;
                                            for i in 0..*sample {
                                                let sample_val = samples[i];
                                                let content = arrform!(40, "{}\n", sample_val);
                                                file.write_all(content.as_bytes())?;
                                            }
                                            Ok(())
                                        },
                                    )?;
                                    fs.open_file_with_options_and_then(
                                        |o| o.write(true).create(true).append(true),
                                        &accel_path,
                                        |file| {
                                            use littlefs2::io::Write;
                                            for i in 0..*accel_sample {
                                                let sample_val = accel_samples[i];
                                                let content = arrform!(
                                                    120,
                                                    "{},{},{}\n",
                                                    sample_val.x,
                                                    sample_val.y,
                                                    sample_val.z
                                                );
                                                file.write_all(content.as_bytes())?;
                                            }
                                            Ok(())
                                        },
                                    )
                                })
                                .await
                                .unwrap();

                            state = State::Idle;
                        }
                    }
                }

                let valid_values = draw_state.filtered.valid_values();
                let min = valid_values.iter().min_by(|l, r| l.total_cmp(r));
                let max = valid_values.iter().max_by(|l, r| l.total_cmp(r));
                if valid_values.len() > 0 {
                    let min = *min.unwrap();
                    let max = *max.unwrap();
                    let range = (max - min) as f32;

                    let end = 176;
                    let sample_to_pixel = |sample| {
                        let norm = (sample - min) as f32 / range;
                        end - (norm * 50.0) as i32
                    };
                    let pixel_0 = sample_to_pixel(0.0);
                    for (y, sample) in draw_state.filtered.valid_values().iter().enumerate() {
                        let x = sample_to_pixel(*sample);
                        let y = y as i32;
                        if x < pixel_0 {
                            let end = pixel_0.min(end);
                            ctx.lcd.set_line(y, x, end, Rgb111::red());
                        }
                        ctx.lcd
                            .set_line(y, pixel_0.clamp(x, end), end, Rgb111::yellow());
                    }
                }

                let mut w =
                    TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);

                if let State::Idle = state {
                    let valid_vals = draw_state.filtered.valid_values();
                    if !valid_vals.is_empty() {
                        let min = *min.unwrap();
                        let max = *max.unwrap();
                        let _ = writeln!(w, "range: [{}, {}]", min, max);
                    }
                    if let Some(bpm) = last_bpm.as_ref() {
                        let _ = writeln!(w, "bpm: {}", bpm.0);
                    } else {
                        let _ = writeln!(w, "bpm: ??");
                    }

                    let _ = writeln!(w, "s_time: {}", draw_state.bpm_detector.millis_per_sample());

                    let _ = writeln!(w, "status: {}", r.status);
                    let _ = writeln!(w, "env: {:?}", r.env_value);
                    let _ = writeln!(w, "ps: {}", r.ps_value);
                    let _ = writeln!(w, "pd: {:?}", r.pd_res_value);
                    let _ = writeln!(w, "cur: {:?}", r.current_value);
                    let _ = writeln!(w, "pre: {:?}", r.pre_value);
                    let _ = writeln!(w, "irq_status: {}", r.irq_status);
                }

                if matches!(state, State::Idle) {
                    record_button.render(&mut *ctx.lcd).unwrap();
                }
                plus_button.render(&mut *ctx.lcd).unwrap();
                minus_button.render(&mut *ctx.lcd).unwrap();

                ctx.lcd.present().await;
            }
            select::Either3::Second(_d) => {
                break;
            }
            select::Either3::Third(e) => {
                ctx.backlight.active().await;
                if record_button.clicked(&e) {
                    let (path, accel_path) = ctx
                        .flash
                        .with_fs(|fs| {
                            fs.create_dir_all(b"/hrm/\0".try_into().unwrap())?;
                            for i in 0.. {
                                let path =
                                    PathBuf::from(arrform!(40, "/hrm/samples{}.bin", i).as_str());
                                let accel_path = PathBuf::from(
                                    arrform!(40, "/hrm/accel_samples{}.bin", i).as_str(),
                                );
                                if fs.metadata(&path) == Err(littlefs2::io::Error::NoSuchEntry)
                                    && fs.metadata(&accel_path)
                                        == Err(littlefs2::io::Error::NoSuchEntry)
                                {
                                    return Ok((path, accel_path));
                                }
                            }
                            panic!("Too many recordings");
                        })
                        .await
                        .unwrap();
                    state = State::Recording {
                        since: Instant::now(),
                        path,
                        samples: [0; 2000],
                        sample: 0,
                        accel_path,
                        accel_samples: [Default::default(); 2000],
                        accel_sample: 0,
                    }
                }
                if plus_button.clicked(&e) {
                    hrm.update_sample_delay(|c| {
                        *c += 1;
                        defmt::println!("Delay: {}", *c);
                    })
                    .await;
                }
                if minus_button.clicked(&e) {
                    hrm.update_sample_delay(|c| {
                        *c -= 1;
                        defmt::println!("Delay: {}", *c);
                    })
                    .await;
                }
            }
        }
    }
    hrm.disable().await;
}
