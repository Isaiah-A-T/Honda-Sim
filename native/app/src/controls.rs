//! Wheel and pedals through gilrs. Every brand numbers its axes and buttons differently and many pedals
//! rest at +1, so nothing is hard-coded: "Learn" records which axis moves and its rest and pressed values,
//! or which button gets pressed. Wheel and pedals can be separate devices; each mapping remembers its device.

use gilrs::Gilrs;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const AXES: [&str; 3] = ["throttle", "brake", "clutch"];
pub const BUTTONS: [&str; 10] = ["shiftUp", "shiftDown", "gear1", "gear2", "gear3", "gear4", "gear5", "gear6", "reverse", "start"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AxisMap {
    pub pad: String,
    pub code: u32,
    pub rest: f32,
    pub full: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ButtonMap {
    pub pad: String,
    pub code: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Mapping {
    pub axes: BTreeMap<String, AxisMap>,
    pub buttons: BTreeMap<String, ButtonMap>,
}

pub enum Learning {
    Axis { name: String, baseline: HashMap<(String, u32), f32>, extreme: HashMap<(String, u32), f32> },
    Button { name: String },
}

impl Learning {
    pub fn name(&self) -> &str {
        match self {
            Learning::Axis { name, .. } | Learning::Button { name } => name,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct PadInput {
    pub throttle: Option<f64>,
    pub brake: Option<f64>,
    pub clutch: Option<f64>,
}

pub struct Pads {
    gilrs: Option<Gilrs>,
    prev: HashSet<(String, u32)>,
    pub error: Option<String>,
}

fn norm(raw: f32, rest: f32, full: f32) -> f64 {
    if full == rest { 0.0 } else { (((raw - rest) / (full - rest)) as f64).clamp(0.0, 1.0) }
}

impl Pads {
    pub fn new() -> Pads {
        match Gilrs::new() {
            Ok(g) => Pads { gilrs: Some(g), prev: HashSet::new(), error: None },
            Err(e) => Pads { gilrs: None, prev: HashSet::new(), error: Some(format!("Game controller input unavailable: {e}")) },
        }
    }

    /// Read every connected device. Returns pedal values for mapped axes, mapped button presses as
    /// events, and the connected device names. Finishes a pending "Learn" when it sees the input.
    pub fn poll(&mut self, mapping: &mut Mapping, learning: &mut Option<Learning>, learned: &mut Option<String>) -> (PadInput, Vec<String>, Vec<String>) {
        let Some(g) = self.gilrs.as_mut() else { return (PadInput::default(), vec![], vec![]) };
        while g.next_event().is_some() {}
        let mut axes: HashMap<(String, u32), f32> = HashMap::new();
        let mut pressed: HashSet<(String, u32)> = HashSet::new();
        let mut names = Vec::new();
        for (_, pad) in g.gamepads() {
            let name = pad.name().to_string();
            names.push(name.clone());
            for (code, data) in pad.state().axes() {
                axes.insert((name.clone(), code.into_u32()), data.value());
            }
            for (code, data) in pad.state().buttons() {
                if data.is_pressed() {
                    pressed.insert((name.clone(), code.into_u32()));
                }
                // Some pedals report as analog buttons; treat them as axes 0..1 too.
                axes.entry((name.clone(), 0x8000_0000 | code.into_u32())).or_insert(data.value());
            }
        }

        // Learning.
        let mut done = false;
        match learning {
            Some(Learning::Axis { name, baseline, extreme }) => {
                if baseline.is_empty() {
                    *baseline = axes.clone();
                }
                // Remember how far each axis has gone from where it started.
                for (k, v) in &axes {
                    let rest = *baseline.entry(k.clone()).or_insert(*v);
                    let e = extreme.entry(k.clone()).or_insert(rest);
                    if (v - rest).abs() > (*e - rest).abs() {
                        *e = *v;
                    }
                }
                // The axis that moved most (at least a third of its range) is the one; done when it's released.
                let best = extreme.iter().map(|(k, e)| (k.clone(), baseline[k], *e)).filter(|(_, r, e)| (e - r).abs() > 0.35)
                    .max_by(|a, b| (a.2 - a.1).abs().partial_cmp(&(b.2 - b.1).abs()).unwrap());
                if let Some((k, rest, full)) = best {
                    let now = axes.get(&k).copied().unwrap_or(rest);
                    if (now - rest).abs() < 0.15 * (full - rest).abs() {
                        mapping.axes.insert(name.clone(), AxisMap { pad: k.0.clone(), code: k.1, rest, full });
                        *learned = Some(format!("{name}: axis {} on {}", k.1 & 0x7fff_ffff, k.0));
                        done = true;
                    }
                }
            }
            Some(Learning::Button { name }) => {
                if let Some(k) = pressed.iter().find(|k| !self.prev.contains(*k)) {
                    mapping.buttons.insert(name.clone(), ButtonMap { pad: k.0.clone(), code: k.1 });
                    *learned = Some(format!("{name}: button {} on {}", k.1, k.0));
                    done = true;
                }
            }
            None => {}
        }
        if done {
            *learning = None;
        }

        let read = |n: &str| mapping.axes.get(n).and_then(|m| axes.get(&(m.pad.clone(), m.code)).map(|v| norm(*v, m.rest, m.full)));
        let input = PadInput { throttle: read("throttle"), brake: read("brake"), clutch: read("clutch") };
        let mut events = Vec::new();
        if learning.is_none() && !done {
            for (ev, m) in &mapping.buttons {
                let k = (m.pad.clone(), m.code);
                if pressed.contains(&k) && !self.prev.contains(&k) {
                    events.push(ev.clone());
                }
            }
        }
        self.prev = pressed;
        (input, events, names)
    }
}
