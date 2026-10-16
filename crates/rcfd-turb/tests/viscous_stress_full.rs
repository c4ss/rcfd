//! ADR 0032 A3 to A6: the Taylor-Green subgrid checks of ADR 0026 re-evaluated with the full viscous stress
//! (`PimpleSettings::full_viscous_stress`). The assertions of A3 and A5 are the ORIGINAL registered ones of ADR 0026 (TG1, TG3).

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

fn solver(geom: &Arc<FvGeometry>, full: bool) -> PimpleSolver {
    let u = VectorField::from_fn("U", geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut st = PimpleSettings::piso(NU);
    st.convection = ConvScheme::Central;
    st.full_viscous_stress = full;
    PimpleSolver::new(geom.clone(), u, p, st)
}

fn model(key: &str, geom: &Arc<FvGeometry>) -> Box<dyn TurbulenceModel> {
    create_model(key, &ctx(geom, NU, &[])).unwrap()
}

fn ke(s: &PimpleSolver) -> f64 {
    s.kinetic_energy() / s.geom.mesh.total_volume()
}

fn run(s: &mut PimpleSolver, mut m: Option<Box<dyn TurbulenceModel>>, steps: usize) -> (Vec<f64>, usize) {
    let mut k = vec![ke(s)];
    let mut up = 0;
    for _ in 0..steps {
        if let Some(m) = m.as_mut() {
            m.update(&s.u, &s.phi);
            s.nu_face = Some(m.nu_face());
        }
        s.step(DT);
        let e = ke(s);
        if e > k.last().unwrap() * (1.0 + 1e-14) {
            up += 1;
        }
        k.push(e);
    }
    (k, up)
}

/// Mean extra dissipation rate over the first 10 steps; the laminar reference is the Laplacian-form run (full stress with a
/// constant viscosity is the same to 1e-5, A2).
fn extra_rate(key: &str) -> f64 {
    let geom = geometry(16);
    let (kl, _) = run(&mut solver(&geom, false), None, 10);
    let (km, _) = run(&mut solver(&geom, true), Some(model(key, &geom)), 10);
    (kl[10] - km[10]) / (10.0 * DT)
}

fn decay_factor() -> f64 {
    let tau = 10.0 * DT;
    (1.0 - (-6.0 * NU * tau).exp()) / (6.0 * NU * tau)
}

#[test]
fn a3_tg1_original_assertion_smagorinsky_full_stress() {
    let delta = 2.0 * PI / 16.0;
    let meas = extra_rate("smagorinsky");
    let want = 1.44101 * (0.17 * delta).powi(2) * decay_factor();
    println!(
        "A3 Smagorinsky extra dissipation measured {meas:.5e}, closed form {want:.5e}, ratio {:.4}",
        meas / want
    );
    assert!((meas / want - 1.0).abs() <= 0.15);
}

#[test]
fn a4_wale_full_stress_closed_form() {
    let delta = 2.0 * PI / 16.0;
    let meas = extra_rate("wale");
    let want = 0.070217 * (0.325 * delta).powi(2) * decay_factor();
    println!(
        "A4 WALE extra dissipation measured {meas:.5e}, closed form {want:.5e}, ratio {:.4}",
        meas / want
    );
    assert!((meas / want - 1.0).abs() <= 0.15);
}

const STEPS: usize = 400;

struct Long {
    kl: Vec<f64>,
    ks: Vec<f64>,
    kw: Vec<f64>,
    ups: (usize, usize),
}

fn long() -> &'static Long {
    static L: OnceLock<Long> = OnceLock::new();
    L.get_or_init(|| {
        let geom = geometry(16);
        let (kl, _) = run(&mut solver(&geom, false), None, STEPS);
        let (ks, up_s) = run(&mut solver(&geom, true), Some(model("smagorinsky", &geom)), STEPS);
        let (kw, up_w) = run(&mut solver(&geom, true), Some(model("wale", &geom)), STEPS);
        Long {
            kl,
            ks,
            kw,
            ups: (up_s, up_w),
        }
    })
}

#[test]
fn a5_tg3_original_ordering_and_effect_size_full_stress() {
    let l = long();
    let (rs, rw) = (l.ks[STEPS] / l.kl[STEPS], l.kw[STEPS] / l.kl[STEPS]);
    println!(
        "A5 K(4)/K_lam(4): Smagorinsky {rs:.5} (limit 0.94), WALE {rw:.5} (limit 0.995), K_W/K_S {:.5} (original ordering K_W > K_S)",
        l.kw[STEPS] / l.ks[STEPS]
    );
    assert!(l.kw[STEPS] > l.ks[STEPS]);
    assert!(rs <= 0.94);
    assert!(rw <= 0.995);
}

#[test]
fn a6_no_energy_increase_with_full_stress() {
    let l = long();
    println!("A6 energy increases: Smagorinsky {}, WALE {}", l.ups.0, l.ups.1);
    assert_eq!(l.ups.0, 0);
    assert_eq!(l.ups.1, 0);
}
