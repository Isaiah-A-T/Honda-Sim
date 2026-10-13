import { test } from 'node:test';
import assert from 'node:assert/strict';
import { placeholderTune, turboSetup } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';
import { createInternals, stepInternals, cyclesToFailure, peakPressureBar } from '../src/internals.js';
import { createSim, step, startEngine, rebuildEngine } from '../src/sim.js';

function hold(tune, rpm, seconds) {
  const e = createEngine(tune), s = createInternals();
  for (let t = 0; t < seconds; t += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); stepInternals(s, e, 0.001); }
  return { e, s };
}
const flat = v => ({ rows: [1000, 9000], cols: [20, 300], data: [[v, v], [v, v]] });

test('fatigue: no damage under the endurance limit, faster damage the higher the load', () => {
  assert.equal(cyclesToFailure(0.7), Infinity);
  assert.ok(cyclesToFailure(0.9) > cyclesToFailure(1.1));
  assert.equal(cyclesToFailure(1.6), 1);
});

test('stock engine at full throttle up to redline takes no damage', () => {
  for (const rpm of [3000, 6200, 8000]) {
    const { s } = hold(placeholderTune(), rpm, 20);
    assert.ok(Object.values(s.damage).every(d => d === 0), `damage at ${rpm}: ${JSON.stringify(s.damage)}`);
    assert.ok(s.pmaxBar > 50 && s.pmaxBar < 90, `pmax ${s.pmaxBar}`);
  }
});

test('over-revving floats the valves', () => {
  const { s } = hold({ ...placeholderTune(), revLimit: 12000 }, 9600, 20);
  assert.ok(s.load.valvetrain > 1);
  assert.ok(s.damage.valvetrain > 0);
});

test('high boost with too much timing knocks and breaks the stock bottom end', () => {
  const t = { ...turboSetup(), fuelUnits: 'afr', fuelLow: flat(12.5), fuelHigh: flat(12.5), ignLow: flat(22), ignHigh: flat(22), wgDuty: flat(70), overboostKpa: 400 };
  const { s } = hold(t, 6000, 60);
  assert.ok(s.failed, 'expected a failure');
});

test('more boost means more cylinder pressure; retarding timing lowers it', () => {
  const base = { map: 100, ve: 0.95, iat: 25, ign: 30, mbt: 30, lambda: 0.88, torqueNm: 1 };
  assert.ok(peakPressureBar({ ...base, map: 180 }) > peakPressureBar(base) * 1.6);
  assert.ok(peakPressureBar({ ...base, ign: 20 }) < peakPressureBar(base));
});

test('a failed engine stops and only restarts after a rebuild', () => {
  const sim = createSim();
  sim.internals.damage.rod = 0.9999999;
  sim.internals.load.rod = 1.2;
  sim.internals.failed = 'Connecting rods';
  step(sim, { throttle: 0 }, 0.5);
  assert.equal(sim.running, false);
  startEngine(sim);
  assert.equal(sim.running, false);
  rebuildEngine(sim);
  startEngine(sim);
  assert.equal(sim.running, true);
});
