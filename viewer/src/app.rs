//! The viewer window: a 3D view of the K20Z3 with modes for the whole engine, internals, block, airflow, stress
//! and materials, and a side panel. It shows whatever state arrives through its `Link`, and runs a built-in demo
//! while nothing is arriving.

use crate::engine::{material_lookup, Castings, Engine, Mode};
use crate::fake_sim::{FakeSim, ManualInput};
use crate::kinematics::{cycle_angle, set_geometry, stroke_name, Geometry, Side, VTC_MAX_DEG};
use crate::link::Link;
use crate::materials;
use crate::state::SimState;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use three_d::*;

const TARGET: Vec3 = Vec3 { x: 0.0, y: 1.3, z: 0.0 };

struct View {
    label: &'static str,
    dir: [f32; 3],
    dist: f32,
    /// What the camera orbits, in scene units (1 = 100 mm).
    target: Vec3,
}

const VIEWS: [View; 6] = [
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

#[derive(PartialEq, Clone, Copy)]
enum Source {
    Auto,
    Demo,
}

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
        ui.add_sized([110.0, 18.0], egui::Label::new(label));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(110.0, 8.0), egui::Sense::hover());
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
}

/// How the viewer window starts.
pub struct Options {
    /// Where state comes from: `Link::in_process()` when built into the sim, `Link::listen()` when standalone.
    pub link: Link,
    /// The view it opens on.
    pub mode: Mode,
    pub title: String,
    /// Print any pipe that passes through another part, then exit (a model check, run with `--check-clearance`).
    pub check_clearance: bool,
    /// The demo's starting build: (turbo, intercooler).
    pub demo_build: (bool, bool),
    /// The demo starts on a K24A block.
    pub demo_k24: bool,
    /// Starting camera: 0 exhaust side, 1 intake side, 2 front, 3 side, 4 top, 5 valvetrain.
    pub view: usize,
    /// Lets a host program (the native sim) draw its own egui panels around the 3D view. Called every frame,
    /// before the viewer's own panels, with the root `Ui` and the state being shown. Panels it adds with
    /// `egui::Panel::top/left/right/bottom(..).show_inside(ui, ..)` take their space first; the 3D view fills
    /// whatever is left in the middle.
    pub host_panels: Option<Box<dyn FnMut(&mut egui::Ui, &SimState)>>,
    /// Show the viewer's own mode tabs (top) and side panel. A host that draws its own can turn them off.
    pub builtin_panels: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { link: Link::in_process(), mode: Mode::Full, title: "Honda Sim · K20Z3 engine viewer".to_string(), check_clearance: false, demo_build: (true, true), demo_k24: false, view: 0, host_panels: None, builtin_panels: true }
    }
}

/// Opens the viewer window and runs until it is closed. Must be called on the main thread (a windowing rule on
/// Windows and macOS), so a sim built into the same program runs its own loop on another thread and calls
/// `link.send(state)` from there.
pub fn run(mut opts: Options) {
    let mut host_panels = opts.host_panels.take();
    let builtin_panels = opts.builtin_panels;
    let window = Window::new(WindowSettings { title: opts.title.clone(), min_size: (900, 600), initial_size: Some((1400, 860)), ..Default::default() }).unwrap();
    let ctx = window.gl();
    let mut camera = Camera::new_perspective(window.viewport(), view_pos(&VIEWS[0], 1.6), TARGET, vec3(0.0, 1.0, 0.0), degrees(38.0), 0.05, 100.0);
    let mut control = OrbitControl::new(TARGET, 1.5, 30.0);
    let mut gui = three_d::GUI::new(&ctx);
    if opts.check_clearance {
        // Every build on every block.
        let mut problems = Vec::new();
        for (name, g) in [("K20Z3", Geometry::K20Z3), ("K24A", Geometry::K24A)] {
            set_geometry(g);
            problems.extend(Engine::new(&ctx).clearance_problems().into_iter().map(|p| format!("{name} {p}")));
        }
        set_geometry(Geometry::K20Z3);
        for p in &problems { println!("{p}"); }
        println!("{} clearance problem(s)", problems.len());
        std::process::exit(if problems.is_empty() { 0 } else { 1 });
    }
    let mut engine = Engine::new(&ctx);
    let ambient = AmbientLight::new(&ctx, 0.55, Srgba::new(225, 232, 255, 255));
    let sun = DirectionalLight::new(&ctx, 2.2, Srgba::WHITE, vec3(-0.4, -0.8, -0.5));
    let fill = DirectionalLight::new(&ctx, 0.9, Srgba::new(200, 210, 255, 255), vec3(0.6, -0.3, 0.7));
    let link = opts.link.clone();
    let mut mode = opts.mode;
    let mut castings = Castings::SeeThrough;
    let mut slow_mo: f32 = 0.02;
    let mut source = Source::Auto;
    let mut demo = FakeSim::new();
    (demo.turbo, demo.intercooler) = opts.demo_build;
    demo.k24 = opts.demo_k24;
    let mut manual = false;
    let mut manual_in = ManualInput { rpm: 3000.0, throttle: 0.4, boost_kpa: 110.0 };
    let mut choices: HashMap<String, String> = HashMap::new();
    let mut selected: Option<String> = None;
    let mut mouse: Option<PhysicalPoint> = None;
    let mut hovered: Option<usize> = None;
    let mut view_anim: Option<(Vec3, Vec3, Vec3, Vec3, f32)> = None;
    let mut frame_no: u64 = 0;
    let started = Instant::now();

    window.render_loop(move |mut frame_input| {
        let dt = (frame_input.elapsed_time as f32 / 1000.0).min(0.1);
        let time = started.elapsed().as_secs_f32();
        frame_no += 1;

        // Where the sim state comes from this frame.
        let (connected, listening, link_error, fresh) = {
            let l = link.0.lock().unwrap();
            let fresh = l.latest.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(1500)).map(|(s, _)| s.clone());
            (l.connected, l.listening, l.error.clone(), fresh)
        };
        let live = source == Source::Auto && fresh.is_some();
        let state: SimState = if live {
            fresh.unwrap()
        } else {
            let ch = choices.clone();
            demo.step(dt, if manual { Some(&manual_in) } else { None }, &|k: &str| {
                let Some(cat) = materials::catalog(k) else { return 1.0 };
                let id = ch.get(k).map(|s| s.as_str()).unwrap_or(cat.options[0].id);
                cat.options.iter().find(|o| o.id == id).map(|o| o.strength).unwrap_or(1.0)
            })
        };

        for ev in frame_input.events.iter() {
            if let Event::MouseMotion { position, .. } = ev {
                mouse = Some(*position);
            }
            if let Event::MouseLeave = ev {
                mouse = None;
            }
        }

        // ---------------- GUI ----------------
        let dpr = frame_input.device_pixel_ratio;
        let screen = frame_input.viewport;
        let mut view_rect = egui::Rect::NOTHING;
        let mut clicked_view: Option<usize> = None;
        let mut pointer_over_gui = false;
        let crank = engine.crank;
        let (lifts, pins, specs) = (engine.lifts, engine.pins, engine.specs);
        let hovered_part = hovered.map(|i| (engine.parts[i].label.clone(), engine.parts[i].key, engine.parts[i].index));
        gui.update(&mut frame_input.events, frame_input.accumulated_time, screen, dpr, |ui| {
            let ectx = ui.ctx().clone();
            ectx.set_visuals(egui::Visuals::dark());
            if let Some(host) = host_panels.as_mut() {
                host(ui, &state);
            }
            if builtin_panels {
            egui::Panel::top("modes").show_inside(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for m in Mode::ALL {
                        if ui.selectable_label(mode == m, m.label()).clicked() {
                            mode = m;
                            if m != Mode::Materials { selected = None; }
                        }
                    }
                    ui.separator();
                    for (i, v) in VIEWS.iter().enumerate() {
                        if ui.small_button(v.label).clicked() { clicked_view = Some(i); }
                    }
                });
            });
            egui::Panel::right("panel").default_size(320.0).resizable(false).show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                    heading(ui, mode.label());
                    match mode {
                        Mode::Full => {
                            ui.label("Everything at once. Hover a part for its name and load.");
                            ui.horizontal(|ui| {
                                ui.label("Block and head");
                                ui.selectable_value(&mut castings, Castings::SeeThrough, "See-through");
                                ui.selectable_value(&mut castings, Castings::Solid, "Solid");
                                ui.selectable_value(&mut castings, Castings::Hidden, "Hidden");
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
                            let tp = crate::engine::Temps::of(&state);
                            let swatch_row = |ui: &mut egui::Ui, label: &str, c: [u8; 3], text: String| {
                                ui.horizontal(|ui| {
                                    swatch(ui, c);
                                    ui.add_sized([120.0, 18.0], egui::Label::new(label));
                                    ui.monospace(text);
                                });
                            };
                            swatch_row(ui, "Coolant out", crate::engine::coolant_rgb(tp.hot), format!("{:.0} °C", tp.hot));
                            swatch_row(ui, "From radiator", crate::engine::coolant_rgb(tp.cold), format!("{:.0} °C{}", tp.cold, if state.coolant_out_c.is_none() { " (est.)" } else { "" }));
                            swatch_row(ui, "Oil", crate::engine::oil_rgb(tp.oil), format!("{:.0} °C", tp.oil));
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
                                ui.label("The materials the sim's internals model uses. Click a part in the view to find it here.");
                                let mut keys: Vec<_> = pm.keys().cloned().collect();
                                keys.sort();
                                for k in keys {
                                    let m = &pm[&k];
                                    let fam = materials::family(&m.family);
                                    let frame = egui::Frame::group(ui.style()).stroke(if selected.as_deref() == Some(k.as_str()) { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0xff, 0x3b, 0x4f)) } else { egui::Stroke::NONE });
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| { if let Some(f) = fam { swatch(ui, f.color); } ui.strong(if m.part.is_empty() { &k } else { &m.part }); });
                                        ui.label(&m.label);
                                        if !m.detail.is_empty() { ui.weak(&m.detail); }
                                    });
                                }
                            } else {
                                ui.label("Colour shows each part's material. Pick one per part here or click a part in the view.");
                                ui.horizontal_wrapped(|ui| {
                                    for f in materials::FAMILIES { swatch(ui, f.color); ui.label(f.label); }
                                });
                                for cat in materials::CATALOG {
                                    let cur = choices.get(cat.key).cloned().unwrap_or(cat.options[0].id.to_string());
                                    let opt = cat.options.iter().find(|o| o.id == cur).unwrap_or(&cat.options[0]);
                                    let frame = egui::Frame::group(ui.style()).stroke(if selected.as_deref() == Some(cat.key) { egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(0xff, 0x3b, 0x4f)) } else { egui::Stroke::NONE });
                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| { if let Some(f) = materials::family(opt.family) { swatch(ui, f.color); } ui.strong(cat.label); });
                                        let mut pick = cur.clone();
                                        egui::ComboBox::from_id_salt(cat.key).selected_text(opt.label).width(260.0).show_ui(ui, |ui| {
                                            for o in cat.options { ui.selectable_value(&mut pick, o.id.to_string(), o.label); }
                                        });
                                        if pick != cur { choices.insert(cat.key.to_string(), pick); selected = Some(cat.key.to_string()); }
                                        ui.weak(format!("Rough relative strength ×{:.2}", opt.strength));
                                    });
                                }
                                ui.weak("Strength numbers are placeholders until the sim models materials; they only scale the demo's stress.");
                            }
                        }
                    }

                    ui.separator();
                    heading(ui, "Data");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut source, Source::Auto, "Live sim");
                        ui.selectable_value(&mut source, Source::Demo, "Demo");
                    });
                    if let Some(err) = &link_error {
                        ui.colored_label(egui::Color32::from_rgb(0xff, 0x5a, 0x4e), err);
                    } else if live {
                        ui.colored_label(egui::Color32::from_rgb(0x3f, 0xcf, 0x7f), "Following the sim.");
                    } else if source == Source::Auto {
                        ui.weak(if connected { "Sim connected, waiting for data. Showing the demo." } else if listening { "Waiting for the sim on port 8765. Showing the demo until it connects." } else { "Waiting for the sim. Showing the demo." });
                    }
                    if !live {
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut manual, false, "Scripted pull");
                            ui.selectable_value(&mut manual, true, "Sliders");
                        });
                        if manual {
                            ui.add(egui::Slider::new(&mut manual_in.rpm, 800.0..=8200.0).text("rpm"));
                            ui.add(egui::Slider::new(&mut manual_in.throttle, 0.0..=1.0).text("throttle"));
                            ui.add(egui::Slider::new(&mut manual_in.boost_kpa, 0.0..=200.0).text("boost kPa"));
                        }
                        if ui.button("Make it knock").clicked() { demo.knock_until = demo.t + 1.2; }
                        ui.horizontal(|ui| {
                            ui.label("Build");
                            let mut build = if !demo.turbo { 0 } else if demo.intercooler { 1 } else { 2 };
                            ui.selectable_value(&mut build, 0, "N/A");
                            ui.selectable_value(&mut build, 1, "Turbo + intercooler");
                            ui.selectable_value(&mut build, 2, "Turbo, no IC");
                            demo.turbo = build != 0;
                            demo.intercooler = build != 2;
                        });
                        ui.horizontal(|ui| {
                            ui.label("Block");
                            ui.selectable_value(&mut demo.k24, false, "K20Z3");
                            ui.selectable_value(&mut demo.k24, true, "K24A");
                        });
                    }
                    heading(ui, "Playback");
                    let mut pct = slow_mo * 100.0;
                    ui.add(egui::Slider::new(&mut pct, 0.2..=100.0).logarithmic(true).text("% of real speed"));
                    slow_mo = pct / 100.0;
                    ui.weak("At real speed the parts strobe, like under a timing light.");
                });
            });
            }
            view_rect = ui.available_rect_before_wrap();
            pointer_over_gui = ectx.is_pointer_over_egui();

            // Hover label for the part under the mouse.
            if let (Some((label, key, index)), Some(pos)) = (&hovered_part, ectx.pointer_hover_pos()) {
                if view_rect.contains(pos) {
                    let mut text = label.clone();
                    if let Some(k) = key {
                        if mode == Mode::Materials {
                            if let Some((name, _)) = material_lookup(&state, &choices)(k) { text += &format!(" · {name}"); }
                        } else {
                            let v = state.stress_of(k, *index);
                            if v > 0.0 { text += &format!(" · {:.0}% of limit", v * 100.0); }
                        }
                        if let (Some(i), true) = (index, matches!(*k, "pistons" | "rods" | "valves")) {
                            text += &format!(" · {} stroke", stroke_name(cycle_angle(i + 1, crank)));
                        }
                    }
                    egui::Area::new(egui::Id::new("hover")).fixed_pos(pos + egui::vec2(14.0, -28.0)).interactable(false).show(&ectx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| { ui.label(text); });
                    });
                }
            }
        });

        // 3D viewport = what the panels left over.
        let vp = Viewport {
            x: (view_rect.min.x * dpr) as i32,
            y: ((screen.height as f32 / dpr - view_rect.max.y) * dpr) as i32,
            width: ((view_rect.width() * dpr) as u32).max(1),
            height: ((view_rect.height() * dpr) as u32).max(1),
        };
        camera.set_viewport(vp);
        if frame_no == 1 {
            let v = &VIEWS[opts.view.min(VIEWS.len() - 1)];
            camera.set_view(view_pos(v, vp.width as f32 / vp.height as f32), v.target, vec3(0.0, 1.0, 0.0));
            control = OrbitControl::new(v.target, 1.0, 30.0);
        }
        if let Some(i) = clicked_view {
            let v = &VIEWS[i];
            view_anim = Some((camera.position(), view_pos(v, vp.width as f32 / vp.height as f32), camera.target(), v.target, 0.0));
        }
        if let Some((from, to, from_t, to_t, k)) = view_anim.as_mut() {
            *k = (*k + dt / 0.6).min(1.0);
            let e = 1.0 - (1.0 - *k).powi(3);
            let target = *from_t + (*to_t - *from_t) * e;
            camera.set_view(*from + (*to - *from) * e, target, vec3(0.0, 1.0, 0.0));
            if *k >= 1.0 {
                control = OrbitControl::new(target, 1.0, 30.0);
                view_anim = None;
            }
        }
        control.handle_events(&mut camera, &mut frame_input.events);

        // Clicks in the view pick a part (materials mode).
        let mut click = false;
        for ev in frame_input.events.iter() {
            if let Event::MouseRelease { button: MouseButton::Left, handled: false, .. } = ev { click = true; }
        }

        let lookup_state = state.clone();
        let lookup = material_lookup(&lookup_state, &choices);
        // The sim swapped the block, crank or rods: rebuild the engine to the new dimensions.
        let g = state.geometry();
        if g != engine.geometry {
            set_geometry(g);
            engine = Engine::new(&ctx);
        }
        engine.update(&state, dt, slow_mo, mode, castings, &lookup, selected.as_deref(), time);

        // Hover picking, every other frame.
        if frame_no % 2 == 0 || click {
            hovered = None;
            if let Some(m) = mouse {
                let inside = m.x >= vp.x as f32 && m.y >= vp.y as f32 && m.x <= (vp.x + vp.width as i32) as f32 && m.y <= (vp.y + vp.height as i32) as f32;
                if inside && !pointer_over_gui {
                    let ids = engine.pickable();
                    let geoms: Vec<&Mesh> = ids.iter().map(|&i| &engine.parts[i].gm.geometry).collect();
                    if let Ok(Some(hit)) = pick(&ctx, &camera, m, geoms, Cull::None) {
                        hovered = ids.get(hit.geometry_id as usize).copied();
                    }
                }
            }
        }
        if click && mode == Mode::Materials && !pointer_over_gui {
            selected = hovered.and_then(|i| engine.parts[i].key).map(|k| k.to_string());
        }

        frame_input
            .screen()
            .clear(ClearState::color_and_depth(0.07, 0.08, 0.09, 1.0, 1.0))
            .render(&camera, engine.objects(), &[&ambient, &sun, &fill])
            .write(|| gui.render())
            .unwrap();
        FrameOutput::default()
    });
}
