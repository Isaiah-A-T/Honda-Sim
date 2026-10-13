//! What the 3D view is shown: the sim's state in the viewer's `SimState` (same mapping as the web
//! app's src/viewer-link.js, plus the cam card, per-cylinder VTEC pins and the fitted parts' materials).

use honda_sim_core::engine::hp;
use honda_sim_core::gasflow::cam_card;
use honda_sim_core::internals::{part_index, part_material, PARTS};
use honda_sim_core::Sim;
use honda_sim_viewer::state::{PartMaterial, SimState, StressValue};
use honda_sim_viewer::kinematics::{CamSpec, CamSpecs};
use std::collections::HashMap;

fn family_of(name: &str) -> &'static str {
    let n = name.to_lowercase();
    if n.contains("alumin") { "aluminum" }
    else if n.contains("titan") { "titanium" }
    else if n.contains("4340") || n.contains("4330") || n.contains("chrom") || n.contains("300m") { "chromoly" }
    else if n.contains("stainless") { "stainless" }
    else if n.contains("inconel") || n.contains("nickel") { "nickel" }
    else if n.contains("iron") { "iron" }
    else { "steel" }
}

pub fn viewer_state(sim: &Sim) -> SimState {
    let e = &sim.eng;
    let s = &sim.internals;
    let t = &e.tune;
    let running = sim.running;
    let tq = if running { e.torque_nm.max(0.0) } else { 0.0 };
    let firing = running && !e.rev_cut && !e.dfco && !e.boost_cut;
    let worst = |keys: &[&str]| keys.iter().map(|k| { let i = part_index(k); if s.damage[i] >= 1.0 { 1.3 } else { s.load[i] } }).fold(0.0, f64::max);
    let spin = (e.rpm / (t.rev_limit.max(6000.0) + 600.0)).powi(2);
    let rod_limit = t.torque_limit_nm.unwrap_or(350.0);
    let turbo_on = t.turbo.enabled;
    let mut stress = HashMap::new();
    let mut put = |k: &str, v: f64| { stress.insert(k.to_string(), StressValue::One(v as f32)); };
    put("pistons", worst(&["piston"]));
    put("rods", worst(&["rod", "rodBolt", "bearing"]));
    put("crank", worst(&["crank"]));
    put("valves", worst(&["valvetrain"]));
    put("cams", worst(&["valvetrain"]) * 0.75);
    put("head", worst(&["gasket"]).max(if e.knocking { 0.35 } else { 0.0 } + ((e.iat - 25.0) / 60.0).clamp(0.0, 0.5)));
    put("block", worst(&["sleeve", "block"]).max(tq / rod_limit * 0.45));
    put("exhaust", if firing { ((e.egt_c - 300.0) / 700.0).clamp(0.0, 1.3) } else { 0.0 });
    put("intake", match t.overboost_kpa { Some(ob) if turbo_on => ((e.map - 101.0) / (ob - 101.0)).clamp(0.0, 1.3), _ => 0.0 });
    put("turbo", if turbo_on { e.boost.max(0.0) / t.turbo.max_boost_kpa.max(1.0) } else { 0.0 });
    put("clutch", tq / t.clutch_nm.max(1.0));
    put("flywheel", spin * 0.5);

    let mut pm = HashMap::new();
    for (key, part) in [("pistons", "piston"), ("rods", "rod"), ("crank", "crank"), ("valves", "valvetrain"), ("cams", "valvetrain")] {
        let p = &PARTS[part_index(part)];
        let m = part_material(t, &e.bottom, p);
        pm.insert(key.to_string(), PartMaterial {
            part: p.name.to_string(), label: m.name.clone(), family: family_of(&m.name).to_string(),
            detail: format!("yield {} MPa, tensile {} MPa", m.yield_mpa.unwrap_or(0.0), m.uts_mpa.unwrap_or(0.0)),
        });
    }
    let card = cam_card(e.flow());
    let spec = |l: honda_sim_core::gasflow::Lobe| CamSpec { lift_mm: l.lift_mm as f32, duration_deg: l.duration_deg as f32, centerline_deg: l.centerline_deg as f32 };
    let gear = match sim.car.gear { 0 => "N".to_string(), g if g < 0 => "R".to_string(), g => g.to_string() };
    let mut readouts = vec![
        ("Gear".to_string(), gear),
        ("Torque".to_string(), format!("{:.0} Nm  {:.0} hp", tq, hp(tq, e.rpm))),
        ("Displacement".to_string(), format!("{:.0} cc, {:.1}:1", e.bottom.displacement_l * 1000.0, e.bottom.compression)),
        ("Peak pressure".to_string(), format!("{:.0} bar", s.pmax_bar)),
        ("Piston crown".to_string(), format!("{:.0} °C", s.piston_c)),
        ("EGT".to_string(), format!("{:.0} °C", e.egt_c)),
    ];
    if let Some(f) = s.failed {
        readouts.push(("FAILED".to_string(), f.to_string()));
    }
    SimState {
        rpm: e.rpm.max(0.0) as f32,
        running,
        throttle: (e.tps / 100.0).clamp(0.0, 1.0) as f32,
        map_kpa: e.map as f32,
        boost_kpa: e.boost.max(0.0) as f32,
        air_gs: e.air_gs as f32,
        vtec: e.high_cam,
        vtc_deg: e.vtc as f32,
        ign_deg: e.ign as f32,
        knock: e.knocking,
        fuel_cut: e.rev_cut || e.dfco || e.boost_cut,
        lambda: if e.lambda > 5.0 { 1.0 } else { e.lambda as f32 },
        turbo: turbo_on,
        intercooler: turbo_on && t.turbo.intercooler,
        iat_c: e.iat as f32,
        cams: Some(CamSpecs { intake_low: spec(card.intake_low), intake_high: spec(card.intake_high), exhaust_low: spec(card.exhaust_low), exhaust_high: spec(card.exhaust_high) }),
        vtec_pin: Some(StressValue::PerCylinder(e.vtec_pins.iter().map(|p| *p as f32).collect())),
        stress,
        part_materials: Some(pm),
        readouts,
    }
}
