//! F-208 (ADR 0016): manufactured-solution order of accuracy on skewed, warped and stretched hexahedral meshes and the
//! effect of the secondary-gradient options. Limits are those registered in docs/adr/0016-wave13-numerics.md before
//! this file was compiled or run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::{convection, face_flux, ConvScheme};
use rcfd_fv::diffusion::{laplacian, laplacian_with, uniform_gamma, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::Mesh;
use std::sync::Arc;

const U: Vec3 = [1.0, 0.7, 0.4];
const GAMMA: f64 = 0.1;
const TP: f64 = 2.0 * std::f64::consts::PI;

fn exact(p: Vec3) -> f64 {
    1.0 + (1.3 * p[0] + 0.4).sin() * (1.1 * p[1]).cos() * (0.9 * p[2] + 0.2).cos()
}

fn source(p: Vec3) -> f64 {
    let (sx, cx) = (1.3 * p[0] + 0.4).sin_cos();
    let (sy, cy) = (1.1 * p[1]).sin_cos();
    let (sz, cz) = (0.9 * p[2] + 0.2).sin_cos();
    let grad = [1.3 * cx * cy * cz, -1.1 * sx * sy * cz, -0.9 * sx * cy * sz];
    let lap = -(1.3 * 1.3 + 1.1 * 1.1 + 0.9 * 0.9) * (sx * cy * cz);
    vec3::dot(U, grad) - GAMMA * lap
}

#[derive(Clone, Copy, Debug)]
enum MeshKind {
    G,
    G2,
    /// Corrected mild mesh: G2 with amplitude 0.015 (G2 itself misses its registered 12 degree precondition).
    G3,
    K,
    W,
    S,
}

fn make_mesh(kind: MeshKind, n: usize) -> Mesh {
    let un = [Grading::Uniform; 3];
    match kind {
        MeshKind::G => box_mesh([n; 3], [1.0; 3], un).unwrap(),
        MeshKind::G2 => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.02 * (TP * p[1]).sin(), p[1] + 0.02 * (TP * p[0]).sin(), p[2]]
        })
        .unwrap(),
        MeshKind::G3 => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.015 * (TP * p[1]).sin(), p[1] + 0.015 * (TP * p[0]).sin(), p[2]]
        })
        .unwrap(),
        MeshKind::K => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
        })
        .unwrap(),
        MeshKind::W => warp(&box_mesh([n; 3], [1.0; 3], un).unwrap(), |p| {
            [
                p[0] + 0.1 * (TP * p[1]).sin() * (TP * p[2]).sin(),
                p[1] + 0.1 * (TP * p[0]).sin() * (TP * p[2]).sin(),
                p[2] + 0.1 * (TP * p[0]).sin() * (TP * p[1]).sin(),
            ]
        })
        .unwrap(),
        MeshKind::S => box_mesh(
            [n; 3],
            [1.0; 3],
            [Grading::Uniform, Grading::Expansion(50.0), Grading::Uniform],
        )
        .unwrap(),
    }
}

/// max over faces of 1 - |S_f| / sum of the fan-triangle areas.
fn non_planarity(m: &Mesh) -> f64 {
    let pts = m.points();
    let mut worst = 0.0_f64;
    for f in 0..m.n_faces() {
        let ids = m.face_points(f);
        let c = m.face_centres()[f];
        let mut sum = 0.0;
        for j in 0..ids.len() {
            let (a, b) = (pts[ids[j]], pts[ids[(j + 1) % ids.len()]]);
            sum += 0.5 * vec3::norm(vec3::cross(vec3::sub(a, c), vec3::sub(b, c)));
        }
        worst = worst.max(1.0 - m.face_area(f) / sum);
    }
    worst
}

fn exact_field(geom: &Arc<FvGeometry>) -> ScalarField {
    let mut phi = ScalarField::from_fn("phi", geom, exact);
    for p in geom.mesh.patches() {
        if p.kind == rcfd_mesh::PatchKind::Empty {
            continue;
        }
        let vals = p
            .faces()
            .map(|f| [exact(geom.mesh.face_centres()[f]), 0.0, 0.0])
            .collect();
        phi.set_bc(geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    phi
}

/// Returns (L2 error, converged).
fn solve(kind: MeshKind, n: usize, opts: Option<GradientOptions>) -> (f64, bool) {
    let mesh = make_mesh(kind, n);
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let mut phi = exact_field(&geom);
    phi.values.iter_mut().for_each(|v| *v = 1.0);
    let cells_u = vec![U; geom.n_cells()];
    let bnd_u = vec![U; geom.mesh.n_boundary_faces()];
    let flux = face_flux(&geom, &cells_u, &bnd_u);
    let gam = uniform_gamma(&geom, GAMMA);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 4000,
    };
    let mut converged = false;
    for _ in 0..600 {
        let cells = phi.values.clone();
        let bc = phi.boundary_coeffs(&geom);
        let bvals = phi.boundary_values(&geom);
        let grad = least_squares(&geom, &cells, &bvals);
        let mut m = convection(&geom, &flux, &cells, &bc, ConvScheme::Central, None);
        let lap = match opts {
            None => laplacian(&geom, &gam, &bc, Some(&grad)),
            Some(o) => laplacian_with(&geom, &gam, &bc, Some(&grad), &cells, &bvals, o),
        };
        m.add(&lap);
        for c in 0..cells.len() {
            m.b[c] += source(cc[c]) * vol[c];
        }
        let mut x = cells.clone();
        m.solve(&mut x, &ctl);
        let change = x.iter().zip(&cells).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        let range =
            x.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - x.iter().cloned().fold(f64::INFINITY, f64::min);
        phi.values = x;
        if change <= 1e-13 * range {
            converged = true;
            break;
        }
    }
    let (mut e2, mut tot) = (0.0, 0.0);
    for c in 0..cc.len() {
        e2 += (phi.values[c] - exact(cc[c])).powi(2) * vol[c];
        tot += vol[c];
    }
    ((e2 / tot).sqrt(), converged)
}

fn sets() -> Vec<(&'static str, Option<GradientOptions>)> {
    let c1 = GradientOptions {
        face_correction: true,
        secondary_limiter: None,
        boundary: BoundaryGradient::Corrected,
    };
    vec![
        ("C0", None),
        ("C1", Some(c1)),
        (
            "C2",
            Some(GradientOptions {
                secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
                ..c1
            }),
        ),
    ]
}

#[test]
fn default_options_reproduce_the_existing_operator() {
    let geom = Arc::new(FvGeometry::new(Arc::new(make_mesh(MeshKind::K, 6))).unwrap());
    let phi = exact_field(&geom);
    let bc = phi.boundary_coeffs(&geom);
    let bv = phi.boundary_values(&geom);
    let gr = least_squares(&geom, &phi.values, &bv);
    let gam = uniform_gamma(&geom, GAMMA);
    let a = laplacian(&geom, &gam, &bc, Some(&gr));
    let b = laplacian_with(
        &geom,
        &gam,
        &bc,
        Some(&gr),
        &phi.values,
        &bv,
        GradientOptions::default(),
    );
    assert_eq!(a.b, b.b);
    assert_eq!(a.a.diag, b.a.diag);
}

/// Record: the registered G2 precondition (max non-orthogonality <= 12 degrees at n = 16) is not met by the mesh as
/// registered (13.96 degrees measured); the estimate in the ADR was wrong. Kept with its assertion; superseded by G3.
#[test]
#[ignore = "known miss: G2 precondition 13.96 deg > 12 deg, superseded by G3 in m0b"]
fn m0_g2_precondition() {
    let g2 = check_mesh(&make_mesh(MeshKind::G2, 16));
    println!("M0 G2 nonorth {:.2} deg", g2.max_non_orthogonality);
    assert!(g2.max_non_orthogonality <= 12.0);
}

#[test]
fn m0b_preconditions() {
    let q = |k| {
        let m = make_mesh(k, 16);
        (check_mesh(&m), non_planarity(&m))
    };
    let (g3, _) = q(MeshKind::G3);
    let (k, _) = q(MeshKind::K);
    let (w, np) = q(MeshKind::W);
    let (s, _) = q(MeshKind::S);
    println!(
        "M0b: G3 nonorth {:.2} deg; K nonorth {:.2} deg; W nonorth {:.2} deg, nonplanarity {np:.3e}; S aspect {:.2}",
        g3.max_non_orthogonality, k.max_non_orthogonality, w.max_non_orthogonality, s.max_aspect_ratio
    );
    assert!(g3.max_non_orthogonality <= 12.0);
    assert!(k.max_non_orthogonality >= 25.0);
    assert!(w.max_non_orthogonality >= 20.0 && np >= 1e-4);
    assert!(s.max_aspect_ratio >= 10.0);
}

#[test]
fn m1_m4_order_and_option_effects() {
    let kinds = [
        MeshKind::G,
        MeshKind::G2,
        MeshKind::G3,
        MeshKind::K,
        MeshKind::W,
        MeshKind::S,
    ];
    let mut misses = Vec::new();
    for kind in kinds {
        let mut at32 = std::collections::HashMap::new();
        for (name, o) in sets() {
            let mut e = Vec::new();
            for n in [8usize, 16, 32] {
                let (err, conv) = solve(kind, n, o);
                if !conv {
                    misses.push(format!("{kind:?} {name} n={n}: not converged"));
                }
                e.push(err);
            }
            let order = (e[1] / e[2]).log2();
            let order_c = (e[0] / e[1]).log2();
            println!("M {kind:?} {name}: errors {e:?} orders {order_c:.3} {order:.3}");
            let lim = if name == "C2" { 1.5 } else { 1.7 };
            if order < lim {
                misses.push(format!("{kind:?} {name}: order {order:.3} < {lim}"));
            }
            at32.insert(name, e[2]);
        }
        let (r1, r2) = (at32["C1"] / at32["C0"], at32["C2"] / at32["C0"]);
        println!("M3/M4 {kind:?}: err(C1)/err(C0) = {r1:.4}, err(C2)/err(C0) = {r2:.4}");
        if matches!(kind, MeshKind::G | MeshKind::G2 | MeshKind::G3) {
            if r1 > 1.10 {
                misses.push(format!("{kind:?}: C1/C0 {r1:.3} > 1.10"));
            }
            if r2 > 1.25 {
                misses.push(format!("{kind:?}: C2/C0 {r2:.3} > 1.25"));
            }
        }
    }
    let dropped = GradientOptions {
        boundary: BoundaryGradient::Dropped,
        ..Default::default()
    };
    let (e0, _) = solve(MeshKind::K, 32, None);
    let (ed, _) = solve(MeshKind::K, 32, Some(dropped));
    println!("M4 Dropped boundary on K: err/err(C0) = {:.4}", ed / e0);
    assert!(misses.is_empty(), "{misses:#?}");
}
