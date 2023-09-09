use embassy_nrf::{
    gpio::{Input, Pull},
    peripherals::{P0_03, P0_23, P0_25, SAADC},
    saadc::{self, Saadc},
};

pub struct Battery<'a> {
    saadc: Saadc<'a, 1>,
    charge_port_pin: Input<'a, P0_23>,
    charge_complete_pin: Input<'a, P0_25>,
}

pub enum ChargeState {
    Full,
    Charging,
    Draining,
}

impl<'a> Battery<'a> {
    pub fn new(
        saadc: SAADC,
        bat_val_pin: P0_03,
        charge_port_pin: P0_23,
        charge_complete_pin: P0_25,
    ) -> Self {
        let mut config = saadc::Config::default();
        config.resolution = saadc::Resolution::_14BIT;
        //config.oversample = saadc::Oversample::OVER256X;

        let mut channel_config = saadc::ChannelConfig::single_ended(bat_val_pin);
        channel_config.reference = saadc::Reference::VDD1_4;
        channel_config.gain = saadc::Gain::GAIN1_4;
        channel_config.time = saadc::Time::_3US;

        let charge_port_pin = Input::new(charge_port_pin, Pull::None);
        let charge_complete_pin = Input::new(charge_complete_pin, Pull::None);

        let saadc = saadc::Saadc::new(
            saadc,
            crate::Irqs, /*TODO: not sure if this is correct */
            config,
            [channel_config],
        );
        Self {
            saadc,
            charge_port_pin,
            charge_complete_pin,
        }
    }

    pub async fn read(&mut self) -> Reading {
        let mut bat_buf = [0; 1];
        self.saadc.sample(&mut bat_buf).await;
        Reading { raw: bat_buf[0] }
    }

    pub fn charge_state(&self) -> ChargeState {
        if self.charge_port_pin.is_low() {
            if self.charge_complete_pin.is_low() {
                ChargeState::Charging
            } else {
                ChargeState::Full
            }
        } else {
            ChargeState::Draining
        }
    }
}

pub struct Reading {
    pub raw: i16,
}

const FULL_VOLTAGE_VAL: f32 = 0.3144;

impl Reading {
    pub fn voltage(&self) -> f32 {
        self.raw as f32 * (4.2 / 16384.0 / FULL_VOLTAGE_VAL)
    }

    pub fn percentage(&self) -> f32 {
        let voltage = self.voltage();
        let v_100 = 4.2;
        let v_80 = 3.95;
        let v_10 = 3.70;
        let v_0 = 3.3;

        // Piecewise linear approximation as done in espruino
        let percentage = if voltage > v_80 {
            (voltage - v_80) * 20.0 / (v_100 - v_80) + 80.0
        } else if voltage > v_10 {
            (voltage - v_10) * 70.0 / (v_80 - v_10) + 10.0
        } else {
            (voltage - v_0) * 10.0 / (v_10 - v_0)
        };

        percentage
    }
}
