//! Volume-of-fluid free-surface flow of two incompressible immiscible fluids (ADR 0049).
//!
//! * [`AlphaTransport`] advances the phase fraction `alpha` with the conservative flux
//!   `alpha phi + alpha (1 - alpha) phi_r` (interface-compression flux, `phi_r = c |phi| n_f . S / |S|`) limited by a
//!   Zalesak (1979) flux-corrected-transport limiter between the first-order upwind flux and the central-plus-compression
//!   flux, sub-cycled under a Courant limit. The scheme is bounded in `[0, 1]` for a solenoidal flux and conserves the
//!   phase volume to round-off.
//! * [`VofSolver`] couples it with a transient variable-density PISO/PIMPLE kernel in the `p_rgh = p - rho g.x`
//!   formulation. Gravity and the continuum-surface-force surface tension (Brackbill, Kothe and Zemach 1992) are FACE
//!   forces that enter the pressure equation with the same face gradient and geometry coefficient as the pressure term,
//!   and the cell velocity is reconstructed from the very same face quantities, so a still stratified pool has a velocity
//!   of the order of the linear-solver tolerance.

use crate::convection::{convection, face_flux, ConvScheme};
use crate::diffusion::laplacian;
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, face_interpolate, pressure_bc, solve_pressure,
    PressureControls,
};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::{LinearOptions, SolveControl};
use std::sync::Arc;

/// Initial shape of phase 1 (`alpha = 1` inside).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Axis-aligned box `[min, max]`.
    Box {
        /// Lower corner.
        min: Vec3,
        /// Upper corner.
        max: Vec3,
    },
    /// Sphere (disk in 2-D) of the given radius.
    Sphere {
        /// Centre.
        centre: Vec3,
        /// Radius.
        radius: f64,
    },
    /// Half space `n . x <= offset`.
    HalfSpace {
        /// Normal.
        normal: Vec3,
        /// Offset.
        offset: f64,
    },
}

impl Shape {
    /// Is the point inside the shape?
    pub fn contains(&self, x: Vec3) -> bool {
        match *self {
            Shape::Box { min, max } => (0..3).all(|k| x[k] >= min[k] && x[k] <= max[k]),
            Shape::Sphere { centre, radius } => vec3::norm(vec3::sub(x, centre)) <= radius,
            Shape::HalfSpace { normal, offset } => vec3::dot(normal, x) <= offset,
        }
    }
}

/// Phase fraction of every cell of an axis-aligned mesh from sub-sampling the cell bounding box (`n_sub` cell-centred
/// sample points per active direction) with the indicator `inside`.
pub fn indicator_fraction(geom: &FvGeometry, n_sub: usize, inside: impl Fn(Vec3) -> bool) -> Vec<f64> {
    let mesh = &geom.mesh;
    let n_sub = n_sub.max(1);
    (0..geom.n_cells())
        .map(|c| {
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for &f in mesh.cell_faces(c) {
                for &pi in mesh.face_points(f) {
                    let p = mesh.points()[pi];
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
            let ns: [usize; 3] = [0, 1, 2].map(|k| if geom.active[k] { n_sub } else { 1 });
            let mut count = 0usize;
            for i in 0..ns[0] {
                for j in 0..ns[1] {
                    for l in 0..ns[2] {
                        let idx = [i, j, l];
                        let x = [0, 1, 2].map(|k| lo[k] + (hi[k] - lo[k]) * (idx[k] as f64 + 0.5) / ns[k] as f64);
                        if inside(x) {
                            count += 1;
                        }
                    }
                }
            }
            count as f64 / (ns[0] * ns[1] * ns[2]) as f64
        })
        .collect()
}

/// [`indicator_fraction`] of a [`Shape`]; `invert` gives the complement (phase 1 outside the shape).
pub fn shape_fraction(geom: &FvGeometry, shape: Shape, invert: bool, n_sub: usize) -> Vec<f64> {
    indicator_fraction(geom, n_sub, |x| shape.contains(x) != invert)
}

/// Diagnostics of one phase-fraction advance.
#[derive(Debug, Clone)]
pub struct AlphaInfo {
    /// Number of sub-steps.
    pub n_sub: usize,
    /// Courant number of the whole step, `dt max_c (sum of outflow |phi|) / V`.
    pub courant: f64,
    /// Time-averaged total face flux of `alpha` (convective plus compression) over the step.
    pub alpha_phi: Vec<f64>,
}

/// Explicit conservative, bounded transport of the phase fraction (compression flux plus Zalesak limiter).
pub struct AlphaTransport {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Phase fraction (conditions on boundary patches: zero gradient by default).
    pub alpha: ScalarField,
    /// Compression coefficient `c_alpha` (0 = no compression flux).
    pub c_alpha: f64,
    /// Largest Courant number of a sub-step.
    pub courant_max: f64,
    /// Upper bound on the number of sub-steps (a larger need is an error).
    pub max_sub: usize,
    delta: f64,
}

impl AlphaTransport {
    /// Create the transport of `alpha`.
    pub fn new(geom: Arc<FvGeometry>, alpha: ScalarField, c_alpha: f64, courant_max: f64) -> Self {
        let vmean = geom.mesh.total_volume() / geom.n_cells() as f64;
        AlphaTransport {
            geom,
            alpha,
            c_alpha,
            courant_max,
            max_sub: 2000,
            delta: 1e-8 / vmean.cbrt(),
        }
    }

    /// Courant number `dt max_c (sum over outflow faces of |phi|) / V` of the flux `phi`.
    pub fn courant(&self, phi: &[f64], dt: f64) -> f64 {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut out = vec![0.0; g.n_cells()];
        for f in 0..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            if f < g.n_internal {
                if phi[f] >= 0.0 {
                    out[own[f]] += phi[f];
                } else {
                    out[nei[f]] -= phi[f];
                }
            } else if phi[f] > 0.0 {
                out[own[f]] += phi[f];
            }
        }
        let vol = mesh.cell_volumes();
        dt * out.iter().zip(vol).map(|(o, v)| o / v).fold(0.0, f64::max)
    }

    /// Advance `alpha` by `dt` with the frozen face flux `phi`.
    pub fn advance(&mut self, phi: &[f64], dt: f64) -> Result<AlphaInfo> {
        let co = self.courant(phi, dt);
        let n_sub = ((co / self.courant_max).ceil() as usize).max(1);
        if n_sub > self.max_sub {
            return Err(Error::InvalidValue {
                path: "multiphase.alpha_courant".into(),
                reason: format!(
                    "the phase-fraction Courant number {co:.3} needs {n_sub} sub-steps (limit {}); reduce the time step",
                    self.max_sub
                ),
            });
        }
        let dts = dt / n_sub as f64;
        let mut acc = vec![0.0; self.geom.mesh.n_faces()];
        for _ in 0..n_sub {
            self.substep(phi, dts, dts / dt, &mut acc);
        }
        Ok(AlphaInfo {
            n_sub,
            courant: co,
            alpha_phi: acc,
        })
    }

    fn substep(&mut self, phi: &[f64], dts: f64, weight: f64, acc: &mut [f64]) {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let vol = mesh.cell_volumes();
        let a = self.alpha.values.clone();
        let bv = self.alpha.boundary_values(&g);
        let grad = green_gauss(&g, &a, &bv, 0);

        let mut f_low = vec![0.0; n_int];
        let mut anti = vec![0.0; n_int];
        let mut net = vec![0.0; n];
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let w = g.weights[f];
            let up = if phi[f] >= 0.0 { a[o] } else { a[nb] };
            let lin = w * a[o] + (1.0 - w) * a[nb];
            let gf = [0, 1, 2].map(|k| w * grad[o][k] + (1.0 - w) * grad[nb][k]);
            let nd = vec3::dot(gf, sf[f]) / (vec3::norm(gf) + self.delta);
            let phi_r = self.c_alpha * phi[f].abs() / vec3::norm(sf[f]) * nd;
            let high = phi[f] * lin + phi_r * lin * (1.0 - lin);
            f_low[f] = phi[f] * up;
            anti[f] = high - f_low[f];
            net[o] += f_low[f];
            net[nb] -= f_low[f];
        }
        let mut f_bnd = vec![0.0; mesh.n_faces() - n_int];
        for f in n_int..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            f_bnd[f - n_int] = phi[f] * bv[f - n_int];
            net[own[f]] += f_bnd[f - n_int];
        }
        let low: Vec<f64> = (0..n).map(|c| a[c] - dts / vol[c] * net[c]).collect();

        // Zalesak limiter
        let mut p_plus = vec![0.0; n];
        let mut p_minus = vec![0.0; n];
        let mut a_max: Vec<f64> = (0..n).map(|c| a[c].max(low[c])).collect();
        let mut a_min: Vec<f64> = (0..n).map(|c| a[c].min(low[c])).collect();
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let x = anti[f];
            p_plus[o] += (-x).max(0.0);
            p_minus[o] += x.max(0.0);
            p_plus[nb] += x.max(0.0);
            p_minus[nb] += (-x).max(0.0);
            let (hi_n, lo_n) = (a[nb].max(low[nb]), a[nb].min(low[nb]));
            let (hi_o, lo_o) = (a[o].max(low[o]), a[o].min(low[o]));
            a_max[o] = a_max[o].max(hi_n);
            a_min[o] = a_min[o].min(lo_n);
            a_max[nb] = a_max[nb].max(hi_o);
            a_min[nb] = a_min[nb].min(lo_o);
        }
        let mut r_plus = vec![1.0; n];
        let mut r_minus = vec![1.0; n];
        for c in 0..n {
            let q_plus = vol[c] / dts * (a_max[c].min(1.0) - low[c]);
            let q_minus = vol[c] / dts * (low[c] - a_min[c].max(0.0));
            if p_plus[c] > 1e-300 {
                r_plus[c] = (q_plus / p_plus[c]).clamp(0.0, 1.0);
            }
            if p_minus[c] > 1e-300 {
                r_minus[c] = (q_minus / p_minus[c]).clamp(0.0, 1.0);
            }
        }
        let mut net2 = vec![0.0; n];
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let c = if anti[f] >= 0.0 {
                r_plus[nb].min(r_minus[o])
            } else {
                r_plus[o].min(r_minus[nb])
            };
            let flux = f_low[f] + c * anti[f];
            net2[o] += flux;
            net2[nb] -= flux;
            acc[f] += weight * flux;
        }
        for f in n_int..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            net2[own[f]] += f_bnd[f - n_int];
            acc[f] += weight * f_bnd[f - n_int];
        }
        for c in 0..n {
            self.alpha.values[c] = a[c] - dts / vol[c] * net2[c];
        }
    }

    /// Volume of phase 1, `sum alpha V`.
    pub fn phase_volume(&self) -> f64 {
        self.alpha
            .values
            .iter()
            .zip(self.geom.mesh.cell_volumes())
            .map(|(a, v)| a * v)
            .sum()
    }
}

/// Settings of [`VofSolver`].
#[derive(Debug, Clone)]
pub struct VofSettings {
    /// Density of phase 1 (`alpha = 1`) and phase 2.
    pub rho: [f64; 2],
    /// Dynamic viscosity of phase 1 and phase 2.
    pub mu: [f64; 2],
    /// Surface tension coefficient [N/m] (0 = none).
    pub sigma: f64,
    /// Gravity vector.
    pub gravity: Vec3,
    /// Origin of the gravity potential `g . (x - x_ref)`.
    pub x_ref: Vec3,
    /// Interface compression coefficient.
    pub c_alpha: f64,
    /// Largest Courant number of a phase-fraction sub-step.
    pub alpha_courant: f64,
    /// Smoothing passes of the phase fraction before the curvature.
    pub n_smooth: usize,
    /// Convection scheme of momentum.
    pub convection: ConvScheme,
    /// Outer iterations per step (1 = PISO).
    pub n_outer: usize,
    /// Pressure correctors per outer iteration.
    pub n_correctors: usize,
    /// Non-orthogonal pressure correctors.
    pub non_orth_correctors: usize,
    /// Velocity under-relaxation on non-final outer iterations.
    pub alpha_u: f64,
    /// Pressure under-relaxation on non-final outer iterations.
    pub alpha_p: f64,
    /// Transient Rhie-Chow correction.
    pub ddt_correction: bool,
    /// Reference cell and value of `p_rgh` when no boundary fixes the level.
    pub p_ref: (usize, f64),
    /// AMG-preconditioned CG for the pressure equation.
    pub amg_pressure: bool,
    /// Momentum linear solver.
    pub momentum_solver: SolveControl,
    /// Pressure linear solver.
    pub pressure_solver: SolveControl,
    /// Linear solver device.
    pub linear: LinearOptions,
}

impl VofSettings {
    /// Defaults for the given phase properties.
    pub fn new(rho: [f64; 2], mu: [f64; 2]) -> Self {
        VofSettings {
            rho,
            mu,
            sigma: 0.0,
            gravity: [0.0; 3],
            x_ref: [0.0; 3],
            c_alpha: 1.0,
            alpha_courant: 0.25,
            n_smooth: 2,
            convection: ConvScheme::Upwind,
            n_outer: 1,
            n_correctors: 3,
            non_orth_correctors: 0,
            alpha_u: 0.8,
            alpha_p: 0.5,
            ddt_correction: true,
            p_ref: (0, 0.0),
            amg_pressure: false,
            momentum_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-8,
                max_iter: 500,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-16,
                tol_rel: 1e-12,
                max_iter: 5000,
            },
            linear: LinearOptions::default(),
        }
    }
}

/// Diagnostics of one VOF time step.
#[derive(Debug, Clone, Copy)]
pub struct VofStepInfo {
    /// Step number (1-based).
    pub step: usize,
    /// Time after the step.
    pub time: f64,
    /// Step size.
    pub dt: f64,
    /// Flow Courant number of the new flux.
    pub courant: f64,
    /// Phase-fraction Courant number (whole step).
    pub alpha_courant: f64,
    /// Phase-fraction sub-steps.
    pub n_sub: usize,
    /// Normalised continuity error of the new flux.
    pub continuity: f64,
    /// Initial momentum residual, first outer iteration.
    pub res_u: f64,
    /// Initial pressure residual of the first corrector of the last outer iteration.
    pub res_p: f64,
    /// Pressure linear iterations.
    pub p_iters: usize,
    /// `min alpha`.
    pub alpha_min: f64,
    /// `max alpha`.
    pub alpha_max: f64,
    /// True when `dt` exceeded the capillary time-step limit.
    pub capillary_exceeded: bool,
}

/// Transient VOF solver state.
pub struct VofSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: VofSettings,
    /// Velocity.
    pub u: VectorField,
    /// Modified pressure `p - rho g.(x - x_ref)` [Pa].
    pub p_rgh: ScalarField,
    /// Pressure [Pa] `p_rgh + rho g.(x - x_ref)`.
    pub p: Vec<f64>,
    /// Phase fraction transport (field `alpha`).
    pub transport: AlphaTransport,
    /// Volumetric face flux.
    pub phi: Vec<f64>,
    /// Current time.
    pub time: f64,
    /// Diagnostics of every step.
    pub history: Vec<VofStepInfo>,
    step_no: usize,
    /// Inverse of `sum_f S S / |S|` per cell on the active dimensions (row-major).
    recon: Vec<[[f64; 3]; 3]>,
}

impl VofSolver {
    /// Create a solver at `t = 0`; the initial flux is interpolated from `u`.
    pub fn new(
        geom: Arc<FvGeometry>,
        u: VectorField,
        p_rgh: ScalarField,
        alpha: ScalarField,
        settings: VofSettings,
    ) -> Result<Self> {
        if geom.periodic_partner.iter().any(|p| p.is_some()) {
            return Err(Error::InvalidValue {
                path: "mesh".into(),
                reason: "the VOF solver does not support periodic patches".into(),
            });
        }
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        let transport = AlphaTransport::new(geom.clone(), alpha, settings.c_alpha, settings.alpha_courant);
        let recon = reconstruction_tensors(&geom);
        let mut s = VofSolver {
            geom,
            settings,
            u,
            p_rgh,
            p: Vec::new(),
            transport,
            phi,
            time: 0.0,
            history: Vec::new(),
            step_no: 0,
            recon,
        };
        s.update_pressure();
        Ok(s)
    }

    /// Mixture density of every cell.
    pub fn density(&self) -> Vec<f64> {
        let [r1, r2] = self.settings.rho;
        self.transport.alpha.values.iter().map(|a| r2 + (r1 - r2) * a).collect()
    }

    fn gh_cells(&self) -> Vec<f64> {
        let s = &self.settings;
        self.geom
            .mesh
            .cell_centres()
            .iter()
            .map(|x| vec3::dot(s.gravity, vec3::sub(*x, s.x_ref)))
            .collect()
    }

    fn update_pressure(&mut self) {
        let rho = self.density();
        let gh = self.gh_cells();
        self.p = (0..rho.len()).map(|c| self.p_rgh.values[c] + rho[c] * gh[c]).collect();
    }

    /// Capillary time-step limit `sqrt((rho_1 + rho_2) dx^3 / (8 pi sigma))` (infinite without surface tension).
    pub fn capillary_dt(&self) -> f64 {
        let s = &self.settings;
        if s.sigma <= 0.0 {
            return f64::INFINITY;
        }
        let g = &self.geom;
        let dx = (0..g.n_internal)
            .filter(|&f| !g.empty[f])
            .map(|f| vec3::norm(g.delta[f]))
            .fold(f64::INFINITY, f64::min);
        ((s.rho[0] + s.rho[1]) * dx.powi(3) / (8.0 * std::f64::consts::PI * s.sigma)).sqrt()
    }

    /// Cell curvature `kappa = -div(n)` of the normalised gradient of the smoothed phase fraction (internal faces only,
    /// i.e. a 90 degree contact angle at walls).
    pub fn curvature(&self) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let n = g.n_cells();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let vol = mesh.cell_volumes();
        let mut a = self.transport.alpha.values.clone();
        for _ in 0..self.settings.n_smooth {
            let mut sum = a.clone();
            let mut cnt = vec![1.0; n];
            for f in 0..g.n_internal {
                sum[own[f]] += a[nei[f]];
                sum[nei[f]] += a[own[f]];
                cnt[own[f]] += 1.0;
                cnt[nei[f]] += 1.0;
            }
            // zero-gradient mirror at the domain boundary: a missing neighbour counts as the cell itself, so a flat
            // interface stays flat up to the wall
            for f in g.n_internal..mesh.n_faces() {
                if !g.empty[f] {
                    sum[own[f]] += a[own[f]];
                    cnt[own[f]] += 1.0;
                }
            }
            a = (0..n).map(|c| sum[c] / cnt[c]).collect();
        }
        let bv: Vec<f64> = (0..mesh.n_boundary_faces()).map(|i| a[own[g.n_internal + i]]).collect();
        let grad = green_gauss(g, &a, &bv, 0);
        let delta = 1e-8 / (mesh.total_volume() / n as f64).cbrt();
        let mut kappa = vec![0.0; n];
        for f in 0..g.n_internal {
            let w = g.weights[f];
            let gf = [0, 1, 2].map(|k| w * grad[own[f]][k] + (1.0 - w) * grad[nei[f]][k]);
            let flux = vec3::dot(gf, sf[f]) / (vec3::norm(gf) + delta);
            kappa[own[f]] -= flux / vol[own[f]];
            kappa[nei[f]] += flux / vol[nei[f]];
        }
        kappa
    }

    /// Face force flux `lap_coeff [ sigma kappa_f (alpha_N - alpha_P) - (g.x_f)(rho_N - rho_P) ]` of the internal faces.
    fn force_flux(&self, rho: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let s = &self.settings;
        let a = &self.transport.alpha.values;
        let kappa = if s.sigma > 0.0 { Some(self.curvature()) } else { None };
        let cf = mesh.face_centres();
        (0..g.n_internal)
            .map(|f| {
                let (o, nb) = (own[f], nei[f]);
                let mut t = 0.0;
                if let Some(k) = &kappa {
                    let w = g.weights[f];
                    t += s.sigma * (w * k[o] + (1.0 - w) * k[nb]) * (a[nb] - a[o]);
                }
                let ghf = vec3::dot(s.gravity, vec3::sub(cf[f], s.x_ref));
                t -= ghf * (rho[nb] - rho[o]);
                g.lap_coeff[f] * t
            })
            .collect()
    }

    /// Cell vector from a face flux `q` (`sum_f (S/|S|) q_f` times the inverse tensor): the least-squares-exact
    /// reconstruction of a gradient-like quantity whose face flux is `q`.
    fn reconstruct(&self, q: &[f64]) -> Vec<Vec3> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let mut sum = vec![[0.0; 3]; g.n_cells()];
        for (f, &qf) in q.iter().enumerate() {
            if g.empty[f] || qf == 0.0 {
                continue;
            }
            let nrm = vec3::scale(sf[f], 1.0 / vec3::norm(sf[f]));
            // outward normal and outward flux both change sign for the neighbour: same contribution
            vec3::axpy(&mut sum[own[f]], qf, nrm);
            if f < g.n_internal {
                vec3::axpy(&mut sum[nei[f]], qf, nrm);
            }
        }
        sum.iter()
            .zip(&self.recon)
            .map(|(s, t)| [0, 1, 2].map(|i| t[i][0] * s[0] + t[i][1] * s[1] + t[i][2] * s[2]))
            .collect()
    }

    /// Advance one time step of size `dt`.
    pub fn step(&mut self, dt: f64) -> Result<VofStepInfo> {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let nf = mesh.n_faces();
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        self.step_no += 1;
        let [r1, r2] = s.rho;

        let rho_old = self.density();
        let u_old: Vec<Vec3> = self.u.values.clone();
        let phi_old = self.phi.clone();

        // ---- phase fraction ---------------------------------------------------------
        let ainfo = self.transport.advance(&self.phi, dt)?;
        let rho = self.density();
        let a = self.transport.alpha.values.clone();
        let mu_c: Vec<f64> = a.iter().map(|x| s.mu[1] + (s.mu[0] - s.mu[1]) * x).collect();
        let mu_f = face_interpolate(&g, &mu_c);
        let rho_phi: Vec<f64> = (0..nf)
            .map(|f| {
                if g.empty[f] {
                    0.0
                } else {
                    r2 * self.phi[f] + (r1 - r2) * ainfo.alpha_phi[f]
                }
            })
            .collect();
        let force = self.force_flux(&rho);

        // old-level flux defect for the transient Rhie-Chow term
        let ddt_defect: Vec<f64> = if s.ddt_correction {
            let zero_b = vec![[0.0; 3]; mesh.n_boundary_faces()];
            let fo = face_flux(&g, &u_old, &zero_b);
            (0..n_int).map(|f| phi_old[f] - fo[f]).collect()
        } else {
            vec![0.0; n_int]
        };

        let n_active = g.active.iter().filter(|&&x| x).count() as f64;
        let mut res_u: f64 = 0.0;
        let mut res_p = 0.0;
        let mut p_iters = 0;
        let own = mesh.owner();

        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let pbc = pressure_bc(&g, &self.u, &self.p_rgh);
            let ub = self.u.boundary_values(&g);

            // net face acceleration flux of the previous iterate (force minus p_rgh gradient) -> predictor source
            let mut q_pred = vec![0.0; nf];
            for f in 0..n_int {
                if !g.empty[f] {
                    q_pred[f] = force[f]
                        - g.lap_coeff[f] * (self.p_rgh.values[mesh.neighbour()[f]] - self.p_rgh.values[own[f]]);
                }
            }
            for f in n_int..nf {
                let i = f - n_int;
                if !g.empty[f] && !pbc.fixed_u[i] {
                    q_pred[f] = -vec3::norm(sf[f]) * pbc.coeffs[i].gradient(self.p_rgh.values[own[f]]);
                }
            }
            let src = self.reconstruct(&q_pred);

            let mut mats: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            let u_iter: Vec<Vec3> = self.u.values.clone();
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let bc = self.u.boundary_coeffs(&g, k);
                let bvk: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                let grad = if s.convection.needs_gradient() || s.non_orth_correctors > 0 {
                    Some(least_squares(&g, &cells, &bvk))
                } else {
                    None
                };
                let mut m = convection(&g, &rho_phi, &cells, &bc, s.convection, grad.as_deref());
                m.add(&laplacian(&g, &mu_f, &bc, grad.as_deref()));
                for c in 0..n {
                    m.a.diag[c] += rho[c] * vol[c] / dt;
                    m.b[c] += rho_old[c] * vol[c] / dt * u_old[c][k];
                }
                if alpha_u < 1.0 {
                    m.relax(alpha_u, &cells);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] += vol[c] * src[c][k];
                }
                if outer == 0 {
                    res_u = res_u.max(crate::simple::normalised_residual(&sys, &cells));
                }
                let mut x = cells;
                sys.solve(&mut x, &s.momentum_solver);
                for c in 0..n {
                    self.u.values[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                mats[k] = Some(m);
            }
            for x in a_p.iter_mut() {
                *x /= n_active;
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            let gamma_p = face_interpolate(&g, &r_au);
            // transient Rhie-Chow coefficient interpolate(rho rAU) / dt <= 1 (rho and rAU are interpolated together: their
            // separate interpolations are unbounded at a density jump and made the hydrostatic state unstable)
            let rho_r_au: Vec<f64> = (0..n).map(|c| rho[c] * r_au[c]).collect();
            let ddt_coeff = face_interpolate(&g, &rho_r_au);

            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let h = m.a.h_operator(&cells, &m.b);
                        for c in 0..n {
                            hby_a[c][k] = h[c] / a_p[c];
                        }
                    }
                }
                let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a);
                let mut phi_hby_a = face_flux(&g, &hby_a, &hb);
                if s.ddt_correction {
                    for f in 0..n_int {
                        phi_hby_a[f] += ddt_coeff[f] / dt * ddt_defect[f];
                    }
                }
                let phi_hby_a0 = phi_hby_a.clone();
                for f in 0..n_int {
                    if !g.empty[f] {
                        phi_hby_a[f] += gamma_p[f] * force[f];
                    }
                }
                let controls = PressureControls {
                    p_ref: s.p_ref,
                    non_orth_correctors: s.non_orth_correctors,
                    solver: s.pressure_solver,
                    amg: s.amg_pressure,
                    linear: s.linear,
                };
                let p_old = self.p_rgh.values.clone();
                let mut p_new = p_old.clone();
                let (r0, it) = solve_pressure(&g, &gamma_p, &pbc, &phi_hby_a, &mut p_new, &controls);
                p_iters += it;
                if corr == 0 {
                    res_p = r0;
                }
                self.phi = corrected_flux(&g, &gamma_p, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0);
                let mut q = vec![0.0; nf];
                for f in 0..nf {
                    if g.empty[f] || (f >= n_int && pbc.fixed_u[f - n_int]) {
                        continue;
                    }
                    q[f] = (self.phi[f] - phi_hby_a0[f]) / gamma_p[f];
                }
                let rec = self.reconstruct(&q);
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            self.u.values[c][k] = hby_a[c][k] + r_au[c] * rec[c][k];
                        }
                    }
                }
                let last_corr = corr + 1 == s.n_correctors;
                let ap = if last_corr { alpha_p } else { 1.0 };
                for c in 0..n {
                    self.p_rgh.values[c] = p_old[c] + ap * (p_new[c] - p_old[c]);
                }
            }
        }

        self.time += dt;
        self.update_pressure();
        let cont = divergence(&g, &self.phi).iter().fold(0.0f64, |m, &b| m.max(b.abs()));
        let area_scale = sf.iter().map(|x| vec3::norm(*x)).fold(0.0, f64::max);
        let (amin, amax) = self
            .transport
            .alpha
            .values
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                (lo.min(x), hi.max(x))
            });
        let info = VofStepInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant: crate::ddt::max_courant(&g, &self.phi, dt),
            alpha_courant: ainfo.courant,
            n_sub: ainfo.n_sub,
            continuity: cont / area_scale.max(1e-300),
            res_u,
            res_p,
            p_iters,
            alpha_min: amin,
            alpha_max: amax,
            capillary_exceeded: dt > self.capillary_dt(),
        };
        self.history.push(info);
        Ok(info)
    }

    /// Advance to `t_end` with a fixed step.
    pub fn run_fixed(&mut self, t_end: f64, dt: f64) -> Result<Vec<VofStepInfo>> {
        let mut out = Vec::new();
        while self.time < t_end - 1e-12 * dt.max(1e-300) {
            let d = dt.min(t_end - self.time);
            out.push(self.step(d)?);
        }
        Ok(out)
    }

    /// Volume of phase 1.
    pub fn phase_volume(&self) -> f64 {
        self.transport.phase_volume()
    }

    /// Boundary-face values of `p_rgh` under the pressure conditions (diagnostic).
    pub fn boundary_p_rgh(&self) -> Vec<f64> {
        let pbc = pressure_bc(&self.geom, &self.u, &self.p_rgh);
        boundary_values_pbc(&self.geom, &pbc, &self.p_rgh.values)
    }
}

/// Inverse of `T = sum_f S S / |S|` per cell over the non-empty faces; inactive directions get the identity entry.
fn reconstruction_tensors(geom: &FvGeometry) -> Vec<[[f64; 3]; 3]> {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut t = vec![[[0.0; 3]; 3]; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let a = vec3::norm(sf[f]);
        let mut rows = vec![own[f]];
        if f < geom.n_internal {
            rows.push(nei[f]);
        }
        for row in rows {
            for p in 0..3 {
                for q in 0..3 {
                    t[row][p][q] += sf[f][p] * sf[f][q] / a;
                }
            }
        }
    }
    t.iter_mut()
        .map(|m| {
            for k in 0..3 {
                if !geom.active[k] {
                    m[k] = [0.0; 3];
                    for row in m.iter_mut() {
                        row[k] = 0.0;
                    }
                    m[k][k] = 1.0;
                }
            }
            invert3(*m)
        })
        .collect()
}

fn invert3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |r: usize, s: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (s1, s2) = ((s + 1) % 3, (s + 2) % 3);
        m[r1][s1] * m[r2][s2] - m[r1][s2] * m[r2][s1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    let mut inv = [[0.0; 3]; 3];
    for (i, row) in inv.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det;
        }
    }
    inv
}
