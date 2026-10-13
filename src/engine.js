// Engine model: runs the loaded tune the way the ECU would, and works out what the engine really does.
//
// The ECU side reads the tune tables (fuel, low/high cam ignition, VTEC switch point, rev limit,
// cam angle, and for turbo builds the wastegate duty, boost target and overboost cut).
// The physics side decides airflow (gasflow.js: valves, runners, exhaust back pressure, turbine),
// best-torque timing and knock (k20z3.js). Comparing the two is
// what makes the sim useful for testing a tune: too much timing knocks, too little loses torque,
// lean or rich mixtures change power, and VTEC in the wrong place leaves a hole in the curve.

import { clamp, lookup2, guessFuelUnits } from './tables.js';
import { ENGINE, TURBO_GT35, veCorrection, mbt, frictionNm } from './k20z3.js';
import { FLOW, createGasflow, runCycle, settle, vtcFor, valveLiftM } from './gasflow.js';

const AIR_DENSITY_G_PER_L = 1.184;   // 25 °C, sea level
const STOICH = 14.7;
const DECEL_AIR = 4.5;                 // % opening the idle valve holds on decel so the manifold doesn't go to near-vacuum
const FUEL_DENSITY_G_PER_CC = 0.745;
const GAS_EVERY_S = 0.01;              // run one gas exchange cycle this often (results change slowly)
const P_AMB = 101.3, DOWNPIPE_KPA = 4;

const flowGeom = t => ({ ...FLOW, ...(t.flow || {}) });
// Turbo settings, with the GT35 defaults filling anything a saved tune from an older version lacks.
const turboOf = t => (t.turbo?.enabled ? { ...TURBO_GT35, ...t.turbo } : t.turbo);
// Exhaust outlet the gas model empties into: the exhaust system, or the turbine plus the open part of the wastegate.
function exhaustOutlet(e) {
  const tb = turboOf(e.tune), G = e.gas.G;
  if (tb?.enabled && !tb.fixed) return { outKpa: P_AMB + DOWNPIPE_KPA, outAreaM2: (tb.turbineAreaCm2 + tb.wastegateAreaCm2 * e.wgOpen) * 1e-4 };
  return { outKpa: P_AMB, outAreaM2: Math.PI / 4 * G.exhaustDiaM ** 2 };
}
function gasConditions(e, high) {
  const t = e.tune, cut = e.revCut || e.dfco || e.boostCut;
  const vtc = vtcFor(t, e.rpm, e.map, e.highCam);
  e.vtc = vtc;
  return { rpm: Math.max(e.rpm, 300), mapKpa: e.map, iatC: e.iat, highCam: high, vtc, ignDeg: e.ign,
    lambda: Math.min(e.lambda, 3), fire: !cut && e.rpm > 200, octane: t.octane ?? 91, ...exhaustOutlet(e) };
}
const blend = (a, b, f) => Object.fromEntries(Object.keys(a).map(k => [k, Number.isFinite(a[k]) && Number.isFinite(b[k]) ? a[k] + (b[k] - a[k]) * f : (f < 0.5 ? a[k] : b[k])]));
// One gas state per cam lobe set. While VTEC is switching, some cylinders are on each, so both run and
// their results are mixed by the share of cylinders locked to the VTEC lobe.
function updateGas(e, dt, force = false) {
  if (!e.gas || e.gasFlowRef !== e.tune.flow) {
    e.gas = createGasflow(flowGeom(e.tune)); e.gasHi = createGasflow(flowGeom(e.tune)); e.gasFlowRef = e.tune.flow; force = true;
  }
  e.gasT += dt;
  if (!force && e.gasT < GAS_EVERY_S) return;
  e.gasT = 0;
  const f = e.camF;
  const run = (st, high, idle) => (force || idle > 0.2 ? settle(st, gasConditions(e, high), force ? 12 : 4) : runCycle(st, gasConditions(e, high)));
  if (f < 1) { e.gasLoOut = run(e.gas, false, e.loIdle); e.loIdle = 0; } else e.loIdle += GAS_EVERY_S;
  if (f > 0) { e.gasHiOut = run(e.gasHi, true, e.hiIdle); e.hiIdle = 0; } else e.hiIdle += GAS_EVERY_S;
  e.gasOut = f <= 0 ? e.gasLoOut : f >= 1 ? e.gasHiOut : blend(e.gasLoOut, e.gasHiOut, f);
}

// VTEC actuation. The ECU opens the VTEC solenoid (spool valve) when rpm, load and coolant temperature
// allow; engine oil pressure then pushes the lock pins through each cylinder's rockers, locking them to
// the VTEC lobe. A pin can only slide while its rockers sit on the cam's base circle (valves closed),
// so each cylinder switches at its own point in the firing order. The ECU moves to the high-cam fuel
// and ignition tables once the oil pressure switch confirms pressure. Dropping out works the same way
// in reverse: return springs push the pins back, again only on the base circle.
const VTEC_OIL_KPA = 250;          // oil pressure needed to move the pins (estimate)
const PIN_TRAVEL_S = 0.04;         // time for oil to stroke a pin once it's free to move (estimate)
const CYL_OFFSET = [0, 540, 180, 360];   // firing TDC of each cylinder after cylinder 1 (order 1-3-4-2)
function oilPressureKpa(rpm, ectC) {
  const warm = clamp((ectC - 20) / 70, 0, 1);
  return clamp(80 + rpm * 0.065, 0, 600) * (1.25 - 0.25 * warm);   // hot oil is thinner (estimate shape)
}
function stepVtec(e, dt) {
  const t = e.tune, rpm = e.rpm, G = e.gas.G;
  const want = rpm >= t.vtecRpm && e.map >= t.vtecMinMap && (e.ect ?? 90) >= 60;
  // Hysteresis: once on, stay on until rpm or load fall clearly below the switch point.
  e.vtecCmd = e.vtecCmd ? rpm >= t.vtecRpm - 150 && e.map >= t.vtecMinMap - 5 : want;
  e.oilKpa = oilPressureKpa(rpm, e.ect ?? 90);
  const pressurised = e.vtecCmd && e.oilKpa >= VTEC_OIL_KPA;
  for (let i = 0; i < 4; i++) {
    const theta = e.crankDeg - CYL_OFFSET[i];
    const onBase = valveLiftM(theta, true, false, e.vtc ?? 0, G) === 0 && valveLiftM(theta, true, true, e.vtc ?? 0, G) === 0
      && valveLiftM(theta, false, false, 0, G) === 0 && valveLiftM(theta, false, true, 0, G) === 0;
    if (onBase || (e.vtecPins[i] > 0 && e.vtecPins[i] < 1)) {   // a pin already moving finishes its stroke
      e.vtecPins[i] = clamp(e.vtecPins[i] + (pressurised ? 1 : -1) * dt / PIN_TRAVEL_S, 0, 1);
    }
  }
  e.camF = e.vtecPins.filter(p => p >= 1).length / 4;
  // ECU table choice follows the oil pressure switch.
  e.highCam = pressurised;
}

export function createEngine(tune) {
  const e = {
    tune,
    rpm: ENGINE.idleRpm,
    map: 30,
    tps: 0,
    idleAir: 2, idleI: 2,   // % throttle the idle controller adds (output, integral)
    highCam: false, vtecCmd: false, vtecPins: [0, 0, 0, 0], camF: 0, oilKpa: 0, crankDeg: 0,
    gasHi: null, gasLoOut: null, gasHiOut: null, loIdle: 0, hiIdle: 1,
    revCut: false, dfco: false,
    ign: 10, ignTable: 10, knockRetard: 0, knockLimit: 30, knocking: false, knockCount: 0, knockAcc: 0,
    lambda: 1, lambdaTarget: 1, afr: STOICH, ipw: 2,
    torqueNm: 0, airGs: 0, airDeliveredGs: 0,
    iat: 25, ect: 90,
    boost: 0, icDropKpa: 0, chargeOutC: 25, boostTarget: NaN, wgDuty: 0, wgOpen: 0, turboRadS: 0, turbineKw: 0, compressorKw: 0, boostCut: false, overTorque: false,
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
  const tb = turboOf(t);
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
  // Charge air. The compressor heats the air it squeezes (isentropic rise ÷ compressor efficiency).
  // An intercooler takes most of that heat back out (effectiveness) for a pressure drop that grows with
  // airflow; without one the hot charge goes straight to the throttle body. N/A engines breathe ambient air
  // plus a little underhood heat. The intake air temperature then lags as the pipes heat up and cool down.
  const ambC = t.ambientC ?? 25;
  let chargeC = ambC + 3, icDropKpa = 0;
  if (tb?.enabled && e.boost > 0.5) {
    const prC = (P_AMB + e.boost) / (P_AMB - 1);
    const compOutC = (ambC + 273) * (1 + (prC ** 0.2857 - 1) / (tb.compressorEff ?? 0.7)) - 273;
    const ic = tb.intercooler !== false;
    chargeC = ic ? compOutC - (tb.icEffectiveness ?? 0.75) * (compOutC - ambC) : compOutC;
    if (ic) icDropKpa = (tb.icDropKpaAt250Gs ?? 8) * (e.airDeliveredGs / 250) ** 2;
  }
  e.chargeOutC = chargeC; e.icDropKpa = icDropKpa;
  e.iat += (chargeC - e.iat) * Math.min(1, dt * 0.5);

  // Manifold pressure: where airflow through the throttle balances what the engine pumps out.
  updateGas(e, dt);
  const corr = veCorrection(rpm, false) + (veCorrection(rpm, true) - veCorrection(rpm, false)) * e.camF, g = e.gasOut;
  const mapTarget = manifoldPressure(e.tps, rpm, g.veDelivered * corr, 101.3 + e.boost - e.icDropKpa, e.iat, flowGeom(t).throttleDiaM);
  e.map += (mapTarget - e.map) * Math.min(1, dt * 12);

  e.crankDeg = (e.crankDeg + rpm * 6 * dt) % 720;
  stepVtec(e, dt);

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
  e.pmepKpa = g.pmepKpa; e.exhKpa = g.exhKpa; e.egtC = g.egtC; e.residual = g.residual;

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

  // Ignition from the tune, minus knock retard. Timing works through the gas exchange model: it sets
  // when the burn starts, so it decides combustion phasing, peak pressure, exhaust temperature, torque
  // and whether the end gas autoignites (knock).
  e.ignTable = lookup2((e.highCam && t.ignHigh) || t.ignLow, rpm, e.map);
  e.ign = e.ignTable - e.knockRetard;
  e.mbt = mbt(rpm, e.map);
  const fired = !e.revCut && !e.dfco && !e.boostCut;
  e.knockIndex = fired ? g.knockIndex : 0;
  // How far past the knock limit this is: the knock integral grows about 5.5% per degree of advance.
  e.knockLimit = clamp(e.ign - Math.log(Math.max(e.knockIndex, 1e-3)) / 0.055, e.ign - 15, e.ign + 15);
  e.knocking = fired && e.knockIndex >= 1 && e.map > 50;
  if (e.knocking) {
    e.knockAcc += dt * (1 + e.ign - e.knockLimit);
    if (e.knockAcc > 0.1) { e.knockAcc = 0; e.knockCount++; e.knockRetard = Math.min(e.knockRetard + 1, 8); }
  } else e.knockRetard = Math.max(0, e.knockRetard - dt * 0.5);

  // Torque: indicated work from the cylinder pressure trace, less pumping work and mechanical friction.
  e.ca50 = g.ca50Deg; e.burnDeg = g.burnDeg;
  e.cylPmaxBar = g.pmaxBar * corr;
  const indicated = fired ? g.imepKpa * corr * ENGINE.displacementL / (4 * Math.PI) : g.imepKpa * ENGINE.displacementL / (4 * Math.PI);
  e.pumpingNm = e.pmepKpa * ENGINE.displacementL / (4 * Math.PI);
  e.torqueNm = indicated - frictionNm(rpm) - e.pumpingNm;
  e.sparkEff = clamp(1 - ((e.ign - e.mbt) / 28) ** 2, 0.2, 1);   // display only
  e.overTorque = e.torqueNm > (t.torqueLimitNm ?? Infinity);
  return e.torqueNm;
}

// Throttle body: 62 mm bore by default (estimate; tune.flow.throttleDiaM changes it), plate closed at 7°, discharge coefficient 0.75, plus a small
// leak past the closed plate. Flow through it is compressible orifice flow, choked below ~53% of the
// upstream pressure. The engine pumps VE × displacement × rpm/120 of manifold air.
const THROTTLE_D_M = 0.062, PLATE_CLOSED_DEG = 7, THROTTLE_CD = 0.75, LEAK_M2 = 5e-6, R_AIR = 287, GAMMA = 1.4;
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
