use embedded_graphics::prelude::Point;

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, defmt::Format, num_enum::TryFromPrimitive)]
pub enum EventKind {
    Press = 0,
    Release = 1,
    Hold = 2,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, defmt::Format, num_enum::TryFromPrimitive)]
pub enum Gesture {
    None = 0,
    SwipeDown = 1,
    SwipeUp = 2,
    SwipeLeft = 3,
    SwipeRight = 4,
    SinglePress = 5,
    DoublePress = 11,
    LongPress = 12,
}

#[derive(Copy, Clone, Debug, defmt::Format)]
pub struct TouchEvent {
    pub gesture: Gesture,
    pub n_points: u8,
    pub kind: EventKind,
    pub x: u8,
    pub y: u8,
}

impl TouchEvent {
    pub fn point(&self) -> Point {
        Point {
            x: self.x as _,
            y: self.y as _,
        }
    }
}
