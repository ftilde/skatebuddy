use crate::{
    time::{Duration, Instant},
    window::WindowHandle,
};

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum Level {
    Low,
    High,
}
const PRESSED: Level = Level::Low;
const RELEASED: Level = Level::High;

pub struct Button {
    pub window: WindowHandle,
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

const BUTTON_KEY: minifb::Key = minifb::Key::Space;
const POLL_PERIOD: Duration = Duration::from_millis(10);

impl Button {
    pub fn new(window: WindowHandle) -> Self {
        let now = Instant::now();
        Self {
            window,
            last_state: (RELEASED, now),
            last_press: now,
            last_release: now,
        }
    }
    pub fn state(&mut self) -> Level {
        let mut window = self.window.lock().unwrap();
        window.window.update();
        let down = window.window.is_key_down(BUTTON_KEY);
        match down {
            true => Level::Low,
            false => Level::High,
        }
    }

    pub async fn wait_for_state(&mut self, l: Level) {
        loop {
            {
                let mut window = self.window.lock().unwrap();
                window.window.update();
                let down = window.window.is_key_down(BUTTON_KEY);
                let stop = match l {
                    Level::Low => down,
                    Level::High => !down,
                };

                let now = Instant::now();
                self.last_state = (
                    match down {
                        true => Level::Low,
                        false => Level::High,
                    },
                    now,
                );
                if stop {
                    match l {
                        Level::Low => self.last_press = now,
                        Level::High => self.last_release = now,
                    }
                    return;
                }
            }

            smol::Timer::after(POLL_PERIOD).await;
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
