//! Engine internals: cylinder pressure, the loads it puts on each part, and metal fatigue (web/src/internals.js).

use crate::engine::Engine;
use crate::materials::{material, Material};
use crate::parts::BottomEnd;
use crate::tables::{clamp, lookup1};
use crate::thermal::{rotating_oil, RotatingOil};
use crate::tune::Tune;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry {
    pub bore_m: f64,
    pub stroke_m: f64,
    pub rod_m: f64,
    pub compression: f64,
    pub pin_m: f64,
    pub big_end_m: f64,
    pub piston_kg: f64,
    pub pin_kg: f64,
    pub rings_kg: f64,
    pub rod_small_end_kg: f64,
    pub rod_beam_area_m2: f64,
    pub rod_bolt_stress_area_m2: f64,
    pub bearing_width_m: f64,
    pub piston_stress_per_bar: f64,
    pub sleeve_wall_m: f64,
    pub jacket_wall_m: f64,
    pub open_deck_factor: f64,
    pub main_web_area_m2: f64,
    pub main_web_kt: f64,
    pub head_bolts: f64,
    pub head_bolt_area_m2: f64,
    pub head_joint_factor: f64,
}

pub const GEOMETRY: Geometry = Geometry {
    bore_m: 0.086, stroke_m: 0.086, rod_m: 0.139, compression: 11.0,
    pin_m: 0.022, big_end_m: 0.048,
    piston_kg: 0.290, pin_kg: 0.085, rings_kg: 0.035, rod_small_end_kg: 0.160,
    rod_beam_area_m2: 116e-6,
    rod_bolt_stress_area_m2: 2.0 * 39e-6,
    bearing_width_m: 0.017,
    piston_stress_per_bar: 0.55,
    sleeve_wall_m: 0.004, jacket_wall_m: 0.005,
    open_deck_factor: 1.25,
    main_web_area_m2: 1.0e-3, main_web_kt: 2.5,
    head_bolts: 10.0, head_bolt_area_m2: 72.3e-6,
    head_joint_factor: 0.2,
};

pub struct Part {
    pub key: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub default: &'static str,
}

pub const PARTS: [Part; 10] = [
    Part { key: "piston", name: "Pistons", category: "piston", default: "pistonAl" },
    Part { key: "rod", name: "Connecting rods", category: "rod", default: "rodPM" },
    Part { key: "rodBolt", name: "Rod bolts", category: "rodBolt", default: "rodBolt" },
    Part { key: "bearing", name: "Rod bearings", category: "bearing", default: "bearing" },
    Part { key: "crank", name: "Crankshaft", category: "crank", default: "crank" },
    Part { key: "gasket", name: "Head gasket", category: "gasket", default: "gasket" },
    Part { key: "headBolt", name: "Head bolts", category: "headBolt", default: "headBolt" },
    Part { key: "sleeve", name: "Cylinder sleeves", category: "sleeve", default: "sleeve" },
    Part { key: "block", name: "Block (main webs)", category: "block", default: "blockAl" },
    Part { key: "valvetrain", name: "Valvetrain", category: "valveSpring", default: "valveSpring" },
];
const N: usize = PARTS.len();

pub fn part_index(key: &str) -> usize {
    PARTS.iter().position(|p| p.key == key).expect("part key")
}

/// The alloy picked for a part wins; otherwise the alloy of the aftermarket part fitted; otherwise stock.
pub fn part_material_id(tune: &Tune, bottom: &BottomEnd, part: &Part) -> &'static str {
    let ok = |id: &str| material(id).map(|m| m.category == part.category).unwrap_or(false);
    if let Some(id) = tune.materials.get(part.key) {
        if ok(id) {
            return crate::materials::materials().iter().find(|(k, _)| k == id).map(|(k, _)| k.as_str()).unwrap();
        }
    }
    let fitted = match part.key {
        "piston" => bottom.materials.piston,
        "rod" => bottom.materials.rod,
        "crank" => bottom.materials.crank,
        _ => None,
    };
    match fitted {
        Some(id) if ok(id) => id,
        _ => part.default,
    }
}

pub fn part_material(tune: &Tune, bottom: &BottomEnd, part: &Part) -> &'static Material {
    material(part_material_id(tune, bottom, part)).expect("material")
}

/// Geometry with the fitted crank, rods and pistons. Stock sleeves thin as the bore grows; a re-sleeved
/// block (any non-stock sleeve material) has its full wall.
pub fn engine_geometry(tune: &Tune, b: &BottomEnd) -> Geometry {
    if b.geom_stock {
        return GEOMETRY;
    }
    let resleeved = tune.materials.get("sleeve").map(|s| s != "sleeve").unwrap_or(false);
    Geometry {
        bore_m: b.bore_mm / 1000.0,
        stroke_m: b.stroke_mm / 1000.0,
        rod_m: b.rod_mm / 1000.0,
        compression: b.compression,
        pin_m: b.pin_mm / 1000.0,
        big_end_m: b.big_end_bearing_mm / 1000.0,
        piston_kg: b.piston_kg,
        rod_small_end_kg: b.rod_recip_kg,
        sleeve_wall_m: if resleeved { GEOMETRY.sleeve_wall_m } else { (GEOMETRY.sleeve_wall_m - (b.bore_mm - b.block.bore_mm) / 2000.0).max(0.0015) },
        ..GEOMETRY
    }
}

fn derate(m: &Material, temp_c: f64) -> f64 {
    match &m.strength_vs_temp_c {
        Some(t) => {
            let a: Vec<f64> = t.iter().map(|d| d[0]).collect();
            let v: Vec<f64> = t.iter().map(|d| d[1]).collect();
            lookup1(&a, &v, temp_c)
        }
        None => 1.0,
    }
}

fn goodman(amp: f64, mean: f64, m: &Material, k: f64) -> f64 {
    amp / (m.fatigue() * k * (1.0 - clamp(mean / (m.uts() * k), 0.0, 0.95))).max(1.0)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Internals {
    pub pmax_bar: f64,
    pub piston_c: f64,
    pub rod_comp_n: f64,
    pub rod_tension_n: f64,
    pub knock_spike: f64,
    pub sleeve_mpa: f64,
    pub gasket_lift_bar: f64,
    pub load: [f64; N],
    pub damage: [f64; N],
    /// Name of the part that broke.
    pub failed: Option<&'static str>,
    /// Oil to the main and rod bearings and squirters on the last step.
    pub lube: Option<RotatingOil>,
}

impl Default for Internals {
    fn default() -> Self {
        Internals { pmax_bar: 0.0, piston_c: 120.0, rod_comp_n: 0.0, rod_tension_n: 0.0, knock_spike: 1.0, sleeve_mpa: 0.0, gasket_lift_bar: 160.0, load: [0.0; N], damage: [0.0; N], failed: None, lube: None }
    }
}

pub fn cycles_to_failure(load: f64) -> f64 {
    if load < 0.8 {
        f64::INFINITY
    } else if load >= 1.5 {
        1.0
    } else {
        1e7 * (0.8 / load).powi(9)
    }
}

/// Peak cylinder pressure estimate (bar), used when the gas model has no fired cycle.
pub fn peak_pressure_bar(e: &Engine) -> f64 {
    let charge = (e.map / 101.3) * e.ve * (298.0 / (273.0 + e.iat));
    let compression_bar = 1.013 * charge * e.geometry.compression.powf(1.3);
    if e.rev_cut || e.dfco || e.boost_cut {
        return compression_bar;
    }
    let spark = clamp(1.0 + 0.025 * (e.ign - e.mbt), 0.35, 1.3);
    let mixture = if e.lambda < 1.0 { 1.0 - (1.0 - e.lambda) * 0.3 } else { 1.0 / e.lambda };
    compression_bar * 3.8 * spark * mixture
}

pub fn part_status(load: f64, damage: f64) -> &'static str {
    if damage >= 1.0 {
        "failed"
    } else if load >= 1.0 || damage > 0.5 {
        "danger"
    } else if load >= 0.8 || damage > 0.0 {
        "warn"
    } else {
        "ok"
    }
}

impl Internals {
    pub fn load_of(&self, key: &str) -> f64 {
        self.load[part_index(key)]
    }

    /// Update loads and fatigue for dt seconds of running.
    pub fn step(&mut self, e: &Engine, dt: f64) {
        if self.failed.is_some() {
            return;
        }
        let rpm = e.rpm.max(0.0);
        let w = rpm * std::f64::consts::PI / 30.0;
        let m = |key: &str| part_material(&e.tune, &e.bottom, &PARTS[part_index(key)]);
        let (mp, mr, mrb, mb, mc, mg, mhb, ms, mbl) = (m("piston"), m("rod"), m("rodBolt"), m("bearing"), m("crank"), m("gasket"), m("headBolt"), m("sleeve"), m("block"));
        let g = &e.geometry;
        let piston_area = std::f64::consts::PI / 4.0 * g.bore_m.powi(2);
        let lambda_r = g.stroke_m / 2.0 / g.rod_m;

        let over = (e.ign - e.knock_limit).max(0.0);
        self.knock_spike = if e.knocking { 1.0 + (0.15 * (1.0 + over)).min(1.0) } else { 1.0 };
        let cut = e.rev_cut || e.dfco || e.boost_cut;
        self.pmax_bar = (if cut || !e.cyl_pmax_bar.is_finite() { peak_pressure_bar(e) } else { e.cyl_pmax_bar }) * self.knock_spike;

        let k_rel = mp.conductivity_w_mk.unwrap_or(130.0) / 130.0;
        let heat = 0.64 * e.air_gs
            + if e.lambda > 0.85 && e.map > 90.0 { 150.0 * (e.lambda - 0.85).min(0.5) } else { 0.0 }
            + if e.knocking { 40.0 * (self.knock_spike - 1.0) / 0.15 } else { 0.0 };
        // The oil under the crown sets its floor; the oil jets (check balls open at 200 kPa) cool it much better.
        let jets = clamp((e.oil_kpa - e.thermal.g.oil_jet_kpa) / 50.0, 0.0, 1.0);
        let base = e.oil_c + 80.0 - 30.0 * jets;
        let t_target = if cut { base } else { base + heat / k_rel.powf(0.5) };
        self.piston_c += (t_target - self.piston_c) * (dt * 0.25).min(1.0);

        let recip_kg = g.piston_kg * (mp.density() / 2700.0) + g.pin_kg + g.rings_kg + g.rod_small_end_kg * (mr.density() / 7780.0);
        let inertia_n = recip_kg * (g.stroke_m / 2.0) * w * w * (1.0 + lambda_r);
        self.rod_comp_n = (self.pmax_bar * 1e5 * piston_area - 0.85 * inertia_n).max(0.0);
        self.rod_tension_n = inertia_n;

        let el = mp.elongation_pct.unwrap_or(1.0);
        let brittle = if el < 2.0 { 1.6 } else if el < 5.0 { 1.0 } else { 0.6 };
        let piston_bar = self.pmax_bar / self.knock_spike * (1.0 + (self.knock_spike - 1.0) * brittle);
        let mut load = [0.0; N];
        // Oil film strength scales with viscosity: thin hot oil carries less load.
        // Rod bearing oil film: fed through the crank from the mains, thinned by shear heating and by a
        // starved supply when oil pressure can't keep up with rpm.
        let lube = rotating_oil(rpm, e.oil_kpa, e.oil_c, &e.thermal.g, clamp(self.load[3], 0.0, 1.5));
        let film = lube.rods[0].film;
        self.lube = Some(lube);
        load[0] = g.piston_stress_per_bar * piston_bar / (mp.fatigue() * derate(mp, self.piston_c));
        load[1] = (self.rod_comp_n / (g.rod_beam_area_m2 * mr.yld() * 1e6 * 0.75)).max(self.rod_tension_n / (g.rod_beam_area_m2 * 1e6) / mr.fatigue());
        load[2] = (0.25 * self.rod_tension_n / g.rod_bolt_stress_area_m2 / 2.0 / 1e6) / mrb.thread_fatigue_mpa.unwrap_or(1.0);
        load[3] = self.rod_comp_n.max(self.rod_tension_n) / (g.big_end_m * g.bearing_width_m) / 1e6 / (mb.load_rating_mpa.unwrap_or(1.0) * film);
        load[4] = 0.0035 * (self.rod_comp_n + 0.5 * self.rod_tension_n) / mc.fatigue();

        let clamp_n = g.head_bolts / 4.0 * 0.75 * mhb.yld() * 1e6 * g.head_bolt_area_m2;
        self.gasket_lift_bar = mg.seal_factor.unwrap_or(1.0) * clamp_n / piston_area / 1e5;
        // Boiling coolant lets the head run hot and grow, which unloads the gasket.
        let overheat = clamp(e.thermal.overheat_s / 20.0, 0.0, 1.0);
        load[5] = self.pmax_bar / self.gasket_lift_bar * (1.0 + 0.6 * overheat);
        let bolt_amp = g.head_joint_factor * self.pmax_bar * 1e5 * piston_area / (g.head_bolts / 4.0) / g.head_bolt_area_m2 / 2.0 / 1e6;
        load[6] = bolt_amp / mhb.thread_fatigue_mpa.unwrap_or(1.0);

        let block_c = e.ect + 10.0;
        let sleeve_c = e.ect + 30.0;
        let es = ms.modulus() * 1e3;
        let ea = mbl.modulus() * 1e3;
        let share = es * g.sleeve_wall_m / (es * g.sleeve_wall_m + ea * g.jacket_wall_m);
        let hoop = self.pmax_bar * 0.1 * (g.bore_m / 2.0) / g.sleeve_wall_m * share * g.open_deck_factor;
        let thermal = 0.5 * (mbl.expansion() - ms.expansion()) * 1e-6 * (sleeve_c - 20.0) * es * ea * g.jacket_wall_m / (es * g.sleeve_wall_m + ea * g.jacket_wall_m);
        self.sleeve_mpa = hoop + thermal;
        load[7] = goodman(hoop / 2.0, hoop / 2.0 + thermal, ms, 1.0);
        let web = g.main_web_kt * 0.6 * self.rod_comp_n / g.main_web_area_m2 / 1e6;
        load[8] = goodman(web / 2.0, web / 2.0, mbl, derate(mbl, block_c));

        let float_rpm = 9000.0 * if e.high_cam { 1.0 } else { 1.08 };
        let safe_rpm = float_rpm - 400.0;
        load[9] = if rpm <= safe_rpm { 0.8 * (rpm / safe_rpm).powi(2) } else { 0.8 + 0.2 * (rpm - safe_rpm) / 400.0 };
        self.load = load;

        let cycles = rpm / 120.0 * dt;
        for (i, p) in PARTS.iter().enumerate() {
            let n = cycles_to_failure(self.load[i]);
            if n.is_finite() {
                self.damage[i] = (self.damage[i] + cycles / n).min(1.0);
            }
            if self.damage[i] >= 1.0 && self.failed.is_none() {
                self.failed = Some(p.name);
            }
        }
    }
}
