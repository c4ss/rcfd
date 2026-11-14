//! A start field without flow through the immersed bodies (F-515, ADR 0425 D1; the algorithm of ADR 0421 D1).
//!
//! The start velocity is set to zero in the solid cells, its face flux is closed on every face with a solid side, and the
//! result is projected onto the fluid region by a discrete Helmholtz-Hodge projection on the cell faces (the idea after
//! Chorin, Math. Comp. 22 (1968); derived from the discrete continuity equation): `phi_f = phi*_f - a_f (psi_N - psi_P)` on
//! the open faces, with `psi` from `sum_f a_f (psi_P - psi_N) = -sum_f phi*_f` in every cell, solved by Jacobi-preconditioned
//! conjugate gradients (Hestenes and Stiefel, 1952). Open faces are the fluid-fluid internal faces and the boundary faces of
//! fixed pressure (`psi = 0` there); every other face keeps its flux. The face flux is divergence-free to the tolerance of the
//! solve; the cell velocity is corrected by the Gauss gradient of `psi` and is not the exact interpolation of the flux.
//!
//! Opt-in: nothing calls this unless asked ([`crate::pimple::PimpleSolver::project_immersed_start`]).

use crate::convection::face_flux;
use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::immersed::ImmersedForcing;
use crate::pressure::pressure_bc;
use rcfd_core::vec3::Vec3;

/// Relative residual `|r| / |b|` at which the conjugate gradients of the projection stop.
pub const PROJECTION_TOL: f64 = 1e-13;
/// Largest number of conjugate-gradient iterations of the projection.
pub const PROJECTION_MAX_ITERATIONS: usize = 50_000;

/// What the projection measured (ADR 0425 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StartProjection {
    /// Number of solid cells (centre inside a body).
    pub solid_cells: usize,
    /// `sum abs(phi*)` over the internal faces with exactly one solid cell, before they are closed: the flow into the bodies
    /// of the given start.
    pub into_solid: f64,
    /// Conjugate-gradient iterations of the projection.
    pub cg_iterations: usize,
    /// `max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)` of the projected flux.
    pub divergence_ratio: f64,
}

/// The projected start: cell velocity, face flux (positive out of the owner) and the report.
#[derive(Debug, Clone)]
pub struct ProjectedStart {
    /// Cell velocity (zero in the solid cells).
    pub u: Vec<Vec3>,
    /// Face flux (zero on every face with a solid cell on either side).
    pub phi: Vec<f64>,
    /// What was measured.
    pub report: StartProjection,
}

fn invalid(reason: String) -> rcfd_core::Error {
    rcfd_core::Error::InvalidValue {
        path: "immersed start projection".into(),
        reason,
    }
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Per cell `sum_f phi_f` out of the cell.
fn divergence(geom: &FvGeometry, phi: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut d = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        d[own[f]] += phi[f];
        if f < geom.n_internal {
            d[nei[f]] -= phi[f];
        }
    }
    d
}

/// `max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)` (ADR 0421 S1).
pub fn divergence_ratio(geom: &FvGeometry, phi: &[f64]) -> f64 {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut scale = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        scale[own[f]] += phi[f].abs();
        if f < geom.n_internal {
            scale[nei[f]] += phi[f].abs();
        }
    }
    let d = divergence(geom, phi);
    let dmax = d.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let smax = scale.iter().fold(0.0f64, |a, x| a.max(*x));
    dmax / smax
}

/// Project the start velocity `u` (with its boundary conditions) onto the fluid region of the forcing `ib` (ADR 0425 D1). `p`
/// gives the pressure conditions: its fixed-value boundary faces (where the velocity is not prescribed) are the open boundary
/// faces. Fails for a forcing built for another mesh, a domain without such a face, or a solve that does not converge.
pub fn project_start(
    geom: &FvGeometry,
    u: &VectorField,
    p: &ScalarField,
    ib: &ImmersedForcing,
) -> rcfd_core::Result<ProjectedStart> {
    let nc = geom.n_cells();
    if ib.n_cells() != nc {
        return Err(invalid(format!(
            "the forcing has {} cells, the mesh {nc}",
            ib.n_cells()
        )));
    }
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let solid = ib.solid_mask();
    // D1 1: the solid cells at rest.
    let mut field = u.clone();
    for (c, v) in field.values.iter_mut().enumerate() {
        if solid[c] {
            *v = [0.0; 3];
        }
    }
    // D1 2: the face flux, closed on every face with a solid side.
    let bu = field.boundary_values(geom);
    let mut phi_star = face_flux(geom, &field.values, &bu);
    let mut into_solid = 0.0;
    for f in 0..geom.n_internal {
        let (so, sn) = (solid[own[f]], solid[nei[f]]);
        if so != sn {
            into_solid += phi_star[f].abs();
        }
        if so || sn {
            phi_star[f] = 0.0;
        }
    }
    // D1 3: the open faces.
    let cc = mesh.cell_centres();
    let fc = mesh.face_centres();
    let mut internal: Vec<(usize, usize, usize, f64)> = Vec::new();
    for f in 0..geom.n_internal {
        if solid[own[f]] || solid[nei[f]] {
            continue;
        }
        let (o, n) = (own[f], nei[f]);
        internal.push((f, o, n, mesh.face_area(f) / dist(cc[o], cc[n])));
    }
    let pbc = pressure_bc(geom, u, p);
    let mut open_bd: Vec<(usize, usize, f64)> = Vec::new();
    let mut is_open_bd = vec![false; mesh.n_faces()];
    for (i, c) in pbc.coeffs.iter().enumerate() {
        let f = geom.n_internal + i;
        if geom.empty[f] || pbc.fixed_u[i] || pbc.coupled[i].is_some() || c.a != 0.0 {
            continue;
        }
        let o = own[f];
        open_bd.push((f, o, mesh.face_area(f) / dist(cc[o], fc[f])));
        is_open_bd[f] = true;
    }
    if open_bd.is_empty() {
        return Err(invalid(
            "no boundary face of fixed pressure: the projection needs one (a closed domain is not covered)".into(),
        ));
    }
    let mut diag = vec![0.0; nc];
    for &(_, o, n, a) in &internal {
        diag[o] += a;
        diag[n] += a;
    }
    for &(_, o, a) in &open_bd {
        diag[o] += a;
    }
    // Cells without an open face (the solid ones) are decoupled: psi = 0.
    for d in diag.iter_mut() {
        if *d == 0.0 {
            *d = 1.0;
        }
    }
    let apply = |x: &[f64]| -> Vec<f64> {
        let mut y: Vec<f64> = x.iter().zip(&diag).map(|(a, b)| a * b).collect();
        for &(_, o, n, a) in &internal {
            y[o] -= a * x[n];
            y[n] -= a * x[o];
        }
        y
    };
    let div = divergence(geom, &phi_star);
    let b: Vec<f64> = div.iter().map(|d| -d).collect();
    let nb = norm(&b);
    let mut psi = vec![0.0; nc];
    let mut iters = 0;
    if nb > 0.0 {
        let mut r = b.clone();
        let mut z: Vec<f64> = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
        let mut pd = z.clone();
        let mut rz = dot(&r, &z);
        loop {
            if norm(&r) <= PROJECTION_TOL * nb {
                break;
            }
            if iters >= PROJECTION_MAX_ITERATIONS {
                return Err(invalid(format!(
                    "the conjugate gradients did not converge in {PROJECTION_MAX_ITERATIONS} iterations (residual ratio \
                     {:.3e}); is every fluid region connected to a boundary face of fixed pressure?",
                    norm(&r) / nb
                )));
            }
            let ap = apply(&pd);
            let alpha = rz / dot(&pd, &ap);
            for i in 0..nc {
                psi[i] += alpha * pd[i];
                r[i] -= alpha * ap[i];
            }
            z = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
            let rz_new = dot(&r, &z);
            let beta = rz_new / rz;
            rz = rz_new;
            for i in 0..nc {
                pd[i] = z[i] + beta * pd[i];
            }
            iters += 1;
        }
    }
    let mut phi = phi_star;
    for &(f, o, n, a) in &internal {
        phi[f] -= a * (psi[n] - psi[o]);
    }
    for &(f, o, a) in &open_bd {
        phi[f] += a * psi[o];
    }
    // D1 4: the cell velocity corrected by the Gauss gradient of psi.
    let sf = mesh.face_area_vectors();
    let vol = mesh.cell_volumes();
    let mut g = vec![[0.0; 3]; nc];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let o = own[f];
        if f < geom.n_internal {
            let n = nei[f];
            let (psi_o, psi_n) = match (solid[o], solid[n]) {
                (false, false) => {
                    let w = geom.weights[f];
                    let v = w * psi[o] + (1.0 - w) * psi[n];
                    (v, v)
                }
                _ => (psi[o], psi[n]),
            };
            for k in 0..3 {
                g[o][k] += psi_o * sf[f][k];
                g[n][k] -= psi_n * sf[f][k];
            }
        } else {
            let pf = if is_open_bd[f] { 0.0 } else { psi[o] };
            for k in 0..3 {
                g[o][k] += pf * sf[f][k];
            }
        }
    }
    let u_new: Vec<Vec3> = (0..nc)
        .map(|c| {
            if solid[c] {
                [0.0; 3]
            } else {
                let x = field.values[c];
                [x[0] - g[c][0] / vol[c], x[1] - g[c][1] / vol[c], x[2]]
            }
        })
        .collect();
    let divergence_ratio = divergence_ratio(geom, &phi);
    Ok(ProjectedStart {
        u: u_new,
        phi,
        report: StartProjection {
            solid_cells: solid.iter().filter(|&&s| s).count(),
            into_solid,
            cg_iterations: iters,
            divergence_ratio,
        },
    })
}
