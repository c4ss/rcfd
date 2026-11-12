//! The face coefficient of the pressure term that the transient Rhie-Chow carry-over is consistent with (ADR 0245 C,
//! opt-in `PimpleSettings::rhie_chow_step_consistent`).
//!
//! With `rAU = V / a_P` and the linear face interpolation `interp`, the default flux of a corrector is
//! `interp(HbyA) . S + (gamma / dt) K - gamma snGrad(p) |S|`, `gamma = interp(rAU)`, `K` the carried old-level defects.
//! Its converged face defect `d = phi - interp(u) . S` depends on the step size where `a_P / V` differs between the two
//! cells of a face (ADR 0073 R2, ADR 0245). Here the face coefficient is the harmonic face value
//! `Gamma_f = 1 / interp(1 / rAU)` and `[Gamma_f interp(x) - interp(rAU x)] . S` is added to the flux, with
//! `x = grad(p_lag) - b` of the pressure the corrector starts from (`b` a body acceleration). Converged, the defect then
//! obeys `(c0 d - c1 d^n + c2 d^{n-1}) / dt + a_f d = interp(x) . S - snGrad(p) |S|` with `a_f = interp(a_P / V) - c0 / dt`:
//! the step size appears only through the backward difference.

use crate::geometry::FvGeometry;
use crate::gradient::green_gauss;
use crate::pressure::{boundary_values_pbc, PressureBc};
use rcfd_core::vec3::{self, Vec3};

/// The harmonic face value of a positive cell field with the linear face weights; the cell value on a boundary face.
pub(crate) fn harmonic_face(geom: &FvGeometry, cells: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                1.0 / (w / cells[mesh.owner()[f]] + (1.0 - w) / cells[mesh.neighbour()[f]])
            } else {
                cells[mesh.owner()[f]]
            }
        })
        .collect()
}

/// Add `[gamma_f interp(x) - interp(q)] . S` to the flux of every internal face (`q` is `rAU x` of the cells that have a
/// momentum row).
pub(crate) fn add_commutator(geom: &FvGeometry, gamma: &[f64], q: &[Vec3], x: &[Vec3], phi_hby_a: &mut [f64]) {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    for f in 0..geom.n_internal {
        let w = geom.weights[f];
        let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
        let v = [0, 1, 2].map(|k| gamma[f] * (w * x[o][k] + (1.0 - w) * x[n][k]) - (w * q[o][k] + (1.0 - w) * q[n][k]));
        phi_hby_a[f] += vec3::dot(v, sf[f]);
    }
}

/// [`add_commutator`] for a step in which every cell has a momentum row: `x = grad(p_lag) - b` with the Green-Gauss
/// gradient of the corrector and `q = rAU x`.
pub(crate) fn add_lagged_gradient(
    geom: &FvGeometry,
    pbc: &PressureBc,
    gamma: &[f64],
    r_au: &[f64],
    p_lag: &[f64],
    body: Option<&[Vec3]>,
    phi_hby_a: &mut [f64],
) {
    let mut x = green_gauss(geom, p_lag, &boundary_values_pbc(geom, pbc, p_lag), 0);
    if let Some(b) = body {
        for (xc, bc) in x.iter_mut().zip(b) {
            *xc = vec3::sub(*xc, *bc);
        }
    }
    let q: Vec<Vec3> = x.iter().zip(r_au).map(|(xc, r)| vec3::scale(*xc, *r)).collect();
    add_commutator(geom, gamma, &q, &x, phi_hby_a);
}
