//! Fuels and the fuel system that delivers them (src/fuels.js): rail pressure, injectors, pump.

use crate::tune::Tune;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fuel {
    pub id: &'static str,
    pub name: String,
    pub octane: f64,
    pub stoich: f64,
    pub lhv_mj: f64,
    pub hv_kj: f64,
    pub density: f64,
}

const GAS: (f64, f64, f64) = (43.4, 349.0, 0.745);
const ETH: (f64, f64, f64) = (26.7, 921.0, 0.794);
fn blend(eth_vol: f64) -> (f64, f64, f64) {
    let m = eth_vol * ETH.2 / (eth_vol * ETH.2 + (1.0 - eth_vol) * GAS.2);
    (m * ETH.0 + (1.0 - m) * GAS.0, m * ETH.1 + (1.0 - m) * GAS.1, eth_vol * ETH.2 + (1.0 - eth_vol) * GAS.2)
}

pub fn fuels() -> Vec<Fuel> {
    let e10 = blend(0.10);
    let e85 = blend(0.85);
    let f = |id: &'static str, name: &str, octane: f64, stoich: f64, p: (f64, f64, f64)| Fuel { id, name: name.into(), octane, stoich, lhv_mj: p.0, hv_kj: p.1, density: p.2 };
    vec![
        f("pump87", "87 pump gas (E10)", 87.0, 14.1, e10),
        f("pump91", "91 pump gas (E10)", 91.0, 14.1, e10),
        f("pump93", "93 pump gas (E10)", 93.0, 14.1, e10),
        f("e85", "E85", 96.0, 9.7, e85),
        f("sunoco260", "Sunoco 260 GT (100 octane)", 100.0, 14.1, (41.6, e10.1, 0.734)),
        f("c16", "VP C16 race fuel (118 octane)", 118.0, 14.7, (GAS.0, GAS.1, 0.735)),
    ]
}
pub fn ref_fuel() -> Fuel {
    fuels().swap_remove(1)
}

/// The fitted fuel; tunes with only an octane get pump gas with it.
pub fn fuel_of(t: &Tune) -> Fuel {
    if let Some(f) = t.fuel.as_deref().and_then(|id| fuels().into_iter().find(|f| f.id == id)) {
        return f;
    }
    Fuel { id: "pump", name: format!("{} pump gas (E10)", t.octane), octane: t.octane, ..ref_fuel() }
}

pub const RAIL_KPA: f64 = 50.0 * 6.895;
pub const INJECTOR_RATED_KPA: f64 = 300.0;
pub const STOCK_PUMP_LPH: f64 = 160.0;
pub const PUMP_LPH_MIN: f64 = 20.0;
pub const PUMP_LPH_MAX: f64 = 1000.0;
pub const PUMP_LPH_STEP: f64 = 10.0;

/// JS Math.round (halves round up).
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}
pub fn pump_lph_of(t: &Tune) -> f64 {
    js_round(t.fuel_pump_lph.unwrap_or(STOCK_PUMP_LPH).clamp(PUMP_LPH_MIN, PUMP_LPH_MAX) / PUMP_LPH_STEP) * PUMP_LPH_STEP
}
pub fn injector_g_per_ms(cc: f64, fuel: &Fuel, map_kpa: f64) -> f64 {
    let dp = (RAIL_KPA + 101.3 - map_kpa).max(0.0);
    cc * fuel.density / 60000.0 * (dp / INJECTOR_RATED_KPA).sqrt()
}
pub fn pump_gs(lph: f64, fuel: &Fuel) -> f64 {
    lph * fuel.density / 3.6
}
/// Extra charge cooling (K) from the fuel evaporating, relative to the calibration fuel at the same lambda.
pub fn extra_cooling_k(fuel: &Fuel, lambda: f64) -> f64 {
    let lam = lambda.max(0.5);
    let q = |f: &Fuel| f.hv_kj * 1000.0 / (f.stoich * lam);
    (q(fuel) - q(&ref_fuel())) / 1005.0
}
