use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub use drivers_shared::gps::*;
pub struct GPSRessources {}

impl GPSRessources {
    pub async fn on<'a>(&'a mut self) -> GPS<'a> {
        GPS { ressources: self }
    }
}

pub struct GPS<'a> {
    #[allow(unused)]
    ressources: &'a mut GPSRessources,
}

pub struct GPSReceiver<'a> {
    _marker: std::marker::PhantomData<&'a ()>,
    config: CasicMsgConfig,
    initialized: Instant,
    send_queue: VecDeque<CasicMsg>,
}

impl Drop for GPSReceiver<'_> {
    fn drop(&mut self) {
        println!("Drop gps receiver");
    }
}

impl<'a> GPSReceiver<'a> {
    pub async fn new(config: CasicMsgConfig) -> GPSReceiver<'a> {
        GPSReceiver {
            config,
            _marker: Default::default(),
            initialized: Instant::now(),
            send_queue: Default::default(),
        }
    }

    pub async fn update_config(&mut self, config: CasicMsgConfig) {
        self.config = config;
    }

    pub async fn receive(&mut self) -> CasicMsg {
        loop {
            if let Some(m) = self.send_queue.pop_front() {
                return m;
            }

            let elapsed = self.initialized.elapsed();
            let wait_time = 1000 - elapsed.subsec_millis();
            let sec = elapsed.as_secs();
            let time_to_send = |period: u16| period != 0 && (sec % period as u64) == 0;

            smol::Timer::after(Duration::from_millis(wait_time.into())).await;

            let sat_in_view = sec.min(10) as u8;
            let sat_in_fix = sec.saturating_sub(3).min(7) as u8;
            let run_time = sec as u32 * 1000;
            if time_to_send(self.config.nav_pv) {
                self.send_queue.push_back(CasicMsg::NavPv(NavPv {
                    run_time,
                    pos_valid: (sat_in_fix > 0) as u8 * 7,
                    vel_valid: (sat_in_fix > 0) as u8 * 7,
                    system: 1,
                    num_sv: sat_in_fix,
                    num_sv_gps: sat_in_fix,
                    num_sv_bds: 0,
                    num_sv_gln: 0,
                    _reserved: 0,
                    location_dop: 1.0,
                    longitude: 0.0,
                    latitude: 0.0,
                    height_m: 0.0,
                    height_anomaly: 0.0,
                    horizontal_variance: 1.0,
                    vertical_variance: 1.0,
                    north_velocity_m_s: 0.0,
                    east_velocity_m_s: 0.0,
                    heavenly_velocity_m_s: 0.0,
                    speed_3d: 0.0,
                    speed_2d: 0.0,
                    heading: 0.0,
                    variance_speed_2d: 1.0,
                    variance_heading: 1.0,
                }));
            }
            if time_to_send(self.config.nav_time) {
                self.send_queue.push_back(CasicMsg::NavTimeUTC(NavTimeUTC {
                    run_time,
                    t_acc: 0.0,
                    mse: 1.0,
                    ms: 0,
                    year: 0,
                    month: 0,
                    day: 0,
                    hour: 0,
                    min: 0,
                    sec: 0,
                    valid: 0,
                    time_src: 0,
                    date_valid: 0,
                }))
            }
            if time_to_send(self.config.nav_gps_info) {
                self.send_queue.push_back(CasicMsg::NavGpsInfo(NavGpsInfo {
                    run_time,
                    num_view_sv: sat_in_view,
                    num_fix_sv: sat_in_fix,
                    system: 1,
                    _reserved: 0,
                }))
            }
        }
    }
}
