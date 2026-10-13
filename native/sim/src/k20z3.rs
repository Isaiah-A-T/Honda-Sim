//! 2006–2011 Civic Si (K20Z3, 6-speed): car and engine constants, calibration corrections and the
//! placeholder tune (src/k20z3.js). See the JS file for where each number comes from.

use crate::tables::{clamp, lookup1, Table};
use crate::tune::{Tune, Turbo};

pub struct Car {
    pub mass_kg: f64,
    pub gears: [f64; 6],
    pub reverse: f64,
    pub final_drive: f64,
    pub tire_radius_m: f64,
    pub driveline_eff: f64,
    pub cd: f64,
    pub frontal_area_m2: f64,
    pub crr: f64,
    pub clutch_capacity_nm: f64,
    pub traction_mu: f64,
    pub front_weight: f64,
}

pub const CAR: Car = Car {
    mass_kg: 1305.0 + 80.0,
    gears: [3.267, 2.130, 1.517, 1.147, 0.921, 0.659],
    reverse: 3.583,
    final_drive: 4.765,
    tire_radius_m: 0.3127,
    driveline_eff: 0.88,
    cd: 0.31,
    frontal_area_m2: 2.0,
    crr: 0.012,
    clutch_capacity_nm: 320.0,
    traction_mu: 1.0,
    front_weight: 0.61,
};

pub struct EngineConst {
    pub displacement_l: f64,
    pub cylinders: f64,
    pub inertia_kg_m2: f64,
    pub injector_cc_min: f64,
    pub injector_dead_ms: f64,
    pub stall_rpm: f64,
    pub idle_rpm: f64,
}

pub const ENGINE: EngineConst = EngineConst {
    displacement_l: 1.998,
    cylinders: 4.0,
    inertia_kg_m2: 0.16,
    injector_cc_min: 310.0,
    injector_dead_ms: 0.7,
    stall_rpm: 450.0,
    idle_rpm: 750.0,
};

pub const VE_RPM: [f64; 13] = [1000., 2000., 3000., 3500., 4000., 4500., 5000., 5500., 5800., 6500., 7500., 8500., 9000.];
pub const VE_HIGH_RPM: [f64; 13] = [1000., 3000., 4000., 5000., 5500., 5800., 6200., 6800., 7400., 7800., 8200., 8600., 9000.];
// Written by tools/calibrate.mjs into src/k20z3.js; tools/export-native-data.mjs checks they match.
pub const VE_CORR_LOW: [f64; 13] = [0.7385, 0.7563, 0.7949, 0.8228, 0.8598, 0.8923, 0.8825, 0.865, 0.8535, 0.8336, 0.8328, 0.7903, 0.834];
pub const VE_CORR_HIGH: [f64; 13] = [0.4896, 0.6341, 0.7635, 0.8364, 0.8624, 0.9419, 1.0363, 1.0194, 1.0664, 1.0444, 1.0092, 0.9799, 0.9905];

pub fn ve_correction(rpm: f64, high_cam: bool) -> f64 {
    if high_cam { lookup1(&VE_HIGH_RPM, &VE_CORR_HIGH, rpm) } else { lookup1(&VE_RPM, &VE_CORR_LOW, rpm) }
}

/// Spark advance for best torque (degrees BTDC), a quick fit to the gas model.
pub fn mbt(rpm: f64, map_kpa: f64) -> f64 {
    clamp(17.0 + 3.0 * rpm.min(6500.0) / 1000.0 + (100.0 - map_kpa) * 0.13, 8.0, 45.0)
}

/// Highest advance before knock (degrees BTDC), a quick fit used for the placeholder tune.
pub fn knock_limit(rpm: f64, map_kpa: f64, iat_c: f64, octane: f64) -> f64 {
    (4.0 + 5.5 * rpm / 1000.0).min(28.0 + rpm / 1000.0) - (map_kpa - 100.0) * 0.15 - (iat_c - 30.0).max(0.0) * 0.12 + (octane - 91.0) * 1.2
}

/// Mechanical friction torque (Nm).
pub fn friction_nm(rpm: f64) -> f64 {
    5.0 + 0.0012 * rpm + 0.0000003 * rpm * rpm
}

const RPM_AXIS: [f64; 16] = [500., 1000., 1500., 2000., 2500., 3000., 3500., 4000., 4500., 5000., 5500., 6000., 6500., 7000., 7500., 8000.];
const MAP_AXIS: [f64; 9] = [20., 30., 40., 50., 60., 70., 80., 90., 100.];

fn js_round2(v: f64) -> f64 {
    // Math.round(v * 100) / 100 (rounds half up, like JS).
    (v * 100.0 + 0.5).floor() / 100.0
}

fn table(f: impl Fn(f64, f64) -> f64) -> Table {
    Table {
        rows: RPM_AXIS.to_vec(),
        cols: MAP_AXIS.to_vec(),
        data: RPM_AXIS.iter().map(|&r| MAP_AXIS.iter().map(|&m| js_round2(f(r, m))).collect()).collect(),
    }
}

pub fn turbo_gt35() -> Turbo {
    Turbo {
        name: "GT35-class turbo (estimated)".into(),
        enabled: false,
        fixed: false,
        turbine_area_cm2: 9.5,
        wastegate_area_cm2: 9.0,
        compressor_tip_mm: 41.0,
        inertia_kg_m2: 3.0e-5,
        turbine_eff: 0.68,
        compressor_eff: 0.72,
        max_boost_kpa: 160.0,
        spring_kpa: 48.0,
        wg_gain: 1.0,
        intercooler: true,
        ic_effectiveness: 0.75,
        ic_drop_kpa_at250_gs: 8.0,
    }
}

pub fn turbo_6262() -> Turbo {
    Turbo {
        name: "Precision 6262 (estimated)".into(),
        turbine_area_cm2: 10.5,
        wastegate_area_cm2: 9.0,
        compressor_tip_mm: 44.0,
        inertia_kg_m2: 3.5e-5,
        turbine_eff: 0.70,
        compressor_eff: 0.75,
        max_boost_kpa: 200.0,
        ..turbo_gt35()
    }
}

/// Placeholder tune (not a real calibration) so the sim runs before real tables are pasted in.
pub fn placeholder_tune() -> Tune {
    let ign = |r: f64, m: f64| if r <= 1000.0 && m <= 40.0 { 15.0 } else { mbt(r, m).min(knock_limit(r, m, 25.0, 91.0) - 2.0) };
    let lam = |r: f64, m: f64| if m < 70.0 { 1.0 } else if m < 90.0 { 0.95 } else if r > 5000.0 { 0.85 } else { 0.88 };
    Tune {
        name: "Placeholder (not a real calibration)".into(),
        fuel_units: Some("lambda".into()),
        fuel_low: table(lam),
        fuel_high: Some(table(lam)),
        ign_low: table(ign),
        ign_high: Some(table(|r, m| ign(r, m) + 1.0)),
        vtec_rpm: 5800.0,
        vtec_min_map: 70.0,
        rev_limit: 8000.0,
        rev_limit_hyst: 200.0,
        octane: 91.0,
        turbo: turbo_gt35(),
        injector_cc: ENGINE.injector_cc_min,
        injector_dead_ms: ENGINE.injector_dead_ms,
        clutch_nm: CAR.clutch_capacity_nm,
        ..Tune::empty()
    }
}

/// Turbo build on stock internals (GT35-class, 1000 cc injectors, overboost cut, torque warning).
pub fn turbo_setup(tune: &Tune) -> Tune {
    Tune {
        turbo: Turbo { enabled: true, ..turbo_gt35() },
        injector_cc: 1000.0,
        injector_dead_ms: 1.0,
        overboost_kpa: Some(200.0),
        torque_limit_nm: Some(350.0),
        clutch_nm: 450.0,
        ..tune.clone()
    }
}
