//! F-535 (ADR 0471): the opt-in refusal of the first PISO step at the pressure-lag onset (`PimpleSettings::lag_onset_refusal`),
//! naming the largest cell diffusion number. Checks L1 to L5 of the registration; the limits were fixed in ADR 0471 before this
//! file existed. The fast channel FQ is that of ADR 0451 (copied, 30 x 16 cells across a gap of `h = 0.02`, `Wo = 1`).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const H: f64 = 0.02;
const OMEGA: f64 = 2.0 * PI;

fn nu() -> f64 {
    OMEGA * (H / 2.0).powi(2)
}

fn q0() -> f64 {
    1e-3 * H
}

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu());
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s
}

fn fq(s: PimpleSettings) -> PimpleSolver {
    let mesh = box_mesh(
        [30, 16, 1],
        [20.0 * H, H, H],
        [Grading::Expansion(5.5), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, s)
}

fn inflow(n: usize, dt: f64) -> f64 {
    let q = |m: usize| q0() * (OMEGA * m as f64 * dt).sin();
    (q(n) - q(n - 1)) / dt
}

fn step(fl: &mut PimpleSolver, n: usize, dt: f64) {
    let g = fl.geom.clone();
    fl.u.set_bc(&g, "xmin", Box::new(FixedValue::vector([inflow(n, dt), 0.0, 0.0])))
        .unwrap();
    fl.step(dt);
}

fn dn(n_steps: usize) -> f64 {
    nu() * (1.0 / n_steps as f64) / (H / 16.0).powi(2)
}

fn same(a: &[Vec3], b: &[Vec3]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (0..3).all(|k| x[k].to_bits() == y[k].to_bits()))
}

fn same_s(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn l5_default_is_off() {
    assert!(PimpleSettings::piso(1.0).lag_onset_refusal.is_none());
}

#[test]
fn l1_fq_refused_at_step_one_with_the_state_as_it_was() {
    let n = 16;
    let dt = 1.0 / n as f64;
    let mut s = settings();
    s.lag_onset_refusal = Some(20);
    let mut fl = fq(s);
    let (u0, p0, phi0) = (fl.u.values.clone(), fl.p.values.clone(), fl.phi.clone());
    step(&mut fl, 1, dt);
    let d = fl.diverged.as_ref().expect("refused at step 1");
    println!("L1: rho {:e}, diffusion {:e}", d.value, d.diffusion);
    assert_eq!(d.quantity, "lag onset");
    assert_eq!(d.step, 1);
    assert!(d.value >= 1.0, "estimate {}", d.value);
    assert!(
        (d.diffusion / dn(n) - 1.0).abs() <= 1e-9,
        "diffusion {} against {}",
        d.diffusion,
        dn(n)
    );
    let info = d.info;
    let msg = d.message(None);
    assert!(
        msg.contains(&format!("{:.3}", d.diffusion)) && msg.contains("nu dt/dx^2"),
        "{msg}"
    );
    assert!(same(&fl.u.values, &u0) && same_s(&fl.p.values, &p0) && same_s(&fl.phi, &phi0));
    assert_eq!(fl.time, 0.0);
    assert!(fl.history.is_empty() && fl.lag_estimates.is_empty() && fl.lag_warnings.is_empty());
    for k in 2..5 {
        step(&mut fl, k, dt);
        assert_eq!(fl.step(dt).step, info.step);
        assert!(same(&fl.u.values, &u0) && same_s(&fl.p.values, &p0) && same_s(&fl.phi, &phi0));
    }
}

#[test]
fn l2_fq_below_the_onset_equals_the_run_without_the_option() {
    let n = 400;
    let dt = 1.0 / n as f64;
    let mut s = settings();
    s.lag_onset_refusal = Some(20);
    let (mut a, mut b) = (fq(s), fq(settings()));
    for k in 1..=5 {
        step(&mut a, k, dt);
        step(&mut b, k, dt);
        assert!(
            a.diverged.is_none(),
            "step {k}: {:?}",
            a.diverged.as_ref().map(|d| d.message(None))
        );
        assert!(same(&a.u.values, &b.u.values) && same_s(&a.p.values, &b.p.values) && same_s(&a.phi, &b.phi));
        assert_eq!(a.time.to_bits(), b.time.to_bits());
    }
}

#[test]
fn l3_diffusion_number_of_a_2d_cavity_leaves_out_the_empty_planes() {
    let n = 16;
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(n, 1.0).unwrap())).unwrap());
    let u = VectorField::uniform("U", &geom, vec3::ZERO);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let fl = PimpleSolver::new(geom, u, p, PimpleSettings::piso(1e-3));
    let (d, cell) = fl.max_cell_diffusion(0.1);
    let expect = 1e-3 * 0.1 * (n * n) as f64;
    println!("L3: {d:e} against {expect:e} (cell {cell})");
    assert!((d / expect - 1.0).abs() <= 1e-9);
}

#[test]
fn l4_fractional_step_is_not_refused() {
    let n = 16;
    let dt = 1.0 / n as f64;
    let mut s = settings();
    s.fractional = Some(FractionalSettings::default());
    let mut t = s.clone();
    t.lag_onset_refusal = Some(20);
    let (mut a, mut b) = (fq(t), fq(s));
    step(&mut a, 1, dt);
    step(&mut b, 1, dt);
    assert!(a.diverged.as_ref().is_none_or(|d| d.quantity != "lag onset"));
    assert!(same(&a.u.values, &b.u.values) && same_s(&a.p.values, &b.p.values) && same_s(&a.phi, &b.phi));
}

#[test]
fn l3b_diffusion_number_of_a_thin_2d_box_leaves_out_the_empty_planes() {
    use rcfd_mesh::PatchKind;
    let m = box_mesh([8, 8, 1], [1.0, 1.0, 0.01], [Grading::Uniform; 3]).unwrap();
    let mut topo = m.topology().clone();
    for p in topo.patches.iter_mut() {
        if p.name == "zmin" || p.name == "zmax" {
            p.kind = PatchKind::Empty;
        }
    }
    let geom = Arc::new(FvGeometry::new(Arc::new(rcfd_mesh::Mesh::from_topology(topo).unwrap())).unwrap());
    assert_eq!(geom.active, [true, true, false]);
    let u = VectorField::uniform("U", &geom, vec3::ZERO);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let fl = PimpleSolver::new(geom, u, p, PimpleSettings::piso(1e-3));
    let (d, _) = fl.max_cell_diffusion(0.1);
    let expect = 1e-3 * 0.1 * 64.0;
    println!("L3b: {d:e} against {expect:e}");
    assert!((d / expect - 1.0).abs() <= 1e-9);
}
