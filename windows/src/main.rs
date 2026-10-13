//! Honda Sim as one Windows program: the K20Z3 sim (honda-sim-core) runs on its own thread, reading the
//! wheel and pedals; the window is the 3D engine view (honda-sim-viewer) with the sim's dash along the top
//! and its tune, parts and internals panels down the side.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod controls;
mod gui;
mod store;
mod viewer_state;

use controls::{Learning, Mapping, Pads};
use honda_sim_core::{Input, Sim, Tune};
use honda_sim_viewer::{run, Link, Options};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Keyboard state the window passes to the sim thread.
#[derive(Default, Clone, Copy)]
pub struct Keys {
    pub throttle: bool,
    pub fast: bool,
    pub brake: bool,
    pub clutch: bool,
}

pub struct Shared {
    pub sim: Sim,
    /// The tune being edited; `apply_tune` hands it to the sim.
    pub tune: Tune,
    pub mapping: Mapping,
    pub learning: Option<Learning>,
    pub learned: Option<String>,
    pub pads: Vec<String>,
    pub pad_error: Option<String>,
    pub keys: Keys,
    pub events: Vec<String>,
    /// Pedals as last applied (0–1) and whether the clutch is automatic.
    pub throttle: f64,
    pub brake: f64,
    pub clutch: Option<f64>,
    pub paused: bool,
    pub message: String,
    pub run_timer: RunTimer,
}

/// 0–60 mph and quarter-mile times from a standing start.
#[derive(Default, Clone)]
pub struct RunTimer {
    pub armed: bool,
    pub start_t: Option<f64>,
    pub start_x: f64,
    pub zero_sixty: Option<f64>,
    pub quarter: Option<(f64, f64)>,
}

impl Shared {
    pub fn apply_tune(&mut self) {
        self.sim.set_tune(self.tune.clone());
        let _ = store::save_json(&store::data_dir().join("last-tune.json"), &self.tune);
    }
    pub fn save_mapping(&self) {
        let _ = store::save_json(&store::data_dir().join("controls.json"), &self.mapping);
    }
}

fn handle_event(sh: &mut Shared, ev: &str) {
    let sim = &mut sh.sim;
    match ev {
        "shiftUp" => sim.shift_up(),
        "shiftDown" => sim.shift_down(),
        "neutral" => sim.shift(0),
        "reverse" => sim.shift(-1),
        "start" => sim.start_engine(),
        "coldEngine" => sim.cold_engine(),
        "warmEngine" => {
            let e = &mut sim.eng;
            e.thermal = honda_sim_core::thermal::Thermal::new(e.tune.ambient_c.unwrap_or(25.0), true, e.tune.cooling());
            e.ect = e.thermal.coolant_c;
            e.oil_c = e.thermal.oil_c;
        }
        g if g.starts_with("gear") => {
            if let Ok(n) = g[4..].parse::<i32>() {
                sim.shift(n)
            }
        }
        _ => {}
    }
}

fn sim_thread(shared: Arc<Mutex<Shared>>, link: Link) {
    let mut pads = Pads::new();
    shared.lock().unwrap().pad_error = pads.error.clone();
    let mut kb = (0.0f64, 0.0f64, 0.0f64);
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(2));
        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(0.05);
        last = now;
        let state = {
            let mut guard = shared.lock().unwrap();
            let sh = &mut *guard;
            let (pad, mut events, names) = pads.poll(&mut sh.mapping, &mut sh.learning, &mut sh.learned);
            if sh.learned.is_some() && sh.learning.is_none() {
                sh.save_mapping();
            }
            sh.pads = names;
            events.append(&mut sh.events);
            for ev in &events {
                handle_event(sh, ev);
            }
            // Keyboard pedals ramp like a foot would.
            let k = sh.keys;
            let ramp = |v: f64, on: bool, up: f64, down: f64| if on { (v + up * dt).min(1.0) } else { (v - down * dt).max(0.0) };
            kb.0 = ramp(kb.0, k.throttle, if k.fast { 6.0 } else { 2.0 }, 5.0);
            kb.1 = ramp(kb.1, k.brake, 4.0, 6.0);
            kb.2 = ramp(kb.2, k.clutch, 8.0, 4.0);
            sh.throttle = pad.throttle.unwrap_or(0.0).max(kb.0);
            sh.brake = pad.brake.unwrap_or(0.0).max(kb.1);
            // With a clutch pedal mapped (or the clutch key held), the driver works the clutch; otherwise it's automatic.
            sh.clutch = match pad.clutch {
                Some(c) => Some(c.max(kb.2)),
                None if kb.2 > 0.0 => Some(kb.2),
                None => None,
            };
            if !sh.paused {
                let input = Input { throttle: sh.throttle, brake: sh.brake, clutch: sh.clutch };
                sh.sim.step(input, dt);
                timer_step(sh);
            }
            viewer_state::viewer_state(&sh.sim)
        };
        link.send(state);
    }
}

fn timer_step(sh: &mut Shared) {
    let (t, v, x) = (sh.sim.t, sh.sim.car.speed_ms, sh.sim.car.distance_m);
    let r = &mut sh.run_timer;
    if v.abs() < 0.1 {
        r.armed = true;
        r.start_t = None;
        return;
    }
    if r.armed && r.start_t.is_none() {
        *r = RunTimer { armed: false, start_t: Some(t), start_x: x, ..Default::default() };
    }
    if let Some(t0) = r.start_t {
        if r.zero_sixty.is_none() && v >= 26.8224 {
            r.zero_sixty = Some(t - t0);
        }
        if r.quarter.is_none() && x - r.start_x >= 402.336 {
            r.quarter = Some((t - t0, v * 2.23694));
        }
    }
}

fn main() {
    let tune: Tune = store::load_json(&store::data_dir().join("last-tune.json")).unwrap_or_default();
    let mapping: Mapping = store::load_json(&store::data_dir().join("controls.json")).unwrap_or_default();
    let shared = Arc::new(Mutex::new(Shared {
        sim: Sim::new(tune.clone()),
        tune,
        mapping,
        learning: None,
        learned: None,
        pads: vec![],
        pad_error: None,
        keys: Keys::default(),
        events: vec![],
        throttle: 0.0,
        brake: 0.0,
        clutch: None,
        paused: false,
        message: String::new(),
        run_timer: RunTimer::default(),
    }));
    let link = Link::in_process();
    {
        let (shared, link) = (shared.clone(), link.clone());
        std::thread::spawn(move || sim_thread(shared, link));
    }
    let mut ui = gui::Gui::new(shared);
    run(Options {
        link,
        title: format!("Honda Sim {} · K20Z3", env!("CARGO_PKG_VERSION")),
        host_panels: Some(Box::new(move |root, _state| ui.draw(root))),
        ..Default::default()
    });
}
