//! F-651 (ADR 0578): the Arrhenius creep factor at every non-positive temperature (NP1 at the factor and the point update,
//! NP2 at `nl_transient::Integrator::new`, NP3 at `solve_in_time_with`), the integrator's refusal of a factor material without
//! a temperature option (NI1), their positive side (PC1), and the creep rate of one Hex8 of the factor material under a step
//! load in the nonlinear transient integrator after the vibration has decayed (CR1, CR2, DV, CRr). B0/B1 is
//! `sa_creep_arrhenius_nlt_default_bits.rs`.

use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_creep::{arrhenius_factor, update};
use rcfd_fem::material::PointContext;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_in_time_with, NewtonOptions, TimeLoading, TimeSteppingOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable, Method, Rayleigh};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use std::f64::consts::PI;

const E: f64 = 2.0e9;
const NU: f64 = 0.3;
const RHO: f64 = 1200.0;
/// The activation energy of the checks, J/mol.
const Q: f64 = 2.0e5;
const T1: f64 = 700.0;
const T2: f64 = 750.0;
/// The gas constant of the test (its own literal, J/(mol K)).
const R: f64 = 8.314_462_618;
/// The traction of the step load, Pa (D3).
const SIGMA: f64 = 2.0e3;
const N: f64 = 3.0;

/// The factor of the test.
fn f(t: f64) -> f64 {
    (-(Q / (R * t))).exp()
}

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} {detail}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        _ => None,
    }
}

/// Refused with `InvalidValue` and the key `"temperature"` at the top level.
fn top_temperature<T>(r: &rcfd_fem::Result<T>) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (
            matches!(e, FemError::InvalidValue { key: "temperature", .. }),
            format!("{e}"),
        ),
    }
}

fn base() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// The unit cube of one Hex8 with three symmetry planes.
fn cube(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn pulled(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_load(Load::traction("xmax", [SIGMA, 0.0, 0.0])).unwrap();
    m
}

/// The step load of the traction on `xmax`: the constant factor 1 on `[0, t_end]`.
fn step_load(t_end: f64) -> LoadHistory {
    let pattern = nodal_load_vector(&pulled(base()), &[]).unwrap();
    LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, t_end).unwrap())
}

fn tight() -> NewtonOptions {
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-12;
    newton
}

/// The registered non-positive temperatures of NP1 (arithmetic 2).
const S: [f64; 18] = [
    0.0,
    -0.0,
    -5e-324,
    -f64::MIN_POSITIVE,
    -1e-300,
    -1e-3,
    -1.0,
    -33.0,
    -34.0,
    -35.0,
    -100.0,
    -700.0,
    -1e3,
    -1e5,
    -1e10,
    -1e100,
    -1e300,
    -f64::MAX,
];

/// The non-positive temperatures of NP2 and NP3.
const S_D: [f64; 7] = [0.0, -0.0, -5e-324, -1.0, -34.0, -1e10, -f64::MAX];

// ------------------------------------------------------------------------------------------------- NP1 to NP3, NI1, PC1

#[test]
fn np1_np2_np3_ni1_pc1_refusals_at_every_non_positive_temperature() {
    let mut c = Checks::new();
    let factor = || Material::creep_arrhenius(base(), 1e-26, N, 0.0, Q).unwrap();
    let plain = || Material::creep(base(), 1e-26, N, 0.0).unwrap();
    let eps = [7.1e-4, -2.3e-4, -3.9e-4, 1.7e-4, -0.8e-4, 2.6e-4];
    let st = [1.1e-4, -0.4e-4, -0.7e-4, 0.3e-4, 0.5e-4, -0.2e-4, 3.0e-4];

    // NP1: the factor and the point update.
    let fm = factor();
    for t in S {
        let r = arrhenius_factor(Q, t);
        c.holds(
            &format!("NP1 arrhenius_factor at T = {t:e} K: key temperature"),
            r.as_ref().err().and_then(key_of) == Some("temperature"),
            &format!("{r:?}"),
        );
        let p = PointContext::new().with_time(2.0, 0.5).unwrap().with_temperature(t).unwrap();
        let r = update(&fm, &eps, &st, &p);
        c.holds(
            &format!("NP1 update at T = {t:e} K: key temperature"),
            r.as_ref().err().and_then(key_of) == Some("temperature"),
            &format!("{:?}", r.err()),
        );
    }
    let r = arrhenius_factor(Q, f64::NEG_INFINITY);
    c.holds(
        "NP1 arrhenius_factor at T = -inf: key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{r:?}"),
    );
    let r = PointContext::new()
        .with_time(2.0, 0.5)
        .unwrap()
        .with_temperature(f64::NEG_INFINITY);
    c.holds(
        "NP1 with_temperature(-inf): key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{:?}", r.err()),
    );

    // NP2 and NP3: the drivers with a constant temperature and with a history point.
    let fc = pulled(factor());
    let loads = [step_load(10.0)];
    let loading = TimeLoading::new();
    for t in S_D {
        let history = LoadTable::new(vec![(0.0, 700.0), (1.0, 700.0), (2.0, t)]).unwrap();
        for (what, nl, ts) in [
            (
                "temperature",
                NlTransientOptions::new(1e-3).temperature(t),
                TimeSteppingOptions::new(f64::INFINITY).temperature(t),
            ),
            (
                "history",
                NlTransientOptions::new(1e-3).temperature_history(history.clone()),
                TimeSteppingOptions::new(f64::INFINITY).temperature_history(history.clone()),
            ),
        ] {
            let (ok, d) = top_temperature(&NlIntegrator::new(&cube(factor()), &loads, &nl));
            c.holds(&format!("NP2 Integrator::new, {what} at {t:e} K"), ok, &d);
            let (ok, d) = top_temperature(&solve_in_time_with(&fc, &[], &loading, &[0.1, 0.2], &ts));
            c.holds(&format!("NP3 solve_in_time_with, {what} at {t:e} K"), ok, &d);
        }
    }

    // NI1: no temperature option.
    let (ok, d) = top_temperature(&NlIntegrator::new(&cube(factor()), &loads, &NlTransientOptions::new(1e-3)));
    c.holds("NI1 Integrator::new without a temperature option", ok, &d);

    // PC1: the positive side.
    let accepted = |r: rcfd_fem::Result<NlIntegrator>| match r {
        Ok(_) => (true, "accepted".to_string()),
        Err(e) => (false, format!("{e}")),
    };
    let (ok, d) = accepted(NlIntegrator::new(
        &cube(factor()),
        &loads,
        &NlTransientOptions::new(1e-3).temperature(700.0),
    ));
    c.holds("PC1 Integrator::new, factor material at 700 K", ok, &d);
    let (ok, d) = accepted(NlIntegrator::new(
        &cube(factor()),
        &loads,
        &NlTransientOptions::new(1e-3).temperature_history(LoadTable::new(vec![(0.0, 700.0), (2.0, 750.0)]).unwrap()),
    ));
    c.holds("PC1 Integrator::new, factor material with a positive history", ok, &d);
    for (name, o) in [
        ("no temperature", NlTransientOptions::new(1e-3)),
        ("-1 K", NlTransientOptions::new(1e-3).temperature(-1.0)),
    ] {
        let (ok, d) = accepted(NlIntegrator::new(&cube(plain()), &loads, &o));
        c.holds(&format!("PC1 Integrator::new, plain creep, {name}"), ok, &d);
    }
    for (name, o) in [
        ("no temperature", TimeSteppingOptions::new(f64::INFINITY)),
        ("-1 K", TimeSteppingOptions::new(f64::INFINITY).temperature(-1.0)),
    ] {
        let r = solve_in_time_with(&pulled(plain()), &[], &loading, &[0.1, 0.2], &o);
        c.holds(
            &format!("PC1 solve_in_time_with, plain creep, {name}"),
            r.is_ok(),
            &format!("{:?}", r.err()),
        );
    }
    let r = arrhenius_factor(Q, 700.0);
    c.holds(
        "PC1 arrhenius_factor at 700 K positive and finite",
        matches!(r, Ok(x) if x > 0.0 && x.is_finite()),
        &format!("{r:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- CR1, CR2, DV, CRr

/// The measures of one run at the temperature `temp` (D3).
struct Rate {
    by_u: f64,
    by_state: f64,
    dv: f64,
    rate_u: f64,
}

fn creep_run(material: &Material, a: f64, omega: f64, temp: f64) -> Rate {
    let p = 2.0 * PI / omega;
    let h = p / 40.0;
    let steps = 1280u64;
    let model = cube(material.clone());
    let t_end = steps as f64 * h * 1.01;
    let opts = NlTransientOptions::new(h)
        .method(Method::GeneralisedAlpha { rho_inf: 0.5 })
        .damping(Rayleigh {
            mass: omega,
            stiffness: 0.0,
        })
        .newton(tight())
        .temperature(temp);
    let mut it = NlIntegrator::new(&model, &[step_load(t_end)], &opts).unwrap();
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    let eqs: Vec<usize> = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&k| dofs.equation(k, 0).unwrap())
        .collect();
    let mean = |x: &[f64]| eqs.iter().map(|&q| x[q]).sum::<f64>() / eqs.len() as f64;
    let rate = a * f(temp) * SIGMA.powf(N);
    let (mut u_a, mut t_a, mut c_a) = (0.0, 0.0, Vec::new());
    let mut dv: f64 = 0.0;
    for k in 1..=steps {
        it.step().unwrap();
        if k < 640 {
            continue;
        }
        let s = it.state().unwrap();
        let t = k as f64 * h;
        let e = (mean(&s.v) - rate).abs() / rate;
        dv = if e.is_finite() { dv.max(e) } else { f64::NAN };
        let creep: Vec<f64> = s.states[0].values().chunks(7).map(|q| q[0]).collect();
        if k == 640 {
            u_a = mean(&s.u);
            t_a = t;
            c_a = creep;
        } else if k == steps {
            let w = t - t_a;
            let rate_u = (mean(&s.u) - u_a) / w;
            let by_state = creep
                .iter()
                .zip(&c_a)
                .map(|(b, a)| ((b - a) / w - rate).abs() / rate)
                .fold(0.0, |m: f64, e| if e.is_finite() { m.max(e) } else { f64::NAN });
            println!(
                "CR at {temp} K: P = {p:.6e} s, window [{t_a:.6e}, {t:.6e}] s, u_s ~ {:.6e} m, rate by u {rate_u:.9e} against {rate:.9e}",
                SIGMA / E
            );
            return Rate {
                by_u: (rate_u - rate).abs() / rate,
                by_state,
                dv,
                rate_u,
            };
        }
    }
    unreachable!()
}

#[test]
fn cr1_cr2_dv_crr_creep_rate_in_the_nonlinear_transient_integrator() {
    let mut c = Checks::new();
    let modal = modal_analysis(&cube(base()), &ModalOptions::new(1)).unwrap();
    let omega = modal.eigenvalues[0].sqrt();
    let p = 2.0 * PI / omega;
    let t_c = 1e4 * p;
    let a2 = 1.0 / (E * t_c * SIGMA.powf(N - 1.0));
    let a = a2 / f(T2);
    println!("omega_1 = {omega:.9e} rad/s, P = {p:.9e} s, A = {a:.6e}, A f(T2) = {:.6e}", a * f(T2));
    let material = Material::creep_arrhenius(base(), a, N, 0.0, Q).unwrap();
    let r2 = creep_run(&material, a, omega, T2);
    c.at_most("CR2 rate by the displacement at T2", r2.by_u, 1e-2);
    c.at_most("CR2 rate by the creep state at T2, worst point", r2.by_state, 1e-2);
    c.at_most("DV velocity over the window at T2, worst step", r2.dv, 1e-2);
    let r1 = creep_run(&material, a, omega, T1);
    c.at_most("CR1 rate by the displacement at T1", r1.by_u, 1e-2);
    c.at_most("CR1 rate by the creep state at T1, worst point", r1.by_state, 1e-2);
    c.at_most("DV velocity over the window at T1, worst step", r1.dv, 1e-2);
    let ratio = r2.rate_u / r1.rate_u;
    let want = f(T2) / f(T1);
    println!("CRr ratio {ratio:.9} against f(T2)/f(T1) = {want:.9}");
    let e = (ratio / want - 1.0).abs();
    c.at_most(
        "CRr ratio of the rates at T2 and T1 against f(T2)/f(T1)",
        if e.is_finite() { e } else { f64::NAN },
        1e-2,
    );
    c.finish();
}
