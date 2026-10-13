// 2006–2011 Civic Si (K20Z3, 6-speed) physical model and a placeholder tune.
//
// The "truth" functions describe the engine the simulated ECU is running, not the tune. They are
// approximations shaped to land near the published K20Z3 numbers (197 hp @ 7,800, 139 lb-ft @ 6,100),
// not measurements. The placeholder tune below is generic and is not Honda's or Hondata's calibration;
// it is only there so the sim runs before real tables are pasted in.

import { clamp } from './tables.js';

export const CAR = {
  name: '2006–2011 Civic Si (K20Z3)',
  massKg: 1250 + 80,                 // curb weight plus driver
  gears: [3.267, 2.130, 1.517, 1.147, 0.921, 0.738],
  reverse: 3.0,
  finalDrive: 4.764,
  tireRadiusM: 0.3127,               // 215/45R17
  drivelineEff: 0.88,
  cd: 0.31, frontalAreaM2: 2.0, crr: 0.012,
  clutchCapacityNm: 320,
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

// Volumetric efficiency (0–1) at wide-open throttle for each cam profile, given rpm.
export function trueVE(rpm, highCam) {
  const k = rpm / 1000;
  return highCam
    ? 0.70 + 0.31 * Math.exp(-(((k - 7.0) / 2.6) ** 2))
    : 0.58 + 0.29 * Math.exp(-(((k - 4.4) / 2.4) ** 2));
}

// Spark advance for best torque (degrees BTDC).
export function mbt(rpm, mapKpa) {
  return clamp(17 + 3.0 * Math.min(rpm, 6500) / 1000 + (100 - mapKpa) * 0.13, 8, 45);
}

// Highest advance before knock on 91 octane (degrees BTDC). Higher octane adds margin.
export function knockLimit(rpm, mapKpa, iatC = 25, octane = 91) {
  return 21 + 2.4 * rpm / 1000 - (mapKpa - 60) * 0.15 - Math.max(0, iatC - 30) * 0.12 + (octane - 91) * 1.2;
}

// Friction + pumping torque (Nm) the engine loses at a given rpm and manifold pressure.
export function frictionNm(rpm, mapKpa) {
  return 6 + 0.002 * rpm + 0.0000004 * rpm * rpm + (100 - mapKpa) * 0.15;
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
  };
}
