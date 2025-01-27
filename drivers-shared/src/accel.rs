use modular_bitfield::prelude::*;

#[derive(Default, Copy, Clone, defmt::Format)]
pub struct Reading {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

#[derive(Copy, Clone)]
pub struct Config {
    pub cntl1: Cntl1,
    //cntl2: u8,
    //cntl3: u8,
    pub buf_cntl2: BufCntl2,
}

#[derive(Copy, Clone)]
#[bitfield]
pub struct Cntl1 {
    pub tpe: B1,
    pub wufe: B1,
    pub tdte: B1,
    pub gsel: B2,
    pub drdye: B1,
    pub res: B1,
    pub pc1: B1,
}

#[derive(Copy, Clone)]
#[bitfield]
pub struct BufCntl2 {
    pub mode: BufMode,
    pub reserved: B3,
    pub full_interupt_enabled: B1,
    pub resolution: BufRes,
    pub enabled: B1,
}

//#[repr(u8)]
//pub enum Range {
//    G2 = 0,
//    G4 = 1,
//    G8 = 2,
//}

#[derive(Copy, Clone, BitfieldSpecifier)]
pub enum BufRes {
    Bit8 = 0b0,
    Bit16 = 0b1,
}

#[derive(Copy, Clone, BitfieldSpecifier)]
pub enum BufMode {
    Fifo = 0b00,
    Stream = 0b01,
    Trigger = 0b10,
    Filo = 0b11,
}

impl Config {
    pub fn new() -> Self {
        Config {
            cntl1: Cntl1::new().with_pc1(1),
            buf_cntl2: BufCntl2::new(),
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

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_bitfield() {
        let cntrl1 = Cntl1::new().with_pc1(1);
        assert_eq!(cntrl1.bytes, [0b10000000]);
    }
}
