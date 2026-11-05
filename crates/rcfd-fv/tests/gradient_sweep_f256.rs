//! F-256 (ADR 0190) G4 (iv), library part: the scalar sweep with `SweepExtras::gradient = Some(GradientOptions::default())`
//! equals the sweep without options (`None`, the operator of every existing case) bit for bit, on ADR 0016's skewed mesh K.

use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::{uniform_gamma, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::Limiter;
use rcfd_fv::scalar::{sweep_ext, SweepExtras, SweepInputs};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use std::sync::Arc;

/// ADR 0016 mesh K at `n`: `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`.
fn mesh_k(n: usize) -> Arc<FvGeometry> {
    let m = warp(&box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap(), |p| {
        [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
    })
    .unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// Dirichlet values on every boundary from a smooth function, uniform 1 inside.
fn field(geom: &FvGeometry) -> ScalarField {
    let f = |p: [f64; 3]| 1.0 + (1.3 * p[0] + 0.4).sin() * (1.1 * p[1]).cos() * (0.9 * p[2] + 0.2).cos();
    let mut s = ScalarField::uniform("phi", geom, 1.0);
    for p in geom.mesh.patches() {
        let vals = p
            .faces()
            .map(|face| [f(geom.mesh.face_centres()[face]), 0.0, 0.0])
            .collect();
        s.set_bc(geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    s
}

/// Five sweeps of `-div(0.1 grad phi) = 0` with `LinearUpwind` on a zero flux (so the gradient and the secondary flux exist).
fn sweeps(geom: &FvGeometry, gradient: Option<GradientOptions>) -> Vec<f64> {
    let mut s = field(geom);
    let zero = vec![0.0; geom.mesh.n_faces()];
    let gamma = uniform_gamma(geom, 0.1);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-12,
        max_iter: 2000,
    };
    for _ in 0..5 {
        sweep_ext(
            geom,
            &mut s,
            &SweepInputs {
                phi: &zero,
                gamma: &gamma,
                scheme: ConvScheme::LinearUpwind,
                relax: 0.9,
                source: None,
                solver: &ctl,
                ddt: None,
            },
            &SweepExtras {
                gradient,
                ..Default::default()
            },
        );
    }
    s.values
}

#[test]
fn g4_iv_default_options_equal_no_options_bit_for_bit() {
    let geom = mesh_k(8);
    let none = sweeps(&geom, None);
    let default = sweeps(&geom, Some(GradientOptions::default()));
    let differ = none
        .iter()
        .zip(&default)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    println!("G4 (iv): {} cells, {differ} differ in a bit (limit 0)", none.len());
    assert_eq!(differ, 0);
    // control: ADR 0016's option set C2 changes the result, so the comparison above can fail
    let c2 = sweeps(
        &geom,
        Some(GradientOptions {
            face_correction: true,
            secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
            boundary: BoundaryGradient::Corrected,
        }),
    );
    let d = none.iter().zip(&c2).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!("G4 (iv) control: C2 differs from no options by {d:.3e}");
    assert!(d > 0.0);
}
