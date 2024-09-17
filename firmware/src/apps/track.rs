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
use nalgebra::Vector2;
use util::gps::{KalmanFilter, LazyRefConverter};

use crate::ui::ButtonStyle;
use crate::util::SampleCountingEstimator;
use crate::{render_top_bar, ui::TextWriter, Context};

struct RecordingData {
    path: PathBuf,
    samples: [NavigationData; 32],
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
    let font = &embedded_graphics::mono_font::ascii::FONT_10X20;
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    let btn_font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let btn_sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    ctx.lcd.on().await;

    #[derive(Default)]
    struct State {
        num_satellites: u8,
        speed: f32,
        speed_smooth: f32,
        distance: f32,
        distance_smooth: f32,
        height: f32,
        last_pos: Vector2<f32>,
        last_pos_smooth: Vector2<f32>,
        bpm: u16,
    }
    let mut state = State::default();

    let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

    let button_style = ButtonStyle {
        fill: Rgb111::blue(),
        highlight: Rgb111::white(),
        font: btn_font,
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

    let mut hrm = ctx.hrm.on(&mut ctx.twi1).await;
    hrm.enable().await;
    let mut bpm_detector = hrm::HeartbeatDetector::new(SampleCountingEstimator::new());

    let mut ref_converter = LazyRefConverter::default();
    let mut kalman = KalmanFilter::new();
    let movement_threshold_km_h = 3.0;

    let mut recording_state = RecordingState::Idle;

    let mut num_samples_recorded = 0;

    loop {
        ctx.lcd.fill(Rgb111::black());
        render_top_bar(&mut ctx.lcd, &ctx.battery).await;

        let mut w = TextWriter::new(&mut ctx.lcd, sl).y(10 + font.character_size.height as i32);
        let _ = writeln!(w, "BPM: {}", state.bpm);
        let _ = writeln!(w, "{:.1} km/h", state.speed * 3.6);
        let _ = writeln!(w, "{:.1} km/h", state.speed_smooth * 3.6);
        let _ = writeln!(w, "{:.3} km", state.distance / 1000.0);
        let _ = writeln!(w, "{:.3} km", state.distance_smooth / 1000.0);
        let _ = writeln!(w, "h: {}m", state.height);
        let track_size = num_samples_recorded * core::mem::size_of::<NavigationData>();
        let _ = writeln!(w, "track size: {:?}B", track_size);
        let _ = writeln!(w, "sat_n: {:?}", state.num_satellites);

        if matches!(recording_state, RecordingState::Idle) {
            record_button.render(&mut *ctx.lcd).unwrap();
        } else {
            stop_button.render(&mut *ctx.lcd).unwrap();
        }

        ctx.lcd.present().await;

        match select::select4(
            gps.receive(),
            ctx.button.wait_for_press(),
            touch.wait_for_action(),
            hrm.wait_event(),
        )
        .await
        {
            select::Either4::First(msg) => match msg {
                CasicMsg::NavPv(s) => {
                    state.num_satellites = s.num_sv;
                    state.height = s.height_m;

                    let s: NavigationData = s.into();

                    crate::println!("pv msg: {:?}", s);
                    let r = ref_converter.to_relative_full(&s);
                    state.speed = r.vel.norm();

                    let smooth = kalman.add_value(r.into());

                    state.speed_smooth = smooth.vel.norm();

                    if state.speed_smooth * 3.6 > movement_threshold_km_h {
                        state.distance += r.pos.metric_distance(&state.last_pos);
                        state.distance_smooth += smooth.pos.metric_distance(&state.last_pos_smooth);

                        state.last_pos = r.pos;
                        state.last_pos_smooth = smooth.pos;

                        if let RecordingState::Recording(data) = &mut recording_state {
                            data.add_sampale(s.into(), &mut ctx.flash).await;
                            num_samples_recorded += 1;
                        }
                    }
                }
                _ => {}
            },
            select::Either4::Second(_) => {
                break;
            }
            select::Either4::Third(e) => {
                ctx.backlight.active().await;
                match &mut recording_state {
                    RecordingState::Idle => {
                        if record_button.clicked(&e) {
                            let path = ctx
                                .flash
                                .with_fs(|fs| {
                                    fs.create_dir_all(b"/gps/\0".try_into().unwrap())?;
                                    for i in 0.. {
                                        let path = PathBuf::from(
                                            arrform!(40, "/gps/samples{}.bin", i).as_str(),
                                        );
                                        if fs.metadata(&path)
                                            == Err(littlefs2::io::Error::NoSuchEntry)
                                        {
                                            return Ok(path);
                                        }
                                    }
                                    panic!("Too many recordings");
                                })
                                .await
                                .unwrap();
                            recording_state = RecordingState::Recording(RecordingData {
                                path,
                                samples: [NavigationData::zeroed(); 32],
                                sample: 0,
                            })
                        }
                    }
                    RecordingState::Recording(r) => {
                        if stop_button.clicked(&e) {
                            r.flush(&mut ctx.flash).await;
                            recording_state = RecordingState::Idle;
                        }
                    }
                }
            }
            select::Either4::Fourth(batch) => {
                for sample in batch.1.into_iter().flatten() {
                    if let Some(b) = bpm_detector.add_sample(sample).1 {
                        //crate::println!("Samples ms: {}:", bpm_detector.millis_per_sample());
                        state.bpm = b.0;
                    }
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
