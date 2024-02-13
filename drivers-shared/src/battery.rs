#[repr(u8)]
#[derive(Copy, Clone, num_enum::TryFromPrimitive)]
pub enum ChargeState {
    Full,
    Charging,
    Draining,
}

#[derive(Copy, Clone)]
pub struct Reading {
    pub raw: u32,
}

pub const FULL_VOLTAGE_VAL: f32 = 0.3144;

impl Reading {
    pub fn voltage(&self) -> f32 {
        self.raw as f32 * (4.2 / 16384.0 / FULL_VOLTAGE_VAL / (1 << 16) as f32)
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

#[derive(Copy, Clone)]
pub struct CurrentReading {
    pub micro_ampere: u32,
}

impl CurrentReading {
    pub fn micro_ampere(self) -> u32 {
        self.micro_ampere
    }
}
