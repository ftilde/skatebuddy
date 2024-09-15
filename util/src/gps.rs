use nalgebra::{Rotation3, Vector3};

const EARTH_RADIUS: f64 = 6378137.0;
const CENTER_VEC: Vector3<f64> = Vector3::new(EARTH_RADIUS, 0.0, 0.0);

pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

fn mat_from_lon_lat(LonLat { lon, lat }: LonLat) -> Rotation3<f64> {
    let t_lon = Rotation3::from_scaled_axis(Vector3::new(0.0, 0.0, lon.to_radians()));
    let t_lat = Rotation3::from_scaled_axis(Vector3::new(0.0, -lat.to_radians(), 0.0));
    t_lon * t_lat
}

fn ll_to_3d(ll: LonLat) -> Vector3<f64> {
    mat_from_lon_lat(ll) * CENTER_VEC
}

pub struct RefConverter {
    to_relative: Rotation3<f64>,
}

pub struct RelativePos {
    pub east: f64,
    pub north: f64,
}

impl RefConverter {
    pub fn new(ll: LonLat) -> Self {
        let t_ref_inv = mat_from_lon_lat(ll);
        let to_relative = t_ref_inv.inverse();
        Self { to_relative }
    }

    pub fn to_relative(&self, ll: LonLat) -> RelativePos {
        let pos_3d = ll_to_3d(ll);
        let relative_3d = self.to_relative * pos_3d;
        RelativePos {
            east: relative_3d[1],
            north: relative_3d[2],
        }
    }
}
