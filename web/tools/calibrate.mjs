// Fits the per-rpm breathing corrections in src/k20z3.js so the engine model (gas exchange model ×
// correction) hits TORQUE_TARGETS at wide-open throttle on the placeholder (stock-like) tune, then
// rewrites VE_CORR_LOW / VE_CORR_HIGH in place. A correction near 1 means the gas model already gets
// that point right on its own.
//   node tools/calibrate.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import * as K from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';

const file = new URL('../src/k20z3.js', import.meta.url);

// Torque at a fixed rpm and full throttle with a flat correction, holding the cam we're solving for.
function torqueAt(rpm, highCam, corr) {
  const tune = { ...K.placeholderTune(), vtecRpm: highCam ? 0 : 1e9, vtecMinMap: 0, revLimit: 1e9 };
  const save = K.VE_CORR_LOW.slice(), saveH = K.VE_CORR_HIGH.slice();
  K.VE_CORR_LOW.fill(corr); K.VE_CORR_HIGH.fill(corr);
  const e = createEngine(tune);
  for (let i = 0; i < 2000; i++) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  save.forEach((v, i) => { K.VE_CORR_LOW[i] = v; }); saveH.forEach((v, i) => { K.VE_CORR_HIGH[i] = v; });
  return e.torqueNm;
}

// Secant search: torque is close to linear in the correction.
function solve(rpm, highCam, target) {
  let c0 = 1, t0 = torqueAt(rpm, highCam, c0), c1 = 1.1, t1 = torqueAt(rpm, highCam, c1);
  for (let i = 0; i < 8 && Math.abs(t1 - target) > 0.2; i++) {
    const c2 = Math.min(1.6, Math.max(0.3, c1 + (target - t1) * (c1 - c0) / (t1 - t0 || 1e-6)));
    c0 = c1; t0 = t1; c1 = c2; t1 = torqueAt(rpm, highCam, c1);
  }
  return Math.round(c1 * 10000) / 10000;
}

const low = K.TORQUE_TARGETS.low.map(([r, t]) => solve(r, false, t));
const high = K.TORQUE_TARGETS.high.map(([r, t]) => solve(r, true, t));
let src = readFileSync(file, 'utf8');
src = src.replace(/export const VE_CORR_LOW = [^;]*;/, `export const VE_CORR_LOW = [${low.join(', ')}];`);
src = src.replace(/export const VE_CORR_HIGH = [^;]*;/, `export const VE_CORR_HIGH = [${high.join(', ')}];`);
writeFileSync(file, src);
console.log('low ', low.join(' '));
console.log('high', high.join(' '));
