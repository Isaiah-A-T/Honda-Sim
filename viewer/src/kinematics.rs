//! K20Z3 dimensions and the motion of its parts. Lengths in millimetres, angles in degrees.
//! Crank angle 0 = cylinder 1 at top dead centre on its power stroke, counted over the 720° cycle.
//! Cam timing and lift are rough numbers picked to look right, not read from a cam card.

pub const BORE: f32 = 86.0;
pub const STROKE: f32 = 86.0;
pub const ROD_LENGTH: f32 = 139.0;
pub const BORE_SPACING: f32 = 96.0;
pub const DECK_HEIGHT: f32 = 212.0;
pub const VTC_MAX_DEG: f32 = 50.0;
pub const FIRING_ORDER: [usize; 4] = [1, 3, 4, 2];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Intake,
    Exhaust,
}

#[derive(Clone, Copy)]
pub struct CamEvent {
    pub centre: f32,
    pub duration: f32,
    pub lift: f32,
}

pub fn cam_event(side: Side, high: bool) -> CamEvent {
    match (side, high) {
        (Side::Exhaust, false) => CamEvent { centre: 250.0, duration: 220.0, lift: 9.5 },
        (Side::Intake, false) => CamEvent { centre: 470.0, duration: 220.0, lift: 10.0 },
        (Side::Exhaust, true) => CamEvent { centre: 255.0, duration: 255.0, lift: 11.5 },
        (Side::Intake, true) => CamEvent { centre: 465.0, duration: 260.0, lift: 12.0 },
    }
}

pub fn modulo(a: f32, n: f32) -> f32 {
    ((a % n) + n) % n
}

pub fn cyl_x(cyl: usize) -> f32 {
    (cyl as f32 - 2.5) * BORE_SPACING
}

/// Where cylinder `cyl` (1–4) is in its own cycle at `crank_deg`. 0 = TDC firing.
pub fn cycle_angle(cyl: usize, crank_deg: f32) -> f32 {
    let slot = FIRING_ORDER.iter().position(|&c| c == cyl).unwrap() as f32;
    modulo(crank_deg - slot * 180.0, 720.0)
}

/// Crank throw angle: 1 and 4 share a throw, 2 and 3 are 180° away.
pub fn throw_angle(cyl: usize) -> f32 {
    if cyl == 1 || cyl == 4 { 0.0 } else { 180.0 }
}

/// Crank centre to wrist pin, for a crank rotation `theta` from TDC.
pub fn piston_pin_height(theta_deg: f32) -> f32 {
    let r = STROKE / 2.0;
    let t = theta_deg.to_radians();
    r * t.cos() + (ROD_LENGTH * ROD_LENGTH - (r * t.sin()).powi(2)).sqrt()
}

/// The rod's lean from vertical in radians (crank pin sideways offset over rod length).
pub fn rod_angle(theta_deg: f32) -> f32 {
    let r = STROKE / 2.0;
    (r * theta_deg.to_radians().sin() / ROD_LENGTH).asin()
}

pub fn event_lift(cycle_deg: f32, ev: CamEvent) -> f32 {
    let d = modulo(cycle_deg - ev.centre + 360.0, 720.0) - 360.0;
    if d.abs() >= ev.duration / 2.0 {
        return 0.0;
    }
    ev.lift * (0.5 + 0.5 * (std::f32::consts::PI * d / (ev.duration / 2.0)).cos())
}

/// Valve lift in mm. `vtc_deg` advances the intake event (K20Z3 VTC).
pub fn valve_lift(cyl: usize, crank_deg: f32, side: Side, vtec: bool, vtc_deg: f32) -> f32 {
    let shift = if side == Side::Intake { vtc_deg } else { 0.0 };
    event_lift(cycle_angle(cyl, crank_deg + shift), cam_event(side, vtec))
}

pub fn stroke_name(cycle_deg: f32) -> &'static str {
    ["power", "exhaust", "intake", "compression"][(modulo(cycle_deg, 720.0) / 180.0) as usize % 4]
}

pub fn advance_crank(crank_deg: f32, rpm: f32, dt: f32, slow_mo: f32) -> f32 {
    modulo(crank_deg + rpm / 60.0 * 360.0 * dt * slow_mo, 720.0)
}

/// Stress 0–1 (above 1 = past the limit) to a heat colour and how strongly to blend it over the base colour.
pub fn stress_tint(s: f32) -> ([f32; 3], f32) {
    if !(s > 0.0) {
        return ([1.0, 1.0, 1.0], 0.0);
    }
    let x = s.min(1.3);
    let stops: [(f32, [f32; 3]); 4] = [
        (0.5, [1.0, 0.85, 0.2]),
        (0.8, [1.0, 0.45, 0.08]),
        (1.0, [0.9, 0.08, 0.04]),
        (1.3, [0.75, 0.02, 0.02]),
    ];
    if x <= 0.5 {
        return (stops[0].1, x / 0.5 * 0.35);
    }
    for i in 1..stops.len() {
        let (x0, c0) = stops[i - 1];
        let (x1, c1) = stops[i];
        if x <= x1 {
            let f = (x - x0) / (x1 - x0);
            let c = [c0[0] + (c1[0] - c0[0]) * f, c0[1] + (c1[1] - c0[1]) * f, c0[2] + (c1[2] - c0[2]) * f];
            let amount = 0.35 + (x.min(1.0) - 0.5) / 0.5 * 0.55 + if x > 1.0 { 0.1 } else { 0.0 };
            return (c, amount);
        }
    }
    (stops[3].1, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piston_travel_is_the_stroke() {
        assert!((piston_pin_height(0.0) - piston_pin_height(180.0) - STROKE).abs() < 1e-3);
    }

    #[test]
    fn firing_order_1342() {
        assert_eq!(cycle_angle(1, 0.0), 0.0);
        assert_eq!(cycle_angle(3, 180.0), 0.0);
        assert_eq!(cycle_angle(4, 360.0), 0.0);
        assert_eq!(cycle_angle(2, 540.0), 0.0);
    }

    #[test]
    fn firing_cylinder_is_at_the_top() {
        for (cyl, crank) in [(1, 0.0), (3, 180.0), (4, 360.0), (2, 540.0)] {
            let h = piston_pin_height(crank + throw_angle(cyl));
            assert!((h - (STROKE / 2.0 + ROD_LENGTH)).abs() < 1e-3, "cyl {cyl}");
        }
    }

    #[test]
    fn valves_shut_on_power_and_open_on_exhaust_and_intake() {
        assert_eq!(valve_lift(1, 30.0, Side::Intake, false, 0.0), 0.0);
        assert_eq!(valve_lift(1, 30.0, Side::Exhaust, false, 0.0), 0.0);
        assert!(valve_lift(1, 250.0, Side::Exhaust, false, 0.0) > 9.0);
        assert!(valve_lift(1, 470.0, Side::Intake, false, 0.0) > 9.0);
        assert!(valve_lift(1, 465.0, Side::Intake, true, 0.0) > valve_lift(1, 465.0, Side::Intake, false, 0.0));
    }

    #[test]
    fn vtc_moves_only_the_intake() {
        let a = valve_lift(1, 430.0, Side::Intake, false, 40.0);
        let b = valve_lift(1, 470.0, Side::Intake, false, 0.0);
        assert!((a - b).abs() < 1e-4);
        assert_eq!(valve_lift(1, 250.0, Side::Exhaust, false, 40.0), valve_lift(1, 250.0, Side::Exhaust, false, 0.0));
    }

    #[test]
    fn stress_tint_ranges() {
        assert_eq!(stress_tint(0.0).1, 0.0);
        assert!(stress_tint(1.1).1 >= 0.99);
        assert!(stress_tint(0.6).1 < stress_tint(0.9).1);
    }
}
