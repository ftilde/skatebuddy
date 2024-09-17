use std::error::Error;
use std::io::Write;

use drivers_shared::gps::NavigationData;
use plotpy::{Curve, Plot};
use util::gps::{diag, KalmanFilter, LonLat, RelativePos};

fn plot_values(vals: &[(f32, f32)], equal: bool) -> Result<(), Box<dyn Error>> {
    let mut curve = Curve::new();
    curve.set_line_width(2.0);

    curve.points_begin();
    for (x, y) in vals {
        curve.points_add(x, y);
    }
    curve.points_end();

    let mut plot = Plot::new();
    plot.add(&curve)
        .set_equal_axes(equal)
        .grid_and_labels("x", "y");

    if let Err(e) = plot.show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}
fn plot_values_multiple(vals: &[&[(f32, f32)]], equal: bool) -> Result<(), Box<dyn Error>> {
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
        .set_equal_axes(equal)
        .show("out.svg")
    {
        println!("{}", e);
    }

    Ok(())
}

fn read_file(name: &str) -> Vec<NavigationData> {
    let file = std::fs::File::open(name).unwrap();

    let file = unsafe { memmap::Mmap::map(&file).unwrap() };

    let entries: &[drivers_shared::gps::NavigationData] = bytemuck::cast_slice(&*file);

    entries.to_vec()
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    let files = args[1..].iter().map(|n| read_file(n));
    let entries: Vec<NavigationData> = files.flatten().collect();

    let mut positions = Vec::new();
    let mut positions_filtered = Vec::new();

    let mut acc_x = 0.0;
    let mut acc_y = 0.0;
    let mut acc_positions = Vec::new();
    let mut speeds = Vec::new();
    let mut speeds_filtered = Vec::new();
    let converter = util::gps::RefConverter::new(LonLat {
        lon: entries[0].longitude,
        lat: entries[0].latitude,
    });

    let mut kalman_filter = KalmanFilter::new();
    for pv in &entries {
        let p = converter.to_relative_full(pv);
        positions.push((p.pos.x, p.pos.y));
        acc_x += pv.east_velocity_m_s;
        acc_y += pv.north_velocity_m_s;
        acc_positions.push((acc_x as f32, acc_y as f32));
        let ground_speed = diag(pv.north_velocity_m_s, pv.east_velocity_m_s);
        speeds.push((pv.run_time as f32 / 1000.0, ground_speed));

        let filtered = kalman_filter.add_value(p);
        positions_filtered.push((filtered.pos.x, filtered.pos.y));

        let ground_speed = filtered.vel.norm();
        speeds_filtered.push((pv.run_time as f32 / 1000.0, ground_speed));
    }

    for (name, track, speeds) in [
        ("track.csv", &positions, &speeds),
        ("track_smooth.csv", &positions_filtered, &speeds_filtered),
    ] {
        let mut file = std::fs::File::create(name).unwrap();
        writeln!(&mut file, "latitude,longitude,speed").unwrap();
        for (p, s) in track.iter().zip(speeds.iter()) {
            let ll = converter.to_lon_lat(RelativePos {
                east: p.0 as f64,
                north: p.1 as f64,
            });
            writeln!(&mut file, "{},{},{}", ll.lat, ll.lon, s.1).unwrap();
        }
    }

    plot_values_multiple(
        &[positions.as_slice(), positions_filtered.as_slice()],
        false,
    )
    .unwrap();
    plot_values_multiple(&[speeds.as_slice(), speeds_filtered.as_slice()], false).unwrap();
    plot_values(acc_positions.as_slice(), true).unwrap();
    plot_values(positions.as_slice(), true).unwrap();
}
