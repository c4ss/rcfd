//! ADR 0026 TG: Taylor-Green vortex with the subgrid models (library level, PimpleSolver with the model updated every step).

mod common;

use common::sgs::ctx;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use rcfd_turb::{create_model, TurbulenceModel};
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

const NU: f64 = 0.05;
const DT: f64 = 0.01;

fn geometry(n: usize) -> Arc<FvGeometry> {
    let l = 2.0 * PI;
    Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, l, l, true, true).unwrap())).unwrap())
}

fn solver(geom: &Arc<FvGeometry>) -> PimpleSolver {
    let u = VectorField::from_fn("U", geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut st = PimpleSettings::piso(NU);
    st.convection = ConvScheme::Central;
    PimpleSolver::new(geom.clone(), u, p, st)
}

fn model(key: &str, geom: &Arc<FvGeometry>, cs: Option<f64>) -> Box<dyn TurbulenceModel> {
    let mut m = create_model(key, &ctx(geom, NU, &[])).unwrap();
    if let Some(c) = cs {
        m.set_coefficient("cs", c).unwrap();
    }
    m
}

fn ke(s: &PimpleSolver) -> f64 {
    s.kinetic_energy() / s.geom.mesh.total_volume()
}

/// Run `steps` steps; returns the mean kinetic energy after each step (index 0 = initial), the minimum of the eddy
/// viscosity seen over the run, and the number of increases of the energy.
fn run(s: &mut PimpleSolver, mut m: Option<Box<dyn TurbulenceModel>>, steps: usize) -> (Vec<f64>, f64, usize) {
    let mut k = vec![ke(s)];
    let mut nut_min = f64::INFINITY;
    let mut up = 0;
    for _ in 0..steps {
        if let Some(m) = m.as_mut() {
            m.update(&s.u, &s.phi);
            s.nu_face = Some(m.nu_face());
            nut_min = nut_min.min(m.nut().iter().cloned().fold(f64::INFINITY, f64::min));
        }
        s.step(DT);
        let e = ke(s);
        if e > k.last().unwrap() * (1.0 + 1e-14) {
            up += 1;
        }
        k.push(e);
    }
    (k, nut_min, up)
}

fn initial_field(n: usize) -> (Arc<FvGeometry>, VectorField) {
    let geom = geometry(n);
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    (geom, u)
}

#[test]
fn tg0_initial_eddy_viscosity() {
    let (geom, u) = initial_field(32);
    let delta = 2.0 * PI / 32.0;
    let mut ms = model("smagorinsky", &geom, None);
    ms.update(&u, &[]);
    let n = ms.nut().len() as f64;
    let mean = ms.nut().iter().sum::<f64>() / n;
    let max = ms.nut().iter().cloned().fold(0.0, f64::max);
    let want = (0.17 * delta).powi(2) * 2.0 * (2.0 / PI).powi(2);
    let top = 2.0 * (0.17 * delta).powi(2);
    println!(
        "TG0 Smagorinsky: mean {mean:.4e} vs {want:.4e} ({:+.2}%), max/top {:.4}",
        100.0 * (mean / want - 1.0),
        max / top
    );
    assert!((mean / want - 1.0).abs() <= 0.05);
    assert!(max >= 0.9 * top && max <= top);
    assert!(ms.nut().iter().all(|&v| v >= 0.0));
    let mut mw = model("wale", &geom, None);
    mw.update(&u, &[]);
    let mean = mw.nut().iter().sum::<f64>() / n;
    let want = (0.325 * delta).powi(2) * 0.248403;
    println!(
        "TG0 WALE: mean {mean:.4e} vs {want:.4e} ({:+.2}%)",
        100.0 * (mean / want - 1.0)
    );
    assert!((mean / want - 1.0).abs() <= 0.08);
    assert!(mw.nut().iter().all(|&v| v >= 0.0));
}

/// Measured mean extra dissipation rate over the first 10 steps: `(K_laminar - K_model)/tau`.
fn extra_rate(key: &str) -> f64 {
    let geom = geometry(16);
    let mut sl = solver(&geom);
    let (kl, _, _) = run(&mut sl, None, 10);
    let mut sm = solver(&geom);
    let (km, _, _) = run(&mut sm, Some(model(key, &geom, None)), 10);
    (kl[10] - km[10]) / (10.0 * DT)
}

fn decay_factor() -> f64 {
    let tau = 10.0 * DT;
    (1.0 - (-6.0 * NU * tau).exp()) / (6.0 * NU * tau)
}

/// Registered TG1 (ADR 0026, closed form for the full stress divergence `2 nu_sgs S:S`). Missed, see addendum A1.
#[test]
#[ignore = "known miss: the solver's viscous term is the Laplacian form, ADR 0026 addendum A1"]
fn tg1_initial_extra_dissipation_known_miss() {
    let delta = 2.0 * PI / 16.0;
    let meas = extra_rate("smagorinsky");
    let want = 1.44101 * (0.17 * delta).powi(2) * decay_factor();
    println!(
        "TG1 extra dissipation rate measured {meas:.5e}, closed form {want:.5e}, ratio {:.4}",
        meas / want
    );
    assert!((meas / want - 1.0).abs() <= 0.15);
}

#[test]
fn tg1b_extra_dissipation_laplacian_form_smagorinsky() {
    let delta = 2.0 * PI / 16.0;
    let meas = extra_rate("smagorinsky");
    let want = 0.900633 * (0.17 * delta).powi(2) * decay_factor();
    println!(
        "TG1b Smagorinsky extra dissipation rate measured {meas:.5e}, closed form {want:.5e}, ratio {:.4}",
        meas / want
    );
    assert!((meas / want - 1.0).abs() <= 0.15);
}

#[test]
fn tg1c_extra_dissipation_laplacian_form_wale() {
    let delta = 2.0 * PI / 16.0;
    let meas = extra_rate("wale");
    let want = 0.313661 * (0.325 * delta).powi(2) * decay_factor();
    println!(
        "TG1c WALE extra dissipation rate measured {meas:.5e}, closed form {want:.5e}, ratio {:.4}",
        meas / want
    );
    assert!((meas / want - 1.0).abs() <= 0.15);
}

struct Long {
    kl: Vec<f64>,
    ks: Vec<f64>,
    kw: Vec<f64>,
    nmin: (f64, f64),
    ups: (usize, usize),
    cs0_diff: f64,
}

const STEPS: usize = 400;

fn long() -> &'static Long {
    static L: OnceLock<Long> = OnceLock::new();
    L.get_or_init(|| {
        let geom = geometry(16);
        let mut sl = solver(&geom);
        let (kl, _, _) = run(&mut sl, None, STEPS);
        let mut ss = solver(&geom);
        let (ks, nmin_s, up_s) = run(&mut ss, Some(model("smagorinsky", &geom, None)), STEPS);
        let mut sw = solver(&geom);
        let (kw, nmin_w, up_w) = run(&mut sw, Some(model("wale", &geom, None)), STEPS);
        let mut sz = solver(&geom);
        run(&mut sz, Some(model("smagorinsky", &geom, Some(0.0))), 100);
        let mut sl100 = solver(&geom);
        run(&mut sl100, None, 100);
        let cs0_diff =
            sz.u.values
                .iter()
                .zip(&sl100.u.values)
                .map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max))
                .fold(0.0, f64::max);
        Long {
            kl,
            ks,
            kw,
            nmin: (nmin_s, nmin_w),
            ups: (up_s, up_w),
            cs0_diff,
        }
    })
}

#[test]
fn tg2_sign_and_monotone_decay() {
    let l = long();
    println!(
        "TG2 energy increases: Smagorinsky {}, WALE {}; min nut {:.3e} {:.3e}",
        l.ups.0, l.ups.1, l.nmin.0, l.nmin.1
    );
    assert_eq!(l.ups.0, 0);
    assert_eq!(l.ups.1, 0);
    assert!(l.nmin.0 >= 0.0 && l.nmin.1 >= 0.0);
    assert!(l.ks.iter().chain(&l.kw).all(|&e| e > 0.0));
    assert!(
        (1..=STEPS).all(|k| l.ks[k] < l.kl[k]),
        "Smagorinsky not below laminar at every step"
    );
    assert!(
        (1..=STEPS).all(|k| l.kw[k] < l.kl[k]),
        "WALE not below laminar at every step"
    );
}

#[test]
fn tg3_effect_size() {
    let l = long();
    let (rs, rw) = (l.ks[STEPS] / l.kl[STEPS], l.kw[STEPS] / l.kl[STEPS]);
    println!(
        "TG3 K(4)/K_lam(4): Smagorinsky {rs:.5} (limit 0.97; the closed-form prediction 0.920 assumed the full stress divergence), WALE {rw:.5} (limit 0.995, prediction 0.982), K_W/K_S {:.5}",
        l.kw[STEPS] / l.ks[STEPS]
    );
    assert!(rs <= 0.97);
    assert!(rw <= 0.995);
}

/// Registered ordering of TG3 (WALE dissipates less than Smagorinsky). Missed, see addendum A1.
#[test]
#[ignore = "known miss: the solver's viscous term is the Laplacian form, ADR 0026 addendum A1"]
fn tg3_wale_dissipates_less_than_smagorinsky_known_miss() {
    let l = long();
    assert!(l.kw[STEPS] > l.ks[STEPS]);
}

#[test]
fn tg3b_wale_dissipates_more_than_smagorinsky_with_the_laplacian_form() {
    let l = long();
    assert!(l.kw[STEPS] < l.ks[STEPS]);
}

#[test]
fn tg4_zero_constant_adds_nothing() {
    let d = long().cs0_diff;
    println!("TG4 cs = 0 against laminar: {d:.3e}");
    assert!(d <= 1e-13);
}
