//! The tune: FlashPro-style tables and settings. Same JSON shape the web app saves, so tunes move
//! between the two.

use crate::tables::Table;
use crate::thermal::Cooling;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Turbo {
    pub name: String,
    pub enabled: bool,
    /// Supercharger (or a turbo assumed to hold its target): boost follows the wastegate limit directly.
    pub fixed: bool,
    pub turbine_area_cm2: f64,
    pub wastegate_area_cm2: f64,
    pub compressor_tip_mm: f64,
    pub inertia_kg_m2: f64,
    pub turbine_eff: f64,
    pub compressor_eff: f64,
    pub max_boost_kpa: f64,
    pub spring_kpa: f64,
    pub wg_gain: f64,
    pub intercooler: bool,
    pub ic_effectiveness: f64,
    pub ic_drop_kpa_at250_gs: f64,
    /// Centre section has water ports plumbed into the cooling system.
    pub water_cooled: bool,
}

impl Default for Turbo {
    fn default() -> Self {
        crate::k20z3::turbo_gt35()
    }
}

/// Intake and exhaust changes on top of the stock flow geometry (metres).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct FlowOverrides {
    pub throttle_dia_m: Option<f64>,
    pub runner_m: Option<f64>,
    pub exhaust_dia_m: Option<f64>,
}

/// Fitted crank (or stroker kit), rods and pistons by catalog id. None = stock.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Parts {
    pub crank: Option<String>,
    pub piston: Option<String>,
    pub rod: Option<String>,
}

/// Fitted K-series block, head and oil pump by id (docs/engines.json). None = the K20Z3's own.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct EngineSel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oil_pump: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Tune {
    pub name: String,
    /// "lambda", "afr" or "ms"; None = guess from the fuel table's values.
    pub fuel_units: Option<String>,
    pub fuel_low: Table,
    pub fuel_high: Option<Table>,
    pub ign_low: Table,
    pub ign_high: Option<Table>,
    pub vtc_low: Option<Table>,
    pub vtc_high: Option<Table>,
    pub boost_target: Option<Table>,
    pub wg_duty: Option<Table>,
    pub vtec_rpm: f64,
    pub vtec_min_map: f64,
    pub rev_limit: f64,
    pub rev_limit_hyst: f64,
    pub octane: f64,
    pub turbo: Turbo,
    pub injector_cc: f64,
    pub injector_dead_ms: f64,
    pub clutch_nm: f64,
    pub overboost_kpa: Option<f64>,
    pub torque_limit_nm: Option<f64>,
    pub ambient_c: Option<f64>,
    /// Alloy picked per part (internals part key → material id).
    pub materials: BTreeMap<String, String>,
    pub parts: Parts,
    pub engine: EngineSel,
    pub flow: FlowOverrides,
    /// Cooling and oil system changes (thermostat, fans, oil grade, oil cooler); None = stock.
    pub cooling: Option<Cooling>,
}

impl Tune {
    /// The cooling and oil system as fitted (stock unless the tune changes it).
    pub fn cooling(&self) -> Cooling {
        self.cooling.clone().unwrap_or_default()
    }

    /// A tune with empty tables, for filling in field by field.
    pub fn empty() -> Tune {
        let t = Table { rows: vec![0.0], cols: vec![0.0], data: vec![vec![0.0]] };
        Tune {
            name: String::new(),
            fuel_units: None,
            fuel_low: t.clone(),
            fuel_high: None,
            ign_low: t,
            ign_high: None,
            vtc_low: None,
            vtc_high: None,
            boost_target: None,
            wg_duty: None,
            vtec_rpm: 5800.0,
            vtec_min_map: 70.0,
            rev_limit: 8000.0,
            rev_limit_hyst: 200.0,
            octane: 91.0,
            turbo: Turbo::default(),
            injector_cc: 310.0,
            injector_dead_ms: 0.7,
            clutch_nm: 320.0,
            overboost_kpa: None,
            torque_limit_nm: None,
            ambient_c: None,
            materials: BTreeMap::new(),
            parts: Parts::default(),
            engine: EngineSel::default(),
            flow: FlowOverrides::default(),
            cooling: None,
        }
    }
}

impl Default for Tune {
    fn default() -> Self {
        crate::k20z3::placeholder_tune()
    }
}
