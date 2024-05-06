pub use drivers_shared::buzz::*;

pub struct Buzzer {
    _marker: (),
}

impl Buzzer {
    pub(crate) fn new() -> Self {
        Self { _marker: () }
    }

    pub fn on<'a>(&'a mut self) -> BuzzGuard<'a> {
        send_cmd(BuzzCmd::On);
        BuzzGuard { _inner: self }
    }
}

pub struct BuzzGuard<'a> {
    _inner: &'a mut Buzzer,
}

impl Drop for BuzzGuard<'_> {
    fn drop(&mut self) {
        send_cmd(BuzzCmd::Off);
    }
}

fn send_cmd(cmd: BuzzCmd) {
    println!("Buzz: {:?}", cmd);
}
