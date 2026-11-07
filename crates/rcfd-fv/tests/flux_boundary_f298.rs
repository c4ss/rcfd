//! F-298 (ADR 0221) part (b), library: under the gradient options the pressure flux correction takes the non-orthogonal part the
//! pressure equation has on fixed-pressure faces (D3). Limits are those registered in
//! docs/adr/0221-f298-registration-electric-current-and-joule-heat-with-the-gradient-options-and-the-fixed-pressure-flux-correction.md
//! before this file was written.
//!
//! Set-up K-B of ADR 0204: the skewed box K with `xmax` at a fixed pressure. B-lib: with every option set the flux correction
//! carries the face flux of the pressure operator with the same options on every cell, those with a fixed-pressure face included
//! (to 1e-12 of the flux scale). B-lib-T: the flux correction without the options and without the boundary term misses that on
//! the cells beside the fixed-pressure faces. B-lib-D: `None`, the dropped boundary treatment and a run without a gradient give
//! the bits of the existing function; B-lib-D2 (amendment 2): so do the default options.

use rcfd_core::vec3::Vec3;
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::pressure::{
    boundary_values_pbc, corrected_flux_opt, corrected_flux_with, divergence, pressure_bc, PressureBc,
};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use std::sync::Arc;

/// B-lib limit, relative to the flux scale `S`.
const IDENTITY: f64 = 1e-12;
/// B-lib-T limit: the smallest miss without the boundary term.
const TEETH: f64 = 1e-9;

/// ADR 0016 mesh K at `n`.
fn mesh_k(n: usize) -> Arc<FvGeometry> {
    let m = warp(&box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap(), |p| {
        [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
    })
    .unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// ADR 0190's option sets O1 to O6 and C2.
fn sets() -> Vec<(&'static str, GradientOptions)> {
    let d = GradientOptions::default();
    let venk = |k| Some(Limiter::Venkatakrishnan(k));
    vec![
        (
            "O1",
            GradientOptions {
                face_correction: true,
                ..d
            },
        ),
        (
            "O2",
            GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        ),
        (
            "O3",
            GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        ),
        (
            "O4",
            GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        ),
        (
            "O5",
            GradientOptions {
                secondary_limiter: venk(5.0),
                ..d
            },
        ),
        (
            "O6",
            GradientOptions {
                secondary_limiter: venk(0.5),
                ..d
            },
        ),
        (
            "C2",
            GradientOptions {
                face_correction: true,
                secondary_limiter: venk(5.0),
                boundary: BoundaryGradient::Corrected,
            },
        ),
    ]
}

/// K-B as ADR 0204's test builds it: the pressure conditions, the cell pressures, the face diffusivity and the cells with an
/// `xmax` face.
fn setup(geom: &Arc<FvGeometry>) -> (PressureBc, Vec<f64>, Vec<f64>, Vec<bool>) {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    let mut p = ScalarField::uniform("p", geom, 0.0);
    for patch in geom.mesh.patches() {
        if patch.name == "xmax" {
            u.set_bc(geom, &patch.name, Box::new(ZeroGradient)).unwrap();
            p.set_bc(geom, &patch.name, Box::new(FixedValue::scalar(0.0))).unwrap();
        } else {
            u.set_bc(geom, &patch.name, Box::new(FixedValue::vector([0.0; 3])))
                .unwrap();
        }
    }
    let pbc = pressure_bc(geom, &u, &p);
    let cc = geom.mesh.cell_centres();
    let pv: Vec<f64> = (0..geom.n_cells())
        .map(|c| {
            let x = cc[c];
            let s = if c % 2 == 0 { 1.0 } else { -1.0 };
            (1.3 * x[0] + 0.2).cos() * (0.9 * x[1] + 0.4).sin() * (1.1 * x[2]).cos() + 0.05 * s
        })
        .collect();
    let gamma: Vec<f64> = geom
        .mesh
        .face_centres()
        .iter()
        .map(|x| 1.0 + 0.3 * (2.1 * x[0] + 0.7 * x[1] - 0.4 * x[2]).sin())
        .collect();
    let mut beside = vec![false; geom.n_cells()];
    for f in geom.mesh.patch("xmax").expect("xmax").faces() {
        beside[geom.mesh.owner()[f]] = true;
    }
    (pbc, pv, gamma, beside)
}

/// The per-cell miss `|div(phi)_c - ((A p)_c - b_c)|` of the flux `phi` against the operator with `op`, split into the cells
/// with an `xmax` face and the others, and the flux scale `S` of `phi`.
fn identity(
    geom: &FvGeometry,
    pbc: &PressureBc,
    pv: &[f64],
    gamma: &[f64],
    beside: &[bool],
    op: Option<GradientOptions>,
    phi: &[f64],
) -> (f64, f64, f64) {
    let bv = boundary_values_pbc(geom, pbc, pv);
    let grad = least_squares(geom, pv, &bv);
    let r = laplacian_opt(geom, gamma, &pbc.coeffs, Some(&grad), pv, &bv, op).residual(pv);
    let div = divergence(geom, phi);
    let (mut a, mut o) = (0.0_f64, 0.0_f64);
    for c in 0..geom.n_cells() {
        let m = (div[c] + r[c]).abs();
        if beside[c] {
            a = a.max(m);
        } else {
            o = o.max(m);
        }
    }
    let s = (0..geom.n_cells())
        .map(|c| {
            geom.mesh
                .cell_faces(c)
                .iter()
                .filter(|&&f| !geom.empty[f])
                .map(|&f| phi[f].abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max);
    (a, o, s)
}

/// B-lib and B-lib-T on K-B.
#[test]
fn b_lib_flux_correction_carries_the_boundary_term_with_every_option_set() {
    let geom = mesh_k(8);
    let q = rcfd_mesh::quality::check_mesh(&geom.mesh);
    println!(
        "P: mesh K (n = 8) max non-orthogonality {:.3} degrees (at least 25)",
        q.max_non_orthogonality
    );
    assert!(q.max_non_orthogonality >= 25.0, "P set-up");
    let (pbc, pv, gamma, beside) = setup(&geom);
    assert!(beside.iter().any(|&b| b), "set-up: xmax has cells");
    let bv = boundary_values_pbc(&geom, &pbc, &pv);
    let grad = least_squares(&geom, &pv, &bv);
    let zero_f = vec![0.0; geom.mesh.n_faces()];
    let zero_ub: Vec<Vec3> = vec![[0.0; 3]; geom.mesh.n_boundary_faces()];
    let plain = corrected_flux_with(&geom, &gamma, &pbc, &zero_f, &pv, &zero_ub, Some(&grad));
    let mut missed = Vec::new();
    for (name, o) in sets() {
        let phi = corrected_flux_opt(&geom, &gamma, &pbc, &zero_f, &pv, &zero_ub, Some(&grad), Some(o));
        let (a, rest, s) = identity(&geom, &pbc, &pv, &gamma, &beside, Some(o), &phi);
        println!(
            "B-lib {name}: cells with a fixed-pressure face {:.3e} of S, the others {:.3e} (limit {IDENTITY:e}; S = {s:.4})",
            a / s,
            rest / s
        );
        if a > IDENTITY * s || rest > IDENTITY * s {
            missed.push(format!("B-lib {name}: {:e} {:e}", a / s, rest / s));
        }
        if name != "O3" {
            let (at, _, st) = identity(&geom, &pbc, &pv, &gamma, &beside, Some(o), &plain);
            println!(
                "B-lib-T {name}: without the options and the boundary term the cells with a fixed-pressure face miss by {:.3e} of S (limit > {TEETH:e})",
                at / st
            );
            if at <= TEETH * st {
                missed.push(format!("B-lib-T {name}: {:e}", at / st));
            }
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// B-lib-D: `None` is `corrected_flux_with`; the dropped treatment leaves the boundary faces of `corrected_flux_with`; options
/// without a gradient are `corrected_flux_with` without one.
#[test]
fn b_lib_d_no_options_dropped_and_no_gradient_change_no_bit() {
    let geom = mesh_k(8);
    let (pbc, pv, gamma, _) = setup(&geom);
    let bv = boundary_values_pbc(&geom, &pbc, &pv);
    let grad = least_squares(&geom, &pv, &bv);
    let cc = geom.mesh.cell_centres();
    let zero_ub: Vec<Vec3> = vec![[0.0; 3]; geom.mesh.n_boundary_faces()];
    let uc: Vec<Vec3> = cc
        .iter()
        .map(|x| [(1.1 * x[1]).sin(), (0.9 * x[2]).cos(), (0.7 * x[0]).sin()])
        .collect();
    let phi_hby_a = rcfd_fv::convection::face_flux(&geom, &uc, &zero_ub);
    let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let nb = geom.n_internal;
    let base = corrected_flux_with(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad));
    let none = corrected_flux_opt(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad), None);
    let o3 = sets().into_iter().find(|(n, _)| *n == "O3").unwrap().1;
    let dropped = corrected_flux_opt(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad), Some(o3));
    // B-lib-D2 (ADR 0221 amendment 2): the default options are `corrected_flux_with`, the fixed-pressure faces included
    let dflt = corrected_flux_opt(
        &geom,
        &gamma,
        &pbc,
        &phi_hby_a,
        &pv,
        &zero_ub,
        Some(&grad),
        Some(GradientOptions::default()),
    );
    let n4 = bits(&base, &dflt);
    println!(
        "B-lib-D2: the default options {n4} of {} faces differ in a bit (limit 0)",
        base.len()
    );
    assert_eq!(n4, 0, "B-lib-D2");
    let (_, c2) = sets().pop().unwrap();
    let base_ng = corrected_flux_with(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, None);
    let c2_ng = corrected_flux_opt(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, None, Some(c2));
    let (n1, n2, n3) = (
        bits(&base, &none),
        bits(&base[nb..], &dropped[nb..]),
        bits(&base_ng, &c2_ng),
    );
    println!(
        "B-lib-D: None {n1} of {} faces differ in a bit; O3 boundary {n2} of {} faces; C2 without a gradient {n3} of {} faces (limit 0)",
        base.len(),
        base.len() - nb,
        base.len()
    );
    // control: C2 with a gradient moves a boundary face of the fixed-pressure patch
    let c2_g = corrected_flux_opt(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad), Some(c2));
    let moved = base[nb..]
        .iter()
        .zip(&c2_g[nb..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("B-lib-D control: C2 with a gradient moves a boundary flux by {moved:.3e}");
    assert_eq!((n1, n2, n3), (0, 0, 0), "B-lib-D");
    assert!(moved > 0.0, "B-lib-D control");
}
