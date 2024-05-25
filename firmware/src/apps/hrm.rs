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

struct DrawState {
    samples: [u16; 176],
    filtered: [f32; 176],
    next: usize,
}

impl Default for DrawState {
    fn default() -> Self {
        DrawState {
            samples: [0; 176],
            filtered: [0.0; 176],
            next: 0,
        }
    }
}

pub async fn hrm(ctx: &mut Context) {
    //let font = bitmap_font::tamzen::FONT_16x32_BOLD;
    //let sl = TextStyle::new(&font, embedded_graphics::pixelcolor::BinaryColor::On);
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
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
        position: Point::new(126, 0),
        size: Size::new(50, 50),
        style: &button_style,
        text: "Record",
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
    let fc = 10.0; //Hz
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
                        let prev = current.wrapping_sub(1).min(draw_state.samples.len() - 1);
                        draw_state.samples[current] = *sample;

                        let sample_diff =
                            draw_state.samples[current] as f32 - draw_state.samples[prev] as f32;
                        draw_state.filtered[current] =
                            alpha * (draw_state.filtered[prev] + sample_diff);

                        draw_state.next = (draw_state.next + 1) % draw_state.samples.len();
                    }

                    let min = *draw_state.samples.iter().min().unwrap();
                    let max = *draw_state.samples.iter().max().unwrap();
                    let range = (max - min) as f32;

                    for (x, sample) in draw_state.samples.iter().enumerate() {
                        let norm = (sample - min) as f32 / range;
                        let y = 176 - (norm * 50.0) as i32;
                        let x = x as i32;
                        for y in y..176 {
                            ctx.lcd.set(y, x, Rgb111::red());
                        }
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
                    TextWriter::new(&mut ctx.lcd, sl).y(20 + font.character_size.height as i32);
                if let State::Idle = state {
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

                ctx.lcd.present().await;
            }
            select::Either3::Second(_d) => {
                break;
            }
            select::Either3::Third(e) => {
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
            }
        }
    }
    hrm.disable().await;
}
