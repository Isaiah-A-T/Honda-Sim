//! The K20Z3 model: builds every part from the dimensions in kinematics.rs and moves them each frame.
//! Built in millimetres; SCALE turns that into scene units (1 unit = 100 mm).

use crate::kinematics::*;
use crate::materials;
use crate::meshes::*;
use crate::state::SimState;
use three_d::*;

pub const SCALE: f32 = 0.01;
const DECK: f32 = DECK_HEIGHT;
const ROOF: f32 = DECK + 10.0;
const CAM_Y: f32 = DECK + 142.0;
const CAM_Z: f32 = 38.0;
const VALVE_SEAT_Z: f32 = 20.0;
const VALVE_TIP_Y: f32 = DECK + 109.0;
/// Rocker followers ride between the cam's base circle and the valve tips; the VTEC pin runs through them.
const FOLLOWER_Y: f32 = DECK + 118.0;
const BASE_R: f32 = 17.0;
const LEN: f32 = 4.0 * BORE_SPACING + 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sys {
    Casting,
    Internals,
    Intake,
    Exhaust,
    Turbo,
    NaIntake,
    /// Turbo charge piping through an intercooler.
    Intercooled,
    /// Turbo charge pipe straight to the throttle body.
    DirectCharge,
}

/// Which builds a part or flow belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum When {
    Always,
    Na,
    Turbo,
    TurboIc,
    TurboDirect,
}

impl When {
    fn holds(self, turbo: bool, ic: bool) -> bool {
        match self {
            When::Always => true,
            When::Na => !turbo,
            When::Turbo => turbo,
            When::TurboIc => turbo && ic,
            When::TurboDirect => turbo && !ic,
        }
    }
    fn of(sys: Sys) -> When {
        match sys {
            Sys::Turbo => When::Turbo,
            Sys::NaIntake => When::Na,
            Sys::Intercooled => When::TurboIc,
            Sys::DirectCharge => When::TurboDirect,
            _ => When::Always,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Full,
    Internals,
    Block,
    Airflow,
    Stress,
    Materials,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Full, Mode::Internals, Mode::Block, Mode::Airflow, Mode::Stress, Mode::Materials];
    pub fn label(self) -> &'static str {
        match self {
            Mode::Full => "Full engine",
            Mode::Internals => "Internals",
            Mode::Block => "Block",
            Mode::Airflow => "Airflow",
            Mode::Stress => "Stress",
            Mode::Materials => "Materials",
        }
    }
    fn shows(self, sys: Sys) -> bool {
        match self {
            Mode::Internals => sys == Sys::Internals,
            Mode::Block => sys == Sys::Casting,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Castings {
    SeeThrough,
    Solid,
    Hidden,
}

#[derive(Clone, Copy)]
enum Mover {
    Fixed,
    Crank,
    Piston(usize),
    Rod(usize),
    Valve(usize),
    Cam(Side),
    Butterfly,
    TurbineWheel,
    CompWheel,
    /// A rocker follower: the two primary ones ride the low lobes (or the VTEC lobe once locked), the middle
    /// secondary one always rides the VTEC lobe.
    Follower { cyl: usize, side: Side, secondary: bool },
    /// The VTEC lock pin, sliding from the primary follower through the other two.
    Pin { cyl: usize, side: Side },
}

pub struct Part {
    pub gm: Gm<Mesh, PhysicalMaterial>,
    local: Mat4,
    mover: Mover,
    pub key: Option<&'static str>,
    pub index: Option<usize>,
    pub label: String,
    pub sys: Sys,
    base: Srgba,
    base_emissive: Srgba,
    base_alpha: u8,
    opaque: bool,
    /// Glows green while this cylinder's rockers on this side are locked to the VTEC lobe.
    vtec_part: Option<(usize, Side)>,
    pub visible: bool,
}

struct ValveInfo {
    cyl: usize,
    side: Side,
    seat: Vec3,
    axis: Vec3,
}

enum GlowKind {
    Flame(usize),
    Plug(usize),
}

struct Glow {
    gm: Gm<Mesh, ColorMaterial>,
    kind: GlowKind,
}

struct Flow {
    curve: Curve,
    intake: bool,
    cyl: Option<usize>,
    when: When,
    /// Charge air straight off the compressor, before any intercooler.
    hot: bool,
    u: Vec<f32>,
}

/// A pipe's centreline, kept to check it clears every other part.
struct PipeCheck {
    part: usize,
    curve: Curve,
    radius: f32,
}

pub struct Engine {
    pub parts: Vec<Part>,
    valves: Vec<ValveInfo>,
    glows: Vec<Glow>,
    flows: Vec<Flow>,
    particles: Gm<InstancedMesh, ColorMaterial>,
    show_particles: bool,
    transparent_states: RenderStates,
    pub crank: f32,
    turbo_angle: f32,
    pub shake: f32,
    turbo_offset: Vec3,
    tb_pos: Vec3,
    bounds: Vec<Option<(Vec3, Vec3)>>,
    pipes: Vec<PipeCheck>,
    ctx: Context,
    /// The cam card the lobes are drawn from, and each lobe part's (side, high).
    pub specs: CamSpecs,
    lobes: Vec<(usize, Side, bool)>,
    /// VTEC pin travel per cylinder, [intake, exhaust], 0 = low cam, 1 = locked.
    pub pins: [[f32; 2]; 4],
    /// Valve lift (mm) per cylinder this frame, [intake, exhaust].
    pub lifts: [[f32; 2]; 4],
}

fn srgba(hex: u32, a: u8) -> Srgba {
    Srgba::new((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a)
}

fn lerp_color(a: Srgba, b: [f32; 3], t: f32) -> Srgba {
    let f = |x: u8, y: f32| ((x as f32 / 255.0) * (1.0 - t) + y * t).clamp(0.0, 1.0) * 255.0;
    Srgba::new(f(a.r, b[0]) as u8, f(a.g, b[1]) as u8, f(a.b, b[2]) as u8, a.a)
}

struct Builder<'a> {
    ctx: &'a Context,
    parts: Vec<Part>,
    /// Each part's box at rest (mm), for the clearance check. None for parts that move along a path.
    bounds: Vec<Option<(Vec3, Vec3)>>,
    pipes: Vec<PipeCheck>,
}

struct Spec {
    color: u32,
    alpha: u8,
    metal: f32,
    rough: f32,
    emissive: u32,
}

const fn metal(color: u32, metal: f32, rough: f32) -> Spec {
    Spec { color, alpha: 255, metal, rough, emissive: 0 }
}
const fn glass(color: u32, alpha: u8) -> Spec {
    Spec { color, alpha, metal: 0.1, rough: 0.6, emissive: 0 }
}

impl<'a> Builder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, mesh: &CpuMesh, s: Spec, key: Option<&'static str>, label: &str, index: Option<usize>, sys: Sys, mover: Mover, local: Mat4) -> usize {
        let cpu = CpuMaterial {
            albedo: srgba(s.color, s.alpha),
            metallic: s.metal,
            roughness: s.rough,
            emissive: srgba(s.emissive, 255),
            ..Default::default()
        };
        let material = if s.alpha < 255 { PhysicalMaterial::new_transparent(self.ctx, &cpu) } else { PhysicalMaterial::new_opaque(self.ctx, &cpu) };
        let gm = Gm::new(Mesh::new(self.ctx, mesh), material);
        // Parts that only spin about their own axis keep the same box; sliding ones don't get one.
        let bound = matches!(mover, Mover::Fixed | Mover::Crank | Mover::Butterfly).then(|| {
            let local = if matches!(mover, Mover::Butterfly) { Mat4::identity() } else { local };
            let mut lo = vec3(f32::MAX, f32::MAX, f32::MAX);
            let mut hi = -lo;
            for p in mesh.positions.to_f32() {
                let w = (local * p.extend(1.0)).truncate();
                lo = vec3(lo.x.min(w.x), lo.y.min(w.y), lo.z.min(w.z));
                hi = vec3(hi.x.max(w.x), hi.y.max(w.y), hi.z.max(w.z));
            }
            (lo, hi)
        });
        self.bounds.push(bound);
        self.parts.push(Part {
            gm,
            local,
            mover,
            key,
            index,
            label: label.to_string(),
            sys,
            base: srgba(s.color, s.alpha),
            base_emissive: srgba(s.emissive, 255),
            base_alpha: s.alpha,
            opaque: s.alpha == 255,
            vtec_part: None,
            visible: true,
        });
        self.parts.len() - 1
    }

    /// A round pipe along `pts`, recorded for the clearance check.
    #[allow(clippy::too_many_arguments)]
    fn pipe(&mut self, pts: &[[f32; 3]], radius: f32, s: Spec, key: Option<&'static str>, label: &str, sys: Sys) -> usize {
        let curve = Curve::new(pts, false);
        let steps = ((curve.length() / 8.0) as u32).clamp(20, 160);
        let i = self.add(&tube(&curve, radius, steps, 16), s, key, label, None, sys, Mover::Fixed, Mat4::identity());
        self.pipes.push(PipeCheck { part: i, curve, radius });
        i
    }
}

fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(vec3(x, y, z))
}

/// Cam lobe outline (z, y) with the nose pointing +Y: the base circle plus the event's lift, so the drawn lobe
/// has the card's lift and duration (one cam degree is two crank degrees).
fn lobe_outline(ev: CamEvent) -> Vec<(f32, f32)> {
    (0..96)
        .map(|i| {
            let a = i as f32 / 96.0 * std::f32::consts::TAU;
            let d = modulo(a.to_degrees() * 2.0 + 360.0, 720.0) - 360.0;
            let r = BASE_R + event_lift(ev.centre + d, ev);
            (-r * a.sin(), r * a.cos())
        })
        .collect()
}

fn lobe_mesh(specs: &CamSpecs, side: Side, high: bool) -> CpuMesh {
    extrude_x(&lobe_outline(cam_event(specs.get(side, high), side)), if high { 10.0 } else { 12.0 })
}

pub struct FlowPath {
    pts: Vec<[f32; 3]>,
    intake: bool,
    cyl: Option<usize>,
    when: When,
    hot: bool,
}

impl Engine {
    pub fn new(ctx: &Context) -> Self {
        let mut b = Builder { ctx, parts: Vec::new(), bounds: Vec::new(), pipes: Vec::new() };
        let mut valves = Vec::new();
        let mut glows = Vec::new();
        let mut paths: Vec<FlowPath> = Vec::new();
        let mut lobes: Vec<(usize, Side, bool)> = Vec::new();
        let id = Mat4::identity();
        let r = STROKE / 2.0;

        // ---------- Castings ----------
        b.add(&boxed(LEN, DECK + 60.0, 190.0), glass(0x8fa3b8, 34), Some("block"), "Engine block", None, Sys::Casting, Mover::Fixed, t(0.0, (DECK - 60.0) / 2.0, 0.0));
        for c in 1..=4 {
            b.add(&open_cylinder_y(BORE / 2.0 + 2.0, 130.0, 40), glass(0xa9b8c8, 46), Some("block"), "Cylinder liner", None, Sys::Casting, Mover::Fixed, t(cyl_x(c), DECK - 65.0, 0.0));
        }
        b.add(&boxed(LEN - 30.0, 90.0, 170.0), glass(0x6f7782, 34), None, "Oil pan", None, Sys::Casting, Mover::Fixed, t(0.0, -105.0, 0.0));
        b.add(&boxed(LEN, 120.0, 160.0), glass(0x9fb0c2, 30), Some("head"), "Cylinder head", None, Sys::Casting, Mover::Fixed, t(0.0, DECK + 60.0, 0.0));
        b.add(&boxed(LEN - 20.0, 45.0, 150.0), glass(0xb3262b, 40), None, "Cam cover", None, Sys::Casting, Mover::Fixed, t(0.0, DECK + 156.0, 0.0));

        // ---------- Crankshaft ----------
        let steel = || metal(0x9aa0a6, 0.8, 0.3);
        for i in 0..5 {
            b.add(&along_x(cylinder_y(27.0, 27.0, 22.0, 32)), steel(), Some("crank"), "Crankshaft main journal", None, Sys::Internals, Mover::Crank, t((i as f32 - 2.0) * BORE_SPACING, 0.0, 0.0));
        }
        let mut cw_outline: Vec<(f32, f32)> = (0..=16)
            .map(|i| {
                let a = std::f32::consts::PI * (1.18 + 0.64 * i as f32 / 16.0);
                (-70.0 * a.cos(), 70.0 * a.sin())
            })
            .collect();
        cw_outline.push((0.0, 0.0));
        let counterweight = extrude_x(&cw_outline, 14.0);
        for c in 1..=4 {
            let up = if c == 1 || c == 4 { 1.0 } else { -1.0 };
            b.add(&along_x(cylinder_y(24.0, 24.0, 26.0, 28)), steel(), Some("crank"), &format!("Crank pin {c}"), None, Sys::Internals, Mover::Crank, t(cyl_x(c), up * r, 0.0));
            for side in [-1.0, 1.0] {
                let x = cyl_x(c) + side * 24.0;
                b.add(&boxed(14.0, r + 30.0, 48.0), steel(), Some("crank"), "Crank web", None, Sys::Internals, Mover::Crank, t(x, up * r / 2.0, 0.0));
                let flip = if up < 0.0 { Mat4::from_angle_x(degrees(180.0)) } else { id };
                b.add(&counterweight, steel(), Some("crank"), "Counterweight", None, Sys::Internals, Mover::Crank, t(x, 0.0, 0.0) * flip);
            }
        }
        b.add(&along_x(cylinder_y(18.0, 18.0, 70.0, 20)), steel(), Some("crank"), "Crank snout", None, Sys::Internals, Mover::Crank, t(cyl_x(1) - 85.0, 0.0, 0.0));
        b.add(&along_x(cylinder_y(70.0, 70.0, 22.0, 48)), metal(0x2c3138, 0.5, 0.5), None, "Crank pulley", None, Sys::Internals, Mover::Crank, t(cyl_x(1) - 120.0, 0.0, 0.0));
        b.add(&boxed(24.0, 40.0, 8.0), Spec { emissive: 0x666666, ..metal(0xf4f4f4, 0.2, 0.5) }, None, "Timing mark", None, Sys::Internals, Mover::Crank, t(cyl_x(1) - 120.0, 48.0, 0.0));
        b.add(&along_x(cylinder_y(22.0, 22.0, 14.0, 18)), steel(), None, "Crank timing sprocket", None, Sys::Internals, Mover::Crank, t(cyl_x(1) - 72.0, 0.0, 0.0));
        b.add(&along_x(cylinder_y(140.0, 140.0, 26.0, 64)), metal(0x6f757c, 0.85, 0.4), Some("flywheel"), "Flywheel", None, Sys::Internals, Mover::Crank, t(cyl_x(4) + 110.0, 0.0, 0.0));
        b.add(&boxed(28.0, 90.0, 14.0), Spec { emissive: 0x553300, ..metal(0xffc400, 0.3, 0.5) }, None, "Flywheel mark", None, Sys::Internals, Mover::Crank, t(cyl_x(4) + 96.0, 80.0, 0.0));
        b.add(&along_x(cylinder_y(118.0, 118.0, 30.0, 48)), metal(0x4a4f57, 0.55, 0.45), Some("clutch"), "Clutch pressure plate", None, Sys::Internals, Mover::Crank, t(cyl_x(4) + 138.0, 0.0, 0.0));

        // ---------- Pistons and rods ----------
        let piston_mesh = merge(&[
            transformed(cylinder_y(BORE / 2.0 - 0.5, BORE / 2.0 - 0.5, 48.0, 40), t(0.0, 6.0, 0.0)),
        ]);
        let rings = merge(&[24.0, 18.0, 12.0].map(|y| transformed(cylinder_y(BORE / 2.0, BORE / 2.0, 1.6, 40), t(0.0, y, 0.0))));
        for c in 1..=4 {
            let i = Some(c - 1);
            b.add(&piston_mesh, metal(0xd5d8dc, 0.4, 0.35), Some("pistons"), &format!("Piston {c}"), i, Sys::Internals, Mover::Piston(c), id);
            b.add(&rings, metal(0x30343a, 0.5, 0.5), None, &format!("Piston rings {c}"), i, Sys::Internals, Mover::Piston(c), id);
            b.add(&along_x(cylinder_y(10.0, 10.0, 60.0, 16)), metal(0x8c939b, 0.9, 0.3), Some("pistons"), &format!("Wrist pin {c}"), i, Sys::Internals, Mover::Piston(c), id);
            let rod = metal(0xb8bcc2, 0.75, 0.3);
            b.add(&boxed(20.0, ROD_LENGTH - 30.0, 16.0), rod, Some("rods"), &format!("Connecting rod {c}"), i, Sys::Internals, Mover::Rod(c), t(0.0, -ROD_LENGTH / 2.0, 0.0));
            b.add(&along_x(cylinder_y(15.0, 15.0, 22.0, 20)), metal(0xb8bcc2, 0.75, 0.3), Some("rods"), &format!("Rod small end {c}"), i, Sys::Internals, Mover::Rod(c), id);
            b.add(&along_x(cylinder_y(32.0, 32.0, 24.0, 28)), metal(0xb8bcc2, 0.75, 0.3), Some("rods"), &format!("Rod big end {c}"), i, Sys::Internals, Mover::Rod(c), t(0.0, -ROD_LENGTH, 0.0));

            for (kind, color) in [(GlowKind::Flame(c), Srgba::new(255, 140, 30, 0)), (GlowKind::Plug(c), Srgba::new(160, 212, 255, 0))] {
                let mesh = match kind {
                    GlowKind::Flame(_) => cylinder_y(BORE / 2.0 - 4.0, BORE / 2.0 - 4.0, 1.0, 32),
                    GlowKind::Plug(_) => {
                        let mut s = CpuMesh::sphere(12);
                        s.transform(Mat4::from_scale(7.0)).unwrap();
                        s
                    }
                };
                let mut mat = ColorMaterial::new_transparent(ctx, &CpuMaterial { albedo: color, ..Default::default() });
                mat.render_states.blend = Blend::ADD;
                mat.render_states.write_mask = WriteMask::COLOR;
                glows.push(Glow { gm: Gm::new(Mesh::new(ctx, &mesh), mat), kind });
            }
        }

        // ---------- Cams, valves, plugs ----------
        for side in [Side::Intake, Side::Exhaust] {
            let z = if side == Side::Intake { CAM_Z } else { -CAM_Z };
            let name = if side == Side::Intake { "Intake" } else { "Exhaust" };
            let cam = metal(0x7d8590, 0.8, 0.35);
            b.add(&along_x(cylinder_y(12.0, 12.0, LEN - 10.0, 20)), cam, Some("cams"), &format!("{name} camshaft"), None, Sys::Internals, Mover::Cam(side), id);
            b.add(&along_x(cylinder_y(34.0, 34.0, 14.0, 36)), metal(0x5e656e, 0.6, 0.4), None, &format!("{name} cam sprocket"), None, Sys::Internals, Mover::Cam(side), t(cyl_x(1) - 72.0, 0.0, 0.0));
            b.add(&boxed(16.0, 14.0, 6.0), Spec { emissive: 0x555555, ..metal(0xf4f4f4, 0.2, 0.5) }, None, "Cam timing mark", None, Sys::Internals, Mover::Cam(side), t(cyl_x(1) - 72.0, 28.0, 0.0));
            let specs = CamSpecs::default();
            let low_mesh = lobe_mesh(&specs, side, false);
            let high_mesh = lobe_mesh(&specs, side, true);
            for c in 1..=4 {
                // Each lobe is phased so its nose points down at the follower at the event centre.
                let slot = FIRING_ORDER.iter().position(|&x| x == c).unwrap() as f32;
                let phase = |high: bool| {
                    let centre = cam_event(specs.get(side, high), side).centre + slot * 180.0;
                    Mat4::from_angle_x(radians(std::f32::consts::PI - (centre / 2.0).to_radians()))
                };
                for dx in [-17.0, 17.0] {
                    let i = b.add(&low_mesh, metal(0x7d8590, 0.8, 0.35), Some("cams"), &format!("Cyl {c} {} lobe (low cam)", name.to_lowercase()), None, Sys::Internals, Mover::Cam(side), t(cyl_x(c) + dx, 0.0, 0.0) * phase(false));
                    lobes.push((i, side, false));
                }
                let hi = b.add(&high_mesh, metal(0x8a929c, 0.8, 0.35), Some("cams"), &format!("Cyl {c} {} VTEC lobe (high cam)", name.to_lowercase()), None, Sys::Internals, Mover::Cam(side), t(cyl_x(c), 0.0, 0.0) * phase(true));
                b.parts[hi].vtec_part = Some((c, side));
                lobes.push((hi, side, true));

                // Rocker followers and the VTEC lock pin.
                let fz = |side: Side| if side == Side::Intake { CAM_Z } else { -CAM_Z };
                for (dx, secondary) in [(-17.0, false), (0.0, true), (17.0, false)] {
                    let label = if secondary { format!("Cyl {c} {} VTEC rocker (secondary)", name.to_lowercase()) } else { format!("Cyl {c} {} rocker (primary)", name.to_lowercase()) };
                    let color = if secondary { 0x6a7480 } else { 0x58606a };
                    b.add(&boxed(12.0, 14.0, 26.0), metal(color, 0.75, 0.35), Some("valves"), &label, Some(c - 1), Sys::Internals, Mover::Follower { cyl: c, side, secondary }, t(cyl_x(c) + dx, FOLLOWER_Y, fz(side)));
                }
                let pin = b.add(&along_x(cylinder_y(3.5, 3.5, 48.0, 12)), metal(0xd4a017, 0.85, 0.3), None, &format!("Cyl {c} {} VTEC lock pin", name.to_lowercase()), Some(c - 1), Sys::Internals, Mover::Pin { cyl: c, side }, t(0.0, FOLLOWER_Y, fz(side)));
                b.parts[pin].vtec_part = Some((c, side));

                for dx in [-17.0, 17.0] {
                    let seat = vec3(cyl_x(c) + dx, ROOF, if side == Side::Intake { VALVE_SEAT_Z } else { -VALVE_SEAT_Z });
                    let tip = vec3(cyl_x(c) + dx, VALVE_TIP_Y, z);
                    let stem_len = (tip - seat).magnitude();
                    let axis = (tip - seat).normalize();
                    let head_r = if side == Side::Intake { 17.5 } else { 15.0 };
                    let color = if side == Side::Intake { 0xc8b88a } else { 0x9a8a7a };
                    let vi = valves.len();
                    valves.push(ValveInfo { cyl: c, side, seat, axis });
                    let label = format!("Cyl {c} {} valve", name.to_lowercase());
                    b.add(&cylinder_y(head_r, head_r * 0.55, 4.0, 28), metal(color, 0.7, 0.4), Some("valves"), &label, Some(c - 1), Sys::Internals, Mover::Valve(vi), id);
                    b.add(&cylinder_y(2.8, 2.8, stem_len, 10), metal(color, 0.7, 0.4), Some("valves"), &label, Some(c - 1), Sys::Internals, Mover::Valve(vi), t(0.0, stem_len / 2.0, 0.0));
                    b.add(&torus_y(9.0, 1.6, 18, 6), metal(0x50565e, 0.6, 0.45), None, "Valve spring", Some(c - 1), Sys::Internals, Mover::Valve(vi), t(0.0, stem_len - 30.0, 0.0));
                }
            }
        }
        let chain_x = cyl_x(1) - 72.0;
        let chain = Curve::new(
            &[[-25.0, 0.0], [-12.0, -22.0], [12.0, -22.0], [25.0, 0.0], [CAM_Z + 36.0, CAM_Y - 10.0], [CAM_Z + 22.0, CAM_Y + 28.0],
              [0.0, CAM_Y + 30.0], [-CAM_Z - 22.0, CAM_Y + 28.0], [-CAM_Z - 36.0, CAM_Y - 10.0]]
                .map(|[zz, y]| [chain_x, y, zz]),
            true,
        );
        b.add(&tube(&chain, 3.0, 160, 6), metal(0x3b4048, 0.9, 0.4), None, "Timing chain", None, Sys::Internals, Mover::Fixed, id);
        for c in 1..=4 {
            b.add(&cylinder_y(6.0, 6.0, 90.0, 12), metal(0xeeeeee, 0.1, 0.4), None, &format!("Spark plug {c}"), None, Sys::Internals, Mover::Fixed, t(cyl_x(c), ROOF + 56.0, 0.0));
            b.add(&boxed(26.0, 40.0, 26.0), metal(0x1e2226, 0.3, 0.6), None, &format!("Ignition coil {c}"), None, Sys::Internals, Mover::Fixed, t(cyl_x(c), ROOF + 126.0, 0.0));
        }

        // ---------- Intake ----------
        // The K20's throttle body sits on the transmission (+x) end of the manifold, the far end from the timing chain.
        let intake = || metal(0x3a3f46, 0.4, 0.6);
        for c in 1..=4 {
            let x = cyl_x(c);
            b.pipe(&[[x, DECK + 70.0, 80.0], [x, DECK + 110.0, 150.0], [x, DECK + 70.0, 220.0], [x, DECK - 10.0, 235.0]], 19.0, intake(), Some("intake"), &format!("Intake runner {c}"), Sys::Intake);
            b.add(&transformed(cylinder_y(6.0, 4.0, 55.0, 10), Mat4::from_angle_x(radians(-0.6))), metal(0x24282e, 0.3, 0.6), None, &format!("Fuel injector {c}"), None, Sys::Intake, Mover::Fixed, t(x + 20.0, DECK + 100.0, 105.0));
            paths.push(FlowPath { pts: vec![[x, DECK - 30.0, 250.0], [x, DECK - 10.0, 235.0], [x, DECK + 70.0, 220.0], [x, DECK + 110.0, 150.0], [x, DECK + 70.0, 80.0], [x, ROOF + 22.0, VALVE_SEAT_Z + 6.0], [x, ROOF - 10.0, 0.0]], intake: true, cyl: Some(c), when: When::Always, hot: false });
        }
        let tb_pos = vec3(LEN / 2.0 + 40.0, DECK - 30.0, 250.0);
        let tb_in = [LEN / 2.0 + 70.0, DECK - 30.0, 250.0];
        paths.push(FlowPath { pts: vec![tb_in, [LEN / 2.0 - 30.0, DECK - 30.0, 250.0], [-LEN / 2.0 + 30.0, DECK - 30.0, 250.0]], intake: true, cyl: None, when: When::Always, hot: false });
        b.add(&along_x(cylinder_y(55.0, 55.0, LEN - 20.0, 32)), intake(), Some("intake"), "Intake plenum", None, Sys::Intake, Mover::Fixed, t(0.0, DECK - 30.0, 250.0));
        b.add(&along_x(open_cylinder_y(36.0, 60.0, 32)), metal(0x9aa3ad, 0.6, 0.4), None, "Throttle body", None, Sys::Intake, Mover::Fixed, t(tb_pos.x, tb_pos.y, tb_pos.z));
        b.add(&along_x(cylinder_y(33.0, 33.0, 2.0, 32)), metal(0xc7ccd2, 0.6, 0.35), None, "Throttle plate", None, Sys::Intake, Mover::Butterfly, id);

        // N/A: intake pipe from the throttle body forward to a cone filter.
        let na = [tb_in, [LEN / 2.0 + 150.0, DECK - 30.0, 262.0], [LEN / 2.0 + 210.0, DECK - 60.0, 340.0], [LEN / 2.0 + 230.0, DECK - 90.0, 420.0]];
        b.pipe(&na, 36.0, metal(0x2b2f35, 0.3, 0.8), Some("intake"), "Intake pipe", Sys::NaIntake);
        let filter_at = vec3(LEN / 2.0 + 232.0, DECK - 92.0, 490.0);
        b.add(&transformed(cylinder_y(48.0, 62.0, 140.0, 28), Mat4::from_angle_x(degrees(90.0))), metal(0xb3262b, 0.1, 0.9), None, "Air filter", None, Sys::NaIntake, Mover::Fixed, t(filter_at.x, filter_at.y, filter_at.z));
        let mut na_flow = vec![[filter_at.x, filter_at.y, filter_at.z + 60.0]];
        na_flow.extend(na.iter().rev());
        paths.push(FlowPath { pts: na_flow, intake: true, cyl: None, when: When::Na, hot: false });

        // ---------- Exhaust ----------
        let exh = || metal(0x4f4945, 0.45, 0.8);
        for c in 1..=4 {
            let x = cyl_x(c);
            let runner = Curve::new(&[[x, DECK + 70.0, -80.0], [x, DECK + 60.0, -150.0], [x * 0.6, DECK - 10.0, -210.0], [x * 0.15, DECK - 80.0, -235.0]], false);
            b.add(&tube(&runner, 18.0, 48, 16), exh(), Some("exhaust"), &format!("Exhaust runner {c}"), None, Sys::Exhaust, Mover::Fixed, id);
            paths.push(FlowPath { pts: vec![[x, ROOF - 10.0, 0.0], [x, ROOF + 22.0, -VALVE_SEAT_Z - 6.0], [x, DECK + 70.0, -80.0], [x, DECK + 60.0, -150.0], [x * 0.6, DECK - 10.0, -210.0], [x * 0.15, DECK - 80.0, -235.0], [0.0, DECK - 140.0, -240.0]], intake: false, cyl: Some(c), when: When::Always, hot: false });
        }
        b.add(&cylinder_y(32.0, 40.0, 70.0, 24), exh(), Some("exhaust"), "Exhaust collector", None, Sys::Exhaust, Mover::Fixed, t(0.0, DECK - 110.0, -240.0));
        paths.push(FlowPath { pts: vec![[0.0, DECK - 140.0, -240.0], [0.0, DECK - 260.0, -240.0]], intake: false, cyl: None, when: When::Na, hot: false });

        // ---------- Turbo ----------
        let to = vec3(0.0, DECK - 180.0, -260.0);
        let tt = |x: f32, y: f32, z: f32| t(to.x + x, to.y + y, to.z + z);
        let shift = |pts: &[[f32; 3]]| -> Vec<[f32; 3]> { pts.iter().map(|p| [p[0] + to.x, p[1] + to.y, p[2] + to.z]).collect() };
        b.add(&along_x(torus_y(55.0, 30.0, 40, 16)), metal(0x8a8f96, 0.6, 0.5), Some("turbo"), "Turbine housing", None, Sys::Turbo, Mover::Fixed, tt(-50.0, 0.0, 0.0));
        b.add(&along_x(torus_y(65.0, 32.0, 40, 16)), metal(0xb3b8be, 0.5, 0.45), Some("turbo"), "Compressor housing", None, Sys::Turbo, Mover::Fixed, tt(70.0, 0.0, 0.0));
        b.add(&along_x(cylinder_y(38.0, 38.0, 90.0, 24)), metal(0x52575e, 0.55, 0.45), Some("turbo"), "Centre section (bearings)", None, Sys::Turbo, Mover::Fixed, tt(0.0, 0.0, 0.0));
        let wheel = |radius: f32, blades: u32| {
            let mut parts = vec![along_x(cylinder_y(12.0, 12.0, 30.0, 12))];
            for i in 0..blades {
                let rot = Mat4::from_angle_x(radians(i as f32 / blades as f32 * std::f32::consts::TAU));
                parts.push(transformed(boxed(26.0, radius, 2.5), rot * t(0.0, radius / 2.0, 0.0) * Mat4::from_angle_y(radians(0.5))));
            }
            merge(&parts)
        };
        b.add(&wheel(36.0, 11), metal(0x8d7f70, 0.85, 0.35), None, "Turbine wheel", None, Sys::Turbo, Mover::TurbineWheel, id);
        b.add(&wheel(44.0, 12), metal(0xd8dde2, 0.85, 0.35), None, "Compressor wheel", None, Sys::Turbo, Mover::CompWheel, id);
        let downpipe = Curve::new(&shift(&[[-50.0, 0.0, -60.0], [-60.0, -60.0, -80.0], [-60.0, -160.0, -60.0]]), false);
        b.add(&tube(&downpipe, 30.0, 30, 16), metal(0x8a8f96, 0.6, 0.5), Some("exhaust"), "Downpipe", None, Sys::Turbo, Mover::Fixed, id);
        let up = Curve::new(&[[0.0, DECK - 140.0, -240.0], [-30.0, DECK - 160.0, -250.0], [-50.0, DECK - 180.0, -262.0]], false);
        b.add(&tube(&up, 28.0, 20, 16), exh(), Some("exhaust"), "Turbo up-pipe", None, Sys::Turbo, Mover::Fixed, id);
        let mut ex_turbo = vec![[0.0, DECK - 140.0, -240.0], [-30.0, DECK - 160.0, -250.0], [-50.0, DECK - 180.0, -262.0]];
        ex_turbo.extend(shift(&[[-50.0, 0.0, -60.0], [-60.0, -60.0, -80.0], [-60.0, -160.0, -60.0]]));
        paths.push(FlowPath { pts: ex_turbo, intake: false, cyl: None, when: When::Turbo, hot: false });

        // Charge piping. The turbo sits on the exhaust side (-z) and the throttle body on the intake side (+z) at the
        // transmission end, so every route goes around the engine rather than across it.
        let charge = || metal(0x2f6fb5, 0.5, 0.45);
        let comp_out = [to.x + 110.0, to.y, to.z];

        // No intercooler: around the transmission end, clear of the flywheel and clutch, into the throttle body.
        let direct = [comp_out, [220.0, 40.0, -310.0], [370.0, 60.0, -230.0], [395.0, 110.0, 40.0], [360.0, DECK - 30.0, 240.0], [330.0, DECK - 30.0, 252.0], tb_in];
        b.pipe(&direct, 26.0, charge(), Some("intake"), "Charge pipe", Sys::DirectCharge);
        let mut f = shift(&[[70.0, 0.0, 0.0]]);
        f.extend(direct);
        paths.push(FlowPath { pts: f, intake: true, cyl: None, when: When::TurboDirect, hot: true });

        // Intercooler: front-mounted below the intake side. Hot pipe runs under the oil pan to its left tank,
        // the cold pipe leaves the right tank and climbs to the throttle body.
        let ic = vec3(0.0, -90.0, 480.0);
        b.add(&boxed(520.0, 180.0, 60.0), metal(0x9aa0a8, 0.7, 0.45), Some("intake"), "Intercooler core", None, Sys::Intercooled, Mover::Fixed, t(ic.x, ic.y, ic.z));
        for (side, name) in [(-1.0, "Intercooler inlet tank"), (1.0, "Intercooler outlet tank")] {
            b.add(&boxed(50.0, 200.0, 80.0), metal(0x6d737b, 0.7, 0.4), Some("intake"), name, None, Sys::Intercooled, Mover::Fixed, t(side * 285.0, ic.y, ic.z));
        }
        let hot = [comp_out, [200.0, -20.0, -330.0], [170.0, -200.0, -320.0], [0.0, -250.0, -150.0], [-200.0, -240.0, 150.0], [-300.0, -160.0, 380.0], [-290.0, -90.0, 455.0]];
        b.pipe(&hot, 26.0, charge(), Some("intake"), "Charge pipe (hot side)", Sys::Intercooled);
        let cold = [[290.0, -90.0, 455.0], [310.0, -60.0, 380.0], [350.0, 40.0, 330.0], [350.0, 150.0, 285.0], [330.0, DECK - 30.0, 255.0], tb_in];
        b.pipe(&cold, 26.0, charge(), Some("intake"), "Charge pipe (cold side)", Sys::Intercooled);
        let mut f = shift(&[[70.0, 0.0, 0.0]]);
        f.extend(hot);
        paths.push(FlowPath { pts: f, intake: true, cyl: None, when: When::TurboIc, hot: true });
        paths.push(FlowPath { pts: vec![[-285.0, ic.y, ic.z], [-150.0, ic.y + 40.0, ic.z], [0.0, ic.y - 40.0, ic.z], [150.0, ic.y + 40.0, ic.z], [285.0, ic.y, ic.z]], intake: true, cyl: None, when: When::TurboIc, hot: false });
        paths.push(FlowPath { pts: cold.to_vec(), intake: true, cyl: None, when: When::TurboIc, hot: false });

        let flows = paths
            .into_iter()
            .map(|p| {
                let curve = Curve::new(&p.pts, false);
                let n = ((curve.length() / 16.0).round() as usize).max(6);
                let u = (0..n).map(|i| (i as f32 + 0.3 * ((i * 7919) % 10) as f32 / 10.0) / n as f32).collect();
                Flow { curve, intake: p.intake, cyl: p.cyl, when: p.when, hot: p.hot, u }
            })
            .collect();
        let mut dot = CpuMesh::sphere(6);
        dot.transform(Mat4::from_scale(4.5)).unwrap();
        let particles = Gm::new(InstancedMesh::new(ctx, &Instances::default(), &dot), ColorMaterial::new_opaque(ctx, &CpuMaterial { albedo: Srgba::WHITE, ..Default::default() }));
        let transparent_states = PhysicalMaterial::new_transparent(ctx, &CpuMaterial { albedo: Srgba::new(255, 255, 255, 100), ..Default::default() }).render_states;

        Engine {
            parts: b.parts,
            valves,
            glows,
            flows,
            particles,
            show_particles: false,
            transparent_states,
            crank: 0.0,
            turbo_angle: 0.0,
            shake: 0.0,
            turbo_offset: to,
            tb_pos,
            bounds: b.bounds,
            pipes: b.pipes,
            ctx: ctx.clone(),
            specs: CamSpecs::default(),
            lobes,
            pins: [[0.0; 2]; 4],
            lifts: [[0.0; 2]; 4],
        }
    }

    /// Advance the animation and apply the mode's visibility and colours.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, s: &SimState, dt: f32, slow_mo: f32, mode: Mode, castings: Castings, materials: &dyn Fn(&str) -> Option<(String, [u8; 3])>, selected: Option<&str>, time: f32) {
        self.crank = advance_crank(self.crank, s.rpm, dt, slow_mo);
        let crank = self.crank;
        let vtc = s.vtc_deg.clamp(0.0, VTC_MAX_DEG);
        let burning = s.running && !s.fuel_cut && s.rpm > 300.0;
        if s.turbo {
            let spin = (s.boost_kpa.max(0.0) / 100.0 * 9.0 + s.rpm / 8000.0 * 2.0) * if s.running { 1.0 } else { 0.2 };
            self.turbo_angle = (self.turbo_angle + spin * dt * std::f32::consts::TAU) % std::f32::consts::TAU;
        }
        self.shake = if s.knock && burning { (self.shake + dt * 8.0).min(1.0) } else { (self.shake - dt * 4.0).max(0.0) };
        let jitter = if self.shake > 0.0 {
            let n = (time * 97.0).sin() * 43758.547;
            let m = (time * 61.0).cos() * 12543.13;
            vec3((n.fract() - 0.5) * 2.0 * self.shake, (m.fract() - 0.5) * 2.0 * self.shake, 0.0)
        } else {
            vec3(0.0, 0.0, 0.0)
        };
        let world = Mat4::from_scale(SCALE) * Mat4::from_translation(jitter);

        // A new cam card from the sim: redraw the lobes to match.
        if let Some(cams) = s.cams {
            if cams != self.specs {
                self.specs = cams;
                for &(i, side, high) in &self.lobes {
                    self.parts[i].gm.geometry = Mesh::new(&self.ctx, &lobe_mesh(&cams, side, high));
                }
            }
        }
        let specs = self.specs;

        // VTEC: oil pressure pushes each pin through its three followers, but a pin can only slide while the
        // followers line up, which is when both lobes are on the base circle. So each cylinder locks (and unlocks)
        // at its own moment in the cycle.
        let sim_dt = dt * slow_mo;
        for c in 1..=4 {
            for (k, side) in [Side::Intake, Side::Exhaust].into_iter().enumerate() {
                let v = if side == Side::Intake { vtc } else { 0.0 };
                let pin = &mut self.pins[c - 1][k];
                if let Some(p) = s.vtec_pin_of(c) {
                    *pin = p.clamp(0.0, 1.0);
                } else {
                    let aligned = lobe_lift(c, crank, side, true, v, &specs) < 0.05 && lobe_lift(c, crank, side, false, v, &specs) < 0.05;
                    if aligned {
                        let target = if s.vtec { 1.0 } else { 0.0 };
                        let step = sim_dt / 0.008;         // ~8 ms of pin travel once the followers line up
                        *pin = if target > *pin { (*pin + step).min(target) } else { (*pin - step).max(target) };
                    }
                }
                self.lifts[c - 1][k] = lobe_lift(c, crank, side, *pin >= 0.999, v, &specs);
            }
        }
        let lifts = self.lifts;
        let pins = self.pins;

        for p in self.parts.iter_mut() {
            let m = match p.mover {
                Mover::Fixed => Mat4::identity(),
                Mover::Crank => Mat4::from_angle_x(degrees(crank)),
                Mover::Piston(c) => {
                    let pin = piston_pin_height(crank + throw_angle(c));
                    t(cyl_x(c), pin, 0.0)
                }
                Mover::Rod(c) => {
                    let theta = crank + throw_angle(c);
                    t(cyl_x(c), piston_pin_height(theta), 0.0) * Mat4::from_angle_x(radians(-rod_angle(theta)))
                }
                Mover::Valve(i) => {
                    let v = &self.valves[i];
                    let lift = lifts[v.cyl - 1][if v.side == Side::Intake { 0 } else { 1 }];
                    let pos = v.seat - v.axis * lift;
                    let rot = Quat::from_arc(vec3(0.0, 1.0, 0.0), v.axis, None);
                    Mat4::from_translation(pos) * Mat4::from(rot)
                }
                Mover::Cam(side) => {
                    let a = if side == Side::Intake { (crank + vtc) / 2.0 } else { crank / 2.0 };
                    t(0.0, CAM_Y, if side == Side::Intake { CAM_Z } else { -CAM_Z }) * Mat4::from_angle_x(degrees(a))
                }
                Mover::Butterfly => Mat4::from_translation(self.tb_pos) * Mat4::from_angle_y(radians(0.12 + s.throttle.clamp(0.0, 1.0) * 1.38)),
                Mover::Follower { cyl, side, secondary } => {
                    let k = if side == Side::Intake { 0 } else { 1 };
                    let lift = if secondary { lobe_lift(cyl, crank, side, true, if k == 0 { vtc } else { 0.0 }, &specs) } else { lifts[cyl - 1][k] };
                    t(0.0, -lift, 0.0)
                }
                Mover::Pin { cyl, side } => {
                    let k = if side == Side::Intake { 0 } else { 1 };
                    t(cyl_x(cyl) - 30.0 + 30.0 * pins[cyl - 1][k], -lifts[cyl - 1][k], 0.0)
                }
                Mover::TurbineWheel => Mat4::from_translation(self.turbo_offset + vec3(-50.0, 0.0, 0.0)) * Mat4::from_angle_x(radians(self.turbo_angle)),
                Mover::CompWheel => Mat4::from_translation(self.turbo_offset + vec3(70.0, 0.0, 0.0)) * Mat4::from_angle_x(radians(self.turbo_angle)),
            };
            p.gm.geometry.set_transformation(world * m * p.local);

            // Visibility for this mode.
            let mut vis = mode.shows(p.sys);
            vis &= When::of(p.sys).holds(s.turbo, s.intercooler);
            let cast_mode = match mode {
                Mode::Block => Castings::Solid,
                Mode::Airflow => Castings::SeeThrough,
                _ => castings,
            };
            if p.sys == Sys::Casting { vis &= cast_mode != Castings::Hidden; }
            p.visible = vis;

            // Transparency: castings follow the setting; airflow mode ghosts the plumbing.
            let ghost = mode == Mode::Airflow && matches!(p.sys, Sys::Intake | Sys::Exhaust | Sys::Turbo | Sys::NaIntake | Sys::Intercooled | Sys::DirectCharge);
            let (alpha, transparent) = if p.sys == Sys::Casting {
                match (mode, cast_mode) {
                    (Mode::Airflow, _) => ((p.base_alpha as f32 * 0.45) as u8, true),
                    (_, Castings::Solid) => (235, true),
                    _ => (p.base_alpha, true),
                }
            } else if ghost {
                (56, true)
            } else {
                (p.base_alpha, !p.opaque)
            };
            let mat = &mut p.gm.material;
            if transparent != mat.is_transparent {
                mat.is_transparent = transparent;
                mat.render_states = if transparent { self.transparent_states } else { RenderStates::default() };
            }

            // Colour for this mode.
            let mut albedo = p.base;
            let mut emissive = p.base_emissive;
            match mode {
                Mode::Stress => {
                    if let Some(k) = p.key {
                        let (rgb, amount) = stress_tint(s.stress_of(k, p.index));
                        albedo = lerp_color(p.base, rgb, amount);
                        let e = amount * amount * if k == "exhaust" { 0.8 } else { 0.3 };
                        emissive = lerp_color(p.base_emissive, rgb, e);
                    }
                }
                Mode::Materials => {
                    if let Some(k) = p.key {
                        match materials(k) {
                            Some((_, c)) => albedo = Srgba::new(c[0], c[1], c[2], p.base.a),
                            None => albedo = Srgba::new((p.base.r as f32 * 0.55) as u8, (p.base.g as f32 * 0.55) as u8, (p.base.b as f32 * 0.55) as u8, p.base.a),
                        }
                        let pulse = if selected == Some(k) { (0.18 + 0.12 * (time * 6.0).sin()) * 255.0 } else { 0.0 };
                        emissive = Srgba::new(pulse as u8, pulse as u8, pulse as u8, 255);
                    }
                }
                _ => {}
            }
            let locked = p.vtec_part.is_some_and(|(c, side)| pins[c - 1][if side == Side::Intake { 0 } else { 1 }] >= 0.999);
            if locked && mode != Mode::Materials {
                emissive = Srgba::new(28, 140, 76, 255);
            }
            albedo.a = alpha;
            mat.albedo = albedo;
            mat.emissive = emissive;
        }

        // Plug flashes and combustion.
        let load = (s.map_kpa / 100.0).clamp(0.15, 2.2);
        let lean = ((s.lambda - 0.85) / 0.35).clamp(0.0, 1.0);
        let internals_shown = mode.shows(Sys::Internals);
        for g in self.glows.iter_mut() {
            let c = match g.kind { GlowKind::Flame(c) | GlowKind::Plug(c) => c };
            let cyc = cycle_angle(c, crank);
            let since = modulo(cyc + s.ign_deg, 720.0);
            let pin = piston_pin_height(crank + throw_angle(c));
            match g.kind {
                GlowKind::Plug(_) => {
                    let a = if burning && since < 30.0 { 1.0 - since / 30.0 } else { 0.0 };
                    let a = a * if internals_shown { 1.0 } else { 0.0 };     // additive: scale the colour itself
                    g.gm.material.color = Srgba::new((160.0 * a) as u8, (212.0 * a) as u8, (255.0 * a) as u8, 255);
                    g.gm.geometry.set_transformation(world * t(cyl_x(c), ROOF - 4.0, 0.0));
                }
                GlowKind::Flame(_) => {
                    let burn = if burning && since < 110.0 { (std::f32::consts::PI * since / 110.0).sin() } else { 0.0 };
                    let a = burn * (0.35 + 0.35 * load.min(1.5)) * if internals_shown { 1.0 } else { 0.0 };
                    let color = if s.knock { [255, 40, 25] } else { [255, (115.0 + 100.0 * lean) as u8, (30.0 + 190.0 * lean) as u8] };
                    g.gm.material.color = Srgba::new((color[0] as f32 * a) as u8, (color[1] as f32 * a) as u8, (color[2] as f32 * a) as u8, 255);
                    let crown = pin + 30.0;
                    let h = (ROOF - crown).max(1.0);
                    g.gm.geometry.set_transformation(world * t(cyl_x(c), (crown + ROOF) / 2.0, 0.0) * Mat4::from_nonuniform_scale(1.0, h, 1.0));
                }
            }
        }

        // Airflow particles.
        self.show_particles = mode == Mode::Airflow;
        if self.show_particles {
            let base = if s.running && s.rpm > 300.0 { 220.0 + 900.0 * (s.air_gs / 150.0).clamp(0.0, 2.2) } else { 0.0 };
            let boost = (s.boost_kpa / 150.0).clamp(0.0, 1.0);
            let heat = s.stress_of("exhaust", None).clamp(0.0, 1.2);
            let mut tf = Vec::new();
            let mut colors = Vec::new();
            for f in self.flows.iter_mut() {
                if !f.when.holds(s.turbo, s.intercooler) { continue; }
                let gate = match f.cyl {
                    Some(c) => {
                        let lift = lifts[c - 1][if f.intake { 0 } else { 1 }];
                        let max = specs.get(if f.intake { Side::Intake } else { Side::Exhaust }, true).lift_mm;
                        (lift / max * 1.6).min(1.0)
                    }
                    None => 1.0,
                };
                let cut = if !f.intake && s.fuel_cut { 0.5 } else { 1.0 };
                let du = base * gate * cut * dt / f.curve.length();
                let col = if f.intake && f.hot {
                    [255.0, (0.75 - 0.25 * boost) * 255.0, (0.55 - 0.35 * boost) * 255.0]   // compressor heat
                } else if f.intake {
                    let warm = ((s.iat_c - 30.0) / 60.0).clamp(0.0, 1.0);
                    [(0.43 + 0.5 * boost + 0.5 * warm).min(1.0) * 255.0, (0.78 + 0.2 * boost - 0.2 * warm) * 255.0, (1.0 - 0.5 * warm) * 255.0]
                } else {
                    [255.0, (0.42 + 0.45 * heat) * 255.0, (0.1 + 0.35 * (heat - 0.6).max(0.0)) * 255.0]
                };
                let dim = if f.cyl.is_some() { 0.45 + 0.55 * gate } else { 1.0 };
                for u in f.u.iter_mut() {
                    *u = (*u + du) % 1.0;
                    let p = f.curve.point_at(*u);
                    tf.push(world * Mat4::from_translation(p));
                    colors.push(Srgba::new((col[0] * dim) as u8, (col[1] * dim) as u8, (col[2] * dim) as u8, 255));
                }
            }
            self.particles.geometry.set_instances(&Instances { transformations: tf, colors: Some(colors), ..Default::default() });
        }
    }

    /// Every pipe that passes through another part, as "pipe / part" lines, for each build (N/A, turbo with and
    /// without an intercooler). A pipe may sit inside the parts its ends connect to.
    pub fn clearance_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (turbo, ic, build) in [(false, true, "N/A"), (true, true, "turbo + intercooler"), (true, false, "turbo, no intercooler")] {
            let shown = |i: usize| When::of(self.parts[i].sys).holds(turbo, ic);
            for pc in self.pipes.iter().filter(|pc| shown(pc.part)) {
                let len = pc.curve.length();
                let end_skip = (pc.radius * 2.5).min(len * 0.25);
                for (j, part) in self.parts.iter().enumerate() {
                    if j == pc.part || !shown(j) || part.label.starts_with("Fuel injector") { continue; }
                    let Some((lo, hi)) = self.bounds[j] else { continue };
                    let dist = |p: Vec3| vec3((lo.x - p.x).max(0.0).max(p.x - hi.x), (lo.y - p.y).max(0.0).max(p.y - hi.y), (lo.z - p.z).max(0.0).max(p.z - hi.z)).magnitude();
                    // What a pipe's ends sit in is what it connects to (runner into plenum, charge pipe into throttle body).
                    if dist(pc.curve.point_at(0.0)) < pc.radius || dist(pc.curve.point_at(1.0)) < pc.radius { continue; }
                    let hit = (0..=240).map(|k| k as f32 / 240.0).filter(|u| u * len > end_skip && (1.0 - u) * len > end_skip).find(|&u| {
                        dist(pc.curve.point_at(u)) < pc.radius - 1.0
                    });
                    if let Some(u) = hit {
                        out.push(format!("{build}: {} passes through {} at {:.0}% along", self.parts[pc.part].label, part.label, u * 100.0));
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    pub fn objects(&self) -> Vec<&dyn Object> {
        let mut v: Vec<&dyn Object> = self.parts.iter().filter(|p| p.visible).map(|p| &p.gm as &dyn Object).collect();
        if self.show_particles {
            v.push(&self.particles);
        }
        v.extend(self.glows.iter().map(|g| &g.gm as &dyn Object));
        v
    }

    /// Indices of the parts that can be hovered, in the order they're passed to `pick`.
    pub fn pickable(&self) -> Vec<usize> {
        self.parts.iter().enumerate().filter(|(_, p)| p.visible && p.gm.material.albedo.a > 100).map(|(i, _)| i).collect()
    }
}

/// Material colour for a part key: the sim's own materials when it sent them, else the placeholder catalog.
pub fn material_lookup<'a>(s: &'a SimState, choices: &'a std::collections::HashMap<String, String>) -> impl Fn(&str) -> Option<(String, [u8; 3])> + 'a {
    move |key: &str| {
        if let Some(pm) = &s.part_materials {
            let m = pm.get(key)?;
            let fam = materials::family(&m.family)?;
            return Some((m.label.clone(), fam.color));
        }
        let cat = materials::catalog(key)?;
        let id = choices.get(key).map(|s| s.as_str()).unwrap_or(cat.options[0].id);
        let opt = cat.options.iter().find(|o| o.id == id).unwrap_or(&cat.options[0]);
        Some((opt.label.to_string(), materials::family(opt.family)?.color))
    }
}
