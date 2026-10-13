import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createSim, step, shift, snapshot, logToCsv } from '../src/sim.js';
import { createEngine, stepEngine, hp } from '../src/engine.js';
import { placeholderTune } from '../src/k20z3.js';
import * as placeholderTuneModule from '../src/k20z3.js';

const run = (sim, input, seconds) => { for (let t = 0; t < seconds; t += 0.01) step(sim, input, 0.01); };

// Steady-state WOT torque at a fixed rpm (as on an engine dyno).
function wot(tune, rpm) {
  const e = createEngine(tune);
  for (let i = 0; i < 3000; i++) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  return e;
}

test('idles near 750 rpm in neutral', () => {
  const s = createSim();
  run(s, { throttle: 0 }, 10);
  assert.ok(Math.abs(s.eng.rpm - 750) < 30, `idle ${s.eng.rpm}`);
  assert.ok(s.running);
});

test('stock-like power: roughly 180–200 hp near 7,800 rpm on the placeholder tune', () => {
  const e = wot(placeholderTune(), 7800);
  const p = hp(e.torqueNm, 7800);
  assert.ok(p > 175 && p < 205, `hp ${p}`);
  assert.ok(e.highCam);
});

test('launches, pulls through the gears with the auto-clutch and hits the rev limiter', () => {
  const s = createSim();
  run(s, { throttle: 0 }, 1);
  shift(s, 1);
  let sawCut = false;
  for (let t = 0; t < 5; t += 0.01) { step(s, { throttle: 1 }, 0.01); sawCut ||= s.eng.revCut; }
  assert.ok(s.running, 'did not stall');
  assert.ok(sawCut, 'rev limiter should cut in 1st');
  shift(s, 2);
  run(s, { throttle: 1 }, 2);
  assert.ok(s.car.locked);
  assert.ok(s.car.speedMs * 2.237 > 45, `speed ${s.car.speedMs * 2.237} mph`);
});

test('a VTEC point too high loses power between the cam crossover and the switch', () => {
  const early = { ...placeholderTune(), vtecRpm: 5000 };
  const late = { ...placeholderTune(), vtecRpm: 6500 };
  assert.ok(wot(early, 5800).torqueNm > wot(late, 5800).torqueNm + 10);
});

test('too much timing knocks and gets retarded; too little loses torque', () => {
  const base = placeholderTune();
  const plus = (tbl, d) => ({ ...tbl, data: tbl.data.map(r => r.map(v => v + d)) });
  const hot = wot({ ...base, ignLow: plus(base.ignLow, 10) }, 3500);
  assert.ok(hot.knockCount > 0 && hot.knockRetard > 0);
  const lazy = wot({ ...base, ignLow: plus(base.ignLow, -10) }, 3500);
  assert.ok(lazy.torqueNm < wot(base, 3500).torqueNm - 5);
});

test('pulse-width fuel tables set the mixture from injector flow', () => {
  const t = placeholderTune();
  const ms = v => ({ ...t.fuelLow, data: t.fuelLow.data.map(r => r.map(() => v)) });
  const lean = wot({ ...t, fuelUnits: 'ms', fuelLow: ms(6) }, 3000);
  const rich = wot({ ...t, fuelUnits: 'ms', fuelLow: ms(14) }, 3000);
  assert.ok(lean.lambda > rich.lambda);
  assert.ok(rich.lambda < 0.9 && lean.lambda > 1.0, `${rich.lambda} ${lean.lambda}`);
});

test('datalog rows export as CSV', () => {
  const s = createSim();
  s.logging = true;
  run(s, { throttle: 0.2 }, 1);
  assert.ok(s.log.length >= 15);
  const csv = logToCsv(s.log);
  assert.match(csv.split('\n')[0], /^time,rpm,vss_kmh,gear/);
  assert.equal(Object.keys(snapshot(s)).length, csv.split('\n')[0].split(',').length);
});

test('turbo build makes boost, and wastegate duty raises it above the spring', () => {
  const { turboSetup } = placeholderTuneModule;
  const flat = (v) => ({ rows: [1000, 8000], cols: [20, 250], data: [[v, v], [v, v]] });
  const base = { ...turboSetup(), fuelUnits: 'afr', fuelLow: flat(11.8), fuelHigh: flat(11.8), ignLow: flat(10), ignHigh: flat(10), overboostKpa: 300 };
  const springOnly = wot({ ...base, wgDuty: flat(0) }, 6000);
  const withDuty = wot({ ...base, wgDuty: flat(40) }, 6000);
  assert.ok(Math.abs(springOnly.map - 101.3 - base.turbo.springKpa) < 8, `spring boost ${springOnly.map}`);
  assert.ok(withDuty.map > springOnly.map + 30);
  assert.ok(withDuty.torqueNm > springOnly.torqueNm);
});

test('overboost cut stops fuel above the limit', () => {
  const { turboSetup } = placeholderTuneModule;
  const flat = (v) => ({ rows: [1000, 8000], cols: [20, 250], data: [[v, v], [v, v]] });
  const t = { ...turboSetup(), fuelUnits: 'afr', fuelLow: flat(11.8), ignLow: flat(8), wgDuty: flat(100), overboostKpa: 180 };
  delete t.fuelHigh; delete t.ignHigh;   // single tables are used for both cams
  const e = createEngine(t);
  let sawCut = false;
  for (let i = 0; i < 8000; i++) { e.rpm = 6000; stepEngine(e, 1, 0.001); sawCut ||= e.boostCut; }
  assert.ok(sawCut);
});
