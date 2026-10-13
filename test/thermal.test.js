import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createSim, step, coldEngine } from '../src/sim.js';
import { createThermal, stepThermal, viscosityCst, oilPressureKpa, COOLING, circuits } from '../src/thermal.js';
import { stepEngine } from '../src/engine.js';
import { renderState } from '../src/simstate.js';

const run = (sim, input, seconds) => { for (let t = 0; t < seconds; t += 0.01) step(sim, input, 0.01); };

test('oil viscosity passes through the grade points and thins as it heats', () => {
  assert.ok(Math.abs(viscosityCst(40) - COOLING.oilVisc40) < 1e-6);
  assert.ok(Math.abs(viscosityCst(100) - COOLING.oilVisc100) < 1e-6);
  assert.ok(viscosityCst(0) > viscosityCst(40) * 4);
});

test('cold oil raises oil pressure up to the relief valve', () => {
  const hot = oilPressureKpa(3000, 100), cold = oilPressureKpa(3000, 10);
  assert.ok(cold > hot * 1.5);
  assert.ok(oilPressureKpa(7000, -10) <= COOLING.oilReliefKpa + 1e-9);
});

test('a cold engine warms up with the thermostat shut, then regulates on the highway', () => {
  const th = createThermal(20, false);
  const load = { rpm: 2500, fuelGs: 2.2, frictionW: 4000, heatLoss: 0.3, speedMs: 27, mapKpa: 55, ambientC: 20 };
  let shutWhileCold = true;
  for (let t = 0; t < 1500; t += 0.05) {
    stepThermal(th, load, 0.05);
    if (th.coolantC < COOLING.thermostatOpenC - 8 && th.thermostat > 0.01) shutWhileCold = false;
  }
  assert.ok(shutWhileCold, 'thermostat opened on a cold engine');
  assert.ok(th.coolantC > 80 && th.coolantC < 98, `ECT ${th.coolantC}`);
  assert.ok(th.oilC > 80 && th.oilC < 115, `oil ${th.oilC}`);
  assert.ok(th.thermostat > 0.02 && th.thermostat < 1);
  assert.ok(!th.fanOn);
});

test('fans come on when stationary under load and switch off with hysteresis', () => {
  const th = createThermal(30, true);
  const stand = { rpm: 3000, fuelGs: 4, frictionW: 5000, heatLoss: 0.3, speedMs: 0, mapKpa: 70, ambientC: 30 };
  let on = false;
  for (let t = 0; t < 600 && !on; t += 0.05) { stepThermal(th, stand, 0.05); on = th.fanOn; }
  assert.ok(on, 'fans never came on');
  assert.ok(th.coolantC > COOLING.thermostatFullC);
  stepThermal(th, { ...stand, fuelGs: 0, frictionW: 0 }, 0.05);
  assert.ok(th.fanOn, 'fans dropped out right away instead of at the off point');
});

test('the coolant boils with no airflow at full load', () => {
  const th = createThermal(30, true);
  for (let t = 0; t < 300; t += 0.05) stepThermal(th, { rpm: 7000, fuelGs: 14, frictionW: 20000, heatLoss: 0.3, speedMs: 0, mapKpa: 100, ambientC: 30 }, 0.05);
  assert.ok(th.boiling && th.overheatS > 0);
});

test('a water oil cooler pulls oil temperature toward coolant temperature', () => {
  const load = { rpm: 6500, fuelGs: 10, frictionW: 15000, heatLoss: 0.3, speedMs: 40, mapKpa: 100, ambientC: 25 };
  const plain = createThermal(25, true, { oilCooler: 'none' }), cooled = createThermal(25, true, { oilCooler: 'water' });
  for (let t = 0; t < 600; t += 0.05) { stepThermal(plain, load, 0.05); stepThermal(cooled, load, 0.05); }
  assert.ok(cooled.oilC < plain.oilC - 3, `${cooled.oilC} vs ${plain.oilC}`);
  assert.ok(circuits(cooled).oil.some(s => s.to === 'oilCooler'));
  assert.ok(!circuits(plain).oil.some(s => s.to === 'oilCooler'));
});

test('a cold engine holds VTEC off, runs more friction and higher oil pressure', () => {
  const warm = createSim(), cold = createSim();
  coldEngine(cold);
  for (const s of [warm, cold]) for (let i = 0; i < 500; i++) { s.eng.rpm = 6500; stepEngine(s.eng, 1, 0.001); }
  assert.ok(cold.eng.ect < 40);
  assert.ok(cold.eng.frictionNm > warm.eng.frictionNm);
  assert.ok(cold.eng.oilKpa >= warm.eng.oilKpa);
  assert.ok(!cold.eng.vtecCmd && warm.eng.vtecCmd);
});

test('renderState exposes the cooling and oil circuits', () => {
  const s = createSim();
  run(s, { throttle: 0.2 }, 1);
  const c = renderState(s).cooling;
  assert.ok(c.coolantC > 0 && c.oil.tempC > 0 && c.oil.pressureKpa > 0);
  const names = new Set(c.coolantCircuit.flatMap(x => [x.from, x.to]));
  for (const n of ['pump', 'blockJacket', 'headJacket', 'thermostat', 'radiator']) assert.ok(names.has(n), n);
  assert.ok(c.oilCircuit.some(x => x.from === 'sump'));
  JSON.stringify(c);
});

test('turbo oil feed and drain flow while running; a water-cooled turbo heat-soaks less after shutdown', () => {
  const soak = waterCooled => {
    const th = createThermal(25, true);
    const run = { rpm: 6500, fuelGs: 14, frictionW: 15000, heatLoss: 0.3, speedMs: 40, mapKpa: 180, ambientC: 25,
      turbo: { gasGs: 220, egtC: 950, oilKpa: 500, waterCooled } };
    for (let t = 0; t < 120; t += 0.05) stepThermal(th, run, 0.05);
    assert.ok(th.turboOilLpm > 0.5 && th.turboOilKw > 0);
    const lines = circuits(th, 500, run.turbo);
    assert.ok(lines.oil.some(s => s.to === 'turbo') && lines.oil.some(s => s.from === 'turbo' && s.to === 'sump'));
    assert.equal(lines.coolant.some(s => s.to === 'turbo'), waterCooled);
    let peak = 0;
    const off = { ...run, rpm: 0, fuelGs: 0, frictionW: 0, speedMs: 0, turbo: { ...run.turbo, gasGs: 0, oilKpa: 0 } };
    for (let t = 0; t < 600; t += 0.05) { stepThermal(th, off, 0.05); peak = Math.max(peak, th.turboBearingC); }
    assert.equal(th.turboOilLpm, 0);
    return peak;
  };
  const water = soak(true), oilOnly = soak(false);
  assert.ok(oilOnly > water + 40, `oil-only ${oilOnly} vs water-cooled ${water}`);
  assert.ok(!circuits(createThermal(25, true)).oil.some(s => s.to === 'turbo'));
});
