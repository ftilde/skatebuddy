use bitvec::prelude::*;

pub struct Reading {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

pub struct Config {
    pub cntl1: BitArr!(for 8, in u8),
    //cntl2: u8,
    //cntl3: u8,
}

//#[repr(u8)]
//pub enum Range {
//    G2 = 0,
//    G4 = 1,
//    G8 = 2,
//}

impl Config {
    pub fn new() -> Self {
        Config {
            cntl1: bitarr![u8, Lsb0; 0, 0, 0, 0, 0, 0, 0, 1],
        }
    }
    //pub fn high_res(&mut self, set: bool) {
    //    self.cntl1.set(6, set)
    //}
    //pub fn range(&mut self, range: Range) {
    //    let r = range as u8;
    //    self.cntl1.set(3, (r & 1) != 0);
    //    self.cntl1.set(4, (r & 2) != 0);
    //}
}
