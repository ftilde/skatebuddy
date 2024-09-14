use arrform::*;
use bytemuck::Zeroable;
use core::fmt::Write;
use drivers::gps::{CasicMsg, NavPv};
use drivers::lpm013m1126c::Rgb111;
use drivers::time::{Duration, Instant};
use drivers::{futures::select, gps::CasicMsgConfig};
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::MonoTextStyle;
use littlefs2::path::PathBuf;

use crate::ui::ButtonStyle;
use crate::{render_top_bar, ui::TextWriter, Context};

pub async fn track(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let mut gps = drivers::gps::GPSReceiver::new(CasicMsgConfig {
        nav_pv: 1,
        nav_gps_info: 1,
        ..Default::default()
    })
    .await;
    ctx.lcd.on().await;

    struct State {
        num_satellites: u8,
        num_satellites_in_view: u8,
        num_satellites_in_fix: u8,
        pos_valid: u8,
        vel_valid: u8,
        lon: u64,
        lat: u64,
        height: u32,
        vel_east: u32,
        vel_north: u32,
    }

    let mut state = State {
        num_satellites: 0,
        num_satellites_in_view: 0,
        num_satellites_in_fix: 0,
        pos_valid: 0,
        vel_valid: 0,
        lon: 0,
        lat: 0,
        height: 0,
        vel_east: 0,
        vel_north: 0,
    };

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

    enum RecordingState {
        Idle,
        Recording {
            since: Instant,
            path: PathBuf,
            samples: [NavPv; 30],
            sample: usize,
        },
    }

    let mut recording_state = RecordingState::Idle;

    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);
        let _ = writeln!(w, "sat_n: {:?}", state.num_satellites);
        let _ = writeln!(w, "sat_v: {:?}", state.num_satellites_in_view);
        let _ = writeln!(w, "sat_f: {:?}", state.num_satellites_in_fix);
        let _ = writeln!(w, "pos_valid {:?}", state.pos_valid);
        let _ = writeln!(w, "vel_valid: {:?}", state.vel_valid);
        let _ = writeln!(w, "lon: {:?}", state.lon);
        let _ = writeln!(w, "lat: {:?}", state.lat);
        let _ = writeln!(w, "height: {:?}", state.height);
        let _ = writeln!(w, "vel_east: {:?}", state.vel_east);
        let _ = writeln!(w, "vel_north: {:?}", state.vel_north);

        if matches!(recording_state, RecordingState::Idle) {
            record_button.render(&mut *ctx.lcd).unwrap();
        }

        ctx.lcd.present().await;

        match select::select3(
            gps.receive(),
            ctx.button.wait_for_press(),
            touch.wait_for_action(),
        )
        .await
        {
            select::Either3::First(msg) => match msg {
                CasicMsg::NavPv(s) => {
                    crate::println!("pv msg: {:?}", s);
                    state.num_satellites = s.num_sv;
                    state.pos_valid = s.pos_valid;
                    state.vel_valid = s.vel_valid;
                    state.lon = s.longitude;
                    state.lat = s.latitude;
                    state.height = s.height_m;
                    state.vel_east = s.east_velocity_m_s;
                    state.vel_north = s.north_velocity_m_s;

                    if let RecordingState::Recording {
                        since,
                        path,
                        sample,
                        samples,
                    } = &mut recording_state
                    {
                        samples[*sample] = s;
                        *sample += 1;

                        if since.elapsed() > Duration::from_secs(1000) || *sample == samples.len() {
                            ctx.flash
                                .with_fs(|fs| {
                                    fs.open_file_with_options_and_then(
                                        |o| o.write(true).create(true).append(true),
                                        &path,
                                        |file| {
                                            use littlefs2::io::Write;
                                            for i in 0..*sample {
                                                let sample_val = samples[i];
                                                file.write_all(bytemuck::bytes_of(&sample_val))?;
                                            }
                                            Ok(())
                                        },
                                    )
                                })
                                .await
                                .unwrap();
                            recording_state = RecordingState::Idle;
                        }
                    }
                }
                CasicMsg::NavGpsInfo(i) => {
                    crate::println!("gps msg: {:?}", i);
                    state.num_satellites_in_view = i.num_view_sv;
                    state.num_satellites_in_fix = i.num_fix_sv;
                }
                _ => {}
            },
            select::Either3::Second(_) => {
                break;
            }
            select::Either3::Third(e) => {
                ctx.backlight.active().await;
                if record_button.clicked(&e) {
                    let path = ctx
                        .flash
                        .with_fs(|fs| {
                            fs.create_dir_all(b"/gps/\0".try_into().unwrap())?;
                            for i in 0.. {
                                let path =
                                    PathBuf::from(arrform!(40, "/gps/samples{}.bin", i).as_str());
                                if fs.metadata(&path) == Err(littlefs2::io::Error::NoSuchEntry) {
                                    return Ok(path);
                                }
                            }
                            panic!("Too many recordings");
                        })
                        .await
                        .unwrap();
                    recording_state = RecordingState::Recording {
                        since: Instant::now(),
                        path,
                        samples: [NavPv::zeroed(); 30],
                        sample: 0,
                    }
                }
            }
        }
    }
}
