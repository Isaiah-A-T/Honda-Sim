// 2006–2011 Civic Si (K20Z3, 6-speed) physical model and a placeholder tune.
//
// The "truth" functions describe the engine the simulated ECU is running, not the tune. They are
// approximations shaped to land near the published K20Z3 numbers (197 hp @ 7,800, 139 lb-ft @ 6,100),
// not measurements. The placeholder tune below is generic and is not Honda's or Hondata's calibration;
// it is only there so the sim runs before real tables are pasted in.

import { clamp, lookup1 } from './tables.js';

export const CAR = {
  name: '2006–2011 Civic Si (K20Z3)',
  massKg: 1305 + 80,                 // 2,877 lb curb (Honda spec) plus driver
  gears: [3.267, 2.130, 1.517, 1.147, 0.921, 0.659],   // Honda spec sheet
  reverse: 3.583,
  finalDrive: 4.765,
  tireRadiusM: 0.3127,               // 215/45R17
  drivelineEff: 0.88,
  cd: 0.31, frontalAreaM2: 2.0, crr: 0.012,
  clutchCapacityNm: 320,
  tractionMu: 1.0,                   // tire grip; 61% of the weight is on the front (driven) wheels
  frontWeight: 0.61,
};

export const ENGINE = {
  displacementL: 1.998,
  cylinders: 4,
  inertiaKgM2: 0.16,                 // crank, flywheel, clutch
  injectorCcMin: 310,                // stock injector flow, approximate
  injectorDeadMs: 0.7,
  stallRpm: 450,
  idleRpm: 750,
};

// Crank torque targets (Nm) at wide-open throttle on a stock engine, per cam profile. The volumetric
// efficiency tables below are solved from these by tools/calibrate.mjs, so edit these, not VE_*.
// Anchors: Honda's rating of 197 hp @ 7,800 and 139 lb-ft (188 Nm) @ 6,200 rpm; Hondata's note that
// moving VTEC to 4,500 on a stock car loses power from 4,500 to 5,600 rpm (so the high cam only
// overtakes the low cam around 5,600); the stock VTEC point of 5,800 rpm. The low cam shape between
// those anchors is an estimate from the usual look of stock K20Z3 dyno charts, not a measurement.
export const TORQUE_TARGETS = {
  low: [[1000, 125], [2000, 145], [3000, 158], [3500, 163], [4000, 168], [4500, 171], [5000, 170], [5500, 166], [5800, 162], [6500, 150], [7500, 128], [8500, 105], [9000, 92]],
  high: [[1000, 80], [3000, 120], [4000, 140], [5000, 152], [5500, 161], [5800, 172], [6200, 188], [6800, 187], [7400, 184], [7800, 180], [8200, 170], [8600, 158], [9000, 145]],
};

// Breathing comes from the gas exchange model in gasflow.js (valves, cams, runners, exhaust). These
// are the small per-rpm corrections tools/calibrate.mjs fits on top of it so the stock engine lands on
// TORQUE_TARGETS; they stand for what the simple model leaves out (true wave dynamics, port shape,
// wall heat). Edit TORQUE_TARGETS or the geometry in gasflow.js and rerun the tool, not these.
export const VE_RPM = TORQUE_TARGETS.low.map(([r]) => r);
export const VE_HIGH_RPM = TORQUE_TARGETS.high.map(([r]) => r);
export const VE_CORR_LOW = [0.6826, 0.7286, 0.7796, 0.8165, 0.8581, 0.896, 0.8922, 0.877, 0.8708, 0.8562, 0.8152, 0.7613, 0.8078];
export const VE_CORR_HIGH = [0.4266, 0.6116, 0.7452, 0.8249, 0.8538, 0.9311, 1.0284, 1.0093, 1.0296, 0.9981, 0.9605, 0.9346, 0.9484];

export function veCorrection(rpm, highCam) {
  return highCam ? lookup1(VE_HIGH_RPM, VE_CORR_HIGH, rpm) : lookup1(VE_RPM, VE_CORR_LOW, rpm);
}

// Spark advance for best torque (degrees BTDC).
export function mbt(rpm, mapKpa) {
  return clamp(17 + 3.0 * Math.min(rpm, 6500) / 1000 + (100 - mapKpa) * 0.13, 8, 45);
}

// Highest advance before knock on 91 octane (degrees BTDC). Higher octane adds margin.
export function knockLimit(rpm, mapKpa, iatC = 25, octane = 91) {
  return 21 + 2.4 * rpm / 1000 - (mapKpa - 60) * 0.15 - Math.max(0, iatC - 30) * 0.12 + (octane - 91) * 1.2;
}

// Mechanical friction torque (Nm): rings, bearings, valvetrain, oil and water pumps. Pumping losses
// come from the gas exchange model. About 200 kPa FMEP at 7,800 rpm, normal for a high-revving 2.0.
export function frictionNm(rpm) {
  return 5 + 0.0012 * rpm + 0.0000003 * rpm * rpm;
}

const RPM_AXIS = [500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500, 8000];
const MAP_AXIS = [20, 30, 40, 50, 60, 70, 80, 90, 100];
const table = (fn) => ({ rows: RPM_AXIS, cols: MAP_AXIS, data: RPM_AXIS.map(r => MAP_AXIS.map(m => Math.round(fn(r, m) * 100) / 100)) });

// Placeholder tune in the shape a FlashPro K-series calibration uses: separate low and high cam tables.
export function placeholderTune() {
  const ign = (r, m) => (r <= 1000 && m <= 40 ? 15 : Math.min(mbt(r, m), knockLimit(r, m) - 2));
  const lam = (r, m) => (m < 70 ? 1.0 : m < 90 ? 0.95 : r > 5000 ? 0.85 : 0.88);
  return {
    name: 'Placeholder (not a real calibration)',
    fuelUnits: 'lambda',
    fuelLow: table(lam),
    fuelHigh: table(lam),
    ignLow: table(ign),
    ignHigh: table((r, m) => ign(r, m) + 1),
    vtecRpm: 5800,                   // switch to high cam above this at load
    vtecMinMap: 70,                  // below this MAP the ECU stays on low cam
    revLimit: 8000,
    revLimitHyst: 200,
    octane: 91,
    turbo: { ...TURBO_GT35, enabled: false },
    injectorCc: ENGINE.injectorCcMin,
    injectorDeadMs: ENGINE.injectorDeadMs,
    clutchNm: CAR.clutchCapacityNm,
  };
}

// A GT35-class ball-bearing turbo, described by its hardware rather than by when it makes boost.
// The turbine is driven by exhaust energy from the gas exchange model: exhaust flow, temperature and
// the pressure drop across it. The compressor takes power to make boost; the shaft speeds up when the
// turbine makes more than the compressor uses. Turbine and wastegate are effective flow areas (port
// area × discharge coefficient). Sizes are estimates for a GT3582R with a 0.82 A/R hot side and a
// 44 mm wastegate; efficiencies are typical map-centre values. maxBoostKpa is where this compressor
// runs off its map on a 2.0 L.
export const TURBO_GT35 = {
  name: 'GT35-class turbo (estimated)',
  turbineAreaCm2: 9.5, wastegateAreaCm2: 9.0, compressorTipMm: 41, inertiaKgM2: 3.0e-5,
  turbineEff: 0.68, compressorEff: 0.72, maxBoostKpa: 160, springKpa: 48,
};

// Turbo build on stock internals: GT35-class turbo, 1000 cc injectors, 4-bar MAP, overboost cut and a torque
// warning for the stock rods. Tables still come from the user's tune.
export function turboSetup(tune = placeholderTune()) {
  return {
    ...tune,
    turbo: { ...TURBO_GT35, enabled: true },
    injectorCc: 1000,
    injectorDeadMs: 1.0,
    overboostKpa: 200,
    torqueLimitNm: 350,              // commonly quoted ceiling for stock K20 rods; a warning, not a failure model
    clutchNm: 450,                   // assumes an upgraded clutch; a stock clutch slips with this much torque
  };
}
