//! One frame of the phone app: the dash along the top, the 3D engine filling the screen with the pedals and
//! shift buttons over it, the menu sheet when it's open, and the on-screen keyboard when a box has focus.
//! In landscape the sheet slides in from the left so the engine stays in view while you tune; in portrait it
//! sits under the dash.

use crate::gui::phone::{self, Page};
use crate::input::{Control, Input, Zone};
use crate::keyboard::Keyboard;
use crate::scene::{Scene, SceneUi};
use crate::Shared;
use honda_sim_viewer::SimState;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use three_d::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind};
use three_d::*;

/// What lives on the GPU; dropped and rebuilt when the phone takes the screen away (app sent to the background).
struct Gpu {
    egui: GUI,
    scene: Scene,
}

pub struct App {
    shared: Arc<Mutex<Shared>>,
    latest: crate::Latest,
    gui: crate::gui::Gui,
    pub input: Input,
    keyboard: Keyboard,
    scene_ui: SceneUi,
    page: Option<Page>,
    started: Instant,
    last: Instant,
    gpu: Option<Gpu>,
    last_state: SimState,
    styled: bool,
    /// The box that had keyboard focus last frame.
    focus_seen: Option<egui::Id>,
}

const ACCENT: Color32 = Color32::from_rgb(0x2f, 0x6f, 0xd0);

impl App {
    pub fn new() -> App {
        let (shared, latest) = crate::start_sim();
        let gui = crate::gui::Gui::new(shared.clone());
        App {
            shared,
            latest,
            gui,
            input: Input::default(),
            keyboard: Keyboard::default(),
            scene_ui: SceneUi::default(),
            page: None,
            started: Instant::now(),
            last: Instant::now(),
            gpu: None,
            last_state: SimState::default(),
            styled: false,
            focus_seen: None,
        }
    }

    /// The window has a graphics context again (first start, or back from the background).
    pub fn resume(&mut self, ctx: &Context) {
        self.gpu = Some(Gpu { egui: GUI::new(ctx), scene: Scene::new(ctx) });
        self.styled = false;
    }

    /// The graphics context is going away.
    pub fn suspend(&mut self) {
        self.gpu = None;
    }

    /// Draws one frame into `target`, `width` × `height` physical pixels, `dpr` pixels per point.
    pub fn frame(&mut self, target: &RenderTarget, width: u32, height: u32, dpr: f32) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let time = (now - self.started).as_secs_f32();

        // Touch targets a little bigger than the desktop's on phones; tablets get more room still.
        let short = width.min(height) as f32 / dpr;
        let scale = dpr * (short / 360.0).clamp(1.0, 1.4);
        self.input.scale = scale;
        self.input.height = height as f32;
        let (w, h) = (width as f32 / scale, height as f32 / scale);
        let landscape = w > h;

        // The drive controls and keyboard from touches since the last frame.
        let pedals = self.input.pedals();
        let fired: Vec<&'static str> = self.input.fired.drain(..).collect();
        let keys: Vec<&'static str> = self.input.keys.drain(..).collect();
        if std::mem::take(&mut self.input.back) {
            self.page = match self.page { Some(Page::Home) | None => None, Some(_) => Some(Page::Home) };
        }
        let paste = || crate::platform().clipboard.as_ref().and_then(|c| c());
        for k in keys {
            if k == "done" {
                let ectx = gpu.egui.context().clone();
                if let Some(id) = ectx.memory(|m| m.focused()) {
                    ectx.memory_mut(|m| m.surrender_focus(id));
                }
            } else {
                let evs = self.keyboard.press(k, &paste);
                self.input.events.extend(evs);
            }
        }
        {
            let mut sh = self.shared.lock().unwrap();
            sh.touch = pedals;
            sh.events.extend(fired.iter().map(|s| s.to_string()));
        }

        let state = self.latest.lock().unwrap().take();
        if let Some(s) = state {
            self.last_state = s;
        }
        let state = self.last_state.clone();

        let mut events = self.input.take_events();
        let viewport = Viewport::new_at_origo(width, height);
        let mut view_rect = Rect::NOTHING;
        let mut zones: Vec<Zone> = Vec::new();
        let mut over_gui = false;
        let (gui, page, scene_ui, keyboard, input) = (&mut self.gui, &mut self.page, &mut self.scene_ui, &mut self.keyboard, &self.input);
        let scene = &gpu.scene;
        let shared = self.shared.clone();
        let styled = &mut self.styled;
        let focus_seen = &mut self.focus_seen;
        gpu.egui.update(&mut events, time as f64 * 1000.0, viewport, scale, |ui| {
            let ectx = ui.ctx().clone();
            if !*styled {
                style(&ectx);
                *styled = true;
            }
            let mut guard = shared.lock().unwrap();
            let sh = &mut *guard;
            gui.hardware_keys(&ectx, sh);
            let typing = ectx.egui_wants_keyboard_input();

            egui::Panel::top("phone-dash").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let open = page.is_some();
                    let menu = egui::Button::new(RichText::new(if open { "×  Close" } else { "☰ Menu" }).size(17.0).strong())
                        .fill(if open { Color32::from_gray(60) } else { ACCENT })
                        .min_size(egui::vec2(92.0, 40.0));
                    if ui.add(menu).clicked() {
                        *page = if open { None } else { Some(Page::Home) };
                    }
                    egui::ScrollArea::horizontal().id_salt("dash-scroll").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| gui.compact_dash(ui, sh));
                });
            });
            if let Some(p) = *page {
                let sheet = |ui: &mut egui::Ui| {
                    ui.horizontal(|ui| {
                        if p != Page::Home && ui.add(egui::Button::new(RichText::new("‹ Menu").size(16.0)).min_size(egui::vec2(80.0, 36.0))).clicked() {
                            *page = Some(Page::Home);
                        }
                        ui.label(RichText::new(phone::title(p)).size(19.0).strong());
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt(("sheet", phone::title(p))).auto_shrink([false, false]).show(ui, |ui| {
                        match p {
                            Page::Home => home(ui, page, w),
                            Page::Tab(i) => gui.page(ui, sh, i),
                            Page::View => scene.panel(ui, scene_ui, &state),
                        }
                        // Bring a box that just got focus into view.
                        let focused = ectx.memory(|m| m.focused());
                        if typing && focused != *focus_seen {
                            if let Some(r) = focused.and_then(|id| ectx.read_response(id)) {
                                ui.scroll_to_rect(r.rect, Some(egui::Align::Center));
                            }
                        }
                        *focus_seen = focused;
                    });
                };
                let frame = egui::Frame::side_top_panel(&ectx.global_style()).fill(Color32::from_rgb(0x16, 0x18, 0x1b)).inner_margin(10.0);
                if landscape {
                    egui::Panel::left("sheet").exact_size((w * 0.6).clamp(300.0, 620.0)).resizable(false).frame(frame).show_inside(ui, sheet);
                } else {
                    egui::Panel::top("sheet").exact_size(h * 0.55).resizable(false).frame(frame).show_inside(ui, sheet);
                }
            }
            view_rect = ui.available_rect_before_wrap();
            over_gui = ectx.is_pointer_over_egui();

            if typing {
                // The keyboard takes the engine's side of the screen, so the page being typed into stays in view.
                zones = keyboard.show(&ectx, view_rect, &|c| input.held(c));
            } else {
                zones = drive_controls(&ectx, view_rect, &|c| input.held(c), sh);
            }
            // A tapped part's name, for a few seconds.
            if let Some((text, at)) = &scene.tapped {
                if time - at < 4.0 && !text.is_empty() {
                    egui::Area::new(egui::Id::new("tapped")).fixed_pos(view_rect.center_top() + egui::vec2(-140.0, 8.0)).interactable(false).show(&ectx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| { ui.set_max_width(280.0); ui.label(text); });
                    });
                }
            }
        });
        self.input.zones = zones;

        let vp = Viewport {
            x: (view_rect.min.x * scale) as i32,
            y: ((h - view_rect.max.y) * scale) as i32,
            width: ((view_rect.width() * scale) as u32).max(1),
            height: ((view_rect.height() * scale) as u32).max(1),
        };
        target.clear(ClearState::color_and_depth(0.07, 0.08, 0.09, 1.0, 1.0));
        gpu.scene.render(target, vp, &mut events, &mut self.scene_ui, &state, dt, time, over_gui);
        let _ = target.write(|| gpu.egui.render());
    }
}

fn style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.all_styles_mut(|s| {
        s.spacing.interact_size = egui::vec2(44.0, 32.0);
        s.spacing.button_padding = egui::vec2(12.0, 7.0);
        s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        s.spacing.icon_width = 22.0;
        s.spacing.icon_width_inner = 12.0;
        s.spacing.combo_height = 320.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.spacing.scroll.bar_width = 8.0;
        s.spacing.slider_width = 180.0;
        s.interaction.tooltip_delay = 0.4;
    });
}

/// The menu's tiles.
fn home(ui: &mut egui::Ui, page: &mut Option<Page>, width: f32) {
    let cols = if ui.available_width() > 640.0 { 2 } else { 1 };
    let _ = width;
    let tile_w = (ui.available_width() - 10.0 * (cols as f32 - 1.0)) / cols as f32;
    egui::Grid::new("tiles").num_columns(cols).spacing([10.0, 10.0]).show(ui, |ui| {
        for (i, (p, title, sub)) in phone::tiles().into_iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(tile_w, 64.0), egui::Sense::click());
            let fill = if resp.is_pointer_button_down_on() { Color32::from_gray(70) } else { Color32::from_gray(40) };
            let painter = ui.painter();
            painter.rect(rect, 10.0, fill, Stroke::new(1.0_f32, Color32::from_gray(60)), StrokeKind::Inside);
            painter.text(rect.left_top() + egui::vec2(14.0, 12.0), Align2::LEFT_TOP, title, FontId::proportional(18.0), Color32::WHITE);
            painter.with_clip_rect(rect.shrink2(egui::vec2(10.0, 0.0)).with_max_x(rect.max.x - 34.0)).text(rect.left_top() + egui::vec2(14.0, 38.0), Align2::LEFT_TOP, sub, FontId::proportional(12.5), Color32::from_gray(160));
            painter.text(rect.right_center() - egui::vec2(14.0, 0.0), Align2::RIGHT_CENTER, "›", FontId::proportional(22.0), Color32::from_gray(140));
            if resp.clicked() {
                *page = Some(p);
            }
            if (i + 1) % cols == 0 {
                ui.end_row();
            }
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Same sim, tables and tune files as the Windows app. Tunes saved here are the same JSON.").small().color(Color32::from_gray(140)));
}

/// Pedals and shift buttons over the 3D view. They're painted, not egui widgets, because several fingers work
/// them at once (src/input.rs routes each finger by these rectangles).
fn drive_controls(ctx: &egui::Context, view: Rect, held: &dyn Fn(Control) -> bool, sh: &Shared) -> Vec<Zone> {
    let m = 12.0;
    let pedal_h = (view.height() * 0.62).clamp(110.0, 240.0);
    let pedal_w = (view.width() * 0.12).clamp(62.0, 96.0);
    let bottom = view.max.y - m;
    let mut zones = Vec::new();
    let mut add = |r: Rect, c: Control| zones.push(Zone { rect: r, control: c });

    let throttle = Rect::from_min_max(Pos2::new(view.max.x - m - pedal_w, bottom - pedal_h), Pos2::new(view.max.x - m, bottom));
    let clutch = Rect::from_min_max(Pos2::new(view.min.x + m, bottom - pedal_h * 0.8), Pos2::new(view.min.x + m + pedal_w * 0.7, bottom));
    let brake = Rect::from_min_max(Pos2::new(clutch.max.x + 10.0, bottom - pedal_h), Pos2::new(clutch.max.x + 10.0 + pedal_w, bottom));
    let bw = pedal_w.min(80.0);
    let bh = (pedal_h - 10.0) / 2.0;
    let up = Rect::from_min_max(Pos2::new(throttle.min.x - 10.0 - bw, bottom - pedal_h), Pos2::new(throttle.min.x - 10.0, bottom - pedal_h + bh));
    let down = Rect::from_min_max(Pos2::new(up.min.x, up.max.y + 10.0), Pos2::new(up.max.x, bottom));
    let small = egui::vec2(58.0, 40.0);
    let start = Rect::from_min_size(Pos2::new(view.max.x - m - small.x, view.min.y + m), small);
    let neutral = Rect::from_min_size(start.min - egui::vec2(small.x + 8.0, 0.0), small);
    let reverse = Rect::from_min_size(neutral.min - egui::vec2(small.x + 8.0, 0.0), small);
    // Not enough room (a very short view in landscape with the menu open): just the pedals.
    let roomy = up.min.x > brake.max.x + 10.0 && reverse.min.x > view.min.x + m && view.height() > pedal_h + small.y + 3.0 * m;

    let painter = ctx.layer_painter(egui::LayerId::background());
    let pedal = |r: Rect, label: &str, v: f64, on: bool, color: Color32| {
        painter.rect(r, 10.0, Color32::from_rgba_unmultiplied(30, 33, 38, 190), Stroke::new(if on { 2.0_f32 } else { 1.0 }, if on { color } else { Color32::from_gray(90) }), StrokeKind::Inside);
        let mut f = r.shrink(4.0);
        f.min.y = f.max.y - f.height() * v.clamp(0.0, 1.0) as f32;
        if v > 0.0 { painter.rect_filled(f, 7.0, color.gamma_multiply(0.75)); }
        painter.text(Pos2::new(r.center().x, r.max.y - 14.0), Align2::CENTER_CENTER, label, FontId::proportional(13.0), Color32::WHITE);
    };
    pedal(throttle, "GAS", sh.throttle, held(Control::Throttle), Color32::from_rgb(0x3f, 0xcf, 0x7f));
    pedal(brake, "BRAKE", sh.brake, held(Control::Brake), Color32::from_rgb(0xff, 0x5a, 0x4e));
    pedal(clutch, "CLUTCH", sh.clutch.unwrap_or(0.0), held(Control::Clutch), Color32::from_rgb(0x6e, 0xc8, 0xff));
    add(throttle, Control::Throttle);
    add(brake, Control::Brake);
    add(clutch, Control::Clutch);
    let button = |r: Rect, label: &str, on: bool| {
        painter.rect(r, 10.0, if on { ACCENT } else { Color32::from_rgba_unmultiplied(30, 33, 38, 200) }, Stroke::new(1.0_f32, Color32::from_gray(100)), StrokeKind::Inside);
        painter.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(20.0), Color32::WHITE);
    };
    if roomy {
        for (r, ev, dir) in [(up, "shiftUp", -1.0), (down, "shiftDown", 1.0)] {
            button(r, "", held(Control::Button(ev)));
            let (c, s) = (r.center() - egui::vec2(0.0, 8.0), 13.0);
            let tri = vec![c + egui::vec2(-s, -dir * s * 0.6), c + egui::vec2(s, -dir * s * 0.6), c + egui::vec2(0.0, dir * s * 0.7)];
            painter.add(egui::Shape::convex_polygon(tri, Color32::WHITE, Stroke::NONE));
            painter.text(Pos2::new(r.center().x, r.max.y - 14.0), Align2::CENTER_CENTER, if dir < 0.0 { "SHIFT UP" } else { "SHIFT DOWN" }, FontId::proportional(11.0), Color32::from_gray(200));
            add(r, Control::Button(ev));
        }
        for (r, label, ev) in [(start, "START", "start"), (neutral, "N", "neutral"), (reverse, "R", "reverse")] {
            painter.rect(r, 8.0, if held(Control::Button(ev)) { ACCENT } else { Color32::from_rgba_unmultiplied(30, 33, 38, 200) }, Stroke::new(1.0_f32, Color32::from_gray(100)), StrokeKind::Inside);
            painter.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(14.0), Color32::WHITE);
            add(r, Control::Button(ev));
        }
    }
    zones
}
