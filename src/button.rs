use embassy_nrf::gpio::{Input, Level, Pull};
use embassy_time::{Duration, Instant, Timer};

use crate::hardware::btn as hw;

const DEBOUNCE_TIME: Duration = Duration::from_millis(10);
pub const PRESSED: Level = Level::Low;
pub const RELEASED: Level = Level::High;

pub struct Button {
    pin: Input<'static, hw::EN>,
    last_state: (Level, Instant),
}

fn other(l: Level) -> Level {
    match l {
        Level::Low => Level::High,
        Level::High => Level::Low,
    }
}
impl Button {
    pub fn new(pin: hw::EN) -> Self {
        Self {
            pin: Input::new(pin, Pull::Up),
            last_state: (RELEASED, Instant::now()),
        }
    }

    fn state(&self) -> Level {
        self.pin.get_level()
    }

    pub async fn wait_for_state(&mut self, l: Level) {
        Timer::at(self.last_state.1 + DEBOUNCE_TIME).await;
        if self.state() != l {
            self.pin.wait_for_any_edge().await;
        }
        self.last_state.0 = l;
        self.last_state.1 = Instant::now();
    }

    pub async fn wait_for_press(&mut self) {
        self.wait_for_state(PRESSED).await
    }

    pub async fn wait_for_release(&mut self) {
        self.wait_for_state(RELEASED).await;
    }

    pub async fn wait_for_change(&mut self) -> Level {
        self.wait_for_state(other(self.last_state.0)).await;
        self.last_state.0
    }
}
