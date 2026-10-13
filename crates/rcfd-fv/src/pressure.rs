//! Pressure-equation building blocks shared by the steady (SIMPLE) and
//! transient (PISO/PIMPLE) solvers: effective pressure boundary conditions,
//! assembly and solution of `div(rAU grad p) = div(phiHbyA)`, and the flux
//! correction `phi = phiHbyA - rAU snGrad(p) |S|`.

use crate::bc::FaceCoeffs;
use crate::diffusion::laplacian;
use crate::field::rotate;
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{cg, Amg, AmgSettings, CsrMatrix, LduMatrix, SolveControl};

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
fn coupled_gamma(geom: &FvGeometry, bc: &PressureBc, gamma: &[f64]) -> Vec<f64> {
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
    let div = divergence(geom, phi_hby_a);
    let gamma_eff = coupled_gamma(geom, bc, gamma_p);
    let (own, sf) = (geom.mesh.owner(), geom.mesh.face_area_vectors());
    let (mut res0, mut iters) = (0.0, 0);
    for pass in 0..=ctl.non_orth_correctors {
        let grad = if pass > 0 {
            let bv = boundary_values_pbc(geom, bc, p);
            Some(least_squares(geom, p, &bv))
        } else {
            None
        };
        let mut pm: FvMatrix = laplacian(geom, &gamma_eff, &bc.coeffs, grad.as_deref());
        for (b, d) in pm.b.iter_mut().zip(&div) {
            *b -= d;
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
        if !bc.has_dirichlet() {
            let (cell, value) = ctl.p_ref;
            let d = pm.a.diag[cell];
            pm.a.diag[cell] += d;
            pm.b[cell] += d * value;
        }
        if extras.is_empty() {
            if pass == 0 {
                res0 = crate::simple::normalised_residual(&pm, p);
            }
            iters += if ctl.amg {
                pm.solve_amg(p, &ctl.solver)
            } else {
                pm.solve(p, &ctl.solver)
            }
            .iterations;
        } else {
            let mut t = ldu_triplets(&pm.a);
            t.extend(extras);
            let csr = CsrMatrix::from_triplets(p.len(), t);
            if pass == 0 {
                res0 = crate::simple::normalised_residual_csr(&csr, &pm.b, p);
            }
            let amg = Amg::new(&csr, AmgSettings::default());
            iters += cg(&csr, &pm.b, p, &amg, &ctl.solver).iterations;
        }
    }
    (res0, iters)
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
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let gamma_p = &coupled_gamma(geom, bc, gamma_p)[..];
    let grad = if non_orth {
        let bv = boundary_values_pbc(geom, bc, p);
        Some(least_squares(geom, p, &bv))
    } else {
        None
    };
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
                let w = geom.weights[f];
                let gf = [0, 1, 2].map(|i| w * gr[own[f]][i] + (1.0 - w) * gr[nei[f]][i]);
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
