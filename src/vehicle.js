// Car and drivetrain: 6-speed gearbox, clutch that can lock or slip, and road load.
//
// When the clutch is locked the engine and wheels turn together, so engine rpm comes from road speed.
// When it slips (launching, shifting, clutch pedal down) the engine spins on its own inertia and the
// clutch passes at most its capacity in torque.

import { clamp } from './tables.js';
import { CAR, ENGINE } from './k20z3.js';

const G = 9.81, RHO = 1.2, RPM_PER_RADS = 60 / (2 * Math.PI);

export function createVehicle() {
  return { speedMs: 0, gear: 0, locked: false, clutchTorque: 0, accel: 0, distanceM: 0, wheelTorque: 0 };
}

// Overall ratio engine:wheel for a gear (0 = neutral, -1 = reverse).
export function overallRatio(gear) {
  if (gear === 0) return 0;
  if (gear < 0) return -CAR.reverse * CAR.finalDrive;
  return CAR.gears[gear - 1] * CAR.finalDrive;
}

export function roadLoadN(speedMs, brake = 0) {
  const roll = CAR.crr * CAR.massKg * G;
  const aero = 0.5 * RHO * CAR.cd * CAR.frontalAreaM2 * speedMs * speedMs;
  const brakes = brake * CAR.massKg * G * 1.0;
  return (roll + brakes) * Math.sign(speedMs) + aero * Math.sign(speedMs);
}

// Step the engine/car pair. engineTorque is brake torque at the crank; clutch is 0 (pressed) to 1 (released).
// Updates eng.rpm and v.speedMs.
export function stepDrivetrain(eng, v, engineTorque, { clutch = 1, brake = 0 }, dt) {
  const ratio = overallRatio(v.gear);
  const r = CAR.tireRadiusM, eff = CAR.drivelineEff;
  const capacity = CAR.clutchCapacityNm * clamp(clutch, 0, 1);
  let we = eng.rpm / RPM_PER_RADS;
  const wt = v.speedMs / r * ratio;           // clutch output speed, rad/s at the crank side

  // Resistance: brakes oppose motion (and hold the car at rest), rolling and aero.
  const resist = (sp) => roadLoadN(sp, brake);
  const atRest = Math.abs(v.speedMs) < 0.05;

  if (ratio === 0 || capacity < 1) {
    v.locked = false;
    v.clutchTorque = 0;
  } else if (v.locked) {
    // Engine and car move as one mass.
    const mEff = CAR.massKg + ENGINE.inertiaKgM2 * (ratio / r) ** 2;
    const drive = engineTorque * ratio * eff / r;
    let fRes = resist(v.speedMs);
    if (atRest && Math.abs(drive) <= roadLoadN(1, brake)) fRes = drive;   // static friction / brakes hold
    const a = (drive - fRes) / mEff;
    const engAccel = a / r * ratio;           // rad/s² at the crank
    v.clutchTorque = engineTorque - ENGINE.inertiaKgM2 * engAccel;
    if (Math.abs(v.clutchTorque) > capacity) v.locked = false;
    else {
      v.accel = a;
      v.speedMs += a * dt;
      eng.rpm = Math.max(0, v.speedMs / r * ratio * RPM_PER_RADS);
      v.distanceM += v.speedMs * dt;
      v.wheelTorque = v.clutchTorque * ratio * eff;
      return;
    }
  }

  // Slipping or disengaged.
  const slip = we - wt;
  v.clutchTorque = ratio === 0 ? 0 : Math.sign(slip) * capacity * clamp(Math.abs(slip), 0, 1);
  we += (engineTorque - v.clutchTorque) / ENGINE.inertiaKgM2 * dt;
  let fRes = resist(v.speedMs);
  const drive = v.clutchTorque * ratio * eff / r;
  if (atRest && Math.abs(drive) <= roadLoadN(1, brake)) fRes = drive;
  v.accel = (drive - fRes) / CAR.massKg;
  v.speedMs += v.accel * dt;
  if (brake > 0 && Math.abs(v.speedMs) < 0.05) v.speedMs = 0;
  eng.rpm = Math.max(0, we * RPM_PER_RADS);
  v.distanceM += v.speedMs * dt;
  v.wheelTorque = v.clutchTorque * ratio * eff;

  // Lock up once the clutch speeds match.
  const wt2 = v.speedMs / r * ratio;
  if (ratio !== 0 && capacity >= 1 && (Math.sign(we - wt2) !== Math.sign(slip) || (Math.abs(we - wt2) < 3 && Math.abs(engineTorque) <= capacity))) {
    v.locked = true;
    eng.rpm = Math.max(0, wt2 * RPM_PER_RADS);
  }
}
