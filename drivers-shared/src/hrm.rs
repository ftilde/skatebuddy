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
