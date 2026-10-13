// Driver input: wheel/pedals through the browser Gamepad API, with a keyboard fallback.
//
// Wheelbases and pedal sets show up as gamepads, but every brand numbers its axes and buttons
// differently and many pedals rest at +1 and go to -1 when pressed. So nothing is hard-coded:
// "Learn" records which axis moves and its rest/pressed values, or which button gets pressed.

import { clamp } from './tables.js';

const STORE_KEY = 'honda-sim.controls.v1';

export const AXES = ['throttle', 'brake', 'clutch'];
export const BUTTONS = ['shiftUp', 'shiftDown', 'gear1', 'gear2', 'gear3', 'gear4', 'gear5', 'gear6', 'reverse', 'start'];

export function loadMapping() {
  try { return JSON.parse(localStorage.getItem(STORE_KEY)) || {}; } catch { return {}; }
}
export function saveMapping(map) {
  try { localStorage.setItem(STORE_KEY, JSON.stringify(map)); } catch { /* private mode: mapping lasts this session */ }
}

// Normalize a raw axis reading to 0 (rest) … 1 (fully pressed).
export function normAxis(raw, { rest, full }) {
  if (full === rest) return 0;
  return clamp((raw - rest) / (full - rest), 0, 1);
}

export function createInput() {
  return {
    mapping: loadMapping(),
    keys: new Set(),
    kb: { throttle: 0, brake: 0, clutch: 0 },
    prevButtons: {},
    events: [],            // 'shiftUp', 'shiftDown', 'gear3', 'neutral', 'start', ...
    learning: null,        // { kind: 'axis'|'button', name, baseline, pad, extreme }
    padName: '',
  };
}

export function attachKeyboard(inp, target = window) {
  const keyEvents = { KeyE: 'shiftUp', KeyQ: 'shiftDown', KeyN: 'neutral', KeyR: 'start', Digit1: 'gear1', Digit2: 'gear2', Digit3: 'gear3', Digit4: 'gear4', Digit5: 'gear5', Digit6: 'gear6', Digit0: 'reverse' };
  target.addEventListener('keydown', ev => {
    if (ev.target && /INPUT|TEXTAREA|SELECT/.test(ev.target.tagName)) return;
    if (!inp.keys.has(ev.code) && keyEvents[ev.code]) inp.events.push(keyEvents[ev.code]);
    inp.keys.add(ev.code);
    if (/Arrow|Space/.test(ev.code)) ev.preventDefault();
  });
  target.addEventListener('keyup', ev => inp.keys.delete(ev.code));
  target.addEventListener('blur', () => inp.keys.clear());
}

function firstPad() {
  const pads = typeof navigator !== 'undefined' && navigator.getGamepads ? [...navigator.getGamepads()] : [];
  return pads.filter(Boolean);
}

export function startLearn(inp, kind, name) {
  const pads = firstPad();
  if (!pads.length) return 'No wheel or pedals detected. Press a pedal or button so the browser sees them, then try again.';
  inp.learning = { kind, name, baseline: pads.map(p => ({ index: p.index, axes: [...p.axes], buttons: p.buttons.map(b => b.pressed) })), best: null };
  return kind === 'axis' ? `Press the ${name} pedal all the way down and release it.` : `Press the button or paddle for ${name}.`;
}

function updateLearn(inp, pads) {
  const L = inp.learning;
  for (const p of pads) {
    const base = L.baseline.find(b => b.index === p.index);
    if (!base) continue;
    if (L.kind === 'axis') {
      p.axes.forEach((v, a) => {
        const d = Math.abs(v - base.axes[a]);
        if (d > 0.5 && (!L.best || d > L.best.d)) L.best = { d, pad: p.id, axis: a, rest: base.axes[a], full: v };
      });
      // Finish once the pedal is back near rest after a big movement.
      if (L.best && L.best.pad === p.id && Math.abs(p.axes[L.best.axis] - L.best.rest) < 0.1) {
        const { pad, axis, rest, full } = L.best;
        inp.mapping[L.name] = { pad, axis, rest, full: Math.abs(full) > 0.97 ? Math.sign(full) : full };
        saveMapping(inp.mapping);
        inp.learning = null;
        return `${L.name} set to axis ${axis}.`;
      }
    } else {
      const b = p.buttons.findIndex((btn, i) => btn.pressed && !base.buttons[i]);
      if (b >= 0) {
        inp.mapping[L.name] = { pad: p.id, button: b };
        saveMapping(inp.mapping);
        inp.learning = null;
        return `${L.name} set to button ${b}.`;
      }
    }
  }
  return null;
}

// Read everything for this frame. Returns { throttle, brake, clutch (null = auto), events, message }.
export function readInput(inp, dt) {
  const pads = firstPad();
  inp.padName = pads[0]?.id || '';
  let message = null;
  if (inp.learning) message = updateLearn(inp, pads);

  // Keyboard: pedals ramp so tapping a key feels like a pedal.
  const k = inp.keys, kb = inp.kb;
  const ramp = (cur, on, up = 3, down = 5) => clamp(cur + (on ? up : -down) * dt, 0, 1);
  kb.throttle = ramp(kb.throttle, k.has('KeyW') || k.has('ArrowUp'), k.has('ShiftLeft') ? 6 : 2.5);
  kb.brake = ramp(kb.brake, k.has('KeyS') || k.has('ArrowDown'), 4, 8);
  kb.clutch = ramp(kb.clutch, k.has('KeyC') || k.has('Space'), 8, 4);

  const out = { throttle: kb.throttle, brake: kb.brake, clutch: kb.clutch > 0.01 ? kb.clutch : null, events: inp.events.splice(0), message };

  if (!inp.learning) {
    const m = inp.mapping;
    const padFor = id => pads.find(p => p.id === id);
    for (const name of AXES) {
      const a = m[name], p = a && padFor(a.pad);
      if (!p || p.axes[a.axis] === undefined) continue;
      const v = normAxis(p.axes[a.axis], a);
      if (name === 'clutch') out.clutch = Math.max(v, out.clutch ?? 0);
      else out[name] = Math.max(v, out[name]);
    }
    for (const name of BUTTONS) {
      const b = m[name], p = b && padFor(b.pad);
      if (!p || !p.buttons[b.button]) continue;
      const pressed = p.buttons[b.button].pressed, key = `${name}`;
      if (pressed && !inp.prevButtons[key]) out.events.push(name);
      // H-pattern: releasing a gear button drops to neutral.
      if (!pressed && inp.prevButtons[key] && /^gear\d$|^reverse$/.test(name)) out.events.push('neutral');
      inp.prevButtons[key] = pressed;
    }
  }
  return out;
}
