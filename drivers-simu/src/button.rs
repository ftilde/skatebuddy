use crate::time::Duration;

pub enum Level {
    Low,
    High,
}
const PRESSED: Level = Level::Low;
const RELEASED: Level = Level::High;

pub struct Button {}

//fn other(l: Level) -> Level {
//    match l {
//        Level::Low => Level::High,
//        Level::High => Level::Low,
//    }
//}
impl Button {
    pub async fn wait_for_state(&mut self, _l: Level) {
        smol::future::pending().await
    }

    pub async fn wait_for_down(&mut self) {
        self.wait_for_state(PRESSED).await
    }

    pub async fn wait_for_up(&mut self) {
        self.wait_for_state(RELEASED).await;
    }

    pub async fn wait_for_press(&mut self) -> Duration {
        smol::future::pending().await
    }

    //pub async fn wait_for_change(&mut self) -> Level {
    //    self.wait_for_state(other(self.last_state.0)).await;
    //    self.last_state.0
    //}
}
