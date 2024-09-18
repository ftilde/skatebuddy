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
    replay_naviation_data: Vec<NavigationData>,
    replay_naviation_data_index: usize,
}

impl Drop for GPSReceiver<'_> {
    fn drop(&mut self) {
        println!("Drop gps receiver");
    }
}

fn try_read_replay_data() -> Result<Vec<NavigationData>, &'static str> {
    let file_name =
        std::env::var("REPLAY_NAVIGATION_DATA").map_err(|_| "No env var REPLAY_NAVIGATION_DATA")?;

    let file = std::fs::File::open(file_name).map_err(|_| "Failed to open navigation data file")?;

    let file = unsafe { memmap::Mmap::map(&file).unwrap() };

    let entries: &[drivers_shared::gps::NavigationData] = bytemuck::cast_slice(&*file);

    Ok(entries.to_vec())
}

impl<'a> GPSReceiver<'a> {
    pub async fn new(config: CasicMsgConfig) -> GPSReceiver<'a> {
        let replay_naviation_data = match try_read_replay_data() {
            Ok(d) => d,
            Err(e) => {
                eprint!("Error reading replay data: {}", e);
                Vec::new()
            }
        };

        GPSReceiver {
            config,
            _marker: Default::default(),
            initialized: Instant::now(),
            send_queue: Default::default(),
            replay_naviation_data,
            replay_naviation_data_index: 0,
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
                if let Some(d) = self
                    .replay_naviation_data
                    .get(self.replay_naviation_data_index)
                {
                    self.replay_naviation_data_index += 1;
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
                        longitude: d.longitude,
                        latitude: d.latitude,
                        height_m: 0.0,
                        height_anomaly: d.height_anomaly,
                        horizontal_variance: d.horizontal_variance,
                        vertical_variance: d.vertical_variance,
                        north_velocity_m_s: d.north_velocity_m_s,
                        east_velocity_m_s: d.north_velocity_m_s,
                        heavenly_velocity_m_s: d.heavenly_velocity_m_s,
                        speed_3d: 0.0,
                        speed_2d: 0.0,
                        heading: 0.0,
                        variance_speed_2d: 1.0,
                        variance_heading: 1.0,
                    }));
                } else {
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
                        longitude: sec as f64 / 1000.0,
                        latitude: sec as f64 / 1000.0,
                        height_m: 0.0,
                        height_anomaly: 0.0,
                        horizontal_variance: 1.0,
                        vertical_variance: 1.0,
                        north_velocity_m_s: 1.0,
                        east_velocity_m_s: 1.0,
                        heavenly_velocity_m_s: 0.0,
                        speed_3d: 0.0,
                        speed_2d: 0.0,
                        heading: 0.0,
                        variance_speed_2d: 1.0,
                        variance_heading: 1.0,
                    }));
                }
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
