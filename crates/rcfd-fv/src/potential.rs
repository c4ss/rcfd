//! Potential-flow initial solve (F-205, ADR 0019 D1): a scalar potential `psi` with `div(grad psi) = 0`, `u = grad psi`.
//!
//! The boundary conditions are Dirichlet `psi = 0` on outlets (patches the caller marks [`PotentialBc::Dirichlet`]) and
//! Neumann `d psi / d n = u_b . n` on every other patch ([`PotentialBc::Flux`], one value per patch face). A problem
//! without any Dirichlet patch is a pure Neumann problem: its compatibility (zero net boundary flux) is checked and the
//! level is fixed by doubling the diagonal of cell 0.
//!
//! The result is an initial guess for the viscous solver (no viscous effects, no wall layers).

use crate::bc::{FixedGradient, FixedValue, Profile};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::ScalarField;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;

/// Condition of the potential on one patch.
#[derive(Debug, Clone, PartialEq)]
pub enum PotentialBc {
    /// `psi = 0` (an outlet).
    Dirichlet,
    /// Prescribed outward normal velocity `u_b . n` of every face of the patch.
    Flux(Vec<f64>),
}

/// Result of [`potential_flow`].
#[derive(Debug, Clone)]
pub struct PotentialFlow {
    /// Cell velocity `grad psi`.
    pub u: Vec<Vec3>,
    /// Potential at the cell centres.
    pub psi: Vec<f64>,
    /// Net boundary flux of the prescribed Neumann faces (zero for a compatible problem), `sum u_b . S`.
    pub net_flux: f64,
}

/// Solve the potential problem. `patches` lists the patches with a condition; every patch not listed (and not empty)
/// gets zero normal flux.
pub fn potential_flow(geom: &FvGeometry, patches: &[(String, PotentialBc)]) -> Result<PotentialFlow, String> {
    let mut psi = ScalarField::uniform("psi", geom, 0.0);
    let sf = geom.mesh.face_area_vectors();
    let mut have_dirichlet = false;
    let (mut net, mut abs) = (0.0, 0.0);
    for (name, bc) in patches {
        let patch = geom
            .mesh
            .patch(name)
            .ok_or_else(|| format!("the mesh has no patch '{name}'"))?;
        match bc {
            PotentialBc::Dirichlet => {
                have_dirichlet = true;
                psi.set_bc(geom, name, Box::new(FixedValue::scalar(0.0)))
                    .map_err(|e| e.to_string())?;
            }
            PotentialBc::Flux(vals) => {
                let faces: Vec<usize> = patch.faces().collect();
                if vals.len() != faces.len() {
                    return Err(format!(
                        "patch '{name}': {} flux values for {} faces",
                        vals.len(),
                        faces.len()
                    ));
                }
                for (&f, v) in faces.iter().zip(vals) {
                    if !geom.empty[f] {
                        net += v * vec3::norm(sf[f]);
                        abs += (v * vec3::norm(sf[f])).abs();
                    }
                }
                let g: Vec<[f64; 3]> = vals.iter().map(|v| [*v, 0.0, 0.0]).collect();
                psi.set_bc(geom, name, Box::new(FixedGradient(Profile::PerFace(g))))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    if !have_dirichlet && net.abs() > 1e-8 * abs.max(1e-300) {
        return Err(format!(
            "the prescribed boundary fluxes do not balance: net volume flow {net:e} against a total of {abs:e}; a closed domain needs zero net flow (add an outlet with a fixed pressure)"
        ));
    }
    let n = geom.n_cells();
    let gamma = uniform_gamma(geom, 1.0);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 5000,
    };
    let mut grad: Option<Vec<Vec3>> = None;
    for _ in 0..4 {
        let bc = psi.boundary_coeffs(geom);
        let mut m = laplacian(geom, &gamma, &bc, grad.as_deref());
        if !have_dirichlet {
            let d = m.a.diag[0];
            m.a.diag[0] += d;
        }
        let mut x = psi.values.clone();
        if n > 2000 {
            m.solve_amg(&mut x, &ctl);
        } else {
            m.solve(&mut x, &ctl);
        }
        psi.values = x;
        let bv = psi.boundary_values(geom);
        grad = Some(least_squares(geom, &psi.values, &bv));
    }
    let mut u = grad.unwrap_or_else(|| vec![[0.0; 3]; n]);
    for v in u.iter_mut() {
        for k in 0..3 {
            if !geom.active[k] {
                v[k] = 0.0;
            }
        }
    }
    Ok(PotentialFlow {
        u,
        psi: psi.values,
        net_flux: net,
    })
}

/// Bernoulli head `H = p_b + |u|^2 / 2` (kinematic pressure), area-weighted over the faces of the listed patches
/// `(patch name, boundary pressure)` with the velocity of the owner cell; `None` when the list has no face.
pub fn bernoulli_head(geom: &FvGeometry, u: &[Vec3], fixed_pressure: &[(String, f64)]) -> Option<f64> {
    let sf = geom.mesh.face_area_vectors();
    let own = geom.mesh.owner();
    let (mut num, mut den) = (0.0, 0.0);
    for (name, pb) in fixed_pressure {
        let patch = geom.mesh.patch(name)?;
        for f in patch.faces() {
            if geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let q = vec3::dot(u[own[f]], u[own[f]]);
            num += a * (pb + 0.5 * q);
            den += a;
        }
    }
    (den > 0.0).then(|| num / den)
}

/// Kinematic pressure `p = H - |u|^2 / 2`.
pub fn bernoulli_pressure(u: &[Vec3], head: f64) -> Vec<f64> {
    u.iter().map(|v| head - 0.5 * vec3::dot(*v, *v)).collect()
}
