//! Shared helpers of the conjugate-heat-transfer library checks (ADR 0024).
#![allow(dead_code)]

use rcfd_fv::cht::ChtMaterials;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::Mesh;
use std::sync::Arc;

pub fn geometry(mesh: Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// Tight linear-solver controls of the registered checks (ADR 0024, "Common set-up").
pub fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 5000,
    }
}

/// Energy solver on `mats` (per-face conductivity and capacity weights set), relaxation 1, tight linear solves.
pub fn cht_solver(geom: &Arc<FvGeometry>, t: ScalarField, mats: &ChtMaterials, scheme: ConvScheme) -> EnergySolver {
    let mut s = EnergySolver::new(geom.clone(), t, 1.0);
    s.convection = scheme;
    s.relax = 1.0;
    s.solver = tight();
    mats.configure(geom, &mut s);
    s
}

/// Sweep until the largest temperature change falls below `tol` (or `max` sweeps); returns (sweeps, last change).
/// The change cannot go below about `tol_rel * T` because every sweep restarts the linear solve (ADR 0024 addendum).
pub fn converge(en: &mut EnergySolver, phi: &[f64], tol: f64, max: usize) -> (usize, f64) {
    let mut change = f64::INFINITY;
    for i in 1..=max {
        let before = en.t.values.clone();
        en.iterate(phi);
        change = before
            .iter()
            .zip(&en.t.values)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        if change < tol {
            return (i, change);
        }
    }
    (max, change)
}

/// Homogeneous materials: conductivity `k`, capacity `rho_cp` everywhere.
pub fn uniform_materials(geom: &FvGeometry, k: f64, rho_cp: f64) -> ChtMaterials {
    ChtMaterials {
        rho_cp_ref: rho_cp,
        k_cell: vec![k; geom.n_cells()],
        rho_cp_cell: vec![rho_cp; geom.n_cells()],
        contacts: vec![],
    }
}

/// Materials from a per-cell label: `k[label]`, `rho_cp[label]`; the reference capacity is `rho_cp_ref`.
pub fn labelled_materials(label: &[usize], k: &[f64], rho_cp: &[f64], rho_cp_ref: f64) -> ChtMaterials {
    ChtMaterials {
        rho_cp_ref,
        k_cell: label.iter().map(|&l| k[l]).collect(),
        rho_cp_cell: label.iter().map(|&l| rho_cp[l]).collect(),
        contacts: vec![],
    }
}

/// Total area of a patch.
pub fn patch_area(geom: &FvGeometry, patch: &str) -> f64 {
    let sf = geom.mesh.face_area_vectors();
    geom.mesh
        .patch(patch)
        .unwrap()
        .faces()
        .map(|f| rcfd_core::vec3::norm(sf[f]))
        .sum()
}

/// Complementary error function (series below 3, continued fraction above).
pub fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 3.0 {
        let (mut term, mut sum, mut n) = (x, x, 0.0);
        while term.abs() > 1e-17 {
            n += 1.0;
            term *= -x * x / n;
            sum += term / (2.0 * n + 1.0);
        }
        1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum
    } else {
        let mut f = 0.0;
        for k in (1..60).rev() {
            f = (k as f64 / 2.0) / (x + f);
        }
        (-x * x).exp() / ((x + f) * std::f64::consts::PI.sqrt())
    }
}
