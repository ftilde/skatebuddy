pub use drivers_shared::buzz::*;

pub struct Buzzer {
    _marker: (),
}

impl Buzzer {
    pub(crate) fn new() -> Self {
        Self { _marker: () }
    }

    pub fn on<'a>(&'a mut self) -> BuzzHandle<'a> {
        send_cmd(BuzzCmd::On);
        BuzzHandle { _inner: self }
    }
}

pub struct BuzzHandle<'a> {
    _inner: &'a mut Buzzer,
}

impl BuzzHandle<'_> {
    pub fn on(&mut self) {
        send_cmd(BuzzCmd::On);
    }

    pub fn off(&mut self) {
        send_cmd(BuzzCmd::On);
    }

    pub fn pattern(&mut self, pat: [u8; 7]) {
        send_cmd(BuzzCmd::Pattern(pat));
    }
}

impl Drop for BuzzHandle<'_> {
    fn drop(&mut self) {
        send_cmd(BuzzCmd::Off);
    }
}

fn send_cmd(cmd: BuzzCmd) {
    println!("Buzz: {:?}", cmd);
}
