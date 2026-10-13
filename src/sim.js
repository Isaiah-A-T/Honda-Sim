// Simulation loop: driver inputs → ECU/engine → drivetrain → car, plus shifting, stalling and the datalog.

import { clamp } from './tables.js';
import { CAR, ENGINE, placeholderTune } from './k20z3.js';
import { createEngine, stepEngine, hp } from './engine.js';
import { createVehicle, stepDrivetrain, overallRatio } from './vehicle.js';
import { createInternals, stepInternals } from './internals.js';
import { createThermal, stepThermal, COOLING } from './thermal.js';

const SUBSTEP = 0.001;
const SHIFT_TIME = 0.2;                 // seconds the auto-clutch stays open for a shift
const LOG_HZ = 20;

export function createSim(tune = placeholderTune()) {
  return {
    t: 0,
    eng: createEngine(tune),
    car: createVehicle(),
    internals: createInternals(),
    running: true,
    shiftT: 0,
    ac: 0,
    launching: false,
    lastClutch: 1,
    log: [],
    logEvery: 1 / LOG_HZ, logT: 0, logging: false,
  };
}

export function setTune(sim, tune) {
  sim.eng.tune = tune;
}

// Inputs: { throttle 0–1, brake 0–1, clutch 0–1 (1 = pedal down) or null for auto-clutch }
export function shift(sim, gear) {
  const g = clamp(gear, -1, CAR.gears.length);
  if (g === sim.car.gear) return;
  sim.car.gear = g;
  sim.car.locked = false;
  sim.shiftT = SHIFT_TIME;
}
export const shiftUp = sim => shift(sim, sim.car.gear + 1);
export const shiftDown = sim => shift(sim, sim.car.gear - 1);

export function startEngine(sim) {
  if (sim.running || sim.internals.failed) return;
  if (sim.car.gear !== 0 && !(sim.lastClutch < 0.2)) return;   // needs neutral or clutch in
  sim.running = true;
  sim.eng.rpm = ENGINE.idleRpm + 300;
  sim.eng.knockRetard = 0;
}

// Turbo builds: what the turbo's oil and coolant lines see (null with no turbo or a supercharger).
export function turboLines(e, on) {
  const tb = e.tune.turbo;
  if (!tb?.enabled || tb.fixed) return null;
  return { gasGs: on ? e.airGs + e.fuelGs : 0, egtC: e.egtC ?? 600, oilKpa: on ? e.oilKpa ?? 0 : 0, waterCooled: tb.waterCooled ?? true };
}

// Engine cold (coolant and oil at the outside air temperature), as after sitting overnight.
export function coldEngine(sim) {
  const e = sim.eng;
  e.thermal = createThermal(e.tune.ambientC ?? 25, false, e.tune.cooling);
  e.ect = e.thermal.coolantC; e.oilC = e.thermal.oilC;
}

// Fresh internals after a failure (or to reset wear).
export function rebuildEngine(sim) {
  sim.internals = createInternals();
  sim.eng.knockCount = 0;
}

// Auto-clutch: opens during a shift, slips to launch from a stop, declutches before a stall,
// otherwise closes smoothly. Returns engagement 0–1.
function autoClutch(sim, throttle, h) {
  let target;
  const wheelRpm = Math.abs(sim.car.speedMs / CAR.tireRadiusM * overallRatio(sim.car.gear)) * 60 / (2 * Math.PI);
  if (sim.shiftT > 0 || sim.car.gear === 0) target = 0;
  else if (wheelRpm < 1000 && throttle < 0.05) target = 0;            // rolling to a stop: don't stall
  else if (wheelRpm < 1100 || (sim.launching && !sim.car.locked)) {
    sim.launching = true;
    // Launch: slip the clutch to hold the engine near a launch rpm that rises with throttle (~4,500 at WOT).
    const launchRpm = 1300 + throttle * 3200;
    const cap = sim.eng.tune.clutchNm ?? CAR.clutchCapacityNm;
    sim.ac = clamp(Math.max(0, sim.eng.torqueNm) / cap + (sim.eng.rpm - launchRpm) * 0.0008, 0, 1);
    return sim.ac;
  }
  else target = 1;
  if (sim.car.locked) sim.launching = false;
  sim.ac += (target - sim.ac) * Math.min(1, h * (target > sim.ac ? 10 : 30));
  return sim.ac;
}

export function step(sim, input, dt) {
  const throttle = clamp(input.throttle ?? 0, 0, 1);
  const brake = clamp(input.brake ?? 0, 0, 1);
  const n = Math.max(1, Math.round(dt / SUBSTEP));
  const h = dt / n;
  for (let i = 0; i < n; i++) {
    sim.shiftT = Math.max(0, sim.shiftT - h);
    const clutch = input.clutch == null
      ? autoClutch(sim, throttle, h)
      : 1 - clamp(input.clutch, 0, 1);
    sim.lastClutch = clutch;
    let tq = 0;
    if (sim.running) {
      tq = stepEngine(sim.eng, throttle, h);
      stepInternals(sim.internals, sim.eng, h);
      if (sim.internals.failed) { sim.running = false; tq = -60; }   // a broken engine locks up hard
      if (sim.eng.rpm < ENGINE.stallRpm) { sim.running = false; }
    } else {
      sim.eng.torqueNm = 0;
      tq = sim.eng.rpm > 1 ? -12 : 0;              // a stopped engine just drags
    }
    stepDrivetrain(sim.eng, sim.car, tq, { clutch, brake, capacityNm: sim.eng.tune.clutchNm }, h);
    // Coolant and oil: heat from combustion and friction while running, then the engine cools down.
    const e = sim.eng, th = e.thermal;
    th.g = e.tune.cooling ? { ...COOLING, ...e.tune.cooling } : COOLING;
    const on = sim.running;
    stepThermal(th, { rpm: e.rpm, fuelGs: on ? e.fuelGs : 0, frictionW: on ? e.frictionNm * e.rpm * Math.PI / 30 : 0,
      heatLoss: 0.30, speedMs: sim.car.speedMs, mapKpa: e.map, ambientC: e.tune.ambientC ?? 25, turbo: turboLines(e, on) }, h);
    e.ect = th.coolantC; e.oilC = th.oilC;
    if (!sim.running && sim.eng.rpm < 1) sim.eng.rpm = 0;
    sim.t += h;
  }
  if (sim.logging) {
    sim.logT += dt;
    if (sim.logT >= sim.logEvery) { sim.logT = 0; sim.log.push(snapshot(sim, throttle)); }
  }
}

export function snapshot(sim, throttle = 0) {
  const e = sim.eng, c = sim.car;
  return {
    time: +sim.t.toFixed(3),
    rpm: Math.round(e.rpm),
    vss_kmh: +(c.speedMs * 3.6).toFixed(1),
    gear: c.gear,
    pedal: +(throttle * 100).toFixed(1),
    tps: +e.tps.toFixed(1),
    map_kpa: +e.map.toFixed(1),
    vtec: e.highCam ? 1 : 0,
    ign_deg: +e.ign.toFixed(1),
    ign_table: +e.ignTable.toFixed(1),
    mbt_deg: +(e.mbt ?? 0).toFixed(1),
    knock_limit: +e.knockLimit.toFixed(1),
    knock_retard: +e.knockRetard.toFixed(1),
    knock_count: e.knockCount,
    afr: +e.afr.toFixed(2),
    lambda: +e.lambda.toFixed(3),
    inj_ms: +e.ipw.toFixed(2),
    inj_duty: +(e.injDuty ?? 0).toFixed(1),
    torque_nm: +(sim.running ? e.torqueNm : 0).toFixed(1),
    hp: +(sim.running ? hp(e.torqueNm, e.rpm) : 0).toFixed(1),
    rev_cut: e.revCut ? 1 : 0,
    boost_psi: +(Math.max(0, e.map - 101.3) * 0.145).toFixed(1),
    boost_target_kpa: Number.isFinite(e.boostTarget) ? +e.boostTarget.toFixed(0) : '',
    wg_duty: +e.wgDuty.toFixed(1),
    boost_cut: e.boostCut ? 1 : 0,
    iat_c: +e.iat.toFixed(1),
    over_torque: e.overTorque ? 1 : 0,
    pmax_bar: +sim.internals.pmaxBar.toFixed(0),
    piston_c: +sim.internals.pistonC.toFixed(0),
    rod_load: +sim.internals.load.rod.toFixed(2),
    piston_load: +sim.internals.load.piston.toFixed(2),
    bearing_load: +sim.internals.load.bearing.toFixed(2),
    sleeve_load: +sim.internals.load.sleeve.toFixed(2),
    vtc_deg: +(e.vtc ?? 0).toFixed(1),
    ve: +(e.ve ?? 0).toFixed(3),
    ca50_deg: Number.isFinite(e.ca50) ? +e.ca50.toFixed(1) : '',
    knock_index: +(e.knockIndex ?? 0).toFixed(2),
    egt_c: +(e.egtC ?? 0).toFixed(0),
    exh_kpa: +(e.exhKpa ?? 0).toFixed(1),
    pmep_kpa: +(e.pmepKpa ?? 0).toFixed(0),
    residual_pct: +((e.residual ?? 0) * 100).toFixed(1),
    turbo_krpm: +((e.turboRadS ?? 0) * 30 / Math.PI / 1000).toFixed(0),
    wg_open_pct: +((e.wgOpen ?? 0) * 100).toFixed(0),
    ect_c: +e.ect.toFixed(1),
    oil_c: +e.oilC.toFixed(1),
    oil_kpa: +(e.oilKpa ?? 0).toFixed(0),
    thermostat_pct: +(e.thermal.thermostat * 100).toFixed(0),
    fan: e.thermal.fanOn ? 1 : 0,
    radiator_kw: +e.thermal.radiatorKw.toFixed(1),
  };
}

export function logToCsv(rows) {
  if (!rows.length) return '';
  const keys = Object.keys(rows[0]);
  return [keys.join(','), ...rows.map(r => keys.map(k => r[k]).join(','))].join('\n') + '\n';
}
