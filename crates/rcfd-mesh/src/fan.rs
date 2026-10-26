//! Parametric axial-fan blade geometry and a closed blade surface triangulation (wave 32A, ADR 0065).
//!
//! Sections lie on cylinders and are described in the unrolled plane `(z, s)` with `s = r theta`. The chord frame of a
//! station with stagger `g` is `e_c = (cos g, -sin g)` (leading to trailing edge) and `e_v = (-sin g, -cos g)` (`v > 0` is
//! the suction side, at smaller `theta`). A positive stagger therefore turns the trailing edge toward `-theta`: the blade
//! pushes fluid along `+z` when the rotor turns about `+z`.
//!
//! Section families (published forms, ADR 0065): NACA 4-digit (thickness with the closed-trailing-edge coefficient
//! `-0.1036`, two-piece parabolic camber, normal offset by `theta = atan(yc')`) and a double circular arc (circular camber
//! arc, circular thickness arc, same normal offset).
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use rcfd_core::vec3::{self, Vec3};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::fmt;

/// A located validation error (`path` names the offending parameter, e.g. `stations[2].chord`).
#[derive(Debug, Clone, PartialEq)]
pub struct FanError {
    /// Parameter path.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl FanError {
    /// New error.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for FanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Section family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionFamily {
    /// NACA 4-digit (camber `m`, position `p`, thickness `t`).
    Naca4,
    /// Double circular arc (camber `m`, thickness `t`; `camber_pos` is ignored).
    CircularArc,
}

/// One radial station of the blade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Station {
    /// Radius [m].
    pub radius: f64,
    /// Chord [m].
    pub chord: f64,
    /// Stagger angle from the axis [rad] (positive: trailing edge toward `-theta`).
    pub stagger: f64,
    /// Maximum camber as a fraction of the chord.
    pub camber: f64,
    /// Chordwise position of maximum camber (NACA 4-digit only).
    pub camber_pos: f64,
    /// Maximum thickness as a fraction of the chord.
    pub thickness: f64,
    /// Axial shift [m] of the leading edge (sweep).
    pub sweep: f64,
    /// Tangential shift [m] of the leading edge along the cylinder (lean).
    pub lean: f64,
}

/// Parametric axial fan rotor (one blade is described; the blade count sets the sector angle).
#[derive(Debug, Clone, PartialEq)]
pub struct AxialFan {
    /// Number of blades `N` (sector angle `2 pi / N`).
    pub n_blades: usize,
    /// Hub radius [m].
    pub r_hub: f64,
    /// Blade tip radius [m].
    pub r_tip: f64,
    /// Radial gap between blade tip and shroud [m] (shroud radius = `r_tip + tip_clearance`).
    pub tip_clearance: f64,
    /// Axial extent of the domain [m] (inlet plane `z = 0`, outlet plane `z = axial_extent`).
    pub axial_extent: f64,
    /// Axial position of the leading edge at the hub before sweep [m].
    pub z_le_root: f64,
    /// Section family.
    pub family: SectionFamily,
    /// Radial stations (first at `r_hub`, last at `r_tip`, strictly increasing).
    pub stations: Vec<Station>,
}

/// Which blade surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Suction side (`v > 0`, smaller `theta`).
    Suction,
    /// Pressure side (`v < 0`, larger `theta`).
    Pressure,
}

impl Side {
    /// `+1` for the suction side, `-1` for the pressure side.
    pub fn sign(self) -> f64 {
        match self {
            Side::Suction => 1.0,
            Side::Pressure => -1.0,
        }
    }
}

/// Camber `yc` and slope `dyc/dx` of a unit-chord section.
pub fn camber_line(family: SectionFamily, m: f64, p: f64, x: f64) -> (f64, f64) {
    match family {
        SectionFamily::Naca4 => {
            if m == 0.0 {
                (0.0, 0.0)
            } else if x < p {
                (m / (p * p) * (2.0 * p * x - x * x), 2.0 * m / (p * p) * (p - x))
            } else {
                (
                    m / ((1.0 - p) * (1.0 - p)) * ((1.0 - 2.0 * p) + 2.0 * p * x - x * x),
                    2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x),
                )
            }
        }
        SectionFamily::CircularArc => {
            if m.abs() < 1e-14 {
                (0.0, 0.0)
            } else {
                let r = (0.25 + m * m) / (2.0 * m);
                let q = (r * r - (x - 0.5) * (x - 0.5)).sqrt();
                (q - (r - m), -(x - 0.5) / q)
            }
        }
    }
}

/// Half thickness of a unit-chord section.
pub fn half_thickness(family: SectionFamily, t: f64, x: f64) -> f64 {
    match family {
        SectionFamily::Naca4 => {
            5.0 * t * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x * x * x - 0.1036 * x * x * x * x)
        }
        SectionFamily::CircularArc => {
            let h = 0.5 * t;
            let r = (0.25 + h * h) / (2.0 * h);
            ((r * r - (x - 0.5) * (x - 0.5)).sqrt() - (r - h)).max(0.0)
        }
    }
}

/// Unit-chord surface point `(u, v)` at chord parameter `x` (normal offset of the camber line by the half thickness).
pub fn section_uv(family: SectionFamily, m: f64, p: f64, t: f64, x: f64, side: Side) -> (f64, f64) {
    let (yc, dyc) = camber_line(family, m, p, x);
    let th = dyc.atan();
    let yt = half_thickness(family, t, x);
    let sg = side.sign();
    (x - sg * yt * th.sin(), yc + sg * yt * th.cos())
}

/// Gauss-Legendre nodes and weights on `[-1, 1]`.
pub fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0; n];
    let mut ws = vec![0.0; n];
    for i in 0..n {
        let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            let dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-16 {
                break;
            }
        }
        let (mut p0, mut p1) = (1.0, x);
        for k in 2..=n {
            let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
            p0 = p1;
            p1 = p2;
        }
        let dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
        xs[i] = x;
        ws[i] = 2.0 / ((1.0 - x * x) * dp * dp);
    }
    (xs, ws)
}

/// Area of the unit-chord section by Gauss-Legendre quadrature of `int 2 yt sqrt(1 + yc'^2) dx` (exact for the
/// normal-offset construction, `dA = (1 - kappa v) ds dv`), with `x = (1 - cos b) / 2`.
pub fn unit_section_area(family: SectionFamily, m: f64, p: f64, t: f64) -> f64 {
    let (gx, gw) = gauss_legendre(16);
    let panels = 16;
    let mut a = 0.0;
    for q in 0..panels {
        let (b0, b1) = (PI * q as f64 / panels as f64, PI * (q + 1) as f64 / panels as f64);
        for (xi, wi) in gx.iter().zip(&gw) {
            let b = 0.5 * (b0 + b1) + 0.5 * (b1 - b0) * xi;
            let x = 0.5 * (1.0 - b.cos());
            let dxdb = 0.5 * b.sin();
            let (_, dyc) = camber_line(family, m, p, x);
            a += wi * 0.5 * (b1 - b0) * 2.0 * half_thickness(family, t, x) * (1.0 + dyc * dyc).sqrt() * dxdb;
        }
    }
    a
}

fn lerp(a: f64, b: f64, w: f64) -> f64 {
    a + (b - a) * w
}

impl AxialFan {
    /// Shroud radius.
    pub fn r_shroud(&self) -> f64 {
        self.r_tip + self.tip_clearance
    }

    /// Sector angle `2 pi / N`.
    pub fn sector_angle(&self) -> f64 {
        TAU / self.n_blades as f64
    }

    /// Two-station convenience constructor (linear hub-to-tip variation) with a third mid station left implicit.
    #[allow(clippy::too_many_arguments)]
    pub fn linear(
        n_blades: usize,
        r_hub: f64,
        r_tip: f64,
        tip_clearance: f64,
        axial_extent: f64,
        z_le_root: f64,
        family: SectionFamily,
        hub: Station,
        tip: Station,
    ) -> Self {
        let mut h = hub;
        let mut t = tip;
        h.radius = r_hub;
        t.radius = r_tip;
        Self {
            n_blades,
            r_hub,
            r_tip,
            tip_clearance,
            axial_extent,
            z_le_root,
            family,
            stations: vec![h, t],
        }
    }

    /// Validate the parameters; every problem is reported with its path.
    pub fn validate(&self) -> Vec<FanError> {
        let mut e = Vec::new();
        let mut bad = |p: String, m: &str| e.push(FanError::new(p, m));
        if self.n_blades < 2 {
            bad(
                "n_blades".into(),
                "needs at least 2 blades (sector angle 2 pi / N < 2 pi)",
            );
        }
        if !(self.r_hub.is_finite() && self.r_hub > 0.0) {
            bad("r_hub".into(), "must be a positive finite radius");
        }
        if !(self.r_tip.is_finite() && self.r_tip > self.r_hub) {
            bad("r_tip".into(), "must be finite and larger than r_hub");
        }
        if !(self.tip_clearance.is_finite() && self.tip_clearance > 0.0) {
            bad(
                "tip_clearance".into(),
                "must be positive (the shroud lies at r_tip + tip_clearance)",
            );
        }
        if !(self.axial_extent.is_finite() && self.axial_extent > 0.0) {
            bad("axial_extent".into(), "must be positive");
        }
        if !self.z_le_root.is_finite() || self.z_le_root < 0.0 || self.z_le_root > self.axial_extent {
            bad("z_le_root".into(), "must lie inside [0, axial_extent]");
        }
        if self.stations.len() < 2 {
            bad("stations".into(), "needs at least two stations (hub and tip)");
        }
        for (i, s) in self.stations.iter().enumerate() {
            let p = |f: &str| format!("stations[{i}].{f}");
            if !s.radius.is_finite() {
                bad(p("radius"), "must be finite");
            }
            if i > 0 && s.radius <= self.stations[i - 1].radius {
                bad(p("radius"), "must be strictly increasing");
            }
            if !(s.chord.is_finite() && s.chord > 0.0) {
                bad(p("chord"), "must be positive");
            }
            if !(s.stagger.is_finite() && s.stagger.abs() < 80f64.to_radians()) {
                bad(
                    p("stagger"),
                    "must be finite with |stagger| < 80 deg (radians in the API)",
                );
            }
            if !(s.camber.is_finite() && (0.0..=0.3).contains(&s.camber)) {
                bad(p("camber"), "must lie in [0, 0.3] (fraction of chord)");
            }
            if self.family == SectionFamily::Naca4 && !(s.camber_pos > 0.05 && s.camber_pos < 0.95) {
                bad(p("camber_pos"), "must lie in (0.05, 0.95) for the NACA 4-digit family");
            }
            if !(s.thickness.is_finite() && s.thickness >= 0.005 && s.thickness <= 0.4) {
                bad(p("thickness"), "must lie in [0.005, 0.4] (fraction of chord)");
            }
            if !s.sweep.is_finite() {
                bad(p("sweep"), "must be finite");
            }
            if !s.lean.is_finite() {
                bad(p("lean"), "must be finite");
            }
        }
        if let (Some(f), Some(l)) = (self.stations.first(), self.stations.last()) {
            if (f.radius - self.r_hub).abs() > 1e-12 * self.r_hub {
                bad("stations[0].radius".into(), "the first station must be at r_hub");
            }
            if (l.radius - self.r_tip).abs() > 1e-12 * self.r_tip {
                bad(
                    format!("stations[{}].radius", self.stations.len() - 1),
                    "the last station must be at r_tip",
                );
            }
        }
        e
    }

    /// Station interpolated linearly in `r` (clamped to the blade span).
    pub fn station_at(&self, r: f64) -> Station {
        let st = &self.stations;
        if r <= st[0].radius {
            return Station { radius: r, ..st[0] };
        }
        let n = st.len();
        if r >= st[n - 1].radius {
            return Station { radius: r, ..st[n - 1] };
        }
        let mut i = 0;
        while r > st[i + 1].radius {
            i += 1;
        }
        let (a, b) = (&st[i], &st[i + 1]);
        let w = (r - a.radius) / (b.radius - a.radius);
        Station {
            radius: r,
            chord: lerp(a.chord, b.chord, w),
            stagger: lerp(a.stagger, b.stagger, w),
            camber: lerp(a.camber, b.camber, w),
            camber_pos: lerp(a.camber_pos, b.camber_pos, w),
            thickness: lerp(a.thickness, b.thickness, w),
            sweep: lerp(a.sweep, b.sweep, w),
            lean: lerp(a.lean, b.lean, w),
        }
    }

    /// Unrolled `(z, s)` position of the unit-chord point `(u, v)` of a station.
    pub fn place(&self, st: &Station, u: f64, v: f64) -> (f64, f64) {
        let (sg, cg) = st.stagger.sin_cos();
        let z0 = self.z_le_root + st.sweep;
        (
            z0 + st.chord * (u * cg - v * sg),
            st.lean + st.chord * (-u * sg - v * cg),
        )
    }

    /// Unrolled `(z, s)` of a surface point at chord parameter `x`.
    pub fn surface_zs(&self, st: &Station, x: f64, side: Side) -> (f64, f64) {
        let (u, v) = section_uv(self.family, st.camber, st.camber_pos, st.thickness, x, side);
        self.place(st, u, v)
    }

    /// Unrolled `(z, s)` of the camber line at chord parameter `x`.
    pub fn camber_zs(&self, st: &Station, x: f64) -> (f64, f64) {
        let (yc, _) = camber_line(self.family, st.camber, st.camber_pos, x);
        self.place(st, x, yc)
    }

    /// 3-D point of the exact blade surface at radius `r`, chord parameter `x`.
    pub fn surface_point(&self, r: f64, x: f64, side: Side) -> Vec3 {
        let st = self.station_at(r);
        let (z, s) = self.surface_zs(&st, x, side);
        let th = s / r;
        [r * th.cos(), r * th.sin(), z]
    }

    /// Exact section area [m^2] at radius `r` (area in the unrolled plane).
    pub fn section_area(&self, r: f64) -> f64 {
        let st = self.station_at(r);
        st.chord * st.chord * unit_section_area(self.family, st.camber, st.camber_pos, st.thickness)
    }

    /// Closed, oriented, watertight triangulation of the blade (`n_chord` points per side, `n_span` spanwise strips).
    pub fn blade_surface(&self, n_chord: usize, n_span: usize) -> Result<BladeSurface, Vec<FanError>> {
        let e = self.validate();
        if !e.is_empty() {
            return Err(e);
        }
        if n_chord < 4 || n_span < 1 {
            return Err(vec![FanError::new(
                "blade_surface",
                "needs n_chord >= 4 and n_span >= 1",
            )]);
        }
        let xs: Vec<f64> = (0..=n_chord)
            .map(|i| 0.5 * (1.0 - (PI * i as f64 / n_chord as f64).cos()))
            .collect();
        // loop: U_0 (= LE) ... U_n (= TE), L_{n-1} ... L_1 ; 2 n points per row
        let m = 2 * n_chord;
        let row = |j: usize| -> Vec<Vec3> {
            let r = lerp(self.r_hub, self.r_tip, j as f64 / n_span as f64);
            let mut v = Vec::with_capacity(m);
            for &x in &xs {
                v.push(self.surface_point(r, x, Side::Suction));
            }
            for &x in xs[1..n_chord].iter().rev() {
                v.push(self.surface_point(r, x, Side::Pressure));
            }
            v
        };
        let mut points = Vec::with_capacity(m * (n_span + 1));
        for j in 0..=n_span {
            points.extend(row(j));
        }
        let id = |j: usize, a: usize| j * m + a % m;
        let mut tri: Vec<[usize; 3]> = Vec::new();
        for j in 0..n_span {
            for a in 0..m {
                tri.push([id(j, a), id(j, a + 1), id(j + 1, a + 1)]);
                tri.push([id(j, a), id(j + 1, a + 1), id(j + 1, a)]);
            }
        }
        // caps: grids between suction index i and pressure index i (subdivided across the thickness so that the flat
        // triangles follow the cylinder); U_i = i, L_i = m - i (L_0 = U_0, L_n = U_n)
        let l_idx = |i: usize| {
            if i == 0 {
                0
            } else if i == n_chord {
                n_chord
            } else {
                m - i
            }
        };
        let n_cap = (n_chord / 8).max(2);
        for (jrow, tip) in [(0usize, false), (n_span, true)] {
            let r = lerp(self.r_hub, self.r_tip, jrow as f64 / n_span as f64);
            let stn = self.station_at(r);
            let mut vid = vec![vec![0usize; n_chord + 1]; n_cap + 1];
            for (q, row_ids) in vid.iter_mut().enumerate() {
                for (i, slot) in row_ids.iter_mut().enumerate() {
                    *slot = if q == 0 || i == 0 {
                        id(jrow, i)
                    } else if q == n_cap {
                        id(jrow, l_idx(i))
                    } else if i == n_chord {
                        id(jrow, n_chord)
                    } else {
                        let (zu, su) = self.surface_zs(&stn, xs[i], Side::Suction);
                        let (zl, sl) = self.surface_zs(&stn, xs[i], Side::Pressure);
                        let w = q as f64 / n_cap as f64;
                        let (z, sv) = (lerp(zu, zl, w), lerp(su, sl, w));
                        points.push([r * (sv / r).cos(), r * (sv / r).sin(), z]);
                        points.len() - 1
                    };
                }
            }
            for i in 0..n_chord {
                for q in 0..n_cap {
                    let (a, b, c, d) = (vid[q][i], vid[q][i + 1], vid[q + 1][i + 1], vid[q + 1][i]);
                    for t in [[b, a, d], [b, d, c]] {
                        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                            continue;
                        }
                        tri.push(if tip { [t[0], t[2], t[1]] } else { t });
                    }
                }
            }
        }
        let mut s = BladeSurface { points, triangles: tri };
        if s.volume() < 0.0 {
            for t in s.triangles.iter_mut() {
                t.swap(1, 2);
            }
        }
        Ok(s)
    }
}

/// Closed triangulated blade surface (outward oriented).
#[derive(Debug, Clone)]
pub struct BladeSurface {
    /// Vertices.
    pub points: Vec<Vec3>,
    /// Triangles (counter-clockwise seen from outside).
    pub triangles: Vec<[usize; 3]>,
}

impl BladeSurface {
    /// Total surface area.
    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (self.points[t[0]], self.points[t[1]], self.points[t[2]]);
                0.5 * vec3::norm(vec3::cross(vec3::sub(b, a), vec3::sub(c, a)))
            })
            .sum()
    }

    /// Enclosed volume (divergence theorem; positive when outward oriented).
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (self.points[t[0]], self.points[t[1]], self.points[t[2]]);
                vec3::dot(a, vec3::cross(b, c)) / 6.0
            })
            .sum()
    }

    /// Centroid of the enclosed solid (first moment of the tetrahedra on the origin over the volume; ADR 0118).
    pub fn centroid(&self) -> Vec3 {
        let mut m = [0.0; 3];
        for t in &self.triangles {
            let (a, b, c) = (self.points[t[0]], self.points[t[1]], self.points[t[2]]);
            let v = vec3::dot(a, vec3::cross(b, c)) / 24.0;
            for k in 0..3 {
                m[k] += v * (a[k] + b[k] + c[k]);
            }
        }
        vec3::scale(m, 1.0 / self.volume())
    }

    /// Watertightness: every directed edge once and its reverse once, Euler characteristic 2.
    pub fn check_watertight(&self) -> Result<(), String> {
        let mut dir: HashMap<(usize, usize), usize> = HashMap::new();
        for t in &self.triangles {
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                return Err("degenerate triangle".into());
            }
            for e in 0..3 {
                *dir.entry((t[e], t[(e + 1) % 3])).or_insert(0) += 1;
            }
        }
        for (&(a, b), &n) in &dir {
            if n != 1 {
                return Err(format!("directed edge ({a},{b}) used {n} times"));
            }
            if dir.get(&(b, a)) != Some(&1) {
                return Err(format!("edge ({a},{b}) has no unique reverse"));
            }
        }
        let used: std::collections::HashSet<usize> = self.triangles.iter().flatten().copied().collect();
        let (v, e, f) = (used.len() as i64, dir.len() as i64 / 2, self.triangles.len() as i64);
        if v - e + f != 2 {
            return Err(format!("Euler characteristic {} (V {v}, E {e}, F {f})", v - e + f));
        }
        Ok(())
    }
}
