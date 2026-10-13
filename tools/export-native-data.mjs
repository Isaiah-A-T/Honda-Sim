// Writes the data the native (Rust) sim compiles in, and parity fixtures its tests compare against.
//   native/sim/data/materials.json   material library (same as src/materials.js)
//   native/sim/data/parts-catalog.json
//   native/sim/data/parity.json      JS results for fixed scenarios
// Run after changing the JS model: node tools/export-native-data.mjs
import { writeFileSync, copyFileSync } from 'node:fs';
import { MATERIALS } from '../src/materials.js';
import { FLOW, createGasflow, settle } from '../src/gasflow.js';
import { placeholderTune, turboSetup, VE_CORR_LOW, VE_CORR_HIGH } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';
import { createInternals, stepInternals } from '../src/internals.js';
import { bottomEnd } from '../src/parts.js';
import { createSim, step, shift } from '../src/sim.js';

const out = p => new URL(`../native/sim/data/${p}`, import.meta.url);
writeFileSync(out('materials.json'), JSON.stringify(MATERIALS, null, 1));
copyFileSync(new URL('../docs/parts-catalog.json', import.meta.url), out('parts-catalog.json'));

const gas = [];
for (const c of [
  { rpm: 3000, mapKpa: 100, iatC: 25, highCam: false, vtc: 30, ignDeg: 24, lambda: 0.88, fire: true, octane: 91 },
  { rpm: 7000, mapKpa: 100, iatC: 25, highCam: true, vtc: 40, ignDeg: 32, lambda: 0.85, fire: true, octane: 91 },
  { rpm: 2000, mapKpa: 40, iatC: 30, highCam: false, vtc: 10, ignDeg: 35, lambda: 1, fire: true, octane: 91 },
  { rpm: 6000, mapKpa: 180, iatC: 45, highCam: true, vtc: 35, ignDeg: 16, lambda: 0.78, fire: true, octane: 93, outKpa: 105.3, outAreaM2: 9.5e-4 },
  { rpm: 4000, mapKpa: 30, iatC: 25, highCam: false, vtc: 0, ignDeg: 20, lambda: 3, fire: false, octane: 91 },
]) {
  const s = createGasflow(FLOW);
  gas.push({ cond: c, out: { ...settle(s, c, 40) } });
}

const wot = (tune, rpm) => {
  const e = createEngine(tune), st = createInternals();
  for (let t = 0; t < 1.5; t += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); stepInternals(st, e, 0.001); }
  return { torqueNm: e.torqueNm, map: e.map, ign: e.ign, lambda: e.lambda, boost: e.boost, camF: e.camF, pmaxBar: st.pmaxBar, load: st.load };
};
const engine = [];
const k24 = { ...placeholderTune(), parts: { crank: 'honda-k24-99-hybrid', piston: 'arias-k24k20-87-11.0' } };
for (const [name, tune] of [['stock', placeholderTune()], ['turbo', turboSetup()], ['k24', k24]]) {
  for (const rpm of [2500, 5000, 7500]) engine.push({ name, rpm, ...wot(tune, rpm) });
}

const parts = ['stock', 'bc-k20-92-stroker', 'toda-k20-90.7-hc-87', 'honda-k24-99-hybrid'].flatMap(crank =>
  [undefined, 'je-k20-87-12.5', 'cp-k24k20-87-14.2'].map(piston => {
    const b = bottomEnd({ parts: { crank, piston, rod: 'eagle-k20-139' } });
    return { parts: { crank, piston, rod: 'eagle-k20-139' }, boreMm: b.boreMm, strokeMm: b.strokeMm, rodMm: b.rodMm, compression: b.compression, displacementL: b.displacementL, pistonKg: b.pistonKg, issues: b.issues.length };
  }));

// A launch and pull through the gears with the auto clutch.
const sim = createSim(placeholderTune());
shift(sim, 1);
const drive = [];
for (let i = 0; i < 600; i++) {
  if (i === 250) shift(sim, 2);
  step(sim, { throttle: 1, brake: 0, clutch: null }, 0.01);
  if (i % 50 === 49) drive.push({ t: sim.t, rpm: sim.eng.rpm, speedMs: sim.car.speedMs, gear: sim.car.gear });
}

writeFileSync(out('parity.json'), JSON.stringify({ veCorrLow: VE_CORR_LOW, veCorrHigh: VE_CORR_HIGH, gas, engine, parts, drive }, null, 1));
console.log(`gas ${gas.length}, engine ${engine.length}, parts ${parts.length}, drive ${drive.length}`);
