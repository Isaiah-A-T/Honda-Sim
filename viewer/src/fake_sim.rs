//! Demo data for when no sim is connected: a scripted pull (idle, full-throttle pull to the limiter, lift
//! and coast) or slider input, with rough part stress. Same idea as the sim's own numbers, much simpler.

use crate::state::{SimState, StressValue};

pub struct FakeSim {
    pub t: f32,
    pub turbo: bool,
    pub intercooler: bool,
    /// Demo a K24A block (87 × 99 mm, 152 mm rods) instead of the K20Z3.
    pub k24: bool,
    /// Warm-up: coolant and oil start cold, the thermostat opens, the fans cycle.
    pub coolant: f32,
    fan_on: bool,
    pub oil: f32,
    pub knock_until: f32,
    rpm: f32,
    boost: f32,
    vtc: f32,
    iat: f32,
    knock_retard: f32,
}

pub struct ManualInput {
    pub rpm: f32,
    pub throttle: f32,
    pub boost_kpa: f32,
}

impl FakeSim {
    pub fn new() -> Self {
        FakeSim { t: 0.0, turbo: true, intercooler: true, k24: false, coolant: 30.0, fan_on: false, oil: 30.0, knock_until: 0.0, rpm: 800.0, boost: 0.0, vtc: 0.0, iat: 25.0, knock_retard: 0.0 }
    }

    pub fn step(&mut self, dt: f32, input: Option<&ManualInput>, strength: &dyn Fn(&str) -> f32) -> SimState {
        self.t += dt;
        let (pedal, target) = match input {
            None => {
                let p = self.t % 12.0;
                if p < 2.0 { (0.0, 850.0) }
                else if p < 8.5 { (1.0, (2500.0 + (p - 2.0) * 950.0).min(8050.0)) }
                else { (0.0, 8000.0 - (p - 8.5) * 2000.0) }
            }
            Some(m) => (m.throttle, m.rpm),
        };
        let k = if input.is_some() { 5.0 } else { 4.0 };
        self.rpm += (target.max(800.0) - self.rpm) * (dt * k).min(1.0);
        let rpm = self.rpm;
        let spool = ((rpm - 2400.0) / 2200.0).clamp(0.0, 1.0).powf(1.6) * pedal;
        let boost_target = if self.turbo { input.map(|m| m.boost_kpa).unwrap_or(110.0) * spool } else { 0.0 };
        let rate = if boost_target > self.boost { 2.5 } else { 8.0 };
        self.boost += (boost_target - self.boost) * (dt * rate).min(1.0);
        let map = 25.0 + 75.0 * pedal + self.boost;
        let high_cam = rpm > 5800.0 && map > 70.0;
        let vtc_target = if pedal > 0.3 { 45.0 * (-((rpm - 4200.0) / 2200.0).powi(2)).exp() } else { 5.0 };
        self.vtc += (vtc_target - self.vtc) * (dt * 3.0).min(1.0);
        let rev_cut = rpm > 8000.0;
        let dfco = pedal < 0.02 && rpm > 1500.0;
        let lambda = if dfco || rev_cut { 1.0 } else if map > 90.0 { 0.82 } else { 1.0 };
        let mbt = 17.0 + 3.0 * rpm.min(6500.0) / 1000.0 + (100.0 - map) * 0.13;
        let knocking = self.t < self.knock_until;
        self.knock_retard = if knocking { (self.knock_retard + dt * 10.0).min(8.0) } else { (self.knock_retard - dt * 0.5).max(0.0) };
        let ign = (mbt - 2.0 - self.boost * 0.06).max(8.0) - self.knock_retard;
        let heat_soak = if self.intercooler { 0.12 } else { 0.45 };   // compressor heat the intercooler doesn't remove
        self.iat += ((25.0 + self.boost * heat_soak) - self.iat) * (dt * 0.3).min(1.0);
        let ve = if high_cam {
            0.70 + 0.31 * (-((rpm / 1000.0 - 7.0) / 2.6).powi(2)).exp()
        } else {
            0.58 + 0.29 * (-((rpm / 1000.0 - 4.4) / 2.4).powi(2)).exp()
        };
        let air_gs = ve * 1.998 * 1.184 * map / 101.3 * rpm / 120.0;
        let torque: f32 = if dfco || rev_cut { 0.0 } else { (235.0 * ve * map / 100.0 - 15.0).max(0.0) };
        let firing = !rev_cut && !dfco;

        // Cooling, sped up so the demo warms up in about a minute: heat in with load, out through the radiator
        // once the thermostat opens (it starts at 78 °C, full at 90), fans on above 96 °C.
        let thermostat = ((self.coolant - 78.0) / 12.0).clamp(0.0, 1.0);
        // Fans as in the sim: on at 93 °C, off again at 88 °C.
        if self.coolant > 93.0 { self.fan_on = true } else if self.coolant < 88.0 { self.fan_on = false }
        let fan = if self.fan_on { 1.0 } else { 0.0 };
        let heat_in = 1.2 + 2.5 * torque / 230.0 * rpm / 7000.0;
        let heat_out = thermostat * (self.coolant - 25.0) * (0.06 + 0.04 * fan);
        self.coolant = (self.coolant + (heat_in - heat_out) * dt).clamp(20.0, 118.0);
        self.oil += ((self.coolant + 8.0 + 10.0 * rpm / 8000.0) - self.oil) * (dt * 0.04).min(1.0);

        let spin = (rpm / 8600.0).powi(2);
        let knock_hit = if knocking { 0.35 } else { 0.0 };
        let rod_limit = 350.0;
        let mut stress = vec![
            ("rods", (torque / rod_limit).max(spin * 0.8)),
            ("pistons", torque / rod_limit * 0.8 + knock_hit + self.knock_retard / 8.0 * 0.25),
            ("crank", (torque / rod_limit * 0.75).max(spin * 0.65)),
            ("valves", spin),
            ("cams", spin * 0.6),
            ("head", knock_hit + ((self.iat - 25.0) / 60.0).clamp(0.0, 0.5) + if firing { map / 400.0 } else { 0.0 }),
            ("block", torque / rod_limit * 0.45),
            ("exhaust", if firing { (rpm / 8000.0 * map / 100.0 * 0.55).clamp(0.0, 1.3) } else { 0.0 }),
            ("intake", if self.turbo { ((map - 101.0) / 99.0).clamp(0.0, 1.3) } else { 0.0 }),
            ("turbo", if self.turbo { self.boost / 165.0 } else { 0.0 }),
            ("clutch", torque / 450.0),
            ("flywheel", spin * 0.5),
        ];
        for (k, v) in stress.iter_mut() {
            *v /= strength(k);
        }

        SimState {
            rpm,
            running: true,
            throttle: pedal,
            map_kpa: map,
            boost_kpa: self.boost,
            air_gs,
            vtec: high_cam,
            vtc_deg: self.vtc,
            ign_deg: ign,
            knock: knocking,
            fuel_cut: rev_cut || dfco,
            lambda,
            turbo: self.turbo,
            intercooler: self.intercooler,
            turbo_water_cooled: true,
            block: Some(if self.k24 { "K24A" } else { "K20Z3" }.to_string()),
            bore_mm: None,
            stroke_mm: None,
            rod_mm: None,
            deck_mm: None,
            iat_c: self.iat,
            coolant_c: self.coolant,
            coolant_out_c: None,
            oil_c: self.oil,
            oil_kpa: None,
            thermostat,
            fan,
            cams: None,
            vtec_pin: None,
            stress: stress.into_iter().map(|(k, v)| (k.to_string(), StressValue::One(v))).collect(),
            part_materials: None,
            readouts: vec![],
        }
    }
}
