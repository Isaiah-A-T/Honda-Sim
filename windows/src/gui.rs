//! The sim's panels around the 3D view: the dash along the top and the tune, settings, parts,
//! materials, internals, wheel and datalog panels down the left side.

use crate::controls::{Learning, AXES, BUTTONS};
use crate::store;
use crate::Shared;
use honda_sim_core::engine::hp;
use honda_sim_core::internals::{part_material_id, part_status, PARTS};
use honda_sim_core::k20z3::{placeholder_tune, turbo_6262, turbo_gt35, turbo_setup};
use honda_sim_core::materials::{material, materials_for};
use honda_sim_core::engines::{engines, head_fit, pump_fit};
use honda_sim_core::fuels::{fuel_of, fuels, pump_lph_of, PUMP_LPH_MAX, PUMP_LPH_MIN, PUMP_LPH_STEP, RAIL_KPA, STOCK_PUMP_LPH};
use honda_sim_core::parts::{catalog, crank_problem, cranks_for, piston_problem, pistons_for, rod_problem};
use honda_sim_core::sim::log_to_csv;
use honda_sim_core::thermal::{boiling_c, viscosity_cst, Cooling};
use honda_sim_core::tables::{active_cell, guess_fuel_units, parse_table, Table};
use honda_sim_core::Tune;
use honda_sim_viewer::egui::{self, Color32, RichText};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Drive,
    Tune,
    Settings,
    Parts,
    Internals,
    Cooling,
    Wheel,
    Log,
}

const TABS: [(Tab, &str); 8] = [
    (Tab::Drive, "Drive"), (Tab::Tune, "Tables"), (Tab::Settings, "Settings"), (Tab::Parts, "Parts"),
    (Tab::Internals, "Internals"), (Tab::Cooling, "Cooling"), (Tab::Wheel, "Wheel"), (Tab::Log, "Log"),
];

// Tables the tune can hold: key, label.
const TABLES: [(&str, &str); 8] = [
    ("fuelLow", "Fuel, low cam"), ("fuelHigh", "Fuel, high cam"), ("ignLow", "Ignition, low cam"), ("ignHigh", "Ignition, high cam"),
    ("vtcLow", "Cam angle, low cam"), ("vtcHigh", "Cam angle, high cam"), ("boostTarget", "Boost target"), ("wgDuty", "Wastegate duty"),
];

fn table_mut<'a>(t: &'a mut Tune, key: &str) -> Option<&'a mut Table> {
    match key {
        "fuelLow" => Some(&mut t.fuel_low),
        "fuelHigh" => t.fuel_high.as_mut(),
        "ignLow" => Some(&mut t.ign_low),
        "ignHigh" => t.ign_high.as_mut(),
        "vtcLow" => t.vtc_low.as_mut(),
        "vtcHigh" => t.vtc_high.as_mut(),
        "boostTarget" => t.boost_target.as_mut(),
        "wgDuty" => t.wg_duty.as_mut(),
        _ => None,
    }
}

fn set_table(t: &mut Tune, key: &str, v: Table) {
    match key {
        "fuelLow" => t.fuel_low = v,
        "fuelHigh" => t.fuel_high = Some(v),
        "ignLow" => t.ign_low = v,
        "ignHigh" => t.ign_high = Some(v),
        "vtcLow" => t.vtc_low = Some(v),
        "vtcHigh" => t.vtc_high = Some(v),
        "boostTarget" => t.boost_target = Some(v),
        "wgDuty" => t.wg_duty = Some(v),
        _ => {}
    }
}

/// Engine: which block, head and oil pump, and whether they go together (web/src/engines.js).
fn engine_section(ui: &mut egui::Ui, sh: &mut Shared, b: &honda_sim_core::parts::BottomEnd) -> bool {
    let data = engines();
    let mut c = false;
    let grey = Color32::from_gray(160);
    let warn = Color32::from_rgb(0xf2, 0xa9, 0x3b);
    ui.heading("Engine");
    ui.label(RichText::new("Mix any K-series block, head and oil pump. Combinations no build guide documents are greyed out with the reason.").small());

    ui.label("Block (with its stock crank, rods and pistons)");
    egui::ComboBox::from_id_salt("block").width(340.0).selected_text(&b.block.name).show_ui(ui, |ui| {
        for k in &data.blocks {
            if ui.selectable_label(k.id == b.block.id, &k.name).clicked() {
                sh.tune.engine.block = if k.id == "K20Z3" { None } else { Some(k.id.clone()) };
                sh.tune.engine.head = None;
                sh.tune.engine.oil_pump = None;
                if sh.tune.parts.crank.as_deref() == Some("honda-k24-99-hybrid") { sh.tune.parts.crank = None; }
                c = true;
            }
        }
    });
    let r = &b.block.rating;
    ui.label(RichText::new(format!(
        "{} × {} mm, {} mm rods, {:.1}:1 stock. Rated {:.0} hp at {:.0} and {:.0} lb-ft at {:.0} rpm; redline {:.0}.",
        b.block.bore_mm, b.block.stroke_mm, b.block.rod_mm, b.block.compression, r.hp, r.hp_rpm, r.torque_lbft, r.torque_rpm, r.redline_rpm
    )).small().color(grey));

    ui.add_space(4.0);
    ui.label("Head");
    egui::ComboBox::from_id_salt("head").width(340.0).selected_text(&b.head.name).show_ui(ui, |ui| {
        for h in &data.heads {
            let fit = head_fit(h, b.block);
            let label = match fit.works.as_str() {
                "no" => format!("{} (no documented build on a {})", h.name, b.block.family),
                "mods" => format!("{} (needs swap parts)", h.name),
                _ => h.name.clone(),
            };
            if ui.add_enabled(fit.works != "no", egui::Button::selectable(h.id == b.head.id, label)).clicked() {
                sh.tune.engine.head = if h.id == b.block.head { None } else { Some(h.id.clone()) };
                c = true;
            }
        }
    });
    let vtec = match b.head.vtec.as_str() {
        "economy" => "Economy i-VTEC: intake cam only; below the switch point one intake valve barely opens",
        "intake" => "VTEC on the intake cam only",
        _ => "VTEC on both cams",
    };
    ui.label(RichText::new(format!("{vtec}. Valves {} / {} mm. Stock VTEC point {:.0} rpm.", b.head.in_valve_mm, b.head.ex_valve_mm, b.head.vtec_rpm)).small().color(grey));
    ui.horizontal(|ui| {
        if (sh.tune.vtec_rpm - b.head.vtec_rpm).abs() > 1.0 && ui.small_button(format!("Set VTEC to {:.0} rpm", b.head.vtec_rpm)).clicked() {
            sh.tune.vtec_rpm = b.head.vtec_rpm;
            c = true;
        }
        let limit = r.rev_limit_rpm.unwrap_or(r.redline_rpm);
        if (sh.tune.rev_limit - limit).abs() > 1.0 && ui.small_button(format!("Set rev limit to {limit:.0} rpm")).clicked() {
            sh.tune.rev_limit = limit;
            c = true;
        }
    });
    if b.head_fit.works == "mods" {
        ui.label(RichText::new(format!("Swap parts: {}.", b.head_fit.needs.join("; "))).color(warn));
    }
    if !b.head_fit.note.is_empty() && b.head.id != b.block.head {
        ui.label(RichText::new(&b.head_fit.note).small().color(grey));
    }

    ui.add_space(4.0);
    ui.label("Oil pump");
    egui::ComboBox::from_id_salt("oilpump").width(340.0).selected_text(&b.pump.name).show_ui(ui, |ui| {
        for p in &data.oil_pumps {
            let fit = pump_fit(p, b.block);
            let label = match fit.works.as_str() {
                "no" => format!("{} (not listed for the {})", p.name, b.block.id),
                "mods" => format!("{} (needs install parts)", p.name),
                _ => p.name.clone(),
            };
            if ui.add_enabled(fit.works != "no", egui::Button::selectable(p.id == b.pump.id, label)).clicked() {
                sh.tune.engine.oil_pump = if p.id == b.block.oil_pump { None } else { Some(p.id.clone()) };
                c = true;
            }
        }
    });
    let p = b.pump;
    let flow = if p.flow_scale != 1.0 { format!("{:+.0}% flow", (p.flow_scale - 1.0) * 100.0) } else { "Stock flow".to_string() };
    ui.label(RichText::new(format!(
        "{}. {flow}, holds pressure to about {:.0} rpm.",
        if p.balance_shafts { "Drives the balance shafts" } else { "No balance shafts (frees about 7 hp at 7,800 rpm, vendor claims)" },
        p.max_rpm
    )).small().color(grey));
    if b.pump_fit.works == "mods" {
        ui.label(RichText::new(format!("Install parts: {}.", b.pump_fit.needs.join("; "))).color(warn));
    }
    let mut est: Vec<&String> = b.head.estimated.values().collect();
    est.extend(p.estimated.values());
    for v in est {
        ui.label(RichText::new(format!("Estimated: {v}")).small().color(Color32::from_gray(150)));
    }
    let mut srcs: Vec<(String, String)> = vec![];
    srcs.extend(b.block.sources.iter().take(2).map(|u| (b.block.id.clone(), u.clone())));
    srcs.extend(b.head.sources.iter().take(1).map(|u| (format!("{} head", b.head.id), u.clone())));
    srcs.extend(p.sources.iter().take(1).map(|u| ("pump".to_string(), u.clone())));
    srcs.extend(b.head_fit.sources.iter().take(1).map(|u| ("swap".to_string(), u.clone())));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Sources:").small());
        for (i, (what, u)) in srcs.iter().enumerate() {
            ui.hyperlink_to(RichText::new(format!("{what} {}", i + 1)).small(), u);
        }
    });
    c
}

/// One part's alloy picker with its published properties, for the Parts tab.
fn material_row(ui: &mut egui::Ui, tune: &mut Tune, b: &honda_sim_core::parts::BottomEnd, key: &str) -> bool {
    let p = PARTS.iter().find(|p| p.key == key).expect("part key");
    let id = part_material_id(tune, b, p);
    let m = material(id).unwrap();
    let mut c = false;
    ui.horizontal(|ui| {
        ui.add_sized([56.0, 18.0], egui::Label::new(RichText::new("Material").color(Color32::from_gray(160)).small()));
        egui::ComboBox::from_id_salt(("material", p.key)).width(300.0).selected_text(&m.name).show_ui(ui, |ui| {
            for opt in materials_for(p.category) {
                if ui.selectable_label(opt == id, &material(opt).unwrap().name).clicked() {
                    tune.materials.insert(p.key.to_string(), opt.to_string());
                    c = true;
                }
            }
        });
    });
    let est = |f: &str| if m.estimated.contains_key(f) { " est." } else { "" };
    let mut line = vec![];
    if let Some(v) = m.yield_mpa { line.push(format!("yield {v:.0}{} MPa", est("yieldMPa"))); }
    if let Some(v) = m.uts_mpa { line.push(format!("tensile {v:.0}{} MPa", est("utsMPa"))); }
    if let Some(v) = m.fatigue_mpa { line.push(format!("fatigue {v:.0}{} MPa", est("fatigueMPa"))); }
    if let Some(v) = m.density_kg_m3 { line.push(format!("{v:.0} kg/m³")); }
    if let Some(v) = m.elongation_pct { line.push(format!("elong. {v}%")); }
    if let Some(v) = m.max_service_temp_c { line.push(format!("max {v:.0} °C")); }
    let tip: Vec<String> = m.estimated.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    ui.horizontal_wrapped(|ui| {
        let r = ui.label(RichText::new(line.join(", ")).small().color(Color32::from_gray(150)));
        if !tip.is_empty() {
            r.on_hover_text(tip.join("\n"));
        }
        if let Some(u) = m.sources.first() {
            ui.hyperlink_to(RichText::new("source").small(), u);
        }
    });
    c
}

fn load_color(v: f64) -> Color32 {
    if v >= 1.0 { Color32::from_rgb(0xff, 0x5a, 0x4e) }
    else if v >= 0.8 { Color32::from_rgb(0xf2, 0xa9, 0x3b) }
    else if v >= 0.5 { Color32::from_rgb(0xe8, 0xc6, 0x3a) }
    else { Color32::from_rgb(0x3f, 0xcf, 0x7f) }
}

fn heat(v: f64, lo: f64, hi: f64) -> Color32 {
    let x = if hi > lo { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
    Color32::from_rgb((40.0 + 150.0 * x) as u8, (70.0 + 60.0 * (1.0 - (2.0 * x - 1.0).abs())) as u8, (170.0 - 130.0 * x) as u8)
}

/// A dash reading in a fixed-width slot, so nothing moves when values change.
fn stat(ui: &mut egui::Ui, label: &str, value: String, width: f32) {
    ui.allocate_ui_with_layout(egui::vec2(width, 36.0), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_width(width);
        ui.set_max_width(width);
        ui.label(RichText::new(label).small().color(Color32::from_gray(150)));
        ui.add(egui::Label::new(RichText::new(value).monospace().strong()).truncate());
    });
}

/// A warning lamp that always takes its place on the dash: lit when on, dim when off.
fn lamp(ui: &mut egui::Ui, text: &str, on: bool, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 18.0), egui::Sense::hover());
    let (fill, fg) = if on { (color, Color32::WHITE) } else { (Color32::from_gray(36), Color32::from_gray(95)) };
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), fg);
}

/// Fuel, fuel pump and the (fixed) rail pressure, as grid rows.
fn fuel_fields(ui: &mut egui::Ui, t: &mut Tune) -> bool {
    let mut c = false;
    let cur = fuel_of(t);
    ui.label("Fuel");
    egui::ComboBox::from_id_salt("fuel").width(260.0).selected_text(&cur.name).show_ui(ui, |ui| {
        for f in fuels() {
            if ui.selectable_label(cur.id == f.id, &f.name).clicked() {
                t.fuel = Some(f.id.to_string());
                t.octane = f.octane;
                c = true;
            }
        }
    });
    ui.end_row();
    ui.label("Fuel pump");
    let lph = pump_lph_of(t);
    let name = |v: f64| if v == STOCK_PUMP_LPH { format!("{v:.0} l/h (stock)") } else { format!("{v:.0} l/h") };
    egui::ComboBox::from_id_salt("fuel-pump").width(260.0).selected_text(name(lph)).show_ui(ui, |ui| {
        let mut v = PUMP_LPH_MIN;
        while v <= PUMP_LPH_MAX {
            if ui.selectable_label(v == lph, name(v)).clicked() {
                t.fuel_pump_lph = Some(v);
                c = true;
            }
            v += PUMP_LPH_STEP;
        }
    });
    ui.end_row();
    ui.label("Fuel pressure");
    ui.label(format!("{:.0} psi, fixed (returnless rail)", RAIL_KPA / 6.895));
    ui.end_row();
    c
}

fn num_field(ui: &mut egui::Ui, label: &str, v: &mut f64, speed: f64, range: std::ops::RangeInclusive<f64>, suffix: &str) -> bool {
    ui.label(label);
    let r = ui.add(egui::DragValue::new(v).speed(speed).range(range).suffix(suffix)).changed();
    ui.end_row();
    r
}

fn opt_field(ui: &mut egui::Ui, label: &str, v: &mut Option<f64>, default: f64, speed: f64, suffix: &str) -> bool {
    ui.label(label);
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut on = v.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *v = if on { Some(default) } else { None };
            changed = true;
        }
        if let Some(x) = v.as_mut() {
            changed |= ui.add(egui::DragValue::new(x).speed(speed).suffix(suffix)).changed();
        } else {
            ui.label(RichText::new("off").color(Color32::from_gray(140)));
        }
    });
    ui.end_row();
    changed
}

pub struct Gui {
    shared: Arc<Mutex<Shared>>,
    tab: Tab,
    table: &'static str,
    cell: Option<(usize, usize)>,
    paste: String,
    paste_target: &'static str,
    paste_msg: String,
    tune_name: String,
    tunes: Vec<(String, std::path::PathBuf)>,
}

impl Gui {
    pub fn new(shared: Arc<Mutex<Shared>>) -> Gui {
        let name = shared.lock().unwrap().tune.name.clone();
        Gui { shared, tab: Tab::Drive, table: "fuelLow", cell: None, paste: String::new(), paste_target: "fuelLow", paste_msg: String::new(), tune_name: name, tunes: store::list_tunes() }
    }

    pub fn draw(&mut self, root: &mut egui::Ui) {
        let shared = self.shared.clone();
        let mut guard = shared.lock().unwrap();
        let sh = &mut *guard;
        self.keyboard(root.ctx(), sh);
        egui::Panel::top("dash").show_inside(root, |ui| self.dash(ui, sh));
        egui::Panel::left("sim").default_size(440.0).resizable(true).show_inside(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (t, label) in TABS {
                    if ui.selectable_label(self.tab == t, label).clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let changed = match self.tab {
                    Tab::Drive => { self.drive(ui, sh); false }
                    Tab::Tune => self.tables(ui, sh),
                    Tab::Settings => self.settings(ui, sh),
                    Tab::Parts => self.parts(ui, sh),
                    Tab::Internals => { self.internals(ui, sh); false }
                    Tab::Cooling => self.cooling(ui, sh),
                    Tab::Wheel => { self.wheel(ui, sh); false }
                    Tab::Log => { self.log(ui, sh); false }
                };
                if changed {
                    sh.apply_tune();
                }
            });
        });
    }

    fn keyboard(&mut self, ctx: &egui::Context, sh: &mut Shared) {
        if ctx.egui_wants_keyboard_input() {
            sh.keys = Default::default();
            return;
        }
        use egui::Key;
        ctx.input(|i| {
            sh.keys.throttle = i.key_down(Key::W) || i.key_down(Key::ArrowUp);
            sh.keys.fast = i.modifiers.shift;
            sh.keys.brake = i.key_down(Key::S) || i.key_down(Key::ArrowDown);
            sh.keys.clutch = i.key_down(Key::C) || i.key_down(Key::Space);
            for (k, ev) in [(Key::E, "shiftUp"), (Key::Q, "shiftDown"), (Key::N, "neutral"), (Key::R, "start"), (Key::Num0, "reverse"),
                (Key::Num1, "gear1"), (Key::Num2, "gear2"), (Key::Num3, "gear3"), (Key::Num4, "gear4"), (Key::Num5, "gear5"), (Key::Num6, "gear6")] {
                if i.key_pressed(k) {
                    sh.events.push(ev.to_string());
                }
            }
        });
    }

    fn dash(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        let sim = &sh.sim;
        let e = &sim.eng;
        let tq = if sim.running { e.torque_nm } else { 0.0 };
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(120.0, 36.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(120.0);
                ui.label(RichText::new(format!("{:>5.0}", e.rpm.max(0.0))).size(26.0).monospace().strong());
                ui.label(RichText::new("rpm").color(Color32::from_gray(150)));
            });
            ui.separator();
            let gear = match sim.car.gear { 0 => "N".into(), g if g < 0 => "R".into(), g => g.to_string() };
            stat(ui, "Gear", gear, 34.0);
            stat(ui, "Speed", format!("{:>3.0} mph", sim.car.speed_ms * 2.23694), 64.0);
            stat(ui, "MAP", format!("{:>3.0} kPa", e.map), 64.0);
            stat(ui, "Boost", format!("{:>4.1} psi", ((e.map - 101.3) * 0.145).max(0.0)), 64.0);
            stat(ui, "AFR", if e.lambda > 5.0 { " cut".into() } else { format!("{:>4.1}", e.afr) }, 40.0);
            stat(ui, "Timing", format!("{:>5.1}°", e.ign), 52.0);
            stat(ui, "Knock ret.", format!("{:>4.1}°", e.knock_retard), 60.0);
            stat(ui, "VTC", format!("{:>3.0}°", e.vtc), 36.0);
            stat(ui, "Torque", format!("{:>4.0} Nm", tq + 0.0), 62.0);
            stat(ui, "Power", format!("{:>4.0} hp", hp(tq, e.rpm) + 0.0), 58.0);
            stat(ui, "IAT", format!("{:>3.0} °C", e.iat), 48.0);
            stat(ui, "Coolant", format!("{:>3.0} °C", e.ect), 54.0);
            stat(ui, "Oil", format!("{:>3.0} °C", e.oil_c), 48.0);
            stat(ui, "Oil press.", format!("{:>3.0} kPa", e.oil_kpa), 62.0);
            stat(ui, "EGT", format!("{:>4.0} °C", e.egt_c), 56.0);
        });
        ui.horizontal(|ui| {
            let turbo = e.tune.turbo.enabled && !e.tune.turbo.fixed;
            lamp(ui, "VTEC", e.high_cam, Color32::from_rgb(0xd0, 0x3a, 0x2e));
            lamp(ui, "KNOCK", e.knocking, Color32::from_rgb(0xc8, 0x6a, 0x10));
            lamp(ui, "REV LIM", e.rev_cut, Color32::from_rgb(0x80, 0x30, 0xa0));
            lamp(ui, "FANS", e.thermal.fan_on, Color32::from_rgb(0x20, 0x60, 0xa0));
            lamp(ui, "COLD", sim.running && e.ect < 60.0, Color32::from_rgb(0x30, 0x50, 0x90));
            lamp(ui, "HOT", e.thermal.boiling, Color32::from_rgb(0xb0, 0x10, 0x10));
            lamp(ui, "SOAK", turbo && e.thermal.turbo_bearing_c > e.thermal.g.turbo_coke_c, Color32::from_rgb(0xc8, 0x6a, 0x10));
            let status = if let Some(f) = sim.internals.failed { Some((format!("{} FAILED", f.to_uppercase()), Color32::from_rgb(0xb0, 0x10, 0x10))) }
                else if !sim.running { Some(("ENGINE OFF (R to start)".to_string(), Color32::from_gray(70))) } else { None };
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(egui::vec2(220.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(220.0);
                if let Some((t, c)) = status {
                    ui.label(RichText::new(format!(" {t} ")).strong().small().color(Color32::WHITE).background_color(c));
                }
            });
        });
    }

    fn drive(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        ui.heading("Driving");
        let bar = |ui: &mut egui::Ui, label: &str, v: f64| {
            ui.horizontal(|ui| {
                ui.add_sized([60.0, 16.0], egui::Label::new(label));
                ui.add(egui::ProgressBar::new(v as f32).desired_width(200.0).text(format!("{:.0}%", v * 100.0)));
            });
        };
        bar(ui, "Throttle", sh.throttle);
        bar(ui, "Brake", sh.brake);
        match sh.clutch {
            Some(c) => bar(ui, "Clutch", c),
            None => { ui.label("Clutch: automatic (map a clutch pedal, or hold C, to work it yourself)"); }
        }
        ui.horizontal(|ui| {
            if ui.button("Start engine").clicked() { sh.events.push("start".into()); }
            if ui.button("Neutral").clicked() { sh.events.push("neutral".into()); }
            if ui.button("1st").clicked() { sh.events.push("gear1".into()); }
            ui.checkbox(&mut sh.paused, "Pause");
        });
        ui.add_space(6.0);
        let r = &sh.run_timer;
        ui.label(format!("0–60 mph: {}", r.zero_sixty.map(|t| format!("{t:.2} s")).unwrap_or("–".into())));
        ui.label(format!("Quarter mile: {}", r.quarter.map(|(t, v)| format!("{t:.2} s @ {v:.1} mph")).unwrap_or("–".into())));
        ui.label(RichText::new("Times start when the car moves off from a stop.").small().color(Color32::from_gray(150)));
        ui.add_space(6.0);
        ui.label(RichText::new("Keyboard").strong());
        ui.label("W or Up arrow throttle (Shift for faster), S or Down arrow brake, C or Space clutch, E shift up, Q shift down, 1–6 gears, 0 reverse, N neutral, R start.");
        ui.add_space(6.0);
        let e = &sh.sim.eng;
        egui::Grid::new("drive-readouts").num_columns(2).show(ui, |ui| {
            for (k, v) in [
                ("Ignition table", format!("{:.1}° (best torque {:.1}°, knock limit {:.1}°)", e.ign_table, e.mbt, e.knock_limit)),
                ("Lambda", format!("{:.3} (target {})", e.lambda, if e.lambda_target.is_finite() { format!("{:.2}", e.lambda_target) } else { "–".into() })),
                ("Injectors", format!("{:.2} ms, {:.0}% duty", e.ipw, e.inj_duty)),
                ("Fuel pump", format!("{:.0}% of its flow, rail {:.0} psi", e.fuel_pump_pct, e.fuel_pressure_kpa / 6.895)),
                ("Volumetric eff.", format!("{:.1}%", e.ve * 100.0)),
                ("Air flow", format!("{:.0} g/s", e.air_gs)),
                ("VTEC pins", format!("{:?}", e.vtec_pins.map(|p| (p * 100.0).round() / 100.0))),
                ("Oil pressure", format!("{:.0} kPa", e.oil_kpa)),
                ("CA50", if e.ca50.is_finite() { format!("{:.1}° ATDC", e.ca50) } else { "–".into() }),
                ("Exhaust manifold", format!("{:.0} kPa", e.exh_kpa)),
                ("Pumping loss", format!("{:.0} kPa", e.pmep_kpa)),
                ("Residual gas", format!("{:.1}%", e.residual * 100.0)),
                ("Turbo", if e.tune.turbo.enabled { format!("{:.0}k rpm, wastegate {:.0}%", e.turbo_rad_s * 30.0 / std::f64::consts::PI / 1000.0, e.wg_open * 100.0) } else { "N/A engine".into() }),
            ] {
                ui.label(RichText::new(k).color(Color32::from_gray(160)));
                ui.label(v);
                ui.end_row();
            }
        });
    }

    fn tables(&mut self, ui: &mut egui::Ui, sh: &mut Shared) -> bool {
        let mut changed = false;
        ui.heading("Tune tables");
        ui.label(RichText::new("Everything a tune needs: the eight tables below plus these switch points. Click a table name, then click a cell to edit it, or paste a whole table from FlashPro.").small());
        egui::Grid::new("tune-points").num_columns(2).show(ui, |ui| {
            let t = &mut sh.tune;
            changed |= num_field(ui, "VTEC point", &mut t.vtec_rpm, 25.0, 2000.0..=9000.0, " rpm");
            changed |= num_field(ui, "VTEC minimum MAP", &mut t.vtec_min_map, 1.0, 0.0..=250.0, " kPa");
            changed |= num_field(ui, "Rev limit", &mut t.rev_limit, 25.0, 3000.0..=9500.0, " rpm");
            changed |= num_field(ui, "Rev limit hysteresis", &mut t.rev_limit_hyst, 10.0, 0.0..=1000.0, " rpm");
            changed |= num_field(ui, "Injector size", &mut t.injector_cc, 5.0, 100.0..=2500.0, " cc/min");
            changed |= num_field(ui, "Injector dead time", &mut t.injector_dead_ms, 0.01, 0.0..=3.0, " ms");
            changed |= opt_field(ui, "Overboost cut", &mut t.overboost_kpa, 200.0, 1.0, " kPa");
            ui.label("Fuel table holds");
            let mut u = t.fuel_units.clone().unwrap_or_else(|| guess_fuel_units(&t.fuel_low).to_string());
            egui::ComboBox::from_id_salt("fuel-units").selected_text(match u.as_str() { "afr" => "AFR target", "ms" => "injector ms", _ => "lambda target" }).show_ui(ui, |ui| {
                for (k, l) in [("lambda", "lambda target"), ("afr", "AFR target"), ("ms", "injector ms")] {
                    if ui.selectable_value(&mut u, k.to_string(), l).changed() {
                        t.fuel_units = Some(u.clone());
                        changed = true;
                    }
                }
            });
            ui.end_row();
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for (k, label) in TABLES {
                let exists = table_mut(&mut sh.tune, k).is_some();
                let text = if exists { RichText::new(label) } else { RichText::new(label).color(Color32::from_gray(120)) };
                if ui.selectable_label(self.table == k, text).clicked() {
                    self.table = k;
                    self.cell = None;
                }
            }
        });
        let e = &sh.sim.eng;
        let (rpm, map) = (e.rpm, e.map);
        let live_key = match self.table {
            "fuelLow" | "ignLow" | "vtcLow" => !e.high_cam,
            "fuelHigh" | "ignHigh" | "vtcHigh" => e.high_cam,
            _ => true,
        };
        let units = guess_fuel_units(&sh.tune.fuel_low);
        let key = self.table;
        if let Some(t) = table_mut(&mut sh.tune, key) {
            let all: Vec<f64> = t.data.iter().flatten().copied().collect();
            let (lo, hi) = (all.iter().cloned().fold(f64::INFINITY, f64::min), all.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
            let active = if live_key { Some(active_cell(t, rpm, map)) } else { None };
            if key.starts_with("fuel") {
                ui.label(RichText::new(format!("Values look like: {units}")).small());
            }
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("table").spacing([2.0, 2.0]).show(ui, |ui| {
                    ui.label(RichText::new("rpm \\ kPa").small());
                    for c in &t.cols {
                        ui.label(RichText::new(format!("{c:.0}")).small().strong());
                    }
                    ui.end_row();
                    for (ri, r) in t.rows.iter().enumerate() {
                        ui.label(RichText::new(format!("{r:.0}")).small().strong());
                        for ci in 0..t.cols.len() {
                            let v = t.data[ri][ci];
                            let mut text = RichText::new(format!("{v:.2}")).small().monospace().color(Color32::WHITE).background_color(heat(v, lo, hi));
                            if active == Some((ri, ci)) || self.cell == Some((ri, ci)) {
                                text = text.strong().underline();
                            }
                            if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                                self.cell = Some((ri, ci));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
            if let Some((r, c)) = self.cell {
                if r < t.rows.len() && c < t.cols.len() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{:.0} rpm, {:.0} kPa:", t.rows[r], t.cols[c]));
                        changed |= ui.add(egui::DragValue::new(&mut t.data[r][c]).speed(0.05)).changed();
                        if ui.button("−").clicked() { t.data[r][c] -= if key.starts_with("fuel") && units != "ms" { 0.01 } else { 0.5 }; changed = true; }
                        if ui.button("+").clicked() { t.data[r][c] += if key.starts_with("fuel") && units != "ms" { 0.01 } else { 0.5 }; changed = true; }
                    });
                }
            }
            ui.label(RichText::new("Underlined: the cell the engine is on now (or the one you clicked to edit).").small().color(Color32::from_gray(150)));
        } else {
            ui.label("This tune doesn't have that table yet. Paste one below, or start from a filled-in one:");
            if ui.button("Create this table").clicked() {
                let base = sh.tune.fuel_low.clone();
                let fill = |f: &dyn Fn(f64, f64) -> f64| Table { rows: base.rows.clone(), cols: base.cols.clone(), data: base.rows.iter().map(|&r| base.cols.iter().map(|&c| (f(r, c) * 100.0).round() / 100.0).collect()).collect() };
                let t = match key {
                    "fuelHigh" => sh.tune.fuel_low.clone(),
                    "ignHigh" => sh.tune.ign_low.clone(),
                    "vtcLow" => fill(&|r, c| honda_sim_core::gasflow::stock_vtc(r, c, false)),
                    "vtcHigh" => fill(&|r, c| honda_sim_core::gasflow::stock_vtc(r, c, true)),
                    "boostTarget" => fill(&|_, _| 101.3 + sh.tune.turbo.spring_kpa),
                    _ => fill(&|_, _| 0.0),
                };
                set_table(&mut sh.tune, key, t);
                changed = true;
            }
        }

        ui.separator();
        ui.label(RichText::new("Paste a table from FlashPro Manager").strong());
        ui.label(RichText::new("In FlashPro: select the table, Ctrl+A, Ctrl+C, then paste here. Column axis (MAP) on the first row, rpm down the left.").small());
        egui::ComboBox::from_id_salt("paste-target").selected_text(TABLES.iter().find(|t| t.0 == self.paste_target).unwrap().1).show_ui(ui, |ui| {
            for (k, label) in TABLES {
                ui.selectable_value(&mut self.paste_target, k, label);
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.paste).desired_rows(5).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
        if ui.button("Load pasted table").clicked() {
            match parse_table(&self.paste) {
                Ok(t) => {
                    if self.paste_target.starts_with("fuel") {
                        sh.tune.fuel_units = Some(guess_fuel_units(&t).to_string());
                    }
                    set_table(&mut sh.tune, self.paste_target, t);
                    self.paste_msg = format!("Loaded into {}.", TABLES.iter().find(|t| t.0 == self.paste_target).unwrap().1);
                    self.table = self.paste_target;
                    self.paste.clear();
                    changed = true;
                }
                Err(e) => self.paste_msg = e,
            }
        }
        if !self.paste_msg.is_empty() {
            ui.label(&self.paste_msg);
        }

        ui.separator();
        ui.label(RichText::new("Tune files").strong());
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.tune_name);
            if ui.button("Save").clicked() {
                sh.tune.name = self.tune_name.clone();
                let p = store::sub("tunes").join(format!("{}.json", store::safe_name(&self.tune_name)));
                sh.message = match store::save_json(&p, &sh.tune) { Ok(_) => format!("Saved {}", p.display()), Err(e) => e };
                self.tunes = store::list_tunes();
                changed = true;
            }
        });
        let mut load: Option<std::path::PathBuf> = None;
        for (name, path) in &self.tunes {
            ui.horizontal(|ui| {
                ui.label(name);
                if ui.small_button("Load").clicked() {
                    load = Some(path.clone());
                }
            });
        }
        if let Some(p) = load {
            match store::load_json::<Tune>(&p) {
                Some(t) => { self.tune_name = t.name.clone(); sh.tune = t; sh.message = format!("Loaded {}", p.display()); changed = true; }
                None => sh.message = format!("Couldn't read {}", p.display()),
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Open tunes folder").clicked() { store::open_folder(&store::sub("tunes")); }
            if ui.button("Placeholder tune").clicked() { sh.tune = Tune { parts: sh.tune.parts.clone(), materials: sh.tune.materials.clone(), ..placeholder_tune() }; changed = true; }
        });
        ui.label(RichText::new("Tunes are JSON, the same format the web version saves, so they move between the two.").small().color(Color32::from_gray(150)));
        if !sh.message.is_empty() {
            ui.label(RichText::new(&sh.message).small());
        }
        changed
    }

    fn settings(&mut self, ui: &mut egui::Ui, sh: &mut Shared) -> bool {
        let t = &mut sh.tune;
        let mut c = false;
        ui.heading("ECU settings");
        egui::Grid::new("ecu").num_columns(2).show(ui, |ui| {
            c |= num_field(ui, "VTEC point", &mut t.vtec_rpm, 25.0, 2000.0..=9000.0, " rpm");
            c |= num_field(ui, "VTEC minimum MAP", &mut t.vtec_min_map, 1.0, 0.0..=250.0, " kPa");
            c |= num_field(ui, "Rev limit", &mut t.rev_limit, 25.0, 3000.0..=9500.0, " rpm");
            c |= fuel_fields(ui, t);
            c |= num_field(ui, "Injector size", &mut t.injector_cc, 5.0, 100.0..=2500.0, " cc/min");
            c |= num_field(ui, "Injector dead time", &mut t.injector_dead_ms, 0.01, 0.0..=3.0, " ms");
            c |= num_field(ui, "Clutch capacity", &mut t.clutch_nm, 5.0, 100.0..=1500.0, " Nm");
            c |= opt_field(ui, "Overboost cut", &mut t.overboost_kpa, 200.0, 1.0, " kPa");
            c |= opt_field(ui, "Torque warning", &mut t.torque_limit_nm, 350.0, 5.0, " Nm");
            c |= opt_field(ui, "Ambient air", &mut t.ambient_c, 25.0, 0.5, " °C");
        });
        ui.separator();
        ui.heading("Induction");
        ui.horizontal(|ui| {
            if ui.button("N/A (stock)").clicked() { t.turbo.enabled = false; c = true; }
            if ui.button("GT35 turbo kit").clicked() { *t = turbo_setup(t); c = true; }
            if ui.button("Precision 6262").clicked() { *t = turbo_setup(t); t.turbo = honda_sim_core::tune::Turbo { enabled: true, ..turbo_6262() }; c = true; }
            if ui.button("Supercharger").clicked() { *t = turbo_setup(t); t.turbo = honda_sim_core::tune::Turbo { enabled: true, fixed: true, name: "Supercharger (boost held at the spring/duty target)".into(), spring_kpa: 88.0, max_boost_kpa: 88.0, ..turbo_gt35() }; c = true; }
        });
        let tb = &mut t.turbo;
        egui::Grid::new("turbo").num_columns(2).show(ui, |ui| {
            ui.label("Boost");
            let label = if tb.enabled { tb.name.clone() } else { "off (N/A)".to_string() };
            c |= ui.checkbox(&mut tb.enabled, label).changed();
            ui.end_row();
            if tb.enabled {
                ui.label("Intercooler");
                c |= ui.checkbox(&mut tb.intercooler, "front-mount").changed();
                ui.end_row();
                ui.label("Supercharger mode");
                c |= ui.checkbox(&mut tb.fixed, "hold boost at target").changed();
                ui.end_row();
                if !tb.fixed {
                    ui.label("Turbo coolant lines");
                    c |= ui.checkbox(&mut tb.water_cooled, "water-cooled centre section").changed();
                    ui.end_row();
                }
                c |= num_field(ui, "Wastegate spring", &mut tb.spring_kpa, 1.0, 10.0..=300.0, " kPa");
                c |= num_field(ui, "Max boost", &mut tb.max_boost_kpa, 1.0, 20.0..=350.0, " kPa");
                c |= num_field(ui, "Wastegate duty gain", &mut tb.wg_gain, 0.05, 0.0..=3.0, "");
                if !tb.fixed {
                    c |= num_field(ui, "Turbine flow area", &mut tb.turbine_area_cm2, 0.1, 2.0..=30.0, " cm²");
                    c |= num_field(ui, "Wastegate area", &mut tb.wastegate_area_cm2, 0.1, 0.0..=30.0, " cm²");
                    c |= num_field(ui, "Compressor tip", &mut tb.compressor_tip_mm, 0.5, 20.0..=90.0, " mm");
                }
            }
        });
        ui.separator();
        ui.heading("Intake and exhaust");
        let flow = &mut t.flow;
        let mut mm = |ui: &mut egui::Ui, label: &str, v: &mut Option<f64>, default_m: f64| {
            ui.label(label);
            let mut x = v.unwrap_or(default_m) * 1000.0;
            if ui.add(egui::DragValue::new(&mut x).speed(0.5).range(10.0..=2000.0).suffix(" mm")).changed() {
                *v = Some(x / 1000.0);
                c = true;
            }
            ui.end_row();
        };
        egui::Grid::new("flow").num_columns(2).show(ui, |ui| {
            mm(ui, "Throttle body bore", &mut flow.throttle_dia_m, 0.062);
            mm(ui, "Intake runner (effective)", &mut flow.runner_m, 0.15);
            mm(ui, "Exhaust (effective diameter)", &mut flow.exhaust_dia_m, 0.052);
        });
        c
    }

    fn parts(&mut self, ui: &mut egui::Ui, sh: &mut Shared) -> bool {
        let cat = catalog();
        let b = sh.sim.eng.bottom.clone();
        let mut c = false;
        c |= engine_section(ui, sh, &b);
        ui.separator();
        ui.heading("Bottom end");
        ui.label(RichText::new("Real aftermarket parts. Ones that don't fit with the chosen crank are greyed out with the reason.").small());
        let crank = &b.crank;
        ui.label("Crank / kit");
        egui::ComboBox::from_id_salt("crank").width(340.0).selected_text(&crank.name).show_ui(ui, |ui| {
            for k in &cranks_for(b.block) {
                let why = crank_problem(b.block, k);
                let label = match &why { Some(w) => format!("{} ({w})", k.name), None => k.name.clone() };
                if ui.add_enabled(why.is_none(), egui::Button::selectable(k.id == crank.id, label)).clicked() {
                    sh.tune.parts.crank = if k.id == "stock" { None } else { Some(k.id.clone()) };
                    sh.tune.materials.remove("crank");
                    sh.tune.materials.remove("piston");
                    sh.tune.materials.remove("rod");
                    c = true;
                }
            }
        });
        c |= material_row(ui, &mut sh.tune, &b, "crank");
        ui.add_space(4.0);
        ui.label("Rods");
        if let Some(kr) = &crank.kit_rod {
            ui.label(format!("{kr} (comes with the kit)"));
        } else {
            egui::ComboBox::from_id_salt("rod").width(340.0).selected_text(&b.rod.name).show_ui(ui, |ui| {
                for r in &cat.rods {
                    let why = rod_problem(crank, r, Some(&b.piston), b.block);
                    let label = match &why { Some(w) => format!("{} ({w})", r.name), None => r.name.clone() };
                    if ui.add_enabled(why.is_none(), egui::Button::selectable(r.id == b.rod.id, label)).clicked() {
                        sh.tune.parts.rod = if r.id == "stock" { None } else { Some(r.id.clone()) };
                        sh.tune.materials.remove("rod");
                        c = true;
                    }
                }
            });
        }
        for k in ["rod", "rodBolt", "bearing"] {
            c |= material_row(ui, &mut sh.tune, &b, k);
        }
        ui.add_space(4.0);
        ui.label("Pistons");
        if let Some(kp) = &crank.kit_piston {
            ui.label(format!("{} (comes with the kit)", kp.name));
        } else {
            egui::ComboBox::from_id_salt("piston").width(340.0).selected_text(&b.piston.name).show_ui(ui, |ui| {
                for p in &pistons_for(b.block) {
                    let why = piston_problem(crank, p, b.block);
                    let label = match &why { Some(w) => format!("{} ({w})", p.name), None => p.name.clone() };
                    if ui.add_enabled(why.is_none(), egui::Button::selectable(p.id == b.piston.id, label)).clicked() {
                        sh.tune.parts.piston = if p.id == "stock" { None } else { Some(p.id.clone()) };
                        sh.tune.materials.remove("piston");
                        if p.bore_mm > b.block.bore_mm + 1.0 && sh.tune.materials.get("sleeve").map(|s| s == "sleeve").unwrap_or(true) {
                            sh.tune.materials.insert("sleeve".into(), "sleeveDuctile".into());
                        }
                        c = true;
                    }
                }
            });
        }
        c |= material_row(ui, &mut sh.tune, &b, "piston");
        ui.add_space(6.0);
        egui::Grid::new("bottom").num_columns(2).show(ui, |ui| {
            for (k, v) in [
                ("Bore × stroke", format!("{} × {} mm", b.bore_mm, b.stroke_mm)),
                ("Rod length", format!("{} mm (rod/stroke {:.2})", b.rod_mm, b.rod_mm / b.stroke_mm)),
                ("Displacement", format!("{:.0} cc", b.displacement_l * 1000.0)),
                ("Compression", format!("{:.1}:1", b.compression)),
                ("Deck clearance", format!("{:.2} mm", b.deck_clearance_mm)),
                ("Deck height", format!("{} mm ({} block)", b.deck_mm, b.block.id)),
                ("Piston weight", format!("{:.0} g", b.piston_kg * 1000.0)),
            ] {
                ui.label(RichText::new(k).color(Color32::from_gray(160)));
                ui.label(v);
                ui.end_row();
            }
        });
        for i in &b.issues {
            ui.label(RichText::new(i).color(Color32::from_rgb(0xf2, 0xa9, 0x3b)));
        }
        for v in b.estimated.values() {
            ui.label(RichText::new(format!("Estimated: {v}")).small().color(Color32::from_gray(150)));
        }
        let mut srcs: Vec<(String, String)> = vec![];
        if crank.id != "stock" { srcs.extend(crank.sources.iter().take(2).map(|u| (crank.brand.clone(), u.clone()))); }
        if crank.kit_rod.is_none() && b.rod.id != "stock" { srcs.extend(b.rod.sources.iter().take(2).map(|u| (b.rod.brand.clone(), u.clone()))); }
        if crank.kit_piston.is_none() && b.piston.id != "stock" { srcs.extend(b.piston.sources.iter().take(2).map(|u| (b.piston.brand.clone(), u.clone()))); }
        if !srcs.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Sources:").small());
                for (i, (brand, u)) in srcs.iter().enumerate() {
                    ui.hyperlink_to(RichText::new(format!("{brand} {}", i + 1)).small(), u);
                }
            });
        }
        ui.separator();
        ui.heading("Head, block and valvetrain");
        ui.label(RichText::new("Pick an alloy per part. Values are published data unless marked est. (hover for why).").small());
        for k in ["block", "sleeve", "gasket", "headBolt", "valvetrain"] {
            c |= material_row(ui, &mut sh.tune, &b, k);
            ui.add_space(2.0);
        }
        c
    }

    fn internals(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        let s = &sh.sim.internals;
        ui.heading("Engine internals");
        ui.label(format!("Peak cylinder pressure {:.0} bar, piston crown {:.0} °C", s.pmax_bar, s.piston_c));
        ui.label(format!("Rod load {:.1} kN down / {:.1} kN up, sleeve {:.0} MPa", s.rod_comp_n / 1000.0, s.rod_tension_n / 1000.0, s.sleeve_mpa));
        egui::Grid::new("loads").num_columns(4).show(ui, |ui| {
            ui.label(RichText::new("Part").strong());
            ui.label(RichText::new("Load now").strong());
            ui.label(RichText::new("Life used").strong());
            ui.label(RichText::new("Status").strong());
            ui.end_row();
            for (i, p) in PARTS.iter().enumerate() {
                ui.label(p.name);
                ui.add(egui::ProgressBar::new((s.load[i] / 1.5) as f32).desired_width(110.0).fill(load_color(s.load[i])).text(format!("{:.0}%", s.load[i] * 100.0)));
                ui.label(format!("{:.1}%", s.damage[i] * 100.0));
                let st = part_status(s.load[i], s.damage[i]);
                let col = match st { "ok" => Color32::from_rgb(0x3f, 0xcf, 0x7f), "warn" => Color32::from_rgb(0xf2, 0xa9, 0x3b), _ => Color32::from_rgb(0xff, 0x5a, 0x4e) };
                ui.label(RichText::new(st.to_uppercase()).color(col));
                ui.end_row();
            }
        });
        ui.label(RichText::new("Load is demand ÷ capacity. Below 80% a part lives indefinitely; above it fatigue builds up; at 150% it breaks at once.").small());
        if ui.button("Rebuild engine").clicked() {
            sh.sim.rebuild_engine();
            sh.sim.start_engine();
        }
    }

    fn cooling(&mut self, ui: &mut egui::Ui, sh: &mut Shared) -> bool {
        let e = &sh.sim.eng;
        let th = &e.thermal;
        let g = &th.g;
        ui.heading("Coolant and oil");
        ui.horizontal(|ui| {
            if ui.button("Cold start").on_hover_text("Coolant and oil at the outside air temperature, as after sitting overnight").clicked() {
                sh.events.push("coldEngine".into());
            }
            if ui.button("Warm engine").on_hover_text("Coolant at 90 °C and oil at 100 °C").clicked() {
                sh.events.push("warmEngine".into());
            }
        });
        let grey = |s: &str| RichText::new(s.to_string()).color(Color32::from_gray(160));
        let temp_color = |c: f64, hot: f64| if c >= hot { Color32::from_rgb(0xe0, 0x50, 0x40) } else if c < 60.0 { Color32::from_rgb(0x70, 0xa0, 0xe0) } else { Color32::from_gray(230) };
        egui::Grid::new("cooling-live").num_columns(2).show(ui, |ui| {
            let boil = boiling_c(g);
            ui.label(grey("Coolant, head (ECT1)"));
            ui.label(RichText::new(format!("{:.1} °C", th.coolant_c)).color(temp_color(th.coolant_c, 105.0)));
            ui.end_row();
            ui.label(grey("Radiator outlet (ECT2)"));
            ui.label(format!("{:.1} °C", th.ect2_c));
            ui.end_row();
            ui.label(grey("Pump inlet"));
            ui.label(format!("{:.1} °C", th.inlet_c));
            ui.end_row();
            ui.label(grey("Thermostat"));
            ui.label(format!("{:.0}% open (opens {:.0} °C, fully {:.0} °C)", th.thermostat * 100.0, g.thermostat_open_c, g.thermostat_full_c));
            ui.end_row();
            ui.label(grey("Coolant flow"));
            ui.label(format!("pump {:.0} L/min: radiator {:.0}, bypass {:.0}", th.pump_lpm, th.radiator_lpm, th.bypass_lpm));
            ui.end_row();
            ui.label(grey("Fans"));
            ui.label(format!("{} (on at {:.0} °C, off at {:.0} °C on ECT2)", if th.fan_on { "ON" } else { "off" }, g.fan_on_c, g.fan_off_c));
            ui.end_row();
            ui.label(grey("Air through the radiator"));
            ui.label(format!("{:.1} m/s", th.air_ms));
            ui.end_row();
            ui.label(grey("Heat into coolant / oil"));
            ui.label(format!("{:.1} kW / {:.1} kW", th.heat_to_coolant_kw, th.heat_to_oil_kw));
            ui.end_row();
            ui.label(grey("Radiator rejecting"));
            ui.label(format!("{:.1} kW", th.radiator_kw));
            ui.end_row();
            ui.label(grey("Boiling point"));
            ui.label(if th.boiling { RichText::new(format!("{boil:.0} °C, BOILING for {:.0} s", th.overheat_s)).color(Color32::from_rgb(0xe0, 0x50, 0x40)) } else { RichText::new(format!("{boil:.0} °C at {:.0} kPa cap", g.cap_kpa)) });
            ui.end_row();
            ui.label(grey("Oil temperature"));
            ui.label(RichText::new(format!("{:.1} °C", th.oil_c)).color(temp_color(th.oil_c, 130.0)));
            ui.end_row();
            ui.label(grey("Oil pressure"));
            ui.label(format!("{:.0} kPa ({:.0} psi){}", e.oil_kpa, e.oil_kpa * 0.145, if e.oil_kpa >= g.oil_jet_kpa { ", piston jets spraying" } else { "" }));
            ui.end_row();
            ui.label(grey("Oil viscosity"));
            ui.label(format!("{:.1} cSt ({})", viscosity_cst(th.oil_c, g), g.oil_grade));
            ui.end_row();
            ui.label(grey("Oil cooler"));
            ui.label(format!("{:.1} kW", th.oil_cooler_kw));
            ui.end_row();
            ui.label(grey("Friction"));
            ui.label(format!("{:.1} Nm", e.friction_nm));
            ui.end_row();
        });
        if let Some(tl) = honda_sim_core::sim::turbo_lines(e, sh.sim.running) {
            ui.add_space(4.0);
            ui.label(RichText::new("Turbo lines").strong());
            egui::Grid::new("turbo-lines").num_columns(2).show(ui, |ui| {
                ui.label(grey("Oil feed"));
                ui.label(format!("{:.2} L/min through the restrictor, {:.1} kW into the oil", th.turbo_oil_lpm, th.turbo_oil_kw));
                ui.end_row();
                ui.label(grey("Coolant lines"));
                ui.label(if tl.water_cooled { format!("{:.1} kW into the coolant", th.turbo_water_kw) } else { "none (oil-cooled only)".into() });
                ui.end_row();
                ui.label(grey("Turbine housing"));
                ui.label(format!("{:.0} °C", th.turbo_housing_c));
                ui.end_row();
                ui.label(grey("Bearing housing"));
                ui.label(RichText::new(format!("{:.0} °C", th.turbo_bearing_c)).color(temp_color(th.turbo_bearing_c, g.turbo_coke_c)));
                ui.end_row();
                if th.turbo_coke_s > 0.0 {
                    ui.label(grey("Oil coking"));
                    ui.label(RichText::new(format!("{:.0} s hot with no oil moving", th.turbo_coke_s)).color(Color32::from_rgb(0xe0, 0x50, 0x40)));
                    ui.end_row();
                }
            });
            ui.label(RichText::new("Idle a minute before switching off after hard driving: the oil stops when the engine does, and the turbine housing soaks into the bearing.").small().color(Color32::from_gray(150)));
        }
        if let (Some(l), true) = (&sh.sim.internals.lube, sh.sim.running) {
            ui.add_space(4.0);
            ui.label(RichText::new("Rotating assembly").strong());
            egui::Grid::new("rotating-oil").num_columns(2).show(ui, |ui| {
                ui.label(grey("Crank feed"));
                ui.label(format!("{:.1} L/min to the mains; rod journals need {:.0} kPa", l.feed_lpm * 0.5, l.rod_need_kpa));
                ui.end_row();
                ui.label(grey("Rod bearing supply"));
                ui.label(if l.supply >= 1.0 { RichText::new("full") } else { RichText::new(format!("{:.0}%, STARVING", l.supply * 100.0)).color(Color32::from_rgb(0xe0, 0x50, 0x40)) });
                ui.end_row();
                ui.label(grey("Main bearings 1–5"));
                ui.label(format!("{:.2} L/min each, film {:.0} °C, strength {:.2}", l.mains[0].feed_lpm, l.mains[0].film_c, l.mains[0].film));
                ui.end_row();
                ui.label(grey("Rod bearings 1–4"));
                ui.label(format!("{:.2} L/min each through the crank, film {:.0} °C, strength {:.2}", l.rods[0].feed_lpm, l.rods[0].film_c, l.rods[0].film));
                ui.end_row();
                ui.label(grey("Piston squirters"));
                ui.label(if l.jets[0].open { format!("spraying, {:.2} L/min each", l.jets[0].flow_lpm) } else { "shut (under 200 kPa)".into() });
                ui.end_row();
                ui.label(grey("Splash back to the pan"));
                ui.label(format!("{:.1} L/min", l.splash_lpm));
                ui.end_row();
            });
            ui.label(RichText::new("Film strength 1.0 = 100 °C oil at full supply. Lower means a thinner film: the rod bearings load up and wear faster.").small().color(Color32::from_gray(150)));
        }
        ui.label(RichText::new("Cold oil is thick: more friction and oil pressure. The ECU holds VTEC off and raises idle until the engine warms up.").small().color(Color32::from_gray(150)));

        ui.separator();
        ui.heading("Cooling system parts");
        let t = &mut sh.tune;
        let mut c = false;
        let mut cg = t.cooling();
        let stock = Cooling::default();
        egui::Grid::new("cooling-set").num_columns(2).show(ui, |ui| {
            ui.label("Thermostat");
            egui::ComboBox::from_id_salt("thermostat").selected_text(if cg.thermostat_open_c == stock.thermostat_open_c && cg.thermostat_full_c == stock.thermostat_full_c { "Stock (78 °C)" } else { "Custom" }).show_ui(ui, |ui| {
                if ui.selectable_label(false, "Stock 19301-RAF-004 (opens 78 °C, fully 90 °C)").clicked() { cg.thermostat_open_c = 78.0; cg.thermostat_full_c = 90.0; c = true; }
                if ui.selectable_label(false, "Low-temp (fully open 68 °C)").clicked() { cg.thermostat_open_c = 60.0; cg.thermostat_full_c = 68.0; c = true; }
            });
            ui.end_row();
            c |= num_field(ui, "Thermostat opens", &mut cg.thermostat_open_c, 0.5, 40.0..=100.0, " °C");
            c |= num_field(ui, "Thermostat fully open", &mut cg.thermostat_full_c, 0.5, 45.0..=110.0, " °C");
            c |= num_field(ui, "Fans on (ECT2)", &mut cg.fan_on_c, 0.5, 60.0..=115.0, " °C");
            c |= num_field(ui, "Fans off (ECT2)", &mut cg.fan_off_c, 0.5, 55.0..=110.0, " °C");
            ui.label("Radiator");
            let two_row = cg.radiator_ua_wk_at5ms > stock.radiator_ua_wk_at5ms;
            egui::ComboBox::from_id_salt("radiator").selected_text(if two_row { "Aluminium 2-row" } else { "Stock single-row" }).show_ui(ui, |ui| {
                if ui.selectable_label(!two_row, "Stock Denso single-row (16 mm core)").clicked() { cg.radiator_ua_wk_at5ms = stock.radiator_ua_wk_at5ms; cg.radiator_metal_jk = stock.radiator_metal_jk; c = true; }
                if ui.selectable_label(two_row, "Aluminium 2-row (40 mm core)").clicked() { cg.radiator_ua_wk_at5ms = 2600.0; cg.radiator_metal_jk = 7000.0; c = true; }
            });
            ui.end_row();
            ui.label("Radiator cap");
            egui::ComboBox::from_id_salt("cap").selected_text(format!("{:.0} kPa ({:.0} psi)", cg.cap_kpa, cg.cap_kpa * 0.145)).show_ui(ui, |ui| {
                if ui.selectable_label(cg.cap_kpa == 110.0, "Stock 16 psi").clicked() { cg.cap_kpa = 110.0; c = true; }
                if ui.selectable_label(cg.cap_kpa == 130.0, "1.3 bar (19 psi)").clicked() { cg.cap_kpa = 130.0; c = true; }
            });
            ui.end_row();
            ui.label("Oil cooler");
            egui::ComboBox::from_id_salt("oil-cooler").selected_text(match cg.oil_cooler.as_str() { "water" => "Stock water-to-oil", "air" => "Air-to-oil (aftermarket)", _ => "None" }).show_ui(ui, |ui| {
                for (k, l) in [("water", "Stock water-to-oil (at the filter)"), ("air", "Air-to-oil (aftermarket)"), ("none", "None")] {
                    if ui.selectable_label(cg.oil_cooler == k, l).clicked() { cg.oil_cooler = k.into(); c = true; }
                }
            });
            ui.end_row();
            ui.label("Oil");
            egui::ComboBox::from_id_salt("oil-grade").selected_text(cg.oil_grade.clone()).show_ui(ui, |ui| {
                // Typical kinematic viscosity at 40 and 100 °C for each grade.
                for (gr, v40, v100) in [("0W-20", 44.0, 8.5), ("5W-30", 62.0, 10.5), ("10W-40", 95.0, 14.5), ("15W-50", 130.0, 18.5)] {
                    if ui.selectable_label(cg.oil_grade == gr, if gr == "5W-30" { "5W-30 (Honda spec)" } else { gr }).clicked() {
                        cg.oil_grade = gr.into(); cg.oil_visc40 = v40; cg.oil_visc100 = v100; c = true;
                    }
                }
            });
            ui.end_row();
            c |= num_field(ui, "VTEC minimum coolant", &mut cg.ect_vtec_min_c, 1.0, -40.0..=110.0, " °C");
            c |= opt_field(ui, "Outside air", &mut t.ambient_c, 25.0, 0.5, " °C");
        });
        if ui.button("Stock cooling system").clicked() {
            cg = Cooling::default();
            c = true;
        }
        if c {
            t.cooling = if cg == stock { None } else { Some(cg) };
        }
        c
    }

    fn wheel(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        ui.heading("Wheel and pedals");
        if let Some(e) = &sh.pad_error {
            ui.label(RichText::new(e).color(Color32::from_rgb(0xff, 0x5a, 0x4e)));
        }
        if sh.pads.is_empty() {
            ui.label("No wheel or pedals detected. Plug them in; they show up here.");
        } else {
            for p in &sh.pads {
                ui.label(format!("Connected: {p}"));
            }
        }
        ui.label(RichText::new("Learn: click, then press the pedal all the way and let it go (or press the button).").small());
        let learning = sh.learning.as_ref().map(|l| l.name().to_string());
        let mut start: Option<Learning> = None;
        let mut clear: Option<(bool, String)> = None;
        egui::Grid::new("map").num_columns(3).show(ui, |ui| {
            for a in AXES {
                ui.label(a);
                ui.label(sh.mapping.axes.get(a).map(|m| format!("axis {} on {}", m.code & 0x7fff_ffff, m.pad)).unwrap_or("not set".into()));
                ui.horizontal(|ui| {
                    if ui.button(if learning.as_deref() == Some(a) { "press it…" } else { "Learn" }).clicked() {
                        start = Some(Learning::Axis { name: a.into(), baseline: HashMap::new(), extreme: HashMap::new() });
                    }
                    if ui.small_button("×").clicked() { clear = Some((true, a.into())); }
                });
                ui.end_row();
            }
            for b in BUTTONS {
                ui.label(b);
                ui.label(sh.mapping.buttons.get(b).map(|m| format!("button {} on {}", m.code, m.pad)).unwrap_or("not set".into()));
                ui.horizontal(|ui| {
                    if ui.button(if learning.as_deref() == Some(b) { "press it…" } else { "Learn" }).clicked() {
                        start = Some(Learning::Button { name: b.into() });
                    }
                    if ui.small_button("×").clicked() { clear = Some((false, b.into())); }
                });
                ui.end_row();
            }
        });
        if let Some(l) = start {
            sh.learning = Some(l);
            sh.learned = None;
        }
        if let Some((axis, k)) = clear {
            if axis { sh.mapping.axes.remove(&k); } else { sh.mapping.buttons.remove(&k); }
            sh.save_mapping();
        }
        if sh.learning.is_some() && ui.button("Cancel learning").clicked() {
            sh.learning = None;
        }
        if let Some(m) = &sh.learned {
            ui.label(RichText::new(format!("Learned {m}")).color(Color32::from_rgb(0x3f, 0xcf, 0x7f)));
        }
    }

    fn log(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        ui.heading("Datalog");
        ui.label(format!("{} rows at 20 Hz", sh.sim.log.len()));
        ui.horizontal(|ui| {
            if ui.button(if sh.sim.logging { "Stop logging" } else { "Start logging" }).clicked() {
                sh.sim.logging = !sh.sim.logging;
            }
            if ui.button("Clear").clicked() {
                sh.sim.log.clear();
            }
            if ui.button("Save CSV").clicked() {
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let p = store::sub("logs").join(format!("log-{secs}.csv"));
                sh.message = match std::fs::write(&p, log_to_csv(&sh.sim.log)) { Ok(_) => format!("Saved {}", p.display()), Err(e) => e.to_string() };
            }
            if ui.button("Open logs folder").clicked() {
                store::open_folder(&store::sub("logs"));
            }
        });
        if !sh.message.is_empty() {
            ui.label(RichText::new(&sh.message).small());
        }
        ui.label(RichText::new("Same columns as the web version's CSV, so logs open in the same tools.").small().color(Color32::from_gray(150)));
    }
}
