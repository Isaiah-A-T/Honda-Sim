// Engine model: runs the loaded tune the way the ECU would, and works out what the engine really does.
//
// The ECU side reads the tune tables (fuel, low/high cam ignition, VTEC switch point, rev limit,
// cam angle, and for turbo builds the wastegate duty, boost target and overboost cut).
// The physics side decides airflow (gasflow.js: valves, runners, exhaust back pressure, turbine),
// best-torque timing and knock (k20z3.js). Comparing the two is
// what makes the sim useful for testing a tune: too much timing knocks, too little loses torque,
// lean or rich mixtures change power, and VTEC in the wrong place leaves a hole in the curve.

import { clamp, lookup2, guessFuelUnits } from './tables.js';
import { ENGINE, veCorrection, mbt, knockLimit, frictionNm } from './k20z3.js';
import { FLOW, createGasflow, runCycle, settle, vtcFor } from './gasflow.js';

const AIR_DENSITY_G_PER_L = 1.184;   // 25 °C, sea level
const FUEL_LHV_J_PER_G = 44000;
const INDICATED_EFF = 0.40;
const STOICH = 14.7;
const DECEL_AIR = 4.5;                 // % opening the idle valve holds on decel so the manifold doesn't go to near-vacuum
const FUEL_DENSITY_G_PER_CC = 0.745;
const GAS_EVERY_S = 0.01;              // run one gas exchange cycle this often (results change slowly)
const P_AMB = 101.3, DOWNPIPE_KPA = 4;

const flowGeom = t => ({ ...FLOW, ...(t.flow || {}) });
// Exhaust outlet the gas model empties into: the exhaust system, or the turbine plus the open part of the wastegate.
function exhaustOutlet(e) {
  const tb = e.tune.turbo, G = e.gas.G;
  if (tb?.enabled && !tb.fixed) return { outKpa: P_AMB + DOWNPIPE_KPA, outAreaM2: (tb.turbineAreaCm2 + tb.wastegateAreaCm2 * e.wgOpen) * 1e-4 };
  return { outKpa: P_AMB, outAreaM2: Math.PI / 4 * G.exhaustDiaM ** 2 };
}
function gasConditions(e) {
  const t = e.tune, cut = e.revCut || e.dfco || e.boostCut;
  e.vtc = vtcFor(t, e.rpm, e.map, e.highCam);
  return { rpm: Math.max(e.rpm, 300), mapKpa: e.map, iatC: e.iat, highCam: e.highCam, vtc: e.vtc, ignDeg: e.ign,
    lambda: Math.min(e.lambda, 3), fire: !cut && e.rpm > 200, ...exhaustOutlet(e) };
}
function updateGas(e, dt, force = false) {
  if (!e.gas || e.gasFlowRef !== e.tune.flow) { e.gas = createGasflow(flowGeom(e.tune)); e.gasFlowRef = e.tune.flow; force = true; }
  e.gasT += dt;
  if (!force && e.gasT < GAS_EVERY_S) return;
  e.gasT = 0;
  e.gasOut = force ? settle(e.gas, gasConditions(e), 12) : runCycle(e.gas, gasConditions(e));
}

export function createEngine(tune) {
  const e = {
    tune,
    rpm: ENGINE.idleRpm,
    map: 30,
    tps: 0,
    idleAir: 2, idleI: 2,   // % throttle the idle controller adds (output, integral)
    highCam: false,
    revCut: false, dfco: false,
    ign: 10, ignTable: 10, knockRetard: 0, knockLimit: 30, knocking: false, knockCount: 0, knockAcc: 0,
    lambda: 1, lambdaTarget: 1, afr: STOICH, ipw: 2,
    torqueNm: 0, airGs: 0, airDeliveredGs: 0,
    iat: 25, ect: 90,
    boost: 0, boostTarget: NaN, wgDuty: 0, wgOpen: 0, turboRadS: 0, turbineKw: 0, compressorKw: 0, boostCut: false, overTorque: false,
    gas: null, gasOut: null, gasT: 0, gasFlowRef: undefined, vtc: 0, pmepKpa: 0, exhKpa: P_AMB, egtC: 600,
  };
  updateGas(e, 0, true);
  return e;
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
  } else e.idleAir += ((pedal < 0.02 ? DECEL_AIR : e.idleI) - e.idleAir) * Math.min(1, dt * 2);
  e.tps = clamp(pedal * 100 + (pedal < 0.02 ? e.idleAir : 0), 0, 100);

  // Turbo. The turbine takes energy out of the exhaust: flow × temperature × the pressure drop from the
  // manifold (from the gas exchange model) to the downpipe. The compressor needs power to push air at
  // the boost its wheel speed makes. Any difference speeds the shaft up or slows it down. The wastegate
  // actuator opens once boost passes the spring (plus whatever solenoid duty the tune adds), letting
  // exhaust bypass the turbine, which is what holds boost at target.
  const tb = t.turbo;
  e.boostTarget = t.boostTarget ? lookup2(t.boostTarget, rpm, e.map) : NaN;
  e.wgDuty = 0;
  if (tb?.enabled) {
    const spring = Math.min(tb.springKpa, tb.maxBoostKpa);
    e.wgDuty = t.wgDuty ? clamp(lookup2(t.wgDuty, rpm, e.map), 0, 100) : 0;
    // Duty → boost depends on the solenoid and wastegate; wgGain scales it (1 = full duty reaches max boost).
    const wastegateLimit = spring + clamp(e.wgDuty / 100 * (tb.wgGain ?? 1), 0, 1) * (tb.maxBoostKpa - spring);
    if (tb.fixed) {                                       // supercharger, or a turbo assumed to hold its target
      const want = e.boostCut || e.revCut || e.dfco ? 0 : wastegateLimit;
      e.boost += (want - e.boost) * Math.min(1, dt * 20);
    } else {
      const g = e.gasOut, airKgS = e.airDeliveredGs / 1000;
      const exhKgS = airKgS * (1 + (e.lambda < 5 ? 1 / (STOICH * e.lambda) : 0));
      const share = tb.turbineAreaCm2 / (tb.turbineAreaCm2 + tb.wastegateAreaCm2 * e.wgOpen);
      const p4 = P_AMB + DOWNPIPE_KPA, er = g.exhKpa > p4 ? 1 - (p4 / g.exhKpa) ** 0.248 : 0;
      const turbineW = tb.turbineEff * exhKgS * share * 1150 * g.exhTempK * er;
      const pr = (P_AMB + e.boost) / (P_AMB - 1);          // 1 kPa lost in the air filter
      const compressorW = airKgS * 1005 * 298 * (pr ** 0.2857 - 1) / tb.compressorEff;
      const w = Math.max(e.turboRadS, 500), I = tb.inertiaKgM2;
      // Pressure ratio the wheel makes at a shaft speed (ideal work coefficient ~0.7 at the tip), and the
      // speed where this compressor runs off its map at maxBoostKpa (it can't spin faster usefully).
      const prAt = radS => (1 + 0.7 * tb.compressorEff * (radS * tb.compressorTipMm / 2000) ** 2 / (1005 * 298)) ** 3.5;
      const prMax = (P_AMB + tb.maxBoostKpa) / (P_AMB - 1);
      const maxRadS = Math.sqrt((prMax ** (1 / 3.5) - 1) * 1005 * 298 / (0.7 * tb.compressorEff)) * 2000 / tb.compressorTipMm;
      e.turboRadS = clamp(e.turboRadS + (turbineW - compressorW - 2e-6 * w * w) / (I * w) * dt, 0, maxRadS);
      const prWheel = prAt(e.turboRadS);
      e.boost = clamp((P_AMB - 1) * prWheel - P_AMB, 0, tb.maxBoostKpa);
      const open = clamp((e.boost - wastegateLimit) / 6, 0, 1);
      e.wgOpen += (open - e.wgOpen) * Math.min(1, dt * 40);
      e.turbineKw = turbineW / 1000; e.compressorKw = compressorW / 1000;
    }
  } else { e.boost = 0; e.turboRadS = 0; }
  e.iat += ((25 + e.boost * 0.12) - e.iat) * Math.min(1, dt * 0.3);   // intercooled: some heat soak on boost

  // Manifold pressure: where airflow through the throttle balances what the engine pumps out.
  updateGas(e, dt);
  const corr = veCorrection(rpm, e.highCam), g = e.gasOut;
  const mapTarget = manifoldPressure(e.tps, rpm, g.veDelivered * corr, 101.3 + e.boost, e.iat, flowGeom(t).throttleDiaM);
  e.map += (mapTarget - e.map) * Math.min(1, dt * 12);

  // VTEC: switch to high cam above the set rpm at load, with a little hysteresis.
  if (!e.highCam && rpm >= t.vtecRpm && e.map >= t.vtecMinMap) e.highCam = true;
  else if (e.highCam && (rpm < t.vtecRpm - 150 || e.map < t.vtecMinMap - 5)) e.highCam = false;

  // Decel fuel cut, then the rev limiter (also a fuel cut).
  e.dfco = pedal < 0.02 && rpm > 1500;
  if (rpm > t.revLimit) e.revCut = true;
  else if (rpm < t.revLimit - (t.revLimitHyst ?? 200)) e.revCut = false;
  if (t.overboostKpa && e.map > t.overboostKpa) e.boostCut = true;
  else if (e.map < (t.overboostKpa ?? 0) - 20) e.boostCut = false;

  // Air the engine actually traps this cycle (grams per 2 revolutions, whole engine), from the gas
  // exchange model, plus what passes straight through during valve overlap.
  const ve = g.ve * corr;
  e.ve = ve;
  const density = ENGINE.displacementL * AIR_DENSITY_G_PER_L * (e.map / 101.3) * (298 / (273 + e.iat));
  const airPerCycle = ve * density;
  e.airGs = airPerCycle * rpm / 120;
  e.airDeliveredGs = Math.max(g.veDelivered * corr, ve) * density * rpm / 120;
  e.pmepKpa = g.pmepKpa; e.exhKpa = g.exhKpa; e.egtC = g.egtC; e.residual = g.residual; e.cylPmaxBar = g.pmaxBar;

  // Fuel the ECU commands from the tune.
  const fuelTable = (e.highCam && t.fuelHigh) || t.fuelLow;
  const units = t.fuelUnits || guessFuelUnits(fuelTable);
  const fuelVal = lookup2(fuelTable, rpm, e.map);
  const injGPerMs = (t.injectorCc ?? ENGINE.injectorCcMin) * FUEL_DENSITY_G_PER_CC / 60000;
  const deadMs = t.injectorDeadMs ?? ENGINE.injectorDeadMs;
  let fuelPerCyl;
  if (units === 'ms') {
    e.ipw = fuelVal;
    fuelPerCyl = Math.max(0, fuelVal - deadMs) * injGPerMs;
    e.lambdaTarget = NaN;
  } else {
    e.lambdaTarget = units === 'afr' ? fuelVal / STOICH : fuelVal;
    fuelPerCyl = airPerCycle / ENGINE.cylinders / (STOICH * e.lambdaTarget);
    e.ipw = fuelPerCyl / injGPerMs + deadMs;
  }
  const cycleMs = 120000 / rpm;
  e.injDuty = clamp(e.ipw / cycleMs * 100, 0, 100);
  if (e.ipw > cycleMs) fuelPerCyl = (cycleMs - deadMs) * injGPerMs;   // injectors maxed out
  if (e.revCut || e.dfco || e.boostCut) fuelPerCyl = 0;
  const fuel = fuelPerCyl * ENGINE.cylinders;
  e.lambda = fuel > 0 ? airPerCycle / (fuel * STOICH) : 9.99;
  e.afr = e.lambda * STOICH;

  // Ignition from the tune, minus knock retard; compare to what the engine wants.
  e.ignTable = lookup2((e.highCam && t.ignHigh) || t.ignLow, rpm, e.map);
  e.ign = e.ignTable - e.knockRetard;
  const best = mbt(rpm, e.map);
  e.knockLimit = knockLimit(rpm, e.map, e.iat, t.octane ?? 91) + (e.lambda < 0.9 ? (0.9 - e.lambda) * 10 : 0);
  const over = e.ign - e.knockLimit;
  e.knocking = !e.revCut && !e.dfco && !e.boostCut && over > 0 && e.map > 50;
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
  // Losses: mechanical friction, and the work spent pumping air in and exhaust out (gas exchange model).
  e.pumpingNm = e.pmepKpa * ENGINE.displacementL / (4 * Math.PI);
  e.torqueNm = indicated - frictionNm(rpm) - e.pumpingNm;
  e.overTorque = e.torqueNm > (t.torqueLimitNm ?? Infinity);
  e.sparkEff = sparkEff;
  e.mbt = best;
  return e.torqueNm;
}

// Throttle body: 62 mm bore by default (estimate; tune.flow.throttleDiaM changes it), plate closed at 7°, discharge coefficient 0.75, plus a small
// leak past the closed plate. Flow through it is compressible orifice flow, choked below ~53% of the
// upstream pressure. The engine pumps VE × displacement × rpm/120 of manifold air.
const THROTTLE_D_M = 0.062, PLATE_CLOSED_DEG = 7, THROTTLE_CD = 0.75, LEAK_M2 = 8e-6, R_AIR = 287, GAMMA = 1.4;
export function throttleAreaM2(tps, diaM = THROTTLE_D_M) {
  const full = Math.PI / 4 * diaM ** 2;
  const a = (PLATE_CLOSED_DEG + clamp(tps, 0, 100) / 100 * (90 - PLATE_CLOSED_DEG)) * Math.PI / 180;
  return LEAK_M2 + full * (1 - Math.cos(a) / Math.cos(PLATE_CLOSED_DEG * Math.PI / 180));
}
function orificeFlow(areaM2, upKpa, downKpa, tempK) {
  const pr = clamp(downKpa / upKpa, 0, 1);
  const crit = (2 / (GAMMA + 1)) ** (GAMMA / (GAMMA - 1));
  const psi = pr <= crit
    ? Math.sqrt(GAMMA * (2 / (GAMMA + 1)) ** ((GAMMA + 1) / (GAMMA - 1)))
    : Math.sqrt(2 * GAMMA / (GAMMA - 1) * (pr ** (2 / GAMMA) - pr ** ((GAMMA + 1) / GAMMA)));
  return THROTTLE_CD * areaM2 * upKpa * 1000 / Math.sqrt(R_AIR * tempK) * psi;     // kg/s
}
export function manifoldPressure(tps, rpm, ve, upstreamKpa, iatC = 25, throttleDiaM = THROTTLE_D_M) {
  const T = 273 + iatC, area = throttleAreaM2(tps, throttleDiaM);
  const pumped = p => ve * ENGINE.displacementL / 1000 * Math.max(rpm, 1) / 120 * p * 1000 / (R_AIR * T);
  let lo = 0.5, hi = upstreamKpa;
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (orificeFlow(area, upstreamKpa, mid, T) > pumped(mid)) lo = mid; else hi = mid;
  }
  return (lo + hi) / 2;
}

export const hp = (torqueNm, rpm) => torqueNm * rpm / 7120.9;   // metric Nm·rpm → hp
