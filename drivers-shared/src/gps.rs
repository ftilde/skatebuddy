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
    pub fn parse(self) -> CasicMsg<'a> {
        match self.id {
            NAV_TIME_UTC => CasicMsg::NavTimeUTC(*bytemuck::from_bytes(self.payload)),
            _ => CasicMsg::Unknown(self),
        }
    }
}

pub const NAV_TIME_UTC: CASICMessageIdentifier = [0x01, 0x10];
pub const CFG_MSG: CASICMessageIdentifier = [0x06, 0x01];

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

pub enum CasicMsg<'a> {
    NavTimeUTC(NavTimeUTC),
    Unknown(RawCasicMsg<'a>),
}
