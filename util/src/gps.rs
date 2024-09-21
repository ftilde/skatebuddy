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
    to_lon_lat: Rotation3<f64>,
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
        Self {
            to_relative,
            to_lon_lat: t_ref_inv,
        }
    }

    pub fn to_relative(&self, ll: LonLat) -> Vector2<f32> {
        let pos_3d = ll_to_3d(ll);
        let relative_3d = self.to_relative * pos_3d;
        Vector2::new(relative_3d[1] as f32, relative_3d[2] as f32)
    }

    pub fn to_lon_lat(&self, p: RelativePos) -> LonLat {
        let c = Vector3::new(EARTH_RADIUS, p.east, p.north).normalize();
        let c = self.to_lon_lat * c;
        let on_equator = Vector3::new(c[0], c[1], 0.0).normalize();
        let lon = libm::acos(on_equator[0]).to_degrees();
        let lat = libm::acos(on_equator.dot(&c)).to_degrees();

        LonLat { lon, lat }
    }

    pub fn to_relative_full(&self, p: &NavigationData) -> RelativeNavigationData {
        let pos = self.to_relative(LonLat {
            lon: p.longitude,
            lat: p.latitude,
        });
        let vel = Vector2::new(p.east_velocity_m_s, p.north_velocity_m_s);

        RelativeNavigationData {
            run_time: p.run_time,
            height_anomaly: p.height_anomaly,
            pos,
            vel,
            horizontal_variance: p.horizontal_variance,
            vertical_variance: p.vertical_variance,
            variance_speed_2d: p.variance_speed_2d,
            heavenly_velocity_m_s: p.heavenly_velocity_m_s,
        }
    }
}

#[derive(Default)]
pub struct LazyRefConverter(Option<RefConverter>);

impl LazyRefConverter {
    pub fn to_relative(&mut self, ll: LonLat) -> Vector2<f32> {
        if let Some(state) = &mut self.0 {
            state.to_relative(ll)
        } else {
            self.0 = Some(RefConverter::new(ll));
            Vector2::new(0.0, 0.0)
        }
    }

    pub fn to_relative_full(&mut self, p: &NavigationData) -> RelativeNavigationData {
        let pos = self.to_relative(LonLat {
            lon: p.longitude,
            lat: p.latitude,
        });
        let vel = Vector2::new(p.east_velocity_m_s, p.north_velocity_m_s);

        RelativeNavigationData {
            run_time: p.run_time,
            height_anomaly: p.height_anomaly,
            pos,
            vel,
            horizontal_variance: p.horizontal_variance,
            vertical_variance: p.vertical_variance,
            variance_speed_2d: p.variance_speed_2d,
            heavenly_velocity_m_s: p.heavenly_velocity_m_s,
        }
    }
}

#[derive(Copy, Clone)]
pub struct RelativeNavigationData {
    pub run_time: u32,
    pub height_anomaly: f32,
    pub pos: nalgebra::Vector2<f32>,
    pub horizontal_variance: f32,
    pub vertical_variance: f32,
    pub vel: nalgebra::Vector2<f32>,
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

pub fn diag(x: f32, y: f32) -> f32 {
    libm::sqrtf(x * x + y * y)
}

pub struct State {
    pub pos: nalgebra::Vector2<f32>,
    pub vel: nalgebra::Vector2<f32>,
}

//fn positive_semidefinite(mat: &Matrix4<f32>) -> bool {
//    // Using https://en.wikipedia.org/wiki/Sylvester's_criterion
//    mat.fixed_view::<1, 1>(0, 0).determinant() >= 0.0
//        && mat.fixed_view::<2, 2>(0, 0).determinant() >= 0.0
//        && mat.fixed_view::<3, 3>(0, 0).determinant() >= 0.0
//        && mat.determinant() >= 0.0
//}

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
            let Q2 = Matrix2::from_fn(|r, c| G[r] * G[c]) * square(avg_a);
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

            assert!(measurement.horizontal_variance > 0.0);
            assert!(measurement.variance_speed_2d > 0.0);

            let R_k = Matrix4::from_diagonal(&R_k_diag);

            let observation = Vector4::new(
                measurement.pos[0],
                measurement.pos[1],
                measurement.vel[0],
                measurement.vel[1],
            );

            //Correction
            let y_k = observation - kalman_state.state_a_priori;
            let S_k = kalman_state.p_a_priori + R_k;
            let Some(S_k_inv) = S_k.try_inverse() else {
                return self.init_state(measurement);
            };
            let K_k = kalman_state.p_a_priori * S_k_inv;
            let state = kalman_state.state_a_priori + K_k * y_k;
            let P = (Matrix4::identity() - K_k) * kalman_state.p_a_priori;

            //prediction
            kalman_state.state_a_priori = F * state;
            kalman_state.p_a_priori = (F * P * F.transpose()) + Q;
            //assert!(
            //    positive_semidefinite(&kalman_state.p_a_priori),
            //    "apriori: {:#?}\nQ: {:#?}\nP: {:#?}\n F: {:#?}\nK_k {:#?}",
            //    kalman_state.p_a_priori,
            //    Q,
            //    P,
            //    F,
            //    K_k,
            //);
            //assert!(
            //    kalman_state.p_a_priori.is_invertible(),
            //    "apriori: {:#?}\nQ: {:#?}\nP: {:#?}\n F: {:#?}\nK_k {:#?}",
            //    kalman_state.p_a_priori,
            //    Q,
            //    P,
            //    F,
            //    K_k,
            //);

            State {
                pos: Vector2::new(state[0], state[1]),
                vel: Vector2::new(state[2], state[3]),
            }
        } else {
            self.init_state(measurement)
        }
    }

    pub fn init_state(&mut self, measurement: RelativeNavigationData) -> State {
        let variance_diag = Vector4::new(
            measurement.horizontal_variance,
            measurement.horizontal_variance,
            measurement.variance_speed_2d,
            measurement.variance_speed_2d,
        );

        let state = Vector4::new(
            measurement.pos[0],
            measurement.pos[1],
            measurement.vel[0],
            measurement.vel[1],
        );

        self.state = Some(KalmanFilterState {
            state_a_priori: state,
            p_a_priori: Matrix4::from_diagonal(&variance_diag),
            t_in_s: measurement.run_time as f32 / 1000.0,
        });
        State {
            pos: Vector2::new(state[0], state[1]),
            vel: Vector2::new(state[2], state[3]),
        }
    }
}
