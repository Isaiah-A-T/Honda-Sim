import { test } from 'node:test';
import assert from 'node:assert/strict';
import { K20, cycleAngle, pistonPinHeight, valveLift, throwAngle, stressTint, advanceCrank } from '../src/kinematics.js';

const r = K20.stroke / 2, l = K20.rodLength;

test('piston travel equals the stroke', () => {
  assert.equal(pistonPinHeight(0), r + l);
  assert.ok(Math.abs(pistonPinHeight(180) - (l - r)) < 1e-9);
});

test('firing order 1-3-4-2, 180° apart', () => {
  assert.equal(cycleAngle(1, 0), 0);
  assert.equal(cycleAngle(3, 180), 0);
  assert.equal(cycleAngle(4, 360), 0);
  assert.equal(cycleAngle(2, 540), 0);
});

test('a cylinder at TDC firing has its piston at the top', () => {
  for (const [cyl, crank] of [[1, 0], [3, 180], [4, 360], [2, 540]])
    assert.ok(Math.abs(pistonPinHeight(crank + throwAngle(cyl)) - (r + l)) < 1e-9, `cyl ${cyl}`);
});

test('valves are shut on the power stroke and open on exhaust and intake', () => {
  assert.equal(valveLift(1, 30, 'intake', false), 0);
  assert.equal(valveLift(1, 30, 'exhaust', false), 0);
  assert.ok(valveLift(1, 250, 'exhaust', false) > 9);
  assert.ok(valveLift(1, 470, 'intake', false) > 9);
  assert.ok(valveLift(1, 465, 'intake', true) > valveLift(1, 465, 'intake', false));
});

test('stress tint is neutral at zero and full red past the limit', () => {
  assert.equal(stressTint(0).amount, 0);
  assert.ok(stressTint(1.1).amount >= 0.99);
  assert.ok(stressTint(0.6).amount < stressTint(0.9).amount);
});

test('crank advances with rpm and wraps at 720°', () => {
  assert.equal(advanceCrank(0, 60, 1), 360);
  assert.equal(advanceCrank(700, 60, 0.0625), 2.5);
});

test('VTC advance moves the intake event earlier and leaves the exhaust alone', () => {
  assert.ok(Math.abs(valveLift(1, 430, 'intake', false, 40) - valveLift(1, 470, 'intake', false)) < 1e-9);
  assert.equal(valveLift(1, 250, 'exhaust', false, 40), valveLift(1, 250, 'exhaust', false));
});

test('the adapter uses the sim internals model where it has one', async () => {
  const { stateFromSim } = await import('../src/sim-adapter.js');
  const eng = { tune: { revLimit: 8000, turbo: { enabled: false } }, rpm: 6000, tps: 100, map: 100, boost: 0, highCam: true,
    ign: 30, mbt: 32, knocking: false, knockRetard: 0, lambda: 0.85, iat: 30, torqueNm: 180, airGs: 120 };
  const load = { piston: 0.5, rod: 0.4, rodBolt: 0.9, bearing: 0.3, crank: 0.2, gasket: 0.6, valvetrain: 0.7 };
  const damage = { piston: 0, rod: 0, rodBolt: 1, bearing: 0, crank: 0, gasket: 0, valvetrain: 0 };
  const s = stateFromSim({ eng, running: true, internals: { load, damage } });
  assert.equal(s.stress.pistons, 0.5);
  assert.equal(s.stress.rods, 1.3);              // rod bolt failed
  assert.equal(s.stress.valves, 0.7);
  assert.equal(s.airGs, 120);
});
