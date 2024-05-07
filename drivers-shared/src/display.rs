pub const DEFAULT_ACTIVE_DURATION: u32 = 10;
pub enum BacklightCmd {
    ActiveFor { secs: u32 },
    Off,
    On,
}
