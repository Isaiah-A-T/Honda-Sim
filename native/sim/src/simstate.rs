//! The sim's state for renderers and other modules, in the same JSON shape as the web app's
//! src/simstate.js (see docs/MODULES.md).

use crate::gasflow::{cam_card, valve_lift_m};
use crate::internals::{part_material, PARTS};
use crate::sim::Sim;
use serde_json::{json, Map, Value};

pub use crate::engine::CYL_OFFSET as FIRING_OFFSET_DEG;

fn num(x: f64) -> Value {
    if x.is_finite() { json!(x) } else { Value::Null }
}

pub fn render_state(sim: &Sim) -> Value {
    let e = &sim.eng;
    let s = &sim.internals;
    let t = &e.tune;
    let b = &e.bottom;
    let g = &e.geometry;
    let flow = e.flow();
    let firing = sim.running && !e.rev_cut && !e.dfco && !e.boost_cut;
    let mut parts = Map::new();
    for (i, p) in PARTS.iter().enumerate() {
        let m = part_material(t, b, p);
        parts.insert(p.key.into(), json!({
            "name": p.name, "load": s.load[i], "damage": s.damage[i], "failed": s.failed == Some(p.name),
            "material": { "name": m.name, "category": m.category, "densityKgM3": m.density_kg_m3 },
        }));
    }
    let induction = if !t.turbo.enabled { "na" } else if t.turbo.fixed { "supercharger" } else { "turbo" };
    json!({
        "running": sim.running,
        "rpm": e.rpm,
        "crankDeg": e.crank_deg,
        "firing": firing,
        "gear": sim.car.gear,
        "speedMs": sim.car.speed_ms,
        "throttlePct": e.tps,
        "mapKpa": e.map,
        "cams": {
            "highCam": e.high_cam, "vtecCommanded": e.vtec_cmd, "oilKpa": e.oil_kpa, "vtecPins": e.vtec_pins,
            "vtecEngagedShare": e.cam_f, "vtcDeg": e.vtc, "vtecRpm": t.vtec_rpm, "card": cam_card(flow),
        },
        "combustion": {
            "ignDeg": e.ign, "mbtDeg": e.mbt, "knockLimitDeg": e.knock_limit, "ca50Deg": num(e.ca50),
            "burnDeg": e.burn_deg, "knocking": e.knocking, "knockIndex": e.knock_index, "lambda": e.lambda,
        },
        "flow": {
            "ve": e.ve, "airGs": e.air_gs, "residual": e.residual, "pumpingKpa": e.pmep_kpa,
            "exhaustKpa": e.exh_kpa, "egtC": e.egt_c, "peakCylinderBar": s.pmax_bar,
        },
        "induction": induction,
        "intercooler": t.turbo.enabled && t.turbo.intercooler,
        "turbo": if t.turbo.enabled { json!({
            "chargeTempC": e.charge_out_c, "intercoolerDropKpa": e.ic_drop_kpa, "boostKpa": e.boost,
            "shaftRpm": e.turbo_rad_s * 30.0 / std::f64::consts::PI, "wastegateOpen": e.wg_open,
            "turbineKw": e.turbine_kw, "compressorKw": e.compressor_kw,
        }) } else { Value::Null },
        "temps": { "pistonC": s.piston_c, "iatC": e.iat, "ectC": e.ect },
        "parts": parts,
        "failed": s.failed,
        "forces": { "rodDownN": s.rod_comp_n, "rodUpN": s.rod_tension_n, "sleeveMPa": s.sleeve_mpa },
        "geometry": {
            "boreM": g.bore_m, "strokeM": g.stroke_m, "rodM": g.rod_m, "compression": g.compression, "displacementL": b.displacement_l,
            "deckM": b.deck_mm / 1000.0, "deckClearanceM": b.deck_clearance_mm / 1000.0, "inValveM": flow.in_valve_m, "exValveM": flow.ex_valve_m,
        },
        "bottomEnd": {
            "stock": b.stock, "crank": b.crank.name, "pistons": b.piston_name, "rods": b.rod_name,
            "block": b.crank.block.unwrap_or("K20Z3"), "issues": b.issues,
        },
    })
}

/// Valve lift (m) for cylinder 1–4 at a crank angle (cylinder 1 firing TDC = 0).
pub fn valve_lift(sim: &Sim, cyl: usize, intake: bool, crank_deg: f64) -> f64 {
    let e = &sim.eng;
    valve_lift_m(crank_deg - FIRING_OFFSET_DEG[cyl - 1], intake, e.high_cam, if intake { e.vtc } else { 0.0 }, e.flow())
}
