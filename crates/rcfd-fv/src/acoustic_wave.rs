//! Linear acoustic wave-equation solver on the finite-volume mesh (F-333, ADR 0351).
//!
//! The unknowns are the pressure perturbation `p` and the velocity `u` of a uniform medium at rest (density `rho0`,
//! sound speed `c`): `p_t + rho0 c^2 div u = 0`, `rho0 u_t + grad p = 0` (ADR 0351 DR1), the small-amplitude limit of the
//! Euler equations of [`crate::compressible`] about a state at rest. Every face takes the exact Riemann flux of the
//! linear system (the upwind flux, DR2) between values extrapolated from the cells with an unlimited least-squares
//! gradient; time is SSP-RK3 with a step chosen by the caller ([`AcousticWaveSolver::max_dt`]).
//!
//! Boundaries (DR3): [`WaveBc::Wall`] (hard wall or symmetry plane, no normal velocity), [`WaveBc::Open`] (the
//! characteristic condition with the ambient state outside: quiet at normal incidence, `(cos - 1)/(cos + 1)` at the
//! angle `theta`), [`WaveBc::Driven`] (a ghost state given by the caller, for a wave entering the domain). Faces on
//! `empty` patches are skipped; cyclic patches are refused.
//!
//! Sponge (DR4): [`WaveSpongeForm::Plain`] damps `p` and `u` with the cell rate; [`WaveSpongeForm::Matched`] is the
//! matched layer of ADR 0237 DR4 in the linear unknowns: `p` and the normal velocity are damped, the tangential velocity is
//! forced by `-(rate/rho0) grad_t Q` with the memory `Q_t = p` per cell. The rates come from
//! [`crate::sponge::SpongeLayer`]. The source is explicit: keep `rate dt` well below 2.5.
//!
//! Opt-in: nothing else in the crate uses this module.

use crate::geometry::FvGeometry;
use crate::sponge::SpongeLayer;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// The medium at rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medium {
    /// Density [kg/m3], finite and `> 0`.
    pub rho0: f64,
    /// Sound speed [m/s], finite and `> 0`.
    pub c: f64,
}

/// A ghost state `(p, u)` as a function of the face centre and the time.
pub type GhostState = Arc<dyn Fn(Vec3, f64) -> (f64, Vec3) + Send + Sync>;

/// Boundary condition of a patch (ADR 0351 DR3).
#[derive(Clone)]
pub enum WaveBc {
    /// Hard wall or symmetry plane: zero normal velocity.
    Wall,
    /// Characteristic condition with the ambient state (`p = 0`, `u = 0`) outside.
    Open,
    /// The Riemann flux against the ghost state given by the function.
    Driven(GhostState),
}

impl std::fmt::Debug for WaveBc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaveBc::Wall => f.write_str("Wall"),
            WaveBc::Open => f.write_str("Open"),
            WaveBc::Driven(_) => f.write_str("Driven(..)"),
        }
    }
}

/// The form of the sponge source (ADR 0351 DR4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveSpongeForm {
    /// `p` and `u` relax to zero with the cell rate.
    Plain,
    /// The matched layer of one planar layer with the given normal (any length but zero; pointing into the layer).
    Matched {
        /// Normal of the layer.
        normal: Vec3,
    },
}

/// A sponge: one damping rate per cell [1/s] (zero outside the layer) and the form.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveSponge {
    /// Rate of every cell, finite and `>= 0`.
    pub rate: Vec<f64>,
    /// Plain or matched.
    pub form: WaveSpongeForm,
}

impl WaveSponge {
    /// The sponge of one planar layer on `geom` (rates at the cell centres), plain or (`matched`) matched with the layer's
    /// normal.
    pub fn from_layer(geom: &FvGeometry, layer: &SpongeLayer, matched: bool) -> Result<Self> {
        let rate = layer.rates(geom)?;
        let form = if matched {
            WaveSpongeForm::Matched { normal: layer.normal }
        } else {
            WaveSpongeForm::Plain
        };
        Ok(Self { rate, form })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FaceKind {
    Empty,
    Wall,
    Open,
    Driven(usize),
}

fn invalid<T>(path: &str, reason: String) -> Result<T> {
    Err(Error::InvalidValue {
        path: path.into(),
        reason,
    })
}

/// The linear acoustic solver (module documentation).
pub struct AcousticWaveSolver {
    geom: FvGeometry,
    medium: Medium,
    kinds: Vec<FaceKind>,
    drivers: Vec<GhostState>,
    /// Per cell: cofactor matrix and determinant of the masked least-squares matrix.
    ls_inv: Vec<([[f64; 3]; 3], f64)>,
    sponge: Option<(Vec<f64>, Option<Vec3>)>,
    /// Pressure perturbation per cell [Pa].
    pub p: Vec<f64>,
    /// Velocity per cell [m/s].
    pub u: Vec<Vec3>,
    /// Memory of the matched layer per cell [Pa s] (zero outside the layer and without a matched sponge).
    pub q: Vec<f64>,
    /// Time [s].
    pub t: f64,
}

impl AcousticWaveSolver {
    /// A solver at rest on `geom` with a condition for every patch that is neither empty nor named twice. Every
    /// non-empty patch must be named; a cyclic patch is refused.
    pub fn new(geom: FvGeometry, medium: Medium, bcs: &[(&str, WaveBc)]) -> Result<Self> {
        if !(medium.rho0.is_finite() && medium.rho0 > 0.0 && medium.c.is_finite() && medium.c > 0.0) {
            return invalid(
                "acoustic_wave.medium",
                format!(
                    "rho0 and c must be finite and > 0, got {} and {}",
                    medium.rho0, medium.c
                ),
            );
        }
        let mesh = geom.mesh.clone();
        let n_int = geom.n_internal;
        let nf = mesh.n_faces();
        for (name, _) in bcs {
            if mesh.patch(name).is_none() {
                return invalid("acoustic_wave.bc", format!("no patch named '{name}'"));
            }
            if bcs.iter().filter(|(n, _)| n == name).count() > 1 {
                return invalid("acoustic_wave.bc", format!("patch '{name}' named twice"));
            }
        }
        let mut kinds = vec![FaceKind::Empty; nf - n_int];
        let mut drivers = Vec::new();
        for patch in mesh.patches() {
            if patch.kind == PatchKind::Empty {
                continue;
            }
            if patch.kind == PatchKind::Cyclic || patch.periodic_with.is_some() {
                return invalid(
                    "acoustic_wave.bc",
                    format!("patch '{}' is cyclic; periodic patches are not supported", patch.name),
                );
            }
            let Some((_, bc)) = bcs.iter().find(|(n, _)| *n == patch.name) else {
                return invalid("acoustic_wave.bc", format!("no condition for patch '{}'", patch.name));
            };
            let kind = match bc {
                WaveBc::Wall => FaceKind::Wall,
                WaveBc::Open => FaceKind::Open,
                WaveBc::Driven(f) => {
                    drivers.push(f.clone());
                    FaceKind::Driven(drivers.len() - 1)
                }
            };
            for f in patch.faces() {
                if !geom.empty[f] {
                    kinds[f - n_int] = kind;
                }
            }
        }
        let ls_inv = ls_matrices(&geom);
        let n = mesh.n_cells();
        Ok(Self {
            geom,
            medium,
            kinds,
            drivers,
            ls_inv,
            sponge: None,
            p: vec![0.0; n],
            u: vec![[0.0; 3]; n],
            q: vec![0.0; n],
            t: 0.0,
        })
    }

    /// The geometry.
    pub fn geometry(&self) -> &FvGeometry {
        &self.geom
    }

    /// The medium.
    pub fn medium(&self) -> Medium {
        self.medium
    }

    /// Set (`Some`) or clear (`None`) the sponge. The memory `q` is set to zero.
    pub fn set_sponge(&mut self, sponge: Option<WaveSponge>) -> Result<()> {
        self.q.iter_mut().for_each(|q| *q = 0.0);
        let Some(s) = sponge else {
            self.sponge = None;
            return Ok(());
        };
        let n = self.p.len();
        if s.rate.len() != n {
            return invalid("acoustic_wave.sponge", format!("{} rates for {n} cells", s.rate.len()));
        }
        if let Some(i) = s.rate.iter().position(|r| !(r.is_finite() && *r >= 0.0)) {
            return invalid(
                "acoustic_wave.sponge",
                format!("rate of cell {i} must be finite and >= 0, got {}", s.rate[i]),
            );
        }
        let normal = match s.form {
            WaveSpongeForm::Plain => None,
            WaveSpongeForm::Matched { normal } => {
                let len = vec3::norm(normal);
                if !(len.is_finite() && len > 0.0) {
                    return invalid(
                        "acoustic_wave.sponge",
                        "the matched normal must be finite and non-zero".into(),
                    );
                }
                Some(vec3::scale(normal, 1.0 / len))
            }
        };
        self.sponge = Some((s.rate, normal));
        Ok(())
    }

    /// The step for the multi-dimensional acoustic number `cfl`: `cfl min_c 2 V_c / (c sum_f |S_f|)` over the non-empty
    /// faces of each cell (ADR 0351 DR2).
    pub fn max_dt(&self, cfl: f64) -> f64 {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let mut sum = vec![0.0; mesh.n_cells()];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            sum[own[f]] += a;
            if f < self.geom.n_internal {
                sum[nei[f]] += a;
            }
        }
        let vol = mesh.cell_volumes();
        (0..mesh.n_cells())
            .map(|c| cfl * 2.0 * vol[c] / (self.medium.c * sum[c]))
            .fold(f64::INFINITY, f64::min)
    }

    /// The integral of `p` over the domain, `sum_c p_c V_c`.
    pub fn pressure_integral(&self) -> f64 {
        self.p
            .iter()
            .zip(self.geom.mesh.cell_volumes())
            .map(|(p, v)| p * v)
            .sum()
    }

    /// The acoustic energy `sum_c (p^2/(2 rho0 c^2) + rho0 |u|^2/2) V_c` [J].
    pub fn energy(&self) -> f64 {
        let k = self.medium.rho0 * self.medium.c * self.medium.c;
        let vol = self.geom.mesh.cell_volumes();
        (0..self.p.len())
            .map(|c| {
                (self.p[c] * self.p[c] / (2.0 * k) + 0.5 * self.medium.rho0 * vec3::dot(self.u[c], self.u[c])) * vol[c]
            })
            .sum()
    }

    /// The least-squares cell gradient of `cells` with the boundary values `bvals` (indexed `face - n_internal`) used by
    /// the solver: the inverse-distance-squared weighted fit of [`crate::gradient::least_squares`] with the matrices
    /// factored once.
    pub fn gradient(&self, cells: &[f64], bvals: &[f64]) -> Vec<Vec3> {
        let mut out = vec![[0.0; 3]; cells.len()];
        self.gradient_into(cells, |f, _| bvals[f], &mut out);
        out
    }

    fn gradient_into(&self, cells: &[f64], bval: impl Fn(usize, usize) -> f64, out: &mut [Vec3]) {
        let geom = &self.geom;
        let mesh = &geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        out.iter_mut().for_each(|g| *g = [0.0; 3]);
        for f in 0..geom.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let d = geom.delta[f];
            let w = 1.0 / vec3::dot(d, d);
            let dphi = cells[nb] - cells[o];
            for i in 0..3 {
                out[o][i] += w * d[i] * dphi;
                out[nb][i] += w * (-d[i]) * (-dphi);
            }
        }
        for f in geom.n_internal..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let o = own[f];
            let d = geom.delta[f];
            let w = 1.0 / vec3::dot(d, d);
            let dphi = bval(f - geom.n_internal, o) - cells[o];
            for i in 0..3 {
                out[o][i] += w * d[i] * dphi;
            }
        }
        for (c, g) in out.iter_mut().enumerate() {
            let (cof, det) = &self.ls_inv[c];
            if det.abs() < 1e-300 {
                *g = [0.0; 3];
                continue;
            }
            let b = *g;
            for (r, gr) in g.iter_mut().enumerate() {
                *gr = (0..3).map(|k| cof[r][k] * b[k]).sum::<f64>() / det;
            }
        }
    }

    /// Advance one SSP-RK3 step of size `dt`.
    pub fn step(&mut self, dt: f64) -> Result<()> {
        if !(dt.is_finite() && dt > 0.0) {
            return invalid("acoustic_wave.dt", format!("the step must be finite and > 0, got {dt}"));
        }
        let n = self.p.len();
        let t0 = self.t;
        let (p0, u0, q0) = (self.p.clone(), self.u.clone(), self.q.clone());
        let mut dp = vec![0.0; n];
        let mut du = vec![[0.0; 3]; n];
        let mut dq = vec![0.0; n];
        // Stage 1.
        self.rhs(t0, &mut dp, &mut du, &mut dq);
        for c in 0..n {
            self.p[c] = p0[c] + dt * dp[c];
            for k in 0..3 {
                self.u[c][k] = u0[c][k] + dt * du[c][k];
            }
            self.q[c] = q0[c] + dt * dq[c];
        }
        // Stage 2.
        self.rhs(t0 + dt, &mut dp, &mut du, &mut dq);
        for c in 0..n {
            self.p[c] = 0.75 * p0[c] + 0.25 * (self.p[c] + dt * dp[c]);
            for k in 0..3 {
                self.u[c][k] = 0.75 * u0[c][k] + 0.25 * (self.u[c][k] + dt * du[c][k]);
            }
            self.q[c] = 0.75 * q0[c] + 0.25 * (self.q[c] + dt * dq[c]);
        }
        // Stage 3.
        self.rhs(t0 + 0.5 * dt, &mut dp, &mut du, &mut dq);
        for c in 0..n {
            self.p[c] = p0[c] / 3.0 + 2.0 / 3.0 * (self.p[c] + dt * dp[c]);
            for k in 0..3 {
                self.u[c][k] = u0[c][k] / 3.0 + 2.0 / 3.0 * (self.u[c][k] + dt * du[c][k]);
            }
            self.q[c] = q0[c] / 3.0 + 2.0 / 3.0 * (self.q[c] + dt * dq[c]);
        }
        self.t = t0 + dt;
        if self.p.iter().any(|v| !v.is_finite()) {
            return Err(Error::Numerical(format!(
                "acoustic wave solver: non-finite pressure at t = {}",
                self.t
            )));
        }
        Ok(())
    }

    /// The time derivatives of the current state at the time `t`.
    fn rhs(&self, t: f64, dp: &mut [f64], du: &mut [Vec3], dq: &mut [f64]) {
        let geom = &self.geom;
        let mesh = &geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let cf = mesh.face_centres();
        let cc = mesh.cell_centres();
        let vol = mesh.cell_volumes();
        let n = self.p.len();
        let n_int = geom.n_internal;
        let (rho0, c) = (self.medium.rho0, self.medium.c);
        let z = rho0 * c;
        let kmod = rho0 * c * c;

        // Gradients (boundary values: DR3).
        let unit = |f: usize| vec3::scale(sf[f], 1.0 / vec3::norm(sf[f]));
        let mut gp = vec![[0.0; 3]; n];
        self.gradient_into(&self.p, |_, o| self.p[o], &mut gp);
        let mut gu = [vec![[0.0; 3]; n], vec![[0.0; 3]; n], vec![[0.0; 3]; n]];
        let mut comp = vec![0.0; n];
        for k in 0..3 {
            if !geom.active[k] {
                continue;
            }
            for (cv, u) in comp.iter_mut().zip(&self.u) {
                *cv = u[k];
            }
            let kinds = &self.kinds;
            let u = &self.u;
            self.gradient_into(
                &comp,
                |b, o| {
                    if kinds[b] == FaceKind::Wall {
                        let nf = unit(b + n_int);
                        u[o][k] - vec3::dot(u[o], nf) * nf[k]
                    } else {
                        u[o][k]
                    }
                },
                &mut gu[k],
            );
        }
        let recon = |cell: usize, x: Vec3| -> (f64, Vec3) {
            let d = vec3::sub(x, cc[cell]);
            let p = self.p[cell] + vec3::dot(gp[cell], d);
            let u = [0, 1, 2].map(|k| self.u[cell][k] + vec3::dot(gu[k][cell], d));
            (p, u)
        };

        dp.iter_mut().for_each(|v| *v = 0.0);
        du.iter_mut().for_each(|v| *v = [0.0; 3]);
        dq.iter_mut().for_each(|v| *v = 0.0);
        for f in 0..n_int {
            let (o, nb) = (own[f], nei[f]);
            let a = vec3::norm(sf[f]);
            let nf = vec3::scale(sf[f], 1.0 / a);
            let (pl, ul) = recon(o, cf[f]);
            let (pr, ur) = recon(nb, cf[f]);
            let (unl, unr) = (vec3::dot(ul, nf), vec3::dot(ur, nf));
            let ps = 0.5 * (pl + pr) + 0.5 * z * (unl - unr);
            let uns = 0.5 * (unl + unr) + 0.5 * (pl - pr) / z;
            let fp = kmod * uns * a;
            dp[o] -= fp;
            dp[nb] += fp;
            for k in 0..3 {
                let fu = ps / rho0 * sf[f][k];
                du[o][k] -= fu;
                du[nb][k] += fu;
            }
        }
        for f in n_int..mesh.n_faces() {
            let kind = self.kinds[f - n_int];
            if kind == FaceKind::Empty {
                continue;
            }
            let o = own[f];
            let a = vec3::norm(sf[f]);
            let nf = vec3::scale(sf[f], 1.0 / a);
            let (pl, ul) = recon(o, cf[f]);
            let unl = vec3::dot(ul, nf);
            let (ps, uns) = match kind {
                FaceKind::Wall => (pl + z * unl, 0.0),
                FaceKind::Open => (0.5 * pl + 0.5 * z * unl, 0.5 * unl + 0.5 * pl / z),
                FaceKind::Driven(i) => {
                    let (pg, ug) = (self.drivers[i])(cf[f], t);
                    let ung = vec3::dot(ug, nf);
                    (
                        0.5 * (pl + pg) + 0.5 * z * (unl - ung),
                        0.5 * (unl + ung) + 0.5 * (pl - pg) / z,
                    )
                }
                FaceKind::Empty => unreachable!("skipped above"),
            };
            dp[o] -= kmod * uns * a;
            for k in 0..3 {
                du[o][k] -= ps / rho0 * sf[f][k];
            }
        }
        for cell in 0..n {
            dp[cell] /= vol[cell];
            for k in 0..3 {
                du[cell][k] /= vol[cell];
            }
        }

        if let Some((rate, normal)) = &self.sponge {
            match normal {
                None => {
                    for cell in 0..n {
                        let s = rate[cell];
                        dp[cell] -= s * self.p[cell];
                        for k in 0..3 {
                            du[cell][k] -= s * self.u[cell][k];
                        }
                    }
                }
                Some(nrm) => {
                    let mut gq = vec![[0.0; 3]; n];
                    self.gradient_into(&self.q, |_, o| self.q[o], &mut gq);
                    for cell in 0..n {
                        let s = rate[cell];
                        if s == 0.0 {
                            continue;
                        }
                        dp[cell] -= s * self.p[cell];
                        let un = vec3::dot(self.u[cell], *nrm);
                        let gn = vec3::dot(gq[cell], *nrm);
                        for k in 0..3 {
                            du[cell][k] -= s * un * nrm[k];
                            du[cell][k] -= s / rho0 * (gq[cell][k] - gn * nrm[k]);
                        }
                        dq[cell] = self.p[cell];
                    }
                }
            }
        }
    }
}

/// Per cell the cofactor matrix (transposed, so that `x = cof b / det`) and the determinant of the masked
/// least-squares matrix, as `gradient::least_squares` builds and solves it.
fn ls_matrices(geom: &FvGeometry) -> Vec<([[f64; 3]; 3], f64)> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n = mesh.n_cells();
    let mut mat = vec![[[0.0; 3]; 3]; n];
    let mut add = |c: usize, dr: Vec3| {
        let w = 1.0 / vec3::dot(dr, dr);
        for i in 0..3 {
            for j in 0..3 {
                mat[c][i][j] += w * dr[i] * dr[j];
            }
        }
    };
    for f in 0..geom.n_internal {
        let d = geom.delta[f];
        add(own[f], d);
        add(nei[f], vec3::scale(d, -1.0));
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            add(own[f], geom.delta[f]);
        }
    }
    mat.into_iter()
        .map(|mut m| {
            for d in 0..3 {
                if !geom.active[d] {
                    for k in 0..3 {
                        m[d][k] = 0.0;
                        m[k][d] = 0.0;
                    }
                    m[d][d] = 1.0;
                }
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            let mut cof = [[0.0; 3]; 3];
            for (r, row) in cof.iter_mut().enumerate() {
                for (col, v) in row.iter_mut().enumerate() {
                    let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
                    let (c1, c2) = ((col + 1) % 3, (col + 2) % 3);
                    *v = m[c1][r1] * m[c2][r2] - m[c1][r2] * m[c2][r1];
                }
            }
            (cof, det)
        })
        .collect()
}
