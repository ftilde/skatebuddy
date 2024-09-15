use std::error::Error;

use drivers_shared::gps::NavPv;
use plotpy::{Curve, Plot};
use util::gps::LonLat;

fn plot_values(vals: &[(f32, f32)]) -> Result<(), Box<dyn Error>> {
    let mut curve = Curve::new();
    curve.set_line_width(2.0);

    curve.points_begin();
    for (x, y) in vals {
        curve.points_add(x, y);
    }
    curve.points_end();

    let mut plot = Plot::new();
    plot.add(&curve)
        .set_equal_axes(true)
        .grid_and_labels("x", "y");

    if let Err(e) = plot.show("out.svg") {
        println!("{}", e);
    }

    Ok(())
}

fn read_file(name: &str) -> Vec<NavPv> {
    let file = std::fs::File::open(name).unwrap();

    let file = unsafe { memmap::Mmap::map(&file).unwrap() };

    let entries: &[drivers_shared::gps::NavPv] = bytemuck::cast_slice(&*file);

    entries.to_vec()
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    let files = args[1..].iter().map(|n| read_file(n));
    let entries: Vec<NavPv> = files.flatten().collect();

    let mut values = Vec::new();
    let converter = util::gps::RefConverter::new(LonLat {
        lon: entries[0].longitude,
        lat: entries[0].latitude,
    });
    for pv in entries {
        let p = converter.to_relative(LonLat {
            lon: pv.longitude,
            lat: pv.latitude,
        });
        values.push((p.east as f32, p.north as f32));
        println!("PV: {:?}", pv);
    }

    plot_values(values.as_slice()).unwrap();
}
