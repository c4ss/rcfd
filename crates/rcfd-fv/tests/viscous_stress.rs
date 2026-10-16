//! ADR 0032 A1, A2, A7: the transposed viscous-stress term (library level).

use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{transposed_stress_source, PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

#[test]
fn a1_operator_is_exact_for_linear_fields() {
    let (nx, ny, lx, ly) = (14usize, 12usize, 1.4, 1.2);
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap())).unwrap());
    let g = [[0.3, -0.7, 0.0], [0.5, 0.2, 0.0], [0.0; 3]];
    let exact = |x: [f64; 3]| -> [f64; 3] { [0, 1, 2].map(|j| (0..3).map(|k| g[j][k] * x[k]).sum::<f64>()) };
    let mut u = VectorField::from_fn("U", &geom, exact);
    let fc = geom.mesh.face_centres();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        let vals: Vec<[f64; 3]> = geom.mesh.patch(p).unwrap().faces().map(|f| exact(fc[f])).collect();
        u.set_bc(&geom, p, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    let nu_of = |x: [f64; 3]| 1.0 + 0.4 * x[0] + 0.25 * x[1];
    let nu_face: Vec<f64> = (0..geom.mesh.n_faces()).map(|f| nu_of(fc[f])).collect();
    let s = transposed_stress_source(&geom, &u, &nu_face);
    let cc = geom.mesh.cell_centres();
    let dnu = [0.4, 0.25, 0.0];
    let (hx, hy) = (lx / nx as f64, ly / ny as f64);
    let mut err: f64 = 0.0;
    let mut scale: f64 = 0.0;
    let mut count = 0;
    for c in 0..geom.n_cells() {
        let x = cc[c];
        // interior: at least two cells away from every wall
        if x[0] < 2.0 * hx || x[0] > lx - 2.0 * hx || x[1] < 2.0 * hy || x[1] > ly - 2.0 * hy {
            continue;
        }
        count += 1;
        // S_i = G_ji d_j nu
        for i in 0..3 {
            let want: f64 = (0..3).map(|j| g[j][i] * dnu[j]).sum();
            err = err.max((s[c][i] - want).abs());
            scale = scale.max(want.abs());
        }
    }
    println!("A1 cells {count}, max error {err:.3e} (scale {scale:.3e})");
    assert!(count > 50);
    assert!(err <= 1e-12 * scale);
}

fn solver(full: bool) -> PimpleSolver {
    solver_n(full, 16)
}

fn solver_n(full: bool, n: usize) -> PimpleSolver {
    let l = 2.0 * PI;
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, l, l, true, true).unwrap())).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut st = PimpleSettings::piso(0.05);
    st.convection = ConvScheme::Central;
    st.full_viscous_stress = full;
    PimpleSolver::new(geom, u, p, st)
}

/// Registered A2 (limit 0.02). The first draft of the source skipped the periodic (coupled) faces and measured 0.0595 (an
/// implementation defect, found by the n-scaling diagnostic and fixed before any commit; see the ADR 0032 addendum).
#[test]
fn a2_constant_viscosity_full_stress_is_small() {
    let mut lap = solver(false);
    let mut full = solver(true);
    let k0 = lap.kinetic_energy();
    for _ in 0..10 {
        lap.step(0.01);
        full.step(0.01);
    }
    let (kl, kf) = (lap.kinetic_energy(), full.kinetic_energy());
    let r = (kf - kl).abs() / (k0 - kl);
    println!("A2 K0 {k0:.6e}, K_lap {kl:.6e}, K_full {kf:.6e}, ratio {r:.3e} (limit 0.02)");
    assert!(r <= 0.02);
}

#[test]
fn a7_default_is_off_and_explicit_false_is_bit_identical() {
    assert!(!PimpleSettings::piso(0.1).full_viscous_stress);
    assert!(!PimpleSettings::pimple(0.1, 3).full_viscous_stress);
    let mut a = solver(false);
    let mut b = {
        let mut s = solver(false);
        s.settings.full_viscous_stress = false;
        s
    };
    for _ in 0..10 {
        a.step(0.01);
        b.step(0.01);
    }
    assert!(a.u.values == b.u.values && a.p.values == b.p.values);
}

fn a2_ratio(n: usize) -> f64 {
    let mut lap = solver_n(false, n);
    let mut full = solver_n(true, n);
    let k0 = lap.kinetic_energy();
    for _ in 0..10 {
        lap.step(0.01);
        full.step(0.01);
    }
    let (kl, kf) = (lap.kinetic_energy(), full.kinetic_energy());
    (kf - kl).abs() / (k0 - kl)
}

/// A2b (added after the A2 miss, limit fixed by the diagnosis before this run): the A2 ratio is a discretisation error of the
/// zero continuum term, so it must fall at least by a factor 3 from n = 16 to n = 32 (second order would give 4). Added after the draft-implementation miss.
#[test]
fn a2b_constant_viscosity_discrepancy_is_a_discretisation_error() {
    let (r16, r32) = (a2_ratio(16), a2_ratio(32));
    println!("A2b ratio n=16 {r16:.4e}, n=32 {r32:.4e}, reduction {:.3}", r16 / r32);
    assert!(r16 / r32 >= 3.0);
}
