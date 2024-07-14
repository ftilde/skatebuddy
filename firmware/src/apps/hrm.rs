use arrform::*;
use core::fmt::Write;
use drivers::lpm013m1126c::Rgb111;
use drivers::time::Duration;
use drivers::{futures::select, time::Instant};
use embedded_graphics::{
    geometry::{Point, Size},
    mono_font::MonoTextStyle,
};
use littlefs2::path::PathBuf;
use util::{HeartbeatDetector, BPM};

use crate::{
    render_top_bar,
    ui::{ButtonStyle, TextWriter},
    Context,
};

struct DrawState {
    filtered: [f32; 176],
    next_sample_pos: usize,
    bpm_detector: HeartbeatDetector,
    start: Instant,
    last_bpm: Option<BPM>,
}

impl Default for DrawState {
    fn default() -> Self {
        DrawState {
            filtered: [0.0; 176],
            next_sample_pos: 0,
            start: Instant::now(),
            bpm_detector: Default::default(),
            last_bpm: None,
        }
    }
}

pub async fn hrm(ctx: &mut Context) {
    //let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    //let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);
    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut hrm = ctx.hrm.on(&mut ctx.twi1).await;
    hrm.enable().await;

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

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
            samples: [u16; 500],
            sample: usize,
        },
    }

    let mut state = State::Idle;

    let mut draw_state = DrawState::default();
    let mut last_current = 0u8;
    let mut last_res = 0u8;
    let fc = 0.5; //Hz
    let dt = 0.04; //s
    let rc = 1.0 / (core::f32::consts::TAU * fc);
    let alpha = rc / (rc + dt);

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

                if r.current_value[0] != last_current || r.pd_res_value[0] != last_res {
                    last_current = r.current_value[0];
                    last_res = r.pd_res_value[0];
                    draw_state = Default::default();
                }

                if let Some(sample_vals) = s {
                    //let _ = writeln!(w, "s: {:?}", sample_vals);

                    for sample in &sample_vals {
                        let (filtered, bpm) = draw_state.bpm_detector.add_sample(*sample);
                        if let Some(bpm) = bpm {
                            draw_state.last_bpm = Some(bpm);
                        }

                        let current = draw_state.next_sample_pos;
                        draw_state.next_sample_pos =
                            (draw_state.next_sample_pos + 1) % draw_state.filtered.len();
                        draw_state.filtered[current] = filtered;
                    }

                    let min = *draw_state
                        .filtered
                        .iter()
                        .min_by(|l, r| l.total_cmp(r))
                        .unwrap();
                    let max = *draw_state
                        .filtered
                        .iter()
                        .max_by(|l, r| l.total_cmp(r))
                        .unwrap();
                    //let min = *draw_state.samples.iter().min().unwrap();
                    //let max = *draw_state.samples.iter().max().unwrap();
                    let range = (max - min) as f32;

                    let end = 176;
                    let sample_to_pixel = |sample| {
                        let norm = (sample - min) as f32 / range;
                        end - (norm * 50.0) as i32
                    };
                    let pixel_0 = sample_to_pixel(0.0);
                    for (y, sample) in draw_state.filtered.iter().enumerate() {
                        let x = sample_to_pixel(*sample);
                        let y = y as i32;
                        if x < pixel_0 {
                            let end = pixel_0.min(end);
                            ctx.lcd.set_line(y, x, end, Rgb111::red());
                        }
                        ctx.lcd
                            .set_line(y, pixel_0.clamp(x, end), end, Rgb111::yellow());
                    }

                    if let State::Recording {
                        since,
                        path,
                        sample,
                        samples,
                    } = &mut state
                    {
                        for val in sample_vals {
                            if *sample == samples.len() {
                                break;
                            }
                            samples[*sample] = val;
                            *sample += 1;
                        }
                        if since.elapsed() > Duration::from_secs(10) || *sample == samples.len() {
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
                                    )
                                })
                                .await
                                .unwrap();
                            state = State::Idle;
                        }
                    }
                }
                let mut w =
                    TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);
                if let State::Idle = state {
                    //let min = *draw_state.samples.iter().min().unwrap();
                    //let max = *draw_state.samples.iter().max().unwrap();
                    let min = *draw_state
                        .filtered
                        .iter()
                        .min_by(|l, r| l.total_cmp(r))
                        .unwrap();
                    let max = *draw_state
                        .filtered
                        .iter()
                        .max_by(|l, r| l.total_cmp(r))
                        .unwrap();

                    let _ = writeln!(w, "range: [{}, {}]", min, max);
                    if let Some(bpm) = draw_state.last_bpm.as_ref() {
                        let _ = writeln!(w, "bpm: {}", bpm.0);
                    } else {
                        let _ = writeln!(w, "bpm: ??");
                    }

                    let s_time = draw_state.start.elapsed().as_millis() as f32
                        / draw_state.bpm_detector.num_samples() as f32;
                    let _ = writeln!(w, "s_time: {}", s_time);

                    let _ = writeln!(w, "status: {}", r.status);
                    let _ = writeln!(w, "irq_status: {}", r.irq_status);
                    let _ = writeln!(w, "env: {:?}", r.env_value);
                    let _ = writeln!(w, "pre: {:?}", r.pre_value);
                    let _ = writeln!(w, "ps: {}", r.ps_value);
                    let _ = writeln!(w, "pd: {:?}", r.pd_res_value);
                    let _ = writeln!(w, "cur: {:?}", r.current_value);
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
                    let path = ctx
                        .flash
                        .with_fs(|fs| {
                            fs.create_dir_all(b"/hrm/\0".try_into().unwrap())?;
                            for i in 0.. {
                                let path =
                                    PathBuf::from(arrform!(40, "/hrm/samples{}.bin", i).as_str());
                                if fs.metadata(&path) == Err(littlefs2::io::Error::NoSuchEntry) {
                                    return Ok(path);
                                }
                            }
                            panic!("Too many recordings");
                        })
                        .await
                        .unwrap();
                    state = State::Recording {
                        since: Instant::now(),
                        path,
                        samples: [0; 500],
                        sample: 0,
                    }
                }
                if plus_button.clicked(&e) {
                    hrm.update_hrm_res(|c| {
                        c.res = (c.res + 1).min(7);
                    })
                    .await;
                }
                if minus_button.clicked(&e) {
                    hrm.update_hrm_res(|c| {
                        c.res = c.res.saturating_sub(1);
                    })
                    .await;
                }
            }
        }
    }
    hrm.disable().await;
}
