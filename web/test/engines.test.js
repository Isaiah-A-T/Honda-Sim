import { test } from 'node:test';
import assert from 'node:assert/strict';
import { BLOCKS, HEADS, OIL_PUMPS, engineSetup, headFit, pumpFit, balancerNm } from '../src/engines.js';
import { bottomEnd, flowGeometry } from '../src/parts.js';
import { placeholderTune } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';
import { oilPressureKpa } from '../src/thermal.js';

const byId = (l, id) => l.find(x => x.id === id);
function wot(engine, rpm, extra = {}) {
  const e = createEngine({ ...placeholderTune(), engine, octane: 100, ...extra });
  for (let x = 0; x < 1.5; x += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  return e;
}

test('every block, head and pump has sources; every block\'s own head and pump fit it', () => {
  for (const x of [...BLOCKS, ...HEADS, ...OIL_PUMPS]) assert.ok(x.sources.length, x.id);
  for (const b of BLOCKS) {
    assert.equal(headFit(byId(HEADS, b.head), b).works, 'yes', b.id);
    assert.equal(pumpFit(byId(OIL_PUMPS, b.oilPump), b).works, 'yes', b.id);
    // stock pistons sit flush: stroke/2 + rod + 30 mm compression height = deck
    assert.ok(Math.abs(b.strokeMm / 2 + b.rodMm + b.compressionHeightMm - b.deckMm) < 0.01, b.id);
  }
});

test('a block brings its own bore, stroke, rods and compression', () => {
  const k24 = bottomEnd({ engine: { block: 'K24A2', head: 'K24A2' } });
  assert.equal(k24.strokeMm, 99); assert.equal(k24.rodMm, 152); assert.equal(k24.deckMm, 231.5);
  assert.ok(Math.abs(k24.displacementL - 2.354) < 0.001);
  assert.ok(Math.abs(k24.compression - 10.5) < 1e-9);
  assert.equal(k24.deckClearanceMm, 0);
  const a3 = bottomEnd({ engine: { block: 'K20A3' } });
  assert.ok(Math.abs(a3.compression - 9.8) < 1e-9);
  assert.equal(a3.head.id, 'K20A3');
});

test('a K20 head goes on a K24A with the swap parts; K24 heads on K20 blocks are refused', () => {
  const frank = engineSetup({ engine: { block: 'K24A2', head: 'K20Z3' } });
  assert.equal(frank.headFit.works, 'mods');
  assert.ok(frank.headFit.needs.some(n => /chain/.test(n)));
  const bad = engineSetup({ engine: { block: 'K20Z3', head: 'K24A2' } });
  assert.equal(bad.head.id, 'K20Z3');
  assert.match(bad.issues[0], /can't go on/);
  // K24A8 pistons hit a K20 head's valves; aftermarket pistons fix it
  const a8 = bottomEnd({ engine: { block: 'K24A8', head: 'K20A2' } });
  assert.equal(a8.head.id, 'K24A4');
  assert.ok(a8.issues.some(i => /hit/.test(i)));
  const a8f = bottomEnd({ engine: { block: 'K24A8', head: 'K20A2' }, parts: { piston: 'cp-k24k20-87-14.2' } });
  assert.equal(a8f.head.id, 'K20A2');
  assert.equal(a8f.issues.length, 0);
});

test('K20 stroker kits and 86 mm pistons don\'t go in a K24; aftermarket parts don\'t go in a K20A3', () => {
  const k = bottomEnd({ engine: { block: 'K24A2' }, parts: { crank: 'bc-k20-92-stroker', piston: 'je-k20-87-12.5' } });
  assert.equal(k.crank.id, 'stock');
  assert.ok(k.issues.some(i => /deck/.test(i)));
  const small = bottomEnd({ engine: { block: 'K24A2' }, parts: { piston: 'wiseco-k20-87-9.7' } });
  assert.equal(small.piston.id, 'wiseco-k20-87-9.7');   // 87 in an 87 bore is fine
  const a3 = bottomEnd({ engine: { block: 'K20A3' }, parts: { rod: 'eagle-k20-139' } });
  assert.equal(a3.rod.id, 'stock');
});

test('the head changes the gas model\'s valves and cams; economy heads crack one intake valve', () => {
  assert.deepEqual(flowGeometry(placeholderTune()), {});
  assert.equal(flowGeometry({ engine: { head: 'K24A2', block: 'K24A2' } }).inValveM, 0.036);
  assert.equal(flowGeometry({ engine: { block: 'K20A3' } }).cams.low.inValves, 1.15);
  const z3 = wot({}, 7500), r = wot({ head: 'K20A-R', block: 'K20A' }, 7500, { vtecRpm: 5800 });
  assert.ok(r.torqueNm > z3.torqueNm, `${z3.torqueNm} -> ${r.torqueNm}`);
  const econ = wot({ block: 'K20A3' }, 7500, { vtecRpm: 2200 });
  assert.ok(econ.torqueNm < z3.torqueNm * 0.95, `${z3.torqueNm} -> ${econ.torqueNm}`);
});

test('oil pumps: the RSX Type S pump drops the balance shafts; a ported pump raises pressure', () => {
  const stock = byId(OIL_PUMPS, 'rbc'), prb = byId(OIL_PUMPS, 'prb'), ported = byId(OIL_PUMPS, '4p-ported');
  assert.equal(pumpFit(prb, byId(BLOCKS, 'K20Z3')).works, 'mods');
  assert.equal(pumpFit(stock, byId(BLOCKS, 'K24A2')).works, 'no');
  assert.ok(oilPressureKpa(3000, 100, undefined, ported) > oilPressureKpa(3000, 100, undefined, stock) * 1.2);
  assert.equal(oilPressureKpa(9500, 100, undefined, prb), oilPressureKpa(8500, 100, undefined, prb));
  assert.ok(Math.abs(balancerNm(7800) * 7800 * Math.PI / 30 - 5600) < 1);
  const a = wot({}, 6000), b = wot({ oilPump: 'prb' }, 6000);
  assert.ok(b.torqueNm > a.torqueNm + 3 && b.torqueNm < a.torqueNm + 8, `${a.torqueNm} -> ${b.torqueNm}`);
  assert.equal(bottomEnd({ engine: { oilPump: 'prb' } }).geomStock, true);
});
