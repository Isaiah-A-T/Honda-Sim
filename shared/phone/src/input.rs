//! Turns the window's touches, mouse and keys into what the app needs. Each finger is routed by where it lands:
//! on a pedal or drive button it belongs to that control for as long as it stays down (so you can hold the
//! throttle and tap shift up with another finger); anywhere else the first finger drives the menus and turns
//! the 3D view, and a second finger pinches to zoom.

use crate::controls;
use std::collections::HashMap;
use three_d::{Event, Key, Modifiers, MouseButton, PhysicalPoint};
use winit::event::{ElementState, KeyboardInput, Touch, TouchPhase, VirtualKeyCode, WindowEvent};

/// A screen area owned by a drive control, in egui points (top-left origin).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Control {
    Throttle,
    Brake,
    Clutch,
    /// Fires the sim event once when pressed.
    Button(&'static str),
    /// A key on the on-screen keyboard (src/keyboard.rs).
    Key(&'static str),
}

#[derive(Clone, Copy)]
pub struct Zone {
    pub rect: three_d::egui::Rect,
    pub control: Control,
}

struct Finger {
    pos: (f32, f32),
    control: Option<Control>,
}

#[derive(Default)]
pub struct Input {
    fingers: HashMap<u64, Finger>,
    /// The finger acting as the mouse, and a second one pinching.
    primary: Option<u64>,
    secondary: Option<u64>,
    pub events: Vec<Event>,
    /// Sim events from drive buttons pressed since the last frame.
    pub fired: Vec<&'static str>,
    /// On-screen keyboard keys pressed since the last frame.
    pub keys: Vec<&'static str>,
    /// Drive control areas from the last frame's layout.
    pub zones: Vec<Zone>,
    /// Physical pixels per egui point, and the window height in physical pixels.
    pub scale: f32,
    pub height: f32,
    modifiers: Modifiers,
    /// Hardware keys the desktop uses for driving (W/S/C/E/Q...) arrive as egui keys; this is only the
    /// Android back button.
    pub back: bool,
    leave: bool,
}

impl Input {
    fn point(&self, x: f32, y: f32) -> PhysicalPoint {
        PhysicalPoint { x, y: self.height - y }
    }

    /// This frame's events, oldest first.
    pub fn take_events(&mut self) -> Vec<Event> {
        let events = std::mem::take(&mut self.events);
        if std::mem::take(&mut self.leave) {
            self.events.push(Event::MouseLeave);
        }
        events
    }

    fn zone_at(&self, x: f32, y: f32) -> Option<Control> {
        let p = three_d::egui::pos2(x / self.scale, y / self.scale);
        self.zones.iter().find(|z| z.rect.contains(p)).map(|z| z.control)
    }

    /// How far each pedal is pressed: higher on the pedal is more.
    pub fn pedals(&self) -> crate::Touch {
        let mut t = crate::Touch::default();
        for f in self.fingers.values() {
            let Some(c) = f.control else { continue };
            let Some(z) = self.zones.iter().find(|z| z.control == c) else { continue };
            let y = f.pos.1 / self.scale;
            let v = (((z.rect.max.y - y) / z.rect.height()) as f64 * 1.15).clamp(0.05, 1.0);
            match c {
                Control::Throttle => t.throttle = t.throttle.max(v),
                Control::Brake => t.brake = t.brake.max(v),
                Control::Clutch => t.clutch = 1.0,
                Control::Button(_) | Control::Key(_) => {}
            }
        }
        t
    }

    /// Which controls a finger is on now (for drawing them pressed).
    pub fn held(&self, c: Control) -> bool {
        self.fingers.values().any(|f| f.control == Some(c))
    }

    pub fn touch(&mut self, t: &Touch) {
        let (x, y) = (t.location.x as f32, t.location.y as f32);
        match t.phase {
            TouchPhase::Started => {
                let control = self.zone_at(x, y);
                match control {
                    Some(Control::Button(ev)) => self.fired.push(ev),
                    Some(Control::Key(k)) => self.keys.push(k),
                    _ => {}
                }
                self.fingers.insert(t.id, Finger { pos: (x, y), control });
                if control.is_none() {
                    if self.primary.is_none() {
                        self.primary = Some(t.id);
                        self.leave = false;
                        self.events.retain(|e| !matches!(e, Event::MouseLeave));
                        let position = self.point(x, y);
                        self.events.push(Event::MouseMotion { button: None, delta: (0.0, 0.0), position, modifiers: self.modifiers, handled: false });
                        self.events.push(Event::MousePress { button: MouseButton::Left, position, modifiers: self.modifiers, handled: false });
                    } else if self.secondary.is_none() {
                        self.secondary = Some(t.id);
                    }
                }
            }
            TouchPhase::Moved => {
                let Some(f) = self.fingers.get(&t.id) else { return };
                let last = f.pos;
                let is_control = f.control.is_some();
                self.fingers.get_mut(&t.id).unwrap().pos = (x, y);
                if is_control {
                    return;
                }
                let other = |id: Option<u64>, fingers: &HashMap<u64, Finger>| id.and_then(|i| fingers.get(&i)).map(|f| f.pos);
                if Some(t.id) == self.primary {
                    if let Some(p) = other(self.secondary, &self.fingers) {
                        self.pinch(last, (x, y), p);
                    } else {
                        let s = self.scale;
                        self.events.push(Event::MouseMotion {
                            button: Some(MouseButton::Left),
                            delta: ((x - last.0) / s, (y - last.1) / s),
                            position: self.point(x, y),
                            modifiers: self.modifiers,
                            handled: false,
                        });
                    }
                } else if Some(t.id) == self.secondary {
                    if let Some(p) = other(self.primary, &self.fingers) {
                        self.pinch(last, (x, y), p);
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.fingers.remove(&t.id);
                if Some(t.id) == self.primary {
                    self.primary = None;
                    self.events.push(Event::MouseRelease { button: MouseButton::Left, position: self.point(x, y), modifiers: self.modifiers, handled: false });
                    // The finger leaves next frame: in the same frame as the release, egui would lose the click.
                    self.leave = true;
                } else if Some(t.id) == self.secondary {
                    self.secondary = None;
                }
            }
        }
    }

    fn pinch(&mut self, from: (f32, f32), to: (f32, f32), other: (f32, f32)) {
        let d = |a: (f32, f32)| ((a.0 - other.0).powi(2) + (a.1 - other.1).powi(2)).sqrt();
        let delta = (d(to) - d(from)) / self.scale;
        let position = self.point(other.0, other.1);
        self.events.push(Event::MouseWheel { delta: (0.0, delta * 1.5), position, modifiers: self.modifiers, handled: false });
    }

    /// Window events other than touches: keys, typed text, and a mouse standing in for one finger on desktops.
    pub fn window_event(&mut self, ev: &WindowEvent, cursor: &mut (f32, f32), mouse_down: &mut bool) {
        match ev {
            WindowEvent::KeyboardInput { input, .. } => self.key(input),
            WindowEvent::ReceivedCharacter(c) if !c.is_control() => self.events.push(Event::Text(c.to_string())),
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = Modifiers { alt: m.alt(), ctrl: m.ctrl(), shift: m.shift(), command: m.ctrl() || m.logo() };
                self.events.push(Event::ModifiersChange { modifiers: self.modifiers });
            }
            // On desktops the mouse is a finger with id u64::MAX, so the preview behaves like the phone.
            WindowEvent::CursorMoved { position, .. } => {
                *cursor = (position.x as f32, position.y as f32);
                if *mouse_down {
                    self.touch(&fake_touch(TouchPhase::Moved, *cursor));
                }
            }
            WindowEvent::MouseInput { state, button: winit::event::MouseButton::Left, .. } => {
                *mouse_down = *state == ElementState::Pressed;
                self.touch(&fake_touch(if *mouse_down { TouchPhase::Started } else { TouchPhase::Ended }, *cursor));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 30.0,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let position = self.point(cursor.0, cursor.1);
                self.events.push(Event::MouseWheel { delta: (0.0, dy), position, modifiers: self.modifiers, handled: false });
            }
            WindowEvent::Touch(t) => self.touch(t),
            _ => {}
        }
    }

    fn key(&mut self, input: &KeyboardInput) {
        let pressed = input.state == ElementState::Pressed;
        let Some(vk) = input.virtual_keycode else {
            // No keyboard meaning: a gamepad button (or the Android back button, scan code 158).
            if input.scancode == 158 {
                self.back |= pressed;
            } else {
                controls::key_event(input.scancode, pressed);
            }
            return;
        };
        if let Some(kind) = key(vk) {
            let modifiers = self.modifiers;
            self.events.push(if pressed { Event::KeyPress { kind, modifiers, handled: false } } else { Event::KeyRelease { kind, modifiers, handled: false } });
        }
        if vk == VirtualKeyCode::Escape && pressed {
            self.back = true;
        }
    }
}

fn fake_touch(phase: TouchPhase, at: (f32, f32)) -> Touch {
    // DeviceId has no public constructor on every platform; winit allows a dummy one.
    Touch { device_id: unsafe { winit::event::DeviceId::dummy() }, phase, location: winit::dpi::PhysicalPosition::new(at.0 as f64, at.1 as f64), force: None, id: u64::MAX }
}

fn key(k: VirtualKeyCode) -> Option<Key> {
    use VirtualKeyCode as V;
    Some(match k {
        V::Down => Key::ArrowDown,
        V::Left => Key::ArrowLeft,
        V::Right => Key::ArrowRight,
        V::Up => Key::ArrowUp,
        V::Escape => Key::Escape,
        V::Tab => Key::Tab,
        V::Back => Key::Backspace,
        V::Return | V::NumpadEnter => Key::Enter,
        V::Space => Key::Space,
        V::Delete => Key::Delete,
        V::Home => Key::Home,
        V::End => Key::End,
        V::Key0 | V::Numpad0 => Key::Num0,
        V::Key1 | V::Numpad1 => Key::Num1,
        V::Key2 | V::Numpad2 => Key::Num2,
        V::Key3 | V::Numpad3 => Key::Num3,
        V::Key4 | V::Numpad4 => Key::Num4,
        V::Key5 | V::Numpad5 => Key::Num5,
        V::Key6 | V::Numpad6 => Key::Num6,
        V::Key7 | V::Numpad7 => Key::Num7,
        V::Key8 | V::Numpad8 => Key::Num8,
        V::Key9 | V::Numpad9 => Key::Num9,
        V::A => Key::A, V::C => Key::C, V::E => Key::E, V::N => Key::N, V::Q => Key::Q, V::R => Key::R, V::S => Key::S, V::V => Key::V, V::W => Key::W, V::X => Key::X, V::Z => Key::Z,
        _ => return None,
    })
}
