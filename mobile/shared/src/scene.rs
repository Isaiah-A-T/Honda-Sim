//! The 3D engine view on a phone: the viewer's engine model (honda-sim-viewer's `Engine`) with the viewer
//! window's camera presets, view modes and per-mode readouts (viewer/src/app.rs), changed for touch: drag to turn,
//! pinch to zoom, tap a part to name it (and pick it in Materials mode). The viewer's window loop can't run on a
//! phone (it creates its own desktop window), so this is that loop's per-frame work, called by src/app.rs.

use honda_sim_viewer::engine::{material_lookup, Castings, Engine, Mode};
use honda_sim_viewer::kinematics::{cycle_angle, set_geometry, stroke_name, Side, VTC_MAX_DEG};
use honda_sim_viewer::{materials, SimState};
use std::collections::HashMap;
use three_d::*;

const TARGET: Vec3 = Vec3 { x: 0.0, y: 1.3, z: 0.0 };

pub struct View {
    pub label: &'static str,
    dir: [f32; 3],
    dist: f32,
    target: Vec3,
}

pub const VIEWS: [View; 6] = [
    View { label: "Exhaust side", dir: [-0.45, 0.62, -0.64], dist: 11.5, target: TARGET },
    View { label: "Intake side", dir: [0.35, 0.8, 1.0], dist: 18.0, target: TARGET },
    View { label: "Front", dir: [-1.0, 0.12, 0.001], dist: 10.5, target: TARGET },
    View { label: "Side", dir: [0.001, 0.1, -1.0], dist: 10.5, target: TARGET },
    View { label: "Top", dir: [0.001, 1.0, -0.08], dist: 11.0, target: TARGET },
    View { label: "Valvetrain", dir: [-0.45, 0.3, -1.0], dist: 3.4, target: Vec3 { x: -1.0, y: 3.25, z: -0.3 } },
];

const STRESS_PARTS: [(&str, &str); 12] = [
    ("rods", "Connecting rods"), ("pistons", "Pistons"), ("crank", "Crankshaft"), ("valves", "Valves"), ("cams", "Camshafts"),
    ("head", "Head"), ("block", "Block"), ("exhaust", "Exhaust heat"), ("intake", "Intake pressure"), ("turbo", "Turbo"),
    ("clutch", "Clutch"), ("flywheel", "Flywheel"),
];

fn view_pos(v: &View, aspect: f32) -> Vec3 {
    let fit = (1.3 / aspect.max(0.3)).max(1.0);
    v.target + vec3(v.dir[0], v.dir[1], v.dir[2]).normalize() * v.dist * fit
}

fn level_color(v: f32) -> egui::Color32 {
    if v >= 1.0 { egui::Color32::from_rgb(0xff, 0x5a, 0x4e) }
    else if v >= 0.8 { egui::Color32::from_rgb(0xf2, 0xa9, 0x3b) }
    else if v >= 0.5 { egui::Color32::from_rgb(0xe8, 0xc6, 0x3a) }
    else { egui::Color32::from_rgb(0x3f, 0xcf, 0x7f) }
}

fn bar(ui: &mut egui::Ui, label: &str, v: f32, color: egui::Color32, text: String) {
    ui.horizontal(|ui| {
        ui.add_sized([120.0, 18.0], egui::Label::new(label));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(110.0, 10.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, egui::Color32::from_gray(45));
        let mut fill = rect;
        fill.set_width(rect.width() * v.clamp(0.0, 1.0));
        ui.painter().rect_filled(fill, 3.0, color);
        ui.monospace(text);
    });
}

fn stress_bar(ui: &mut egui::Ui, label: &str, v: f32) {
    bar(ui, label, v / 1.2, level_color(v), format!("{:>4.0}%", v * 100.0));
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text.to_uppercase()).small().strong().color(egui::Color32::from_gray(150)));
}

fn swatch(ui: &mut egui::Ui, c: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
}

/// The view's settings, kept when the phone drops the graphics (app in the background) and the GPU side is rebuilt.
pub struct SceneUi {
    pub mode: Mode,
    pub castings: Castings,
    pub slow_mo: f32,
    pub choices: HashMap<String, String>,
    pub selected: Option<String>,
    /// Camera preset to fly to next frame.
    pub fly_to: Option<usize>,
    pub view: usize,
}

impl Default for SceneUi {
    fn default() -> Self {
        SceneUi { mode: Mode::Full, castings: Castings::SeeThrough, slow_mo: 0.02, choices: HashMap::new(), selected: None, fly_to: Some(0), view: 0 }
    }
}

/// What the GPU holds for the view.
pub struct Scene {
    ctx: Context,
    camera: Camera,
    control: OrbitControl,
    engine: Engine,
    ambient: AmbientLight,
    sun: DirectionalLight,
    fill: DirectionalLight,
    view_anim: Option<(Vec3, Vec3, Vec3, Vec3, f32)>,
    /// A tapped part's name and when it was tapped.
    pub tapped: Option<(String, f32)>,
    press_at: Option<(PhysicalPoint, f32)>,
}

impl Scene {
    pub fn new(ctx: &Context) -> Scene {
        Scene {
            ctx: ctx.clone(),
            camera: Camera::new_perspective(Viewport::new_at_origo(1, 1), view_pos(&VIEWS[0], 1.6), TARGET, vec3(0.0, 1.0, 0.0), degrees(38.0), 0.05, 100.0),
            control: OrbitControl::new(TARGET, 1.5, 30.0),
            engine: Engine::new(ctx),
            ambient: AmbientLight::new(ctx, 0.55, Srgba::new(225, 232, 255, 255)),
            sun: DirectionalLight::new(ctx, 2.2, Srgba::WHITE, vec3(-0.4, -0.8, -0.5)),
            fill: DirectionalLight::new(ctx, 0.9, Srgba::new(200, 210, 255, 255), vec3(0.6, -0.3, 0.7)),
            view_anim: None,
            tapped: None,
            press_at: None,
        }
    }

    /// Readouts and the current mode's panel, for the menu's 3D view sheet.
    pub fn panel(&self, ui: &mut egui::Ui, s: &mut SceneUi, state: &SimState) {
        let crank = self.engine.crank;
        let (lifts, pins, specs) = (self.engine.lifts, self.engine.pins, self.engine.specs);
        heading(ui, "View");
        ui.horizontal_wrapped(|ui| {
            for m in Mode::ALL {
                // As in the desktop app, materials are edited on the Parts page, so there's no Materials mode.
                if m == Mode::Materials { continue; }
                if ui.selectable_label(s.mode == m, m.label()).clicked() {
                    s.mode = m;
                    if m != Mode::Materials { s.selected = None; }
                }
            }
        });
        heading(ui, "Camera");
        ui.horizontal_wrapped(|ui| {
            for (i, v) in VIEWS.iter().enumerate() {
                if ui.selectable_label(s.view == i, v.label).clicked() { s.view = i; s.fly_to = Some(i); }
            }
        });
        ui.label(egui::RichText::new("Drag the engine to turn it, pinch to zoom, tap a part to name it.").small().color(egui::Color32::from_gray(150)));
        ui.separator();
        // Readouts
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{:.0}", state.rpm)).size(30.0).monospace());
            ui.label("rpm");
        });
        ui.horizontal(|ui| {
            let lamp = |ui: &mut egui::Ui, on: bool, text: &str, c: egui::Color32| {
                let t = egui::RichText::new(text).small().strong();
                ui.label(if on { t.background_color(c).color(egui::Color32::WHITE) } else { t.color(egui::Color32::from_gray(110)) });
            };
            lamp(ui, state.vtec, " VTEC ", egui::Color32::from_rgb(0x1f, 0x9d, 0x55));
            lamp(ui, state.knock, " KNOCK ", egui::Color32::from_rgb(0xd4, 0x2a, 0x1f));
            lamp(ui, state.fuel_cut && state.rpm > 7800.0, " CUT ", egui::Color32::from_rgb(0xd9, 0x8a, 0x00));
        });
        egui::Grid::new("readouts").num_columns(2).show(ui, |ui| {
            ui.label("MAP"); ui.monospace(format!("{:.0} kPa", state.map_kpa)); ui.end_row();
            ui.label("Boost"); ui.monospace(if state.turbo { format!("{:.1} psi", state.boost_kpa * 0.145) } else { "NA".into() }); ui.end_row();
            ui.label("Ignition"); ui.monospace(format!("{:.1}° BTDC", state.ign_deg)); ui.end_row();
            ui.label("Intake air"); ui.monospace(format!("{:.0} °C", state.iat_c)); ui.end_row();
            ui.label("Induction"); ui.monospace(if !state.turbo { "N/A" } else if state.intercooler { "Turbo, intercooled" } else { "Turbo, no intercooler" }); ui.end_row();
            for (k, v) in &state.readouts { ui.label(k); ui.monospace(v); ui.end_row(); }
        });
        ui.separator();

        // Mode panel
        heading(ui, s.mode.label());
        match s.mode {
            Mode::Full => {
                ui.label("Everything at once. Tap a part for its name and load.");
                ui.horizontal(|ui| {
                    ui.label("Block and head");
                    ui.selectable_value(&mut s.castings, Castings::SeeThrough, "See-through");
                    ui.selectable_value(&mut s.castings, Castings::Solid, "Solid");
                    ui.selectable_value(&mut s.castings, Castings::Hidden, "Hidden");
                });
            }
            Mode::Internals => {
                ui.label("Crank, rods, pistons and the valvetrain with the castings and plumbing taken away.");
                bar(ui, "VTC advance", state.vtc_deg / VTC_MAX_DEG, egui::Color32::from_rgb(0x3f, 0xcf, 0x7f), format!("{:.0}°", state.vtc_deg));
                egui::Grid::new("cyls").num_columns(5).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.weak("Cyl"); ui.weak("Stroke"); ui.weak("Intake"); ui.weak("Exhaust"); ui.weak("Rockers"); ui.end_row();
                    for c in 1..=4 {
                        ui.monospace(c.to_string());
                        ui.label(stroke_name(cycle_angle(c, crank)));
                        for (k, (side, col)) in [(Side::Intake, egui::Color32::from_rgb(0x6e, 0xc8, 0xff)), (Side::Exhaust, egui::Color32::from_rgb(0xff, 0x8a, 0x3d))].into_iter().enumerate() {
                            let l = lifts[c - 1][k];
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 8.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 3.0, egui::Color32::from_gray(45));
                            let mut f = rect;
                            f.set_width(rect.width() * (l / specs.get(side, true).lift_mm).min(1.0));
                            ui.painter().rect_filled(f, 3.0, col);
                        }
                        let p = pins[c - 1];
                        let (text, col) = if p[0] >= 0.999 && p[1] >= 0.999 { ("VTEC", egui::Color32::from_rgb(0x3f, 0xcf, 0x7f)) }
                            else if p[0] > 0.0 || p[1] > 0.0 { ("pin moving", egui::Color32::from_rgb(0xe8, 0xc6, 0x3a)) }
                            else { ("low cam", egui::Color32::from_gray(140)) };
                        ui.label(egui::RichText::new(text).color(col).small());
                        ui.end_row();
                    }
                });
                ui.weak("Firing order 1-3-4-2. Lift bars are full at VTEC-lobe peak lift. When VTEC switches on, oil pressure pushes a pin through each cylinder's three rocker followers, but it can only slide while they line up on the base circle, so each cylinder switches at its own point in the cycle.");
                heading(ui, if state.cams.is_some() { "Cam card (from the sim)" } else { "Cam card (stand-in until the sim sends it)" });
                egui::Grid::new("cams").num_columns(4).spacing([10.0, 2.0]).show(ui, |ui| {
                    ui.weak("Lobe"); ui.weak("Lift"); ui.weak("Dur @1mm"); ui.weak("Centre"); ui.end_row();
                    for (name, sp) in [("Intake low", specs.intake_low), ("Intake VTEC", specs.intake_high), ("Exhaust low", specs.exhaust_low), ("Exhaust VTEC", specs.exhaust_high)] {
                        ui.label(name); ui.monospace(format!("{:.1} mm", sp.lift_mm)); ui.monospace(format!("{:.0}°", sp.duration_deg)); ui.monospace(format!("{:.0}°", sp.centerline_deg)); ui.end_row();
                    }
                });
            }
            Mode::Block => {
                ui.label("The block, head, cam cover and oil pan as solid castings.");
                egui::Grid::new("spec").num_columns(2).show(ui, |ui| {
                    ui.label("Displacement"); ui.monospace("1,998 cc"); ui.end_row();
                    ui.label("Bore × stroke"); ui.monospace("86 × 86 mm"); ui.end_row();
                    ui.label("Rod length"); ui.monospace("139 mm"); ui.end_row();
                    ui.label("Bore spacing"); ui.monospace("96 mm"); ui.end_row();
                });
                stress_bar(ui, "Block", state.stress_of("block", None));
                stress_bar(ui, "Head", state.stress_of("head", None));
            }
            Mode::Airflow => {
                ui.label("Charge air in blue, exhaust in orange. Each runner only moves while its valve is open, so the cylinders breathe in firing order.");
                egui::Grid::new("air").num_columns(2).show(ui, |ui| {
                    ui.label("Airflow"); ui.monospace(format!("{:.0} g/s", state.air_gs)); ui.end_row();
                    ui.label("Throttle"); ui.monospace(format!("{:.0}%", state.throttle * 100.0)); ui.end_row();
                });
                ui.horizontal(|ui| {
                    for c in 1..=4 {
                        let (i, e) = (lifts[c - 1][0], lifts[c - 1][1]);
                        let (t, col) = if i > e && i > 0.5 { ("in", egui::Color32::from_rgb(0x6e, 0xc8, 0xff)) } else if e > 0.5 { ("out", egui::Color32::from_rgb(0xff, 0x8a, 0x3d)) } else { ("sealed", egui::Color32::from_gray(120)) };
                        ui.label(egui::RichText::new(format!("{c} {t}")).color(col).monospace());
                    }
                });
                stress_bar(ui, "Exhaust heat", state.stress_of("exhaust", None));
                stress_bar(ui, "Intake pressure", state.stress_of("intake", None));
                stress_bar(ui, "Turbo", state.stress_of("turbo", None));
            }
            Mode::Cooling => {
                ui.label("Coolant and oil, coloured by temperature. Coolant runs from the water pump through the block's water jacket and up into the head, out to the radiator once the thermostat opens (round the bypass until then) and through the heater core. Oil runs from the pan through the filter into the main gallery: to the main bearings, through drillings in the crank to the rod bearings, out of the piston squirters onto the pistons, and up to the head, then falls back to the pan.");
                if state.turbo {
                    ui.label(if state.turbo_water_cooled {
                        "Turbo: oil fed from a sandwich plate under the filter, drained into the pan; coolant from the block's exhaust side, returned to the heater hose."
                    } else {
                        "Turbo: oil fed from a sandwich plate under the filter, drained into the pan (oil-cooled centre section)."
                    });
                }
                let tp = honda_sim_viewer::engine::Temps::of(&state);
                let swatch_row = |ui: &mut egui::Ui, label: &str, c: [u8; 3], text: String| {
                    ui.horizontal(|ui| {
                        swatch(ui, c);
                        ui.add_sized([120.0, 18.0], egui::Label::new(label));
                        ui.monospace(text);
                    });
                };
                swatch_row(ui, "Coolant out", honda_sim_viewer::engine::coolant_rgb(tp.hot), format!("{:.0} °C", tp.hot));
                swatch_row(ui, "From radiator", honda_sim_viewer::engine::coolant_rgb(tp.cold), format!("{:.0} °C{}", tp.cold, if state.coolant_out_c.is_none() { " (est.)" } else { "" }));
                swatch_row(ui, "Oil", honda_sim_viewer::engine::oil_rgb(tp.oil), format!("{:.0} °C", tp.oil));
                ui.horizontal(|ui| {
                    ui.add_sized([134.0, 18.0], egui::Label::new("Oil pressure"));
                    ui.monospace(format!("{:.0} kPa{}", tp.oil_kpa, if state.oil_kpa.is_none() { " (est.)" } else { "" }));
                });
                bar(ui, "Thermostat", state.thermostat, egui::Color32::from_rgb(0x6e, 0xc8, 0xff), format!("{:.0}% open", state.thermostat * 100.0));
                bar(ui, "Fans", state.fan, egui::Color32::from_rgb(0x9a, 0xa3, 0xad), if state.fan > 0.0 { format!("{:.0}%", state.fan * 100.0) } else { "off".into() });
                ui.weak("Coolant: blue cold, cyan warming, yellow at operating temperature, red overheating. Oil: dark when cold, amber when hot.");
            }
            Mode::Stress => {
                ui.label("Parts tint yellow, orange, then red as they near or pass the limit the sim uses.");
                for (k, l) in STRESS_PARTS { stress_bar(ui, l, state.stress_of(k, None)); }
            }
            Mode::Materials => {
                if let Some(pm) = state.part_materials.clone() {
                    ui.label("The materials the sim's internals model uses. Tap a part in the view to find it here.");
                    let mut keys: Vec<_> = pm.keys().cloned().collect();
                    keys.sort();
                    for k in keys {
                        let m = &pm[&k];
                        let fam = materials::family(&m.family);
                        let frame = egui::Frame::group(ui.style()).stroke(if s.selected.as_deref() == Some(k.as_str()) { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0xff, 0x3b, 0x4f)) } else { egui::Stroke::NONE });
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| { if let Some(f) = fam { swatch(ui, f.color); } ui.strong(if m.part.is_empty() { &k } else { &m.part }); });
                            ui.label(&m.label);
                            if !m.detail.is_empty() { ui.weak(&m.detail); }
                        });
                    }
                } else {
                    ui.label("Colour shows each part's material. Pick one per part here or tap a part in the view.");
                    ui.horizontal_wrapped(|ui| {
                        for f in materials::FAMILIES { swatch(ui, f.color); ui.label(f.label); }
                    });
                    for cat in materials::CATALOG {
                        let cur = s.choices.get(cat.key).cloned().unwrap_or(cat.options[0].id.to_string());
                        let opt = cat.options.iter().find(|o| o.id == cur).unwrap_or(&cat.options[0]);
                        let frame = egui::Frame::group(ui.style()).stroke(if s.selected.as_deref() == Some(cat.key) { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0xff, 0x3b, 0x4f)) } else { egui::Stroke::NONE });
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| { if let Some(f) = materials::family(opt.family) { swatch(ui, f.color); } ui.strong(cat.label); });
                            let mut pick = cur.clone();
                            egui::ComboBox::from_id_salt(cat.key).selected_text(opt.label).width(220.0).show_ui(ui, |ui| {
                                for o in cat.options { ui.selectable_value(&mut pick, o.id.to_string(), o.label); }
                            });
                            if pick != cur { s.choices.insert(cat.key.to_string(), pick); s.selected = Some(cat.key.to_string()); }
                            ui.weak(format!("Rough relative strength ×{:.2}", opt.strength));
                        });
                    }
                    ui.weak("Strength numbers are placeholders until the sim models materials; they only scale the demo's stress.");
                }
            }
        }

        heading(ui, "Playback");
        let mut pct = s.slow_mo * 100.0;
        ui.add(egui::Slider::new(&mut pct, 0.2..=100.0).logarithmic(true).text("% of real speed"));
        s.slow_mo = pct / 100.0;
        ui.weak("At real speed the parts strobe, like under a timing light.");
    }

    /// Turns the camera from this frame's touches, advances the model and draws it into `vp` of `target`.
    pub fn render(&mut self, target: &RenderTarget, vp: Viewport, events: &mut [Event], s: &mut SceneUi, state: &SimState, dt: f32, time: f32, over_gui: bool) {
        self.camera.set_viewport(vp);
        let aspect = vp.width as f32 / vp.height as f32;
        if let Some(i) = s.fly_to.take() {
            let v = &VIEWS[i.min(VIEWS.len() - 1)];
            self.view_anim = Some((self.camera.position(), view_pos(v, aspect), self.camera.target(), v.target, 0.0));
        }
        if let Some((from, to, from_t, to_t, k)) = self.view_anim.as_mut() {
            *k = (*k + dt / 0.6).min(1.0);
            let e = 1.0 - (1.0 - *k).powi(3);
            let tgt = *from_t + (*to_t - *from_t) * e;
            self.camera.set_view(*from + (*to - *from) * e, tgt, vec3(0.0, 1.0, 0.0));
            if *k >= 1.0 {
                self.control = OrbitControl::new(tgt, 1.0, 30.0);
                self.view_anim = None;
            }
        }
        // A press and release without much movement in the view is a tap.
        let mut tap: Option<PhysicalPoint> = None;
        for ev in events.iter() {
            match ev {
                Event::MousePress { position, handled: false, .. } => self.press_at = Some((*position, time)),
                Event::MouseRelease { position, handled: false, .. } => {
                    if let Some((p, t)) = self.press_at.take() {
                        if (p.x - position.x).abs() + (p.y - position.y).abs() < 24.0 && time - t < 0.5 { tap = Some(*position); }
                    }
                }
                _ => {}
            }
        }
        if !over_gui {
            self.control.handle_events(&mut self.camera, events);
        }

        let lookup = material_lookup(state, &s.choices);
        let g = state.geometry();
        if g != self.engine.geometry {
            set_geometry(g);
            self.engine = Engine::new(&self.ctx);
        }
        self.engine.update(state, dt, s.slow_mo, s.mode, s.castings, &lookup, s.selected.as_deref(), time);

        if let (Some(m), false) = (tap, over_gui) {
            let inside = m.x >= vp.x as f32 && m.y >= vp.y as f32 && m.x <= (vp.x + vp.width as i32) as f32 && m.y <= (vp.y + vp.height as i32) as f32;
            if inside {
                let ids = self.engine.pickable();
                let geoms: Vec<&Mesh> = ids.iter().map(|&i| &self.engine.parts[i].gm.geometry).collect();
                let hit = pick(&self.ctx, &self.camera, m, geoms, Cull::None).ok().flatten().and_then(|h| ids.get(h.geometry_id as usize).copied());
                self.tapped = hit.map(|i| {
                    let p = &self.engine.parts[i];
                    let mut text = p.label.clone();
                    if let Some(k) = p.key {
                        if s.mode == Mode::Materials {
                            if let Some((name, _)) = lookup(k) { text += &format!(" · {name}"); }
                        } else {
                            let v = state.stress_of(k, p.index);
                            if v > 0.0 { text += &format!(" · {:.0}% of limit", v * 100.0); }
                        }
                        if let (Some(i), true) = (p.index, matches!(k, "pistons" | "rods" | "valves")) {
                            text += &format!(" · {} stroke", stroke_name(cycle_angle(i + 1, self.engine.crank)));
                        }
                    }
                    (text, time)
                });
                if s.mode == Mode::Materials {
                    s.selected = hit.and_then(|i| self.engine.parts[i].key).map(|k| k.to_string());
                }
            }
        }

        target
            .clear_partially(vp.into(), ClearState::color_and_depth(0.07, 0.08, 0.09, 1.0, 1.0))
            .render(&self.camera, self.engine.objects(), &[&self.ambient, &self.sun, &self.fill]);
    }
}
