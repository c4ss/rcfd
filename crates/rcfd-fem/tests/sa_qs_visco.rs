//! F-576 (ADR 0499): quasi-static viscoelastic time stepping on the Newton driver with load and displacement histories, and a
//! uniform temperature in the nonlinear transient integrator. Checks R1t, C1t, C2t, Q2 (ADR 0485 R1, C1, C2 by one call of
//! the driver), R3, C3, C4 (histories that change in time), H1, H2 (halving), S1d (temperature on the driver), V1T, NT1 to NT3
//! (temperature in the integrator), B2 (an elastic model keeps its bits with a temperature) and RF2 (refusals) of the
//! registration. Every aggregated error is required finite.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_visco::shift_factor;
use rcfd_fem::material::{PointContext, PronyTerm, TimeTemperatureShift};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    element_second_piola_kirchhoff, initial_states, solve_from_at, solve_in_time_with, NewtonOptions, TimeHistory,
    TimeLoading, TimeSteppingOptions,
};
use rcfd_fem::transient::{LoadHistory, LoadTable, MotionHistory};
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

/// `max` that keeps a NaN (b4).
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

fn three() -> Material {
    equal(&W3, &TAU3, NU, None)
}

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

/// `E(t) = E_inf + sum E_i exp(-t/tau_i)`, `E_i = w_i E_0`.
fn relaxation(w: &[f64], tau: &[f64], t: f64) -> f64 {
    let einf = E0 * (1.0 - w.iter().sum::<f64>());
    einf + w.iter().zip(tau).map(|(wi, ti)| E0 * wi * (-t / ti).exp()).sum::<f64>()
}

fn ctx(t: f64, dt: f64) -> PointContext {
    PointContext::new().with_time(t, dt).unwrap()
}

fn times(steps: usize, dt: f64) -> Vec<f64> {
    (1..=steps).map(|k| k as f64 * dt).collect()
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

fn set_x_equations(model: &SolidModel, set: &str) -> Vec<(usize, usize)> {
    let dofs = model.dof_map();
    model
        .mesh()
        .node_set(set)
        .unwrap()
        .iter()
        .map(|&n| (n, dofs.equation(n, 0).unwrap()))
        .collect()
}

/// `u_x = amplitude s(t)` at the `xmax` nodes.
fn pull_history(model: &SolidModel, amplitude: f64, table: LoadTable) -> MotionHistory {
    let list = set_x_equations(model, "xmax")
        .into_iter()
        .map(|(n, _)| Prescribed::new(n, 0, amplitude))
        .collect();
    MotionHistory::new(list, table)
}

/// The force pattern `total/4` on the `x` equations of the `xmax` nodes.
fn face_pattern(model: &SolidModel, total: f64) -> Vec<f64> {
    let mut p = vec![0.0; model.dof_map().n_equations()];
    for (_, q) in set_x_equations(model, "xmax") {
        p[q] = total / 4.0;
    }
    p
}

/// The second Piola-Kirchhoff stresses at the points of element 0 (default context: the committed stress).
fn stresses(model: &SolidModel, u: &[f64], state: &ElementState) -> Vec<[f64; 6]> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let nodes = mesh.element_nodes(0);
    let coords: Vec<[f64; 3]> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let ue: Vec<f64> = nodes
        .iter()
        .flat_map(|&n| (0..3).map(move |c| (n, c)))
        .map(|(n, c)| u[dofs.equation(n, c).unwrap()])
        .collect();
    let material = &model.materials()[0].1;
    element_second_piola_kirchhoff(element_for(mesh.element_kind(0)), &coords, material, &ue, state).unwrap()
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
    let (_, q) = set_x_equations(model, "xmax")[0];
    let a = u[q];
    a + 0.5 * a * a
}

fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

fn flat(u: &[f64], states: &[ElementState], f: &[f64]) -> Vec<u64> {
    let mut v = u.to_vec();
    v.extend_from_slice(f);
    for s in states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    bits(&v)
}

fn flat_history(h: &TimeHistory) -> Vec<Vec<u64>> {
    h.steps
        .iter()
        .map(|s| flat(&s.u, &s.states, &s.internal_force))
        .collect()
}

/// A loop of `solve_from_at` written here with the driver's time arithmetic: `with_time(0, 0)`, then `with_time(t_k, t_k -
/// t_{k-1})`, the prescribed `u_x = value` at `xmax` written into the start, the load vector `load`.
fn loop_of_solve_from_at(
    model: &SolidModel,
    value: f64,
    load: &[f64],
    ts: &[f64],
    newton: &NewtonOptions,
) -> Vec<Vec<u64>> {
    let n = model.dof_map().n_equations();
    let mut u = vec![0.0; n];
    let pres: Vec<Prescribed> = if value != 0.0 {
        set_x_equations(model, "xmax")
            .into_iter()
            .map(|(nd, _)| Prescribed::new(nd, 0, value))
            .collect()
    } else {
        Vec::new()
    };
    let eqs: Vec<usize> = set_x_equations(model, "xmax").into_iter().map(|(_, q)| q).collect();
    let mut states = initial_states(model).unwrap();
    let mut out = Vec::new();
    let mut previous = 0.0;
    for (k, &t) in std::iter::once(&0.0).chain(ts).enumerate() {
        if value != 0.0 {
            for &q in &eqs {
                u[q] = value;
            }
        }
        let p = if k == 0 { ctx(0.0, 0.0) } else { ctx(t, t - previous) };
        let eq = solve_from_at(model, &pres, load, &u, &states, newton, &[], &p).unwrap();
        u = eq.u;
        states = eq.states;
        out.push(flat(&u, &states, &eq.internal_force));
        previous = t;
    }
    out
}

// ------------------------------------------------------------------------------------------------- R1t, C1t, C2t, Q2

const EPS0: f64 = 1e-3;
const W_SLS: f64 = 0.5;
const TAU_SLS: f64 = 1.0;

fn sls() -> Material {
    equal(&[W_SLS], &[TAU_SLS], NU, None)
}

fn einf_sls() -> f64 {
    E0 * (1.0 - W_SLS)
}

fn tauc_sls() -> f64 {
    TAU_SLS * E0 / einf_sls()
}

fn creep_closed(sig0: f64, t: f64) -> f64 {
    let einf = einf_sls();
    sig0 * (1.0 / einf - (1.0 / einf - 1.0 / E0) * (-t / tauc_sls()).exp())
}

/// C1t: the largest relative error of `u_x(xmax)` over the step ends at `per` steps per `tau` to `5 tau_c`, one call.
fn creep_by_driver(per: usize) -> f64 {
    let model = cube(sls());
    let sig0 = 1e-8 * einf_sls();
    let loading = TimeLoading::new().load(LoadHistory::new(
        face_pattern(&model, sig0),
        LoadTable::constant(1.0, 0.0, 11.0).unwrap(),
    ));
    let dt = TAU_SLS / per as f64;
    let steps = (5.0 * tauc_sls() / dt).round() as usize;
    let h = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times(steps, dt),
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    let xmax = set_x_equations(&model, "xmax");
    let mut worst: f64 = 0.0;
    for s in &h.steps[1..] {
        let eps = xmax.iter().map(|&(_, q)| s.u[q]).sum::<f64>() / 4.0;
        let want = creep_closed(sig0, s.time);
        worse(&mut worst, (eps - want).abs() / want);
    }
    worst
}

#[test]
fn r1t_c1t_c2t_q2_relaxation_and_creep_by_one_call() {
    let mut c = Checks::new();
    // R1t: the strain step as a displacement history, 100 steps of 0.02 s, one call.
    let model = cube(three());
    let loading = TimeLoading::new().motion(pull_history(&model, EPS0, LoadTable::constant(1.0, 0.0, 3.0).unwrap()));
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(tight());
    let h = solve_in_time_with(&model, &[], &loading, &times(100, 0.02), &opts).unwrap();
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for s in &h.steps {
        let e = exx(&model, &s.u);
        let want = e * relaxation(&W3, &TAU3, s.time);
        for st in stresses(&model, &s.u, &s.states[0]) {
            worse(&mut worst, (st[0] - want).abs() / (E0 * e));
            count += 1;
        }
    }
    c.holds("R1t set-up", count == 101 * 8, &format!("{count} point values"));
    c.at_most(
        "R1t relaxation of one element by the driver, three branches",
        worst,
        1e-10,
    );

    // C1t, C2t: the dead load as a load history.
    let (e20, e40) = (creep_by_driver(20), creep_by_driver(40));
    c.at_most("C1t creep of one element by the driver, 20 steps per tau", e20, 1e-3);
    println!("C2t creep of one element by the driver, 40 steps per tau: {e40:.6e}");
    c.at_least("C2t observed order", (e20 / e40).log2(), 1.9);

    // Q2: the driver against a loop written here, the first 20 steps of R1t and C1t.
    let ts = times(20, 0.02);
    let h = solve_in_time_with(&model, &[], &loading, &ts, &opts).unwrap();
    let n = model.dof_map().n_equations();
    c.holds(
        "Q2 R1t, 20 steps",
        flat_history(&h) == loop_of_solve_from_at(&model, EPS0, &vec![0.0; n], &ts, &tight()),
        "bits",
    );
    let cm = cube(sls());
    let sig0 = 1e-8 * einf_sls();
    let pattern = face_pattern(&cm, sig0);
    let loading = TimeLoading::new().load(LoadHistory::new(
        pattern.clone(),
        LoadTable::constant(1.0, 0.0, 11.0).unwrap(),
    ));
    let ts = times(20, TAU_SLS / 20.0);
    let h = solve_in_time_with(&cm, &[], &loading, &ts, &TimeSteppingOptions::new(f64::INFINITY)).unwrap();
    c.holds(
        "Q2 C1t, 20 steps",
        flat_history(&h) == loop_of_solve_from_at(&cm, 0.0, &pattern, &ts, &NewtonOptions::new(1)),
        "bits",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R3, C3, C4

/// The closed form of arithmetic 3 (i): `S_xx(t)` of the strain ramp `a = r t` with `E_xx = a + a^2/2`.
fn ramp_closed(r: f64, t: f64) -> f64 {
    let winf = 1.0 - W3.iter().sum::<f64>();
    let g1 = winf * t
        + W3.iter()
            .zip(&TAU3)
            .map(|(w, ta)| w * ta * (1.0 - (-t / ta).exp()))
            .sum::<f64>();
    let g2 = winf * t * t / 2.0
        + W3.iter()
            .zip(&TAU3)
            .map(|(w, ta)| w * ta * (t - ta * (1.0 - (-t / ta).exp())))
            .sum::<f64>();
    E0 * (r * g1 + r * r * g2)
}

/// C3: the creep under the load ramp `sigma = s' t`, largest relative error over the step ends `k >= 1`.
fn ramp_creep_by_driver(per: usize) -> f64 {
    let model = cube(sls());
    let t_end = 5.0 * tauc_sls();
    let sig_end = 1e-8 * einf_sls();
    let rate = sig_end / t_end;
    // The factor (0, 0), (20, 2): s(t) = t/10 with t_end = 10 s.
    let loading = TimeLoading::new().load(LoadHistory::new(
        face_pattern(&model, sig_end),
        LoadTable::new(vec![(0.0, 0.0), (20.0, 2.0)]).unwrap(),
    ));
    let dt = TAU_SLS / per as f64;
    let steps = (t_end / dt).round() as usize;
    let h = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times(steps, dt),
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    let xmax = set_x_equations(&model, "xmax");
    let einf = einf_sls();
    let tauc = tauc_sls();
    let mut worst: f64 = 0.0;
    for s in &h.steps[1..] {
        let t = s.time;
        let eps = xmax.iter().map(|&(_, q)| s.u[q]).sum::<f64>() / 4.0;
        let want = rate * (t / einf - (1.0 / einf - 1.0 / E0) * tauc * (1.0 - (-t / tauc).exp()));
        worse(&mut worst, (eps - want).abs() / want);
    }
    worst
}

#[test]
fn r3_c3_c4_histories_that_change_in_time() {
    let mut c = Checks::new();
    // R3: the strain ramp u_x = 1e-3 t through a displacement history.
    let model = cube(three());
    let r = 1e-3;
    let loading = TimeLoading::new().motion(pull_history(
        &model,
        r,
        LoadTable::new(vec![(0.0, 0.0), (2.0, 2.0)]).unwrap(),
    ));
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(tight());
    let h = solve_in_time_with(&model, &[], &loading, &times(100, 0.01), &opts).unwrap();
    let scale = E0 * (r + 0.5 * r * r);
    let mut worst: f64 = 0.0;
    for s in &h.steps {
        let want = ramp_closed(r, s.time);
        for st in stresses(&model, &s.u, &s.states[0]) {
            worse(&mut worst, (st[0] - want).abs() / scale);
        }
    }
    c.at_most("R3 strain ramp through a displacement history", worst, 1e-7);
    // C3, C4: the load ramp through a load history.
    let (e20, e40) = (ramp_creep_by_driver(20), ramp_creep_by_driver(40));
    c.at_most("C3 creep under a load ramp, 20 steps per tau", e20, 1e-3);
    println!("C3 creep under a load ramp, 40 steps per tau: {e40:.6e}");
    c.at_least("C4 observed order", (e20 / e40).log2(), 1.9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- H1, H2

/// The cube of arithmetic 4: nu = 0, the lateral motion held at every node, `u_x = 0` on `xmin`.
fn uniaxial_cube() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", equal(&W3, &TAU3, 0.0, None)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn one_correction() -> NewtonOptions {
    let mut o = NewtonOptions::new(1).max_iterations(1);
    o.residual_tolerance = 1.5e-2;
    o.energy_tolerance = 1.0;
    o
}

#[test]
fn h1_h2_halving_with_a_load_history() {
    let mut c = Checks::new();
    let model = uniaxial_cube();
    let loading = TimeLoading::new().load(LoadHistory::new(
        face_pattern(&model, 0.01 * E0),
        LoadTable::new(vec![(0.0, 0.0), (1.0, 1.0)]).unwrap(),
    ));
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(one_correction());
    let halved = solve_in_time_with(&model, &[], &loading, &[1.0], &opts).unwrap();
    let s = &halved.steps[1];
    c.holds(
        "H1 one halving, two sub-steps",
        s.halvings == 1 && s.substeps == 2,
        &format!("{} halvings, {} sub-steps", s.halvings, s.substeps),
    );
    let explicit = solve_in_time_with(&model, &[], &loading, &[0.5, 1.0], &opts).unwrap();
    let none = explicit.steps.iter().all(|s| s.halvings == 0);
    let a = flat_history(&halved);
    let b = flat_history(&explicit);
    c.holds(
        "H1 the halved step equals the explicit sub-steps",
        none && a.len() == 2 && b.len() == 3 && a[0] == b[0] && a[1] == b[2],
        &format!(
            "bits; explicit halvings {:?}",
            explicit.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
        ),
    );
    println!("H1 u_x(1 s) = {:.6e}", s.u[set_x_equations(&model, "xmax")[0].1]);
    // H2: no halving allowed.
    let r = solve_in_time_with(&model, &[], &loading, &[1.0], &opts.clone().max_halvings(0));
    let (ok, detail) = match &r {
        Err(e @ FemError::Solver { solver, reason }) => (
            *solver == "creep time stepping" && reason.contains("step 1 of 1") && reason.contains("not converged"),
            format!("{e}"),
        ),
        Err(e) => (false, format!("{e}")),
        Ok(_) => (false, "accepted".to_string()),
    };
    c.holds("H2 refused without a halving", ok, &detail);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1d

fn wlf_formula(t: f64) -> f64 {
    10f64.powf(-17.44 * (t - T_REF) / (51.6 + t - T_REF))
}

#[test]
fn s1d_shifted_relaxation_by_the_driver_with_a_temperature() {
    let mut c = Checks::new();
    let temp = T_REF + 10.0;
    let a = wlf_formula(temp);
    let model = cube(equal(&W3, &TAU3, NU, Some(wlf())));
    let loading = TimeLoading::new().motion(pull_history(&model, EPS0, LoadTable::constant(1.0, 0.0, 1.0).unwrap()));
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature(temp);
    let h = solve_in_time_with(&model, &[], &loading, &times(100, 0.02 * a), &opts).unwrap();
    let material = &model.materials()[0].1;
    let at = PointContext::new().with_temperature(temp).unwrap();
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for s in &h.steps {
        let e = exx(&model, &s.u);
        let want = e * relaxation(&W3, &TAU3, s.time / a);
        for sxx in committed_sxx(material, &s.states, &at) {
            worse(&mut worst, (sxx - want).abs() / (E0 * e));
            count += 1;
        }
    }
    c.holds("S1d set-up", count == 101 * 8, &format!("{count} point values"));
    c.at_most("S1d shifted relaxation by the driver at T", worst, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- V1T, NT1, NT2, NT3, B2

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

/// A free vibration run: `u` and the committed `S_xx` of every point (at the context with `temperature`) at every step end.
struct Run {
    u: Vec<Vec<f64>>,
    s: Vec<Vec<f64>>,
}

fn vibrate(material: Material, temperature: f64, r: &Rod, dt: f64, steps: usize) -> Run {
    let model = rod(material);
    let opts = NlTransientOptions::new(dt).temperature(temperature);
    let mut it = NlIntegrator::new(&model, &[], &opts).unwrap();
    it.start(0.0, &vec![0.0; r.v0.len()], &r.v0).unwrap();
    let at = PointContext::new().with_temperature(temperature).unwrap();
    let material = &model.materials()[0].1;
    let mut run = Run {
        u: Vec::new(),
        s: Vec::new(),
    };
    for _ in 0..steps {
        it.step().unwrap();
        let st = it.state().unwrap();
        run.s.push(committed_sxx(material, &st.states, &at));
        run.u.push(st.u);
    }
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

#[test]
fn v1t_nt1_temperature_in_the_nonlinear_transient_integrator() {
    let mut c = Checks::new();
    let r = rod_modes();
    let omega = r.lambda.sqrt();
    let dt = 2.0 * PI / omega / 200.0;
    let temp = T_REF + 10.0;
    let a = shift_factor(&wlf(), temp).unwrap();
    println!("V1T a_T = {a:.9e}");
    let tau_t: Vec<f64> = TAU3.iter().map(|t| t / (0.1 * omega)).collect();
    let tau_ref: Vec<f64> = tau_t.iter().map(|t| t / a).collect();
    let tau_scaled: Vec<f64> = tau_ref.iter().map(|t| a * t).collect();
    let run_a = vibrate(equal(&W3, &tau_ref, 0.0, Some(wlf())), temp, &r, dt, 800);
    let run_b = vibrate(equal(&W3, &tau_scaled, 0.0, Some(wlf())), T_REF, &r, dt, 800);
    c.at_most(
        "V1T displacements at T against T_ref with a_T tau",
        relative(&run_a.u, &run_b.u),
        1e-10,
    );
    c.at_most(
        "V1T committed stresses at T against T_ref with a_T tau",
        relative(&run_a.s, &run_b.s),
        1e-10,
    );
    let run_c = vibrate(equal(&W3, &tau_ref, 0.0, Some(wlf())), T_REF, &r, dt, 800);
    c.at_least(
        "NT1 the run at T_ref without scaling differs",
        relative(&run_a.u, &run_c.u),
        1e-2,
    );
    c.finish();
}

#[test]
fn nt2_nt3_b2_rf2_refusals_and_bits() {
    let mut c = Checks::new();
    let r = rod_modes();
    let dt = 2.0 * PI / r.lambda.sqrt() / 200.0;
    let n = r.v0.len();
    // NT2: the shifted rod without a temperature.
    let sr = rod(equal(&W3, &TAU3, 0.0, Some(wlf())));
    let mut it = NlIntegrator::new(&sr, &[], &NlTransientOptions::new(dt)).unwrap();
    let (ok, d) = refused_with(it.start(0.0, &vec![0.0; n], &r.v0), "temperature");
    c.holds("NT2 shifted rod without a temperature refused at start", ok, &d);
    // NT3: a NaN temperature.
    let (ok, d) = refused_with(
        NlIntegrator::new(&sr, &[], &NlTransientOptions::new(dt).temperature(f64::NAN)),
        "temperature",
    );
    c.holds("NT3 Integrator::new with a NaN temperature", ok, &d);
    let cm = cube(three());
    let (ok, d) = refused_with(
        solve_in_time_with(
            &cm,
            &[],
            &TimeLoading::new(),
            &[1.0],
            &TimeSteppingOptions::new(f64::INFINITY).temperature(f64::NAN),
        ),
        "temperature",
    );
    c.holds("NT3 solve_in_time_with with a NaN temperature", ok, &d);

    // B2: the elastic rod with and without a temperature.
    let run = |temperature: Option<f64>| {
        let model = rod(base(0.0));
        let mut o = NlTransientOptions::new(dt);
        if let Some(t) = temperature {
            o = o.temperature(t);
        }
        let mut it = NlIntegrator::new(&model, &[], &o).unwrap();
        it.start(0.0, &vec![0.0; n], &r.v0).unwrap();
        let mut v = Vec::new();
        for _ in 0..100 {
            it.step().unwrap();
            let s = it.state().unwrap();
            v.extend_from_slice(&s.u);
            v.extend_from_slice(&s.v);
            v.extend_from_slice(&s.a);
            v.extend_from_slice(&s.internal_force);
        }
        (bits(&v), it.state().unwrap().to_bytes())
    };
    c.holds(
        "B2 elastic rod with a temperature keeps its bits",
        run(None) == run(Some(383.0)),
        "bits and bytes",
    );

    // RF2: the refusals of the histories.
    let ne = cm.dof_map().n_equations();
    let opts = TimeSteppingOptions::new(f64::INFINITY).newton(tight());
    let one = LoadTable::constant(1.0, 0.0, 2.0).unwrap();
    let cases: Vec<(&str, TimeLoading, &str)> = vec![
        (
            "pattern of the wrong length",
            TimeLoading::new().load(LoadHistory::new(vec![0.0; ne - 1], one.clone())),
            "load pattern",
        ),
        (
            "pattern with a NaN",
            TimeLoading::new().load(LoadHistory::new(
                {
                    let mut p = vec![0.0; ne];
                    p[ne - 1] = f64::NAN;
                    p
                },
                one.clone(),
            )),
            "load pattern",
        ),
        (
            "load table ending before the last time",
            TimeLoading::new().load(LoadHistory::new(
                vec![0.0; ne],
                LoadTable::constant(1.0, 0.0, 0.5).unwrap(),
            )),
            "load history",
        ),
        (
            "motion table starting after 0",
            TimeLoading::new().motion(pull_history(&cm, EPS0, LoadTable::constant(1.0, 0.1, 2.0).unwrap())),
            "motion history",
        ),
        (
            "motion of an equation the model prescribes",
            TimeLoading::new().motion(MotionHistory::new(
                vec![Prescribed::new(set_x_equations(&cm, "xmin")[0].0, 0, EPS0)],
                one.clone(),
            )),
            "motion history",
        ),
        (
            "the same equation in two motions",
            TimeLoading::new()
                .motion(pull_history(&cm, EPS0, one.clone()))
                .motion(pull_history(&cm, EPS0, one.clone())),
            "motion history",
        ),
    ];
    for (name, loading, key) in cases {
        let (ok, d) = refused_with(solve_in_time_with(&cm, &[], &loading, &[1.0], &opts), key);
        c.holds(&format!("RF2 {name}"), ok, &d);
    }
    c.finish();
}
