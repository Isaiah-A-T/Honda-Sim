import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FLOW, createGasflow, settle, stockVtc } from '../src/gasflow.js';
import { placeholderTune, turboSetup } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';
import { createInternals, stepInternals, materialsFor } from '../src/internals.js';

const cyc = (c, geom = FLOW) => settle(createGasflow(geom), { rpm: 6000, mapKpa: 100, iatC: 25, highCam: true, vtc: 40, ignDeg: 30, lambda: 0.88, fire: true, octane: 91, ...c });
function wot(tune, rpm, s = 3) {
  const e = createEngine(tune), st = createInternals();
  for (let t = 0; t < s; t += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); stepInternals(st, e, 0.001); }
  return { e, s: st };
}

test('retarding the spark moves the burn later: lower peak pressure, hotter exhaust, less knock', () => {
  const early = cyc({ ignDeg: 32 }), late = cyc({ ignDeg: 12 });
  assert.ok(late.ca50Deg > early.ca50Deg + 10);
  assert.ok(late.pmaxBar < early.pmaxBar - 15);
  assert.ok(late.egtC > early.egtC + 30);
  assert.ok(late.knockIndex < early.knockIndex);
});

test('there is a best-torque timing: too early and too late both lose work', () => {
  const work = ign => { const o = cyc({ ignDeg: ign }); return o.imepKpa - o.pmepKpa; };
  assert.ok(work(32) > work(10) && work(32) > work(55));
});

test('higher octane and cooler air raise the knock margin; boost lowers it', () => {
  const base = cyc({ rpm: 3000, highCam: false, ignDeg: 22 });
  assert.ok(cyc({ rpm: 3000, highCam: false, ignDeg: 22, octane: 100 }).knockIndex < base.knockIndex);
  assert.ok(cyc({ rpm: 3000, highCam: false, ignDeg: 22, iatC: 60 }).knockIndex > base.knockIndex);
  assert.ok(cyc({ rpm: 3000, highCam: false, ignDeg: 22, mapKpa: 160 }).knockIndex > base.knockIndex);
});

test('cam phasing and lobes change breathing: the high cam wins up top, the low cam down low', () => {
  const ve = (rpm, hc) => cyc({ rpm, highCam: hc, vtc: stockVtc(rpm, 100, hc) }).ve;
  assert.ok(ve(8000, true) > ve(8000, false) + 0.03);
  assert.ok(ve(3000, false) > ve(3000, true));
  assert.notEqual(cyc({ rpm: 3000, highCam: false, vtc: 0 }).ve.toFixed(3), cyc({ rpm: 3000, highCam: false, vtc: 40 }).ve.toFixed(3));
});

test('a freer exhaust lowers back pressure and pumping work', () => {
  const stock = cyc({ rpm: 7800 }), free = cyc({ rpm: 7800 }, { ...FLOW, exhaustDiaM: 0.065 });
  assert.ok(free.exhKpa < stock.exhKpa - 3);
  assert.ok(free.pmepKpa < stock.pmepKpa);
});

test('a bigger exhaust gains power on the engine', () => {
  const t = placeholderTune();
  assert.ok(wot({ ...t, flow: { exhaustDiaM: 0.065 } }, 7800).e.torqueNm > wot(t, 7800).e.torqueNm + 2);
});

test('the turbo spools from exhaust energy and makes back pressure', () => {
  const flat = v => ({ rows: [1000, 9000], cols: [20, 300], data: [[v, v], [v, v]] });
  const t = { ...turboSetup(), fuelUnits: 'afr', fuelLow: flat(11.5), fuelHigh: flat(11.5), ignLow: flat(12), ignHigh: flat(12), wgDuty: flat(0) };
  const { e } = wot(t, 6000);
  assert.ok(e.turboRadS > 5000 && e.turbineKw > 3);
  assert.ok(e.exhKpa > 130, `exhaust ${e.exhKpa}`);
});

test('every part has stock and aftermarket alloys to choose from', () => {
  for (const c of ['piston', 'rod', 'rodBolt', 'bearing', 'crank', 'headBolt', 'sleeve']) assert.ok(materialsFor(c).length >= 2, c);
});

test('forged internals survive boost and timing that break the stock bottom end', () => {
  const flat = v => ({ rows: [1000, 9000], cols: [20, 300], data: [[v, v], [v, v]] });
  const t = { ...turboSetup(), fuelUnits: 'afr', fuelLow: flat(11.5), fuelHigh: flat(11.5), ignLow: flat(16), ignHigh: flat(16), wgDuty: flat(70), overboostKpa: 400 };
  const forged = { piston: 'piston2618', rod: 'rod4340', rodBolt: 'boltARP2000', headBolt: 'headStudARP2000', sleeve: 'sleeveDuctile', bearing: 'bearingTriMetal', crank: 'crank4340' };
  const stock = wot(t, 6000, 10), built = wot({ ...t, materials: forged }, 6000, 10);
  const worst = s => Math.max(...Object.entries(s.load).filter(([k]) => k !== 'valvetrain').map(([, v]) => v));
  assert.ok(worst(built.s) < worst(stock.s) * 0.75, `${worst(built.s)} vs ${worst(stock.s)}`);
});
