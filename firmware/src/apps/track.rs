use core::fmt::Write;
use drivers::gps::CasicMsg;
use drivers::lpm013m1126c::Rgb111;
use drivers::{futures::select, gps::CasicMsgConfig};
use embedded_graphics::mono_font::MonoTextStyle;

use crate::{render_top_bar, ui::TextWriter, Context};

pub async fn track(ctx: &mut Context) {
    let font = &embedded_graphics::mono_font::ascii::FONT_8X13;
    //let sl = TextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);
    let sl = MonoTextStyle::new(font, embedded_graphics::pixelcolor::BinaryColor::On);

    //let mut touch = ctx.touch.enabled(&mut ctx.twi0).await;

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

        ctx.lcd.present().await;

        match select::select(gps.receive(), ctx.button.wait_for_press()).await {
            select::Either::First(msg) => match msg {
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
                }
                CasicMsg::NavGpsInfo(i) => {
                    crate::println!("gps msg: {:?}", i);
                    state.num_satellites_in_view = i.num_view_sv;
                    state.num_satellites_in_fix = i.num_fix_sv;
                }
                _ => {}
            },
            select::Either::Second(_) => {
                break;
            }
        }
    }
}
