//! Block-coupled pressure-velocity solver for steady incompressible flow (F-032).
//!
//! Instead of the segregated predictor/corrector of SIMPLE, every outer (Picard) iteration solves
//! the momentum and continuity equations for `(u, v[, w], p)` in one linear system
//!
//! ```text
//!   [ M_x   0   G_x ] [u]   [b_x]
//!   [  0   M_y  G_y ] [v] = [b_y]
//!   [ D_x  D_y   C  ] [p]   [b_p]
//! ```
//!
//! * `M_k` is the implicitly under-relaxed momentum matrix of component `k` (convection with the
//!   previous face flux, diffusion, deferred higher-order corrections), exactly as in SIMPLE;
//! * `G_k` is the implicit Green-Gauss pressure gradient `sum_f p_f S_f,k` (linear face pressure,
//!   zero-gradient boundary pressure);
//! * the continuity row of a cell is `sum_f phi_f = 0` with the Rhie-Chow face flux
//!   `phi_f = u_f . S_f + (rAU grad p)_f . S_f - rAU_f |S|^2/(S . d) (p_N - p_P)` where `rAU = V / a_P`,
//!   `u_f` and `(rAU grad p)_f` are linear interpolations and everything is implicit in `u` and `p`,
//!   so `D_k` holds the velocity part and `C` the (wide-stencil) pressure part;
//! * boundary faces carry the prescribed flux `u_b . S` (only closed domains with prescribed
//!   velocity on every boundary are supported), and the pressure level is fixed by replacing the
//!   continuity row of the reference cell with `p = p_ref`.
//!
//! Only the coefficients (`M_k`, `rAU`, the face flux used by convection and the deferred
//! corrections) are lagged between outer iterations. The linear system is solved by right-preconditioned
//! GMRES with the classic block-triangular "SIMPLE" preconditioner: ILU(0) of each `M_k`, and for the
//! pressure Schur complement `S = C - sum_k D_k diag(M_k)^-1 G_k` (a scaled pressure Poisson operator)
//! one aggregation-AMG V-cycle (`rcfd_linalg::Amg`).

use crate::convection::{convection, face_flux, ConvScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::pressure::pressure_bc;
use crate::simple::{normalised_residual_csr, IterationInfo, RunSummary};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::{gmres, Amg, AmgSettings, CsrMatrix, Ilu0, LinearOperator, Preconditioner, SolveControl};
use std::sync::Arc;

/// Settings of the coupled solver.
#[derive(Debug, Clone)]
pub struct CoupledSettings {
    /// Kinematic viscosity.
    pub nu: f64,
    /// Implicit under-relaxation of the momentum equations (the pressure is not relaxed).
    pub alpha_u: f64,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Reference cell and value that fix the pressure level.
    pub p_ref: (usize, f64),
    /// GMRES controls of the coupled linear system.
    pub linear: SolveControl,
    /// GMRES restart length.
    pub restart: usize,
    /// Use the AMG V-cycle for the pressure Schur complement (otherwise the Jacobi-scaled identity).
    pub amg: bool,
}

impl CoupledSettings {
    /// Defaults: `alpha_u = 0.9`, relative linear tolerance 1e-3.
    pub fn new(nu: f64) -> Self {
        CoupledSettings {
            nu,
            alpha_u: 0.9,
            convection: ConvScheme::Upwind,
            p_ref: (0, 0.0),
            linear: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-3,
                max_iter: 400,
            },
            restart: 40,
            amg: true,
        }
    }
}

/// Coupled system blocks.
struct Blocks {
    n: usize,
    nk: usize,
    m: Vec<CsrMatrix>,
    g: Vec<CsrMatrix>,
    d: Vec<CsrMatrix>,
    c: CsrMatrix,
}

impl LinearOperator for Blocks {
    fn size(&self) -> usize {
        (self.nk + 1) * self.n
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) {
        let n = self.n;
        let p = &x[self.nk * n..];
        let mut tmp = vec![0.0; n];
        let mut yp = vec![0.0; n];
        self.c.matvec(p, &mut yp);
        for k in 0..self.nk {
            self.m[k].matvec(&x[k * n..(k + 1) * n], &mut y[k * n..(k + 1) * n]);
            self.g[k].matvec(p, &mut tmp);
            for i in 0..n {
                y[k * n + i] += tmp[i];
            }
            self.d[k].matvec(&x[k * n..(k + 1) * n], &mut tmp);
            for i in 0..n {
                yp[i] += tmp[i];
            }
        }
        y[self.nk * n..].copy_from_slice(&yp);
    }
}

/// Block-triangular preconditioner.
struct BlockPrecond<'a> {
    blocks: &'a Blocks,
    ilu: Vec<Ilu0>,
    schur: Option<Amg>,
    schur_diag_inv: Vec<f64>,
}

impl Preconditioner for BlockPrecond<'_> {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let (n, nk) = (self.blocks.n, self.blocks.nk);
        let mut y = vec![vec![0.0; n]; nk];
        for k in 0..nk {
            self.ilu[k].apply(&r[k * n..(k + 1) * n], &mut y[k]);
        }
        let mut s = r[nk * n..].to_vec();
        let mut tmp = vec![0.0; n];
        for k in 0..nk {
            self.blocks.d[k].matvec(&y[k], &mut tmp);
            for i in 0..n {
                s[i] -= tmp[i];
            }
        }
        let mut dp = vec![0.0; n];
        match &self.schur {
            Some(amg) => amg.vcycle(&s, &mut dp),
            None => {
                for i in 0..n {
                    dp[i] = s[i] * self.schur_diag_inv[i];
                }
            }
        }
        for k in 0..nk {
            self.blocks.g[k].matvec(&dp, &mut tmp);
            let mut corr = vec![0.0; n];
            self.ilu[k].apply(&tmp, &mut corr);
            for i in 0..n {
                z[k * n + i] = y[k][i] - corr[i];
            }
        }
        z[nk * n..].copy_from_slice(&dp);
    }
}

/// Steady coupled solver state.
pub struct CoupledSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: CoupledSettings,
    /// Velocity field with its boundary conditions.
    pub u: VectorField,
    /// Kinematic pressure.
    pub p: ScalarField,
    /// Face flux (positive out of the owner).
    pub phi: Vec<f64>,
    /// Diagnostics of every outer iteration (`p_iters` holds the GMRES iterations).
    pub history: Vec<IterationInfo>,
    /// Effective kinematic viscosity at every face; `None` uses `settings.nu`.
    pub nu_face: Option<Vec<f64>>,
    /// Total GMRES iterations so far.
    pub linear_iterations: usize,
    iteration: usize,
    comps: Vec<usize>,
    /// Pressure-gradient operators `G_k` (unscaled by volume), constant per mesh.
    g: Vec<CsrMatrix>,
    /// Velocity part of the continuity rows, constant per mesh (reference row removed).
    d: Vec<CsrMatrix>,
    /// Cell pressure-gradient rows `G_k / V` as (column, coefficient) lists.
    gcell: Vec<Vec<Vec<(usize, f64)>>>,
}

impl CoupledSolver {
    /// Create a solver. The domain must be closed: the velocity must be prescribed on every
    /// boundary face (walls, moving walls, fixed-velocity inlets that balance the outflow are
    /// the caller's responsibility), with no periodic patches.
    pub fn new(geom: Arc<FvGeometry>, u: VectorField, p: ScalarField, settings: CoupledSettings) -> Result<Self> {
        let pbc = pressure_bc(&geom, &u, &p);
        let open = (0..pbc.fixed_u.len()).any(|i| !geom.empty[geom.n_internal + i] && !pbc.fixed_u[i]);
        if open || pbc.coupled.iter().any(Option::is_some) {
            return Err(Error::InvalidValue {
                path: "solver.algorithm".into(),
                reason: "the coupled solver needs the velocity prescribed on every boundary (closed domain, no periodic patches)".into(),
            });
        }
        let n = geom.n_cells();
        if settings.p_ref.0 >= n {
            return Err(Error::InvalidValue {
                path: "solver.p_ref_cell".into(),
                reason: format!("cell {} does not exist (the mesh has {n} cells)", settings.p_ref.0),
            });
        }
        let comps: Vec<usize> = (0..3).filter(|&k| geom.active[k]).collect();
        let mesh = &geom.mesh;
        let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
        let vol = mesh.cell_volumes();
        let mut g_t: Vec<Vec<(usize, usize, f64)>> = vec![Vec::new(); comps.len()];
        let mut d_t: Vec<Vec<(usize, usize, f64)>> = vec![Vec::new(); comps.len()];
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            for (ci, &k) in comps.iter().enumerate() {
                let s = sf[f][k];
                if f < geom.n_internal {
                    let (o, nb, w) = (own[f], nei[f], geom.weights[f]);
                    // pressure gradient: outward face integral for each adjacent cell
                    g_t[ci].extend([
                        (o, o, s * w),
                        (o, nb, s * (1.0 - w)),
                        (nb, o, -s * w),
                        (nb, nb, -s * (1.0 - w)),
                    ]);
                    // continuity: u_f . S_f with linear interpolation
                    d_t[ci].extend([
                        (o, o, s * w),
                        (o, nb, s * (1.0 - w)),
                        (nb, o, -s * w),
                        (nb, nb, -s * (1.0 - w)),
                    ]);
                } else {
                    // zero-gradient boundary pressure; the boundary flux is prescribed
                    g_t[ci].push((own[f], own[f], s));
                }
            }
        }
        // the reference cell's continuity row is replaced by p = p_ref
        let ref_cell = settings.p_ref.0;
        for t in d_t.iter_mut() {
            t.retain(|&(r, _, _)| r != ref_cell);
        }
        let g: Vec<CsrMatrix> = g_t.into_iter().map(|t| CsrMatrix::from_triplets(n, t)).collect();
        let d: Vec<CsrMatrix> = d_t.into_iter().map(|t| CsrMatrix::from_triplets(n, t)).collect();
        let gcell: Vec<Vec<Vec<(usize, f64)>>> = g
            .iter()
            .map(|gk| {
                (0..n)
                    .map(|c| {
                        (gk.row_ptr[c]..gk.row_ptr[c + 1])
                            .map(|j| (gk.col[j], gk.val[j] / vol[c]))
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        Ok(CoupledSolver {
            geom,
            settings,
            u,
            p,
            phi,
            history: Vec::new(),
            nu_face: None,
            linear_iterations: 0,
            iteration: 0,
            comps,
            g,
            d,
            gcell,
        })
    }

    /// Iterations performed.
    pub fn iterations(&self) -> usize {
        self.iteration
    }

    /// Run one outer iteration.
    pub fn iterate(&mut self) -> IterationInfo {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let nk = self.comps.len();
        let vol = mesh.cell_volumes();
        let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
        let s = self.settings.clone();
        self.iteration += 1;
        let u_old = self.u.values.clone();
        let p_old = self.p.values.clone();
        let gamma = match &self.nu_face {
            Some(v) => v.clone(),
            None => uniform_gamma(&g, s.nu),
        };
        let ub = self.u.boundary_values(&g);

        // ---- momentum blocks -------------------------------------------------
        let mut m_csr = Vec::with_capacity(nk);
        let mut b_vec: Vec<Vec<f64>> = Vec::with_capacity(nk);
        let mut a_p = vec![0.0; n];
        let mut res_u: f64 = 0.0;
        for (ci, &k) in self.comps.iter().enumerate() {
            let cells: Vec<f64> = u_old.iter().map(|v| v[k]).collect();
            let bc = self.u.boundary_coeffs(&g, k);
            let bvals: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            let grad = s.convection.needs_gradient().then(|| least_squares(&g, &cells, &bvals));
            let mut m = convection(&g, &self.phi, &cells, &bc, s.convection, grad.as_deref());
            m.add(&laplacian(&g, &gamma, &bc, None));
            m.relax(s.alpha_u, &cells);
            for c in 0..n {
                a_p[c] += m.a.diag[c] / nk as f64;
            }
            let csr = m.a.to_csr();
            // momentum residual of the current state (with the old pressure gradient)
            let mut gp = vec![0.0; n];
            self.g[ci].matvec(&p_old, &mut gp);
            let rhs: Vec<f64> = (0..n).map(|c| m.b[c] - gp[c]).collect();
            res_u = res_u.max(normalised_residual_csr(&csr, &rhs, &cells));
            m_csr.push(csr);
            b_vec.push(m.b);
        }
        let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();

        // ---- continuity block (Rhie-Chow, implicit in u and p) ------------------
        let mut c_t: Vec<(usize, usize, f64)> = Vec::new();
        for f in 0..g.n_internal {
            if g.empty[f] {
                continue;
            }
            let (o, nb, w) = (own[f], nei[f], g.weights[f]);
            let rf = w * r_au[o] + (1.0 - w) * r_au[nb];
            let lap = rf * g.lap_coeff[f];
            let mut coef: Vec<(usize, f64)> = vec![(o, lap), (nb, -lap)];
            for (ci, &k) in self.comps.iter().enumerate() {
                let sk = sf[f][k];
                for &(col, v) in &self.gcell[ci][o] {
                    coef.push((col, sk * w * r_au[o] * v));
                }
                for &(col, v) in &self.gcell[ci][nb] {
                    coef.push((col, sk * (1.0 - w) * r_au[nb] * v));
                }
            }
            for (col, v) in coef {
                if o != s.p_ref.0 {
                    c_t.push((o, col, v));
                }
                if nb != s.p_ref.0 {
                    c_t.push((nb, col, -v));
                }
            }
        }
        c_t.push((s.p_ref.0, s.p_ref.0, 1.0));
        let c_mat = CsrMatrix::from_triplets(n, c_t);
        let mut b_p = vec![0.0; n];
        for f in g.n_internal..mesh.n_faces() {
            if !g.empty[f] {
                b_p[own[f]] -= vec3::dot(ub[f - g.n_internal], sf[f]);
            }
        }
        b_p[s.p_ref.0] = s.p_ref.1;

        let blocks = Blocks {
            n,
            nk,
            m: m_csr,
            g: self.g.clone(),
            d: self.d.clone(),
            c: c_mat,
        };

        // ---- state, residual, preconditioner, solve -------------------------------
        let mut x = vec![0.0; (nk + 1) * n];
        let mut rhs = vec![0.0; (nk + 1) * n];
        for (ci, &k) in self.comps.iter().enumerate() {
            for c in 0..n {
                x[ci * n + c] = u_old[c][k];
                rhs[ci * n + c] = b_vec[ci][c];
            }
        }
        x[nk * n..].copy_from_slice(&p_old);
        rhs[nk * n..].copy_from_slice(&b_p);
        let mut r0 = vec![0.0; nk * n + n];
        blocks.apply(&x, &mut r0);
        let res_p = {
            let num: f64 = (0..n).map(|c| (rhs[nk * n + c] - r0[nk * n + c]).abs()).sum();
            let den: f64 = 0.5 * self.phi.iter().map(|v| v.abs()).sum::<f64>();
            num / (den + 1e-300)
        };
        // Schur complement approximation S = C - sum_k D_k diag(M_k)^-1 G_k
        let mut s_t: Vec<(usize, usize, f64)> = Vec::new();
        for r in 0..n {
            for j in blocks.c.row_ptr[r]..blocks.c.row_ptr[r + 1] {
                s_t.push((r, blocks.c.col[j], blocks.c.val[j]));
            }
        }
        for k in 0..nk {
            let diag = blocks.m[k].diag();
            let mut gs = blocks.g[k].clone();
            for r in 0..n {
                for j in gs.row_ptr[r]..gs.row_ptr[r + 1] {
                    gs.val[j] /= diag[r];
                }
            }
            let prod = blocks.d[k].matmul(&gs);
            for r in 0..n {
                for j in prod.row_ptr[r]..prod.row_ptr[r + 1] {
                    s_t.push((r, prod.col[j], -prod.val[j]));
                }
            }
        }
        let s_mat = CsrMatrix::from_triplets(n, s_t);
        let schur = s.amg.then(|| Amg::new(&s_mat, AmgSettings::default()));
        let schur_diag_inv: Vec<f64> = s_mat
            .diag()
            .iter()
            .map(|&d| if d.abs() > 1e-300 { 1.0 / d } else { 1.0 })
            .collect();
        let pre = BlockPrecond {
            ilu: blocks.m.iter().map(Ilu0::new).collect(),
            blocks: &blocks,
            schur,
            schur_diag_inv,
        };
        let stats = gmres(&blocks, &rhs, &mut x, &pre, s.restart, &s.linear);
        self.linear_iterations += stats.iterations;

        // ---- update ----------------------------------------------------------------
        let mut max_du: f64 = 0.0;
        for (ci, &k) in self.comps.iter().enumerate() {
            for c in 0..n {
                let un = x[ci * n + c];
                max_du = max_du.max((un - u_old[c][k]).abs());
                self.u.values[c][k] = un;
            }
        }
        self.p.values.copy_from_slice(&x[nk * n..]);
        // face flux consistent with the continuity rows that were solved
        let pn = &self.p.values;
        let mut gradp: Vec<Vec3> = vec![[0.0; 3]; n];
        for (ci, &k) in self.comps.iter().enumerate() {
            for c in 0..n {
                gradp[c][k] = self.gcell[ci][c].iter().map(|&(col, v)| v * pn[col]).sum();
            }
        }
        let mut phi = vec![0.0; mesh.n_faces()];
        for f in 0..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            if f < g.n_internal {
                let (o, nb, w) = (own[f], nei[f], g.weights[f]);
                let rf = w * r_au[o] + (1.0 - w) * r_au[nb];
                let mut v = -rf * g.lap_coeff[f] * (pn[nb] - pn[o]);
                for &k in &self.comps {
                    let uf = w * self.u.values[o][k] + (1.0 - w) * self.u.values[nb][k];
                    let gf = w * r_au[o] * gradp[o][k] + (1.0 - w) * r_au[nb] * gradp[nb][k];
                    v += sf[f][k] * (uf + gf);
                }
                phi[f] = v;
            } else {
                phi[f] = vec3::dot(ub[f - g.n_internal], sf[f]);
            }
        }
        self.phi = phi;
        let mut cont: f64 = 0.0;
        for c in 0..n {
            let mut sum = 0.0;
            for &f in mesh.cell_faces(c) {
                if g.empty[f] {
                    continue;
                }
                sum += if f >= g.n_internal || own[f] == c {
                    self.phi[f]
                } else {
                    -self.phi[f]
                };
            }
            if c != s.p_ref.0 {
                cont = cont.max(sum.abs());
            }
        }
        let area_scale = sf.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
        let info = IterationInfo {
            iteration: self.iteration,
            max_du,
            continuity: cont / area_scale.max(1e-300),
            p_iters: stats.iterations,
            res_u,
            res_p,
        };
        self.history.push(info);
        info
    }

    /// Iterate until the largest velocity change falls below `tol` or `max_iter` is reached.
    pub fn run(&mut self, max_iter: usize, tol: f64) -> RunSummary {
        let mut last = IterationInfo {
            iteration: 0,
            max_du: f64::INFINITY,
            continuity: f64::INFINITY,
            p_iters: 0,
            res_u: f64::INFINITY,
            res_p: f64::INFINITY,
        };
        for _ in 0..max_iter {
            last = self.iterate();
            if last.max_du < tol && last.continuity < 1e-6 {
                return RunSummary {
                    iterations: last.iteration,
                    converged: true,
                    last,
                };
            }
        }
        RunSummary {
            iterations: last.iteration,
            converged: false,
            last,
        }
    }
}
