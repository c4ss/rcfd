//! Equivalent radiated power and the Rayleigh integral of a vibrating surface. Declared by ADR 0440 (F-408, the scaffold of
//! ADR 0311); filled by ADR 0468 (the registration of F-418; design stub ADR 0321).
//!
//! Every amplitude is a complex peak amplitude, `x(t) = Re(X e^{i omega t})` as `rcfd_fem::harmonic::HarmonicResponse`, and
//! every power is time-averaged (W): with peak amplitudes the mean of `p v` carries the factor 1/2.
//!
//! - [`equivalent_radiated_power`]: `ERP = (rho c/2) int_S |V . n|^2 dS`, any surface, radiation efficiency taken as one.
//! - [`BaffledPlane`]: a plane radiator in an infinite rigid baffle by the Rayleigh integral `P(x) = (i omega rho/(2 pi))
//!   int_S V_n e^{-i k R}/R dS`: pressure at field points, the far field `P ~ F e^{-i k r}/r`, the directivity, and the radiated
//!   power `W = (rho c k/(4 pi)) int int Re(V_n(x) conj V_n(y)) sin(k R)/R dS dS` (the smooth real part of the kernel).
//! - [`power_level_db`] re [`W_REF`] and [`PowerSpectrum`] with third-octave bands numbered as `crate::acoustics`.
//!
//! Derivations and the checks: ADR 0468.

use crate::acoustics::{band_index, third_octave_bands, Spectrum};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::harmonic::HarmonicResponse;
use rcfd_fem::model::SolidModel;
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Reference sound power for power levels, W.
pub const W_REF: f64 = 1e-12;

/// The quadrature order of the equivalent radiated power (exact for the bilinear interpolant on a parallelogram).
pub const ERP_ORDER: usize = 3;

/// The note carried by every equivalent-radiated-power report.
pub const ERP_NOTE: &str =
    "equivalent radiated power: assumes a radiation efficiency of one (rho c |v_n|^2/2 per unit area, \
peak amplitudes); it over-predicts below coincidence and for surfaces small against the wavelength";

type Res<T> = Result<T, String>;

fn err<T>(m: String) -> Res<T> {
    Err(format!("solid acoustics: {m}"))
}

fn positive(what: &str, x: f64) -> Res<()> {
    if x.is_finite() && x > 0.0 {
        Ok(())
    } else {
        err(format!("{what} must be positive and finite, got {x}"))
    }
}

/// A complex peak amplitude `re + i im`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Phasor {
    /// Real part (in phase with `cos omega t`).
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Phasor {
    /// The phasor `re + i im`.
    pub const fn new(re: f64, im: f64) -> Phasor {
        Phasor { re, im }
    }

    /// The modulus.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// The squared modulus.
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

fn cmul(a: Phasor, b: Phasor) -> Phasor {
    Phasor::new(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

/// A surface of Tri3 and Quad4 faces with a complex velocity vector at each node.
#[derive(Debug, Clone, PartialEq)]
pub struct VibratingSurface {
    nodes: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    velocity: Vec<[Phasor; 3]>,
}

/// The four corners of a face for the bilinear map (a triangle repeats its last node).
fn corners(face: &[usize]) -> [usize; 4] {
    if face.len() == 3 {
        [face[0], face[1], face[2], face[2]]
    } else {
        [face[0], face[1], face[2], face[3]]
    }
}

fn shape(u: f64, v: f64) -> ([f64; 4], [f64; 4], [f64; 4]) {
    (
        [
            (1.0 - u) * (1.0 - v) / 4.0,
            (1.0 + u) * (1.0 - v) / 4.0,
            (1.0 + u) * (1.0 + v) / 4.0,
            (1.0 - u) * (1.0 + v) / 4.0,
        ],
        [-(1.0 - v) / 4.0, (1.0 - v) / 4.0, (1.0 + v) / 4.0, -(1.0 + v) / 4.0],
        [-(1.0 - u) / 4.0, -(1.0 + u) / 4.0, (1.0 + u) / 4.0, (1.0 - u) / 4.0],
    )
}

/// Gauss-Legendre points and weights on [-1, 1] (Newton on the three-term recurrence).
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        out.push((x, 2.0 / ((1.0 - x * x) * dp * dp)));
    }
    out
}

/// A quadrature point: position, weight (area element), complex velocity vector, unit normal of the face there.
#[derive(Debug, Clone, Copy)]
struct Point {
    x: Vec3,
    w: f64,
    v: [Phasor; 3],
    n: Vec3,
}

impl VibratingSurface {
    /// A surface from its nodes, faces of 3 or 4 node indices (a Quad4 in cyclic order) and the complex peak velocity vector of
    /// each node. Refuses faces of other sizes, indices out of range, a velocity list of another length, values that are not
    /// finite and faces of zero area.
    pub fn new(nodes: Vec<Vec3>, faces: Vec<Vec<usize>>, velocity: Vec<[Phasor; 3]>) -> Res<VibratingSurface> {
        if faces.is_empty() {
            return err("the surface has no face".into());
        }
        if velocity.len() != nodes.len() {
            return err(format!("{} velocities for {} nodes", velocity.len(), nodes.len()));
        }
        if let Some(i) = nodes.iter().position(|x| x.iter().any(|c| !c.is_finite())) {
            return err(format!("node {i}: coordinate not finite"));
        }
        if let Some(i) = velocity.iter().position(|v| v.iter().any(|c| !c.is_finite())) {
            return err(format!("node {i}: velocity not finite"));
        }
        for (f, face) in faces.iter().enumerate() {
            if face.len() != 3 && face.len() != 4 {
                return err(format!("face {f}: {} nodes (3 or 4 accepted)", face.len()));
            }
            if let Some(&i) = face.iter().find(|&&i| i >= nodes.len()) {
                return err(format!("face {f}: node {i} out of range ({} nodes)", nodes.len()));
            }
        }
        let s = VibratingSurface { nodes, faces, velocity };
        for f in 0..s.faces.len() {
            let a: f64 = s.face_points(f, 2, (-1.0, 1.0, -1.0, 1.0)).iter().map(|p| p.w).sum();
            if a.is_nan() || a <= 0.0 {
                return err(format!("face {f}: zero area"));
            }
        }
        Ok(s)
    }

    /// The faces of the surface set `set` of `model` with the velocity `i omega U` of the nodal displacement `U = re + i im` (one
    /// value per equation of `model.dof_map()`, components 0 to 2; a node without an equation is at rest). Faces of 3 and 4 nodes
    /// only.
    pub fn from_displacement(
        model: &SolidModel,
        set: &str,
        omega: f64,
        re: &[f64],
        im: &[f64],
    ) -> Res<VibratingSurface> {
        positive("omega", omega)?;
        let dofs = model.dof_map();
        if re.len() != dofs.n_equations() || im.len() != dofs.n_equations() {
            return err(format!(
                "displacement of {} and {} values for {} equations",
                re.len(),
                im.len(),
                dofs.n_equations()
            ));
        }
        let mesh = model.mesh();
        let Some(refs) = mesh.surface_set(set) else {
            return err(format!("unknown surface set \"{set}\""));
        };
        let mut local: BTreeMap<usize, usize> = BTreeMap::new();
        let mut nodes = Vec::new();
        let mut velocity = Vec::new();
        let mut faces = Vec::with_capacity(refs.len());
        for (k, &f) in refs.iter().enumerate() {
            let g = mesh.face_nodes(f);
            if g.len() != 3 && g.len() != 4 {
                return err(format!(
                    "surface set \"{set}\" face {k} (element {}, face {}): {} nodes (faces of 3 or 4 nodes accepted)",
                    f.element,
                    f.face,
                    g.len()
                ));
            }
            let mut face = Vec::with_capacity(g.len());
            for n in g {
                let next = nodes.len();
                let id = *local.entry(n).or_insert(next);
                if id == next {
                    nodes.push(mesh.nodes()[n]);
                    let mut v = [Phasor::default(); 3];
                    for (c, vc) in v.iter_mut().enumerate() {
                        if let Some(e) = dofs.equation(n, c) {
                            *vc = Phasor::new(-omega * im[e], omega * re[e]);
                        }
                    }
                    velocity.push(v);
                }
                face.push(id);
            }
            faces.push(face);
        }
        VibratingSurface::new(nodes, faces, velocity)
    }

    /// [`VibratingSurface::from_displacement`] of a harmonic response (ADR 0391 or ADR 0315).
    pub fn from_response(model: &SolidModel, set: &str, response: &HarmonicResponse) -> Res<VibratingSurface> {
        VibratingSurface::from_displacement(model, set, response.omega, &response.re, &response.im)
    }

    /// The nodes.
    pub fn nodes(&self) -> &[Vec3] {
        &self.nodes
    }

    /// The faces (node indices).
    pub fn faces(&self) -> &[Vec<usize>] {
        &self.faces
    }

    /// The complex velocity vector of each node.
    pub fn velocity(&self) -> &[[Phasor; 3]] {
        &self.velocity
    }

    /// The area (Gauss order 3 on each face).
    pub fn area(&self) -> f64 {
        (0..self.faces.len())
            .map(|f| {
                self.face_points(f, ERP_ORDER, (-1.0, 1.0, -1.0, 1.0))
                    .iter()
                    .map(|p| p.w)
                    .sum::<f64>()
            })
            .sum()
    }

    /// Gauss points of order `n` on the parameter box `(u0, u1, v0, v1)` of face `f`.
    fn face_points(&self, f: usize, n: usize, b: (f64, f64, f64, f64)) -> Vec<Point> {
        let c = corners(&self.faces[f]);
        let rule = gauss_legendre(n);
        let (u0, u1, v0, v1) = b;
        let scale = (u1 - u0) * (v1 - v0) / 4.0;
        let mut out = Vec::with_capacity(n * n);
        for &(gu, wu) in &rule {
            for &(gv, wv) in &rule {
                let u = u0 + (gu + 1.0) * (u1 - u0) / 2.0;
                let v = v0 + (gv + 1.0) * (v1 - v0) / 2.0;
                let (nn, nu, nv) = shape(u, v);
                let mut x = [0.0; 3];
                let mut xu = [0.0; 3];
                let mut xv = [0.0; 3];
                let mut vel = [Phasor::default(); 3];
                for a in 0..4 {
                    let p = self.nodes[c[a]];
                    let va = self.velocity[c[a]];
                    for i in 0..3 {
                        x[i] += nn[a] * p[i];
                        xu[i] += nu[a] * p[i];
                        xv[i] += nv[a] * p[i];
                        vel[i].re += nn[a] * va[i].re;
                        vel[i].im += nn[a] * va[i].im;
                    }
                }
                let cr = vec3::cross(xu, xv);
                let j = vec3::norm(cr);
                let normal = if j > 0.0 { vec3::scale(cr, 1.0 / j) } else { [0.0; 3] };
                out.push(Point {
                    x,
                    w: wu * wv * j * scale,
                    v: vel,
                    n: normal,
                });
            }
        }
        out
    }
}

fn vdot(v: &[Phasor; 3], n: Vec3) -> Phasor {
    Phasor::new(
        v[0].re * n[0] + v[1].re * n[1] + v[2].re * n[2],
        v[0].im * n[0] + v[1].im * n[1] + v[2].im * n[2],
    )
}

/// Power level in dB re [`W_REF`]; `None` for zero power.
pub fn power_level_db(w: f64) -> Res<Option<f64>> {
    if !w.is_finite() || w < 0.0 {
        return err(format!("power must be finite and not negative, got {w}"));
    }
    if w == 0.0 {
        return Ok(None);
    }
    Ok(Some(10.0 * (w / W_REF).log10()))
}

/// The equivalent radiated power of a surface.
#[derive(Debug, Clone, PartialEq)]
pub struct ErpReport {
    /// `(rho c/2) int |V . n|^2 dS`, W (time-averaged, from peak amplitudes).
    pub power: f64,
    /// Its level re 1e-12 W; `None` for zero power.
    pub level_db: Option<f64>,
    /// The area of the surface, m^2.
    pub area: f64,
    /// What the figure assumes ([`ERP_NOTE`]).
    pub note: String,
}

/// The equivalent radiated power `(rho c/2) int_S |V . n|^2 dS` of any surface (curved, warped, any orientation), with the
/// local unit normal at each quadrature point. Assumes a radiation efficiency of one.
pub fn equivalent_radiated_power(surface: &VibratingSurface, rho: f64, c: f64) -> Res<ErpReport> {
    positive("rho", rho)?;
    positive("c", c)?;
    let mut s = 0.0;
    let mut area = 0.0;
    for f in 0..surface.faces.len() {
        for p in surface.face_points(f, ERP_ORDER, (-1.0, 1.0, -1.0, 1.0)) {
            s += p.w * vdot(&p.v, p.n).norm_sqr();
            area += p.w;
        }
    }
    let power = rho * c / 2.0 * s;
    Ok(ErpReport {
        power,
        level_db: power_level_db(power)?,
        area,
        note: ERP_NOTE.to_string(),
    })
}

// ------------------------------------------------------------------------------------------------------------ spectra

/// Power at discrete frequencies (lines), W.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerSpectrum {
    /// Frequencies, Hz, positive and strictly increasing.
    pub freq: Vec<f64>,
    /// Power of each line, W.
    pub power: Vec<f64>,
}

/// One base-10 third-octave band of power (numbering, edges and labels as `crate::acoustics::Band`).
#[derive(Debug, Clone, PartialEq)]
pub struct PowerBand {
    /// Band index `n` (centre `1000 * 10^(n/10)`).
    pub n: i32,
    /// Nominal label, Hz.
    pub nominal: f64,
    /// Exact centre, Hz.
    pub centre: f64,
    /// Lower edge (inclusive), Hz.
    pub lo: f64,
    /// Upper edge (exclusive), Hz.
    pub hi: f64,
    /// Power in the band, W.
    pub power: f64,
}

impl PowerBand {
    /// Band level re 1e-12 W; `None` for zero power.
    pub fn level_db(&self) -> Option<f64> {
        power_level_db(self.power).ok().flatten()
    }
}

impl PowerSpectrum {
    /// A spectrum; refuses lengths that differ, frequencies not positive or not strictly increasing, and powers negative or not
    /// finite.
    pub fn new(freq: Vec<f64>, power: Vec<f64>) -> Res<PowerSpectrum> {
        if freq.len() != power.len() || freq.is_empty() {
            return err(format!("{} frequencies and {} powers", freq.len(), power.len()));
        }
        if let Some(i) = freq.iter().position(|f| !(f.is_finite() && *f > 0.0)) {
            return err(format!(
                "line {i}: frequency must be positive and finite, got {}",
                freq[i]
            ));
        }
        if let Some(i) = freq
            .windows(2)
            .position(|w| w[1].partial_cmp(&w[0]) != Some(std::cmp::Ordering::Greater))
        {
            return err(format!("line {}: frequencies must be strictly increasing", i + 1));
        }
        if let Some(i) = power.iter().position(|w| !(w.is_finite() && *w >= 0.0)) {
            return err(format!(
                "line {i}: power must be finite and not negative, got {}",
                power[i]
            ));
        }
        Ok(PowerSpectrum { freq, power })
    }

    /// The sum of the lines, W.
    pub fn total(&self) -> f64 {
        self.power.iter().sum()
    }

    /// The level of the total re 1e-12 W; `None` for zero power.
    pub fn level_db(&self) -> Option<f64> {
        power_level_db(self.total()).ok().flatten()
    }

    /// The bands from the one holding `f_min` to the one holding `f_max`; each line is assigned by `acoustics::band_index` and
    /// lines in other bands are dropped. Edges and labels are those of `acoustics::third_octave_bands`.
    pub fn third_octave_bands(&self, f_min: f64, f_max: f64) -> Res<Vec<PowerBand>> {
        let empty = Spectrum {
            df: 1.0,
            freq: vec![0.0],
            ms: vec![0.0],
            n_used: 1,
        };
        let layout = third_octave_bands(&empty, f_min, f_max).map_err(|e| format!("solid acoustics: {e}"))?;
        let mut bands: Vec<PowerBand> = layout
            .into_iter()
            .map(|b| PowerBand {
                n: b.n,
                nominal: b.nominal,
                centre: b.centre,
                lo: b.lo,
                hi: b.hi,
                power: 0.0,
            })
            .collect();
        let n_lo = bands[0].n;
        let n_hi = bands[bands.len() - 1].n;
        for (f, w) in self.freq.iter().zip(&self.power) {
            let n = band_index(*f).map_err(|e| format!("solid acoustics: {e}"))?;
            if n >= n_lo && n <= n_hi {
                bands[(n - n_lo) as usize].power += w;
            }
        }
        Ok(bands)
    }
}

/// The equivalent-radiated-power spectrum of the surface set `set` over harmonic responses (one line per response at
/// `omega/(2 pi)`, frequencies strictly increasing).
pub fn erp_spectrum(
    model: &SolidModel,
    set: &str,
    responses: &[HarmonicResponse],
    rho: f64,
    c: f64,
) -> Res<PowerSpectrum> {
    let mut freq = Vec::with_capacity(responses.len());
    let mut power = Vec::with_capacity(responses.len());
    for r in responses {
        let s = VibratingSurface::from_response(model, set, r)?;
        freq.push(r.omega / (2.0 * PI));
        power.push(equivalent_radiated_power(&s, rho, c)?.power);
    }
    PowerSpectrum::new(freq, power)
}

// ------------------------------------------------------------------------------------------------------------ Rayleigh

/// Options of the Rayleigh integral.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayleighOptions {
    /// Largest distance of a node from the mean plane, as a fraction of the diameter (default 1e-6).
    pub plane_tolerance: f64,
    /// Gauss order per direction on each face (default 3).
    pub order: usize,
    /// A face is split for a field point closer to its centroid than `near` times its diameter (default 4).
    pub near: f64,
    /// Largest number of splits (default 8).
    pub max_depth: usize,
}

impl Default for RayleighOptions {
    fn default() -> RayleighOptions {
        RayleighOptions {
            plane_tolerance: 1e-6,
            order: 3,
            near: 4.0,
            max_depth: 8,
        }
    }
}

impl RayleighOptions {
    /// The defaults.
    pub fn new() -> RayleighOptions {
        RayleighOptions::default()
    }

    /// Set the plane tolerance.
    pub fn plane_tolerance(mut self, t: f64) -> RayleighOptions {
        self.plane_tolerance = t;
        self
    }

    /// Set the Gauss order.
    pub fn order(mut self, n: usize) -> RayleighOptions {
        self.order = n;
        self
    }

    /// Set the near-field factor.
    pub fn near(mut self, f: f64) -> RayleighOptions {
        self.near = f;
        self
    }

    /// Set the largest number of splits.
    pub fn max_depth(mut self, d: usize) -> RayleighOptions {
        self.max_depth = d;
        self
    }
}

/// The radiated power of a baffled plane.
#[derive(Debug, Clone, PartialEq)]
pub struct RadiatedPower {
    /// `W`, W.
    pub power: f64,
    /// The equivalent radiated power of the same surface, W.
    pub erp: f64,
    /// `power/erp` (the radiation efficiency).
    pub efficiency: f64,
    /// The level of `power` re 1e-12 W; `None` for zero power.
    pub level_db: Option<f64>,
}

/// A plane surface in an infinite rigid baffle, radiating into the half-space on the side of its normal.
#[derive(Debug, Clone)]
pub struct BaffledPlane {
    surface: VibratingSurface,
    options: RayleighOptions,
    normal: Vec3,
    origin: Vec3,
    deviation: f64,
    diameter: f64,
    points: Vec<(Vec3, f64, Phasor)>,
}

impl BaffledPlane {
    /// The plane of `surface`: normal the normalised sum of the face area vectors, origin the area-weighted centroid. Refused
    /// when the faces are not consistently oriented or a node is farther from the plane than `plane_tolerance` times the
    /// diameter (the error states the deviation).
    pub fn new(surface: &VibratingSurface, options: &RayleighOptions) -> Res<BaffledPlane> {
        let o = *options;
        if !(o.plane_tolerance.is_finite() && o.plane_tolerance >= 0.0) {
            return err(format!(
                "plane_tolerance must be finite and not negative, got {}",
                o.plane_tolerance
            ));
        }
        if o.order == 0 || o.order > 16 {
            return err(format!("order must be 1 to 16, got {}", o.order));
        }
        positive("near", o.near)?;
        let mut sum_a = [0.0; 3];
        let mut area = 0.0;
        let mut centroid = [0.0; 3];
        let mut raw = Vec::new();
        for f in 0..surface.faces.len() {
            for p in surface.face_points(f, o.order, (-1.0, 1.0, -1.0, 1.0)) {
                vec3::axpy(&mut sum_a, p.w, p.n);
                vec3::axpy(&mut centroid, p.w, p.x);
                area += p.w;
                raw.push(p);
            }
        }
        let len = vec3::norm(sum_a);
        if len.is_nan() || len < 0.5 * area {
            return err(format!(
                "Rayleigh integral: the faces are not consistently oriented (|sum of area vectors| {len:e} for an area {area:e})"
            ));
        }
        let normal = vec3::scale(sum_a, 1.0 / len);
        let origin = vec3::scale(centroid, 1.0 / area);
        let mut deviation: f64 = 0.0;
        let mut radius: f64 = 0.0;
        for x in &surface.nodes {
            let d = vec3::sub(*x, origin);
            deviation = deviation.max(vec3::dot(d, normal).abs());
            radius = radius.max(vec3::norm(d));
        }
        let diameter = 2.0 * radius;
        if deviation > o.plane_tolerance * diameter {
            return err(format!(
                "Rayleigh integral: the surface is not plane: deviation {deviation:e} m from its mean plane ({:e} of its \
                 diameter), above the tolerance {:e}",
                deviation / diameter,
                o.plane_tolerance
            ));
        }
        let points = raw.iter().map(|p| (p.x, p.w, vdot(&p.v, normal))).collect();
        Ok(BaffledPlane {
            surface: surface.clone(),
            options: o,
            normal,
            origin,
            deviation,
            diameter,
            points,
        })
    }

    /// The unit normal (the side radiated into).
    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    /// The area-weighted centroid (the origin of the far field).
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The largest distance of a node from the plane, m.
    pub fn deviation(&self) -> f64 {
        self.deviation
    }

    /// Twice the largest distance of a node from the origin, m.
    pub fn diameter(&self) -> f64 {
        self.diameter
    }

    fn check_medium(rho: f64, c: f64, omega: f64) -> Res<()> {
        positive("rho", rho)?;
        positive("c", c)?;
        positive("omega", omega)
    }

    /// The pressure `P(x) = (i omega rho/(2 pi)) int V_n e^{-i k R}/R dS` at a point in front of the plane.
    pub fn pressure(&self, x: Vec3, rho: f64, c: f64, omega: f64) -> Res<Phasor> {
        BaffledPlane::check_medium(rho, c, omega)?;
        if x.iter().any(|v| !v.is_finite()) {
            return err("field point not finite".into());
        }
        let h = vec3::dot(vec3::sub(x, self.origin), self.normal);
        if h.is_nan() || h <= self.options.plane_tolerance * self.diameter {
            return err(format!(
                "Rayleigh integral: field point {x:?} is not in front of the plane (height {h:e} m)"
            ));
        }
        let k = omega / c;
        let mut s = Phasor::default();
        for f in 0..self.surface.faces.len() {
            self.accumulate(f, (-1.0, 1.0, -1.0, 1.0), 0, x, k, &mut s);
        }
        // i omega rho/(2 pi) times s
        let a = omega * rho / (2.0 * PI);
        Ok(Phasor::new(-a * s.im, a * s.re))
    }

    fn accumulate(&self, f: usize, b: (f64, f64, f64, f64), depth: usize, x: Vec3, k: f64, s: &mut Phasor) {
        let (u0, u1, v0, v1) = b;
        if depth < self.options.max_depth {
            let c = corners(&self.surface.faces[f]);
            let at = |u: f64, v: f64| {
                let (nn, _, _) = shape(u, v);
                let mut p = [0.0; 3];
                for a in 0..4 {
                    vec3::axpy(&mut p, nn[a], self.surface.nodes[c[a]]);
                }
                p
            };
            let q = [at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)];
            let mut diam: f64 = 0.0;
            for i in 0..4 {
                for j in i + 1..4 {
                    diam = diam.max(vec3::norm(vec3::sub(q[i], q[j])));
                }
            }
            let centre = at((u0 + u1) / 2.0, (v0 + v1) / 2.0);
            if vec3::norm(vec3::sub(x, centre)) < self.options.near * diam {
                let um = (u0 + u1) / 2.0;
                let vm = (v0 + v1) / 2.0;
                for sb in [(u0, um, v0, vm), (um, u1, v0, vm), (um, u1, vm, v1), (u0, um, vm, v1)] {
                    self.accumulate(f, sb, depth + 1, x, k, s);
                }
                return;
            }
        }
        for p in self.surface.face_points(f, self.options.order, b) {
            let vn = vdot(&p.v, self.normal);
            let r = vec3::norm(vec3::sub(x, p.x));
            let g = Phasor::new((k * r).cos() / r, -(k * r).sin() / r);
            let t = cmul(vn, g);
            s.re += p.w * t.re;
            s.im += p.w * t.im;
        }
    }

    /// The far field `F(d)` with `P ~ F e^{-i k r}/r`, `r` from [`BaffledPlane::origin`] along the unit direction `d`.
    pub fn far_field(&self, direction: Vec3, rho: f64, c: f64, omega: f64) -> Res<Phasor> {
        BaffledPlane::check_medium(rho, c, omega)?;
        let len = vec3::norm(direction);
        if !(len.is_finite() && len > 0.0) {
            return err(format!("direction {direction:?} has no length"));
        }
        let d = vec3::scale(direction, 1.0 / len);
        let k = omega / c;
        let mut s = Phasor::default();
        for (y, w, vn) in &self.points {
            let ph = k * vec3::dot(d, vec3::sub(*y, self.origin));
            let t = cmul(*vn, Phasor::new(ph.cos(), ph.sin()));
            s.re += w * t.re;
            s.im += w * t.im;
        }
        let a = omega * rho / (2.0 * PI);
        Ok(Phasor::new(-a * s.im, a * s.re))
    }

    /// `|F(direction)|/|F(normal)|`; refused when the far field on the normal is zero.
    pub fn directivity(&self, direction: Vec3, c: f64, omega: f64) -> Res<f64> {
        let f0 = self.far_field(self.normal, 1.0, c, omega)?.abs();
        if f0 == 0.0 {
            return err("directivity: the far field on the normal is zero".into());
        }
        Ok(self.far_field(direction, 1.0, c, omega)?.abs() / f0)
    }

    /// The radiated power `(rho c k/(4 pi)) sum_i sum_j w_i w_j Re(V_i conj V_j) sin(k R_ij)/R_ij` over the quadrature points
    /// (`sin(k R)/R = k` at `R = 0`), with the equivalent radiated power of the same surface and their ratio.
    pub fn power(&self, rho: f64, c: f64, omega: f64) -> Res<RadiatedPower> {
        BaffledPlane::check_medium(rho, c, omega)?;
        let k = omega / c;
        let p = &self.points;
        let mut total = 0.0;
        for i in 0..p.len() {
            let (xi, wi, vi) = p[i];
            let mut row = 0.0;
            for &(xj, wj, vj) in &p[i + 1..] {
                let r = vec3::norm(vec3::sub(xi, xj));
                let s = if r > 0.0 { (k * r).sin() / r } else { k };
                row += wj * (vi.re * vj.re + vi.im * vj.im) * s;
            }
            total += wi * (2.0 * row + wi * vi.norm_sqr() * k);
        }
        let power = rho * c * k / (4.0 * PI) * total;
        let erp = equivalent_radiated_power(&self.surface, rho, c)?.power;
        Ok(RadiatedPower {
            power,
            erp,
            efficiency: if erp > 0.0 { power / erp } else { f64::NAN },
            level_db: power_level_db(power.max(0.0))?,
        })
    }

    /// The radiated power from the far field, `(1/(4 rho c)) int_sphere |F|^2 dOmega`, by `acoustics::sphere_quadrature(n_theta,
    /// n_phi)` (the field mirrored about the plane): a check of [`BaffledPlane::power`].
    pub fn power_from_far_field(&self, rho: f64, c: f64, omega: f64, n_theta: usize, n_phi: usize) -> Res<f64> {
        BaffledPlane::check_medium(rho, c, omega)?;
        let q = crate::acoustics::sphere_quadrature(n_theta, n_phi).map_err(|e| format!("solid acoustics: {e}"))?;
        let mut s = 0.0;
        for (d, w) in q {
            s += w * self.far_field(d, rho, c, omega)?.norm_sqr();
        }
        Ok(s / (4.0 * rho * c))
    }
}
