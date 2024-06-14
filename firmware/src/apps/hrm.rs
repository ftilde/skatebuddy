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

use crate::{
    render_top_bar,
    ui::{ButtonStyle, TextWriter},
    Context,
};

#[derive(Copy, Clone)]
enum BeatRegion {
    Above,
    Below,
}

struct DrawState {
    samples: [u16; 176],
    filtered: [f32; 176],
    filter_state: HrmFilter,
    running_mean: f32,
    next: usize,
    region: BeatRegion,
    last_bpm: u16,
    sample_count: usize,
    last_beat_sample: usize,
    min_since_cross: i32,
    min_sample: usize,
}

impl Default for DrawState {
    fn default() -> Self {
        DrawState {
            samples: [0; 176],
            running_mean: 4096.0 / 2.0,
            filter_state: HrmFilter::new(),
            filtered: [0.0; 176],
            next: 0,
            region: BeatRegion::Below,
            sample_count: 0,
            last_bpm: 0,
            last_beat_sample: 0,
            min_since_cross: i32::MAX,
            min_sample: 0,
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
                        let current = draw_state.next;
                        //let prev = current.wrapping_sub(1).min(draw_state.samples.len() - 1);
                        draw_state.samples[current] = *sample;
                        let filtered = draw_state.filter_state.filter(*sample);
                        draw_state.filtered[current] = filtered as f32;

                        draw_state.sample_count += 1;
                        if filtered < draw_state.min_since_cross {
                            draw_state.min_since_cross = filtered;
                            draw_state.min_sample = draw_state.sample_count;
                        }

                        match (draw_state.region, filtered > 0) {
                            (BeatRegion::Above, false) => {
                                draw_state.region = BeatRegion::Below;
                            }
                            (BeatRegion::Below, true) => {
                                let samples_since_last_beat =
                                    draw_state.min_sample - draw_state.last_beat_sample;
                                draw_state.last_beat_sample = draw_state.min_sample;
                                draw_state.min_since_cross = i32::MAX;

                                let beat_duration_millis = samples_since_last_beat * 40 /* 40ms = 1/25Hz */;
                                draw_state.last_bpm = ((60 * 1000) / beat_duration_millis) as u16;

                                draw_state.region = BeatRegion::Above;
                            }
                            _ => {}
                        }

                        //let sample_diff =
                        //    draw_state.samples[current] as f32 - draw_state.samples[prev] as f32;
                        //draw_state.filtered[current] =
                        //    alpha * (draw_state.filtered[prev] + sample_diff);
                        //draw_state.running_mean = draw_state.running_mean * alpha
                        //    + (1.0 - alpha) * draw_state.samples[current] as f32;
                        //draw_state.filtered[current] =
                        //    draw_state.samples[current] as f32 - draw_state.running_mean;
                        //draw_state.filtered[current] = draw_state.running_mean;

                        draw_state.next = (draw_state.next + 1) % draw_state.samples.len();
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

                    for (y, sample) in draw_state.filtered.iter().enumerate() {
                        let norm = (sample - min) as f32 / range;
                        let end = 176;
                        let x = end - (norm * 50.0) as i32;
                        let y = y as i32;
                        ctx.lcd.set_line(y, x, end, Rgb111::red());
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
                    let _ = writeln!(w, "bpm: {}", draw_state.last_bpm);

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

struct HrmFilter {
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
    fn new() -> Self {
        Self {
            history: [0; FILTER_SIZE],
            next_pos: 0,
        }
    }

    fn filter(&mut self, val: u16) -> i32 {
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
