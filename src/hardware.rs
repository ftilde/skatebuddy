#![allow(non_camel_case_types)]
#![allow(unused)]

pub mod btn {
    use embassy_nrf::peripherals::*;
    pub type EN = P0_17;
}

pub mod lcd {
    use embassy_nrf::peripherals::*;
    pub type CS = P0_05;
    pub type EXTCOMIN = P0_06;
    pub type DISP = P0_07;
    pub type SCK = P0_26;
    pub type MOSI = P0_27;
    pub type BL = P0_08;
}

pub mod touch {
    use embassy_nrf::peripherals::*;
    pub type SDA = P1_01; //P33
    pub type SCL = P1_02; //P34
    pub type RST = P1_03; //P35
    pub type IRQ = P1_04; //P36
    pub const ADDR: u8 = 0x15;
}

pub mod vibrate {
    use embassy_nrf::peripherals::*;
    pub type EN = P0_19;
}

pub mod gps {
    use embassy_nrf::peripherals::*;
    pub type EN = P0_29;
    pub type RX = P0_30;
    pub type TX = P0_31;
}

pub mod bat {
    use embassy_nrf::peripherals::*;
    pub type CHARGING = P0_23;
    pub type FULL = P0_25;
    pub type VOLTAGE = P0_03;
}

pub mod hr {
    use embassy_nrf::peripherals::*;
    pub type SDA = P0_24;
    pub type SCL = P1_00;
    pub type EN = P0_21;
    pub type INT = P0_22;
    const ADDR: u8 = 0x33;
}

pub mod accel {
    use embassy_nrf::peripherals::*;
    pub type SDA = P1_06; //P38
    pub type SCL = P1_05; //P37
    pub const ADDR: u8 = 0x1e;
}

pub mod mag {
    use embassy_nrf::peripherals::*;
    pub type SDA = P1_12; //P44
    pub type SCL = P1_13; //P45
    pub const ADDR: u8 = 0x0C;
}

pub mod pressure {
    use embassy_nrf::peripherals::*;
    pub type SDA = P1_15; //P47
    pub type SCL = P0_02;
    pub const ADDR: u8 = 0x76;
}

pub mod flash {
    use embassy_nrf::peripherals::*;
    pub type CS = P0_14;
    pub type SCK = P0_16;
    pub type MOSI = P0_15;
    pub type MISO = P0_13;
    pub type UNUSED0 = P1_10; //P42
    pub type UNUSED1 = P1_11; //P43

    //pub type WP = ??;
    //pub type RST = ??;
    pub const SIZE: usize = 4096 * 2048; // 8MB
}
