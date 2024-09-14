#[repr(C, packed)]
#[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CASICPacketHeader {
    pub len: u16,
    pub msg_id: CASICMessageIdentifier,
}

//#[repr(C, packed)]
//#[derive(Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub type CASICMessageIdentifier = [u8; 2];
// { class: u8, number: u8 }

#[derive(Copy, Clone, Debug, defmt::Format)]
pub struct RawCasicMsg<'a> {
    pub id: CASICMessageIdentifier,
    pub payload: &'a [u8],
}

impl<'a> RawCasicMsg<'a> {
    pub fn parse(self) -> CasicMsg {
        match self.id {
            NAV_TIME_UTC => CasicMsg::NavTimeUTC(*bytemuck::from_bytes(self.payload)),
            NAV_PV => CasicMsg::NavPv(*bytemuck::from_bytes(self.payload)),
            NAV_GPS_INFO => CasicMsg::NavGpsInfo(*bytemuck::from_bytes(
                &self.payload[..core::mem::size_of::<NavGpsInfo>()],
            )),
            _ => CasicMsg::Unknown(self.id),
        }
    }
}

pub const NAV_TIME_UTC: CASICMessageIdentifier = [0x01, 0x10];
pub const NAV_PV: CASICMessageIdentifier = [0x01, 0x03];
pub const NAV_GPS_INFO: CASICMessageIdentifier = [0x01, 0x20];
pub const CFG_MSG: CASICMessageIdentifier = [0x06, 0x01];

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, defmt::Format, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CasicMsgConfig {
    pub nav_time: u16,
    pub nav_pv: u16,
    pub nav_gps_info: u16,
}

impl CasicMsgConfig {
    pub fn merge(&self, other: &Self) -> Self {
        CasicMsgConfig {
            nav_time: self.nav_time.max(other.nav_time),
            nav_pv: self.nav_pv.max(other.nav_pv),
            nav_gps_info: self.nav_gps_info.max(other.nav_gps_info),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, defmt::Format, bytemuck::Zeroable, bytemuck::Pod)]
pub struct NavTimeUTC {
    pub run_time: u32,
    pub t_acc: f32,
    pub mse: f32,
    pub ms: u16,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub valid: u8,
    pub time_src: u8,
    pub date_valid: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, defmt::Format, bytemuck::Zeroable, bytemuck::Pod)]
pub struct NavPv {
    pub run_time: u32,
    pub pos_valid: u8,
    pub vel_valid: u8,
    pub system: u8,
    pub num_sv: u8,
    pub num_sv_gps: u8,
    pub num_sv_bds: u8,
    pub num_sv_gln: u8,
    _reserved: u8,
    pub location_dop: u32,
    pub longitude: u64,
    pub latitude: u64,
    pub height_m: u32,
    pub horizontal_anomaly: u32,
    pub horizontal_variance: u32,
    pub vertical_variance: u32,
    pub north_velocity_m_s: u32,
    pub east_velocity_m_s: u32,
    pub heavenly_velocity_m_s: u32,
    pub speed_3d: u32,
    pub speed_2d: u32,
    pub heading: u32,
    pub variance_speed_2d: u32,
    pub variance_heading: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, defmt::Format, bytemuck::Zeroable, bytemuck::Pod)]
pub struct NavGpsInfo {
    pub run_time: u32,
    pub num_view_sv: u8,
    pub num_fix_sv: u8,
    pub system: u8,
    _reserved: u8,
}

#[derive(Clone)]
pub enum CasicMsg {
    NavTimeUTC(NavTimeUTC),
    NavPv(NavPv),
    NavGpsInfo(NavGpsInfo),
    Unknown(CASICMessageIdentifier),
}
