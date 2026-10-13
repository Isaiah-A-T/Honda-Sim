//! What the viewer shows, as sent by the sim (JSON over the local connection) or made up by the demo.
//! Field names match the sim's src/viewer-link.js.

use serde::{Deserialize, Serialize};
use crate::kinematics::{CamSpecs, Geometry};
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
    /// Turbo centre section is water-cooled (coolant lines to the block), as well as oil-fed (ignored when N/A).
    pub turbo_water_cooled: bool,
    /// Intake air temperature at the throttle body, °C.
    pub iat_c: f32,
    /// Coolant temperature leaving the engine (water outlet), °C.
    pub coolant_c: f32,
    /// Coolant temperature leaving the radiator, °C. None = estimated from the thermostat and fan.
    pub coolant_out_c: Option<f32>,
    /// Oil temperature, °C.
    pub oil_c: f32,
    /// Oil pressure, kPa. None = estimated from rpm.
    pub oil_kpa: Option<f32>,
    /// Thermostat opening, 0 (shut, coolant goes round the bypass) to 1 (fully open, through the radiator).
    pub thermostat: f32,
    /// Radiator fan speed, 0 (off) to 1 (full).
    pub fan: f32,
    /// The K20Z3 cam card. None = the viewer's stand-in figures.
    pub cams: Option<CamSpecs>,
    /// VTEC lock pin travel, 0 (low cam) to 1 (locked to the VTEC lobe), when the sim models it: one value for all
    /// cylinders or one per cylinder (1–4). None = the viewer moves the pins itself from `vtec`, each cylinder's only
    /// while its followers are on the base circle.
    pub vtec_pin: Option<StressValue>,
    pub stress: HashMap<String, StressValue>,
    /// The sim's own materials per part (read-only in the viewer). None = use the placeholder catalog.
    pub part_materials: Option<HashMap<String, PartMaterial>>,
    /// Extra readouts the sim wants shown, label → value text.
    pub readouts: Vec<(String, String)>,
    /// Block fitted: "K20Z3" (default) or "K24A". Sets the bore, stroke, rod length and deck height below default to.
    pub block: Option<String>,
    /// Bottom-end dimensions, mm, when the sim's parts differ from the block's stock ones.
    pub bore_mm: Option<f32>,
    pub stroke_mm: Option<f32>,
    pub rod_mm: Option<f32>,
    pub deck_mm: Option<f32>,
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
            turbo_water_cooled: true,
            iat_c: 25.0,
            coolant_c: 88.0,
            coolant_out_c: None,
            oil_c: 95.0,
            oil_kpa: None,
            thermostat: 1.0,
            fan: 0.0,
            cams: None,
            vtec_pin: None,
            stress: HashMap::new(),
            part_materials: None,
            readouts: Vec::new(),
            block: None,
            bore_mm: None,
            stroke_mm: None,
            rod_mm: None,
            deck_mm: None,
        }
    }
}

impl SimState {
    /// The engine's dimensions: the block's stock ones with any the sim sent on top. The deck follows a changed
    /// stroke or rod (compression height kept) unless the sim sent it too.
    pub fn geometry(&self) -> Geometry {
        let base = match self.block.as_deref().map(|b| b.to_ascii_uppercase()) {
            Some(b) if b.starts_with("K24") => Geometry::K24A,
            _ => Geometry::K20Z3,
        };
        let ok = |v: Option<f32>, lo: f32, hi: f32| v.filter(|x| x.is_finite() && (lo..=hi).contains(x));
        let bore = ok(self.bore_mm, 70.0, 95.0).unwrap_or(base.bore);
        let stroke = ok(self.stroke_mm, 70.0, 110.0).unwrap_or(base.stroke);
        let rod = ok(self.rod_mm, 120.0, 170.0).unwrap_or(base.rod);
        let comp = base.deck - base.stroke / 2.0 - base.rod;
        let deck = ok(self.deck_mm, 190.0, 250.0).unwrap_or(stroke / 2.0 + rod + comp);
        Geometry { bore, stroke, rod, deck }
    }

    /// Stress for a part (worst cylinder when `index` is None and the sim sent one value per cylinder).
    /// Pin travel for cylinder `cyl` (1–4), if the sim sent it.
    pub fn vtec_pin_of(&self, cyl: usize) -> Option<f32> {
        match self.vtec_pin.as_ref()? {
            StressValue::One(v) => Some(*v),
            StressValue::PerCylinder(v) => v.get(cyl - 1).copied(),
        }
    }

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
                "readouts":[["Peak pressure","95 bar"]],"unknownField":1,"vtecPin":[1,0.5,0,0]}"#,
        )
        .unwrap();
        assert_eq!(s.rpm, 6500.0);
        assert!(s.vtec && s.turbo);
        assert_eq!(s.stress_of("rods", None), 0.9);
        assert_eq!(s.stress_of("pistons", Some(2)), 0.7);
        assert_eq!(s.stress_of("pistons", None), 0.8);
        assert_eq!(s.vtec_pin_of(2), Some(0.5));
        assert_eq!(s.part_materials.as_ref().unwrap()["pistons"].family, "aluminum");
        assert_eq!(s.readouts[0].1, "95 bar");
        assert_eq!(s.ign_deg, 15.0); // default kept
        assert_eq!(s.geometry(), crate::kinematics::Geometry::K20Z3);
    }

    #[test]
    fn geometry_follows_the_block_and_parts() {
        let s: SimState = serde_json::from_str(r#"{"block":"K24A"}"#).unwrap();
        assert_eq!(s.geometry(), crate::kinematics::Geometry::K24A);
        // Longer rods on a K24: the deck follows (compression height kept) unless the sim sends it.
        let s: SimState = serde_json::from_str(r#"{"block":"K24A","rodMm":160}"#).unwrap();
        assert!((s.geometry().deck - (49.5 + 160.0 + 30.0)).abs() < 1e-3);
        let s: SimState = serde_json::from_str(r#"{"boreMm":87,"strokeMm":99,"rodMm":152,"deckMm":231.5}"#).unwrap();
        assert_eq!(s.geometry(), crate::kinematics::Geometry::K24A);
    }
}
