//! Pressure-equation building blocks shared by the steady (SIMPLE) and
//! transient (PISO/PIMPLE) solvers: effective pressure boundary conditions,
//! assembly and solution of `div(rAU grad p) = div(phiHbyA)`, and the flux
//! correction `phi = phiHbyA - rAU snGrad(p) |S|`.

use crate::bc::FaceCoeffs;
use crate::diffusion::{
    laplacian, laplacian_with, secondary_boundary_gradient, secondary_cell_gradient, secondary_face_gradient,
    BoundaryGradient, GradientOptions,
};
use crate::field::rotate;
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{
    cg, Amg, CsrMatrix, LduMatrix, LinearBackend, LinearDevice, LinearOptions, SolveControl, SolveStats,
};

/// Pressure boundary coefficients with the velocity-boundary logic applied.
#[derive(Debug, Clone)]
pub struct PressureBc {
    /// Effective coefficients (Neumann forced where the velocity is prescribed).
    pub coeffs: Vec<FaceCoeffs>,
    /// True where the face-normal velocity is prescribed, so the flux is fixed.
    pub fixed_u: Vec<bool>,
    /// For periodic boundary faces the global index of the partner face. The two
    /// faces of a pair are coupled implicitly in the pressure equation (exactly,
    /// not through a lagged partner value).
    pub coupled: Vec<Option<usize>>,
}

impl PressureBc {
    /// True when the pressure level is determined by a boundary condition
    /// (periodic coupling does not fix the level).
    pub fn has_dirichlet(&self) -> bool {
        self.coeffs
            .iter()
            .zip(&self.coupled)
            .any(|(c, cp)| c.c != 0.0 && cp.is_none())
    }
}

/// Build the effective pressure conditions from the velocity and pressure fields.
pub fn pressure_bc(geom: &FvGeometry, u: &VectorField, p: &ScalarField) -> PressureBc {
    let ucoef: Vec<Vec<FaceCoeffs>> = (0..3).map(|k| u.boundary_coeffs(geom, k)).collect();
    let pcoef = p.boundary_coeffs(geom);
    let fixed_u: Vec<bool> = (0..pcoef.len())
        .map(|i| {
            let n = geom.normal(geom.n_internal + i);
            let k = (0..3).max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs())).unwrap_or(0);
            // a symmetry plane has a = 1 - n_k^2, which is 1e-16 rather than 0 when the face normal is
            // not exactly axis-aligned in floating point
            ucoef[k][i].a.abs() < 1e-12
        })
        .collect();
    let coeffs = pcoef
        .iter()
        .zip(&fixed_u)
        .map(|(c, &fu)| {
            if fu {
                FaceCoeffs {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 0.0,
                }
            } else {
                *c
            }
        })
        .collect();
    let mut coupled = vec![None; pcoef.len()];
    for (pi, patch) in geom.mesh.patches().iter().enumerate() {
        if p.bc(pi).is_coupled() {
            if let Some(map) = &geom.periodic_partner[pi] {
                for (local, f) in patch.faces().enumerate() {
                    coupled[f - geom.n_internal] = Some(map[local]);
                }
            }
        }
    }
    PressureBc {
        coeffs,
        fixed_u,
        coupled,
    }
}

/// Fixed-flux pressure condition under a body acceleration: where the normal velocity is prescribed (and the face is not
/// coupled) the pressure gradient balances the body force, `dp/dn = bf.face / |S|` (as in the steady solver). Used by the
/// PISO/PIMPLE step and, with `PimpleSettings::fractional_body_force`, by the fractional step (F-771, ADR 0691).
pub fn apply_body_force_wall_gradient(geom: &FvGeometry, bf: &crate::simple::BodyForce, bc: &mut PressureBc) {
    let sf = geom.mesh.face_area_vectors();
    let n_int = geom.n_internal;
    for i in 0..bc.coeffs.len() {
        if bc.fixed_u[i] && bc.coupled[i].is_none() {
            let f = n_int + i;
            let gradient = bf.face[f] / vec3::norm(sf[f]).max(1e-300);
            bc.coeffs[i] = FaceCoeffs {
                a: 1.0,
                b: geom.dn[i] * gradient,
                c: 0.0,
                d: gradient,
            };
        }
    }
}

/// Boundary face values of a cell field under the pressure conditions, with
/// periodic faces using the *current* partner cell value (not a lagged copy).
pub fn boundary_values_pbc(geom: &FvGeometry, bc: &PressureBc, cells: &[f64]) -> Vec<f64> {
    let own = geom.mesh.owner();
    (0..bc.coeffs.len())
        .map(|i| {
            let c = &bc.coeffs[i];
            match bc.coupled[i] {
                Some(g) => c.a * cells[own[geom.n_internal + i]] + (1.0 - c.a) * cells[own[g]],
                None => c.value(cells[own[geom.n_internal + i]]),
            }
        })
        .collect()
}

/// Face diffusivity with the two faces of every periodic pair set to the same
/// distance-weighted mean (so the pair carries exactly opposite fluxes).
pub(crate) fn coupled_gamma(geom: &FvGeometry, bc: &PressureBc, gamma: &[f64]) -> Vec<f64> {
    let mut out = gamma.to_vec();
    for (i, cp) in bc.coupled.iter().enumerate() {
        if let Some(g) = *cp {
            let f = geom.n_internal + i;
            let (dp, dq) = (geom.dn[i], geom.dn[g - geom.n_internal]);
            let w = dq / (dp + dq);
            out[f] = w * gamma[f] + (1.0 - w) * gamma[g];
        }
    }
    out
}

fn ldu_triplets(a: &LduMatrix) -> Vec<(usize, usize, f64)> {
    let mut t: Vec<(usize, usize, f64)> = a.diag.iter().enumerate().map(|(i, &d)| (i, i, d)).collect();
    for f in 0..a.upper.len() {
        t.push((a.addr.lower[f], a.addr.upper[f], a.upper[f]));
        t.push((a.addr.upper[f], a.addr.lower[f], a.lower[f]));
    }
    t
}

/// Boundary face values from coefficients and cell values (index `f - n_internal`).
pub fn boundary_values(geom: &FvGeometry, coeffs: &[FaceCoeffs], cells: &[f64]) -> Vec<f64> {
    let own = geom.mesh.owner();
    coeffs
        .iter()
        .enumerate()
        .map(|(i, c)| c.value(cells[own[geom.n_internal + i]]))
        .collect()
}

/// Cell field interpolated to all faces (owner value on boundary faces).
pub fn face_interpolate(geom: &FvGeometry, cells: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                w * cells[mesh.owner()[f]] + (1.0 - w) * cells[mesh.neighbour()[f]]
            } else {
                cells[mesh.owner()[f]]
            }
        })
        .collect()
}

/// Settings of one pressure solve.
#[derive(Debug, Clone, Copy)]
pub struct PressureControls {
    /// Reference cell and value when the level is undetermined.
    pub p_ref: (usize, f64),
    /// Extra solves with non-orthogonal correction.
    pub non_orth_correctors: usize,
    /// Linear solver controls.
    pub solver: SolveControl,
    /// Precondition the pressure CG with algebraic multigrid instead of incomplete Cholesky.
    pub amg: bool,
    /// Linear-solver device (ADR 0038). `device = cpu` (the default) leaves the CPU path untouched.
    pub linear: LinearOptions,
}

/// Solve the pressure system on the device chosen by `linear` when that is not the CPU. `None` means the caller must
/// use the unchanged CPU path (device `cpu`, or `auto` that resolved to the CPU). An explicitly requested device that
/// fails is fatal (a `cuda` request is never silently downgraded); under `auto` a failing device solve falls back.
fn device_solve(
    a: &CsrMatrix,
    b: &[f64],
    x: &mut [f64],
    ctl: &SolveControl,
    linear: &LinearOptions,
    symmetric: bool,
) -> Option<SolveStats> {
    if linear.device == LinearDevice::Cpu {
        return None;
    }
    let backend: &dyn LinearBackend = match rcfd_gpu::resolve(linear.device, a.n) {
        Ok(r) => r.backend?,
        Err(e) => panic!("{e}"),
    };
    let x0 = x.to_vec();
    let res = if symmetric {
        backend.cg(a, b, x, ctl, linear)
    } else {
        backend.bicgstab(a, b, x, ctl, linear)
    };
    match res {
        Ok(s) => Some(s),
        Err(e) if linear.device == LinearDevice::Auto => {
            rcfd_gpu::note_fallback(&e.message);
            x.copy_from_slice(&x0);
            None
        }
        Err(e) => panic!("linear_solver.device = \"cuda\": the device solve failed: {e}"),
    }
}

/// Net outward flux of each cell for a face flux field.
pub fn divergence(geom: &FvGeometry, phi: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_cells())
        .map(|c| {
            let mut sum = 0.0;
            for &f in mesh.cell_faces(c) {
                if geom.empty[f] {
                    continue;
                }
                sum += if f >= geom.n_internal || mesh.owner()[f] == c {
                    phi[f]
                } else {
                    -phi[f]
                };
            }
            sum
        })
        .collect()
}

/// Solve `div(gamma grad p) = div(phiHbyA)` starting from `p`. Returns the
/// normalised initial residual and the total linear iterations.
pub fn solve_pressure(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
) -> (f64, usize) {
    let (r, it, _) = solve_pressure_parts(geom, gamma_p, bc, phi_hby_a, p, ctl, false);
    (r, it)
}

/// [`solve_pressure`] that can also return the parts of the initial residual (diagnostic, ADR 0023; `None` unless
/// `want_parts`). The solution is identical either way.
pub fn solve_pressure_parts(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    solve_pressure_parts_src(geom, gamma_p, bc, phi_hby_a, p, ctl, want_parts, None)
}

/// [`solve_pressure`] for a flow with a volume source: the corrected flux has the net outflow `vol_src[c]` (m^3/s, the
/// source integrated over the cell) instead of zero, e.g. the volume created by evaporation (ADR 0052). Returns the
/// normalised initial residual and the total linear iterations.
pub fn solve_pressure_src(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    vol_src: &[f64],
) -> (f64, usize) {
    let (r, it, _) = solve_pressure_parts_src(geom, gamma_p, bc, phi_hby_a, p, ctl, false, Some(vol_src));
    (r, it)
}

/// [`solve_pressure_parts`] with an optional cell volume source (see [`solve_pressure_src`]); `None` is the unchanged path.
#[allow(clippy::too_many_arguments)]
pub fn solve_pressure_parts_src(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
    vol_src: Option<&[f64]>,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    solve_pressure_parts_src_k(geom, gamma_p, bc, phi_hby_a, p, ctl, want_parts, vol_src, None)
}

/// [`solve_pressure_parts_src`] with a volume source that is linear in the pressure, `S(p) = vol_src + slope (p - 0)`: `slope[c] <= 0`
/// (m^3/(s Pa)) is `dS/dp` of cell `c`. The term is implicit, `diag -= slope` (positive, so the matrix stays symmetric positive
/// definite) and the caller passes `vol_src = S_k - slope p_k` for a source linearised at the pressure `p_k` (the implicit
/// cavitation coupling of ADR 0056). `None` is the unchanged path (no operation is added to it).
#[allow(clippy::too_many_arguments)]
pub fn solve_pressure_parts_src_k(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
    vol_src: Option<&[f64]>,
    slope: Option<&[f64]>,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    solve_pressure_inner(
        geom, gamma_p, bc, phi_hby_a, p, ctl, want_parts, vol_src, slope, None, None, None,
    )
}

/// A pressure gradient for the explicit non-orthogonal correction, as a function of the current cell pressures.
pub type PressureGradientFn<'a> = &'a dyn Fn(&[f64]) -> Vec<Vec3>;

/// [`PressureGradientFn`] that can be shared between threads (the operator of [`solve_pressure_implicit_non_orth`]).
pub type SyncPressureGradientFn<'a> = &'a (dyn Fn(&[f64]) -> Vec<Vec3> + Sync);

/// [`solve_pressure_parts`] whose explicit non-orthogonal correction takes its cell gradient from `grad` (called with the current
/// pressure in every corrector pass) instead of the least-squares gradient of the pressure and its boundary values (ADR 0193 V2-c).
/// `None` is [`solve_pressure_parts`]; `grad` is not called without non-orthogonal correctors.
#[allow(clippy::too_many_arguments)]
pub fn solve_pressure_parts_grad(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
    grad: Option<PressureGradientFn>,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    solve_pressure_inner(
        geom, gamma_p, bc, phi_hby_a, p, ctl, want_parts, None, None, grad, None, None,
    )
}

/// [`solve_pressure_parts_grad`] with the gradient options on the explicit non-orthogonal flux of the pressure equation (ADR 0204
/// K3): every corrector pass assembles [`laplacian_with`] with the pass's pressure and its boundary values. `None` is
/// [`solve_pressure_parts_grad`]; without non-orthogonal correctors there is no secondary flux and the options change nothing.
#[allow(clippy::too_many_arguments)]
pub fn solve_pressure_parts_opt(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
    grad: Option<PressureGradientFn>,
    opts: Option<GradientOptions>,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    solve_pressure_inner(
        geom, gamma_p, bc, phi_hby_a, p, ctl, want_parts, None, None, grad, opts, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn solve_pressure_inner(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    want_parts: bool,
    vol_src: Option<&[f64]>,
    slope: Option<&[f64]>,
    grad_fn: Option<PressureGradientFn>,
    opts: Option<GradientOptions>,
    coupled_no: Option<&CoupledNonOrth>,
) -> (f64, usize, Option<crate::simple::ResidualParts>) {
    let mut parts = None;
    let mut div = divergence(geom, phi_hby_a);
    if let Some(src) = vol_src {
        for (d, s) in div.iter_mut().zip(src) {
            *d -= s;
        }
    }
    let gamma_eff = coupled_gamma(geom, bc, gamma_p);
    let (own, sf) = (geom.mesh.owner(), geom.mesh.face_area_vectors());
    let (mut res0, mut iters) = (0.0, 0);
    // assembled pressure equation for an optional explicit non-orthogonal correction gradient; `vals` (the cell and boundary
    // pressures) are given only with the gradient options (ADR 0204 K3)
    let assemble = |grad: Option<&[Vec3]>, vals: Option<(&[f64], &[f64])>| -> (FvMatrix, Vec<(usize, usize, f64)>) {
        let mut pm: FvMatrix = match (opts, vals) {
            (Some(o), Some((cells, bvals))) => laplacian_with(geom, &gamma_eff, &bc.coeffs, grad, cells, bvals, o),
            _ => laplacian(geom, &gamma_eff, &bc.coeffs, grad),
        };
        for (b, d) in pm.b.iter_mut().zip(&div) {
            *b -= d;
        }
        if let Some(k) = slope {
            for (d, kk) in pm.a.diag.iter_mut().zip(k) {
                *d -= kk;
            }
        }
        // implicit periodic coupling: drop the lagged partner value, add the matrix entry
        let mut extras: Vec<(usize, usize, f64)> = Vec::new();
        for (i, cp) in bc.coupled.iter().enumerate() {
            if let Some(g) = *cp {
                let f = geom.n_internal + i;
                let co = &bc.coeffs[i];
                let area = vec3::norm(sf[f]);
                pm.b[own[f]] -= gamma_eff[f] * area * co.d;
                extras.push((own[f], own[g], gamma_eff[f] * area * co.c));
            }
        }
        // F-744 (ADR 0667, opt-in): the explicit non-orthogonal part of the coupled pairs
        if let (Some(cn), Some(gr)) = (coupled_no, grad) {
            cn.add_scalar_rhs(&mut pm.b, &gamma_eff, gr);
        }
        if !bc.has_dirichlet() {
            let (cell, value) = ctl.p_ref;
            let d = pm.a.diag[cell];
            pm.a.diag[cell] += d;
            pm.b[cell] += d * value;
        }
        (pm, extras)
    };
    let residual = |pm: &FvMatrix, extras: &[(usize, usize, f64)], p: &[f64]| -> f64 {
        if extras.is_empty() {
            crate::simple::normalised_residual(pm, p)
        } else {
            let mut t = ldu_triplets(&pm.a);
            t.extend_from_slice(extras);
            let csr = CsrMatrix::from_triplets(p.len(), t);
            crate::simple::normalised_residual_csr(&csr, &pm.b, p)
        }
    };
    for pass in 0..=ctl.non_orth_correctors {
        // the explicit non-orthogonal term uses the current pressure from the first solve on (so that repeated
        // outer iterations converge to the fully corrected solution and the initial residual below vanishes there)
        let grad = if ctl.non_orth_correctors > 0 {
            Some(match grad_fn {
                Some(gf) => gf(p),
                None => {
                    let bv = boundary_values_pbc(geom, bc, p);
                    least_squares(geom, p, &bv)
                }
            })
        } else {
            None
        };
        let bv_opts = match (opts, &grad) {
            (Some(_), Some(_)) => Some(boundary_values_pbc(geom, bc, p)),
            _ => None,
        };
        let (pm, extras) = assemble(grad.as_deref(), bv_opts.as_deref().map(|bv| (&*p, bv)));
        if pass == 0 {
            res0 = residual(&pm, &extras, p);
            if want_parts {
                parts = Some(if extras.is_empty() {
                    crate::simple::residual_parts(&pm, p)
                } else {
                    let mut t = ldu_triplets(&pm.a);
                    t.extend_from_slice(&extras);
                    let csr = CsrMatrix::from_triplets(p.len(), t);
                    crate::simple::residual_parts_csr(&csr, &pm.b, p)
                });
            }
        }
        if extras.is_empty() {
            let dev = if ctl.linear.device == LinearDevice::Cpu {
                None
            } else {
                device_solve(&pm.a.to_csr(), &pm.b, p, &ctl.solver, &ctl.linear, pm.a.is_symmetric())
            };
            iters += match dev {
                Some(s) => s,
                None if ctl.amg => pm.solve_amg_with(p, &ctl.solver, ctl.linear.amg_settings()),
                None => pm.solve(p, &ctl.solver),
            }
            .iterations;
        } else {
            let mut t = ldu_triplets(&pm.a);
            t.extend(extras);
            let csr = CsrMatrix::from_triplets(p.len(), t);
            let dev = if ctl.linear.device == LinearDevice::Cpu {
                None
            } else {
                device_solve(&csr, &pm.b, p, &ctl.solver, &ctl.linear, true)
            };
            iters += match dev {
                Some(s) => s,
                None => {
                    let amg = Amg::new(&csr, ctl.linear.amg_settings());
                    cg(&csr, &pm.b, p, &amg, &ctl.solver)
                }
            }
            .iterations;
        }
    }
    (res0, iters, parts)
}

/// The assembled pressure system of the first pass of [`solve_pressure_src`] (`vol_src` given) or [`solve_pressure`]
/// (`None`), by the same operations in the same order (F-466, ADR 0363 D2: the replay of a captured solve,
/// [`crate::pimple::PressureCapture`]). `None` where that solve takes another path: non-orthogonal correctors, periodic
/// coupling. Diagnostic; the solvers do not call it.
pub fn pressure_system_src(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    ctl: &PressureControls,
    vol_src: Option<&[f64]>,
) -> Option<FvMatrix> {
    if ctl.non_orth_correctors > 0 || bc.coupled.iter().any(|c| c.is_some()) {
        return None;
    }
    let mut div = divergence(geom, phi_hby_a);
    if let Some(src) = vol_src {
        for (d, s) in div.iter_mut().zip(src) {
            *d -= s;
        }
    }
    let gamma_eff = coupled_gamma(geom, bc, gamma_p);
    let mut pm = laplacian(geom, &gamma_eff, &bc.coeffs, None);
    for (b, d) in pm.b.iter_mut().zip(&div) {
        *b -= d;
    }
    if !bc.has_dirichlet() {
        let (cell, value) = ctl.p_ref;
        let d = pm.a.diag[cell];
        pm.a.diag[cell] += d;
        pm.b[cell] += d * value;
    }
    Some(pm)
}

/// The pressure equation of [`solve_pressure_parts_grad`] with its explicit non-orthogonal part made implicit (ADR 0198, F-285):
/// `F(p) = A p - b(grad(p)) = 0`, `A` and `b` the assembly of the corrector passes (the compact matrix with the reference-cell pin,
/// and the right-hand side with the non-orthogonal part of the gradient `grad`, `None` the least-squares gradient), solved from `p`
/// by restarted GMRES (restart 30) right-preconditioned with one AMG V-cycle of `A`, to the controls `ctl.solver` relative to the
/// initial residual `|F(p)|`. Its solutions are the fixed points of the corrector passes. Periodic (coupled) faces are not
/// supported (panics). Returns the normalised residual of the first corrector pass (as [`solve_pressure_parts`]) and the GMRES
/// iterations. Used by `SimpleSolver::implicit_non_orth_pressure`.
pub fn solve_pressure_implicit_non_orth(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    grad: Option<SyncPressureGradientFn>,
) -> (f64, usize) {
    assert!(
        bc.coupled.iter().all(|c| c.is_none()),
        "implicit non-orthogonal pressure solve: periodic faces are not supported"
    );
    let div = divergence(geom, phi_hby_a);
    let gamma_eff = coupled_gamma(geom, bc, gamma_p);
    let grad_of = |x: &[f64]| -> Vec<Vec3> {
        match grad {
            Some(gf) => gf(x),
            None => least_squares(geom, x, &boundary_values_pbc(geom, bc, x)),
        }
    };
    let assemble = |gr: Option<&[Vec3]>| -> FvMatrix {
        let mut pm: FvMatrix = laplacian(geom, &gamma_eff, &bc.coeffs, gr);
        for (b, d) in pm.b.iter_mut().zip(&div) {
            *b -= d;
        }
        if !bc.has_dirichlet() {
            let (cell, value) = ctl.p_ref;
            let d = pm.a.diag[cell];
            pm.a.diag[cell] += d;
            pm.b[cell] += d * value;
        }
        pm
    };
    let x0 = p.to_vec();
    let m0 = assemble(Some(&grad_of(&x0)));
    let res0 = crate::simple::normalised_residual(&m0, &x0);
    struct Op<'a> {
        a: &'a LduMatrix,
        rhs: &'a (dyn Fn(&[f64]) -> Vec<f64> + Sync),
        b0: &'a [f64],
        x0: &'a [f64],
    }
    impl rcfd_linalg::LinearOperator for Op<'_> {
        fn size(&self) -> usize {
            self.x0.len()
        }
        fn apply(&self, d: &[f64], y: &mut [f64]) {
            // J d = A d - (b(grad(x0 + d)) - b(grad(x0))), exact for the affine F
            self.a.matvec(d, y);
            let x: Vec<f64> = self.x0.iter().zip(d).map(|(a, b)| a + b).collect();
            let b = (self.rhs)(&x);
            for ((yi, bi), b0) in y.iter_mut().zip(&b).zip(self.b0) {
                *yi -= bi - b0;
            }
        }
    }
    // the matrix part does not depend on the gradient; only the right-hand side does
    let rhs = |x: &[f64]| assemble(Some(&grad_of(x))).b;
    let op = Op {
        a: &m0.a,
        rhs: &rhs,
        b0: &m0.b,
        x0: &x0,
    };
    let mut ax = vec![0.0; x0.len()];
    m0.a.matvec(&x0, &mut ax);
    let r0: Vec<f64> = m0.b.iter().zip(&ax).map(|(b, a)| b - a).collect();
    let amg = Amg::new(&m0.a.to_csr(), ctl.linear.amg_settings());
    let mut d = vec![0.0; x0.len()];
    let st = rcfd_linalg::gmres(&op, &r0, &mut d, &amg, 30, &ctl.solver);
    for (pi, di) in p.iter_mut().zip(&d) {
        *pi += di;
    }
    (res0, st.iterations)
}

/// Conservative face flux `phiHbyA - gamma snGrad(p) |S|` (prescribed flux on
/// fixed-velocity boundary faces, which use `ub`).
pub fn corrected_flux(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &[f64],
    ub: &[Vec3],
    non_orth: bool,
) -> Vec<f64> {
    let grad = if non_orth {
        let bv = boundary_values_pbc(geom, bc, p);
        Some(least_squares(geom, p, &bv))
    } else {
        None
    };
    corrected_flux_with(geom, gamma_p, bc, phi_hby_a, p, ub, grad.as_deref())
}

/// [`corrected_flux`] with the cell gradient of the explicit non-orthogonal part given (`None`: no non-orthogonal part); ADR 0193
/// V2-c passes the body-force-weighted gradient. [`corrected_flux`] calls it with the least-squares gradient.
pub fn corrected_flux_with(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &[f64],
    ub: &[Vec3],
    grad: Option<&[Vec3]>,
) -> Vec<f64> {
    corrected_flux_impl(geom, gamma_p, bc, phi_hby_a, p, ub, grad, false, None)
}

/// [`corrected_flux_with`] with the gradient options of the pressure equation (ADR 0204 K3): the explicit non-orthogonal part of
/// every internal face takes the cell gradients limited as [`laplacian_with`] limits them
/// ([`secondary_cell_gradient`], with the pressure and its boundary values) and the face correction
/// ([`secondary_face_gradient`]); every fixed-pressure boundary face (`a = 0`, neither a fixed-velocity nor a periodic face) takes
/// the non-orthogonal part the equation has there, `gamma_f k_f . g_b` with `g_b` of [`secondary_boundary_gradient`] under the
/// options' boundary treatment (ADR 0221 D3; none for `Dropped`). So the flux carries the face flux of the pressure operator
/// assembled with the same options on every face. `None` options, `Some(GradientOptions::default())` (ADR 0221 amendment 2) or no
/// gradient is [`corrected_flux_with`], bit for bit (whose fixed-pressure faces have no non-orthogonal part, ADR 0204 Q3).
#[allow(clippy::too_many_arguments)]
pub fn corrected_flux_opt(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &[f64],
    ub: &[Vec3],
    grad: Option<&[Vec3]>,
    opts: Option<GradientOptions>,
) -> Vec<f64> {
    match (grad, opts) {
        (Some(gr), Some(o)) => {
            let bv = boundary_values_pbc(geom, bc, p);
            let limited = secondary_cell_gradient(geom, p, &bv, gr, &o);
            let g = limited.as_deref().unwrap_or(gr);
            corrected_flux_impl(
                geom,
                gamma_p,
                bc,
                phi_hby_a,
                p,
                ub,
                Some(g),
                o.face_correction,
                // ADR 0221 amendment 2: the default options are the default flux, bit for bit
                (o != GradientOptions::default()).then_some((o.boundary, &bv[..])),
            )
        }
        _ => corrected_flux_with(geom, gamma_p, bc, phi_hby_a, p, ub, grad),
    }
}

/// The face flux of the flux correction. `boundary`: the boundary treatment and the boundary pressures of the options, for the
/// non-orthogonal part of the fixed-pressure faces (ADR 0221 D3); `None` adds no operation there.
#[allow(clippy::too_many_arguments)]
fn corrected_flux_impl(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &[f64],
    ub: &[Vec3],
    grad: Option<&[Vec3]>,
    face_correction: bool,
    boundary: Option<(BoundaryGradient, &[f64])>,
) -> Vec<f64> {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let gamma_p = &coupled_gamma(geom, bc, gamma_p)[..];
    let mut phi = vec![0.0; mesh.n_faces()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        if f < geom.n_internal {
            let mut flux = gamma_p[f] * geom.lap_coeff[f] * (p[nei[f]] - p[own[f]]);
            if let Some(gr) = &grad {
                let d = geom.delta[f];
                let delta = vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d));
                let k = vec3::sub(sf[f], delta);
                let gf = if face_correction {
                    secondary_face_gradient(geom, f, gr, p, true)
                } else {
                    let w = geom.weights[f];
                    [0, 1, 2].map(|i| w * gr[own[f]][i] + (1.0 - w) * gr[nei[f]][i])
                };
                flux += gamma_p[f] * vec3::dot(k, gf);
            }
            phi[f] = phi_hby_a[f] - flux;
        } else if bc.fixed_u[f - geom.n_internal] {
            phi[f] = vec3::dot(ub[f - geom.n_internal], sf[f]);
        } else if let Some(g) = bc.coupled[f - geom.n_internal] {
            let span = geom.dn[f - geom.n_internal] + geom.dn[g - geom.n_internal];
            phi[f] = phi_hby_a[f] - gamma_p[f] * vec3::norm(sf[f]) * (p[own[g]] - p[own[f]]) / span;
        } else {
            let co = &bc.coeffs[f - geom.n_internal];
            phi[f] = phi_hby_a[f] - gamma_p[f] * vec3::norm(sf[f]) * co.gradient(p[own[f]]);
            // ADR 0221 D3: the non-orthogonal part of the paired equation on a fixed-pressure face, under the options
            if let (Some(gr), Some((kind, bv)), true) = (grad, boundary, co.a == 0.0) {
                if let Some(gb) = secondary_boundary_gradient(geom, f, gr, p, bv, kind) {
                    let d = geom.delta[f];
                    let a = vec3::norm(sf[f]);
                    let k = vec3::sub(sf[f], vec3::scale(d, a * a / vec3::dot(sf[f], d)));
                    phi[f] -= gamma_p[f] * vec3::dot(k, gb);
                }
            }
        }
    }
    phi
}

/// Boundary-face values of `HbyA` for the flux `phiHbyA`: the prescribed
/// velocity where the normal velocity is fixed, the distance-weighted mean of
/// owner and partner cell on coupled (periodic) patches so that the two
/// faces of a pair carry opposite fluxes, and the owner value elsewhere.
pub fn boundary_hby_a(geom: &FvGeometry, bc: &PressureBc, ub: &[Vec3], hby_a: &[Vec3]) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let own = mesh.owner();
    let mut out: Vec<Vec3> = (0..mesh.n_boundary_faces())
        .map(|i| {
            if bc.fixed_u[i] {
                ub[i]
            } else {
                hby_a[own[geom.n_internal + i]]
            }
        })
        .collect();
    for (pi, p) in mesh.patches().iter().enumerate() {
        if let Some(partner) = &geom.periodic_partner[pi] {
            for (local, f) in p.faces().enumerate() {
                let g = partner[local];
                let (dp, dq) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
                let w = dq / (dp + dq);
                let a = hby_a[own[f]];
                let mut b = hby_a[own[g]];
                if let Some(r) = &geom.periodic_rot[pi] {
                    b = rotate(r, b);
                }
                out[f - geom.n_internal] = [0, 1, 2].map(|k| w * a[k] + (1.0 - w) * b[k]);
            }
        }
    }
    out
}

/// One coupled (periodic) boundary face of [`CoupledNonOrth`].
#[derive(Debug, Clone, Copy)]
struct CoupledFace {
    /// Global face index.
    f: usize,
    /// Owner cell.
    r: usize,
    /// The partner face's owner cell.
    c: usize,
    /// Rotation of the pair (the partner's frame to this face's frame); `None` for a translation.
    rot: Option<[[f64; 3]; 3]>,
    /// Owner weight `dn_g / (dn_f + dn_g)` (the `a` of `bc::Periodic`).
    w: f64,
    /// Owner centre to the partner centre's image, `(x_f - c_r) + R (c_c - x_g)`.
    d: Vec3,
    /// Non-orthogonal remainder `S - d |S|^2 / (S . d)`.
    k: Vec3,
}

/// F-744 (ADR 0667): the explicit non-orthogonal correction of the coupled (periodic) faces, each pair treated as the internal
/// face between the owner and the partner cell's image in the owner's frame: `d = (x_f - c_r) + R (c_c - x_g)`, the orthogonal
/// part `|S|^2 / (S . d) = |S| / (dn_f + dn_g)` (the coefficient the coupled faces already carry) and the remainder
/// `k = S - d |S|^2 / (S . d)` with the face gradient `w grad_r + (1 - w) R grad_c`. The two faces of a pair carry opposite
/// corrections. The cell gradients are the least-squares gradients with the partner cell as the point of every coupled face
/// (ADR 0077 F-H0, [`crate::gradient::least_squares_partner`]). Built only with `PimpleSettings::coupled_non_orth`.
#[derive(Debug, Clone, Default)]
pub struct CoupledNonOrth {
    faces: Vec<CoupledFace>,
}

impl CoupledNonOrth {
    /// The coupled faces of the patches for which `coupled(patch index)` holds and that have a partner map (empty faces
    /// skipped), in boundary-face order.
    pub fn build(geom: &FvGeometry, coupled: impl Fn(usize) -> bool) -> Self {
        let mesh = &geom.mesh;
        let (own, cc, fc, sf) = (
            mesh.owner(),
            mesh.cell_centres(),
            mesh.face_centres(),
            mesh.face_area_vectors(),
        );
        let mut faces = Vec::new();
        for (pi, p) in mesh.patches().iter().enumerate() {
            let (Some(map), true) = (&geom.periodic_partner[pi], coupled(pi)) else {
                continue;
            };
            let rot = geom.periodic_rot[pi];
            for (local, f) in p.faces().enumerate() {
                if geom.empty[f] {
                    continue;
                }
                let g = map[local];
                let (r, c) = (own[f], own[g]);
                let back = vec3::sub(cc[c], fc[g]);
                let d = vec3::add(vec3::sub(fc[f], cc[r]), rot.as_ref().map_or(back, |m| rotate(m, back)));
                let s = sf[f];
                let k = vec3::sub(s, vec3::scale(d, vec3::dot(s, s) / vec3::dot(s, d)));
                let (dp, dq) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
                faces.push(CoupledFace {
                    f,
                    r,
                    c,
                    rot,
                    w: dq / (dp + dq),
                    d,
                    k,
                });
            }
        }
        faces.sort_by_key(|e| e.f);
        CoupledNonOrth { faces }
    }

    /// True when there is no coupled face.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// `(face, d, k)` of every coupled face (diagnostic, ADR 0667 H2).
    pub fn geometry(&self) -> Vec<(usize, Vec3, Vec3)> {
        self.faces.iter().map(|e| (e.f, e.d, e.k)).collect()
    }

    fn rot(e: &CoupledFace, v: Vec3) -> Vec3 {
        e.rot.as_ref().map_or(v, |m| rotate(m, v))
    }

    /// The face gradient `w grad_r + (1 - w) R grad_c` of a scalar.
    fn face_gradient(e: &CoupledFace, grad: &[Vec3]) -> Vec3 {
        let gc = Self::rot(e, grad[e.c]);
        [0, 1, 2].map(|i| e.w * grad[e.r][i] + (1.0 - e.w) * gc[i])
    }

    /// The least-squares gradient of a scalar with the partner cell as the point of every coupled face.
    pub fn scalar_gradient(&self, geom: &FvGeometry, cells: &[f64], bvals: &[f64]) -> Vec<Vec3> {
        let pts: Vec<(usize, Vec3, f64)> = self
            .faces
            .iter()
            .map(|e| (e.f - geom.n_internal, e.d, cells[e.c]))
            .collect();
        crate::gradient::least_squares_partner(geom, cells, bvals, &pts)
    }

    /// The least-squares gradients of the active components of a vector field (`cells[j]`, `bvals[j]` the cell and boundary
    /// values of component `j`, `values` the cell vectors the partner values are rotated from), the partner cell (its
    /// rotated value) the point of every coupled face. Inactive components get `None`.
    pub fn vector_gradients(
        &self,
        geom: &FvGeometry,
        values: &[Vec3],
        cells: &[Vec<f64>; 3],
        bvals: &[Vec<f64>; 3],
    ) -> [Option<Vec<Vec3>>; 3] {
        let q: Vec<Vec3> = self.faces.iter().map(|e| Self::rot(e, values[e.c])).collect();
        [0, 1, 2].map(|j| {
            geom.active[j].then(|| {
                let pts: Vec<(usize, Vec3, f64)> = self
                    .faces
                    .iter()
                    .zip(&q)
                    .map(|(e, v)| (e.f - geom.n_internal, e.d, v[j]))
                    .collect();
                crate::gradient::least_squares_partner(geom, &cells[j], &bvals[j], &pts)
            })
        })
    }

    /// Add `gamma_f k_f . (w grad_r + (1 - w) R grad_c)` of every coupled face to `b[r]` (a scalar, e.g. the pressure).
    pub fn add_scalar_rhs(&self, b: &mut [f64], gamma: &[f64], grad: &[Vec3]) {
        for e in &self.faces {
            b[e.r] += gamma[e.f] * vec3::dot(e.k, Self::face_gradient(e, grad));
        }
    }

    /// Subtract `gamma_f k_f . grad_f` of every coupled face from its flux (the flux of the pressure equation with
    /// [`Self::add_scalar_rhs`]; `gamma` the pair-averaged coefficient of that equation).
    pub fn fix_flux(&self, phi: &mut [f64], gamma: &[f64], grad: &[Vec3]) {
        for e in &self.faces {
            phi[e.f] -= gamma[e.f] * vec3::dot(e.k, Self::face_gradient(e, grad));
        }
    }

    /// Add the correction of component `k` of a vector field to `b[r]`: the face gradient of component `k` is
    /// `w grad u_k(r) + (1 - w) sum_i R_ki R grad u_i(c)` (the partner's gradient tensor in the owner's frame).
    pub fn add_vector_rhs(&self, b: &mut [f64], k: usize, gamma: &[f64], grads: &[Option<Vec<Vec3>>; 3]) {
        let Some(gk) = &grads[k] else {
            return;
        };
        for e in &self.faces {
            let gc = match &e.rot {
                None => gk[e.c],
                Some(m) => {
                    let mut gc = [0.0; 3];
                    for (i, gi) in grads.iter().enumerate() {
                        if let Some(gi) = gi {
                            vec3::axpy(&mut gc, m[k][i], rotate(m, gi[e.c]));
                        }
                    }
                    gc
                }
            };
            let gf = [0, 1, 2].map(|i| e.w * gk[e.r][i] + (1.0 - e.w) * gc[i]);
            b[e.r] += gamma[e.f] * vec3::dot(e.k, gf);
        }
    }
}

/// [`solve_pressure`] with the explicit non-orthogonal correction of the coupled faces (F-744, ADR 0667): every corrector pass
/// takes the partner-point least-squares gradient of the current pressure ([`CoupledNonOrth::scalar_gradient`]) for the
/// non-orthogonal part of every face and adds that of the coupled pairs to the right-hand side. Without non-orthogonal
/// correctors it is [`solve_pressure`].
pub fn solve_pressure_coupled(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &mut [f64],
    ctl: &PressureControls,
    cn: &CoupledNonOrth,
) -> (f64, usize) {
    let gf = |x: &[f64]| cn.scalar_gradient(geom, x, &boundary_values_pbc(geom, bc, x));
    let (r, it, _) = solve_pressure_inner(
        geom,
        gamma_p,
        bc,
        phi_hby_a,
        p,
        ctl,
        false,
        None,
        None,
        Some(&gf),
        None,
        Some(cn),
    );
    (r, it)
}

/// [`corrected_flux`] with the explicit non-orthogonal correction of the coupled faces (F-744, ADR 0667), the flux of
/// [`solve_pressure_coupled`]: the partner-point gradient of `p` in the non-orthogonal part of every face. `non_orth = false`
/// is [`corrected_flux`] without correction.
#[allow(clippy::too_many_arguments)]
pub fn corrected_flux_coupled(
    geom: &FvGeometry,
    gamma_p: &[f64],
    bc: &PressureBc,
    phi_hby_a: &[f64],
    p: &[f64],
    ub: &[Vec3],
    non_orth: bool,
    cn: &CoupledNonOrth,
) -> Vec<f64> {
    if !non_orth {
        return corrected_flux(geom, gamma_p, bc, phi_hby_a, p, ub, false);
    }
    let grad = cn.scalar_gradient(geom, p, &boundary_values_pbc(geom, bc, p));
    let mut phi = corrected_flux_with(geom, gamma_p, bc, phi_hby_a, p, ub, Some(&grad));
    cn.fix_flux(&mut phi, &coupled_gamma(geom, bc, gamma_p), &grad);
    phi
}
