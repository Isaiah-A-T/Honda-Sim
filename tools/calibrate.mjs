// Solves the volumetric-efficiency tables in src/k20z3.js so the engine model hits TORQUE_TARGETS
// at wide-open throttle on the placeholder (stock-like) tune, then rewrites VE_LOW / VE_HIGH in place.
//   node tools/calibrate.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import * as K from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';

const file = new URL('../src/k20z3.js', import.meta.url);

// Torque at a fixed rpm and full throttle with a given VE, holding the cam we're solving for.
function torqueAt(rpm, highCam, ve) {
  const tune = { ...K.placeholderTune(), vtecRpm: highCam ? 0 : 1e9, vtecMinMap: 0, revLimit: 1e9 };
  const save = K.VE_LOW.slice(), saveH = K.VE_HIGH.slice();
  K.VE_LOW.length = 0; K.VE_HIGH.length = 0;
  K.VE_LOW.push(...K.VE_RPM.map(() => ve)); K.VE_HIGH.push(...K.VE_HIGH_RPM.map(() => ve));
  const e = createEngine(tune);
  for (let i = 0; i < 3000; i++) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  K.VE_LOW.length = 0; K.VE_HIGH.length = 0; K.VE_LOW.push(...save); K.VE_HIGH.push(...saveH);
  return e.torqueNm;
}

function solve(rpm, highCam, target) {
  let lo = 0.2, hi = 1.4;
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (torqueAt(rpm, highCam, mid) < target) lo = mid; else hi = mid;
  }
  return Math.round((lo + hi) / 2 * 10000) / 10000;
}

const low = K.TORQUE_TARGETS.low.map(([r, t]) => solve(r, false, t));
const high = K.TORQUE_TARGETS.high.map(([r, t]) => solve(r, true, t));
let src = readFileSync(file, 'utf8');
src = src.replace(/export const VE_LOW = \[[^\]]*\];/, `export const VE_LOW = [${low.join(', ')}];`);
src = src.replace(/export const VE_HIGH = \[[^\]]*\];/, `export const VE_HIGH = [${high.join(', ')}];`);
writeFileSync(file, src);
console.log('low ', low.join(' '));
console.log('high', high.join(' '));
