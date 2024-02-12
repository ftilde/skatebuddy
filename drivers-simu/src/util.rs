use minifb::Key;

use crate::window::Window;

#[derive(Copy, Clone)]
enum KeyPosition {
    Up,
    Down,
}

pub struct KeyState {
    pos: KeyPosition,
    key: Key,
}

impl KeyState {
    pub fn new(key: Key) -> Self {
        Self {
            pos: KeyPosition::Up,
            key,
        }
    }
    pub fn pressed(&mut self, window: &Window) -> bool {
        let new_pos = if window.window.is_key_down(self.key) {
            KeyPosition::Down
        } else {
            KeyPosition::Up
        };
        let old_pos = self.pos;
        self.pos = new_pos;
        match (old_pos, new_pos) {
            (KeyPosition::Down, KeyPosition::Up) => true,
            _ => false,
        }
    }
}
