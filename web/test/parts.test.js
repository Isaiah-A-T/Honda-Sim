import { test } from 'node:test';
import assert from 'node:assert/strict';
import { bottomEnd, CRANKS, PISTONS, RODS, pistonProblem, rodProblem } from '../src/parts.js';
import { placeholderTune } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';
import { createInternals, stepInternals, engineGeometry } from '../src/internals.js';
import { renderState } from '../src/simstate.js';

function wot(parts, rpm, extra = {}) {
  const t = { ...placeholderTune(), parts, ...extra }, e = createEngine(t), s = createInternals();
  for (let x = 0; x < 1.5; x += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); stepInternals(s, e, 0.001); }
  return { e, s };
}

test('stock bottom end is 86 x 86, 1998 cc, 11.0:1, zero deck', () => {
  const b = bottomEnd(placeholderTune());
  assert.equal(b.stock, true);
  assert.equal(b.displacementL, 1.998);
  assert.equal(b.compression, 11);
  assert.equal(b.deckClearanceMm, 0);
});

test('every catalog part is real: brand, part number and a source page', () => {
  for (const p of [...PISTONS, ...RODS].filter(p => p.id !== 'stock')) {
    assert.ok(p.brand && p.sources?.length, p.id);
  }
  for (const c of CRANKS.filter(c => c.id !== 'stock')) assert.ok(c.sources?.length, c.id);
});

test('pistons keep their advertised compression in the engine they were made for', () => {
  for (const p of PISTONS.filter(p => p.forStroke_mm === 86 && p.id !== 'stock')) {
    assert.ok(Math.abs(bottomEnd({ parts: { piston: p.id } }).compression - p.compressionRatio) < 0.01, p.id);
  }
  const k24 = bottomEnd({ parts: { crank: 'honda-k24-99-hybrid', piston: 'cp-k24k20-87-14.2' } });
  assert.ok(Math.abs(k24.compression - 14.2) < 0.01);
  assert.ok(Math.abs(k24.displacementL - 2.354) < 0.01);
});

test('a longer stroke grows displacement and compression; the kit rods come with it', () => {
  const b = bottomEnd({ parts: { crank: 'bc-k20-92-stroker', rod: 'eagle-k20-139' } });
  assert.equal(b.strokeMm, 92);
  assert.equal(b.rodMm, 140);
  assert.equal(b.rod.id, 'bc-k20-stroker-140');
  assert.ok(b.displacementL > 2.13 && b.compression > 11.5);
  const toda = bottomEnd({ parts: { crank: 'toda-k20-90.7-hc-87' } });
  assert.ok(Math.abs(toda.displacementL - 2.157) < 0.005);
  assert.equal(toda.compression, 12.3);
});

test('parts that do not fit are refused with a reason', () => {
  const stock = CRANKS[0], stroker = CRANKS.find(c => c.id === 'bc-k20-92-stroker');
  assert.match(rodProblem(stock, RODS.find(r => r.id === 'bc-k20-stroker-140')), /139/);
  assert.ok(pistonProblem(stroker, PISTONS.find(p => p.id === 'je-k20-87-12.5')));
  const b = bottomEnd({ parts: { rod: 'bc-k20-stroker-140' } });
  assert.equal(b.rod.id, 'stock');
  assert.equal(b.issues.length, 1);
});

test('fitted parts bring their alloy and mass; more stroke means more rod tension at the same rpm', () => {
  const t = { parts: { crank: 'honda-k24-99-hybrid' } };
  assert.equal(engineGeometry(t).strokeM, 0.099);
  const forged = bottomEnd({ parts: { piston: 'wiseco-k20-87-9.7', rod: 'eagle-k20-139' } });
  assert.deepEqual([forged.materials.piston, forged.materials.rod], ['piston2618', 'rod4340']);
  const a = wot({}, 8000), k24 = wot(t.parts, 8000);
  assert.ok(k24.s.rodTensionN > a.s.rodTensionN * 1.1);
});

test('the bigger engine makes more torque, and the sim state shows the new geometry', () => {
  const a = wot({}, 3000, { octane: 100 }), b = wot({ crank: 'honda-k24-99-hybrid', piston: 'arias-k24k20-87-11.0' }, 3000, { octane: 100 });
  assert.ok(b.e.torqueNm > a.e.torqueNm * 1.1, `${a.e.torqueNm} -> ${b.e.torqueNm}`);
  const st = renderState({ eng: b.e, internals: b.s, running: true, car: { gear: 3, speedMs: 20 }, pedal: 1 });
  assert.equal(st.geometry.strokeM, 0.099);
  assert.equal(st.bottomEnd.block, 'K24A2');
  assert.equal(st.bottomEnd.head, 'K20Z3');
  JSON.stringify(st);
});

test('boring stock sleeves thins them; a re-sleeved block gets its wall back', () => {
  const p = { piston: 'cp-k20-90-11.0' };
  assert.ok(engineGeometry({ parts: p }).sleeveWallM < 0.0025);
  assert.equal(engineGeometry({ parts: p, materials: { sleeve: 'sleeveDuctile' } }).sleeveWallM, 0.004);
});
