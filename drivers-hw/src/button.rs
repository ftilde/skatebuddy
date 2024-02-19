use embassy_nrf::gpio::{Input, Pull};
use embassy_time::{Duration, Instant, Timer};

use super::hardware::btn as hw;

pub use embassy_nrf::gpio::Level;

const DEBOUNCE_TIME: Duration = Duration::from_millis(10);
const PRESSED: Level = Level::Low;
const RELEASED: Level = Level::High;

pub struct Button {
    pin: Input<'static, hw::EN>,
    last_state: (Level, Instant),
    last_press: Instant,
    last_release: Instant,
}

//fn other(l: Level) -> Level {
//    match l {
//        Level::Low => Level::High,
//        Level::High => Level::Low,
//    }
//}
impl Button {
    pub(crate) fn new(pin: hw::EN) -> Self {
        let now = Instant::now();
        Self {
            pin: Input::new(pin, Pull::Up),
            last_state: (RELEASED, now),
            last_press: now,
            last_release: now,
        }
    }

    pub fn state(&mut self) -> Level {
        self.pin.get_level()
    }

    fn set_last_state(&mut self, state: Level) {
        let now = Instant::now();
        match state {
            PRESSED => self.last_press = now,
            RELEASED => self.last_release = now,
        }
        self.last_state.0 = state;
        self.last_state.1 = now;
    }

    pub async fn wait_for_state(&mut self, l: Level) {
        let mut in_debounce = self.last_state.1.elapsed() < DEBOUNCE_TIME;
        let debounced_state = if in_debounce {
            self.last_state.0
        } else {
            let current_state = self.state();

            if self.last_state.0 != current_state {
                self.set_last_state(current_state);
                in_debounce = true;
            }

            current_state
        };

        if debounced_state != l {
            if in_debounce {
                Timer::at(self.last_state.1 + DEBOUNCE_TIME).await;
            }
            self.pin.wait_for_any_edge().await;

            // Previously, the state was no l, but we detected an edge, so the state must have been
            // in l either before or now. Either way, we waited for l, and we assume the last
            // measured state was l as well. If it isn't, then it will be detected on the next
            // call.

            self.set_last_state(l);
        }
    }

    pub async fn wait_for_down(&mut self) {
        self.wait_for_state(PRESSED).await
    }

    pub async fn wait_for_up(&mut self) {
        self.wait_for_state(RELEASED).await;
    }

    pub async fn wait_for_press(&mut self) -> Duration {
        if self.last_state.0 != PRESSED {
            self.wait_for_down().await;
        }
        self.wait_for_up().await;
        self.last_press.elapsed()
    }

    //pub async fn wait_for_change(&mut self) -> Level {
    //    self.wait_for_state(other(self.last_state.0)).await;
    //    self.last_state.0
    //}
}
