//! Cooling and lubrication (src/thermal.js): coolant and oil temperatures, flows and pressures.
//!
//! Lumped model: one node for the engine (metal plus the coolant in it), one for the radiator, one for the
//! oil. The water pump pushes coolant through the block and head; below the thermostat's opening point it
//! all returns through the bypass, and as the thermostat opens more goes through the radiator. The fans run
//! off ECT2 in the radiator's bottom tank. Oil heats from friction and the pistons and gives its heat to the
//! block, the sump's air and the water-to-oil cooler. Values and sources: docs/cooling-oil-specs.json.

use crate::tables::clamp;
use serde::{Deserialize, Serialize};

/// Cooling and oil system settings. A tune's `cooling` overrides any of them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Cooling {
    pub coolant_l: f64,
    pub engine_coolant_share: f64,
    pub thermostat_open_c: f64,
    pub thermostat_full_c: f64,
    pub pump_lpm_per_krpm: f64,
    pub cap_kpa: f64,
    pub radiator_face_m2: f64,
    pub radiator_ua_wk_at5ms: f64,
    pub grille_share: f64,
    pub fan_face_ms: f64,
    pub fans: f64,
    pub fan_on_c: f64,
    pub fan_off_c: f64,
    pub engine_metal_jk: f64,
    pub radiator_metal_jk: f64,
    pub oil_jk: f64,
    pub wall_to_coolant: f64,
    pub friction_to_oil: f64,
    pub oil_to_engine_ua_wk: f64,
    pub oil_to_engine_ua_per_krpm: f64,
    pub sump_ua_wk: f64,
    pub sump_ua_per_ms: f64,
    /// "none", "water" (oil-to-coolant at the filter) or "air".
    pub oil_cooler: String,
    pub oil_cooler_ua_wk: f64,
    pub oil_l: f64,
    pub oil_grade: String,
    pub oil_visc40: f64,
    pub oil_visc100: f64,
    pub oil_relief_kpa: f64,
    pub oil_hot_idle_kpa: f64,
    pub oil_kpa_per_rpm: f64,
    pub oil_jet_kpa: f64,
    pub ect_vtec_min_c: f64,
}

impl Default for Cooling {
    fn default() -> Self {
        Cooling {
            coolant_l: 6.4,
            engine_coolant_share: 0.55,
            thermostat_open_c: 78.0,
            thermostat_full_c: 90.0,
            pump_lpm_per_krpm: 20.0,
            cap_kpa: 110.0,
            radiator_face_m2: 0.254,
            radiator_ua_wk_at5ms: 1500.0,
            grille_share: 0.30,
            fan_face_ms: 3.0,
            fans: 2.0,
            fan_on_c: 93.0,
            fan_off_c: 88.0,
            engine_metal_jk: 45000.0,
            radiator_metal_jk: 4500.0,
            oil_jk: 9000.0,
            wall_to_coolant: 0.85,
            friction_to_oil: 0.5,
            oil_to_engine_ua_wk: 200.0,
            oil_to_engine_ua_per_krpm: 100.0,
            sump_ua_wk: 15.0,
            sump_ua_per_ms: 1.5,
            oil_cooler: "water".into(),
            oil_cooler_ua_wk: 250.0,
            oil_l: 4.4,
            oil_grade: "5W-30".into(),
            oil_visc40: 62.0,
            oil_visc100: 10.5,
            oil_relief_kpa: 600.0,
            oil_hot_idle_kpa: 80.0,
            oil_kpa_per_rpm: 0.09,
            oil_jet_kpa: 200.0,
            ect_vtec_min_c: 60.0,
        }
    }
}

const CP_COOLANT: f64 = 3500.0;
const RHO_COOLANT: f64 = 1.04;
const RHO_AIR: f64 = 1.18;
const CP_AIR: f64 = 1005.0;

/// Kinematic viscosity (cSt): Vogel fit with C = 95 through the grade's 40 and 100 °C points.
pub fn viscosity_cst(oil_c: f64, g: &Cooling) -> f64 {
    let (c, t1, t2) = (95.0, 40.0, 100.0);
    let b = (g.oil_visc40 / g.oil_visc100).ln() / (1.0 / (t1 + c) - 1.0 / (t2 + c));
    let a = g.oil_visc100 / (b / (t2 + c)).exp();
    a * (b / (clamp(oil_c, -30.0, 180.0) + c)).exp()
}

/// Viscosity relative to hot (100 °C) oil.
pub fn visc_ratio(oil_c: f64, g: &Cooling) -> f64 {
    viscosity_cst(oil_c, g) / g.oil_visc100
}

/// Coolant boiling point at the cap pressure (°C).
pub fn boiling_c(g: &Cooling) -> f64 {
    107.0 + 0.2 * g.cap_kpa
}

/// Oil pressure (kPa): rises with pump speed and viscosity until the relief valve opens.
pub fn oil_pressure_kpa(rpm: f64, oil_c: f64, g: &Cooling) -> f64 {
    let hot = g.oil_hot_idle_kpa + rpm * g.oil_kpa_per_rpm;
    clamp(hot * visc_ratio(oil_c, g).powf(0.5), 0.0, g.oil_relief_kpa)
}

#[derive(Clone, Debug)]
pub struct Thermal {
    pub g: Cooling,
    /// Engine outlet (what ECT1 in the head reads).
    pub coolant_c: f64,
    pub radiator_c: f64,
    pub oil_c: f64,
    /// Thermostat opening, 0–1.
    pub thermostat: f64,
    pub fan_on: bool,
    pub pump_lpm: f64,
    pub radiator_lpm: f64,
    pub bypass_lpm: f64,
    pub air_ms: f64,
    pub radiator_kw: f64,
    pub heat_to_coolant_kw: f64,
    pub heat_to_oil_kw: f64,
    pub oil_to_coolant_kw: f64,
    pub sump_kw: f64,
    pub oil_cooler_kw: f64,
    pub radiator_out_c: f64,
    /// ECT2, in the radiator's bottom tank (fan control).
    pub ect2_c: f64,
    pub inlet_c: f64,
    pub boiling: bool,
    pub overheat_s: f64,
}

pub struct ThermalInput {
    pub rpm: f64,
    pub fuel_gs: f64,
    pub friction_w: f64,
    pub heat_loss: f64,
    pub speed_ms: f64,
    pub map_kpa: f64,
    pub ambient_c: f64,
}

impl Thermal {
    pub fn new(ambient_c: f64, warm: bool, g: Cooling) -> Thermal {
        let thermostat = if warm { clamp((90.0 - g.thermostat_open_c) / (g.thermostat_full_c - g.thermostat_open_c), 0.0, 1.0) } else { 0.0 };
        let w = |hot: f64| if warm { hot } else { ambient_c };
        Thermal {
            g,
            coolant_c: w(90.0),
            radiator_c: w(70.0),
            oil_c: w(100.0),
            thermostat,
            fan_on: false,
            pump_lpm: 0.0,
            radiator_lpm: 0.0,
            bypass_lpm: 0.0,
            air_ms: 0.0,
            radiator_kw: 0.0,
            heat_to_coolant_kw: 0.0,
            heat_to_oil_kw: 0.0,
            oil_to_coolant_kw: 0.0,
            sump_kw: 0.0,
            oil_cooler_kw: 0.0,
            radiator_out_c: w(70.0),
            ect2_c: w(70.0),
            inlet_c: w(84.0),
            boiling: false,
            overheat_s: 0.0,
        }
    }

    pub fn step(&mut self, i: &ThermalInput, dt: f64) {
        let g = &self.g;
        let fuel_w = i.fuel_gs / 1000.0 * 44e6;
        let light = 1.0 + 0.8 * clamp(1.0 - i.map_kpa / 100.0, 0.0, 1.0);
        let wall_w = fuel_w * i.heat_loss * 0.75 * light;
        let to_coolant = wall_w * g.wall_to_coolant + i.friction_w * (1.0 - g.friction_to_oil);
        let to_oil = wall_w * (1.0 - g.wall_to_coolant) + i.friction_w * g.friction_to_oil;

        self.pump_lpm = g.pump_lpm_per_krpm * i.rpm.max(0.0) / 1000.0;
        self.radiator_lpm = self.pump_lpm * self.thermostat;
        self.bypass_lpm = self.pump_lpm - self.radiator_lpm;
        let mix = if self.pump_lpm > 0.0 { (self.radiator_out_c * self.radiator_lpm + self.coolant_c * self.bypass_lpm) / self.pump_lpm } else { self.coolant_c };
        let c_pump = self.pump_lpm / 60.0 * RHO_COOLANT * CP_COOLANT;
        self.inlet_c = if c_pump > 0.0 { mix.min(self.coolant_c - self.heat_to_coolant_kw * 1000.0 / c_pump) } else { self.coolant_c };
        let want = clamp((self.inlet_c - g.thermostat_open_c) / (g.thermostat_full_c - g.thermostat_open_c), 0.0, 1.0);
        self.thermostat += (want - self.thermostat) * (dt / 4.0).min(1.0);

        self.ect2_c += (self.radiator_out_c - self.ect2_c) * (dt / 15.0).min(1.0);
        if self.ect2_c >= g.fan_on_c {
            self.fan_on = true;
        } else if self.ect2_c <= g.fan_off_c {
            self.fan_on = false;
        }
        let ram = g.grille_share * i.speed_ms.abs();
        let fan = if self.fan_on { g.fan_face_ms } else { 0.0 };
        self.air_ms = (ram * ram + fan * fan).sqrt() + 0.3;

        let m_cool = self.radiator_lpm / 60.0 * RHO_COOLANT;
        let c_cool = m_cool * CP_COOLANT;
        let c_air = RHO_AIR * g.radiator_face_m2 * self.air_ms * CP_AIR;
        let mut q_rad = 0.0;
        if c_cool > 0.0 {
            let c_min = c_cool.min(c_air);
            let c_max = c_cool.max(c_air);
            let cr = c_min / c_max;
            let ua = g.radiator_ua_wk_at5ms * (self.air_ms / 5.0).powf(0.6);
            let ntu = ua / c_min;
            let eff = 1.0 - ((ntu.powf(0.22) / cr) * ((-cr * ntu.powf(0.78)).exp() - 1.0)).exp();
            q_rad = eff * c_min * (self.coolant_c - i.ambient_c);
        }
        self.radiator_out_c = if c_cool > 0.0 { self.coolant_c - q_rad / c_cool } else { self.radiator_c };
        let engine_jk = g.engine_metal_jk + g.coolant_l * g.engine_coolant_share * RHO_COOLANT * CP_COOLANT;
        let rad_jk = g.radiator_metal_jk + g.coolant_l * (1.0 - g.engine_coolant_share) * RHO_COOLANT * CP_COOLANT;
        let exchange = c_cool * (self.coolant_c - self.radiator_c);

        let ua_oil = g.oil_to_engine_ua_wk + g.oil_to_engine_ua_per_krpm * i.rpm.max(0.0) / 1000.0;
        let oil_to_engine = ua_oil * (self.oil_c - self.coolant_c);
        let sump = (g.sump_ua_wk + g.sump_ua_per_ms * i.speed_ms.abs()) * (self.oil_c - i.ambient_c);
        let cooler = match g.oil_cooler.as_str() {
            "water" => g.oil_cooler_ua_wk * (self.oil_c - self.coolant_c),
            "air" => g.oil_cooler_ua_wk * (self.air_ms / 5.0).powf(0.6) * (self.oil_c - i.ambient_c),
            _ => 0.0,
        };
        let cooler_to_coolant = if g.oil_cooler == "water" { cooler } else { 0.0 };

        let boil = boiling_c(g);
        self.boiling = self.coolant_c > boil;
        self.overheat_s = if self.boiling { self.overheat_s + dt } else { (self.overheat_s - dt).max(0.0) };

        self.coolant_c += (to_coolant + oil_to_engine + cooler_to_coolant - exchange) / engine_jk * dt;
        self.radiator_c += (exchange - q_rad) / rad_jk * dt;
        self.oil_c += (to_oil - oil_to_engine - sump - cooler) / g.oil_jk * dt;

        self.heat_to_coolant_kw = to_coolant / 1000.0;
        self.heat_to_oil_kw = to_oil / 1000.0;
        self.radiator_kw = q_rad / 1000.0;
        self.oil_to_coolant_kw = (oil_to_engine + cooler_to_coolant) / 1000.0;
        self.sump_kw = sump / 1000.0;
        self.oil_cooler_kw = cooler / 1000.0;
    }
}

/// One stretch of hose, passage or line between two components, with what flows in it now.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub from: &'static str,
    pub to: &'static str,
    pub name: &'static str,
    pub flow_lpm: f64,
    pub temp_c: Option<f64>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Circuits {
    pub coolant: Vec<Segment>,
    pub oil: Vec<Segment>,
}

/// The coolant and oil circuits as segments between named components (see COMPONENTS).
pub fn circuits(th: &Thermal, oil_kpa: f64) -> Circuits {
    let g = &th.g;
    let oil_lpm = (4.0 + th.pump_lpm.max(0.0) * 0.25).min(60.0);
    let p = th.pump_lpm.max(1e-6);
    let seg = |from, to, name, flow_lpm, temp_c| Segment { from, to, name, flow_lpm, temp_c };
    let mut coolant = vec![
        seg("pump", "blockJacket", "Water pump to block water jacket", th.pump_lpm, Some(th.radiator_out_c * (th.radiator_lpm / p) + th.coolant_c * (th.bypass_lpm / p))),
        seg("blockJacket", "headJacket", "Block to head through the head gasket", th.pump_lpm, Some(th.coolant_c - 3.0)),
        seg("headJacket", "waterOutlet", "Head to water outlet", th.pump_lpm, Some(th.coolant_c)),
        seg("waterOutlet", "radiator", "Upper radiator hose", th.radiator_lpm, Some(th.coolant_c)),
        seg("radiator", "thermostat", "Lower radiator hose", th.radiator_lpm, Some(th.radiator_out_c)),
        seg("waterOutlet", "bypass", "Bypass (warm-up loop back to the pump)", th.bypass_lpm, Some(th.coolant_c)),
        seg("bypass", "thermostat", "Bypass into the thermostat case", th.bypass_lpm, Some(th.coolant_c)),
        seg("thermostat", "pump", "Thermostat to pump inlet", th.pump_lpm, None),
        seg("waterOutlet", "heaterCore", "Heater feed hose", 0.0, Some(th.coolant_c)),
        seg("heaterCore", "pump", "Heater return hose", 0.0, Some(th.coolant_c)),
        seg("radiator", "reserveTank", "Overflow to the reserve tank", 0.0, Some(th.radiator_c)),
    ];
    let mut oil = vec![
        seg("sump", "oilPump", "Pickup", oil_lpm, Some(th.oil_c)),
        seg("oilPump", "filter", "Pump to filter", oil_lpm, Some(th.oil_c)),
        seg("filter", "mainGallery", "Filter to main gallery", oil_lpm, Some(th.oil_c)),
        seg("mainGallery", "mainBearings", "Main bearings, then through the crank to the rod bearings", oil_lpm * 0.5, Some(th.oil_c)),
        seg("mainGallery", "pistonJets", "Piston oil jets (open above 200 kPa)", if oil_kpa >= g.oil_jet_kpa { oil_lpm * 0.1 } else { 0.0 }, Some(th.oil_c)),
        seg("mainGallery", "head", "Up to the head: cam journals, rocker shafts, VTEC spool and pins", oil_lpm * 0.3, Some(th.oil_c)),
        seg("mainGallery", "vtc", "VTC actuator and chain tensioner", oil_lpm * 0.1, Some(th.oil_c)),
        seg("head", "sump", "Drain back", oil_lpm * 0.4, Some(th.oil_c)),
    ];
    if g.oil_cooler != "none" {
        let water = g.oil_cooler == "water";
        oil.splice(1..2, [
            seg("oilPump", "oilCooler", if water { "Pump to the water-to-oil cooler" } else { "Oil cooler feed line" }, oil_lpm, Some(th.oil_c)),
            seg("oilCooler", "filter", "Cooler to filter", oil_lpm, Some(th.oil_c)),
        ]);
        if water {
            let lpm = th.pump_lpm * 0.05;
            coolant.push(seg("pump", "oilCooler", "Oil cooler in hose (19422-RRB-A01)", lpm, Some(th.inlet_c)));
            coolant.push(seg("oilCooler", "thermostat", "Oil cooler outlet hose (19423-RRB-A01)", lpm,
                Some(th.inlet_c + th.oil_cooler_kw * 1000.0 / (lpm / 60.0 * RHO_COOLANT * CP_COOLANT).max(1.0))));
        }
    }
    Circuits { coolant, oil }
}

/// A component of the coolant or oil system and roughly where it sits, for drawing. Engine frame in metres:
/// origin on the crank axis midway along the block, +x toward the timing-chain end (cylinder 1), +y up the
/// bores, +z toward the front of the car (the Si's intake side, facing the radiator). Positions are
/// approximate placements, not measurements.
#[derive(Serialize, Clone, Debug)]
pub struct Component {
    pub key: &'static str,
    pub circuit: &'static str,
    pub name: &'static str,
    pub pos: [f64; 3],
    pub size: [f64; 3],
}

macro_rules! comp {
    ($k:literal, $c:literal, $n:literal, $p:expr, $s:expr) => {
        Component { key: $k, circuit: $c, name: $n, pos: $p, size: $s }
    };
}

pub const COMPONENTS: &[Component] = &[
    comp!("pump", "coolant", "Water pump (19200-RBC-013), belt driven, timing-chain end", [0.22, 0.10, 0.12], [0.06, 0.14, 0.14]),
    comp!("thermostat", "coolant", "Thermostat (19301-RAF-004) in its case at the pump inlet", [0.22, 0.02, 0.18], [0.07, 0.07, 0.07]),
    comp!("blockJacket", "coolant", "Block water jacket around the four iron liners", [0.0, 0.17, 0.0], [0.40, 0.10, 0.14]),
    comp!("headJacket", "coolant", "Head water jacket (ECT1 sensor here)", [0.0, 0.25, 0.0], [0.42, 0.06, 0.18]),
    comp!("waterOutlet", "coolant", "Water neck / EGR passage (18712-RRA-A00) on the head, transmission end", [-0.24, 0.27, 0.06], [0.06, 0.06, 0.08]),
    comp!("bypass", "coolant", "Bypass hose (19504-RAC-000) under the intake manifold", [0.0, 0.12, 0.16], [0.44, 0.03, 0.03]),
    comp!("radiator", "coolant", "Radiator (Denso 19010-RNB-A51), single-row down-flow, plastic tanks, 16 psi cap", [0.0, 0.10, 0.62], [0.68, 0.375, 0.016]),
    comp!("radiatorFan", "coolant", "Radiator fan (19020-RRA-A01), driver side", [-0.17, 0.10, 0.58], [0.37, 0.37, 0.10]),
    comp!("condenserFan", "coolant", "Condenser fan (38611-RRA-A01), passenger side", [0.17, 0.10, 0.58], [0.37, 0.37, 0.09]),
    comp!("reserveTank", "coolant", "Reserve tank, 0.4 L", [0.32, 0.28, 0.52], [0.10, 0.12, 0.08]),
    comp!("heaterCore", "coolant", "Heater core (behind the firewall)", [0.0, 0.30, -0.80], [0.20, 0.15, 0.05]),
    comp!("sump", "oil", "Cast aluminium oil pan, 4.4 L with filter", [0.0, -0.13, 0.0], [0.42, 0.12, 0.26]),
    comp!("oilPump", "oil", "Rotor oil pump, chain driven, in the balancer unit in the pan", [0.0, -0.07, 0.0], [0.20, 0.06, 0.12]),
    comp!("filter", "oil", "Oil filter", [-0.05, 0.02, -0.16], [0.08, 0.09, 0.08]),
    comp!("oilCooler", "oil", "Water-to-oil cooler (15500-RBC-004), sandwiched under the filter", [-0.05, 0.02, -0.12], [0.09, 0.04, 0.09]),
    comp!("mainGallery", "oil", "Main oil gallery along the block", [0.0, 0.03, -0.08], [0.40, 0.012, 0.012]),
    comp!("mainBearings", "oil", "Main and rod bearings", [0.0, 0.0, 0.0], [0.40, 0.06, 0.06]),
    comp!("pistonJets", "oil", "Piston oil jets (15280-PRB-A00), one per cylinder", [0.0, 0.06, -0.05], [0.36, 0.02, 0.02]),
    comp!("head", "oil", "Cam journals, rocker shafts, VTEC spool valve and pressure switch", [0.0, 0.32, 0.0], [0.42, 0.06, 0.18]),
    comp!("vtc", "oil", "VTC oil control valve (15830-RBB-003) and intake cam actuator", [0.22, 0.32, 0.06], [0.05, 0.10, 0.10]),
];
