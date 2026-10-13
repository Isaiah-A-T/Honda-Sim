//! The native model against the JavaScript model's results (native/sim/data/parity.json, written by
//! tools/export-native-data.mjs).

use honda_sim_core::engine::Engine;
use honda_sim_core::gasflow::{flow_defaults, Cond, Gas};
use honda_sim_core::internals::Internals;
use honda_sim_core::k20z3::{placeholder_tune, turbo_setup, VE_CORR_HIGH, VE_CORR_LOW};
use honda_sim_core::parts::bottom_end;
use honda_sim_core::sim::{Input, Sim};
use honda_sim_core::tune::{Parts, Tune};
use serde_json::Value;

fn data() -> Value {
    serde_json::from_str(include_str!("../data/parity.json")).unwrap()
}
fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(f64::NAN)
}
fn close(name: &str, a: f64, b: f64, rel: f64, abs: f64) {
    if a.is_nan() && b.is_nan() {
        return;
    }
    assert!((a - b).abs() <= abs + rel * b.abs(), "{name}: native {a} vs js {b}");
}

#[test]
fn calibration_matches_the_js_tables() {
    let d = data();
    let lo: Vec<f64> = d["veCorrLow"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let hi: Vec<f64> = d["veCorrHigh"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(lo, VE_CORR_LOW.to_vec(), "rerun tools/calibrate.mjs output into k20z3.rs");
    assert_eq!(hi, VE_CORR_HIGH.to_vec());
}

#[test]
fn gas_exchange_cycle_matches() {
    for case in data()["gas"].as_array().unwrap() {
        let c = &case["cond"];
        let cond = Cond {
            rpm: f(c, "rpm"), map_kpa: f(c, "mapKpa"), iat_c: f(c, "iatC"), high_cam: c["highCam"].as_bool().unwrap(), vtc: f(c, "vtc"),
            ign_deg: f(c, "ignDeg"), lambda: f(c, "lambda"), fire: c["fire"].as_bool().unwrap(), octane: f(c, "octane"),
            out_kpa: c["outKpa"].as_f64().unwrap_or(101.3), out_area_m2: c["outAreaM2"].as_f64(),
        };
        let mut g = Gas::new(flow_defaults());
        let o = g.settle(&cond, 40);
        let js = &case["out"];
        for (k, v) in [("ve", o.ve), ("veDelivered", o.ve_delivered), ("imepKpa", o.imep_kpa), ("pmepKpa", o.pmep_kpa), ("pmaxBar", o.pmax_bar),
            ("egtC", o.egt_c), ("exhKpa", o.exh_kpa), ("knockIndex", o.knock_index), ("residual", o.residual)] {
            close(&format!("{} @ {}", k, f(c, "rpm")), v, f(js, k), 1e-6, 1e-6);
        }
    }
}

fn tune_named(name: &str) -> Tune {
    match name {
        "turbo" => turbo_setup(&placeholder_tune()),
        "k24" => Tune { parts: Parts { crank: Some("honda-k24-99-hybrid".into()), piston: Some("arias-k24k20-87-11.0".into()), rod: None }, ..placeholder_tune() },
        _ => placeholder_tune(),
    }
}

#[test]
fn engine_at_full_throttle_matches() {
    for case in data()["engine"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let rpm = f(case, "rpm");
        let mut e = Engine::new(tune_named(name));
        let mut s = Internals::default();
        let mut t = 0.0;
        while t < 1.5 {
            e.rpm = rpm;
            e.step(1.0, 0.001);
            s.step(&e, 0.001);
            t += 0.001;
        }
        let tag = format!("{name} {rpm}");
        close(&format!("torque {tag}"), e.torque_nm, f(case, "torqueNm"), 1e-6, 1e-6);
        close(&format!("map {tag}"), e.map, f(case, "map"), 1e-6, 1e-6);
        close(&format!("boost {tag}"), e.boost, f(case, "boost"), 1e-6, 1e-6);
        close(&format!("pmax {tag}"), s.pmax_bar, f(case, "pmaxBar"), 1e-6, 1e-6);
        for k in ["piston", "rod", "bearing", "sleeve", "gasket", "block", "valvetrain"] {
            close(&format!("load {k} {tag}"), s.load_of(k), f(&case["load"], k), 1e-6, 1e-6);
        }
    }
}

#[test]
fn bottom_end_matches() {
    for case in data()["parts"].as_array().unwrap() {
        let p = &case["parts"];
        let s = |k: &str| p[k].as_str().map(String::from);
        let t = Tune { parts: Parts { crank: s("crank"), piston: s("piston"), rod: s("rod") }, ..placeholder_tune() };
        let b = bottom_end(&t);
        let tag = p.to_string();
        for (k, v) in [("boreMm", b.bore_mm), ("strokeMm", b.stroke_mm), ("rodMm", b.rod_mm), ("compression", b.compression), ("displacementL", b.displacement_l), ("pistonKg", b.piston_kg)] {
            close(&format!("{k} {tag}"), v, f(case, k), 1e-9, 1e-9);
        }
        assert_eq!(b.issues.len() as f64, f(case, "issues"), "{tag}");
    }
}

#[test]
fn launch_and_pull_matches() {
    let mut sim = Sim::new(placeholder_tune());
    sim.shift(1);
    let mut got = Vec::new();
    for i in 0..600 {
        if i == 250 {
            sim.shift(2);
        }
        sim.step(Input { throttle: 1.0, brake: 0.0, clutch: None }, 0.01);
        if i % 50 == 49 {
            got.push((sim.t, sim.eng.rpm, sim.car.speed_ms, sim.car.gear));
        }
    }
    for (g, js) in got.iter().zip(data()["drive"].as_array().unwrap()) {
        close("t", g.0, f(js, "t"), 1e-9, 1e-9);
        close(&format!("rpm at {}", g.0), g.1, f(js, "rpm"), 1e-4, 1e-3);
        close(&format!("speed at {}", g.0), g.2, f(js, "speedMs"), 1e-4, 1e-4);
        assert_eq!(g.3 as f64, f(js, "gear"));
    }
}

#[test]
fn render_state_has_the_web_fields() {
    let mut sim = Sim::new(placeholder_tune());
    sim.step(Input { throttle: 0.5, ..Default::default() }, 0.05);
    let s = honda_sim_core::simstate::render_state(&sim);
    for k in ["rpm", "crankDeg", "cams", "combustion", "flow", "induction", "parts", "geometry", "bottomEnd"] {
        assert!(!s[k].is_null(), "{k}");
    }
    assert_eq!(s["cams"]["card"]["intakeHigh"]["liftMm"], 11.7);
}
