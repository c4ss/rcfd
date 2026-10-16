//! ADR 0032 B1 to B6: SA-based DES / DDES (library level).

use rcfd_fv::bc::ZeroGradient;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use rcfd_turb::sa::{fv2, fw, r_function, s_tilde};
use rcfd_turb::{
    cell_delta, ddes_fd, ddes_length, ddes_rd, des_length, sa_local_source, DesDelta, DesMode, SLimiter,
    SaCoefficients, SaWallMode, SpalartAllmaras,
};
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

#[test]
fn b1_des_length_prescribed_values() {
    for (d, delta, c, want) in [(0.3, 0.2, 0.65, 0.13), (0.1, 0.2, 0.65, 0.1), (0.13, 0.2, 0.65, 0.13)] {
        let got = des_length(d, delta, c);
        println!("B1 d {d} Delta {delta}: {got:.16} (hand {want})");
        assert!(rel(got, want) <= 1e-12);
    }
}

#[test]
fn b2_ddes_shielding_hand_values() {
    let (nu, grad, d, kappa) = (1e-5, 100.0, 0.1, 0.41);
    // (nu_t, r_d, f_d, d~) computed outside the code with double-precision math.tanh
    let cases = [
        (1e-3, 0.006008328375966686, 0.9998889468350446, 0.06500388686077344),
        (2e-2, 0.11903628792385483, 0.30189679720847473, 0.0894336120977034),
        (0.1, 0.594943486020226, 0.0, 0.1),
    ];
    for (nut, rd_h, fd_h, dt_h) in cases {
        let rd = ddes_rd(nu, nut, grad, kappa, d);
        let fd = ddes_fd(rd);
        let dl = ddes_length(d, 0.1, 0.65, fd);
        println!("B2 nu_t {nut}: r_d {rd:.16e} f_d {fd:.16e} d~ {dl:.16e}");
        assert!(rel(rd, rd_h) <= 1e-12);
        assert!((fd - fd_h).abs() <= 1e-12 * fd_h.max(1.0));
        assert!(rel(dl, dt_h) <= 1e-12);
    }
}

#[test]
fn b3_delta_definitions() {
    // dx = 0.2, dy = 0.1, dz (thickness) = lx/nx = 0.2
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(10, 10, 2.0, 1.0, false, false).unwrap())).unwrap());
    let a = cell_delta(&geom, DesDelta::MaxExtent);
    let b = cell_delta(&geom, DesDelta::CbrtVolume);
    let want_b = (0.2f64 * 0.1 * 0.2).cbrt();
    let ea = a.iter().map(|&x| (x - 0.2).abs()).fold(0.0, f64::max);
    let eb = b.iter().map(|&x| (x - want_b).abs()).fold(0.0, f64::max);
    println!("B3 max_extent error {ea:.3e}, cbrt_volume error {eb:.3e}");
    assert!(ea <= 1e-12 && eb <= 1e-12);
}

const NU: f64 = 1e-3;
const DT: f64 = 0.02;

fn channel() -> (Arc<FvGeometry>, PimpleSolver) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(16, 24, 2.0, 1.0, true, false).unwrap())).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [4.0 * c[1] * (1.0 - c[1]), 0.0, 0.0]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(NU);
    st.convection = ConvScheme::Upwind;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.body_force = Some(BodyForce::uniform(&geom, [0.05, 0.0, 0.0]));
    (geom, s)
}

fn sa(geom: &Arc<FvGeometry>) -> SpalartAllmaras {
    let mut m = SpalartAllmaras::new(geom.clone(), NU, &["ymin", "ymax"], 3.0 * NU).unwrap();
    m.wall_mode = SaWallMode::Resolved;
    m
}

fn run_channel(mut m: SpalartAllmaras, steps: usize) -> (Vec<[f64; 3]>, Vec<f64>) {
    let (_, mut s) = channel();
    for _ in 0..steps {
        m.begin_step(DT);
        m.update(&s.u, &s.phi);
        s.nu_face = Some(m.nu_face());
        s.step(DT);
    }
    (s.u.values.clone(), m.nutilde.values.clone())
}

fn diff(a: &(Vec<[f64; 3]>, Vec<f64>), b: &(Vec<[f64; 3]>, Vec<f64>)) -> (f64, f64) {
    let du =
        a.0.iter()
            .zip(&b.0)
            .map(|(x, y)| (0..3).map(|k| (x[k] - y[k]).abs()).fold(0.0, f64::max))
            .fold(0.0, f64::max);
    let dn = a.1.iter().zip(&b.1).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    (du, dn)
}

#[test]
fn b4_rans_limit_equals_sa() {
    let (geom, _) = channel();
    let base = run_channel(sa(&geom), 30);
    assert!(base.1.iter().cloned().fold(0.0, f64::max) > 0.0);
    for mode in [DesMode::Des, DesMode::Ddes] {
        let mut m = sa(&geom);
        m.des = mode;
        m.c_des = 1000.0;
        let r = run_channel(m, 30);
        let (du, dn) = diff(&base, &r);
        println!("B4 {mode:?}: max |du| {du:.3e}, max |dnuTilda| {dn:.3e} (limit 1e-12)");
        assert!(du <= 1e-12 && dn <= 1e-12);
    }
    // teeth: with a small C_DES the DES run differs
    let mut m = sa(&geom);
    m.des = DesMode::Des;
    m.c_des = 0.05;
    let r = run_channel(m, 30);
    let (du, dn) = diff(&base, &r);
    println!("B4 teeth (C_DES 0.05): max |du| {du:.3e}, max |dnuTilda| {dn:.3e}");
    assert!(dn > 1e-9);
}

#[test]
fn b5_les_limit_fixed_point_scales_with_length_squared() {
    let cf = SaCoefficients::default();
    let (omega, nu) = (10.0, 1e-14);
    let root = |d: f64| -> f64 {
        let f = |x: f64| sa_local_source(x, omega, d, nu, &cf, SLimiter::Allmaras);
        let (mut lo, mut hi) = (1e-6 * d * d * omega, 1e3 * d * d * omega);
        assert!(f(lo) > 0.0 && f(hi) < 0.0);
        for _ in 0..300 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let (x1, x2) = (root(0.01), root(0.02));
    let s1 = sa_local_source(x1, omega, 0.01, nu, &cf, SLimiter::Allmaras);
    let scale = cf.cb1 * omega * x1;
    println!(
        "B5 nuT*(0.01) {x1:.6e}, nuT*(0.02) {x2:.6e}, ratio {:.8}, |source|/scale {:.3e}",
        x2 / x1,
        s1.abs() / scale
    );
    assert!(s1.abs() <= 1e-10 * scale);
    assert!((x2 / x1 - 4.0).abs() <= 4e-3);
}

/// B6. The first draft used the RANS length (non-uniform `d`), whose cell-to-cell differences couple through the implicit
/// diffusion (measured 1.3e-4); the registered text asks for a uniform field, so the corrected test uses DES with
/// `C_DES Delta` below the smallest wall distance (uniform `d~`) and zero-gradient boundaries.
#[test]
fn b6_time_derivative_hand_evaluated_implicit_euler_step() {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(16, 24, 2.0, 1.0, false, false).unwrap())).unwrap());
    let nu = NU;
    let x0 = 1e-3;
    let dt = 0.1;
    let mut m = SpalartAllmaras::new(geom.clone(), nu, &["ymin", "ymax"], x0).unwrap();
    m.wall_mode = SaWallMode::Resolved;
    m.solver.tol_rel = 1e-14;
    for w in ["ymin", "ymax"] {
        m.nutilde.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    m.des = DesMode::Des;
    m.des_delta = DesDelta::MaxExtent;
    m.c_des = 0.1;
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = vec![0.0; geom.mesh.n_faces()];
    m.begin_step(dt);
    m.update(&u, &phi);
    let delta = cell_delta(&geom, DesDelta::MaxExtent);
    let cf = m.coef;
    let mut worst: f64 = 0.0;
    for c in 0..geom.n_cells() {
        let len = des_length(m.wall_distance()[c], delta[c], 0.1);
        assert!((len - 0.1 * 0.125).abs() <= 1e-12, "d~ not uniform: {len}");
        let chi = x0 / nu;
        let sb = x0 * fv2(chi, cf.cv1) / (cf.kappa * cf.kappa * len * len);
        let st = s_tilde(0.0, sb, SLimiter::Allmaras, cf.c2, cf.c3);
        let fwv = fw(r_function(x0, st, cf.kappa, len), cf.cw2, cf.cw3);
        let want = (cf.cb1 * st.max(0.0) * x0 + x0 / dt) / (1.0 / dt + cf.cw1() * fwv * x0 / (len * len));
        worst = worst.max(rel(m.nutilde.values[c], want));
    }
    println!(
        "B6: {} cells, worst relative error {worst:.3e} (limit 1e-10)",
        geom.n_cells()
    );
    assert!(worst <= 1e-10);
    // teeth: the update is not the old value
    assert!(rel(m.nutilde.values[0], x0) > 1e-3);
}
