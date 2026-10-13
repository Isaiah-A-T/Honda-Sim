//! What the viewer shows, as sent by the sim (JSON over the local connection) or made up by the demo.
//! Field names match the sim's src/viewer-link.js.

use serde::{Deserialize, Serialize};
use crate::kinematics::CamSpecs;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum StressValue {
    One(f32),
    PerCylinder(Vec<f32>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PartMaterial {
    #[serde(default)]
    pub part: String,
    pub label: String,
    pub family: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct SimState {
    pub rpm: f32,
    pub running: bool,
    pub throttle: f32,
    pub map_kpa: f32,
    pub boost_kpa: f32,
    pub air_gs: f32,
    pub vtec: bool,
    pub vtc_deg: f32,
    pub ign_deg: f32,
    pub knock: bool,
    pub fuel_cut: bool,
    pub lambda: f32,
    pub turbo: bool,
    /// Turbo build has an intercooler between the compressor and the throttle body (ignored when N/A).
    pub intercooler: bool,
    /// Intake air temperature at the throttle body, °C.
    pub iat_c: f32,
    /// The K20Z3 cam card. None = the viewer's stand-in figures.
    pub cams: Option<CamSpecs>,
    /// VTEC lock pin travel, 0 (low cam) to 1 (locked to the VTEC lobe), when the sim models it. None = the viewer
    /// moves the pins itself from `vtec`, each cylinder's only while its followers are on the base circle.
    pub vtec_pin: Option<f32>,
    pub stress: HashMap<String, StressValue>,
    /// The sim's own materials per part (read-only in the viewer). None = use the placeholder catalog.
    pub part_materials: Option<HashMap<String, PartMaterial>>,
    /// Extra readouts the sim wants shown, label → value text.
    pub readouts: Vec<(String, String)>,
}

impl Default for SimState {
    fn default() -> Self {
        SimState {
            rpm: 0.0,
            running: true,
            throttle: 0.0,
            map_kpa: 30.0,
            boost_kpa: 0.0,
            air_gs: 0.0,
            vtec: false,
            vtc_deg: 0.0,
            ign_deg: 15.0,
            knock: false,
            fuel_cut: false,
            lambda: 1.0,
            turbo: false,
            intercooler: true,
            iat_c: 25.0,
            cams: None,
            vtec_pin: None,
            stress: HashMap::new(),
            part_materials: None,
            readouts: Vec::new(),
        }
    }
}

impl SimState {
    /// Stress for a part (worst cylinder when `index` is None and the sim sent one value per cylinder).
    pub fn stress_of(&self, key: &str, index: Option<usize>) -> f32 {
        match self.stress.get(key) {
            Some(StressValue::One(v)) => *v,
            Some(StressValue::PerCylinder(v)) => match index {
                Some(i) => v.get(i).copied().unwrap_or(0.0),
                None => v.iter().copied().fold(0.0, f32::max),
            },
            None => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_sims_json() {
        let s: SimState = serde_json::from_str(
            r#"{"rpm":6500,"throttle":1,"mapKpa":180,"vtec":true,"vtcDeg":30,"turbo":true,
                "stress":{"rods":0.9,"pistons":[0.5,0.6,0.7,0.8]},
                "partMaterials":{"pistons":{"label":"Cast","family":"aluminum"}},
                "readouts":[["Peak pressure","95 bar"]],"unknownField":1}"#,
        )
        .unwrap();
        assert_eq!(s.rpm, 6500.0);
        assert!(s.vtec && s.turbo);
        assert_eq!(s.stress_of("rods", None), 0.9);
        assert_eq!(s.stress_of("pistons", Some(2)), 0.7);
        assert_eq!(s.stress_of("pistons", None), 0.8);
        assert_eq!(s.part_materials.unwrap()["pistons"].family, "aluminum");
        assert_eq!(s.readouts[0].1, "95 bar");
        assert_eq!(s.ign_deg, 15.0); // default kept
    }
}
