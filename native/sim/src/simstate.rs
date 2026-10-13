//! The sim's state for renderers and other modules, in the same JSON shape as the web app's
//! src/simstate.js (see docs/MODULES.md).

use crate::gasflow::{cam_card, valve_lift_m};
use crate::internals::{part_material, PARTS};
use crate::sim::Sim;
use crate::thermal::{boiling_c, circuits, viscosity_cst, COMPONENTS};
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
    let th = &e.thermal;
    let tg = &th.g;
    let tl = crate::sim::turbo_lines(e, sim.running);
    let c = circuits(th, e.oil_kpa, tl);
    let cooling = json!({
        "coolantC": th.coolant_c, "radiatorInC": th.coolant_c, "radiatorOutC": th.radiator_out_c, "pumpInletC": th.inlet_c,
        "boilingC": boiling_c(tg), "boiling": th.boiling, "overheatS": th.overheat_s,
        "thermostatOpen": th.thermostat, "thermostatOpenC": tg.thermostat_open_c, "thermostatFullC": tg.thermostat_full_c,
        "fanOn": th.fan_on, "fans": tg.fans, "fanOnC": tg.fan_on_c, "fanOffC": tg.fan_off_c, "airMs": th.air_ms,
        "pumpLpm": th.pump_lpm, "radiatorLpm": th.radiator_lpm, "bypassLpm": th.bypass_lpm,
        "radiatorKw": th.radiator_kw, "heatToCoolantKw": th.heat_to_coolant_kw, "heatToOilKw": th.heat_to_oil_kw,
        "oil": {
            "tempC": th.oil_c, "pressureKpa": e.oil_kpa, "viscosityCst": viscosity_cst(th.oil_c, tg), "grade": tg.oil_grade,
            "cooler": tg.oil_cooler, "coolerKw": th.oil_cooler_kw, "toCoolantKw": th.oil_to_coolant_kw, "sumpKw": th.sump_kw,
        },
        "turbo": match tl { Some(tl) => json!({
            "oilFeedLpm": th.turbo_oil_lpm, "oilKw": th.turbo_oil_kw, "waterKw": th.turbo_water_kw, "waterCooled": tl.water_cooled,
            "housingC": th.turbo_housing_c, "bearingC": th.turbo_bearing_c, "cokingS": th.turbo_coke_s,
        }), None => Value::Null },
        "coolantCircuit": c.coolant, "oilCircuit": c.oil,
    });
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
        "cooling": cooling,
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

/// Where each coolant and oil component sits (approximate, for drawing). Static, so it's not in render_state.
pub fn cooling_components() -> Value {
    json!(COMPONENTS)
}
