//! The phone layout of the desktop app's panels. Every page is the desktop's own tab (the parent module is
//! windows/src/gui.rs), so the phone has every feature the desktop has; this file only decides how they're
//! reached on a small touch screen: a compact dash, and a menu of big tiles that each open one tab as a sheet.

use super::*;

/// One page of the menu.
#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    /// The grid of tiles.
    Home,
    /// One of the desktop's tabs, by its index in `TABS`.
    Tab(usize),
    /// The 3D view's modes, camera presets and readouts (drawn by src/app.rs, which owns the view).
    View,
}

/// A tile's title and the line under it.
pub fn tiles() -> Vec<(Page, &'static str, &'static str)> {
    let mut v: Vec<(Page, &'static str, &'static str)> = TABS
        .iter()
        .enumerate()
        .map(|(i, (t, label))| {
            let (title, sub) = match t {
                Tab::Drive => (*label, "Pedals, gears, 0–60 and quarter mile"),
                Tab::Tune => (*label, "Fuel, ignition, cam, boost; paste from FlashPro"),
                Tab::Settings => (*label, "Engine, car and conditions"),
                Tab::Parts => (*label, "Turbo, injectors, internals, materials"),
                Tab::Internals => (*label, "Rods, pistons, bearings and their limits"),
                Tab::Cooling => (*label, "Radiator, fans, oil and turbo cooling"),
                Tab::Wheel => ("Controller", "Bluetooth gamepad buttons and triggers"),
                Tab::Log => (*label, "Record and save datalogs"),
            };
            (Page::Tab(i), title, sub)
        })
        .collect();
    v.push((Page::View, "3D view", "View modes, camera and engine readouts"));
    v
}

pub fn title(page: Page) -> &'static str {
    match page {
        Page::Home => "Menu",
        Page::View => "3D view",
        p => tiles().into_iter().find(|t| t.0 == p).map(|t| t.1).unwrap_or("Menu"),
    }
}

impl Gui {
    /// The desktop's hardware-keyboard driving keys, for a phone or tablet with a keyboard attached.
    pub fn hardware_keys(&mut self, ctx: &egui::Context, sh: &mut Shared) {
        self.keyboard(ctx, sh);
    }

    /// The top bar: rpm, gear, speed and the warning lamps that matter while driving.
    pub fn compact_dash(&mut self, ui: &mut egui::Ui, sh: &mut Shared) {
        let sim = &sh.sim;
        let e = &sim.eng;
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{:>5.0}", e.rpm.max(0.0))).size(24.0).monospace().strong());
            ui.label(RichText::new("rpm").small().color(Color32::from_gray(150)));
            let gear = match sim.car.gear { 0 => "N".into(), g if g < 0 => "R".into(), g => g.to_string() };
            ui.label(RichText::new(gear).size(24.0).strong().color(Color32::from_rgb(0xff, 0xd0, 0x40)));
            ui.label(RichText::new(format!("{:.0} mph", sim.car.speed_ms * 2.23694)).monospace());
            if e.map > 101.3 {
                ui.label(RichText::new(format!("{:.1} psi", (e.map - 101.3) * 0.145)).monospace());
            } else {
                ui.label(RichText::new(format!("{:.0} kPa", e.map)).monospace());
            }
            ui.label(RichText::new(if e.lambda > 5.0 { "AFR cut".into() } else { format!("AFR {:.1}", e.afr) }).monospace());
            lamp(ui, "VTEC", e.high_cam, Color32::from_rgb(0xd0, 0x3a, 0x2e));
            if e.knocking { lamp(ui, "KNOCK", true, Color32::from_rgb(0xc8, 0x6a, 0x10)); }
            if e.rev_cut { lamp(ui, "REV LIM", true, Color32::from_rgb(0x80, 0x30, 0xa0)); }
            if e.thermal.boiling { lamp(ui, "HOT", true, Color32::from_rgb(0xb0, 0x10, 0x10)); }
            if let Some(f) = sim.internals.failed {
                ui.label(RichText::new(format!(" {} FAILED ", f.to_uppercase())).strong().small().color(Color32::WHITE).background_color(Color32::from_rgb(0xb0, 0x10, 0x10)));
            } else if !sim.running {
                ui.label(RichText::new(" ENGINE OFF ").strong().small().color(Color32::WHITE).background_color(Color32::from_gray(70)));
            }
        });
    }

    /// One desktop tab as a phone page. The Drive page starts with the desktop's full dash.
    pub fn page(&mut self, ui: &mut egui::Ui, sh: &mut Shared, index: usize) {
        let Some((tab, _)) = TABS.get(index) else { return };
        let changed = match tab {
            Tab::Drive => {
                egui::ScrollArea::horizontal().id_salt("full-dash").show(ui, |ui| self.dash(ui, sh));
                ui.separator();
                self.drive(ui, sh);
                ui.label(RichText::new("On screen: right pedal throttle, left pedal brake (press higher for more), the narrow pedal is the clutch. The two buttons beside the gas shift up and down. A gamepad's triggers and bumpers work too.").small().color(Color32::from_gray(150)));
                false
            }
            Tab::Tune => self.tables(ui, sh),
            Tab::Settings => self.settings(ui, sh),
            Tab::Parts => self.parts(ui, sh),
            Tab::Internals => { self.internals(ui, sh); false }
            Tab::Cooling => self.cooling(ui, sh),
            Tab::Wheel => {
                ui.label(RichText::new("Pair a Bluetooth gamepad in your phone's settings, then press its buttons here. Triggers work as on/off pedals.").small());
                self.wheel(ui, sh);
                false
            }
            Tab::Log => { self.log(ui, sh); false }
        };
        if changed {
            sh.apply_tune();
        }
        if matches!(tab, Tab::Tune | Tab::Log) && !crate::platform().files_hint.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(&crate::platform().files_hint).small().color(Color32::from_gray(150)));
        }
    }
}
