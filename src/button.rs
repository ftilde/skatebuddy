use embassy_nrf::gpio::{Input, Level, Pull};
use embassy_time::{Duration, Instant, Timer};

use crate::hardware::btn as hw;

const DEBOUNCE_TIME: Duration = Duration::from_millis(10);
pub const PRESSED: Level = Level::Low;
pub const RELEASED: Level = Level::High;

pub struct Button {
    pin: Input<'static, hw::EN>,
    last_state: (Level, Instant),
    last_press: Instant,
    last_release: Instant,
}

fn other(l: Level) -> Level {
    match l {
        Level::Low => Level::High,
        Level::High => Level::Low,
    }
}
impl Button {
    pub fn new(pin: hw::EN) -> Self {
        let now = Instant::now();
        Self {
            pin: Input::new(pin, Pull::Up),
            last_state: (RELEASED, now),
            last_press: now,
            last_release: now,
        }
    }

    fn state(&self) -> Level {
        self.pin.get_level()
    }

    pub async fn wait_for_state(&mut self, l: Level) {
        let in_debounce = self.last_state.1.elapsed() < DEBOUNCE_TIME;
        let debounced_state = if in_debounce {
            self.last_state.0
        } else {
            self.state()
        };

        if debounced_state != l {
            if in_debounce {
                Timer::at(self.last_state.1 + DEBOUNCE_TIME).await;
            }
            self.pin.wait_for_any_edge().await;

            let now = Instant::now();
            match l {
                PRESSED => self.last_press = now,
                RELEASED => self.last_release = now,
            }
            self.last_state.0 = l;
            self.last_state.1 = now
        }
    }

    pub async fn wait_for_down(&mut self) {
        self.wait_for_state(PRESSED).await
    }

    pub async fn wait_for_up(&mut self) {
        self.wait_for_state(RELEASED).await;
    }

    pub async fn wait_for_press(&mut self) -> Duration {
        self.wait_for_down().await;
        self.wait_for_up().await;
        self.last_press.elapsed()
    }

    pub async fn wait_for_change(&mut self) -> Level {
        self.wait_for_state(other(self.last_state.0)).await;
        self.last_state.0
    }
}
