#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, num_enum::TryFromPrimitive)]
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

        // Values obtained from tools/calc_bat_coefficients.py
        const P0: f32 = 0.0;
        const P1: f32 = 0.07502381897223538;
        const P2: f32 = 0.6783340794345193;
        const P3: f32 = 1.0;
        const V0: f32 = 3.3217568397521973;
        const V1: f32 = 3.703044909150518;
        const V2: f32 = 3.9337690661061937;
        const V3: f32 = 4.266753673553467;

        // Piecewise linear approximation (3 linear pieces)
        let proportion = if voltage > V2 {
            (voltage - V2) * (P3 - P2) / (V3 - V2) + P2
        } else if voltage > V1 {
            (voltage - V1) * (P2 - P1) / (V2 - V1) + P1
        } else {
            (voltage - V0) * (P1 - P0) / (V1 - V0) + P0
        };

        proportion * 100.0
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
