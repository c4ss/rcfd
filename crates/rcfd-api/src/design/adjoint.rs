//! Shape sensitivities of steady laminar SIMPLE/SIMPLEC solutions by a fixed-point discrete adjoint (ADR 0091, F-137), with
//! free-form-deformation (FFD) mesh morphing driven by a few shape parameters.
//!
//! Formulation (ADR 0091; label: fixed-point discrete adjoint with a finite-difference-assembled dense Jacobian of the
//! one-iteration SIMPLE/SIMPLEC map, small meshes only). The state `X` is everything [`SimpleState`] carries (active velocity
//! components, pressure, face flux on non-empty faces) and one outer iteration is a map `X' = G(X; mesh(a))`. At a converged
//! fixed point `(I - G_X)^-1 G_a = -R_X^-1 R_a` for the discrete residual `R`, whatever the relaxation, so
//! `dJ/da = J_a + lambda^T G_a` with `(I - G_X)^T lambda = J_X^T` is the sensitivity of the discrete solution. `G_X`, `J_X`,
//! `G_a`, `J_a` are central differences of the solver's own iteration; the transposed system is solved by a dense LU. Cost:
//! `2 N` one-iteration evaluations plus `O(N^3)`, independent of the number of parameters.
//!
//! Accurate for: the derivative of the discrete objective of the converged discrete solution on the given mesh and settings
//! with respect to the morph parameters (interior-node motion included), when the inner linear solves are converged to
//! round-off and no face flux sits on the upwind (or limiter) switch. Laminar only: solvers with `nu_face` (turbulence) are
//! refused, so no frozen-turbulence approximation is ever made.

use rcfd_core::vec3::Vec3;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver, SimpleState};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{channel, Grading};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Free-form deformation of the `(x, y)` plane: a Bernstein lattice of degree `deg` over the box `lo..hi`; every shape
/// parameter moves one control point in one direction. Points outside the box do not move; `z` is never moved.
#[derive(Debug, Clone)]
pub struct Ffd {
    /// Lower corner of the box `(x, y)`.
    pub lo: [f64; 2],
    /// Upper corner of the box `(x, y)`.
    pub hi: [f64; 2],
    /// Bernstein degree per direction (control points `deg + 1`).
    pub deg: [usize; 2],
    /// Per parameter: control point `(i, j)` and the direction it moves (0 = x, 1 = y).
    pub moves: Vec<(usize, usize, usize)>,
}

fn bernstein(n: usize, i: usize, t: f64) -> f64 {
    let mut c = 1.0;
    for k in 0..i {
        c = c * (n - k) as f64 / (k + 1) as f64;
    }
    c * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

impl Ffd {
    /// Point positions for the parameter values `a` (one per entry of `moves`).
    pub fn displace(&self, points: &[Vec3], a: &[f64]) -> Vec<Vec3> {
        points
            .iter()
            .map(|p| {
                let s = [0, 1].map(|d| (p[d] - self.lo[d]) / (self.hi[d] - self.lo[d]));
                let mut q = *p;
                if s.iter().all(|t| (0.0..=1.0).contains(t)) {
                    for (&(i, j, dir), v) in self.moves.iter().zip(a) {
                        q[dir] += v * bernstein(self.deg[0], i, s[0]) * bernstein(self.deg[1], j, s[1]);
                    }
                }
                q
            })
            .collect()
    }
}

/// The ADR 0091 duct: length 4, height 1, 40 x 10 cells, parabolic inlet of mean velocity 1, `Re = U H / nu = 20`, outlet at
/// zero pressure, no-slip walls; FFD box `x 1..3`, `y 0..1`, degree (4, 1), parameters = `y` moves of the bottom control points
/// 1, 2, 3 (a positive value lifts the lower wall into the duct).
#[derive(Debug, Clone)]
pub struct Duct {
    /// Undeformed mesh.
    pub base: Arc<Mesh>,
    /// Morph.
    pub ffd: Ffd,
    /// Kinematic viscosity.
    pub nu: f64,
}

impl Default for Duct {
    fn default() -> Self {
        Duct::new([40, 10])
    }
}

impl Duct {
    /// The duct with `n = [nx, ny]` cells.
    pub fn new(n: [usize; 2]) -> Self {
        let base = channel([n[0], n[1], 1], [4.0, 1.0, 0.1], [Grading::Uniform; 3], true).expect("channel mesh");
        Duct {
            base: Arc::new(base),
            ffd: Ffd {
                lo: [1.0, 0.0],
                hi: [3.0, 1.0],
                deg: [4, 1],
                moves: vec![(1, 0, 1), (2, 0, 1), (3, 0, 1)],
            },
            nu: 0.05,
        }
    }

    /// Settings: SIMPLEC defaults with inner linear solves to round-off (smooth one-iteration map).
    pub fn settings(&self) -> SimpleSettings {
        let mut st = SimpleSettings::new(self.nu, Coupling::Simplec);
        st.momentum_solver = SolveControl {
            tol_abs: 1e-15,
            tol_rel: 0.0,
            max_iter: 5000,
        };
        st.pressure_solver = SolveControl {
            tol_abs: 1e-15,
            tol_rel: 0.0,
            max_iter: 20000,
        };
        st
    }

    /// A fresh solver (fields at rest) on the mesh morphed by `a`.
    pub fn solver(&self, a: &[f64]) -> Result<SimpleSolver, String> {
        let pts = self.ffd.displace(self.base.points(), a);
        let mesh = rcfd_mesh::motion::moved_mesh(&self.base, pts).map_err(|e| e.to_string())?;
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).map_err(|e| e.to_string())?);
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        let patch = geom.mesh.patch("inlet").ok_or("no inlet")?.clone();
        let cf = geom.mesh.face_centres();
        let prof: Vec<Vec3> = patch
            .faces()
            .map(|f| {
                let s = cf[f][1];
                [6.0 * s * (1.0 - s), 0.0, 0.0]
            })
            .collect();
        let e = |r: rcfd_core::Result<()>| r.map_err(|e| e.to_string());
        e(u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof)))))?;
        e(u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3]))))?;
        e(u.set_bc(&geom, "outlet", Box::new(ZeroGradient)))?;
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        e(p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))))?;
        Ok(SimpleSolver::new(geom, u, p, self.settings()))
    }
}

fn patch_mean(s: &SimpleSolver, pb: &[f64], patch: &str) -> f64 {
    let g = &s.geom;
    let pa = g.mesh.patch(patch).expect("patch");
    let (mut num, mut den) = (0.0, 0.0);
    for f in pa.faces() {
        let a = g.mesh.face_area(f);
        num += a * pb[f - g.n_internal];
        den += a;
    }
    num / den
}

/// Objective J1: area-mean boundary pressure on `inlet` minus that on `outlet` (kinematic).
pub fn pressure_drop(s: &SimpleSolver) -> f64 {
    let pb = s.p.boundary_values(&s.geom);
    patch_mean(s, &pb, "inlet") - patch_mean(s, &pb, "outlet")
}

/// Objective J2: `x` force of the fluid on the patch `walls` (pressure plus viscous, kinematic, per the 0.1 depth).
pub fn wall_drag(s: &SimpleSolver) -> f64 {
    let pb = s.p.boundary_values(&s.geom);
    let (f, _) = rcfd_post::integrals::wall_force_and_moment(&s.geom, "walls", &s.u, &pb, s.settings.nu, 1.0, [0.0; 3])
        .expect("walls patch");
    f.total()[0]
}

/// Outcome of [`solve_primal`].
#[derive(Debug, Clone, Copy)]
pub struct PrimalRun {
    /// Iterations performed.
    pub iterations: usize,
    /// Smallest `max(res_u, res_p)` seen.
    pub best_res: f64,
    /// Final `max(res_u, res_p)`.
    pub last_res: f64,
}

/// Iterate to stagnation: until `max(res_u, res_p) <= tol`, or until the best residual has not improved by a factor 2 in
/// `patience` iterations, or `max_iter`. Errors on a non-finite residual.
pub fn solve_primal(s: &mut SimpleSolver, tol: f64, patience: usize, max_iter: usize) -> Result<PrimalRun, String> {
    let (mut best, mut best_at, mut last) = (f64::INFINITY, 0, f64::INFINITY);
    for it in 1..=max_iter {
        let info = s.iterate();
        last = info.res_u.max(info.res_p);
        if !last.is_finite() {
            return Err(format!("primal diverged at iteration {it}"));
        }
        if last < 0.5 * best {
            best_at = it;
        }
        best = best.min(last);
        if last <= tol || it - best_at >= patience {
            return Ok(PrimalRun {
                iterations: it,
                best_res: best,
                last_res: last,
            });
        }
    }
    Ok(PrimalRun {
        iterations: max_iter,
        best_res: best,
        last_res: last,
    })
}

/// Controls of [`adjoint_gradient`].
#[derive(Debug, Clone, Copy)]
pub struct AdjointOptions {
    /// Central-difference step of the state, relative to the largest magnitude of its block (u, p or phi).
    pub state_step: f64,
    /// Central-difference step of the shape parameters.
    pub param_step: f64,
    /// Largest state accepted (the dense matrix takes `8 N^2` bytes).
    pub max_state: usize,
}

impl Default for AdjointOptions {
    fn default() -> Self {
        AdjointOptions {
            state_step: 1e-6,
            param_step: 1e-6,
            max_state: 4000,
        }
    }
}

/// Values and gradients of the objectives at a converged state.
#[derive(Debug, Clone)]
pub struct Sensitivity {
    /// Objective values.
    pub values: Vec<f64>,
    /// Per objective: derivative with respect to each shape parameter.
    pub gradients: Vec<Vec<f64>>,
    /// State unknowns.
    pub n_state: usize,
    /// `max |G(X*) - X*|` (the derivation assumes a fixed point).
    pub fixed_point_residual: f64,
}

/// An objective of the converged state.
pub type Objective<'a> = &'a dyn Fn(&SimpleSolver) -> f64;

fn check_supported(s: &SimpleSolver) -> Result<(), String> {
    let hidden = [
        (
            s.nu_face.is_some(),
            "nu_face (turbulent viscosity; only laminar solutions are supported)",
        ),
        (s.body_force.is_some(), "body_force"),
        (s.driver.is_some(), "driver"),
        (s.pseudo_control.is_some(), "pseudo_control"),
        (s.face_jump.is_some(), "face_jump"),
    ];
    match hidden.iter().find(|h| h.0) {
        Some(h) => Err(format!("adjoint: unsupported solver option {}", h.1)),
        None => Ok(()),
    }
}

/// A state unknown: block (0 = u, 1 = p, 2 = phi), cell or face, component.
type Slot = (u8, usize, usize);

fn state_index(s: &SimpleSolver) -> Vec<Slot> {
    let g = &s.geom;
    let mut idx = Vec::new();
    for c in 0..g.n_cells() {
        for k in (0..3).filter(|&k| g.active[k]) {
            idx.push((0, c, k));
        }
    }
    idx.extend((0..g.n_cells()).map(|c| (1, c, 0)));
    idx.extend((0..g.mesh.n_faces()).filter(|&f| !g.empty[f]).map(|f| (2, f, 0)));
    idx
}

fn slot(st: &mut SimpleState, (b, i, k): Slot) -> &mut f64 {
    match b {
        0 => &mut st.u[i][k],
        1 => &mut st.p[i],
        _ => &mut st.phi[i],
    }
}

fn pack(st: &SimpleState, idx: &[Slot]) -> Vec<f64> {
    idx.iter()
        .map(|&(b, i, k)| match b {
            0 => st.u[i][k],
            1 => st.p[i],
            _ => st.phi[i],
        })
        .collect()
}

/// One outer iteration from `st` (packed result), and the objectives at `st`.
fn step(
    s: &mut SimpleSolver,
    st: &SimpleState,
    idx: &[Slot],
    objectives: &[Objective<'_>],
) -> Result<(Vec<f64>, Vec<f64>), String> {
    s.restore(st).map_err(|e| e.to_string())?;
    let j = objectives.iter().map(|o| o(s)).collect();
    s.iterate();
    Ok((pack(&s.snapshot(), idx), j))
}

/// LU with partial pivoting of the row-major `n x n` matrix `a` in place; returns the row swaps.
fn lu_factor(a: &mut [f64], n: usize) -> Result<Vec<usize>, String> {
    let mut piv = Vec::with_capacity(n);
    for k in 0..n {
        let p = (k..n)
            .max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))
            .unwrap_or(k);
        if a[p * n + k] == 0.0 || !a[p * n + k].is_finite() {
            return Err(format!("adjoint: singular or non-finite Jacobian at column {k}"));
        }
        piv.push(p);
        if p != k {
            for c in 0..n {
                a.swap(k * n + c, p * n + c);
            }
        }
        let (top, rest) = a.split_at_mut((k + 1) * n);
        let rowk = &top[k * n..];
        for row in rest.chunks_mut(n) {
            let l = row[k] / rowk[k];
            row[k] = l;
            if l != 0.0 {
                for (x, y) in row[k + 1..].iter_mut().zip(&rowk[k + 1..]) {
                    *x -= l * y;
                }
            }
        }
    }
    Ok(piv)
}

/// Solve with the factors of [`lu_factor`].
fn lu_solve(a: &[f64], n: usize, piv: &[usize], b: &mut [f64]) {
    for (k, &p) in piv.iter().enumerate() {
        b.swap(k, p);
    }
    for i in 0..n {
        let s: f64 = a[i * n..i * n + i].iter().zip(&b[..i]).map(|(x, y)| x * y).sum();
        b[i] -= s;
    }
    for i in (0..n).rev() {
        let s: f64 = a[i * n + i + 1..(i + 1) * n]
            .iter()
            .zip(&b[i + 1..])
            .map(|(x, y)| x * y)
            .sum();
        b[i] = (b[i] - s) / a[i * n + i];
    }
}

/// Gradients of `objectives` with respect to the shape parameters at `a`, given the converged state `converged` of the primal
/// on the mesh of `a`. `build(a)` returns a solver on the mesh morphed by `a` with the case's boundary conditions and settings
/// (its fields are replaced by the converged state). See the module documentation for the formulation and its validity.
pub fn adjoint_gradient(
    build: &dyn Fn(&[f64]) -> Result<SimpleSolver, String>,
    a: &[f64],
    converged: &SimpleState,
    objectives: &[Objective<'_>],
    opts: &AdjointOptions,
) -> Result<Sensitivity, String> {
    let mut s = build(a)?;
    check_supported(&s)?;
    let idx = state_index(&s);
    let n = idx.len();
    if n > opts.max_state {
        return Err(format!(
            "adjoint: {n} state unknowns exceed max_state {} (dense Jacobian)",
            opts.max_state
        ));
    }
    let x = pack(converged, &idx);
    let (g0, values) = step(&mut s, converged, &idx, objectives)?;
    let fixed_point_residual = g0.iter().zip(&x).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max);
    let mut scale = [0.0f64; 3];
    for (&(b, _, _), v) in idx.iter().zip(&x) {
        scale[b as usize] = scale[b as usize].max(v.abs());
    }
    // (I - G_X)^T row-major: row j = e_j - dG/dX_j; right-hand sides J_X
    let mut at = vec![0.0; n * n];
    let mut rhs = vec![vec![0.0; n]; objectives.len()];
    for (j, &sj) in idx.iter().enumerate() {
        let h = opts.state_step * scale[sj.0 as usize].max(1e-30);
        let mut st = converged.clone();
        *slot(&mut st, sj) = x[j] + h;
        let (gp, jp) = step(&mut s, &st, &idx, objectives)?;
        *slot(&mut st, sj) = x[j] - h;
        let (gm, jm) = step(&mut s, &st, &idx, objectives)?;
        let row = &mut at[j * n..(j + 1) * n];
        for ((r, p), m) in row.iter_mut().zip(&gp).zip(&gm) {
            *r = -(p - m) / (2.0 * h);
        }
        row[j] += 1.0;
        for (r, (p, m)) in rhs.iter_mut().zip(jp.iter().zip(&jm)) {
            r[j] = (p - m) / (2.0 * h);
        }
    }
    let piv = lu_factor(&mut at, n)?;
    for r in rhs.iter_mut() {
        lu_solve(&at, n, &piv, r);
    }
    let mut gradients = vec![vec![0.0; a.len()]; objectives.len()];
    let h = opts.param_step;
    for k in 0..a.len() {
        let side = |sign: f64| -> Result<(Vec<f64>, Vec<f64>), String> {
            let mut ak = a.to_vec();
            ak[k] += sign * h;
            let mut sk = build(&ak)?;
            step(&mut sk, converged, &idx, objectives)
        };
        let (gp, jp) = side(1.0)?;
        let (gm, jm) = side(-1.0)?;
        for (o, lam) in rhs.iter().enumerate() {
            let adj: f64 = lam.iter().zip(gp.iter().zip(&gm)).map(|(l, (p, m))| l * (p - m)).sum();
            gradients[o][k] = (jp[o] - jm[o] + adj) / (2.0 * h);
        }
    }
    Ok(Sensitivity {
        values,
        gradients,
        n_state: n,
        fixed_point_residual,
    })
}

/// Primal solve from rest at `a` ([`solve_primal`] with tolerance 1e-14, patience 400, cap 30000) followed by
/// [`adjoint_gradient`]: the value and gradient a gradient-based optimiser needs.
pub fn shape_gradient(
    build: &dyn Fn(&[f64]) -> Result<SimpleSolver, String>,
    a: &[f64],
    objectives: &[Objective<'_>],
    opts: &AdjointOptions,
) -> Result<Sensitivity, String> {
    let mut s = build(a)?;
    check_supported(&s)?;
    solve_primal(&mut s, 1e-14, 400, 30000)?;
    adjoint_gradient(build, a, &s.snapshot(), objectives, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_solves_a_pivoting_system() {
        // needs a row swap (zero leading entry); solution (1, 2, 3)
        let mut a = vec![0.0, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0];
        let mut b = vec![7.0, 6.0, 13.0];
        let piv = lu_factor(&mut a, 3).unwrap();
        lu_solve(&a, 3, &piv, &mut b);
        for (x, e) in b.iter().zip([1.0, 2.0, 3.0]) {
            assert!((x - e).abs() < 1e-14, "{b:?}");
        }
    }

    #[test]
    fn ffd_fixes_box_edges_and_outside() {
        let d = Duct::default();
        let pts = d.ffd.displace(d.base.points(), &[0.1, 0.2, 0.05]);
        for (p, q) in d.base.points().iter().zip(&pts) {
            if p[0] <= 1.0 || p[0] >= 3.0 || p[1] >= 1.0 {
                assert_eq!(p, q);
            }
            assert_eq!((p[0], p[2]), (q[0], q[2]));
        }
        // bottom wall at mid-box: 6 0.5^4 0.2 + 4 0.5^4 (0.1 + 0.05) = 0.1125
        let i = d.base.points().iter().position(|p| p[0] == 2.0 && p[1] == 0.0).unwrap();
        assert!((pts[i][1] - 0.1125).abs() < 1e-14);
    }

    #[test]
    fn adjoint_refuses_turbulent_and_large_states() {
        let d = Duct::new([4, 2]);
        let st = d.solver(&[0.0; 3]).unwrap().snapshot();
        let obj: [Objective<'_>; 1] = [&pressure_drop];
        let turb = |a: &[f64]| {
            let mut s = d.solver(a)?;
            s.nu_face = Some(vec![0.05; s.geom.mesh.n_faces()]);
            Ok(s)
        };
        let e = adjoint_gradient(&turb, &[0.0; 3], &st, &obj, &AdjointOptions::default()).unwrap_err();
        assert!(e.contains("nu_face"), "{e}");
        let small = AdjointOptions {
            max_state: 10,
            ..Default::default()
        };
        let e = adjoint_gradient(&|a: &[f64]| d.solver(a), &[0.0; 3], &st, &obj, &small).unwrap_err();
        assert!(e.contains("max_state"), "{e}");
    }
}
