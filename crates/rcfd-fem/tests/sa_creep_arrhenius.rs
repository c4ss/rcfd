//! F-581 (ADR 0558): the Arrhenius temperature factor of power-law creep, read from the point context, on the time-stepping
//! driver with load and displacement histories. The checks A1, A1e, A2, A3, NA4, TA, RA, CH1, P1, NA1, NA2 and NA3 of the
//! registration. N1 is `sa_creep_arrhenius_default_bits.rs`; R1t, C1t, C2t are the test of ADR 0499 in `sa_qs_visco.rs`,
//! re-run unchanged.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_creep::{arrhenius_factor, update};
use rcfd_fem::material::PointContext;
use rcfd_fem::nonlinear::{
    element_second_piola_kirchhoff, solve_in_time, solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading,
    TimeSteppingOptions,
};
use rcfd_fem::transient::{LoadHistory, LoadTable, MotionHistory};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};

const E: f64 = 2.0e9;
const NU: f64 = 0.3;
const RHO: f64 = 1200.0;
const EPS0: f64 = 1e-3;
/// The activation energy of the checks, J/mol.
const Q: f64 = 2.0e5;
const T1: f64 = 700.0;
const T2: f64 = 750.0;
/// The gas constant of the test (its own literal, J/(mol K)).
const R: f64 = 8.314_462_618;

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

fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0, |a, x| a.max(x.abs()))
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        _ => None,
    }
}

fn base() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn creep(a: f64, n: f64, m: f64) -> Material {
    Material::creep(base(), a, n, m).unwrap()
}

/// The factor material whose `A f(T1)` is `a_eff`.
fn hot(a_eff: f64, n: f64, m: f64) -> Material {
    Material::creep_arrhenius(base(), a_eff / f(T1), n, m, Q).unwrap()
}

/// The pre-exponential factor of a material.
fn pre(material: &Material) -> f64 {
    match material {
        Material::Creep { a, .. } => *a,
        _ => panic!("not a creep material"),
    }
}

fn ctx(t: f64, dt: f64) -> PointContext {
    PointContext::new().with_time(t, dt).unwrap()
}

/// `x = a^-1 b` by Gaussian elimination with partial pivoting.
fn solve_dense<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> [f64; N] {
    for c in 0..N {
        let p = (c..N)
            .max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())
            .unwrap();
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..N {
            let f = a[r][c] / a[c][c];
            for k in c..N {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; N];
    for r in (0..N).rev() {
        let mut s = b[r];
        for k in r + 1..N {
            s -= a[r][k] * x[k];
        }
        x[r] = s / a[r][r];
    }
    x
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

const SIGMA_E: f64 = 0.2;

fn pulled(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_load(Load::traction("xmax", [SIGMA_E, 0.0, 0.0])).unwrap();
    m
}

fn stretched(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_constraint(Constraint::displacement("xmax", 0, EPS0)).unwrap();
    m
}

/// `u_x = EPS0` at the `xmax` nodes as a motion history with the constant factor 1 on `[0, t_end]`.
fn hold(model: &SolidModel, t_end: f64) -> MotionHistory {
    let list = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, EPS0))
        .collect();
    MotionHistory::new(list, LoadTable::constant(1.0, 0.0, t_end).unwrap())
}

fn relax_a(n: f64) -> f64 {
    1.0 / ((n - 1.0) * E * (E * EPS0).powf(n - 1.0))
}

fn times(steps: usize, dt: f64) -> Vec<f64> {
    (1..=steps).map(|k| k as f64 * dt).collect()
}

fn tight() -> NewtonOptions {
    let mut newton = NewtonOptions::new(1);
    newton.residual_tolerance = 1e-12;
    newton
}

/// Every field of every step end of a time history, as bits.
fn flat_history(h: &TimeHistory) -> Vec<u64> {
    let mut v = Vec::new();
    for s in &h.steps {
        v.push(s.time);
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.internal_force);
        for e in &s.states {
            v.push(e.values().len() as f64);
            v.extend_from_slice(e.values());
        }
        v.push(s.halvings as f64);
        v.push(s.substeps as f64);
        v.push(s.max_creep_increment);
        v.push(s.iterations as f64);
    }
    v.iter().map(|x| x.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------- A1, A2

/// Creep at the material point under the uniaxial stress `sigma` (stress control) at the temperature `T1`: the largest
/// relative error of `eps_c,xx` against `A f(T1) sigma^n t^(m+1)/(m+1)` over the step ends.
fn uniaxial_point(mat: &Material, n: f64, m: f64, sigma: f64, steps: usize, dt: f64) -> f64 {
    let a = pre(mat);
    let mut st = [0.0; 7];
    let mut eps = [0.0; 6];
    let target = [sigma, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut worst: f64 = 0.0;
    for k in 0..=steps {
        let (t, h) = if k == 0 { (0.0, 0.0) } else { (k as f64 * dt, dt) };
        let p = ctx(t, h).with_temperature(T1).unwrap();
        let mut it = 0;
        let r = loop {
            let r = update(mat, &eps, &st, &p).unwrap();
            let res: Vec<f64> = (0..6).map(|i| r.stress[i] - target[i]).collect();
            if it > 0 && max_abs(res.iter().copied()) <= 1e-13 * sigma {
                break r;
            }
            assert!(it < 50, "stress control not converged");
            let d = solve_dense(r.tangent, [res[0], res[1], res[2], res[3], res[4], res[5]]);
            for i in 0..6 {
                eps[i] -= d[i];
            }
            it += 1;
        };
        st = r.state;
        if k > 0 {
            let want = a * f(T1) * sigma.powf(n) * t.powf(m + 1.0) / (m + 1.0);
            let e = (st[0] - want).abs() / want;
            worst = if e.is_finite() { worst.max(e) } else { f64::NAN };
        }
    }
    worst
}

/// The worst relative error of `eps_c,xx` over the step ends `k >= 1` and points of a one-element history against `want(t)`.
fn element_error(h: &TimeHistory, want: impl Fn(f64) -> f64) -> f64 {
    let mut worst: f64 = 0.0;
    for s in h.steps.iter().skip(1) {
        let w = want(s.time);
        for q in s.states[0].values().chunks(7) {
            let e = (q[0] - w).abs() / w;
            if !e.is_finite() {
                return f64::NAN;
            }
            worst = worst.max(e);
        }
    }
    worst
}

#[test]
fn a1_a2_a3_na4_uniaxial_creep_with_the_factor() {
    let mut c = Checks::new();
    let n = 3.0;
    c.at_most(
        "A1 material point, m = 0, T1: max |eps_c - A f sigma^n t| / (A f sigma^n t)",
        uniaxial_point(&hot(1e-25, n, 0.0), n, 0.0, 2e6, 100, 0.1),
        1e-8,
    );
    c.at_most(
        "A2 material point, m = -0.5, T1",
        uniaxial_point(&hot(1e-25, n, -0.5), n, -0.5, 2e6, 100, 0.1),
        1e-3,
    );

    // A1e: one Hex8 under the dead traction, uniform temperature T1.
    let mat = hot(1.25e-9, n, 0.0);
    let a = pre(&mat);
    let model = pulled(mat);
    let run = |temp: TimeSteppingOptions| solve_in_time(&model, &[], &times(100, 0.1), &temp).unwrap();
    let h1 = run(TimeSteppingOptions::new(f64::INFINITY).newton(tight()).temperature(T1));
    let s3 = SIGMA_E.powf(n);
    c.at_most(
        "A1e one Hex8, dead traction, temperature(T1), m = 0",
        element_error(&h1, |t| a * f(T1) * s3 * t),
        1e-8,
    );

    // NA4: the same at T2; the ratio of the final creep strains against f(T2)/f(T1).
    let h2 = run(TimeSteppingOptions::new(f64::INFINITY).newton(tight()).temperature(T2));
    let last = |h: &TimeHistory| h.steps.last().unwrap().states[0].values()[0];
    let ratio = last(&h2) / last(&h1);
    let want = f(T2) / f(T1);
    println!("NA4 ratio {ratio:.10} against f(T2)/f(T1) = {want:.10}");
    let e = (ratio / want - 1.0).abs();
    c.at_most(
        "NA4 ratio of the creep strains at T2 and T1 against f(T2)/f(T1)",
        if e.is_finite() { e } else { f64::NAN },
        1e-8,
    );

    // A3: a temperature step T1 -> T2 at t = 5 s by the temperature history.
    let table = LoadTable::new(vec![(0.0, T1), (5.0, T1), (5.0 + 1e-9, T2), (11.0, T2)]).unwrap();
    let h3 = solve_in_time(
        &model,
        &[],
        &times(100, 0.1),
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_history(table),
    )
    .unwrap();
    c.at_most(
        "A3 temperature step T1 -> T2 at 5 s: max relative error against A sigma^n (f1 min(t,5) + f2 max(t-5,0))",
        element_error(&h3, |t| a * s3 * (f(T1) * t.min(5.0) + f(T2) * (t - 5.0).max(0.0))),
        1e-8,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- TA

#[test]
fn ta_tangent_with_the_factor_central_differences() {
    let mut c = Checks::new();
    let eps = [7.1e-4, -2.3e-4, -3.9e-4, 1.7e-4, -0.8e-4, 2.6e-4];
    let mut st = [1.1e-4, -0.4e-4, -0.7e-4, 0.3e-4, 0.5e-4, -0.2e-4, 3.0e-4];
    st[6] = 3e-4;
    for (n, m) in [(3.0, 0.0), (4.0, -0.5)] {
        let mat = hot(1e-26, n, m);
        let p = ctx(2.0, 0.5).with_temperature(T1).unwrap();
        let u0 = update(&mat, &eps, &st, &p).unwrap();
        c.holds(
            &format!("TA n = {n}, m = {m}: set-up (creeping)"),
            u0.creeping && u0.state[6] > st[6],
            &format!(
                "{} local iterations, d eps_bar {:e}",
                u0.iterations,
                u0.state[6] - st[6]
            ),
        );
        let h = 1e-10;
        let mut worst: f64 = 0.0;
        for j in 0..6 {
            let mut a = eps;
            a[j] += h;
            let mut b = eps;
            b[j] -= h;
            let sa = update(&mat, &a, &st, &p).unwrap().stress;
            let sb = update(&mat, &b, &st, &p).unwrap().stress;
            for i in 0..6 {
                worst = worst.max((u0.tangent[i][j] - (sa[i] - sb[i]) / (2.0 * h)).abs());
            }
        }
        let scale = max_abs(u0.tangent.iter().flatten().copied());
        let v = worst / scale;
        c.at_most(
            &format!("TA n = {n}, m = {m}: max |C - C_fd| / max |C|"),
            if v.is_finite() { v } else { f64::NAN },
            1e-6,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- RA

fn stresses(model: &SolidModel, u: &[f64], states: &[ElementState], e: usize) -> Vec<[f64; 6]> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let ue: Vec<f64> = nodes
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect();
    let material = model.material_of(e).unwrap();
    element_second_piola_kirchhoff(element_for(mesh.element_kind(e)), &coords, material, &ue, &states[e]).unwrap()
}

#[test]
fn ra_relaxation_with_the_factor_through_a_motion_history() {
    let mut c = Checks::new();
    let n = 3.0;
    let a_eff = relax_a(n);
    let mat = hot(a_eff, n, 0.0);
    let a = pre(&mat);
    let model = cube(mat);
    let loading = TimeLoading::new().motion(hold(&model, 11.0));
    let h = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times(800, 10.0 / 800.0),
        &TimeSteppingOptions::new(f64::INFINITY).temperature(T1),
    )
    .unwrap();
    let s0: Vec<f64> = stresses(&model, &h.steps[0].u, &h.steps[0].states, 0)
        .iter()
        .map(|s| s[0])
        .collect();
    let mut worst: f64 = 0.0;
    for s in &h.steps {
        for (q, st) in stresses(&model, &s.u, &s.states, 0).iter().enumerate() {
            let want = (s0[q].powf(1.0 - n) + (n - 1.0) * E * a * f(T1) * s.time).powf(1.0 / (1.0 - n));
            let e = (st[0] - want).abs() / want;
            worst = if e.is_finite() { worst.max(e) } else { f64::NAN };
        }
    }
    println!(
        "RA S_xx(10 s)/S_xx(0) = {:.6}",
        stresses(&model, &h.steps[800].u, &h.steps[800].states, 0)[0][0] / s0[0]
    );
    c.at_most(
        "RA relaxation with the factor, motion history, temperature(T1), n = 3, 800 steps",
        worst,
        5e-3,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- CH1

#[test]
fn ch1_constant_histories_give_the_bits_of_adr_0489() {
    let mut c = Checks::new();
    let inf = || TimeSteppingOptions::new(f64::INFINITY);

    // (i) the dead traction as a load history; (iv) the same with the factor at T1.
    for (name, mat, opts) in [
        (
            "CH1 (i) dead traction as a load history",
            creep(1.25e-9, 3.0, 0.0),
            inf(),
        ),
        (
            "CH1 (iv) the same with the factor, temperature(T1)",
            hot(1.25e-9, 3.0, 0.0),
            inf().temperature(T1),
        ),
    ] {
        let with_load = pulled(mat.clone());
        let ts = times(20, 0.1);
        let a = solve_in_time(&with_load, &[], &ts, &opts).unwrap();
        let pattern = nodal_load_vector(&with_load, &[]).unwrap();
        let loading = TimeLoading::new().load(LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 3.0).unwrap()));
        let b = solve_in_time_with(&cube(mat), &[], &loading, &ts, &opts).unwrap();
        c.holds(name, flat_history(&a) == flat_history(&b), "");
    }

    // (ii) the held strain as a motion history; (iii) the halving set-up of ADR 0489 H1.
    let mat = creep(relax_a(3.0), 3.0, 0.0);
    let ts = times(20, 0.0125);
    let a = solve_in_time(&stretched(mat.clone()), &[], &ts, &inf()).unwrap();
    let free = cube(mat.clone());
    let loading = TimeLoading::new().motion(hold(&free, 1.0));
    let b = solve_in_time_with(&free, &[], &loading, &ts, &inf()).unwrap();
    c.holds(
        "CH1 (ii) held strain as a motion history",
        flat_history(&a) == flat_history(&b),
        "",
    );
    let opts = TimeSteppingOptions::new(1e-4);
    let a = solve_in_time(&stretched(mat), &[], &[1.0], &opts).unwrap();
    let b = solve_in_time_with(&free, &[], &loading, &[1.0], &opts).unwrap();
    c.holds(
        "CH1 (iii) halving step with the motion history",
        a.steps[1].halvings == 2 && b.steps[1].halvings == 2 && flat_history(&a) == flat_history(&b),
        &format!("halvings {} and {}", a.steps[1].halvings, b.steps[1].halvings),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- P1, NA1 to NA3

#[test]
fn p1_na1_na2_na3_no_increment_and_refusals() {
    let mut c = Checks::new();
    let eps = [7.1e-4, -2.3e-4, -3.9e-4, 1.7e-4, -0.8e-4, 2.6e-4];
    let st = [1.1e-4, -0.4e-4, -0.7e-4, 0.3e-4, 0.5e-4, -0.2e-4, 3.0e-4];
    let plain = creep(1e-26, 3.0, 0.0);
    let factor = Material::creep_arrhenius(base(), 1e-26, 3.0, 0.0, Q).unwrap();
    let flat = |u: rcfd_fem::mat_creep::Update| -> Vec<u64> {
        let mut v = u.stress.to_vec();
        v.extend(u.tangent.iter().flatten());
        v.extend_from_slice(&u.state);
        v.push(u.iterations as f64);
        v.push(if u.creeping { 1.0 } else { 0.0 });
        v.iter().map(|x| x.to_bits()).collect()
    };
    for (name, p) in [
        ("P1 no time increment, no temperature", ctx(0.0, 0.0)),
        ("P1 default context", PointContext::new()),
    ] {
        let a = update(&plain, &eps, &st, &p).unwrap();
        let b = update(&factor, &eps, &st, &p).unwrap();
        c.holds(name, flat(a) == flat(b), "");
    }

    let r = update(&factor, &eps, &st, &ctx(2.0, 0.5));
    c.holds(
        "NA1 (i) an increment without a temperature: key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{:?}", r.err()),
    );
    let r = solve_in_time(
        &pulled(hot(1.25e-9, 3.0, 0.0)),
        &[],
        &times(3, 0.1),
        &TimeSteppingOptions::new(1.0),
    );
    c.holds(
        "NA1 (ii) the driver without a temperature: InvalidValue key temperature",
        matches!(&r, Err(FemError::InvalidValue { key: "temperature", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    for t in [0.0, -1.0] {
        let r = update(&factor, &eps, &st, &ctx(2.0, 0.5).with_temperature(t).unwrap());
        c.holds(
            &format!("NA2 T = {t} K: key temperature"),
            r.as_ref().err().and_then(key_of) == Some("temperature"),
            &format!("{:?}", r.err()),
        );
    }
    let r = arrhenius_factor(Q, 1e-3);
    c.holds(
        "NA2 underflow at 1e-3 K: key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{:?}", r),
    );
    for q in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let r = Material::creep_arrhenius(base(), 1e-26, 3.0, 0.0, q);
        c.holds(
            &format!("NA3 activation {q}: key activation"),
            r.as_ref().err().and_then(key_of) == Some("activation"),
            &format!("{:?}", r.err()),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- NA2b (amendment 1)

#[test]
fn na2b_a_large_negative_temperature_is_refused() {
    let mut c = Checks::new();
    let eps = [7.1e-4, -2.3e-4, -3.9e-4, 1.7e-4, -0.8e-4, 2.6e-4];
    let st = [1.1e-4, -0.4e-4, -0.7e-4, 0.3e-4, 0.5e-4, -0.2e-4, 3.0e-4];
    let factor = Material::creep_arrhenius(base(), 1e-26, 3.0, 0.0, Q).unwrap();
    let r = update(&factor, &eps, &st, &ctx(2.0, 0.5).with_temperature(-1e10).unwrap());
    c.holds(
        "NA2b update at T = -1e10 K: key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{:?}", r.err()),
    );
    let r = arrhenius_factor(Q, -1e10);
    c.holds(
        "NA2b arrhenius_factor at T = -1e10 K: key temperature",
        r.as_ref().err().and_then(key_of) == Some("temperature"),
        &format!("{:?}", r),
    );
    c.finish();
}
