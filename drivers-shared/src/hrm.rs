#[derive(defmt::Format)]
pub struct ReadResult {
    pub status: u8,
    pub irq_status: u8,
    pub env_value: [u8; 3],
    pub pre_value: [u8; 2],
    pub ps_value: u8,
    pub pd_res_value: [u8; 3],
    pub current_value: [u8; 3],
}

#[derive(Copy, Clone)]
pub struct PdResConfig {
    pub res: u8,
    res_set: u8,
}

impl PdResConfig {
    pub fn from_reg(reg: u8) -> Self {
        Self {
            res: (reg >> 4) & 0x07,
            res_set: reg & 0x8f,
        }
    }

    pub fn to_reg(&self) -> u8 {
        (self.res & 0x07) << 4 | self.res_set & 0x8f
    }
}
