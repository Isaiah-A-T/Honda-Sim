//! Mesh builders: lathe (anything round), boxes, tubes along curves, extruded profiles. Millimetres.

use three_d::*;

fn mesh(positions: Vec<Vec3>, indices: Vec<u32>) -> CpuMesh {
    let mut m = CpuMesh { positions: Positions::F32(positions), indices: Indices::U32(indices), ..Default::default() };
    m.compute_normals();
    m
}

/// Revolves each profile polyline of (radius, y) points around the Y axis. Separate polylines get separate
/// vertices, so the edges between them stay sharp.
pub fn lathe(profiles: &[&[(f32, f32)]], segments: u32) -> CpuMesh {
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    for profile in profiles {
        let base = pos.len() as u32;
        let n = profile.len() as u32;
        for s in 0..=segments {
            let a = s as f32 / segments as f32 * std::f32::consts::TAU;
            for &(r, y) in profile.iter() {
                pos.push(vec3(r * a.cos(), y, -r * a.sin()));
            }
        }
        for s in 0..segments {
            for i in 0..n - 1 {
                let a = base + s * n + i;
                let b = base + (s + 1) * n + i;
                idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }
    mesh(pos, idx)
}

/// Closed cylinder (or cone) along Y, centred on the origin.
pub fn cylinder_y(r_bottom: f32, r_top: f32, len: f32, seg: u32) -> CpuMesh {
    let h = len / 2.0;
    lathe(&[&[(0.0, -h), (r_bottom, -h)], &[(r_bottom, -h), (r_top, h)], &[(r_top, h), (0.0, h)]], seg)
}

/// Open tube along Y (cylinder liners).
pub fn open_cylinder_y(r: f32, len: f32, seg: u32) -> CpuMesh {
    lathe(&[&[(r, -len / 2.0), (r, len / 2.0)]], seg)
}

pub fn torus_y(major: f32, minor: f32, seg: u32, ring: u32) -> CpuMesh {
    let profile: Vec<(f32, f32)> = (0..=ring)
        .map(|i| {
            let a = i as f32 / ring as f32 * std::f32::consts::TAU;
            (major + minor * a.cos(), minor * a.sin())
        })
        .collect();
    lathe(&[&profile], seg)
}

pub fn boxed(w: f32, h: f32, d: f32) -> CpuMesh {
    let mut m = CpuMesh::cube();
    m.transform(Mat4::from_nonuniform_scale(w / 2.0, h / 2.0, d / 2.0)).unwrap();
    m
}

/// Rotates a mesh built along Y so it runs along X.
pub fn along_x(mut m: CpuMesh) -> CpuMesh {
    m.transform(Mat4::from_angle_z(degrees(-90.0))).unwrap();
    m
}

pub fn transformed(mut m: CpuMesh, t: Mat4) -> CpuMesh {
    m.transform(t).unwrap();
    m
}

/// Centripetal-free uniform Catmull-Rom through the points, sampled evenly by parameter.
pub struct Curve {
    pts: Vec<Vec3>,
    closed: bool,
    lengths: Vec<f32>,
    samples: Vec<Vec3>,
}

impl Curve {
    pub fn new(pts: &[[f32; 3]], closed: bool) -> Self {
        let pts: Vec<Vec3> = pts.iter().map(|p| vec3(p[0], p[1], p[2])).collect();
        let mut c = Curve { pts, closed, lengths: vec![], samples: vec![] };
        let n = 200;
        c.samples = (0..=n).map(|i| c.point_raw(i as f32 / n as f32)).collect();
        let mut acc = 0.0;
        c.lengths.push(0.0);
        for w in c.samples.windows(2) {
            acc += (w[1] - w[0]).magnitude();
            c.lengths.push(acc);
        }
        c
    }

    fn point_raw(&self, t: f32) -> Vec3 {
        let n = self.pts.len();
        let segs = if self.closed { n } else { n - 1 } as f32;
        let p = t.clamp(0.0, 1.0) * segs;
        let mut i = p.floor() as isize;
        if i as f32 >= segs { i = segs as isize - 1; }
        let f = p - i as f32;
        let get = |k: isize| -> Vec3 {
            if self.closed { self.pts[k.rem_euclid(n as isize) as usize] } else { self.pts[k.clamp(0, n as isize - 1) as usize] }
        };
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let f2 = f * f;
        let f3 = f2 * f;
        (p1 * 2.0 + (p2 - p0) * f + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * f2 + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * f3) * 0.5
    }

    pub fn length(&self) -> f32 {
        *self.lengths.last().unwrap()
    }

    /// Point at a fraction `u` of the curve's length.
    pub fn point_at(&self, u: f32) -> Vec3 {
        let target = u.clamp(0.0, 1.0) * self.length();
        let i = self.lengths.partition_point(|&l| l < target).clamp(1, self.lengths.len() - 1);
        let (l0, l1) = (self.lengths[i - 1], self.lengths[i]);
        let f = if l1 > l0 { (target - l0) / (l1 - l0) } else { 0.0 };
        self.samples[i - 1] + (self.samples[i] - self.samples[i - 1]) * f
    }
}

/// Round tube along a curve.
pub fn tube(curve: &Curve, radius: f32, steps: u32, ring: u32) -> CpuMesh {
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    let pts: Vec<Vec3> = (0..=steps).map(|i| curve.point_at(i as f32 / steps as f32)).collect();
    let mut normal = {
        let t = (pts[1] - pts[0]).normalize();
        let guess = if t.y.abs() < 0.9 { vec3(0.0, 1.0, 0.0) } else { vec3(1.0, 0.0, 0.0) };
        t.cross(guess).normalize()
    };
    for i in 0..=steps as usize {
        let t = if i + 1 < pts.len() { pts[i + 1] - pts[i] } else { pts[i] - pts[i - 1] }.normalize();
        normal = (normal - t * normal.dot(t)).normalize();          // parallel transport
        let bin = t.cross(normal);
        for j in 0..=ring {
            let a = j as f32 / ring as f32 * std::f32::consts::TAU;
            pos.push(pts[i] + (normal * a.cos() + bin * a.sin()) * radius);
        }
    }
    let r = ring + 1;
    for i in 0..steps {
        for j in 0..ring {
            let a = i * r + j;
            let b = (i + 1) * r + j;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    mesh(pos, idx)
}

/// Extrudes a star-shaped outline given as (z, y) points along X from -w/2 to w/2, with caps.
pub fn extrude_x(outline: &[(f32, f32)], width: f32) -> CpuMesh {
    let n = outline.len() as u32;
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    let h = width / 2.0;
    // sides
    for &(z, y) in outline {
        pos.push(vec3(-h, y, z));
        pos.push(vec3(h, y, z));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        let (a0, a1, b0, b1) = (i * 2, i * 2 + 1, j * 2, j * 2 + 1);
        idx.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
    }
    let mesh_sides = mesh(pos.clone(), idx.clone());
    // caps as separate meshes for sharp edges
    let (cz, cy) = outline.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let centre = (cz / n as f32, cy / n as f32);
    let cap = |x: f32, flip: bool| {
        let mut p = vec![vec3(x, centre.1, centre.0)];
        p.extend(outline.iter().map(|&(z, y)| vec3(x, y, z)));
        let mut ix = Vec::new();
        for i in 0..n {
            let (a, b) = (1 + i, 1 + (i + 1) % n);
            if flip { ix.extend_from_slice(&[0, b, a]); } else { ix.extend_from_slice(&[0, a, b]); }
        }
        mesh(p, ix)
    };
    merge(&[mesh_sides, cap(-h, false), cap(h, true)])
}

pub fn merge(meshes: &[CpuMesh]) -> CpuMesh {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut idx = Vec::new();
    for m in meshes {
        let base = pos.len() as u32;
        pos.extend(m.positions.to_f32());
        nor.extend(m.normals.clone().unwrap_or_default());
        idx.extend(m.indices.to_u32().unwrap_or_default().into_iter().map(|i| i + base));
    }
    CpuMesh { positions: Positions::F32(pos), normals: Some(nor), indices: Indices::U32(idx), ..Default::default() }
}
