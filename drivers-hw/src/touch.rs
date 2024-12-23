use crate::twi::{TwiHandle, TWI};

use super::hardware::touch as hw;
use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    twim,
};
use embassy_time::{Duration, Instant, Timer};

pub use drivers_shared::touch::*;
use embedded_hal::blocking::delay::DelayMs;

pub struct TouchRessources {
    scl: hw::SCL,
    sda: hw::SDA,
    reset: Output<'static, hw::RST>,
    irq: Input<'static, hw::IRQ>,
}

// From espruino. not documented anywhere else, afaik, though...
const CMD_SLEEP: [u8; 2] = [0xE5, 0x03];

// This is sleep mode according to official example code (but looks like standby???)
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
        i2c: &mut TWI,
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

    pub async fn enabled<'a>(&'a mut self, i2c: &'a TWI) -> Touch<'a> {
        // These reset durations are the same that espruino uses, so hopefully this works out.
        self.reset.set_low();
        Timer::after(Duration::from_millis(1)).await;
        self.reset.set_high();
        Timer::after(Duration::from_millis(1)).await;

        let i2c_conf = twim::Config::default();
        // TODO: check if this is allowed
        //i2c_conf.frequency = embassy_nrf::twim::Frequency::K400;

        Touch {
            irq: &mut self.irq,
            i2c: i2c.configure(&mut self.sda, &mut self.scl, i2c_conf),
            mode: Mode::Standby,
            enabled_at: Instant::now(),
        }
    }
}

pub struct Touch<'a> {
    irq: &'a mut Input<'static, hw::IRQ>,
    i2c: TwiHandle<'a, &'a mut hw::SDA, &'a mut hw::SCL>,
    mode: Mode,
    enabled_at: Instant,
}

impl<'a> Drop for Touch<'a> {
    fn drop(&mut self) {
        // Sleeping to early after reseting touch interface leads to i2c connection problems
        let delay = Duration::from_millis(50);
        let safe_to_sleep = self.enabled_at + delay;
        let now = Instant::now();
        if now < safe_to_sleep {
            embassy_time::Delay.delay_ms((safe_to_sleep - now).as_millis() as u32);
        }

        let mut i2c = self.i2c.bind_blocking();
        i2c.blocking_write(hw::ADDR, &CMD_SLEEP).unwrap();
    }
}

#[derive(Copy, Clone)]
enum Mode {
    Standby,
    Dynamic,
}

//#[derive(Copy, Clone)]
//enum State {
//    WaitInterrupt,
//    Read,
//    GoStandby,
//}

impl<'a> Touch<'a> {
    //pub async fn wait_for_event(&mut self) -> TouchEvent {
    //    loop {
    //        match self.state {
    //            State::WaitInterrupt => {
    //                self.hw.irq.wait_for_low().await;
    //                self.state = State::Read;
    //            }
    //            State::Read => {
    //                let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);

    //                let mut buf = [0u8; 6];
    //                i2c.write_read(hw::ADDR, &CMD_READ_EVENT, &mut buf)
    //                    .await
    //                    .unwrap();

    //                let kind = EventKind::try_from(buf[2] >> 6).unwrap();

    //                let n_points = buf[1];
    //                let gesture = Gesture::try_from(buf[0]).unwrap();

    //                // Espruino adjusts these, but it seems like the range is actually [0, 175]?
    //                //let x = (buf[3] as usize * crate::lpm013m1126c::WIDTH / TOUCH_AREA_WIDTH) as u8;
    //                let x = buf[3];
    //                //let y = (buf[5] as usize * crate::lpm013m1126c::HEIGHT / TOUCH_AREA_HEIGHT) as u8;
    //                let y = buf[5];

    //                let event = TouchEvent {
    //                    gesture,
    //                    n_points,
    //                    kind,
    //                    x,
    //                    y,
    //                };
    //                if let EventKind::Release = kind {
    //                    self.state = State::GoStandby;
    //                } else {
    //                    self.state = State::Read;
    //                }

    //                return event;
    //            }
    //            State::GoStandby => {
    //                let mut i2c = build_i2c(&mut self.hw.sda, &mut self.hw.scl, self.instance);
    //                i2c.write(hw::ADDR, &CMD_STANDBY).await.unwrap();
    //                self.state = State::WaitInterrupt;
    //            }
    //        }
    //    }
    //}

    pub async fn wait_for_event(&mut self) -> TouchEvent {
        if let Mode::Standby = self.mode {
            if self.irq.is_high() {
                self.irq.wait_for_low().await;
            }
        } else {
            embassy_futures::yield_now().await;
        }
        self.mode = Mode::Dynamic;

        let mut i2c = self.i2c.bind().await;

        let mut buf = [0u8; 6];
        i2c.blocking_write_read(hw::ADDR, &CMD_READ_EVENT, &mut buf)
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

        if EventKind::Release == kind && Gesture::None != gesture {
            self.mode = Mode::Standby;
            i2c.blocking_write(hw::ADDR, &CMD_STANDBY).unwrap();
        }

        event
    }

    pub async fn wait_for_action(&mut self) -> TouchEvent {
        loop {
            let event = self.wait_for_event().await;

            // If we are too busy with other stuff we sometimes miss the Press event, so we also
            // want to return Hold here. However, ...
            if matches!(event.kind, EventKind::Press | EventKind::Hold) {
                return event;
            }
            // ... A release event without a gesture seems to always be followed by another release
            // event of a specific gesture.
            if EventKind::Release == event.kind && event.gesture != Gesture::None {
                return event;
            }
        }
    }
}
