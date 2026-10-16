//! Reynolds stress transport model with an epsilon equation (F-046, ADR 0032).
//!
//! Sources: the pressure-strain correlation of Speziale, Sarkar and Gatski (1991) in the anisotropy form and with the
//! "outer" coefficient set printed on the NASA Turbulence Modeling Resource page of the SSG/LRR-omega model (verified, see
//! docs/reference-data.md); the isotropisation-of-production model of Launder, Reece and Rodi (1975) (`C1 = 1.8`
//! consistent with the same page, `C2 = 0.6` from memory, unverified); generalised gradient diffusion in the normal-stress
//! form. Incompressible, kinematic:
//!
//! ```text
//! D R_ij/Dt = P_ij + Phi_ij - 2/3 eps delta_ij + d_k[(nu + c_s (k/eps) R_nn) d_k R_ij]
//! P_ij = -R_ik dU_j/dx_k - R_jk dU_i/dx_k,   k = R_ii/2,   P = P_kk/2
//! D eps/Dt = (eps/k)(Ce1 P - Ce2 eps) + d_k[(nu + c_eps (k/eps) R_nn) d_k eps]
//! ```
//!
//! `R_nn = n.R.n` is the normal stress on the face (only the diagonal of the gradient-diffusion tensor enters the implicit
//! face coefficient; the cross terms `R_kl`, `k != l`, are not delivered). Walls are not delivered: there is no wall
//! treatment and no wall reflection.

use crate::eddy::effective_viscosity;
use crate::kepsilon::velocity_gradient;
use crate::model::{CoefficientInfo, ModelContext, TurbulenceModel};
use rcfd_core::{Error, Result};
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::laplacian;
use rcfd_fv::pressure::face_interpolate;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// Symmetric second-order tensor (stored full).
pub type Sym = [[f64; 3]; 3];

/// Pressure-strain variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureStrain {
    /// Speziale, Sarkar and Gatski (quasi-linear).
    Ssg,
    /// Launder, Reece and Rodi, isotropisation of production.
    LrrIp,
}

/// Model coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RsmCoefficients {
    /// SSG `C1`.
    pub c1: f64,
    /// SSG `C1*`.
    pub c1s: f64,
    /// SSG `C2`.
    pub c2: f64,
    /// SSG `C3`.
    pub c3: f64,
    /// SSG `C3*`.
    pub c3s: f64,
    /// SSG `C4`.
    pub c4: f64,
    /// SSG `C5`.
    pub c5: f64,
    /// LRR-IP `C1`.
    pub c1_lrr: f64,
    /// LRR-IP `C2`.
    pub c2_lrr: f64,
    /// `Ce1`.
    pub ce1: f64,
    /// `Ce2` with the SSG pressure-strain.
    pub ce2_ssg: f64,
    /// `Ce2` with LRR-IP.
    pub ce2_lrr: f64,
    /// Diffusion coefficient of the stresses.
    pub c_s: f64,
    /// Diffusion coefficient of epsilon.
    pub c_eps: f64,
    /// `C_mu` of the stabilising eddy viscosity `C_mu k^2/eps` used in the momentum equation.
    pub c_mu: f64,
}

impl Default for RsmCoefficients {
    fn default() -> Self {
        RsmCoefficients {
            c1: 1.7,
            c1s: 0.9,
            c2: 1.05,
            c3: 0.8,
            c3s: 0.65,
            c4: 0.625,
            c5: 0.2,
            c1_lrr: 1.8,
            c2_lrr: 0.6,
            ce1: 1.44,
            ce2_ssg: 1.83,
            ce2_lrr: 1.92,
            c_s: 0.22,
            c_eps: 0.18,
            c_mu: 0.09,
        }
    }
}

impl RsmCoefficients {
    /// `Ce2` of the pressure-strain variant.
    pub fn ce2(&self, ps: PressureStrain) -> f64 {
        match ps {
            PressureStrain::Ssg => self.ce2_ssg,
            PressureStrain::LrrIp => self.ce2_lrr,
        }
    }
}

/// Coefficient list for the settings schema.
pub fn rsm_infos(c: RsmCoefficients) -> Vec<CoefficientInfo> {
    let d = RsmCoefficients::default();
    let mk = |name, value, default, min, max, doc| CoefficientInfo {
        name,
        value,
        default,
        min,
        max,
        doc,
    };
    vec![
        mk("c1", c.c1, d.c1, 0.0, 10.0, "SSG C1 (anisotropy form, 1.7)"),
        mk("c1s", c.c1s, d.c1s, 0.0, 10.0, "SSG C1* (0.9)"),
        mk("c2", c.c2, d.c2, 0.0, 10.0, "SSG C2 (1.05)"),
        mk("c3", c.c3, d.c3, 0.0, 10.0, "SSG C3 (0.8)"),
        mk("c3s", c.c3s, d.c3s, 0.0, 10.0, "SSG C3* (0.65)"),
        mk("c4", c.c4, d.c4, 0.0, 10.0, "SSG C4 (0.625)"),
        mk("c5", c.c5, d.c5, 0.0, 10.0, "SSG C5 (0.2)"),
        mk("c1_lrr", c.c1_lrr, d.c1_lrr, 0.0, 10.0, "LRR-IP C1 (1.8)"),
        mk("c2_lrr", c.c2_lrr, d.c2_lrr, 0.0, 10.0, "LRR-IP C2 (0.6)"),
        mk("ce1", c.ce1, d.ce1, 0.5, 3.0, "epsilon production constant Ce1 (1.44)"),
        mk(
            "ce2_ssg",
            c.ce2_ssg,
            d.ce2_ssg,
            1.01,
            4.0,
            "epsilon destruction constant Ce2 with SSG (1.83)",
        ),
        mk(
            "ce2_lrr",
            c.ce2_lrr,
            d.ce2_lrr,
            1.01,
            4.0,
            "epsilon destruction constant Ce2 with LRR-IP (1.92)",
        ),
        mk(
            "c_s",
            c.c_s,
            d.c_s,
            0.0,
            2.0,
            "generalised gradient diffusion coefficient of the stresses (0.22)",
        ),
        mk(
            "c_eps",
            c.c_eps,
            d.c_eps,
            0.0,
            2.0,
            "generalised gradient diffusion coefficient of epsilon (0.18)",
        ),
        mk(
            "c_mu",
            c.c_mu,
            d.c_mu,
            0.01,
            0.5,
            "C_mu of the stabilising eddy viscosity C_mu k^2/eps in the momentum equation (0.09)",
        ),
    ]
}

/// Coefficient list for the schema (defaults).
pub fn rsm_coefficient_table() -> Vec<CoefficientInfo> {
    rsm_infos(RsmCoefficients::default())
}

/// Trace of a tensor.
pub fn trace(a: &Sym) -> f64 {
    a[0][0] + a[1][1] + a[2][2]
}

/// Production `P_ij = -R_ik dU_j/dx_k - R_jk dU_i/dx_k` for `g[i][j] = dU_i/dx_j`.
pub fn production(r: &Sym, g: &[[f64; 3]; 3]) -> Sym {
    let mut p = [[0.0; 3]; 3];
    for (i, row) in p.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in 0..3 {
                s -= r[i][k] * g[j][k] + r[j][k] * g[i][k];
            }
            *v = s;
        }
    }
    p
}

/// Pressure-strain correlation `Phi_ij` of the variant.
pub fn pressure_strain(ps: PressureStrain, cf: &RsmCoefficients, r: &Sym, eps: f64, g: &[[f64; 3]; 3]) -> Sym {
    let k = (0.5 * trace(r)).max(1e-30);
    let p = production(r, g);
    let pk = 0.5 * trace(&p);
    let mut phi = [[0.0; 3]; 3];
    match ps {
        PressureStrain::LrrIp => {
            for i in 0..3 {
                for j in 0..3 {
                    let dij = if i == j { 1.0 } else { 0.0 };
                    phi[i][j] = -cf.c1_lrr * eps / k * (r[i][j] - 2.0 / 3.0 * k * dij)
                        - cf.c2_lrr * (p[i][j] - 2.0 / 3.0 * pk * dij);
                }
            }
        }
        PressureStrain::Ssg => {
            let mut a = [[0.0; 3]; 3];
            let mut s = [[0.0; 3]; 3];
            let mut w = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    let dij = if i == j { 1.0 } else { 0.0 };
                    a[i][j] = r[i][j] / k - 2.0 / 3.0 * dij;
                    s[i][j] = 0.5 * (g[i][j] + g[j][i]);
                    w[i][j] = 0.5 * (g[i][j] - g[j][i]);
                }
            }
            let tr_s = trace(&s);
            let mut aa = 0.0;
            let mut a_s = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    aa += a[i][j] * a[i][j];
                    a_s += a[i][j] * s[i][j];
                }
            }
            for i in 0..3 {
                for j in 0..3 {
                    let dij = if i == j { 1.0 } else { 0.0 };
                    let mut aak = 0.0;
                    let mut as_t = 0.0;
                    let mut aw = 0.0;
                    for m in 0..3 {
                        aak += a[i][m] * a[m][j];
                        as_t += a[i][m] * s[j][m] + a[j][m] * s[i][m];
                        aw += a[i][m] * w[j][m] + a[j][m] * w[i][m];
                    }
                    let s_star = s[i][j] - tr_s / 3.0 * dij;
                    phi[i][j] = -(cf.c1 * eps + cf.c1s * pk) * a[i][j]
                        + cf.c2 * eps * (aak - aa / 3.0 * dij)
                        + (cf.c3 - cf.c3s * aa.sqrt()) * k * s_star
                        + cf.c4 * k * (as_t - 2.0 / 3.0 * a_s * dij)
                        + cf.c5 * k * aw;
                }
            }
        }
    }
    phi
}

/// Homogeneous rates `(dR_ij/dt, d eps/dt)` for a uniform gradient `g[i][j] = dU_i/dx_j`.
pub fn homogeneous_rates(ps: PressureStrain, cf: &RsmCoefficients, r: &Sym, eps: f64, g: &[[f64; 3]; 3]) -> (Sym, f64) {
    let k = (0.5 * trace(r)).max(1e-30);
    let p = production(r, g);
    let pk = 0.5 * trace(&p);
    let phi = pressure_strain(ps, cf, r, eps, g);
    let mut dr = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let dij = if i == j { 1.0 } else { 0.0 };
            dr[i][j] = p[i][j] + phi[i][j] - 2.0 / 3.0 * eps * dij;
        }
    }
    let deps = eps / k * (cf.ce1 * pk - cf.ce2(ps) * eps);
    (dr, deps)
}

/// Diagonal stabilisation `lambda_ij >= 0` of the semi-implicit update: the isotropisation rate of the pressure-strain model
/// and, on the diagonal, the dissipation `2/3 eps/R_ii`.
pub fn stabilisation(ps: PressureStrain, cf: &RsmCoefficients, r: &Sym, eps: f64, g: &[[f64; 3]; 3]) -> Sym {
    let k = (0.5 * trace(r)).max(1e-30);
    let p = production(r, g);
    let pk = 0.5 * trace(&p);
    let base = match ps {
        PressureStrain::LrrIp => cf.c1_lrr * eps / k,
        PressureStrain::Ssg => (cf.c1 * eps + cf.c1s * pk.max(0.0)) / k,
    };
    let mut l = [[base; 3]; 3];
    for (i, row) in l.iter_mut().enumerate() {
        row[i] += 2.0 / 3.0 * eps / r[i][i].abs().max(1e-30);
    }
    l
}

/// Smallest eigenvalue of a symmetric 3x3 tensor (closed trigonometric form).
pub fn min_eigenvalue(a: &Sym) -> f64 {
    let p1 = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
    if p1 == 0.0 {
        return a[0][0].min(a[1][1]).min(a[2][2]);
    }
    let q = trace(a) / 3.0;
    let p2 = (a[0][0] - q).powi(2) + (a[1][1] - q).powi(2) + (a[2][2] - q).powi(2) + 2.0 * p1;
    let p = (p2 / 6.0).sqrt();
    let mut b = *a;
    for (i, row) in b.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (*v - if i == j { q } else { 0.0 }) / p;
        }
    }
    let det = b[0][0] * (b[1][1] * b[2][2] - b[1][2] * b[2][1]) - b[0][1] * (b[1][0] * b[2][2] - b[1][2] * b[2][0])
        + b[0][2] * (b[1][0] * b[2][1] - b[1][1] * b[2][0]);
    let r = (det / 2.0).clamp(-1.0, 1.0);
    let phi = r.acos() / 3.0;
    q + 2.0 * p * (phi + 2.0 * std::f64::consts::PI / 3.0).cos()
}

const COMPS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (0, 2), (1, 2)];
const NAMES: [&str; 7] = ["Rxx", "Ryy", "Rzz", "Rxy", "Rxz", "Ryz", "epsilon"];

/// Divergence of a cell-valued tensor, `(d_j T_ij)` per cell, faces by linear interpolation. Translationally periodic faces use the
/// partner cell, other boundary faces the owner value (rotationally periodic faces too).
pub fn tensor_divergence(geom: &FvGeometry, t: &[Sym]) -> Vec<[f64; 3]> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let sf = mesh.face_area_vectors();
    let vol = mesh.cell_volumes();
    let n = geom.n_cells();
    let mut out = vec![[0.0; 3]; n];
    for f in 0..geom.n_internal {
        let (o, q) = (own[f], nei[f]);
        let w = geom.weights[f];
        for i in 0..3 {
            let mut fl = 0.0;
            for j in 0..3 {
                fl += (w * t[o][i][j] + (1.0 - w) * t[q][i][j]) * sf[f][j];
            }
            out[o][i] += fl / vol[o];
            out[q][i] -= fl / vol[q];
        }
    }
    let mut done = vec![false; mesh.n_faces()];
    for (pi, patch) in mesh.patches().iter().enumerate() {
        let Some(map) = &geom.periodic_partner[pi] else {
            continue;
        };
        if geom.periodic_rot[pi].is_some() {
            continue;
        }
        for (local, f) in patch.faces().enumerate() {
            let g = map[local];
            let (dn_o, dn_p) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
            let w = dn_p / (dn_o + dn_p);
            let (o, q) = (own[f], own[g]);
            for i in 0..3 {
                let mut fl = 0.0;
                for j in 0..3 {
                    fl += (w * t[o][i][j] + (1.0 - w) * t[q][i][j]) * sf[f][j];
                }
                out[o][i] += fl / vol[o];
            }
            done[f] = true;
        }
    }
    for f in geom.n_internal..mesh.n_faces() {
        if done[f] || geom.empty[f] {
            continue;
        }
        let o = own[f];
        for i in 0..3 {
            let mut fl = 0.0;
            for j in 0..3 {
                fl += t[o][i][j] * sf[f][j];
            }
            out[o][i] += fl / vol[o];
        }
    }
    out
}

/// The Reynolds stress transport model.
pub struct ReynoldsStress {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Laminar kinematic viscosity.
    pub nu: f64,
    /// Pressure-strain variant.
    pub ps: PressureStrain,
    /// Coefficients.
    pub coef: RsmCoefficients,
    /// Stress components `xx, yy, zz, xy, xz, yz`.
    pub r: [ScalarField; 6],
    /// Dissipation rate.
    pub eps: ScalarField,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Pseudo-time relaxation used when the model is swept without a time step.
    pub relax_turb: f64,
    /// Linear solver controls.
    pub solver: rcfd_linalg::SolveControl,
    nut: Vec<f64>,
    zeros: Vec<f64>,
    step_dt: Option<f64>,
    old: Vec<Vec<f64>>,
}

impl ReynoldsStress {
    /// New model with isotropic stresses `R = 2/3 k0 delta` and uniform `eps0`.
    pub fn new(geom: Arc<FvGeometry>, nu: f64, k0: f64, eps0: f64) -> Self {
        let n = geom.n_cells();
        let mk = |name: &str, v: f64| ScalarField::uniform(name, &geom, v);
        let r = [
            mk(NAMES[0], 2.0 / 3.0 * k0),
            mk(NAMES[1], 2.0 / 3.0 * k0),
            mk(NAMES[2], 2.0 / 3.0 * k0),
            mk(NAMES[3], 0.0),
            mk(NAMES[4], 0.0),
            mk(NAMES[5], 0.0),
        ];
        let coef = RsmCoefficients::default();
        let nut0 = coef.c_mu * k0 * k0 / eps0.max(1e-30);
        ReynoldsStress {
            nu,
            ps: PressureStrain::Ssg,
            coef,
            eps: mk(NAMES[6], eps0),
            r,
            convection: ConvScheme::Upwind,
            relax_turb: 0.7,
            solver: rcfd_linalg::SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-10,
                max_iter: 500,
            },
            nut: vec![nut0; n],
            zeros: vec![0.0; n],
            step_dt: None,
            old: Vec::new(),
            geom,
        }
    }

    /// Stress tensor of a cell.
    pub fn stress(&self, c: usize) -> Sym {
        let mut t = [[0.0; 3]; 3];
        for (m, &(i, j)) in COMPS.iter().enumerate() {
            t[i][j] = self.r[m].values[c];
            t[j][i] = t[i][j];
        }
        t
    }

    /// Set the stress tensor of a cell.
    pub fn set_stress(&mut self, c: usize, t: &Sym) {
        for (m, &(i, j)) in COMPS.iter().enumerate() {
            self.r[m].values[c] = t[i][j];
        }
    }

    /// Turbulent kinetic energy of a cell.
    pub fn k(&self, c: usize) -> f64 {
        0.5 * (self.r[0].values[c] + self.r[1].values[c] + self.r[2].values[c])
    }

    fn refresh_nut(&mut self) {
        for c in 0..self.geom.n_cells() {
            let k = self.k(c).max(0.0);
            self.nut[c] = self.coef.c_mu * k * k / self.eps.values[c].max(1e-30);
        }
    }

    /// Start a transient step: the next update adds the implicit-Euler time derivative.
    pub fn begin_step(&mut self, dt: f64) {
        self.step_dt = Some(dt);
        self.old = self.r.iter().map(|f| f.values.clone()).collect();
        self.old.push(self.eps.values.clone());
    }

    /// One sweep (see [`TurbulenceModel::update`]).
    pub fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes().to_vec();
        let gr = velocity_gradient(&g, u);
        let (ps, cf) = (self.ps, self.coef);
        // cell sources at the old iterate
        let mut rate = vec![[0.0; 7]; n];
        let mut lam = vec![[0.0; 7]; n];
        let mut kk = vec![0.0; n];
        let mut stress = Vec::with_capacity(n);
        for c in 0..n {
            let t = self.stress(c);
            let eps = self.eps.values[c];
            let gc: [[f64; 3]; 3] = [0, 1, 2].map(|i| gr[i][c]);
            let (dr, deps) = homogeneous_rates(ps, &cf, &t, eps, &gc);
            let l = stabilisation(ps, &cf, &t, eps, &gc);
            for (m, &(i, j)) in COMPS.iter().enumerate() {
                rate[c][m] = dr[i][j];
                lam[c][m] = l[i][j];
            }
            rate[c][6] = deps;
            lam[c][6] = cf.ce2(ps) * eps / (0.5 * trace(&t)).max(1e-30);
            kk[c] = (0.5 * trace(&t)).max(1e-30);
            stress.push(t);
        }
        // face diffusivities
        let kf_over_eps: Vec<f64> = (0..n).map(|c| kk[c] / self.eps.values[c].max(1e-30)).collect();
        let koe_f = face_interpolate(&g, &kf_over_eps);
        let rt: Vec<Vec<f64>> = (0..6).map(|m| self.r[m].values.clone()).collect();
        let r_f: Vec<Vec<f64>> = rt.iter().map(|v| face_interpolate(&g, v)).collect();
        let nf = g.mesh.n_faces();
        let mut rnn = vec![0.0; nf];
        for (f, v) in rnn.iter_mut().enumerate() {
            if g.empty[f] {
                continue;
            }
            let nrm = g.normal(f);
            let mut s = 0.0;
            for (m, &(i, j)) in COMPS.iter().enumerate() {
                let fac = if i == j { 1.0 } else { 2.0 };
                s += fac * nrm[i] * nrm[j] * r_f[m][f];
            }
            *v = s.max(0.0);
        }
        let gam_r: Vec<f64> = (0..nf).map(|f| self.nu + cf.c_s * koe_f[f] * rnn[f]).collect();
        let gam_e: Vec<f64> = (0..nf).map(|f| self.nu + cf.c_eps * koe_f[f] * rnn[f]).collect();

        let mut new_vals: Vec<Vec<f64>> = Vec::with_capacity(7);
        for m in 0..7 {
            let field = if m < 6 { &self.r[m] } else { &self.eps };
            let cells = field.values.clone();
            let bc = field.boundary_coeffs(&g);
            let mut mat = convection(&g, phi, &cells, &bc, self.convection, None);
            mat.add(&laplacian(&g, if m < 6 { &gam_r } else { &gam_e }, &bc, None));
            for c in 0..n {
                mat.a.diag[c] += vol[c] * lam[c][m];
                mat.b[c] += vol[c] * (rate[c][m] + lam[c][m] * cells[c]);
            }
            if let Some(dt) = self.step_dt {
                for c in 0..n {
                    mat.a.diag[c] += vol[c] / dt;
                    mat.b[c] += vol[c] * self.old[m][c] / dt;
                }
            } else {
                mat.relax(self.relax_turb, &cells);
            }
            let mut x = cells;
            mat.solve(&mut x, &self.solver);
            new_vals.push(x);
        }
        for m in 0..6 {
            self.r[m].values = std::mem::take(&mut new_vals[m]);
        }
        self.eps.values = std::mem::take(&mut new_vals[6]);
        for e in self.eps.values.iter_mut() {
            *e = e.max(1e-30);
        }
        let old_nut = self.nut.clone();
        self.refresh_nut();
        self.step_dt = None;
        (0..n)
            .map(|c| (self.nut[c] - old_nut[c]).abs() / (old_nut[c] + self.nu))
            .fold(0.0, f64::max)
    }

    /// Explicit momentum source (acceleration): `-d_j R_ij - d_j(nu_t d_j u_i)`, the second term removing the part that the
    /// solver already treats implicitly through `nu + nu_t`.
    pub fn momentum_source(&self, u: &VectorField) -> Vec<[f64; 3]> {
        let g = &self.geom;
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes();
        let stresses: Vec<Sym> = (0..n).map(|c| self.stress(c)).collect();
        let div = tensor_divergence(g, &stresses);
        let gamma = effective_viscosity(g, 0.0, &self.nut, &[]);
        let mut out: Vec<[f64; 3]> = div.iter().map(|d| [-d[0], -d[1], -d[2]]).collect();
        for k in 0..3 {
            if !g.active[k] {
                continue;
            }
            let bc = u.boundary_coeffs(g, k);
            let lap = laplacian(g, &gamma, &bc, None);
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let res = lap.residual(&cells);
            for c in 0..n {
                out[c][k] -= res[c] / vol[c];
            }
        }
        out
    }
}

/// Balanced-force body acceleration from cell values (face value: linear interpolation dotted with the area vector).
pub fn body_force_from_cells(geom: &FvGeometry, a: &[[f64; 3]]) -> BodyForce {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let face = (0..mesh.n_faces())
        .map(|f| {
            let v = if f < geom.n_internal {
                let w = geom.weights[f];
                [0, 1, 2].map(|k| w * a[own[f]][k] + (1.0 - w) * a[nei[f]][k])
            } else {
                a[own[f]]
            };
            v[0] * sf[f][0] + v[1] * sf[f][1] + v[2] * sf[f][2]
        })
        .collect();
    BodyForce { cell: a.to_vec(), face }
}

impl TurbulenceModel for ReynoldsStress {
    fn key(&self) -> &str {
        "rsm"
    }
    fn update(&mut self, u: &VectorField, phi: &[f64]) -> f64 {
        ReynoldsStress::update(self, u, phi)
    }
    fn nu_face(&self) -> Vec<f64> {
        effective_viscosity(&self.geom, self.nu, &self.nut, &[])
    }
    fn nut(&self) -> &[f64] {
        &self.nut
    }
    fn wall_distance(&self) -> &[f64] {
        &self.zeros
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        let mut v: Vec<(&'static str, &ScalarField)> = NAMES[..6].iter().copied().zip(self.r.iter()).collect();
        v.push((NAMES[6], &self.eps));
        v
    }
    fn scalar_mut(&mut self, name: &str) -> Option<&mut ScalarField> {
        if name == NAMES[6] {
            return Some(&mut self.eps);
        }
        NAMES[..6].iter().position(|n| *n == name).map(|i| &mut self.r[i])
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        Vec::new()
    }
    fn wall_faces(&self) -> Vec<usize> {
        Vec::new()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        rsm_infos(self.coef)
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> std::result::Result<(), String> {
        let infos = self.coefficients();
        let c = infos
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| format!("unknown coefficient '{name}'"))?;
        if !value.is_finite() || value < c.min || value > c.max {
            return Err(format!("{name} = {value} is outside [{}, {}]", c.min, c.max));
        }
        let cf = &mut self.coef;
        match name {
            "c1" => cf.c1 = value,
            "c1s" => cf.c1s = value,
            "c2" => cf.c2 = value,
            "c3" => cf.c3 = value,
            "c3s" => cf.c3s = value,
            "c4" => cf.c4 = value,
            "c5" => cf.c5 = value,
            "c1_lrr" => cf.c1_lrr = value,
            "c2_lrr" => cf.c2_lrr = value,
            "ce1" => cf.ce1 = value,
            "ce2_ssg" => cf.ce2_ssg = value,
            "ce2_lrr" => cf.ce2_lrr = value,
            "c_s" => cf.c_s = value,
            "c_eps" => cf.c_eps = value,
            _ => cf.c_mu = value,
        }
        self.refresh_nut();
        Ok(())
    }
    fn initialize_from_nut(&mut self, nut: &[f64]) {
        for (c, &v) in nut.iter().enumerate() {
            let k = self.k(c).max(1e-30);
            self.eps.values[c] = (self.coef.c_mu * k * k / v.max(1e-30)).max(1e-30);
        }
        self.refresh_nut();
    }
    fn begin_step(&mut self, dt: f64) {
        ReynoldsStress::begin_step(self, dt)
    }
    fn set_option(&mut self, name: &str, value: &str) -> std::result::Result<(), String> {
        match (name, value) {
            ("pressure_strain", "ssg") => self.ps = PressureStrain::Ssg,
            ("pressure_strain", "lrr_ip") => self.ps = PressureStrain::LrrIp,
            ("pressure_strain", v) => {
                return Err(format!("'{v}' is not a valid value of the option 'pressure_strain'"))
            }
            _ => return Err(format!("the model 'rsm' has no option '{name}'")),
        }
        Ok(())
    }
    fn momentum_source(&self, u: &VectorField) -> Option<Vec<[f64; 3]>> {
        Some(ReynoldsStress::momentum_source(self, u))
    }
}

/// Build from a context (isotropic start from `k0`, `eps0`).
pub fn new_rsm(ctx: &ModelContext) -> Result<ReynoldsStress> {
    if ctx.eps0 <= 0.0 || ctx.k0 <= 0.0 {
        return Err(Error::InvalidValue {
            path: "turbulence".into(),
            reason: "the Reynolds stress model needs a positive initial k and epsilon".into(),
        });
    }
    let mut m = ReynoldsStress::new(ctx.geom.clone(), ctx.nu, ctx.k0, ctx.eps0);
    m.convection = ctx.convection;
    m.relax_turb = ctx.relax_turb;
    Ok(m)
}
