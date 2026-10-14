//! Engine model: runs the tune the way the ECU would and works out what the engine really does
//! (web/src/engine.js): idle control, turbo energy balance, intercooler, manifold pressure, VTEC actuation,
//! fuel, spark, knock and torque from the gas exchange model.

use crate::gasflow::{flow_defaults, valve_lift_m, vtc_for, Cond, Flow, Gas, GasOut};
use crate::internals::{engine_geometry, Geometry};
use crate::k20z3::{friction_nm, mbt, ve_correction, ENGINE};
use crate::engines::{balancer_nm, head_lobes};
use crate::fuels::{extra_cooling_k, fuel_of, injector_g_per_ms, pump_gs, pump_lph_of, ref_fuel, RAIL_KPA};
use crate::parts::{bottom_end, BottomEnd};
use crate::tables::{clamp, guess_fuel_units, lookup2};
use crate::thermal::{oil_pressure_kpa, visc_ratio, Thermal};
use crate::tune::Tune;
use std::f64::consts::PI;

const AIR_DENSITY_G_PER_L: f64 = 1.184;
const DECEL_AIR: f64 = 4.5;
const GAS_EVERY_S: f64 = 0.01;
const P_AMB: f64 = 101.3;
const DOWNPIPE_KPA: f64 = 4.0;
const VTEC_OIL_KPA: f64 = 250.0;
const PIN_TRAVEL_S: f64 = 0.04;
pub const CYL_OFFSET: [f64; 4] = [0.0, 540.0, 180.0, 360.0];

/// Gas model geometry: defaults, then what the fitted parts add up to, then the flow settings.
pub fn flow_geom(t: &Tune, b: &BottomEnd) -> Flow {
    let mut g = flow_defaults();
    if !b.geom_stock {
        g.bore_m = b.bore_mm / 1000.0;
        g.stroke_m = b.stroke_mm / 1000.0;
        g.rod_m = b.rod_mm / 1000.0;
        g.compression = b.compression;
        g.in_valve_m = b.head.in_valve_mm / 1000.0;
        g.ex_valve_m = b.head.ex_valve_mm / 1000.0;
        g.cams_low = head_lobes(&b.head.cams.low);
        g.cams_high = head_lobes(&b.head.cams.high);
    }
    if let Some(v) = t.flow.throttle_dia_m { g.throttle_dia_m = v }
    if let Some(v) = t.flow.runner_m { g.runner_m = v }
    if let Some(v) = t.flow.exhaust_dia_m { g.exhaust_dia_m = v }
    g
}

/// Fast idle on a cold engine: up to about 1,350 rpm when freezing, back to 750 by 60 °C (estimate).
pub fn cold_idle_rpm(ect_c: f64) -> f64 {
    ENGINE.idle_rpm + 600.0 * clamp((60.0 - ect_c) / 60.0, 0.0, 1.0)
}

pub fn hp(torque_nm: f64, rpm: f64) -> f64 {
    torque_nm * rpm / 7120.9
}

#[derive(Clone, Debug)]
pub struct Engine {
    pub tune: Tune,
    pub bottom: BottomEnd,
    pub geometry: Geometry,
    pub disp_l: f64,
    pub rpm: f64,
    pub map: f64,
    pub tps: f64,
    pub idle_air: f64,
    pub idle_i: f64,
    pub high_cam: bool,
    pub vtec_cmd: bool,
    pub vtec_pins: [f64; 4],
    pub cam_f: f64,
    pub oil_kpa: f64,
    pub crank_deg: f64,
    pub rev_cut: bool,
    pub dfco: bool,
    pub ign: f64,
    pub ign_table: f64,
    pub mbt: f64,
    pub knock_retard: f64,
    pub knock_limit: f64,
    pub knock_index: f64,
    pub knocking: bool,
    pub knock_count: u32,
    pub knock_acc: f64,
    pub lambda: f64,
    pub lambda_target: f64,
    pub afr: f64,
    pub ipw: f64,
    pub inj_duty: f64,
    /// Fuel asked of the pump, % of what it can deliver.
    pub fuel_pump_pct: f64,
    /// Rail pressure (kPa gauge): 50 psi until the pump runs out.
    pub fuel_pressure_kpa: f64,
    pub torque_nm: f64,
    pub pumping_nm: f64,
    pub air_gs: f64,
    pub air_delivered_gs: f64,
    pub ve: f64,
    pub iat: f64,
    pub ect: f64,
    pub oil_c: f64,
    pub fuel_gs: f64,
    pub friction_nm: f64,
    pub thermal: Thermal,
    pub boost: f64,
    pub ic_drop_kpa: f64,
    pub charge_out_c: f64,
    pub boost_target: f64,
    pub wg_duty: f64,
    pub wg_open: f64,
    pub turbo_rad_s: f64,
    pub turbine_kw: f64,
    pub compressor_kw: f64,
    pub boost_cut: bool,
    pub over_torque: bool,
    pub vtc: f64,
    pub pmep_kpa: f64,
    pub exh_kpa: f64,
    pub egt_c: f64,
    pub residual: f64,
    pub ca50: f64,
    pub burn_deg: f64,
    pub cyl_pmax_bar: f64,
    pub spark_eff: f64,
    gas: Gas,
    gas_hi: Gas,
    gas_lo_out: Option<GasOut>,
    gas_hi_out: Option<GasOut>,
    pub gas_out: GasOut,
    gas_t: f64,
    lo_idle: f64,
    hi_idle: f64,
    gas_dirty: bool,
}

impl Engine {
    pub fn new(tune: Tune) -> Engine {
        let bottom = bottom_end(&tune);
        let geometry = engine_geometry(&tune, &bottom);
        let g = flow_geom(&tune, &bottom);
        let thermal = Thermal::new(tune.ambient_c.unwrap_or(25.0), true, tune.cooling());
        let stoich = fuel_of(&tune).stoich;
        let mut e = Engine {
            disp_l: bottom.displacement_l,
            bottom,
            geometry,
            tune,
            rpm: ENGINE.idle_rpm,
            map: 30.0,
            tps: 0.0,
            idle_air: 2.0,
            idle_i: 2.0,
            high_cam: false,
            vtec_cmd: false,
            vtec_pins: [0.0; 4],
            cam_f: 0.0,
            oil_kpa: 0.0,
            crank_deg: 0.0,
            rev_cut: false,
            dfco: false,
            ign: 10.0,
            ign_table: 10.0,
            mbt: 0.0,
            knock_retard: 0.0,
            knock_limit: 30.0,
            knock_index: 0.0,
            knocking: false,
            knock_count: 0,
            knock_acc: 0.0,
            lambda: 1.0,
            lambda_target: 1.0,
            afr: stoich,
            fuel_pump_pct: 0.0,
            fuel_pressure_kpa: RAIL_KPA,
            ipw: 2.0,
            inj_duty: 0.0,
            torque_nm: 0.0,
            pumping_nm: 0.0,
            air_gs: 0.0,
            air_delivered_gs: 0.0,
            ve: 0.9,
            iat: 25.0,
            ect: 90.0,
            oil_c: 100.0,
            fuel_gs: 0.0,
            friction_nm: 0.0,
            thermal,
            boost: 0.0,
            ic_drop_kpa: 0.0,
            charge_out_c: 25.0,
            boost_target: f64::NAN,
            wg_duty: 0.0,
            wg_open: 0.0,
            turbo_rad_s: 0.0,
            turbine_kw: 0.0,
            compressor_kw: 0.0,
            boost_cut: false,
            over_torque: false,
            vtc: 0.0,
            pmep_kpa: 0.0,
            exh_kpa: P_AMB,
            egt_c: 600.0,
            residual: 0.0,
            ca50: f64::NAN,
            burn_deg: 0.0,
            cyl_pmax_bar: f64::NAN,
            spark_eff: 1.0,
            gas: Gas::new(g.clone()),
            gas_hi: Gas::new(g),
            gas_lo_out: None,
            gas_hi_out: None,
            gas_out: GasOut::default(),
            gas_t: 0.0,
            lo_idle: 0.0,
            hi_idle: 1.0,
            gas_dirty: false,
        };
        e.update_gas(0.0, true);
        e
    }

    /// Swap the tune. Changes to parts or flow settings rebuild the gas model on the next step.
    pub fn set_tune(&mut self, tune: Tune) {
        let geometry_changed = tune.parts != self.tune.parts || tune.engine != self.tune.engine || tune.flow != self.tune.flow || tune.materials != self.tune.materials;
        self.tune = tune;
        self.thermal.g = self.tune.cooling();
        if geometry_changed {
            self.bottom = bottom_end(&self.tune);
            self.geometry = engine_geometry(&self.tune, &self.bottom);
            self.disp_l = self.bottom.displacement_l;
            self.gas_dirty = true;
        }
    }

    pub fn flow(&self) -> &Flow {
        &self.gas.g
    }

    fn exhaust_outlet(&self) -> (f64, f64) {
        let tb = &self.tune.turbo;
        if tb.enabled && !tb.fixed {
            return (P_AMB + DOWNPIPE_KPA, (tb.turbine_area_cm2 + tb.wastegate_area_cm2 * self.wg_open) * 1e-4);
        }
        (P_AMB, PI / 4.0 * self.gas.g.exhaust_dia_m.powi(2))
    }

    fn gas_conditions(&mut self, high: bool) -> Cond {
        let cut = self.rev_cut || self.dfco || self.boost_cut;
        let vtc = vtc_for(&self.tune, self.rpm, self.map, self.high_cam);
        self.vtc = vtc;
        let (out_kpa, out_area) = self.exhaust_outlet();
        let fuel = fuel_of(&self.tune);
        let rf = ref_fuel();
        Cond {
            rpm: self.rpm.max(300.0),
            map_kpa: self.map,
            iat_c: self.iat,
            high_cam: high,
            vtc,
            ign_deg: self.ign,
            lambda: self.lambda.min(3.0),
            fire: !cut && self.rpm > 200.0,
            octane: fuel.octane,
            energy: fuel.lhv_mj / fuel.stoich / (rf.lhv_mj / rf.stoich),
            cool_k: extra_cooling_k(&fuel, self.lambda.min(3.0)),
            wall_c: self.ect,
            out_kpa,
            out_area_m2: Some(out_area),
        }
    }

    fn update_gas(&mut self, dt: f64, mut force: bool) {
        if self.gas_dirty {
            let g = flow_geom(&self.tune, &self.bottom);
            self.gas = Gas::new(g.clone());
            self.gas_hi = Gas::new(g);
            self.gas_dirty = false;
            force = true;
        }
        self.gas_t += dt;
        if !force && self.gas_t < GAS_EVERY_S {
            return;
        }
        self.gas_t = 0.0;
        let f = self.cam_f;
        if f < 1.0 {
            let c = self.gas_conditions(false);
            let idle = self.lo_idle;
            let o = if force || idle > 0.2 { self.gas.settle(&c, if force { 12 } else { 4 }) } else { self.gas.run_cycle(&c) };
            self.gas_lo_out = Some(o);
            self.lo_idle = 0.0;
        } else {
            self.lo_idle += GAS_EVERY_S;
        }
        if f > 0.0 {
            let c = self.gas_conditions(true);
            let idle = self.hi_idle;
            let o = if force || idle > 0.2 { self.gas_hi.settle(&c, if force { 12 } else { 4 }) } else { self.gas_hi.run_cycle(&c) };
            self.gas_hi_out = Some(o);
            self.hi_idle = 0.0;
        } else {
            self.hi_idle += GAS_EVERY_S;
        }
        let lo = self.gas_lo_out.unwrap_or_default();
        let hi = self.gas_hi_out.unwrap_or_default();
        self.gas_out = if f <= 0.0 { lo } else if f >= 1.0 { hi } else { GasOut::blend(&lo, &hi, f) };
    }

    fn step_vtec(&mut self, dt: f64) {
        let t = &self.tune;
        let rpm = self.rpm;
        let want = rpm >= t.vtec_rpm && self.map >= t.vtec_min_map && self.ect >= self.thermal.g.ect_vtec_min_c;
        self.vtec_cmd = if self.vtec_cmd { rpm >= t.vtec_rpm - 150.0 && self.map >= t.vtec_min_map - 5.0 } else { want };
        self.oil_kpa = oil_pressure_kpa(rpm, self.oil_c, &self.thermal.g, Some(self.bottom.pump));
        let pressurised = self.vtec_cmd && self.oil_kpa >= VTEC_OIL_KPA;
        let g = &self.gas.g;
        for i in 0..4 {
            let theta = self.crank_deg - CYL_OFFSET[i];
            let on_base = valve_lift_m(theta, true, false, self.vtc, g) == 0.0
                && valve_lift_m(theta, true, true, self.vtc, g) == 0.0
                && valve_lift_m(theta, false, false, 0.0, g) == 0.0
                && valve_lift_m(theta, false, true, 0.0, g) == 0.0;
            if on_base || (self.vtec_pins[i] > 0.0 && self.vtec_pins[i] < 1.0) {
                self.vtec_pins[i] = clamp(self.vtec_pins[i] + if pressurised { 1.0 } else { -1.0 } * dt / PIN_TRAVEL_S, 0.0, 1.0);
            }
        }
        self.cam_f = self.vtec_pins.iter().filter(|&&p| p >= 1.0).count() as f64 / 4.0;
        self.high_cam = pressurised;
    }

    /// Advance dt seconds with the pedal at 0–1. Returns brake torque at the crank (Nm).
    pub fn step(&mut self, pedal: f64, dt: f64) -> f64 {
        let rpm = self.rpm.max(1.0);

        let idle_rpm = cold_idle_rpm(self.ect);
        let idle_err = idle_rpm - rpm;
        if pedal < 0.02 && rpm < idle_rpm + 600.0 {
            self.idle_i = clamp(self.idle_i + idle_err * 0.004 * dt, 0.0, 10.0);
            self.idle_air = clamp(self.idle_i + idle_err * 0.006, 0.0, 15.0);
        } else {
            self.idle_air += ((if pedal < 0.02 { DECEL_AIR } else { self.idle_i }) - self.idle_air) * (dt * 2.0).min(1.0);
        }
        self.tps = clamp(pedal * 100.0 + if pedal < 0.02 { self.idle_air } else { 0.0 }, 0.0, 100.0);

        // Turbo energy balance.
        let tb = self.tune.turbo.clone();
        self.boost_target = self.tune.boost_target.as_ref().map(|b| lookup2(b, rpm, self.map)).unwrap_or(f64::NAN);
        self.wg_duty = 0.0;
        if tb.enabled {
            let spring = tb.spring_kpa.min(tb.max_boost_kpa);
            self.wg_duty = self.tune.wg_duty.as_ref().map(|t| clamp(lookup2(t, rpm, self.map), 0.0, 100.0)).unwrap_or(0.0);
            let wastegate_limit = spring + clamp(self.wg_duty / 100.0 * tb.wg_gain, 0.0, 1.0) * (tb.max_boost_kpa - spring);
            if tb.fixed {
                let want = if self.boost_cut || self.rev_cut || self.dfco { 0.0 } else { wastegate_limit };
                self.boost += (want - self.boost) * (dt * 20.0).min(1.0);
            } else {
                let g = self.gas_out;
                let air_kg_s = self.air_delivered_gs / 1000.0;
                let exh_kg_s = air_kg_s * (1.0 + if self.lambda < 5.0 { 1.0 / (fuel_of(&self.tune).stoich * self.lambda) } else { 0.0 });
                let share = tb.turbine_area_cm2 / (tb.turbine_area_cm2 + tb.wastegate_area_cm2 * self.wg_open);
                let p4 = P_AMB + DOWNPIPE_KPA;
                let er = if g.exh_kpa > p4 { 1.0 - (p4 / g.exh_kpa).powf(0.248) } else { 0.0 };
                let turbine_w = tb.turbine_eff * exh_kg_s * share * 1150.0 * g.exh_temp_k * er;
                let pr = (P_AMB + self.boost) / (P_AMB - 1.0);
                let compressor_w = air_kg_s * 1005.0 * 298.0 * (pr.powf(0.2857) - 1.0) / tb.compressor_eff;
                let w = self.turbo_rad_s.max(500.0);
                let i = tb.inertia_kg_m2;
                let pr_at = |rad_s: f64| (1.0 + 0.7 * tb.compressor_eff * (rad_s * tb.compressor_tip_mm / 2000.0).powi(2) / (1005.0 * 298.0)).powf(3.5);
                let pr_max = (P_AMB + tb.max_boost_kpa) / (P_AMB - 1.0);
                let max_rad_s = ((pr_max.powf(1.0 / 3.5) - 1.0) * 1005.0 * 298.0 / (0.7 * tb.compressor_eff)).sqrt() * 2000.0 / tb.compressor_tip_mm;
                self.turbo_rad_s = clamp(self.turbo_rad_s + (turbine_w - compressor_w - 2e-6 * w * w) / (i * w) * dt, 0.0, max_rad_s);
                let pr_wheel = pr_at(self.turbo_rad_s);
                self.boost = clamp((P_AMB - 1.0) * pr_wheel - P_AMB, 0.0, tb.max_boost_kpa);
                let open = clamp((self.boost - wastegate_limit) / 6.0, 0.0, 1.0);
                self.wg_open += (open - self.wg_open) * (dt * 40.0).min(1.0);
                self.turbine_kw = turbine_w / 1000.0;
                self.compressor_kw = compressor_w / 1000.0;
            }
        } else {
            self.boost = 0.0;
            self.turbo_rad_s = 0.0;
        }
        // Charge air temperature: compressor heat, intercooler, underhood.
        let amb_c = self.tune.ambient_c.unwrap_or(25.0);
        let mut charge_c = amb_c + 3.0;
        let mut ic_drop = 0.0;
        if tb.enabled && self.boost > 0.5 {
            let pr_c = (P_AMB + self.boost) / (P_AMB - 1.0);
            let comp_out_c = (amb_c + 273.0) * (1.0 + (pr_c.powf(0.2857) - 1.0) / tb.compressor_eff) - 273.0;
            charge_c = if tb.intercooler { comp_out_c - tb.ic_effectiveness * (comp_out_c - amb_c) } else { comp_out_c };
            if tb.intercooler {
                ic_drop = tb.ic_drop_kpa_at250_gs * (self.air_delivered_gs / 250.0).powi(2);
            }
        }
        self.charge_out_c = charge_c;
        self.ic_drop_kpa = ic_drop;
        self.iat += (charge_c - self.iat) * (dt * 0.5).min(1.0);

        // Manifold pressure.
        self.update_gas(dt, false);
        let corr = ve_correction(rpm, false) + (ve_correction(rpm, true) - ve_correction(rpm, false)) * self.cam_f;
        let g = self.gas_out;
        let map_target = manifold_pressure(self.tps, rpm, g.ve_delivered * corr, 101.3 + self.boost - self.ic_drop_kpa, self.iat, self.gas.g.throttle_dia_m, self.disp_l);
        self.map += (map_target - self.map) * (dt * 12.0).min(1.0);

        self.crank_deg = (self.crank_deg + rpm * 6.0 * dt) % 720.0;
        self.step_vtec(dt);

        let t = &self.tune;
        self.dfco = pedal < 0.02 && rpm > 1500.0;
        if rpm > t.rev_limit {
            self.rev_cut = true;
        } else if rpm < t.rev_limit - t.rev_limit_hyst {
            self.rev_cut = false;
        }
        match t.overboost_kpa {
            Some(ob) if ob != 0.0 && self.map > ob => self.boost_cut = true,
            _ => {
                if self.map < t.overboost_kpa.unwrap_or(0.0) - 20.0 {
                    self.boost_cut = false;
                }
            }
        }

        let ve = g.ve * corr;
        self.ve = ve;
        let density = self.disp_l * AIR_DENSITY_G_PER_L * (self.map / 101.3) * (298.0 / (273.0 + self.iat));
        let air_per_cycle = ve * density;
        self.air_gs = air_per_cycle * rpm / 120.0;
        self.air_delivered_gs = (g.ve_delivered * corr).max(ve) * density * rpm / 120.0;
        self.pmep_kpa = g.pmep_kpa;
        self.exh_kpa = g.exh_kpa;
        self.egt_c = g.egt_c;
        self.residual = g.residual;

        // Fuel the ECU commands.
        let t = &self.tune;
        let fuel_type = fuel_of(t);
        let stoich = fuel_type.stoich;
        let fuel_table = if self.high_cam { t.fuel_high.as_ref().unwrap_or(&t.fuel_low) } else { &t.fuel_low };
        let units = t.fuel_units.clone().filter(|u| !u.is_empty()).unwrap_or_else(|| guess_fuel_units(fuel_table).to_string());
        let fuel_val = lookup2(fuel_table, rpm, self.map);
        let inj_g_per_ms = injector_g_per_ms(t.injector_cc, &fuel_type, self.map);
        let dead_ms = t.injector_dead_ms;
        let mut fuel_per_cyl;
        if units == "ms" {
            self.ipw = fuel_val;
            fuel_per_cyl = (fuel_val - dead_ms).max(0.0) * inj_g_per_ms;
            self.lambda_target = f64::NAN;
        } else {
            self.lambda_target = if units == "afr" { fuel_val / stoich } else { fuel_val };
            fuel_per_cyl = air_per_cycle / ENGINE.cylinders / (stoich * self.lambda_target);
            self.ipw = fuel_per_cyl / inj_g_per_ms + dead_ms;
        }
        let cycle_ms = 120000.0 / rpm;
        self.inj_duty = clamp(self.ipw / cycle_ms * 100.0, 0.0, 100.0);
        if self.ipw > cycle_ms {
            fuel_per_cyl = (cycle_ms - dead_ms) * inj_g_per_ms;
        }
        if self.rev_cut || self.dfco || self.boost_cut {
            fuel_per_cyl = 0.0;
        }
        let pump_max = pump_gs(pump_lph_of(t), &fuel_type);
        let want_gs = fuel_per_cyl * ENGINE.cylinders * rpm / 120.0;
        self.fuel_pump_pct = 100.0 * want_gs / pump_max;
        self.fuel_pressure_kpa = RAIL_KPA * (pump_max / want_gs.max(1e-9)).min(1.0).powi(2);
        if want_gs > pump_max {
            fuel_per_cyl *= pump_max / want_gs;
        }
        let fuel = fuel_per_cyl * ENGINE.cylinders;
        self.fuel_gs = fuel * rpm / 120.0;
        self.lambda = if fuel > 0.0 { air_per_cycle / (fuel * stoich) } else { 9.99 };
        self.afr = self.lambda * stoich;

        // Ignition and knock.
        let ign_tab = if self.high_cam { t.ign_high.as_ref().unwrap_or(&t.ign_low) } else { &t.ign_low };
        self.ign_table = lookup2(ign_tab, rpm, self.map);
        self.ign = self.ign_table - self.knock_retard;
        self.mbt = mbt(rpm, self.map);
        let fired = !self.rev_cut && !self.dfco && !self.boost_cut;
        self.knock_index = if fired { g.knock_index } else { 0.0 };
        self.knock_limit = clamp(self.ign - self.knock_index.max(1e-3).ln() / 0.055, self.ign - 15.0, self.ign + 15.0);
        self.knocking = fired && self.knock_index >= 1.0 && self.map > 50.0;
        if self.knocking {
            self.knock_acc += dt * (1.0 + self.ign - self.knock_limit);
            if self.knock_acc > 0.1 {
                self.knock_acc = 0.0;
                self.knock_count += 1;
                self.knock_retard = (self.knock_retard + 1.0).min(8.0);
            }
        } else {
            self.knock_retard = (self.knock_retard - dt * 0.5).max(0.0);
        }

        // Torque.
        self.ca50 = g.ca50_deg;
        self.burn_deg = g.burn_deg;
        self.cyl_pmax_bar = g.pmax_bar * corr;
        let indicated = if fired { g.imep_kpa * corr * self.disp_l / (4.0 * PI) } else { g.imep_kpa * self.disp_l / (4.0 * PI) };
        self.pumping_nm = self.pmep_kpa * self.disp_l / (4.0 * PI);
        // Friction rises with oil viscosity (the hydrodynamic part; the fixed part is seals and mixed contact).
        let balancer = if self.bottom.pump.balance_shafts { 0.0 } else { balancer_nm(rpm) };
        self.friction_nm = 5.0 + (friction_nm(rpm) - 5.0 - balancer) * visc_ratio(self.oil_c, &self.thermal.g).powf(0.35);
        self.torque_nm = indicated - self.friction_nm - self.pumping_nm;
        self.spark_eff = clamp(1.0 - ((self.ign - self.mbt) / 28.0).powi(2), 0.2, 1.0);
        self.over_torque = self.torque_nm > self.tune.torque_limit_nm.unwrap_or(f64::INFINITY);
        self.torque_nm
    }
}

const PLATE_CLOSED_DEG: f64 = 7.0;
const THROTTLE_CD: f64 = 0.75;
const LEAK_M2: f64 = 5e-6;
const R_AIR: f64 = 287.0;
const GAMMA_AIR: f64 = 1.4;

pub fn throttle_area_m2(tps: f64, dia_m: f64) -> f64 {
    let full = PI / 4.0 * dia_m.powi(2);
    let a = (PLATE_CLOSED_DEG + clamp(tps, 0.0, 100.0) / 100.0 * (90.0 - PLATE_CLOSED_DEG)) * PI / 180.0;
    LEAK_M2 + full * (1.0 - a.cos() / (PLATE_CLOSED_DEG * PI / 180.0).cos())
}

fn orifice_flow(area: f64, up_kpa: f64, down_kpa: f64, temp_k: f64) -> f64 {
    let g = GAMMA_AIR;
    let pr = clamp(down_kpa / up_kpa, 0.0, 1.0);
    let crit = (2.0 / (g + 1.0)).powf(g / (g - 1.0));
    let psi = if pr <= crit {
        (g * (2.0 / (g + 1.0)).powf((g + 1.0) / (g - 1.0))).sqrt()
    } else {
        (2.0 * g / (g - 1.0) * (pr.powf(2.0 / g) - pr.powf((g + 1.0) / g))).sqrt()
    };
    THROTTLE_CD * area * up_kpa * 1000.0 / (R_AIR * temp_k).sqrt() * psi
}

/// Manifold pressure where flow through the throttle balances what the engine pumps.
pub fn manifold_pressure(tps: f64, rpm: f64, ve: f64, upstream_kpa: f64, iat_c: f64, throttle_dia_m: f64, disp_l: f64) -> f64 {
    let t = 273.0 + iat_c;
    let area = throttle_area_m2(tps, throttle_dia_m);
    let pumped = |p: f64| ve * disp_l / 1000.0 * rpm.max(1.0) / 120.0 * p * 1000.0 / (R_AIR * t);
    let (mut lo, mut hi) = (0.5, upstream_kpa);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if orifice_flow(area, upstream_kpa, mid, t) > pumped(mid) { lo = mid } else { hi = mid }
    }
    (lo + hi) / 2.0
}
