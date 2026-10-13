//! The native model against the JavaScript model's results (native/sim/data/parity.json, written by
//! tools/export-native-data.mjs).

use honda_sim_core::engine::Engine;
use honda_sim_core::gasflow::{flow_defaults, Cond, Gas};
use honda_sim_core::internals::Internals;
use honda_sim_core::k20z3::{placeholder_tune, turbo_setup, VE_CORR_HIGH, VE_CORR_LOW};
use honda_sim_core::parts::bottom_end;
use honda_sim_core::sim::{Input, Sim};
use honda_sim_core::thermal::{Cooling, Thermal, ThermalInput};
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
            out_kpa: c["outKpa"].as_f64().unwrap_or(101.3), wall_c: c["wallC"].as_f64().unwrap_or(90.0), out_area_m2: c["outAreaM2"].as_f64(),
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
            got.push((sim.t, sim.eng.rpm, sim.car.speed_ms, sim.car.gear, sim.eng.ect, sim.eng.oil_c, sim.eng.oil_kpa));
        }
    }
    for (g, js) in got.iter().zip(data()["drive"].as_array().unwrap()) {
        close("t", g.0, f(js, "t"), 1e-9, 1e-9);
        close(&format!("rpm at {}", g.0), g.1, f(js, "rpm"), 1e-4, 1e-3);
        close(&format!("speed at {}", g.0), g.2, f(js, "speedMs"), 1e-4, 1e-4);
        assert_eq!(g.3 as f64, f(js, "gear"));
        close(&format!("ect at {}", g.0), g.4, f(js, "ectC"), 1e-6, 1e-6);
        close(&format!("oil at {}", g.0), g.5, f(js, "oilC"), 1e-6, 1e-6);
        close(&format!("oil pressure at {}", g.0), g.6, f(js, "oilKpa"), 1e-5, 1e-4);
    }
}

#[test]
fn cooling_matches() {
    for case in data()["thermal"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let g: Cooling = serde_json::from_value(case["overrides"].clone()).unwrap();
        let l = &case["load"];
        let input = ThermalInput {
            rpm: f(l, "rpm"), fuel_gs: f(l, "fuelGs"), friction_w: f(l, "frictionW"), heat_loss: f(l, "heatLoss"),
            speed_ms: f(l, "speedMs"), map_kpa: f(l, "mapKpa"), ambient_c: f(l, "ambientC"), turbo: None,
        };
        let mut th = Thermal::new(input.ambient_c, name != "dyno", g);
        let rows = case["rows"].as_array().unwrap();
        let mut k = 0;
        for i in 1..=rows.len() * 600 {
            th.step(&input, 0.05);
            if i % 600 == 0 {
                let js = &rows[k];
                let tag = format!("{name} at {}", f(js, "t"));
                close(&format!("{tag} coolant"), th.coolant_c, f(js, "coolantC"), 1e-7, 1e-7);
                close(&format!("{tag} radiator"), th.radiator_c, f(js, "radiatorC"), 1e-7, 1e-7);
                close(&format!("{tag} oil"), th.oil_c, f(js, "oilC"), 1e-7, 1e-7);
                close(&format!("{tag} thermostat"), th.thermostat, f(js, "thermostat"), 1e-7, 1e-7);
                assert_eq!(th.fan_on, js["fanOn"].as_bool().unwrap(), "{tag} fan");
                assert_eq!(th.boiling, js["boiling"].as_bool().unwrap(), "{tag} boiling");
                k += 1;
            }
        }
    }
}

#[test]
fn cold_start_matches() {
    let mut sim = Sim::new(placeholder_tune());
    sim.cold_engine();
    let rows = data()["coldStart"].as_array().unwrap().clone();
    let mut k = 0;
    for i in 0..4000 {
        if i == 2000 {
            sim.shift(1);
        }
        sim.step(Input { throttle: if i < 2000 { 0.0 } else { 0.3 }, brake: 0.0, clutch: None }, 0.01);
        if i % 250 == 249 {
            let js = &rows[k];
            let tag = format!("cold start at {:.2}", sim.t);
            close(&format!("{tag} rpm"), sim.eng.rpm, f(js, "rpm"), 1e-4, 1e-3);
            close(&format!("{tag} speed"), sim.car.speed_ms, f(js, "speedMs"), 1e-4, 1e-4);
            close(&format!("{tag} ect"), sim.eng.ect, f(js, "ectC"), 1e-6, 1e-6);
            close(&format!("{tag} oil"), sim.eng.oil_c, f(js, "oilC"), 1e-6, 1e-6);
            close(&format!("{tag} oil pressure"), sim.eng.oil_kpa, f(js, "oilKpa"), 1e-5, 1e-4);
            close(&format!("{tag} friction"), sim.eng.friction_nm, f(js, "frictionNm"), 1e-5, 1e-5);
            close(&format!("{tag} bearing load"), sim.internals.load[3], f(js, "bearingLoad"), 1e-5, 1e-6);
            close(&format!("{tag} piston"), sim.internals.piston_c, f(js, "pistonC"), 1e-6, 1e-6);
            k += 1;
        }
    }
    assert_eq!(k, rows.len());
}

#[test]
fn render_state_has_the_web_fields() {
    let mut sim = Sim::new(placeholder_tune());
    sim.step(Input { throttle: 0.5, ..Default::default() }, 0.05);
    let s = honda_sim_core::simstate::render_state(&sim);
    for k in ["rpm", "crankDeg", "cams", "combustion", "flow", "induction", "parts", "geometry", "bottomEnd", "cooling"] {
        assert!(!s[k].is_null(), "{k}");
    }
    assert_eq!(s["cams"]["card"]["intakeHigh"]["liftMm"], 11.7);
}

#[test]
fn turbo_lines_and_heat_soak_match() {
    let mut t = turbo_setup(&placeholder_tune());
    t.materials.insert("piston".into(), "piston2618".into());
    t.materials.insert("rod".into(), "rod4340".into());
    let mut sim = Sim::new(t);
    sim.shift(2);
    let rows = data()["turboSoak"].as_array().unwrap().clone();
    let mut k = 0;
    for i in 0..6000 {
        if i == 1500 {
            sim.shift(0);
            sim.running = false;
        }
        sim.step(Input { throttle: if i < 1500 { 1.0 } else { 0.0 }, brake: 0.0, clutch: None }, 0.01);
        if i % 250 == 249 {
            let js = &rows[k];
            let th = &sim.eng.thermal;
            let tag = format!("turbo at {:.2}", sim.t);
            close(&format!("{tag} rpm"), sim.eng.rpm, f(js, "rpm"), 1e-4, 1e-3);
            close(&format!("{tag} housing"), th.turbo_housing_c, f(js, "housingC"), 1e-6, 1e-6);
            close(&format!("{tag} bearing"), th.turbo_bearing_c, f(js, "bearingC"), 1e-6, 1e-6);
            close(&format!("{tag} oil flow"), th.turbo_oil_lpm, f(js, "oilLpm"), 1e-6, 1e-6);
            close(&format!("{tag} oil"), th.oil_c, f(js, "oilC"), 1e-6, 1e-6);
            close(&format!("{tag} ect"), th.coolant_c, f(js, "ectC"), 1e-6, 1e-6);
            k += 1;
        }
    }
    assert_eq!(k, rows.len());
}

#[test]
fn rotating_assembly_oil_matches() {
    let g = Cooling::default();
    for case in data()["lube"].as_array().unwrap() {
        let got = serde_json::to_value(honda_sim_core::thermal::rotating_oil(f(case, "rpm"), f(case, "oilKpa"), f(case, "oilC"), &g, f(case, "loadRatio"))).unwrap();
        let js = &case["out"];
        for k in ["feedLpm", "rodNeedKpa", "supply", "splashLpm"] {
            close(k, f(&got, k), f(js, k), 1e-9, 1e-9);
        }
        for (part, keys) in [("mains", &["feedLpm", "filmC", "film"][..]), ("rods", &["feedLpm", "filmC", "film", "fromMain"][..]), ("jets", &["flowLpm"][..])] {
            for (a, b) in got[part].as_array().unwrap().iter().zip(js[part].as_array().unwrap()) {
                for k in keys {
                    close(&format!("{part} {k}"), f(a, k), f(b, k), 1e-9, 1e-9);
                }
            }
        }
    }
}

#[test]
fn k_series_blocks_heads_and_pumps_match() {
    for case in data()["engines"].as_array().unwrap() {
        let eng: honda_sim_core::tune::EngineSel = serde_json::from_value(case["engine"].clone()).unwrap();
        let parts: Parts = serde_json::from_value(case["parts"].clone()).unwrap();
        let t = Tune { engine: eng, parts, vtec_rpm: 5500.0, ..placeholder_tune() };
        let b = bottom_end(&t);
        let tag = format!("{} {}", case["engine"], case["parts"]);
        assert_eq!(b.head.id, case["head"].as_str().unwrap(), "{tag}");
        assert_eq!(b.pump.id, case["pump"].as_str().unwrap(), "{tag}");
        assert_eq!(b.head_fit.works, case["headWorks"].as_str().unwrap(), "{tag}");
        assert_eq!(b.pump_fit.works, case["pumpWorks"].as_str().unwrap(), "{tag}");
        assert_eq!(b.requires_sleeves, case["requiresSleeves"].as_bool().unwrap(), "{tag}");
        assert_eq!(b.issues.len() as f64, f(case, "issues"), "{tag}: {:?}", b.issues);
        for (k, v) in [("boreMm", b.bore_mm), ("strokeMm", b.stroke_mm), ("rodMm", b.rod_mm), ("compression", b.compression),
                       ("displacementL", b.displacement_l), ("deckClearanceMm", b.deck_clearance_mm)] {
            close(&format!("{k} {tag}"), v, f(case, k), 1e-9, 1e-9);
        }
        for run in case["runs"].as_array().unwrap() {
            let rpm = f(run, "rpm");
            let mut e = Engine::new(t.clone());
            let mut s = 0.0;
            while s < 1.0 {
                e.rpm = rpm;
                e.step(1.0, 0.001);
                s += 0.001;
            }
            close(&format!("torque {tag} {rpm}"), e.torque_nm, f(run, "torqueNm"), 1e-6, 1e-6);
            close(&format!("friction {tag} {rpm}"), e.friction_nm, f(run, "frictionNm"), 1e-6, 1e-6);
            close(&format!("oil {tag} {rpm}"), e.oil_kpa, f(run, "oilKpa"), 1e-6, 1e-6);
            close(&format!("vtec {tag} {rpm}"), e.cam_f, f(run, "camF"), 1e-6, 1e-6);
        }
    }
}
