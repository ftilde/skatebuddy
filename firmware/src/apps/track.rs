use arrform::*;
use bytemuck::Zeroable;
use core::fmt::Write;
use drivers::flash::FlashRessources;
use drivers::gps::{CasicMsg, GPSReceiver, NavGpsInfo, NavigationData};
use drivers::lpm013m1126c::Rgb111;
use drivers::{futures::select, gps::CasicMsgConfig};
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::mono_font::MonoTextStyle;
use littlefs2::path::PathBuf;

use crate::ui::ButtonStyle;
use crate::{render_top_bar, ui::TextWriter, Context};

struct RecordingData {
    path: PathBuf,
    samples: [NavigationData; 12],
    sample: usize,
}

impl RecordingData {
    async fn flush(&mut self, flash: &mut FlashRessources) {
        if self.sample > 0 {
            flash
                .with_fs(|fs| {
                    fs.open_file_with_options_and_then(
                        |o| o.write(true).create(true).append(true),
                        &self.path,
                        |file| {
                            use littlefs2::io::Write;
                            file.write_all(bytemuck::cast_slice(&self.samples[..self.sample]))?;
                            Ok(())
                        },
                    )
                })
                .await
                .unwrap();
            self.sample = 0;
        }
    }
    async fn add_sampale(&mut self, s: NavigationData, flash: &mut FlashRessources) {
        self.samples[self.sample] = s;
        self.sample += 1;
        if self.sample == self.samples.len() {
            self.flush(flash).await
        }
    }
}

enum RecordingState {
    Idle,
    Recording(RecordingData),
}

pub async fn track_app(ctx: &mut Context) {
    let mut gps = drivers::gps::GPSReceiver::new(CasicMsgConfig::default()).await;

    if wait_for_fix(ctx, &mut gps).await.is_ok() {
        show_pos(ctx, &mut gps).await
    }
}

pub async fn show_pos(ctx: &mut Context, gps: &mut GPSReceiver<'_>) {
    gps.update_config(CasicMsgConfig {
        nav_pv: 1,
        ..Default::default()
    })
    .await;

    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    ctx.lcd.on().await;

    struct State {
        num_satellites: u8,
        pos_valid: u8,
        vel_valid: u8,
        lon: f64,
        lat: f64,
        height: f32,
        vel_east: f32,
        vel_north: f32,
    }

    let mut state = State {
        num_satellites: 0,
        pos_valid: 0,
        vel_valid: 0,
        lon: 0.0,
        lat: 0.0,
        height: 0.0,
        vel_east: 0.0,
        vel_north: 0.0,
    };

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font,
    };

    let mut record_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(126, 126),
        size: Size::new(50, 50),
        style: &button_style,
        text: "Record",
    });
    let mut stop_button = crate::ui::Button::from(crate::ui::ButtonDefinition {
        position: Point::new(126, 126),
        size: Size::new(50, 50),
        style: &button_style,
        text: "Stop",
    });

    let mut recording_state = RecordingState::Idle;

    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);
        let _ = writeln!(w, "sat_n: {:?}", state.num_satellites);
        let _ = writeln!(w, "pos_valid {:?}", state.pos_valid);
        let _ = writeln!(w, "vel_valid: {:?}", state.vel_valid);
        let _ = writeln!(w, "lon: {:?}", state.lon);
        let _ = writeln!(w, "lat: {:?}", state.lat);
        let _ = writeln!(w, "height: {:?}", state.height);
        let _ = writeln!(w, "vel_east: {:?}", state.vel_east);
        let _ = writeln!(w, "vel_north: {:?}", state.vel_north);

        if matches!(recording_state, RecordingState::Idle) {
            record_button.render(&mut *ctx.lcd).unwrap();
        } else {
            stop_button.render(&mut *ctx.lcd).unwrap();
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

                    if let RecordingState::Recording(data) = &mut recording_state {
                        data.add_sampale(s.into(), &mut ctx.flash).await;
                    }
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
                    recording_state = RecordingState::Recording(RecordingData {
                        path,
                        samples: [NavigationData::zeroed(); 12],
                        sample: 0,
                    })
                }
                if stop_button.clicked(&e) {
                    let RecordingState::Recording(data) = &mut recording_state else {
                        panic!("Invalid state");
                    };

                    data.flush(&mut ctx.flash).await;
                    recording_state = RecordingState::Idle;
                }
            }
        }
    }

    if let RecordingState::Recording(data) = &mut recording_state {
        data.flush(&mut ctx.flash).await;
    }
}

pub async fn wait_for_fix(ctx: &mut Context, gps: &mut GPSReceiver<'_>) -> Result<(), ()> {
    gps.update_config(CasicMsgConfig {
        nav_gps_info: 1,
        ..Default::default()
    })
    .await;

    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    ctx.lcd.on().await;

    let mut state = NavGpsInfo::zeroed();

    while state.num_fix_sv == 0 {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);
        let _ = writeln!(w, "sat_v: {:?}", state.num_view_sv);
        let _ = writeln!(w, "sat_f: {:?}", state.num_fix_sv);

        ctx.lcd.present().await;

        match select::select(gps.receive(), ctx.button.wait_for_press()).await {
            select::Either::First(msg) => match msg {
                CasicMsg::NavGpsInfo(i) => {
                    state = i;
                }
                _ => {}
            },
            select::Either::Second(_) => {
                return Err(());
            }
        }
    }
    Ok(())
}
