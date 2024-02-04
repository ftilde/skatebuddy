use crate::time::Duration;

pub enum Level {
    Low,
    High,
}
const PRESSED: Level = Level::Low;
const RELEASED: Level = Level::High;

pub struct Button {
    pub window: crate::window::WindowHandle,
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
    pub async fn wait_for_state(&mut self, l: Level) {
        loop {
            let mut window = self.window.lock().await;
            window.window.update();
            let down = window.window.is_key_down(BUTTON_KEY);
            let stop = match l {
                Level::Low => down,
                Level::High => !down,
            };

            if stop {
                return;
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
        loop {
            let mut window = self.window.lock().await;
            window.window.update();

            if window
                .window
                .is_key_pressed(BUTTON_KEY, minifb::KeyRepeat::No)
            {
                return Duration::from_millis(10); //TODO
            }

            smol::Timer::after(POLL_PERIOD).await;
        }
    }

    //pub async fn wait_for_change(&mut self) -> Level {
    //    self.wait_for_state(other(self.last_state.0)).await;
    //    self.last_state.0
    //}
}
