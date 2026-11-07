//! F-291 (ADR 0204), library part of the pressure equation's gradient options. Limits are those registered in
//! docs/adr/0204-f291-registration-gradient-options-on-turbulence-momentum-pressure-and-the-electric-potential.md before this file
//! was written.
//!
//! P: mesh K (ADR 0016) reaches 25 degrees of non-orthogonality. P-lib: on the closed box K the flux correction with the options
//! (`corrected_flux_opt`) carries the face flux of the pressure operator assembled with the same options (`laplacian_opt`), per
//! cell to 1e-12 of the cell's flux scale. P-lib-T: the flux correction without the options misses that identity for every set
//! that changes internal faces. P-lib-D: `None` and `Some(GradientOptions::default())` give the bits of the existing functions.
//! P-lib-B (reported, Q3): with a fixed-pressure patch, the identity on the cells beside it and on the others.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::face_flux;
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::pressure::{
    boundary_values_pbc, corrected_flux_opt, corrected_flux_with, divergence, pressure_bc, solve_pressure_parts,
    solve_pressure_parts_opt, PressureBc, PressureControls,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::{LinearOptions, SolveControl};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use std::sync::Arc;

/// P-lib limit, relative to the flux scale `S`.
const IDENTITY: f64 = 1e-12;
/// P-lib-T limit: the smallest miss of the flux without the options.
const TEETH: f64 = 1e-9;

/// ADR 0016 mesh K at `n`: `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`.
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

/// The set-up K (closed box, every face Neumann for the pressure) or K-B (`xmax` also at a fixed pressure 0): the pressure
/// conditions, the cell pressures and the face diffusivity.
fn setup(geom: &Arc<FvGeometry>, open_xmax: bool) -> (PressureBc, Vec<f64>, Vec<f64>) {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    let mut p = ScalarField::uniform("p", geom, 0.0);
    for patch in geom.mesh.patches() {
        if open_xmax && patch.name == "xmax" {
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
    (pbc, pv, gamma)
}

/// Per cell `abs(div(phi)_c - ((A p)_c - b_c))` with `(A, b)` the pressure operator with `op` and `phi` the flux correction
/// (`phi_hby_a = 0`, `ub = 0`) with `flux` (`None`: `corrected_flux_with`, no options; `Some(o)`: `corrected_flux_opt` with
/// `o`); returns the per-cell misses and the flux scale `S` of that flux.
fn identity(
    geom: &FvGeometry,
    pbc: &PressureBc,
    pv: &[f64],
    gamma: &[f64],
    op: Option<GradientOptions>,
    flux: Option<Option<GradientOptions>>,
) -> (Vec<f64>, f64) {
    let bv = boundary_values_pbc(geom, pbc, pv);
    let grad = least_squares(geom, pv, &bv);
    let m = laplacian_opt(geom, gamma, &pbc.coeffs, Some(&grad), pv, &bv, op);
    let r = m.residual(pv); // b - A p
    let zero_f = vec![0.0; geom.mesh.n_faces()];
    let zero_ub: Vec<Vec3> = vec![[0.0; 3]; geom.mesh.n_boundary_faces()];
    let phi = match flux {
        None => corrected_flux_with(geom, gamma, pbc, &zero_f, pv, &zero_ub, Some(&grad)),
        Some(o) => corrected_flux_opt(geom, gamma, pbc, &zero_f, pv, &zero_ub, Some(&grad), o),
    };
    let div = divergence(geom, &phi);
    let mesh = &geom.mesh;
    let s = (0..geom.n_cells())
        .map(|c| {
            mesh.cell_faces(c)
                .iter()
                .filter(|&&f| !geom.empty[f])
                .map(|&f| phi[f].abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max);
    let miss = (0..geom.n_cells()).map(|c| (div[c] + r[c]).abs()).collect();
    (miss, s)
}

fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(0.0, f64::max)
}

/// P: the precondition on mesh K.
fn precondition(geom: &FvGeometry) {
    let q = rcfd_mesh::quality::check_mesh(&geom.mesh);
    println!(
        "P: mesh K (n = 8) max non-orthogonality {:.3} degrees (at least 25)",
        q.max_non_orthogonality
    );
    assert!(q.max_non_orthogonality >= 25.0, "P set-up: {}", q.max_non_orthogonality);
}

/// P-lib and P-lib-T on the closed box K.
#[test]
fn p_lib_flux_correction_carries_the_operator_flux_with_every_option_set() {
    let geom = mesh_k(8);
    precondition(&geom);
    let (pbc, pv, gamma) = setup(&geom, false);
    assert!(
        pbc.fixed_u.iter().all(|&f| f),
        "set-up: every boundary face is a fixed-velocity face"
    );
    let (m0, s0) = identity(&geom, &pbc, &pv, &gamma, None, Some(None));
    println!(
        "P-lib default: max miss {:.3e}, S = {s0:.4}, ratio {:.3e} (limit {IDENTITY:e})",
        max(&m0),
        max(&m0) / s0
    );
    assert!(max(&m0) <= IDENTITY * s0, "P-lib default");
    let mut missed = Vec::new();
    for (name, o) in sets() {
        let (m, s) = identity(&geom, &pbc, &pv, &gamma, Some(o), Some(Some(o)));
        let ratio = max(&m) / s;
        println!(
            "P-lib {name}: max miss {:.3e}, S = {s:.4}, ratio {ratio:.3e} (limit {IDENTITY:e})",
            max(&m)
        );
        if ratio > IDENTITY {
            missed.push(format!("P-lib {name}: {ratio:e}"));
        }
        if matches!(name, "O1" | "O4" | "O5" | "O6" | "C2") {
            let (mt, _) = identity(&geom, &pbc, &pv, &gamma, Some(o), None);
            let rt = max(&mt) / s;
            println!("P-lib-T {name}: the flux without the options misses by {rt:.3e} of S (limit > {TEETH:e})");
            if rt <= TEETH {
                missed.push(format!("P-lib-T {name}: {rt:e}"));
            }
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// P-lib-D: `None` and the default options give the bits of the existing flux correction and pressure solve.
#[test]
fn p_lib_d_no_options_and_default_options_change_no_bit() {
    let geom = mesh_k(8);
    let (pbc, pv, gamma) = setup(&geom, false);
    let bv = boundary_values_pbc(&geom, &pbc, &pv);
    let grad = least_squares(&geom, &pv, &bv);
    let zero_ub: Vec<Vec3> = vec![[0.0; 3]; geom.mesh.n_boundary_faces()];
    let cc = geom.mesh.cell_centres();
    let uc: Vec<Vec3> = cc
        .iter()
        .map(|x| [(1.1 * x[1]).sin(), (0.9 * x[2]).cos(), (0.7 * x[0]).sin()])
        .collect();
    let phi_hby_a = face_flux(&geom, &uc, &zero_ub);
    let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let base = corrected_flux_with(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad));
    for (label, o) in [("None", None), ("default", Some(GradientOptions::default()))] {
        let f = corrected_flux_opt(&geom, &gamma, &pbc, &phi_hby_a, &pv, &zero_ub, Some(&grad), o);
        let n = bits(&base, &f);
        println!(
            "P-lib-D corrected_flux_opt {label}: {n} of {} faces differ in a bit (limit 0)",
            f.len()
        );
        assert_eq!(n, 0, "P-lib-D flux {label}");
    }
    let ctl = PressureControls {
        p_ref: (0, 0.0),
        non_orth_correctors: 3,
        solver: SolveControl {
            tol_abs: 1e-300,
            tol_rel: 1e-10,
            max_iter: 2000,
        },
        amg: false,
        linear: LinearOptions::default(),
    };
    let mut p0 = vec![0.0; geom.n_cells()];
    let (r0, _, _) = solve_pressure_parts(&geom, &gamma, &pbc, &phi_hby_a, &mut p0, &ctl, false);
    for (label, o) in [("None", None), ("default", Some(GradientOptions::default()))] {
        let mut p1 = vec![0.0; geom.n_cells()];
        let (r1, _, _) = solve_pressure_parts_opt(&geom, &gamma, &pbc, &phi_hby_a, &mut p1, &ctl, false, None, o);
        let n = bits(&p0, &p1);
        println!(
            "P-lib-D solve_pressure_parts_opt {label}: {n} of {} cells differ in a bit (limit 0); residuals {r0:e} {r1:e}",
            p1.len()
        );
        assert_eq!(n, 0, "P-lib-D solve {label}");
        assert_eq!(r0.to_bits(), r1.to_bits(), "P-lib-D residual {label}");
    }
    // control: C2 changes the solve, so the bit comparisons above can fail
    let (_, c2) = sets().pop().unwrap();
    let mut p2 = vec![0.0; geom.n_cells()];
    solve_pressure_parts_opt(&geom, &gamma, &pbc, &phi_hby_a, &mut p2, &ctl, false, None, Some(c2));
    let d = p0.iter().zip(&p2).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!("P-lib-D control: C2 moves the pressure by {d:.3e}");
    assert!(d > 0.0, "P-lib-D control");
}

/// P-lib-B (reported, ADR 0204 Q3): with `xmax` at a fixed pressure, the identity on the cells with a fixed-pressure face and on
/// the others; the others are bound by P-lib's limit.
#[test]
fn p_lib_b_fixed_pressure_faces_reported() {
    let geom = mesh_k(8);
    let (pbc, pv, gamma) = setup(&geom, true);
    let beside: Vec<bool> = {
        let mut b = vec![false; geom.n_cells()];
        for f in geom.mesh.patch("xmax").expect("xmax").faces() {
            b[geom.mesh.owner()[f]] = true;
        }
        b
    };
    assert!(beside.iter().any(|&x| x), "set-up: xmax has cells");
    let split = |m: &[f64]| {
        let (mut a, mut o) = (0.0_f64, 0.0_f64);
        for (c, v) in m.iter().enumerate() {
            if beside[c] {
                a = a.max(*v);
            } else {
                o = o.max(*v);
            }
        }
        (a, o)
    };
    let mut missed = Vec::new();
    let all: Vec<(&str, Option<GradientOptions>)> = std::iter::once(("default", None))
        .chain(sets().into_iter().map(|(n, o)| (n, Some(o))))
        .collect();
    for (name, o) in all {
        let (m, s) = identity(&geom, &pbc, &pv, &gamma, o, Some(o));
        let (a, rest) = split(&m);
        println!(
            "P-lib-B {name}: cells with a fixed-pressure face {:.3e} of S (reported), the others {:.3e} (limit {IDENTITY:e})",
            a / s,
            rest / s
        );
        if rest / s > IDENTITY {
            missed.push(format!("P-lib-B {name}: {:e}", rest / s));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}
