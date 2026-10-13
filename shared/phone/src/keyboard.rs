//! An on-screen keyboard for typing numbers, tune names and pasted FlashPro tables. It shows whenever a text box
//! or number box has focus. Its keys are touch zones (src/input.rs), not egui buttons, because pressing any egui
//! widget would take the focus away from the box being typed in.

use crate::input::{Control, Zone};
use three_d::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind};
use three_d::{Event, Key, Modifiers};

const DIGITS: [&str; 10] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
const LETTERS: [[&str; 10]; 3] = [
    ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
    ["a", "s", "d", "f", "g", "h", "j", "k", "l", "'"],
    ["z", "x", "c", "v", "b", "n", "m", ",", ".", "-"],
];
const SYMBOLS: [[&str; 10]; 3] = [
    ["-", ".", ",", "+", "/", "(", ")", "%", ":", ";"],
    ["_", "#", "@", "&", "*", "=", "!", "?", "\"", "'"],
    ["[", "]", "{", "}", "<", ">", "~", "|", "\\", "°"],
];
const BOTTOM: [(&str, &str, f32); 7] = [("shift", "Shift", 1.3), ("symbols", "#+=", 1.1), ("space", "space", 2.2), ("tab", "tab", 1.0), ("back", "Del", 1.2), ("enter", "Enter", 1.3), ("paste", "Paste", 1.3)];

#[derive(Default)]
pub struct Keyboard {
    shift: bool,
    symbols: bool,
    /// Height it took last frame, in points, so the layout can keep the focused box above it.
    pub height: f32,
}

fn upper(s: &str) -> String {
    s.to_uppercase()
}

impl Keyboard {
    /// Lays the keyboard along the bottom of `screen`, draws it and returns its keys as touch zones.
    pub fn show(&mut self, ctx: &egui::Context, screen: Rect, held: &dyn Fn(Control) -> bool) -> Vec<Zone> {
        let w = screen.width();
        let key_h = ((screen.height() - 40.0) / 5.0 - 4.0).clamp(26.0, 46.0);
        let gap = 4.0;
        let rows = 5.0;
        let top_bar = 30.0;
        self.height = rows * (key_h + gap) + gap + top_bar;
        let top = screen.max.y - self.height;
        let mut zones = Vec::new();
        let mut keys: Vec<(Rect, Control, String)> = Vec::new();
        let unit = (w - gap * 11.0) / 10.0;
        let add_row = |y: f32, row: &[&'static str], keys: &mut Vec<(Rect, Control, String)>, shift: bool| {
            for (i, k) in row.iter().enumerate() {
                let x = screen.min.x + gap + i as f32 * (unit + gap);
                let r = Rect::from_min_size(Pos2::new(x, y), egui::vec2(unit, key_h));
                let label = if shift { upper(k) } else { k.to_string() };
                keys.push((r, Control::Key(k), label));
            }
        };
        let mut y = top + top_bar + gap;
        add_row(y, &DIGITS, &mut keys, false);
        let layer = if self.symbols { &SYMBOLS } else { &LETTERS };
        for row in layer {
            y += key_h + gap;
            add_row(y, row, &mut keys, self.shift && !self.symbols);
        }
        y += key_h + gap;
        let total: f32 = BOTTOM.iter().map(|b| b.2).sum();
        let u = (w - gap * (BOTTOM.len() as f32 + 1.0)) / total;
        let mut x = screen.min.x + gap;
        for (id, label, span) in BOTTOM {
            let r = Rect::from_min_size(Pos2::new(x, y), egui::vec2(u * span, key_h));
            keys.push((r, Control::Key(id), label.to_string()));
            x += u * span + gap;
        }
        // "Done" closes the keyboard; it sits in the bar above the keys.
        let done = Rect::from_min_size(Pos2::new(screen.max.x - 86.0, top + 2.0), egui::vec2(80.0, top_bar - 4.0));
        keys.push((done, Control::Key("done"), "Done".into()));

        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("keyboard")));
        painter.rect_filled(Rect::from_min_max(Pos2::new(screen.min.x, top), screen.max), 0.0, Color32::from_rgb(0x1b, 0x1e, 0x22));
        painter.text(Pos2::new(screen.min.x + 10.0, top + top_bar / 2.0), Align2::LEFT_CENTER, "Typing", FontId::proportional(13.0), Color32::from_gray(150));
        for (r, c, label) in &keys {
            let on = held(*c) || (matches!(c, Control::Key("shift")) && self.shift) || (matches!(c, Control::Key("symbols")) && self.symbols);
            let fill = if on { Color32::from_rgb(0x2f, 0x6f, 0xd0) } else if matches!(c, Control::Key("done")) { Color32::from_rgb(0x1f, 0x8d, 0x4d) } else { Color32::from_gray(58) };
            painter.rect(*r, 6.0, fill, Stroke::NONE, StrokeKind::Inside);
            painter.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional((key_h * 0.45).min(19.0).min(r.width() / label.chars().count().max(1) as f32 * 1.6)), Color32::WHITE);
            zones.push(Zone { rect: *r, control: *c });
        }
        zones
    }

    /// What a pressed key types. `paste` reads the platform clipboard.
    pub fn press(&mut self, key: &str, paste: &dyn Fn() -> Option<String>) -> Vec<Event> {
        let m = Modifiers::default();
        let press = |kind: Key| vec![Event::KeyPress { kind, modifiers: m, handled: false }, Event::KeyRelease { kind, modifiers: m, handled: false }];
        match key {
            "shift" => { self.shift = !self.shift; vec![] }
            "symbols" => { self.symbols = !self.symbols; vec![] }
            "space" => vec![Event::Text(" ".into())],
            "tab" => vec![Event::Text("\t".into())],
            "back" => press(Key::Backspace),
            "enter" => press(Key::Enter),
            // The app closes the keyboard by tapping outside the box, which commits a number the way leaving it does.
            "done" => vec![],
            "paste" => paste().map(|t| vec![Event::Text(t)]).unwrap_or_default(),
            k => {
                let t = if self.shift && !self.symbols { upper(k) } else { k.to_string() };
                self.shift = false;
                vec![Event::Text(t)]
            }
        }
    }
}
