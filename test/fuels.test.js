import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FUELS, fuelOf, pumpLphOf, injectorGPerMs, extraCoolingK, RAIL_KPA } from '../src/fuels.js';
import { placeholderTune, turboSetup, withBoostColumns } from '../src/k20z3.js';
import { createEngine, stepEngine } from '../src/engine.js';

function wot(tune, rpm) {
  const e = createEngine(tune);
  for (let x = 0; x < 1.5; x += 0.001) { e.rpm = rpm; stepEngine(e, 1, 0.001); }
  return e;
}

test('fuels: E85 has less energy per kg but more per kg of air, and cools the charge', () => {
  const pump = FUELS.find(f => f.id === 'pump91'), e85 = FUELS.find(f => f.id === 'e85');
  assert.ok(e85.lhvMJ < pump.lhvMJ);
  assert.ok(e85.lhvMJ / e85.stoich > pump.lhvMJ / pump.stoich);
  assert.ok(extraCoolingK(e85, 0.8) > 20);
  assert.equal(extraCoolingK(pump, 0.8), 0);
  // Old tunes carry only an octane.
  assert.equal(fuelOf({ octane: 93 }).octane, 93);
  assert.equal(fuelOf({ fuel: 'c16' }).octane, 118);
});

test('fuel pump: 20 to 1000 l/h in 10 l/h steps', () => {
  assert.equal(pumpLphOf({ fuelPumpLph: 445 }), 450);
  assert.equal(pumpLphOf({ fuelPumpLph: 420 }), 420);
  assert.equal(pumpLphOf({ fuelPumpLph: 5 }), 20);
  assert.equal(pumpLphOf({ fuelPumpLph: 2000 }), 1000);
});

test('injectors flow less as boost eats into the 50 psi across them', () => {
  const pump = FUELS[1];
  assert.ok(injectorGPerMs(440, pump, 250) < injectorGPerMs(440, pump, 100));
  assert.ok(Math.abs(injectorGPerMs(440, pump, RAIL_KPA + 101.3 - 300) - 440 * pump.density / 60000) < 1e-12);
});

test('the stock engine on 91 is unchanged; E85 makes more under boost; a small pump runs it lean', () => {
  const stock = wot(placeholderTune(), 7500), on91 = wot({ ...placeholderTune(), fuel: 'pump91' }, 7500);
  assert.ok(Math.abs(stock.torqueNm - on91.torqueNm) < 1e-9);
  const t = turboSetup(), gas = wot(t, 7500), e85 = wot({ ...t, fuel: 'e85', fuelPumpLph: 450 }, 7500);
  assert.ok(e85.torqueNm > gas.torqueNm * 1.03, `${e85.torqueNm} vs ${gas.torqueNm}`);
  const starved = wot({ ...t, fuel: 'e85', fuelPumpLph: 100 }, 7500);
  assert.ok(starved.fuelPumpPct > 100 && starved.fuelPressureKpa < RAIL_KPA && starved.lambda > 0.85);
});

test('turbo setup adds boost cells to an NA tune', () => {
  const t = turboSetup();
  assert.ok(Math.max(...t.fuelLow.cols) >= 200 && Math.max(...t.ignLow.cols) >= 200);
  const already = { rows: [1], cols: [30, 150], data: [[1, 1]] };
  assert.equal(withBoostColumns(already, 'fuel'), already);
});
