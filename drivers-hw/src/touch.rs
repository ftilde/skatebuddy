use super::hardware::touch as hw;
use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    twim,
};
use embassy_time::{Duration, Timer};

pub use drivers_shared::touch::*;

pub struct TouchRessources {
    scl: hw::SCL,
    sda: hw::SDA,
    reset: Output<'static, hw::RST>,
    irq: Input<'static, hw::IRQ>,
}

type I2CInstance = embassy_nrf::peripherals::TWISPI0;

// From espruino. not documented anywhere else, afaik, though...
const CMD_SLEEP: [u8; 2] = [0xE5, 0x03];

// This is sleep mode according to official example code (but looks like standby???
const CMD_STANDBY: [u8; 2] = [0xA5, 0x03];

const CMD_READ_EVENT: [u8; 1] = [0x01];

//const TOUCH_AREA_HEIGHT: usize = 160;
//const TOUCH_AREA_WIDTH: usize = 160;

impl TouchRessources {
    pub(crate) async fn new(
        sda: hw::SDA,
        scl: hw::SCL,
        reset: hw::RST,
        irq: hw::IRQ,
        i2c: &mut I2CInstance,
    ) -> Self {
        let mut ret = Self {
            sda,
            scl,
            reset: Output::new(reset, Level::Low, OutputDrive::Standard),
            irq: Input::new(irq, Pull::None),
        };

        {
            // Turn on once to activate sleep mode
            let _ = ret.enabled(i2c);
        }

        ret
    }

    pub async fn enabled<'a>(&'a mut self, i2c: &'a mut I2CInstance) -> Touch<'a> {
        // These reset durations are the same that espruino uses, so hopefully this works out.
        self.reset.set_low();
        Timer::after(Duration::from_millis(1)).await;
        self.reset.set_high();
        Timer::after(Duration::from_millis(1)).await;

        Touch {
            hw: self,
            instance: i2c,
            state: State::WaitInterrupt,
        }
    }
}

pub struct Touch<'a> {
    hw: &'a mut TouchRessources,
    instance: &'a mut I2CInstance,
    state: State,
}

fn build_i2c<'a>(
    sda: &'a mut hw::SDA,
    scl: &'a mut hw::SCL,
    instance: &'a mut I2CInstance,
) -> twim::Twim<'a, I2CInstance> {
    let i2c_conf = twim::Config::default();
    // TODO: check if this is allowed
    //i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

    twim::Twim::new(instance, crate::Irqs, sda, scl, i2c_conf)
}

impl<'a> Drop for Touch<'a> {
    fn drop(&mut self) {
        let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);
        i2c.blocking_write(hw::ADDR, &CMD_SLEEP).unwrap();
    }
}

#[derive(Copy, Clone)]
enum State {
    WaitInterrupt,
    Read,
    GoStandby,
}

impl<'a> Touch<'a> {
    pub async fn wait_for_event(&mut self) -> TouchEvent {
        loop {
            match self.state {
                State::WaitInterrupt => {
                    self.hw.irq.wait_for_low().await;
                    self.state = State::Read;
                }
                State::Read => {
                    let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);

                    let mut buf = [0u8; 6];
                    i2c.write_read(hw::ADDR, &CMD_READ_EVENT, &mut buf)
                        .await
                        .unwrap();

                    let kind = EventKind::try_from(buf[2] >> 6).unwrap();

                    let n_points = buf[1];
                    let gesture = Gesture::try_from(buf[0]).unwrap();

                    // Espruino adjusts these, but it seems like the range is actually [0, 175]?
                    //let x = (buf[3] as usize * crate::lpm013m1126c::WIDTH / TOUCH_AREA_WIDTH) as u8;
                    let x = buf[3];
                    //let y = (buf[5] as usize * crate::lpm013m1126c::HEIGHT / TOUCH_AREA_HEIGHT) as u8;
                    let y = buf[5];

                    let event = TouchEvent {
                        gesture,
                        n_points,
                        kind,
                        x,
                        y,
                    };
                    if let EventKind::Release = kind {
                        self.state = State::GoStandby;
                    } else {
                        self.state = State::Read;
                    }

                    return event;
                }
                State::GoStandby => {
                    let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);
                    i2c.write(hw::ADDR, &CMD_STANDBY).await.unwrap();
                    self.state = State::WaitInterrupt;
                }
            }
        }
    }
    //pub async fn wait_for_event(&mut self) -> TouchEvent {
    //    if let Mode::Standby = self.mode {
    //        self.hw.irq.wait_for_low().await;
    //    }
    //    self.mode = Mode::Dynamic;

    //    let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);

    //    let mut buf = [0u8; 6];
    //    i2c.write_read(hw::ADDR, &CMD_READ_EVENT, &mut buf)
    //        .await
    //        .unwrap();

    //    let kind = EventKind::try_from(buf[2] >> 6).unwrap();

    //    let n_points = buf[1];
    //    let gesture = Gesture::try_from(buf[0]).unwrap();

    //    // Espruino adjusts these, but it seems like the range is actually [0, 175]?
    //    //let x = (buf[3] as usize * crate::lpm013m1126c::WIDTH / TOUCH_AREA_WIDTH) as u8;
    //    let x = buf[3];
    //    //let y = (buf[5] as usize * crate::lpm013m1126c::HEIGHT / TOUCH_AREA_HEIGHT) as u8;
    //    let y = buf[5];

    //    let event = TouchEvent {
    //        gesture,
    //        n_points,
    //        kind,
    //        x,
    //        y,
    //    };

    //    if let EventKind::Release = kind {
    //        self.mode = Mode::Standby;
    //        i2c.write(hw::ADDR, &CMD_STANDBY).await.unwrap();
    //    }

    //    event
    //}

    pub async fn wait_for_action(&mut self) -> TouchEvent {
        loop {
            let event = self.wait_for_event().await;
            if EventKind::Press == event.kind {
                return event;
            }
            if EventKind::Release == event.kind && event.gesture != Gesture::None {
                return event;
            }
        }
    }
}
