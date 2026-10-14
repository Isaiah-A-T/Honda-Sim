//! Car and drivetrain: 6-speed gearbox, a clutch that locks or slips, traction and road load (web/src/vehicle.js).

use crate::engine::Engine;
use crate::k20z3::{CAR, ENGINE};
use crate::tables::clamp;

const G: f64 = 9.81;
const RHO: f64 = 1.2;
const RPM_PER_RADS: f64 = 60.0 / (2.0 * std::f64::consts::PI);
const CG_OVER_WHEELBASE: f64 = 0.19;

fn sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

pub fn traction_limit_n(accel: f64) -> f64 {
    CAR.traction_mu * CAR.mass_kg * G * (CAR.front_weight - CG_OVER_WHEELBASE * accel / G).max(0.3)
}

#[derive(Clone, Debug, Default)]
pub struct Vehicle {
    pub speed_ms: f64,
    /// 0 neutral, -1 reverse, 1–6.
    pub gear: i32,
    pub locked: bool,
    pub clutch_torque: f64,
    pub accel: f64,
    pub distance_m: f64,
    pub wheel_torque: f64,
    pub wheelspin: bool,
}

pub fn overall_ratio(gear: i32) -> f64 {
    if gear == 0 {
        0.0
    } else if gear < 0 {
        -CAR.reverse * CAR.final_drive
    } else {
        CAR.gears[(gear - 1) as usize] * CAR.final_drive
    }
}

pub fn road_load_n(speed_ms: f64, brake: f64) -> f64 {
    let roll = CAR.crr * CAR.mass_kg * G;
    let aero = 0.5 * RHO * CAR.cd * CAR.frontal_area_m2 * speed_ms * speed_ms;
    let brakes = brake * CAR.mass_kg * G;
    (roll + brakes) * sign(speed_ms) + aero * sign(speed_ms)
}

/// Step the engine/car pair. clutch is engagement 0 (pressed) to 1 (released). Updates rpm and speed.
pub fn step_drivetrain(eng: &mut Engine, v: &mut Vehicle, engine_torque: f64, clutch: f64, brake: f64, capacity_nm: f64, dt: f64) {
    let ratio = overall_ratio(v.gear);
    let r = CAR.tire_radius_m;
    let eff = CAR.driveline_eff;
    let capacity = capacity_nm * clamp(clutch, 0.0, 1.0);
    let mut we = eng.rpm / RPM_PER_RADS;
    let wt = v.speed_ms / r * ratio;
    let at_rest = v.speed_ms.abs() < 0.05;

    if ratio == 0.0 || capacity < 1.0 {
        v.locked = false;
        v.clutch_torque = 0.0;
    } else if v.locked {
        let m_eff = CAR.mass_kg + ENGINE.inertia_kg_m2 * (ratio / r).powi(2);
        let mut drive = engine_torque * ratio * eff / r;
        let mut f_res = road_load_n(v.speed_ms, brake);
        if at_rest && drive.abs() <= road_load_n(1.0, brake) {
            f_res = drive;
        }
        let grip = traction_limit_n(v.accel);
        v.wheelspin = drive.abs() > grip;
        if v.wheelspin {
            drive = sign(drive) * grip;
        }
        let a = (drive - f_res) / m_eff;
        let eng_accel = a / r * ratio;
        v.clutch_torque = engine_torque - ENGINE.inertia_kg_m2 * eng_accel;
        if v.clutch_torque.abs() > capacity {
            v.locked = false;
        } else {
            v.accel = a;
            v.speed_ms += a * dt;
            eng.rpm = (v.speed_ms / r * ratio * RPM_PER_RADS).max(0.0);
            v.distance_m += v.speed_ms * dt;
            v.wheel_torque = v.clutch_torque * ratio * eff;
            return;
        }
    }

    let slip = we - wt;
    v.clutch_torque = if ratio == 0.0 { 0.0 } else { sign(slip) * capacity * clamp(slip.abs(), 0.0, 1.0) };
    we += (engine_torque - v.clutch_torque) / ENGINE.inertia_kg_m2 * dt;
    let mut f_res = road_load_n(v.speed_ms, brake);
    let mut drive = v.clutch_torque * ratio * eff / r;
    let grip = traction_limit_n(v.accel);
    v.wheelspin = drive.abs() > grip;
    if v.wheelspin {
        drive = sign(drive) * grip;
    }
    if at_rest && drive.abs() <= road_load_n(1.0, brake) {
        f_res = drive;
    }
    v.accel = (drive - f_res) / CAR.mass_kg;
    v.speed_ms += v.accel * dt;
    if brake > 0.0 && v.speed_ms.abs() < 0.05 {
        v.speed_ms = 0.0;
    }
    eng.rpm = (we * RPM_PER_RADS).max(0.0);
    v.distance_m += v.speed_ms * dt;
    v.wheel_torque = v.clutch_torque * ratio * eff;

    let wt2 = v.speed_ms / r * ratio;
    if ratio != 0.0 && capacity >= 1.0 && (sign(we - wt2) != sign(slip) || ((we - wt2).abs() < 3.0 && engine_torque.abs() <= capacity)) {
        v.locked = true;
        eng.rpm = (wt2 * RPM_PER_RADS).max(0.0);
    }
}
