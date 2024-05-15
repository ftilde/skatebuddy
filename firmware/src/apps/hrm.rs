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
        position: Point::new(120, 120),
        size: Size::new(50, 50),
        style: &button_style,
        text: "Record",
    });

    enum State {
        Idle,
        Recording {
            since: Instant,
            path: PathBuf,
            samples: [(u16, u16); 200],
            sample: usize,
        },
    }

    let mut state = State::Idle;

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

                if let Some(sample_val) = s {
                    let _ = writeln!(w, "s: {:?}", sample_val);

                    if let State::Recording {
                        since,
                        path,
                        sample,
                        samples,
                    } = &mut state
                    {
                        let ts = since.elapsed().as_millis();
                        samples[*sample] = (ts as u16, sample_val);
                        *sample += 1;
                        if since.elapsed() > Duration::from_secs(10) || *sample == samples.len() {
                            ctx.flash
                                .with_fs(|fs| {
                                    fs.open_file_with_options_and_then(
                                        |o| o.write(true).create(true).append(true),
                                        &path,
                                        |file| {
                                            use littlefs2::io::Write;
                                            for i in 0..*sample {
                                                let (ts, sample_val) = samples[i];
                                                let content =
                                                    arrform!(40, "{};{}\n", ts, sample_val);
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
                        samples: [(0, 0); 200],
                        sample: 0,
                    }
                }
            }
        }
    }
    hrm.disable().await;
}
