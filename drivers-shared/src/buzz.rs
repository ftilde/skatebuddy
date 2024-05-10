#[derive(Debug)]
pub enum BuzzCmd {
    On,
    Off,
    Pattern([u8; 7]),
}
