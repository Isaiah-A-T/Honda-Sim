//! Gamepads on phones. Same mapping, "Learn" flow and saved controls.json as the desktop's wheel support
//! (windows/src/controls.rs), so the desktop's Wheel tab works unchanged; only where the input comes from
//! differs. A Bluetooth or USB gamepad's buttons and triggers reach the app as key events with the controller's
//! own button codes (Linux input codes on Android: 304 A, 310/311 bumpers, 312/313 triggers, 315 Start), which
//! the window passes to `key_event`. Triggers act as pedals that are either fully on or off, and ramp like a foot.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;

pub const AXES: [&str; 3] = ["throttle", "brake", "clutch"];
pub const BUTTONS: [&str; 10] = ["shiftUp", "shiftDown", "gear1", "gear2", "gear3", "gear4", "gear5", "gear6", "reverse", "start"];

/// Marks a button used as a pedal, the same way the desktop marks analog buttons.
const AS_AXIS: u32 = 0x8000_0000;
const PAD: &str = "Gamepad";

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

/// A typical gamepad: right trigger throttle, left trigger brake, bumpers shift, Start starts the engine.
pub fn default_mapping() -> Mapping {
    let mut m = Mapping::default();
    for (name, code) in [("throttle", 313), ("brake", 312)] {
        m.axes.insert(name.into(), AxisMap { pad: PAD.into(), code: AS_AXIS | code, rest: 0.0, full: 1.0 });
    }
    for (name, code) in [("shiftUp", 311), ("shiftDown", 310), ("start", 315)] {
        m.buttons.insert(name.into(), ButtonMap { pad: PAD.into(), code });
    }
    m
}

pub enum Learning {
    #[allow(dead_code)]
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

#[derive(Default)]
struct Held {
    down: HashSet<u32>,
    seen: bool,
}

static HELD: Mutex<Option<Held>> = Mutex::new(None);

/// A gamepad button went down or up (called by the window, any thread).
pub fn key_event(code: u32, pressed: bool) {
    let mut g = HELD.lock().unwrap();
    let h = g.get_or_insert_with(Held::default);
    h.seen = true;
    if pressed { h.down.insert(code); } else { h.down.remove(&code); }
}

pub struct Pads {
    prev: HashSet<(String, u32)>,
    pub error: Option<String>,
}

impl Pads {
    pub fn new() -> Pads {
        Pads { prev: HashSet::new(), error: None }
    }

    /// Pedal values for mapped buttons, mapped button presses as events, and the connected controllers.
    /// Finishes a pending "Learn" when a button is pressed.
    pub fn poll(&mut self, mapping: &mut Mapping, learning: &mut Option<Learning>, learned: &mut Option<String>) -> (PadInput, Vec<String>, Vec<String>) {
        let (pressed, seen): (HashSet<(String, u32)>, bool) = {
            let g = HELD.lock().unwrap();
            match g.as_ref() {
                Some(h) => (h.down.iter().map(|c| (PAD.to_string(), *c)).collect(), h.seen),
                None => (HashSet::new(), false),
            }
        };
        let names = if seen { vec!["Gamepad (buttons and triggers)".to_string()] } else { vec![] };

        let mut done = false;
        if let Some(k) = pressed.iter().find(|k| !self.prev.contains(*k)) {
            match learning {
                Some(Learning::Axis { name, .. }) => {
                    mapping.axes.insert(name.clone(), AxisMap { pad: k.0.clone(), code: AS_AXIS | k.1, rest: 0.0, full: 1.0 });
                    *learned = Some(format!("{name}: button {} on {}", k.1, k.0));
                    done = true;
                }
                Some(Learning::Button { name }) => {
                    mapping.buttons.insert(name.clone(), ButtonMap { pad: k.0.clone(), code: k.1 });
                    *learned = Some(format!("{name}: button {} on {}", k.1, k.0));
                    done = true;
                }
                None => {}
            }
        }
        if done {
            *learning = None;
        }

        let read = |n: &str| mapping.axes.get(n).map(|m| if pressed.contains(&(m.pad.clone(), m.code & !AS_AXIS)) { 1.0 } else { 0.0 });
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
