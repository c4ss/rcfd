//! F-592 (ADR 0526): a temperature history on the quasi-static driver and the nonlinear transient integrator, with the
//! Rayleigh stiffness tangent at the temperature. Checks S2, S2c, NC1 (the relaxation under a temperature step against its
//! closed form), S4, S4o (a temperature ramp), S3d, S3i, S3e (a constant table gives the bits of ADR 0499), V2T, NC2 (the
//! integrator under a temperature step), RT1 (the damping tangent at the temperature), NT4 and RF3 (refusals) of the
//! registration. Every aggregated error is required finite.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::ElementState;
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_visco::shift_factor;
use rcfd_fem::material::{PointContext, PronyTerm, TimeTemperatureShift};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::transient::{LoadTable, MotionHistory, Rayleigh};
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, a non-finite value is a miss, the test fails
/// at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.6e} (limit in [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo:e}, {hi:e}]"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// `max` that keeps a NaN (b5).
fn worse(w: &mut f64, v: f64) {
    if v.is_nan() || v > *w {
        *w = v;
    }
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        FemError::Element { source, .. } => key_of(source),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; x.len()];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ------------------------------------------------------------------------------------------------- materials

const E0: f64 = 2.0e9;
const NU: f64 = 0.35;
const W3: [f64; 3] = [0.3, 0.2, 0.1];
const TAU3: [f64; 3] = [0.01, 0.1, 1.0];
const T_REF: f64 = 373.0;
const T1: f64 = 383.0;
const T2: f64 = 393.0;
const EPS0: f64 = 1e-3;

fn base(nu: f64) -> Material {
    Material::isotropic_elastic(E0, nu, 1200.0, 0.0).unwrap()
}

/// Equal shear and bulk weights `w`, times `tau`.
fn equal(w: &[f64], tau: &[f64], nu: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = w
        .iter()
        .zip(tau)
        .map(|(&wi, &ti)| PronyTerm::new(wi, wi, ti).unwrap())
        .collect();
    Material::viscoelastic(base(nu), terms, shift).unwrap()
}

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

/// The WLF factor written here (independent of the library's `shift_factor`).
fn wlf_formula(t: f64) -> f64 {
    10f64.powf(-17.44 * (t - T_REF) / (51.6 + t - T_REF))
}

/// `g(xi) = w_inf + sum w_i exp(-xi/tau_i)`, the normalised relaxation modulus at the reduced time `xi`.
fn g(xi: f64) -> f64 {
    1.0 - W3.iter().sum::<f64>() + W3.iter().zip(TAU3).map(|(w, t)| w * (-xi / t).exp()).sum::<f64>()
}

// ------------------------------------------------------------------------------------------------- the cube

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

fn xmax_x_equation(model: &SolidModel) -> usize {
    let n = model.mesh().node_set("xmax").unwrap()[0];
    model.dof_map().equation(n, 0).unwrap()
}

/// `u_x = EPS0` at the `xmax` nodes from `t = 0` to `t_far`.
fn strain_step(model: &SolidModel, t_far: f64) -> TimeLoading {
    let list = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, EPS0))
        .collect();
    TimeLoading::new().motion(MotionHistory::new(list, LoadTable::constant(1.0, 0.0, t_far).unwrap()))
}

/// The committed `S_xx` of every point of the elements (ADR 0485 amendment 1, S1b): `Material::stress` at the committed
/// strain with the committed state and the context `at` (no time increment).
fn committed_sxx(material: &Material, states: &[ElementState], at: &PointContext) -> Vec<f64> {
    let per = material.state_size();
    let mut out = Vec::new();
    for s in states {
        for st in s.values().chunks(per) {
            let strain = [st[0], st[1], st[2], st[3], st[4], st[5]];
            out.push(material.stress(&strain, at, st).unwrap().0[0]);
        }
    }
    out
}

/// The Green-Lagrange `E_xx` of the homogeneous stretch of the unit cube.
fn exx(model: &SolidModel, u: &[f64]) -> f64 {
    let a = u[xmax_x_equation(model)];
    a + 0.5 * a * a
}

fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

fn flat_history(h: &TimeHistory) -> Vec<Vec<u64>> {
    h.steps
        .iter()
        .map(|s| {
            let mut v = s.u.clone();
            v.extend_from_slice(&s.internal_force);
            for e in &s.states {
                v.push(e.values().len() as f64);
                v.extend_from_slice(e.values());
            }
            v.push(s.halvings as f64);
            v.push(s.substeps as f64);
            bits(&v)
        })
        .collect()
}

/// The largest `|S_xx/(E_0 E_xx) - g(xi(t_n))|` over the step ends and points of a driver run, the committed stress at the
/// context with the temperature `temp(t_n)`; with the number of point values and the largest halving count.
fn relaxation_error(
    model: &SolidModel,
    h: &TimeHistory,
    temp: &dyn Fn(f64) -> f64,
    xi: &dyn Fn(f64) -> f64,
) -> (f64, usize, usize) {
    let material = &model.materials()[0].1;
    let mut worst: f64 = 0.0;
    let mut count = 0;
    let mut halvings = 0;
    for s in &h.steps {
        let e = exx(model, &s.u);
        let at = PointContext::new().with_temperature(temp(s.time)).unwrap();
        let want = g(xi(s.time));
        for sxx in committed_sxx(material, &s.states, &at) {
            worse(&mut worst, (sxx / (E0 * e) - want).abs());
            count += 1;
        }
        halvings = halvings.max(s.halvings);
    }
    (worst, count, halvings)
}

/// `int_0^t f(s) ds` by composite three-point Gauss-Legendre over `panels` panels.
fn gauss3(f: &dyn Fn(f64) -> f64, t: f64, panels: usize) -> f64 {
    let x = [-(0.6f64).sqrt(), 0.0, (0.6f64).sqrt()];
    let w = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];
    let h = t / panels as f64;
    let mut s = 0.0;
    for i in 0..panels {
        let m = (i as f64 + 0.5) * h;
        for k in 0..3 {
            s += w[k] * f(m + x[k] * h / 2.0) * h / 2.0;
        }
    }
    s
}

/// The ramp of arithmetic 5: `T = 383 + 10 s/t_end`, `N` equal steps to `t_end`; the error measure of S2.
fn ramp_error(t_end: f64, steps: usize) -> (f64, usize) {
    let model = cube(equal(&W3, &TAU3, NU, Some(wlf())));
    let table = LoadTable::new(vec![(0.0, T1), (t_end, T2)]).unwrap();
    let h = t_end / steps as f64;
    let times: Vec<f64> = (1..=steps)
        .map(|k| if k == steps { t_end } else { k as f64 * h })
        .collect();
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature_history(table.clone());
    let run = solve_in_time_with(&model, &[], &strain_step(&model, 2.0 * t_end), &times, &opts).unwrap();
    let rate = |s: f64| 1.0 / wlf_formula(T1 + (T2 - T1) * s / t_end);
    let (e, _, halvings) = relaxation_error(&model, &run, &|t| table.value(t).unwrap(), &|t| gauss3(&rate, t, 200));
    (e, halvings)
}

#[test]
fn s2_nc1_s4_s3d_the_quasi_static_driver() {
    let mut c = Checks::new();
    let a1 = wlf_formula(T1);
    let a2 = wlf_formula(T2);
    println!("a_1 = {a1:.9e}, a_2 = {a2:.9e}");
    let model = cube(equal(&W3, &TAU3, NU, Some(wlf())));

    // S2: 50 steps of 0.02 a_1, then 50 steps of 0.02 a_2; the table steps from T_1 to T_2 over the 51st step.
    let (h1, h2) = (0.02 * a1, 0.02 * a2);
    let ts = 50.0 * h1;
    let times: Vec<f64> = (1..=100)
        .map(|k| {
            if k <= 50 {
                k as f64 * h1
            } else {
                ts + (k - 50) as f64 * h2
            }
        })
        .collect();
    let t_last = times[99];
    let table = LoadTable::new(vec![(0.0, T1), (ts, T1), (times[50], T2), (2.0 * t_last, T2)]).unwrap();
    let xi = |t: f64| t.min(ts) / a1 + (t - ts).max(0.0) / a2;
    let loading = strain_step(&model, 2.0 * t_last);
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature_history(table.clone());
    let run = solve_in_time_with(&model, &[], &loading, &times, &opts).unwrap();
    let (worst, count, halvings) = relaxation_error(&model, &run, &|t| table.value(t).unwrap(), &xi);
    c.holds(
        "S2c set-up",
        count == 101 * 8 && halvings == 0,
        &format!("{count} point values, largest halvings {halvings}"),
    );
    c.at_most(
        "S2 relaxation under a temperature step against the closed form",
        worst,
        1e-10,
    );

    // NC1: the constant T_1 against the step closed form.
    let opts1 = TimeSteppingOptions::new(f64::INFINITY).newton(tight()).temperature(T1);
    let run1 = solve_in_time_with(&model, &[], &loading, &times, &opts1).unwrap();
    let (worst1, _, _) = relaxation_error(&model, &run1, &|_| T1, &xi);
    c.at_least("NC1 the constant T_1 differs from the step closed form", worst1, 1e-3);

    // S4, S4o: the ramp, t_end with xi(t_end) = 2.
    let unit = gauss3(&|s: f64| 1.0 / wlf_formula(T1 + (T2 - T1) * s), 1.0, 200);
    let t_end = 2.0 / unit;
    println!("S4 t_end = {t_end:.6e}");
    let (e100, hv100) = ramp_error(t_end, 100);
    let (e200, hv200) = ramp_error(t_end, 200);
    c.holds(
        "S4 set-up (no halving)",
        hv100 == 0 && hv200 == 0,
        &format!("{hv100}, {hv200}"),
    );
    println!("S4 e_200 = {e200:.6e}");
    c.at_most("S4 ramp error at 100 steps", e100, 7e-3);
    c.within("S4o observed order 100 -> 200", (e100 / e200).log2(), 0.9, 1.1);

    // S3d: the S1d set-up of ADR 0499 with a constant table against the constant temperature.
    let a = wlf_formula(T1);
    let times_d: Vec<f64> = (1..=100).map(|k| k as f64 * (0.02 * a)).collect();
    let ld = TimeLoading::new().motion(MotionHistory::new(
        model
            .mesh()
            .node_set("xmax")
            .unwrap()
            .iter()
            .map(|&n| Prescribed::new(n, 0, EPS0))
            .collect(),
        LoadTable::constant(1.0, 0.0, 1.0).unwrap(),
    ));
    let constant = solve_in_time_with(
        &model,
        &[],
        &ld,
        &times_d,
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight()).temperature(T1),
    )
    .unwrap();
    let tabled = solve_in_time_with(
        &model,
        &[],
        &ld,
        &times_d,
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_history(LoadTable::constant(T1, 0.0, 2.0 * times_d[99]).unwrap()),
    )
    .unwrap();
    c.holds(
        "S3d constant table on the driver gives the bits of the constant temperature",
        flat_history(&constant) == flat_history(&tabled),
        &format!("{} step ends", constant.steps.len()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the rod

/// The rod of ADR 0485 V1: 20 Hex8 along x, nu = 0, the lateral motion held.
fn rod(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

struct Rod {
    lambda: f64,
    v0: Vec<f64>,
}

fn rod_modes() -> Rod {
    let m = rod(base(0.0));
    let modal = modal_analysis(&m, &ModalOptions::new(2)).unwrap();
    let mass = consistent(&m).unwrap();
    let mut phi = modal.modes[0].clone();
    let norm = dot(&phi, &mv(&mass, &phi)).sqrt();
    for x in phi.iter_mut() {
        *x /= norm;
    }
    let lambda = modal.eigenvalues[0];
    let omega = lambda.sqrt();
    let peak = phi.iter().fold(0.0, |a: f64, b| a.max(b.abs()));
    let amp = 1e-6 * omega / (peak * PI / 2.0);
    let v0 = phi.iter().map(|x| amp * x).collect();
    Rod { lambda, v0 }
}

/// A run of the integrator: `u` and the committed `S_xx` of every point at every step end, the bits of `u`, `v`, `a` and
/// the internal force at every step end, the largest halving count, the state bytes at the end.
#[derive(Default)]
struct Run {
    u: Vec<Vec<f64>>,
    s: Vec<Vec<f64>>,
    bits: Vec<Vec<u64>>,
    halvings: usize,
    bytes: Vec<u8>,
}

/// Advances `it` by `steps` steps and records them in `run` (the committed stress at the context `at`).
fn advance(it: &mut NlIntegrator, model: &SolidModel, at: &PointContext, steps: usize, run: &mut Run) {
    let material = &model.materials()[0].1;
    for _ in 0..steps {
        let r = it.step().unwrap();
        run.halvings = run.halvings.max(r.halvings);
        let st = it.state().unwrap();
        // ADR 0526 amendment 1: no stress for a material without state (S3e).
        if material.state_size() == 0 {
            run.s.push(Vec::new());
        } else {
            run.s.push(committed_sxx(material, &st.states, at));
        }
        let mut v = st.u.clone();
        v.extend_from_slice(&st.v);
        v.extend_from_slice(&st.a);
        v.extend_from_slice(&st.internal_force);
        run.bits.push(bits(&v));
        run.u.push(st.u);
    }
    run.bytes = it.state().unwrap().to_bytes();
}

fn vibrate(model: &SolidModel, options: &NlTransientOptions, r: &Rod, at: &PointContext, steps: usize) -> Run {
    let mut it = NlIntegrator::new(model, &[], options).unwrap();
    it.start(0.0, &vec![0.0; r.v0.len()], &r.v0).unwrap();
    let mut run = Run::default();
    advance(&mut it, model, at, steps, &mut run);
    run
}

/// `max_n |x_n - y_n|_inf / max_n |y_n|_inf`.
fn relative(x: &[Vec<f64>], y: &[Vec<f64>]) -> f64 {
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for (a, b) in x.iter().zip(y) {
        for (p, q) in a.iter().zip(b) {
            worse(&mut diff, (p - q).abs());
            worse(&mut scale, q.abs());
        }
    }
    if x.len() != y.len() {
        return f64::NAN;
    }
    diff / scale
}

fn at_temperature(t: f64) -> PointContext {
    PointContext::new().with_temperature(t).unwrap()
}

/// The registered run of V2T, NC2, RT1 and S3i (`f592-scratch/run1.log`); its S3e set-up panicked (ADR 0526 amendment 1),
/// S3e is run by `s3e_amended_constant_table_on_the_energy_momentum_step`.
#[test]
#[ignore = "record of the registered run (ADR 0526 amendment 1): V2T, NC2, RT1, S3i met; the S3e set-up was defective"]
fn v2t_nc2_rt1_s3i_s3e_the_nonlinear_transient_integrator() {
    let mut c = Checks::new();
    let r = rod_modes();
    let omega = r.lambda.sqrt();
    let dt = 2.0 * PI / omega / 200.0;
    let a1 = shift_factor(&wlf(), T1).unwrap();
    let tau_t: Vec<f64> = TAU3.iter().map(|t| t / (0.1 * omega)).collect();
    let tau_ref: Vec<f64> = tau_t.iter().map(|t| t / a1).collect();
    let shifted = rod(equal(&W3, &tau_ref, 0.0, Some(wlf())));
    let damping = Rayleigh::from_ratio(0.02, omega, omega).unwrap();
    let o = NlTransientOptions::new(dt).damping(damping);

    // V2T: the table step against the constant T_1 restored into the constant T_2.
    let table = LoadTable::new(vec![(0.0, T1), (400.0 * dt, T1), (401.0 * dt, T2), (1600.0 * dt, T2)]).unwrap();
    let run_a = vibrate(
        &shifted,
        &o.clone().temperature_history(table),
        &r,
        &at_temperature(T1),
        800,
    );
    let mut run_b = Run::default();
    let mut first = NlIntegrator::new(&shifted, &[], &o.clone().temperature(T1)).unwrap();
    first.start(0.0, &vec![0.0; r.v0.len()], &r.v0).unwrap();
    advance(&mut first, &shifted, &at_temperature(T1), 400, &mut run_b);
    let mut second = NlIntegrator::new(&shifted, &[], &o.clone().temperature(T2)).unwrap();
    second.restore(first.state().unwrap()).unwrap();
    advance(&mut second, &shifted, &at_temperature(T1), 400, &mut run_b);
    c.holds(
        "V2T set-up (800 steps each, no halving)",
        run_a.bits.len() == 800 && run_b.bits.len() == 800 && run_a.halvings == 0 && run_b.halvings == 0,
        &format!(
            "{} and {} steps, halvings {} and {}",
            run_a.bits.len(),
            run_b.bits.len(),
            run_a.halvings,
            run_b.halvings
        ),
    );
    let first_diff = run_a.bits.iter().zip(&run_b.bits).position(|(x, y)| x != y);
    c.holds(
        "V2T temperature step against the restart at T_2: bits of u, v, a, f_int and the state bytes",
        first_diff.is_none() && run_a.bytes == run_b.bytes,
        &format!(
            "first differing step {first_diff:?}, bytes equal {}",
            run_a.bytes == run_b.bytes
        ),
    );

    // NC2 and RT1: the constant T_1 with stiffness damping, against the step and against the unshifted rod with a_1 tau.
    let run_c = vibrate(&shifted, &o.clone().temperature(T1), &r, &at_temperature(T1), 800);
    c.at_least(
        "NC2 the step run differs from the constant T_1",
        relative(&run_a.u, &run_c.u),
        1e-2,
    );
    let tau_scaled: Vec<f64> = tau_ref.iter().map(|t| a1 * t).collect();
    let unshifted = rod(equal(&W3, &tau_scaled, 0.0, None));
    let run_d = vibrate(&unshifted, &o, &r, &PointContext::default(), 800);
    c.at_most(
        "RT1 displacements, shifted with stiffness damping at T_1 against unshifted a_1 tau",
        relative(&run_c.u, &run_d.u),
        1e-10,
    );
    c.at_most(
        "RT1 committed stresses, shifted with stiffness damping at T_1 against unshifted a_1 tau",
        relative(&run_c.s, &run_d.s),
        1e-10,
    );

    // S3i: run A of ADR 0499 V1T (no damping) with a constant table against the constant temperature.
    let plain = NlTransientOptions::new(dt);
    let k = vibrate(&shifted, &plain.clone().temperature(T1), &r, &at_temperature(T1), 800);
    let t = vibrate(
        &shifted,
        &plain.temperature_history(LoadTable::constant(T1, 0.0, 1600.0 * dt).unwrap()),
        &r,
        &at_temperature(T1),
        800,
    );
    c.holds(
        "S3i constant table on the integrator gives the bits of the constant temperature",
        k.bits == t.bits && k.bytes == t.bytes,
        &format!("{} steps", k.bits.len()),
    );

    // S3e: the energy-momentum step of an elastic bar with damping.
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut bar = SolidModel::new(mesh);
    bar.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    bar.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    let n = bar.dof_map().n_equations();
    let mut vb = vec![0.0; n];
    for &nd in bar.mesh().node_set("xmax").unwrap() {
        vb[bar.dof_map().equation(nd, 0).unwrap()] = 40.0;
    }
    let jd = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-8,
    };
    let eo = NlTransientOptions::new(2e-6).damping(jd).energy_momentum();
    let bar_rod = Rod { lambda: 0.0, v0: vb };
    let ek = vibrate(
        &bar,
        &eo.clone().temperature(T1),
        &bar_rod,
        &PointContext::default(),
        30,
    );
    let et = vibrate(
        &bar,
        &eo.temperature_history(LoadTable::constant(T1, 0.0, 1.0).unwrap()),
        &bar_rod,
        &PointContext::default(),
        30,
    );
    c.holds(
        "S3e constant table on the energy-momentum step gives the bits of the constant temperature",
        ek.bits == et.bits && ek.bytes == et.bytes,
        &format!("{} steps", ek.bits.len()),
    );
    c.finish();
}

#[test]
fn nt4_rf3_refusals() {
    let mut c = Checks::new();
    let r = rod_modes();
    let omega = r.lambda.sqrt();
    let dt = 2.0 * PI / omega / 200.0;
    let n = r.v0.len();
    let shifted = rod(equal(&W3, &TAU3, 0.0, Some(wlf())));
    let damping = Rayleigh::from_ratio(0.02, omega, omega).unwrap();
    let constant = LoadTable::constant(T1, 0.0, 1.0).unwrap();

    // NT4: the shifted rod with stiffness damping and no temperature.
    let (ok, d) = refused_with(
        NlIntegrator::new(&shifted, &[], &NlTransientOptions::new(dt).damping(damping)),
        "temperature",
    );
    c.holds(
        "NT4 shifted rod with stiffness damping and no temperature refused",
        ok,
        &d,
    );

    // RF3: the driver.
    let cm = cube(equal(&W3, &TAU3, NU, Some(wlf())));
    let loading = strain_step(&cm, 2.0);
    let base_opts = TimeSteppingOptions::new(f64::INFINITY).newton(tight());
    let cases = vec![
        (
            "driver with a table and a temperature",
            base_opts.clone().temperature(T1).temperature_history(constant.clone()),
        ),
        (
            "driver with a table ending before the last time",
            base_opts
                .clone()
                .temperature_history(LoadTable::constant(T1, 0.0, 0.5).unwrap()),
        ),
        (
            "driver with a table starting after 0",
            base_opts
                .clone()
                .temperature_history(LoadTable::constant(T1, 0.1, 2.0).unwrap()),
        ),
    ];
    for (name, opts) in cases {
        let (ok, d) = refused_with(solve_in_time_with(&cm, &[], &loading, &[1.0], &opts), "temperature");
        c.holds(&format!("RF3 {name}"), ok, &d);
    }

    // RF3: the integrator.
    let (ok, d) = refused_with(
        NlIntegrator::new(
            &shifted,
            &[],
            &NlTransientOptions::new(dt)
                .temperature(T1)
                .temperature_history(constant),
        ),
        "temperature",
    );
    c.holds("RF3 Integrator::new with a table and a temperature", ok, &d);
    let late = LoadTable::constant(T1, 1.0, 2.0).unwrap();
    let mut it = NlIntegrator::new(&shifted, &[], &NlTransientOptions::new(dt).temperature_history(late)).unwrap();
    let (ok, d) = refused_with(it.start(0.0, &vec![0.0; n], &r.v0), "temperature");
    c.holds("RF3 start at a time outside the table", ok, &d);
    let short = LoadTable::constant(T1, 0.0, 10.0 * dt).unwrap();
    let mut it = NlIntegrator::new(&shifted, &[], &NlTransientOptions::new(dt).temperature_history(short)).unwrap();
    it.start(0.0, &vec![0.0; n], &r.v0).unwrap();
    let before = it.time().unwrap();
    let (ok, d) = refused_with(it.run(20), "temperature");
    let after = it.time().unwrap();
    c.holds(
        "RF3 run beyond the table refused before any step",
        ok && before.to_bits() == after.to_bits(),
        &format!("{d}; time {before:e} -> {after:e}"),
    );
    c.finish();
}

/// S3e (ADR 0526 amendment 1): the energy-momentum step of an elastic bar with damping, a constant table against the
/// constant temperature.
#[test]
fn s3e_amended_constant_table_on_the_energy_momentum_step() {
    let mut c = Checks::new();
    // S3e: the energy-momentum step of an elastic bar with damping.
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut bar = SolidModel::new(mesh);
    bar.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    bar.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    let n = bar.dof_map().n_equations();
    let mut vb = vec![0.0; n];
    for &nd in bar.mesh().node_set("xmax").unwrap() {
        vb[bar.dof_map().equation(nd, 0).unwrap()] = 40.0;
    }
    let jd = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-8,
    };
    let eo = NlTransientOptions::new(2e-6).damping(jd).energy_momentum();
    let bar_rod = Rod { lambda: 0.0, v0: vb };
    let ek = vibrate(
        &bar,
        &eo.clone().temperature(T1),
        &bar_rod,
        &PointContext::default(),
        30,
    );
    let et = vibrate(
        &bar,
        &eo.temperature_history(LoadTable::constant(T1, 0.0, 1.0).unwrap()),
        &bar_rod,
        &PointContext::default(),
        30,
    );
    c.holds(
        "S3e constant table on the energy-momentum step gives the bits of the constant temperature",
        ek.bits == et.bits && ek.bytes == et.bytes,
        &format!("{} steps", ek.bits.len()),
    );
    c.finish();
}
