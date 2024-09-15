use drivers_shared::gps::NavigationData;
use nalgebra::{Matrix2, Matrix4, Rotation3, Vector2, Vector3, Vector4};

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

    pub fn to_relative_full(&self, p: &NavigationData) -> RelativeNavigationData {
        let rel_pos = self.to_relative(LonLat {
            lon: p.longitude,
            lat: p.latitude,
        });

        RelativeNavigationData {
            run_time: p.run_time,
            height_anomaly: p.height_anomaly,
            pos_north: rel_pos.north as f32,
            pos_east: rel_pos.east as f32,
            horizontal_variance: p.horizontal_variance,
            vertical_variance: p.vertical_variance,
            north_velocity_m_s: p.north_velocity_m_s,
            east_velocity_m_s: p.east_velocity_m_s,
            variance_speed_2d: p.variance_speed_2d,
            heavenly_velocity_m_s: p.heavenly_velocity_m_s,
        }
    }
}

pub struct RelativeNavigationData {
    pub run_time: u32,
    pub height_anomaly: f32,
    pub pos_north: f32,
    pub pos_east: f32,
    pub horizontal_variance: f32,
    pub vertical_variance: f32,
    pub north_velocity_m_s: f32,
    pub east_velocity_m_s: f32,
    pub variance_speed_2d: f32,
    pub heavenly_velocity_m_s: f32,
}

struct KalmanFilterState {
    state_a_priori: Vector4<f32>,
    p_a_priori: Matrix4<f32>,
    t_in_s: f32,
}

pub struct KalmanFilter {
    state: Option<KalmanFilterState>,
}

fn square(v: f32) -> f32 {
    v * v
}

pub struct State {
    pub pos_east: f32,
    pub pos_north: f32,
    pub vel_east: f32,
    pub vel_north: f32,
}

impl KalmanFilter {
    pub fn new() -> Self {
        Self { state: None }
    }

    #[allow(non_snake_case)]
    pub fn add_value(&mut self, measurement: RelativeNavigationData) -> State {
        if let Some(kalman_state) = &mut self.state {
            let avg_g = 0.1;
            let avg_a = avg_g * 9.81;

            let nt = measurement.run_time as f32 / 1000.0;
            let dt = nt - kalman_state.t_in_s;
            assert!(dt > 0.0);
            kalman_state.t_in_s = nt;

            let G = Vector2::new(0.5 * dt * dt, dt);
            //G = np.array([dt, dt, 0.5*dt*dt, 0.5*dt*dt])
            let Q2 = Matrix2::from_fn(|r, c| G[r] * G[c]) * square(avg_a * dt);
            let mut Q = Matrix4::zeros();
            for d in [0, 1] {
                for i in [0, 1] {
                    for j in [0, 1] {
                        Q[(2 * i + d, 2 * j + d)] = Q2[(i, j)];
                    }
                }
            }

            let mut F = Matrix4::identity();
            F[(0, 2)] = dt;
            F[(1, 3)] = dt;

            let R_k_diag = Vector4::new(
                measurement.horizontal_variance,
                measurement.horizontal_variance,
                measurement.variance_speed_2d,
                measurement.variance_speed_2d,
            );

            let R_k = Matrix4::from_diagonal(&R_k_diag);

            let observation = Vector4::new(
                measurement.pos_east,
                measurement.pos_north,
                measurement.east_velocity_m_s,
                measurement.north_velocity_m_s,
            );

            //Correction
            let y_k = observation - kalman_state.state_a_priori;
            let S_k = kalman_state.p_a_priori + R_k;
            let K_k = kalman_state.p_a_priori * S_k.try_inverse().unwrap();

            let state = kalman_state.state_a_priori + K_k * y_k;
            let P = (Matrix4::identity() - K_k) * kalman_state.p_a_priori;

            //prediction
            kalman_state.state_a_priori = F * state;
            kalman_state.p_a_priori = (F * P * F.transpose()) + Q;

            State {
                pos_east: state[0],
                pos_north: state[1],
                vel_east: state[2],
                vel_north: state[3],
            }
        } else {
            let variance_diag = Vector4::new(
                measurement.horizontal_variance,
                measurement.horizontal_variance,
                measurement.variance_speed_2d,
                measurement.variance_speed_2d,
            );

            let state = Vector4::new(
                measurement.pos_east,
                measurement.pos_north,
                measurement.east_velocity_m_s,
                measurement.north_velocity_m_s,
            );

            self.state = Some(KalmanFilterState {
                state_a_priori: state,
                p_a_priori: Matrix4::from_diagonal(&variance_diag),
                t_in_s: measurement.run_time as f32 / 1000.0,
            });
            State {
                pos_east: state[0],
                pos_north: state[1],
                vel_east: state[2],
                vel_north: state[3],
            }
        }
    }
}
