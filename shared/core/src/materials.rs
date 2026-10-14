//! Material library with published properties and sources (same data as web/src/materials.js).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Material {
    pub name: String,
    pub category: String,
    #[serde(rename = "densityKgM3")]
    pub density_kg_m3: Option<f64>,
    #[serde(rename = "modulusGPa")]
    pub modulus_gpa: Option<f64>,
    #[serde(rename = "yieldMPa")]
    pub yield_mpa: Option<f64>,
    #[serde(rename = "utsMPa")]
    pub uts_mpa: Option<f64>,
    #[serde(rename = "fatigueMPa")]
    pub fatigue_mpa: Option<f64>,
    pub elongation_pct: Option<f64>,
    #[serde(rename = "thermalExpansionUmMK")]
    pub thermal_expansion_um_mk: Option<f64>,
    #[serde(rename = "conductivityWmK")]
    pub conductivity_w_mk: Option<f64>,
    pub max_service_temp_c: Option<f64>,
    pub strength_vs_temp_c: Option<Vec<[f64; 2]>>,
    #[serde(rename = "loadRatingMPa")]
    pub load_rating_mpa: Option<f64>,
    pub seal_factor: Option<f64>,
    #[serde(rename = "threadFatigueMPa")]
    pub thread_fatigue_mpa: Option<f64>,
    pub estimated: BTreeMap<String, String>,
    pub sources: Vec<String>,
}

impl Material {
    pub fn density(&self) -> f64 { self.density_kg_m3.unwrap_or(0.0) }
    pub fn fatigue(&self) -> f64 { self.fatigue_mpa.unwrap_or(1.0) }
    pub fn yld(&self) -> f64 { self.yield_mpa.unwrap_or(1.0) }
    pub fn uts(&self) -> f64 { self.uts_mpa.unwrap_or(1.0) }
    pub fn modulus(&self) -> f64 { self.modulus_gpa.unwrap_or(1.0) }
    pub fn expansion(&self) -> f64 { self.thermal_expansion_um_mk.unwrap_or(0.0) }
}

/// All materials by id, in the library's order.
pub fn materials() -> &'static Vec<(String, Material)> {
    static M: OnceLock<Vec<(String, Material)>> = OnceLock::new();
    M.get_or_init(|| {
        let v: serde_json::Map<String, serde_json::Value> = serde_json::from_str(include_str!("../data/materials.json")).expect("materials");
        v.into_iter().map(|(k, m)| (k, serde_json::from_value(m).expect("material"))).collect()
    })
}

pub fn material(id: &str) -> Option<&'static Material> {
    materials().iter().find(|(k, _)| k == id).map(|(_, m)| m)
}

pub fn materials_for(category: &str) -> Vec<&'static str> {
    materials().iter().filter(|(_, m)| m.category == category).map(|(k, _)| k.as_str()).collect()
}
