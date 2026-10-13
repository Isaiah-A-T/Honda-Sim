//! Simulation loop: driver inputs → ECU/engine → drivetrain → car, plus shifting, stalling and the
//! datalog (src/sim.js).

use crate::engine::{hp, Engine};
use crate::internals::Internals;
use crate::k20z3::{CAR, ENGINE};
use crate::tables::clamp;
use crate::tune::Tune;
use crate::thermal::{Thermal, ThermalInput};
use crate::vehicle::{overall_ratio, step_drivetrain, Vehicle};
use serde::Serialize;

const SUBSTEP: f64 = 0.001;
const SHIFT_TIME: f64 = 0.2;
const LOG_HZ: f64 = 20.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub throttle: f64,
    pub brake: f64,
    /// Clutch pedal 0–1 (1 = pressed), or None for the automatic clutch.
    pub clutch: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Sim {
    pub t: f64,
    pub eng: Engine,
    pub car: Vehicle,
    pub internals: Internals,
    pub running: bool,
    pub shift_t: f64,
    pub ac: f64,
    pub launching: bool,
    pub last_clutch: f64,
    pub last_throttle: f64,
    pub log: Vec<LogRow>,
    pub logging: bool,
    log_t: f64,
}

impl Sim {
    pub fn new(tune: Tune) -> Sim {
        Sim { t: 0.0, eng: Engine::new(tune), car: Vehicle::default(), internals: Internals::default(), running: true, shift_t: 0.0, ac: 0.0, launching: false, last_clutch: 1.0, last_throttle: 0.0, log: Vec::new(), logging: false, log_t: 0.0 }
    }

    pub fn set_tune(&mut self, tune: Tune) {
        self.eng.set_tune(tune);
    }

    pub fn shift(&mut self, gear: i32) {
        let g = gear.clamp(-1, CAR.gears.len() as i32);
        if g == self.car.gear {
            return;
        }
        self.car.gear = g;
        self.car.locked = false;
        self.shift_t = SHIFT_TIME;
    }
    pub fn shift_up(&mut self) {
        self.shift(self.car.gear + 1)
    }
    pub fn shift_down(&mut self) {
        self.shift(self.car.gear - 1)
    }

    pub fn start_engine(&mut self) {
        if self.running || self.internals.failed.is_some() {
            return;
        }
        if self.car.gear != 0 && self.last_clutch >= 0.2 {
            return;
        }
        self.running = true;
        self.eng.rpm = ENGINE.idle_rpm + 300.0;
        self.eng.knock_retard = 0.0;
    }

    /// Engine cold (coolant and oil at the outside air temperature), as after sitting overnight.
    pub fn cold_engine(&mut self) {
        let e = &mut self.eng;
        e.thermal = Thermal::new(e.tune.ambient_c.unwrap_or(25.0), false, e.tune.cooling());
        e.ect = e.thermal.coolant_c;
        e.oil_c = e.thermal.oil_c;
    }

    pub fn rebuild_engine(&mut self) {
        self.internals = Internals::default();
        self.eng.knock_count = 0;
    }

    fn auto_clutch(&mut self, throttle: f64, h: f64) -> f64 {
        let wheel_rpm = (self.car.speed_ms / CAR.tire_radius_m * overall_ratio(self.car.gear)).abs() * 60.0 / (2.0 * std::f64::consts::PI);
        let target;
        if self.shift_t > 0.0 || self.car.gear == 0 {
            target = 0.0;
        } else if wheel_rpm < 1000.0 && throttle < 0.05 {
            target = 0.0;
        } else if wheel_rpm < 1100.0 || (self.launching && !self.car.locked) {
            self.launching = true;
            let launch_rpm = 1300.0 + throttle * 3200.0;
            let cap = self.eng.tune.clutch_nm;
            self.ac = clamp(self.eng.torque_nm.max(0.0) / cap + (self.eng.rpm - launch_rpm) * 0.0008, 0.0, 1.0);
            return self.ac;
        } else {
            target = 1.0;
        }
        if self.car.locked {
            self.launching = false;
        }
        self.ac += (target - self.ac) * (h * if target > self.ac { 10.0 } else { 30.0 }).min(1.0);
        self.ac
    }

    pub fn step(&mut self, input: Input, dt: f64) {
        let throttle = clamp(input.throttle, 0.0, 1.0);
        let brake = clamp(input.brake, 0.0, 1.0);
        self.last_throttle = throttle;
        let n = ((dt / SUBSTEP).round() as usize).max(1);
        let h = dt / n as f64;
        for _ in 0..n {
            self.shift_t = (self.shift_t - h).max(0.0);
            let clutch = match input.clutch {
                None => self.auto_clutch(throttle, h),
                Some(c) => 1.0 - clamp(c, 0.0, 1.0),
            };
            self.last_clutch = clutch;
            let mut tq;
            if self.running {
                tq = self.eng.step(throttle, h);
                self.internals.step(&self.eng, h);
                if self.internals.failed.is_some() {
                    self.running = false;
                    tq = -60.0;
                }
                if self.eng.rpm < ENGINE.stall_rpm {
                    self.running = false;
                }
            } else {
                self.eng.torque_nm = 0.0;
                tq = if self.eng.rpm > 1.0 { -12.0 } else { 0.0 };
            }
            let cap = self.eng.tune.clutch_nm;
            step_drivetrain(&mut self.eng, &mut self.car, tq, clutch, brake, cap, h);
            // Coolant and oil: heat from combustion and friction while running, then the engine cools down.
            let on = self.running;
            let e = &mut self.eng;
            let input = ThermalInput {
                rpm: e.rpm,
                fuel_gs: if on { e.fuel_gs } else { 0.0 },
                friction_w: if on { e.friction_nm * e.rpm * std::f64::consts::PI / 30.0 } else { 0.0 },
                heat_loss: 0.30,
                speed_ms: self.car.speed_ms,
                map_kpa: e.map,
                ambient_c: e.tune.ambient_c.unwrap_or(25.0),
            };
            e.thermal.step(&input, h);
            e.ect = e.thermal.coolant_c;
            e.oil_c = e.thermal.oil_c;
            if !self.running && self.eng.rpm < 1.0 {
                self.eng.rpm = 0.0;
            }
            self.t += h;
        }
        if self.logging {
            self.log_t += dt;
            if self.log_t >= 1.0 / LOG_HZ {
                self.log_t = 0.0;
                let row = self.snapshot();
                self.log.push(row);
            }
        }
    }

    pub fn snapshot(&self) -> LogRow {
        let e = &self.eng;
        let c = &self.car;
        let s = &self.internals;
        let r = |x: f64, d: i32| {
            let p = 10f64.powi(d);
            (x * p).round() / p
        };
        LogRow {
            time: r(self.t, 3),
            rpm: e.rpm.round(),
            vss_kmh: r(c.speed_ms * 3.6, 1),
            gear: c.gear,
            pedal: r(self.last_throttle * 100.0, 1),
            tps: r(e.tps, 1),
            map_kpa: r(e.map, 1),
            vtec: e.high_cam as u8,
            ign_deg: r(e.ign, 1),
            ign_table: r(e.ign_table, 1),
            mbt_deg: r(e.mbt, 1),
            knock_limit: r(e.knock_limit, 1),
            knock_retard: r(e.knock_retard, 1),
            knock_count: e.knock_count,
            afr: r(e.afr, 2),
            lambda: r(e.lambda, 3),
            inj_ms: r(e.ipw, 2),
            inj_duty: r(e.inj_duty, 1),
            torque_nm: r(if self.running { e.torque_nm } else { 0.0 }, 1),
            hp: r(if self.running { hp(e.torque_nm, e.rpm) } else { 0.0 }, 1),
            rev_cut: e.rev_cut as u8,
            boost_psi: r((e.map - 101.3).max(0.0) * 0.145, 1),
            boost_target_kpa: if e.boost_target.is_finite() { Some(e.boost_target.round()) } else { None },
            wg_duty: r(e.wg_duty, 1),
            boost_cut: e.boost_cut as u8,
            iat_c: r(e.iat, 1),
            over_torque: e.over_torque as u8,
            pmax_bar: s.pmax_bar.round(),
            piston_c: s.piston_c.round(),
            rod_load: r(s.load_of("rod"), 2),
            piston_load: r(s.load_of("piston"), 2),
            bearing_load: r(s.load_of("bearing"), 2),
            sleeve_load: r(s.load_of("sleeve"), 2),
            vtc_deg: r(e.vtc, 1),
            ve: r(e.ve, 3),
            ca50_deg: if e.ca50.is_finite() { Some(r(e.ca50, 1)) } else { None },
            knock_index: r(e.knock_index, 2),
            egt_c: e.egt_c.round(),
            exh_kpa: r(e.exh_kpa, 1),
            pmep_kpa: e.pmep_kpa.round(),
            residual_pct: r(e.residual * 100.0, 1),
            turbo_krpm: (e.turbo_rad_s * 30.0 / std::f64::consts::PI / 1000.0).round(),
            wg_open_pct: (e.wg_open * 100.0).round(),
            ect_c: r(e.ect, 1),
            oil_c: r(e.oil_c, 1),
            oil_kpa: e.oil_kpa.round(),
            thermostat_pct: (e.thermal.thermostat * 100.0).round(),
            fan: if e.thermal.fan_on { 1 } else { 0 },
            radiator_kw: r(e.thermal.radiator_kw, 1),
        }
    }
}

/// One datalog row; columns match the web app's CSV.
#[derive(Clone, Debug, Serialize)]
pub struct LogRow {
    pub time: f64,
    pub rpm: f64,
    pub vss_kmh: f64,
    pub gear: i32,
    pub pedal: f64,
    pub tps: f64,
    pub map_kpa: f64,
    pub vtec: u8,
    pub ign_deg: f64,
    pub ign_table: f64,
    pub mbt_deg: f64,
    pub knock_limit: f64,
    pub knock_retard: f64,
    pub knock_count: u32,
    pub afr: f64,
    pub lambda: f64,
    pub inj_ms: f64,
    pub inj_duty: f64,
    pub torque_nm: f64,
    pub hp: f64,
    pub rev_cut: u8,
    pub boost_psi: f64,
    pub boost_target_kpa: Option<f64>,
    pub wg_duty: f64,
    pub boost_cut: u8,
    pub iat_c: f64,
    pub over_torque: u8,
    pub pmax_bar: f64,
    pub piston_c: f64,
    pub rod_load: f64,
    pub piston_load: f64,
    pub bearing_load: f64,
    pub sleeve_load: f64,
    pub vtc_deg: f64,
    pub ve: f64,
    pub ca50_deg: Option<f64>,
    pub knock_index: f64,
    pub egt_c: f64,
    pub exh_kpa: f64,
    pub pmep_kpa: f64,
    pub residual_pct: f64,
    pub turbo_krpm: f64,
    pub wg_open_pct: f64,
    pub ect_c: f64,
    pub oil_c: f64,
    pub oil_kpa: f64,
    pub thermostat_pct: f64,
    pub fan: i32,
    pub radiator_kw: f64,
}

/// CSV of log rows (header from the field names).
pub fn log_to_csv(rows: &[LogRow]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let first = serde_json::to_value(&rows[0]).unwrap();
    let keys: Vec<String> = first.as_object().unwrap().keys().cloned().collect();
    out.push_str(&keys.join(","));
    out.push('\n');
    for r in rows {
        let v = serde_json::to_value(r).unwrap();
        let line: Vec<String> = keys.iter().map(|k| match &v[k] {
            serde_json::Value::Null => String::new(),
            x => x.to_string(),
        }).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}
