//! Gas exchange: one cylinder followed degree by degree through a four-stroke cycle (src/gasflow.js).
//! Filling-and-emptying model with compressible valve flow, inertial intake runner and exhaust primary,
//! a shared collector, Wiebe heat release from the spark and a Livengood–Wu knock integral.

use crate::tables::{clamp, lookup1, lookup2};
use crate::tune::Tune;
use serde::Serialize;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lobes {
    pub in_lift: f64,
    pub in_dur: f64,
    pub ex_lift: f64,
    pub ex_dur: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub bore_m: f64,
    pub stroke_m: f64,
    pub rod_m: f64,
    pub compression: f64,
    pub in_valve_m: f64,
    pub ex_valve_m: f64,
    pub valves: f64,
    pub stem_m: f64,
    pub cams_low: Lobes,
    pub cams_high: Lobes,
    pub intake_centre0: f64,
    pub exhaust_centre: f64,
    pub valve_cd: f64,
    pub throttle_dia_m: f64,
    pub runner_m: f64,
    pub runner_dia_m: f64,
    pub intake_port_l: f64,
    pub primary_m: f64,
    pub primary_dia_m: f64,
    pub exhaust_port_l: f64,
    pub collector_l: f64,
    pub exhaust_dia_m: f64,
    pub charge_heat_k: f64,
    pub burn_deg: f64,
    pub heat_loss: f64,
    pub knock_scale: f64,
}

/// Stock geometry. Published: bore, stroke, compression, valve sizes and count, stems. The rest are
/// estimates (see src/gasflow.js and docs/GASFLOW.md).
pub fn flow_defaults() -> Flow {
    Flow {
        bore_m: 0.086,
        stroke_m: 0.0859,
        rod_m: 0.139,
        compression: 11.0,
        in_valve_m: 0.035,
        ex_valve_m: 0.030,
        valves: 2.0,
        stem_m: 0.0055,
        cams_low: Lobes { in_lift: 0.0095, in_dur: 228.0, ex_lift: 0.0090, ex_dur: 222.0 },
        cams_high: Lobes { in_lift: 0.0117, in_dur: 262.0, ex_lift: 0.0110, ex_dur: 256.0 },
        intake_centre0: 130.0,
        exhaust_centre: 105.0,
        valve_cd: 0.72,
        throttle_dia_m: 0.062,
        runner_m: 0.15,
        runner_dia_m: 0.044,
        intake_port_l: 0.40,
        primary_m: 0.15,
        primary_dia_m: 0.036,
        exhaust_port_l: 0.35,
        collector_l: 1.6,
        exhaust_dia_m: 0.052,
        charge_heat_k: 30.0,
        burn_deg: 62.0,
        heat_loss: 0.30,
        knock_scale: 2.0,
    }
}

impl Flow {
    pub fn cams(&self, high: bool) -> &Lobes {
        if high { &self.cams_high } else { &self.cams_low }
    }
}

const R: f64 = 287.0;
const GAMMA: f64 = 1.35;
const CV: f64 = R / (GAMMA - 1.0);
const CP: f64 = CV + R;
const LHV: f64 = 44e6;
const STOICH: f64 = 14.7;
const T_WALL_EXH: f64 = 700.0;

/// Stock VTC (intake cam advance, crank degrees), shaped after Hondata's description.
pub fn stock_vtc(rpm: f64, map_kpa: f64, high_cam: bool) -> f64 {
    let load = clamp((map_kpa - 30.0) / 60.0, 0.0, 1.0);
    if high_cam {
        lookup1(&[5000., 6000., 7000., 8000., 9000.], &[45., 50., 40., 30., 25.], rpm) * (0.4 + 0.6 * load)
    } else {
        lookup1(&[750., 1500., 2500., 3500., 5000., 6000., 7000.], &[0., 10., 25., 40., 40., 35., 25.], rpm) * load
    }
}

/// Cam angle from the tune's cam angle tables, otherwise the stock schedule. 0–50 crank degrees.
pub fn vtc_for(tune: &Tune, rpm: f64, map_kpa: f64, high_cam: bool) -> f64 {
    let t = if high_cam { tune.vtc_high.as_ref().or(tune.vtc_low.as_ref()) } else { tune.vtc_low.as_ref() };
    clamp(t.map(|t| lookup2(t, rpm, map_kpa)).unwrap_or_else(|| stock_vtc(rpm, map_kpa, high_cam)), 0.0, 50.0)
}

fn lift_at(theta: f64, centre: f64, dur: f64, max_lift: f64) -> f64 {
    let d = ((theta - centre) % 720.0 + 1080.0) % 720.0 - 360.0;
    if d.abs() >= dur / 2.0 {
        return 0.0;
    }
    max_lift * (PI * (d / dur + 0.5)).sin().powf(1.6)
}

/// Mass flow (kg/s) through an orifice; positive from a to b.
fn flow(area: f64, pa: f64, ta: f64, pb: f64, tb: f64, g: f64) -> f64 {
    if area <= 0.0 {
        return 0.0;
    }
    let (mut pa, mut pb, mut ta) = (pa, pb, ta);
    let mut sign = 1.0;
    if pb > pa {
        std::mem::swap(&mut pa, &mut pb);
        ta = tb;
        sign = -1.0;
    }
    let pr = pb / pa;
    let crit = (2.0 / (g + 1.0)).powf(g / (g - 1.0));
    let psi = if pr <= crit {
        (g * (2.0 / (g + 1.0)).powf((g + 1.0) / (g - 1.0))).sqrt()
    } else if pr > 0.999 {
        (2.0 * g / (g - 1.0) * (0.999f64.powf(2.0 / g) - 0.999f64.powf((g + 1.0) / g))).sqrt() * (1.0 - pr) / 0.001
    } else {
        (2.0 * g / (g - 1.0) * (pr.powf(2.0 / g) - pr.powf((g + 1.0) / g))).sqrt()
    };
    sign * area * pa / (R * ta).sqrt() * psi
}

/// Keep one step's flow from overshooting the point where the two volumes reach equal pressure.
fn limit(m: f64, pa: f64, ta: f64, va: f64, pb: f64, tb: f64, vb: f64) -> f64 {
    let eq = (pa - pb).abs() / (1.4 * R * (ta / va + if vb != 0.0 { tb / vb } else { 0.0 }));
    clamp(m, -0.7 * eq, 0.7 * eq)
}

/// Valve lift (m) for a cylinder at theta_deg from its firing TDC (overlap TDC at ±360).
pub fn valve_lift_m(theta_deg: f64, intake: bool, high_cam: bool, vtc: f64, g: &Flow) -> f64 {
    let cam = g.cams(high_cam);
    let th = ((theta_deg + 360.0) % 720.0 + 720.0) % 720.0 - 360.0;
    if intake {
        lift_at(th, -360.0 + g.intake_centre0 - vtc, cam.in_dur, cam.in_lift)
    } else {
        lift_at(th, 360.0 - g.exhaust_centre, cam.ex_dur, cam.ex_lift)
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lobe {
    pub lift_mm: f64,
    pub duration_deg: f64,
    pub duration_seat_deg: f64,
    pub centerline_deg: f64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CamCard {
    pub intake_low: Lobe,
    pub intake_high: Lobe,
    pub exhaust_low: Lobe,
    pub exhaust_high: Lobe,
}

/// The cam card: lift, duration at 1 mm of valve lift, and lobe centreline.
pub fn cam_card(g: &Flow) -> CamCard {
    let lobe = |lift: f64, dur: f64, centre: f64| {
        let x = 2.0 * (0.001 / lift).powf(1.0 / 1.6).asin() / PI;
        Lobe { lift_mm: (lift * 1000.0 * 100.0).round() / 100.0, duration_deg: (dur * (1.0 - x) * 10.0).round() / 10.0, duration_seat_deg: dur, centerline_deg: centre }
    };
    let (l, h) = (&g.cams_low, &g.cams_high);
    CamCard {
        intake_low: lobe(l.in_lift, l.in_dur, g.intake_centre0),
        intake_high: lobe(h.in_lift, h.in_dur, g.intake_centre0),
        exhaust_low: lobe(l.ex_lift, l.ex_dur, g.exhaust_centre),
        exhaust_high: lobe(h.ex_lift, h.ex_dur, g.exhaust_centre),
    }
}

/// Conditions for one cycle.
#[derive(Clone, Copy, Debug)]
pub struct Cond {
    pub rpm: f64,
    pub map_kpa: f64,
    pub iat_c: f64,
    pub high_cam: bool,
    pub vtc: f64,
    pub ign_deg: f64,
    pub lambda: f64,
    pub fire: bool,
    pub octane: f64,
    pub out_kpa: f64,
    /// The whole engine's exhaust outlet; None = the exhaust system orifice.
    pub out_area_m2: Option<f64>,
}

impl Default for Cond {
    fn default() -> Self {
        Cond { rpm: 3000.0, map_kpa: 100.0, iat_c: 25.0, high_cam: false, vtc: 0.0, ign_deg: 20.0, lambda: 1.0, fire: true, octane: 91.0, out_kpa: 101.3, out_area_m2: None }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GasOut {
    pub ve: f64,
    pub ve_delivered: f64,
    pub residual: f64,
    pub burn_deg: f64,
    /// NaN when the charge didn't burn.
    pub ca50_deg: f64,
    pub knock_index: f64,
    pub knock_at_deg: f64,
    pub knock_unburned: f64,
    pub imep_kpa: f64,
    pub pmep_kpa: f64,
    pub pmax_bar: f64,
    pub pmax_deg: f64,
    pub egt_c: f64,
    pub exh_kpa: f64,
    pub exh_temp_k: f64,
    pub exh_gs: f64,
}

impl GasOut {
    /// Mix two results by f (0 = a, 1 = b); a non-finite value takes whichever side is nearer.
    pub fn blend(a: &GasOut, b: &GasOut, f: f64) -> GasOut {
        let m = |x: f64, y: f64| if x.is_finite() && y.is_finite() { x + (y - x) * f } else if f < 0.5 { x } else { y };
        GasOut {
            ve: m(a.ve, b.ve),
            ve_delivered: m(a.ve_delivered, b.ve_delivered),
            residual: m(a.residual, b.residual),
            burn_deg: m(a.burn_deg, b.burn_deg),
            ca50_deg: m(a.ca50_deg, b.ca50_deg),
            knock_index: m(a.knock_index, b.knock_index),
            knock_at_deg: m(a.knock_at_deg, b.knock_at_deg),
            knock_unburned: m(a.knock_unburned, b.knock_unburned),
            imep_kpa: m(a.imep_kpa, b.imep_kpa),
            pmep_kpa: m(a.pmep_kpa, b.pmep_kpa),
            pmax_bar: m(a.pmax_bar, b.pmax_bar),
            pmax_deg: m(a.pmax_deg, b.pmax_deg),
            egt_c: m(a.egt_c, b.egt_c),
            exh_kpa: m(a.exh_kpa, b.exh_kpa),
            exh_temp_k: m(a.exh_temp_k, b.exh_temp_k),
            exh_gs: m(a.exh_gs, b.exh_gs),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gas {
    pub g: Flow,
    ap: f64,
    vd: f64,
    vc: f64,
    m: f64,
    t: f64,
    xb: f64,
    p_in: f64,
    xb_in: f64,
    m_run: f64,
    p_ex: f64,
    t_ex: f64,
    p_col: f64,
    t_col: f64,
    m_pri: f64,
    primed: bool,
    pub out: GasOut,
}

impl Gas {
    pub fn new(g: Flow) -> Gas {
        let ap = PI / 4.0 * g.bore_m.powi(2);
        let vd = ap * g.stroke_m;
        let vc = vd / (g.compression - 1.0);
        Gas { g, ap, vd, vc, m: 0.0, t: 800.0, xb: 0.1, p_in: 100e3, xb_in: 0.0, m_run: 0.0, p_ex: 105e3, t_ex: 900.0, p_col: 105e3, t_col: 900.0, m_pri: 0.0, primed: false, out: GasOut::default() }
    }

    fn volume_at(&self, theta_deg: f64) -> f64 {
        let r = self.g.stroke_m / 2.0;
        let l = self.g.rod_m;
        let a = theta_deg * PI / 180.0;
        let x = r * (1.0 - a.cos()) + l - (l * l - (r * a.sin()).powi(2)).sqrt();
        self.vc + self.ap * x
    }

    /// One full cycle, overlap TDC to overlap TDC with firing TDC at 0°.
    pub fn run_cycle(&mut self, c: &Cond) -> GasOut {
        self.run_cycle_step(c, 1.0)
    }

    pub fn run_cycle_step(&mut self, c: &Cond, step_deg: f64) -> GasOut {
        let g = self.g.clone();
        let rpm = c.rpm.max(300.0);
        let cam = *g.cams(c.high_cam);
        let p0 = c.map_kpa * 1000.0;
        let tm = 273.0 + c.iat_c;
        let t0 = tm + g.charge_heat_k * (3000.0 / rpm).powf(0.3);
        let p_out = c.out_kpa * 1000.0;
        let out_area = c.out_area_m2.unwrap_or(PI / 4.0 * g.exhaust_dia_m.powi(2)) / 4.0;
        let in_centre = -360.0 + g.intake_centre0 - c.vtc;
        let ex_centre = 360.0 - g.exhaust_centre;
        let throat_in = g.valves * (PI / 4.0 * (0.88 * g.in_valve_m).powi(2) - PI / 4.0 * g.stem_m.powi(2));
        let throat_ex = g.valves * (PI / 4.0 * (0.88 * g.ex_valve_m).powi(2) - PI / 4.0 * g.stem_m.powi(2));
        let a_run = PI / 4.0 * g.runner_dia_m.powi(2);
        let a_pri = PI / 4.0 * g.primary_dia_m.powi(2);
        let v_in = g.intake_port_l / 1000.0;
        let v_ex = g.exhaust_port_l / 1000.0;
        let v_col = g.collector_l / 1000.0;
        let dt = step_deg / (6.0 * rpm);

        if !self.primed {
            self.p_in = p0;
            self.p_ex = p_out.max(101.3e3);
            self.p_col = self.p_ex;
            self.m = p0 * self.volume_at(-360.0) / (R * 900.0);
            self.t = 900.0;
            self.primed = true;
        }

        let mut v = self.volume_at(-360.0);
        let mut m_fresh = self.m * (1.0 - self.xb);
        let mut m_tot = self.m;
        let (mut pump_w, mut gross_w, mut pmax, mut pmax_deg, mut q_total, mut xb_prev) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut trapped_fresh, mut in_fresh, mut ex_out_mass, mut ex_out_energy) = (0.0, 0.0, 0.0, 0.0);
        let (mut col_out, mut col_p, mut col_pt, mut n) = (0.0, 0.0, 0.0, 0.0);
        let theta_s = -c.ign_deg;
        let octane = c.octane;
        let lambda = c.lambda;
        let (mut ref_taken, mut p_ref, mut t_ref, mut residual, mut burn_dur) = (false, 0.0, 0.0, 0.0, g.burn_deg);
        let mut ca50: Option<f64> = None;
        let (mut knock_i, mut knock_at, mut knock_unburned): (f64, Option<f64>, f64) = (0.0, None, 0.0);
        let mut m_ex = self.p_ex * v_ex / (R * self.t_ex);
        let mut m_col = self.p_col * v_col / (R * self.t_col);
        let mut m_in_port = self.p_in * v_in / (R * t0);

        let mut th = -360.0;
        while th < 360.0 {
            let thm = th + step_deg / 2.0;
            let v2 = self.volume_at(th + step_deg);
            let dv = v2 - v;
            let p = m_tot * R * self.t / v;

            let li = lift_at(thm, in_centre, cam.in_dur, cam.in_lift);
            let le = lift_at(thm, ex_centre, cam.ex_dur, cam.ex_lift);
            let a_iv = if li > 1e-4 { g.valve_cd * (g.valves * PI * g.in_valve_m * li).min(throat_in) } else { 0.0 };
            let a_ev = if le > 1e-4 { g.valve_cd * (g.valves * PI * g.ex_valve_m * le).min(throat_ex) } else { 0.0 };

            let f_iv = limit(flow(a_iv, self.p_in, t0, p, self.t, 1.4) * dt, self.p_in, t0, v_in, p, self.t, v);
            let f_ev = limit(flow(a_ev, p, self.t, self.p_ex, self.t_ex, GAMMA) * dt, p, self.t, v, self.p_ex, self.t_ex, v_ex);
            let f_out = limit(flow(out_area, self.p_col, self.t_col, p_out, 300.0, 1.33) * dt, self.p_col, self.t_col, v_col, p_out, 300.0, 0.0);

            let rho_run = p0 / (R * tm);
            let u_run = self.m_run / (rho_run * a_run);
            self.m_run += (a_run / g.runner_m) * ((p0 - self.p_in) - 0.02 * (g.runner_m / g.runner_dia_m) * 0.5 * rho_run * u_run * u_run.abs()) * dt;
            let rho_pri = self.p_ex / (R * self.t_ex);
            let u_pri = self.m_pri / (rho_pri * a_pri);
            self.m_pri += (a_pri / g.primary_m) * ((self.p_ex - self.p_col) - 0.03 * (g.primary_m / g.primary_dia_m) * 0.5 * rho_pri * u_pri * u_pri.abs()) * dt;
            let f_run = self.m_run * dt;
            let f_pri = self.m_pri * dt;

            let xb_cyl = if m_tot > 0.0 { 1.0 - m_fresh / m_tot } else { 0.0 };
            let port_burned = self.xb_in * m_in_port - (if f_iv > 0.0 { f_iv * self.xb_in } else { f_iv * xb_cyl }) + (if f_run < 0.0 { f_run * self.xb_in } else { 0.0 });
            m_in_port = (m_in_port + f_run - f_iv).max(1e-7);
            self.xb_in = clamp(port_burned / m_in_port, 0.0, 1.0);
            self.p_in = m_in_port * R * t0 / v_in;

            let fresh_frac = if m_tot > 0.0 { m_fresh / m_tot } else { 0.0 };
            let mut du = -p * dv;
            if f_iv > 0.0 {
                du += 1.4 / 0.4 * R * t0 * f_iv;
                m_fresh += f_iv * (1.0 - self.xb_in);
            } else {
                du += CP * self.t * f_iv;
                m_fresh += f_iv * fresh_frac;
            }
            if f_ev > 0.0 {
                du -= CP * self.t * f_ev;
                m_fresh -= f_ev * fresh_frac;
                ex_out_mass += f_ev;
                ex_out_energy += f_ev * self.t;
            } else {
                du -= CP * self.t_ex * f_ev;
            }
            if f_iv > 0.0 { in_fresh += f_iv * (1.0 - self.xb_in) } else { in_fresh += f_iv * fresh_frac }

            if !ref_taken && (th >= -60.0 || (c.fire && th >= theta_s)) {
                ref_taken = true;
                trapped_fresh = m_fresh;
                p_ref = p;
                t_ref = self.t - if c.fire { 30.0 * clamp((1.0 - lambda) / 0.2, 0.0, 1.0) } else { 0.0 };
                residual = 1.0 - m_fresh / m_tot.max(1e-9);
            }

            if c.fire && th >= theta_s && xb_prev < 0.9999 {
                if q_total == 0.0 {
                    let lam = lambda.max(0.5);
                    burn_dur = g.burn_deg * (1.0 + 2.5 * (residual - 0.04).max(0.0)) * (1.0 + 1.5 * (lam - 1.0).max(0.0) + 0.8 * (0.8 - lam).max(0.0)) * (rpm / 4000.0).powf(0.25);
                    let rich_gain = if lam < 1.0 { clamp(1.0 + 0.045 * (1.0 - ((lam - 0.87) / 0.13).powi(2)), 0.85, 1.045) } else { 1.0 };
                    let misfire = if lam > 1.35 { clamp(1.0 - (lam - 1.35) * 4.0, 0.0, 1.0) } else { 1.0 };
                    q_total = (trapped_fresh / (STOICH * lam)).min(trapped_fresh / STOICH) * LHV * (1.0 - g.heat_loss) * rich_gain * misfire;
                }
                let x = clamp((th + step_deg - theta_s) / burn_dur, 0.0, 1.0);
                let xb = 1.0 - (-6.9 * x.powi(3)).exp();
                du += q_total * (xb - xb_prev);
                m_fresh -= trapped_fresh * (xb - xb_prev) * (1.0f64).min(1.0 / lambda.max(1.0));
                xb_prev = xb;
                if xb > 0.1 && xb < 0.9 && ca50.is_none() && xb >= 0.5 {
                    ca50 = Some(th + step_deg);
                }
            }
            if c.fire && ref_taken && xb_prev < 0.9 && p > p_ref {
                let t_u = t_ref * (p / p_ref).powf((GAMMA - 1.0) / GAMMA);
                let tau = g.knock_scale * 17.68e-3 * (octane / 100.0).powf(3.402) * (p / 101325.0).powf(-1.7) * (3800.0 / t_u).exp();
                knock_i += dt / tau;
                if knock_i >= 1.0 && knock_at.is_none() {
                    knock_at = Some(th);
                    knock_unburned = 1.0 - xb_prev;
                }
            }

            let u = CV * m_tot * self.t + du;
            m_tot = (m_tot + f_iv - f_ev).max(1e-7);
            m_fresh = clamp(m_fresh, 0.0, m_tot);
            self.t = clamp(u / (CV * m_tot), 250.0, 4500.0);
            let p2 = m_tot * R * self.t / v2;
            if (-180.0..180.0).contains(&th) { gross_w += 0.5 * (p + p2) * dv } else { pump_w += 0.5 * (p + p2) * dv }
            if p2 > pmax {
                pmax = p2;
                pmax_deg = th + step_deg;
            }
            v = v2;

            let u_ex = CV * m_ex * self.t_ex + (if f_ev > 0.0 { CP * self.t * f_ev } else { CP * self.t_ex * f_ev }) - CP * (if f_pri > 0.0 { self.t_ex } else { self.t_col }) * f_pri;
            m_ex = (m_ex + f_ev - f_pri).max(1e-7);
            self.t_ex = clamp(u_ex / (CV * m_ex), 300.0, 2000.0);
            self.t_ex += (T_WALL_EXH - self.t_ex) * (dt * 40.0).min(1.0);
            self.p_ex = m_ex * R * self.t_ex / v_ex;
            let u_col = CV * m_col * self.t_col + CP * (if f_pri > 0.0 { self.t_ex } else { self.t_col }) * f_pri - CP * self.t_col * f_out;
            m_col = (m_col + f_pri - f_out).max(1e-7);
            self.t_col = clamp(u_col / (CV * m_col), 300.0, 2000.0);
            self.t_col += (T_WALL_EXH - self.t_col) * (dt * 8.0).min(1.0);
            self.p_col = m_col * R * self.t_col / v_col;
            col_out += f_out;
            col_p += self.p_col;
            col_pt += self.t_col;
            n += 1.0;
            th += step_deg;
        }
        self.m = m_tot;
        self.xb = if m_tot > 0.0 { 1.0 - m_fresh / m_tot } else { 0.0 };

        let rho0 = p0 / (R * tm);
        let cycle_s = 120.0 / rpm;
        self.out = GasOut {
            ve: trapped_fresh / (rho0 * self.vd),
            ve_delivered: in_fresh / (rho0 * self.vd),
            residual,
            burn_deg: burn_dur,
            ca50_deg: ca50.unwrap_or(f64::NAN),
            knock_index: knock_i,
            knock_at_deg: knock_at.unwrap_or(f64::NAN),
            knock_unburned,
            imep_kpa: gross_w / self.vd / 1000.0,
            pmep_kpa: -pump_w / self.vd / 1000.0,
            pmax_bar: pmax / 1e5,
            pmax_deg,
            egt_c: (if ex_out_mass > 0.0 { ex_out_energy / ex_out_mass } else { self.t_ex }) - 273.0,
            exh_kpa: col_p / n / 1000.0,
            exh_temp_k: col_pt / n,
            exh_gs: col_out * 4.0 / cycle_s * 1000.0,
        };
        self.out
    }

    /// Run cycles until the result stops changing.
    pub fn settle(&mut self, c: &Cond, max_cycles: usize) -> GasOut {
        let mut prev: Option<GasOut> = None;
        for _ in 0..max_cycles {
            let o = self.run_cycle(c);
            if let Some(p) = prev {
                if (o.ve - p.ve).abs() < 2e-4 && (o.exh_kpa - p.exh_kpa).abs() < 0.05 {
                    break;
                }
            }
            prev = Some(o);
        }
        self.out
    }
}
