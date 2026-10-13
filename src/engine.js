// Engine model: runs the loaded tune the way the ECU would, and works out what the engine really does.
//
// The ECU side reads the tune tables (fuel, low/high cam ignition, VTEC switch point, rev limit).
// The physics side (k20z3.js) decides airflow, best-torque timing and knock. Comparing the two is
// what makes the sim useful for testing a tune: too much timing knocks, too little loses torque,
// lean or rich mixtures change power, and VTEC in the wrong place leaves a hole in the curve.

import { clamp, lookup2, guessFuelUnits } from './tables.js';
import { ENGINE, trueVE, mbt, knockLimit, frictionNm } from './k20z3.js';

const AIR_DENSITY_G_PER_L = 1.184;   // 25 °C, sea level
const FUEL_LHV_J_PER_G = 44000;
const INDICATED_EFF = 0.40;
const STOICH = 14.7;
const FUEL_DENSITY_G_PER_CC = 0.745;

export function createEngine(tune) {
  return {
    tune,
    rpm: ENGINE.idleRpm,
    map: 30,
    tps: 0,
    idleAir: 2, idleI: 2,   // % throttle the idle controller adds (output, integral)
    highCam: false,
    revCut: false, dfco: false,
    ign: 10, ignTable: 10, knockRetard: 0, knockLimit: 30, knocking: false, knockCount: 0, knockAcc: 0,
    lambda: 1, lambdaTarget: 1, afr: STOICH, ipw: 2,
    torqueNm: 0, airGs: 0,
    iat: 25, ect: 90,
  };
}

// Advance the engine state by dt seconds. pedal is 0–1. Returns brake torque at the crank (Nm).
export function stepEngine(e, pedal, dt) {
  const t = e.tune;
  const rpm = Math.max(e.rpm, 1);

  // Idle control: hold idle rpm when the pedal is up and the engine is near idle.
  const idleErr = ENGINE.idleRpm - rpm;
  if (pedal < 0.02 && rpm < ENGINE.idleRpm + 600) {
    e.idleI = clamp(e.idleI + idleErr * 0.004 * dt, 0, 10);
    e.idleAir = clamp(e.idleI + idleErr * 0.006, 0, 15);
  } else e.idleAir += (e.idleI - e.idleAir) * Math.min(1, dt * 2);
  e.tps = clamp(pedal * 100 + (pedal < 0.02 ? e.idleAir : 0), 0, 100);

  // Manifold pressure follows throttle and rpm (more rpm pulls the manifold down at part throttle).
  const k = 0.06 * Math.sqrt(3000 / Math.max(rpm, 600));
  const mapTarget = 12 + 88 * (1 - Math.exp(-e.tps * k));
  e.map += (mapTarget - e.map) * Math.min(1, dt * (mapTarget > e.map ? 6 : 9));

  // VTEC: switch to high cam above the set rpm at load, with a little hysteresis.
  if (!e.highCam && rpm >= t.vtecRpm && e.map >= t.vtecMinMap) e.highCam = true;
  else if (e.highCam && (rpm < t.vtecRpm - 150 || e.map < t.vtecMinMap - 5)) e.highCam = false;

  // Decel fuel cut, then the rev limiter (also a fuel cut).
  e.dfco = pedal < 0.02 && rpm > 1500;
  if (rpm > t.revLimit) e.revCut = true;
  else if (rpm < t.revLimit - (t.revLimitHyst ?? 200)) e.revCut = false;

  // Air the engine actually breathes this cycle (grams per 2 revolutions, whole engine).
  const ve = trueVE(rpm, e.highCam);
  const airPerCycle = ve * ENGINE.displacementL * AIR_DENSITY_G_PER_L * (e.map / 101.3) * (298 / (273 + e.iat));
  e.airGs = airPerCycle * rpm / 120;

  // Fuel the ECU commands from the tune.
  const fuelTable = e.highCam ? t.fuelHigh : t.fuelLow;
  const units = t.fuelUnits || guessFuelUnits(fuelTable);
  const fuelVal = lookup2(fuelTable, rpm, e.map);
  const injGPerMs = ENGINE.injectorCcMin * FUEL_DENSITY_G_PER_CC / 60000;
  let fuelPerCyl;
  if (units === 'ms') {
    e.ipw = fuelVal;
    fuelPerCyl = Math.max(0, fuelVal - ENGINE.injectorDeadMs) * injGPerMs;
    e.lambdaTarget = NaN;
  } else {
    e.lambdaTarget = units === 'afr' ? fuelVal / STOICH : fuelVal;
    fuelPerCyl = airPerCycle / ENGINE.cylinders / (STOICH * e.lambdaTarget);
    e.ipw = fuelPerCyl / injGPerMs + ENGINE.injectorDeadMs;
  }
  const cycleMs = 120000 / rpm;
  e.injDuty = clamp(e.ipw / cycleMs * 100, 0, 100);
  if (e.ipw > cycleMs) fuelPerCyl = (cycleMs - ENGINE.injectorDeadMs) * injGPerMs;   // injectors maxed out
  if (e.revCut || e.dfco) fuelPerCyl = 0;
  const fuel = fuelPerCyl * ENGINE.cylinders;
  e.lambda = fuel > 0 ? airPerCycle / (fuel * STOICH) : 9.99;
  e.afr = e.lambda * STOICH;

  // Ignition from the tune, minus knock retard; compare to what the engine wants.
  e.ignTable = lookup2(e.highCam ? t.ignHigh : t.ignLow, rpm, e.map);
  e.ign = e.ignTable - e.knockRetard;
  const best = mbt(rpm, e.map);
  e.knockLimit = knockLimit(rpm, e.map, e.iat, t.octane ?? 91) + (e.lambda < 0.9 ? (0.9 - e.lambda) * 10 : 0);
  const over = e.ign - e.knockLimit;
  e.knocking = !e.revCut && !e.dfco && over > 0 && e.map > 50;
  if (e.knocking) {
    e.knockAcc += dt * (1 + over);
    if (e.knockAcc > 0.1) { e.knockAcc = 0; e.knockCount++; e.knockRetard = Math.min(e.knockRetard + 1, 8); }
  } else e.knockRetard = Math.max(0, e.knockRetard - dt * 0.5);

  // Torque: energy from the fuel that burns, scaled by spark and mixture efficiency, minus losses.
  const burned = Math.min(fuel, airPerCycle / STOICH);
  const sparkEff = clamp(1 - ((e.ign - best) / 28) ** 2, 0.2, 1);
  const richBonus = e.lambda < 1 ? clamp(1 + 0.045 * (1 - ((e.lambda - 0.87) / 0.13) ** 2), 0.85, 1.045) : 1;
  const leanMisfire = e.lambda > 1.35 ? clamp(1 - (e.lambda - 1.35) * 4, 0, 1) : 1;
  const loadEff = 0.75 + 0.25 * Math.min(e.map, 100) / 100;          // part load burns less efficiently
  const indicated = burned * FUEL_LHV_J_PER_G * INDICATED_EFF * loadEff * sparkEff * richBonus * leanMisfire / (4 * Math.PI);
  e.torqueNm = indicated - frictionNm(rpm, e.map);
  e.sparkEff = sparkEff;
  e.mbt = best;
  return e.torqueNm;
}

export const hp = (torqueNm, rpm) => torqueNm * rpm / 7120.9;   // metric Nm·rpm → hp
