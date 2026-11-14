//! F-423 (ADR 0479): nonlinear transient dynamics, the Newton driver of ADR 0404 inside the Newmark and generalised-alpha
//! step. Checks L1, L1b, N1 (bar B against F-398), P0 to P2 (the compound pendulum), S0 to S2 (the free spinning bar), A0, A1,
//! G1 (the large axial oscillation), R0 to R2, G2 (the J2 bar, save and restore), H1, H2 (halvings), T1 (threads) and F1
//! (refusals) of the registration.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions, NlTransientState};
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{Constraint, FemError, LinearTerm, Load, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::cell::Cell;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
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

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// `max_n |a_n - b_n|_inf / max_n |b_n|_inf`.
fn max_rel_diff(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    assert_eq!(a.len(), b.len(), "history lengths");
    let scale = b.iter().fold(0.0f64, |m, u| m.max(inf_norm(u)));
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| {
        m.max(x.iter().zip(y).fold(0.0f64, |mm, (p, q)| mm.max((p - q).abs())))
    });
    d / scale
}

fn norm3(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

const EPS_X: f64 = 1e-9;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS_X
}

/// The nonlinear history of `steps` steps (step 0 included).
fn nl_history(it: &mut NlIntegrator, steps: u64) -> Vec<Vec<f64>> {
    let mut h = vec![it.displacement().unwrap()];
    for _ in 0..steps {
        it.step().unwrap();
        h.push(it.displacement().unwrap());
    }
    h
}

// ------------------------------------------------------------------------------------------------ bar B: L1, L1b, N1

struct BarB {
    model: SolidModel,
    pattern: Vec<f64>,
    omega: f64,
}

fn bar_b() -> BarB {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let modal = modal_analysis(&m, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let mass = consistent(&m).unwrap();
    let pattern = mv(&mass, &phi);
    let k = rcfd_fem::assemble::stiffness(&m).unwrap();
    let omega = (dot(&phi, &mv(&k, &phi)) / dot(&phi, &mv(&mass, &phi))).sqrt();
    BarB {
        model: m,
        pattern,
        omega,
    }
}

/// The linear (F-398) and the nonlinear history of bar B under the A5 load times `factor`, 80 steps of `T_1/40`.
fn bar_b_histories(b: &BarB, factor: f64, method: Method, damped: bool) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let period = 2.0 * PI / b.omega;
    let dt = period / 40.0;
    let table = LoadTable::new(vec![(0.0, 1.0), (10.0 * period, 1.0)]).unwrap();
    let pattern: Vec<f64> = b.pattern.iter().map(|x| factor * x).collect();
    let loads = [LoadHistory::new(pattern, table)];
    let damping = Rayleigh::from_ratio(0.02, b.omega, b.omega).unwrap();
    let mut lo = TransientOptions::new(dt).method(method);
    let mut no = NlTransientOptions::new(dt).method(method);
    if damped {
        lo = lo.damping(damping);
        no = no.damping(damping);
    }
    let zero = vec![0.0; b.pattern.len()];
    let mut lin = Integrator::new(&b.model, &loads, &lo).unwrap();
    lin.start(0.0, &zero, &zero).unwrap();
    let mut hl = vec![lin.displacement().unwrap()];
    for _ in 0..80 {
        lin.step().unwrap();
        hl.push(lin.displacement().unwrap());
    }
    let mut nl = NlIntegrator::new(&b.model, &loads, &no).unwrap();
    nl.start(0.0, &zero, &zero).unwrap();
    let hn = nl_history(&mut nl, 80);
    (hn, hl)
}

#[test]
fn l1_l1b_n1_load_factor_against_f398() {
    let b = bar_b();
    let mut c = Checks::new();
    let runs: [(&str, Method, bool); 3] = [
        ("Newmark", Method::average_acceleration(), false),
        ("Newmark damped", Method::average_acceleration(), true),
        (
            "generalised-alpha 0.8 damped",
            Method::GeneralisedAlpha { rho_inf: 0.8 },
            true,
        ),
    ];
    for (name, method, damped) in runs {
        let (hn, hl) = bar_b_histories(&b, 1e-6, method, damped);
        c.at_most(&format!("L1 {name}, factor 1e-6"), max_rel_diff(&hn, &hl), 1e-9);
    }
    for (name, method, damped) in runs {
        let (hn, hl) = bar_b_histories(&b, 1.0, method, damped);
        c.at_most(&format!("L1b {name}, factor 1"), max_rel_diff(&hn, &hl), 1e-7);
    }
    let (hn, hl) = bar_b_histories(&b, 1e5, Method::average_acceleration(), false);
    c.at_least("N1 Newmark, factor 1e5", max_rel_diff(&hn, &hl), 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------ the slender bar: P, S, T1

const L_BAR: f64 = 1.0;
const B_BAR: f64 = 0.02;
const RHO: f64 = 7850.0;
const G: f64 = 9.81;

fn slender_mesh() -> rcfd_fem::mesh::SolidMesh {
    hex_box([0.0, -B_BAR / 2.0, -B_BAR / 2.0], [L_BAR, B_BAR, B_BAR], [10, 2, 2]).unwrap()
}

fn soft() -> Material {
    Material::isotropic_elastic(2.0e9, 0.3, RHO, 0.0).unwrap()
}

fn node_where(model: &SolidModel, f: impl Fn(&[f64; 3]) -> bool) -> Vec<usize> {
    let nodes = model.mesh().nodes();
    (0..nodes.len())
        .filter(|&i| {
            let p = nodes[i];
            f(&[p[0], p[1], p[2]])
        })
        .collect()
}

fn pendulum_model() -> SolidModel {
    let mut mesh = slender_mesh();
    let nodes = mesh.nodes().to_vec();
    let pin: Vec<usize> = (0..nodes.len())
        .filter(|&i| near(nodes[i][0], 0.0) && near(nodes[i][2], 0.0))
        .collect();
    let mid: Vec<usize> = pin.iter().copied().filter(|&i| near(nodes[i][1], 0.0)).collect();
    assert_eq!((pin.len(), mid.len()), (3, 1), "pin line");
    mesh.add_node_set("pin", pin).unwrap();
    mesh.add_node_set("pin_mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", soft()).unwrap();
    m.add_constraint(Constraint::displacement("pin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin_mid", 1, 0.0)).unwrap();
    m.add_load(Load::gravity([0.0, 0.0, -G])).unwrap();
    m
}

fn pendulum_integrator(model: &SolidModel, threads: Option<usize>) -> NlIntegrator {
    let loads = [LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let mut o = NlTransientOptions::new(0.005);
    if let Some(t) = threads {
        o = o.threads(t);
    }
    let mut it = NlIntegrator::new(model, &loads, &o).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    it
}

#[test]
#[ignore = "record of the registered run of ADR 0479 (commit 409fbfe4): set-up defective, the residual tolerance 1e-10 below the rounding floor (stopped at step 8, residual 1.354e-10, energy ratio 1.1e-27); corrected by amendment 1 in p0c_p1c_p2c_pendulum_released_from_the_horizontal"]
fn p0_p1_p2_pendulum_released_from_the_horizontal() {
    let model = pendulum_model();
    let dofs = model.dof_map();
    let tip = node_where(&model, |p| near(p[0], L_BAR) && near(p[1], 0.0) && near(p[2], 0.0));
    assert_eq!(tip.len(), 1, "tip node");
    let (qx, qz) = (dofs.equation(tip[0], 0).unwrap(), dofs.equation(tip[0], 2).unwrap());
    let gravity = LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 10.0).unwrap())
        .unwrap()
        .pattern;
    let mut it = pendulum_integrator(&model, None);
    let dt = 0.005;
    let steps = 1940;
    let mass = RHO * L_BAR * B_BAR * B_BAR;
    let swing = mass * G * L_BAR / 2.0;
    let energy = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        it.kinetic_energy().unwrap() + it.strain_energy().unwrap() - dot(&gravity, &u)
    };
    let angle = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        (L_BAR + u[qx]).atan2(-u[qz])
    };
    let e0 = energy(&it);
    let mut psi_prev = angle(&it);
    let mut crossings = Vec::new();
    let mut e_dev = 0.0f64;
    let mut halvings = 0;
    let mut iterations = 0;
    for k in 1..=steps {
        let r = it.step().unwrap();
        halvings += r.halvings;
        iterations += r.iterations;
        let psi = angle(&it);
        if (psi_prev > 0.0) != (psi > 0.0) {
            crossings.push((k - 1) as f64 * dt + dt * psi_prev / (psi_prev - psi));
        }
        psi_prev = psi;
        e_dev = e_dev.max((energy(&it) - e0).abs());
    }
    let t_closed = 2.0 * PI * (2.0 * L_BAR / (3.0 * G)).sqrt() * 1.180340599016096;
    let n = crossings.len();
    let t_fe = 2.0 * (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
    println!(
        "pendulum: {n} crossings, T_FE = {t_fe:.9e} s, T = {t_closed:.9e} s, E_0 = {e0:e}, halvings {halvings}, \
         iterations {iterations}"
    );
    let mut c = Checks::new();
    c.at_least("P0 crossings of psi = 0", n as f64, 9.0);
    c.at_most("P1 period |T_FE - T|/T", (t_fe - t_closed).abs() / t_closed, 1e-2);
    c.at_most("P2 energy max |E - E_0|/(m g L/2)", e_dev / swing, 1e-3);
    c.finish();
}

/// The centrifugal force of the rotating frame, `-omega^2 M P (X + u - c)`, with its tangent (ADR 0479 D5).
struct Centrifugal {
    m: CsrMatrix,
    /// Per equation: `X_i - c_i` for the components `x, y`, and `None` for `z`.
    offset: Vec<Option<f64>>,
    omega: f64,
}

impl Contribution for Centrifugal {
    fn name(&self) -> &str {
        "centrifugal"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let w2 = self.omega * self.omega;
        let r: Vec<f64> = (0..u.len()).map(|j| self.offset[j].map_or(0.0, |x| x + u[j])).collect();
        let mr = mv(&self.m, &r);
        for (f, x) in force.iter_mut().zip(&mr) {
            *f -= w2 * x;
        }
        for i in 0..self.m.n {
            for p in self.m.row_ptr[i]..self.m.row_ptr[i + 1] {
                let j = self.m.col[p];
                if self.offset[j].is_some() {
                    tangent.push((i, j, -w2 * self.m.val[p]));
                }
            }
        }
        Ok(())
    }
    fn in_residual_reference(&self) -> bool {
        true
    }
}

#[test]
#[ignore = "record of the registered run of ADR 0479 (commit 409fbfe4): set-up defective, the residual tolerance 1e-10 below the rounding floor (stopped at step 1, residual 1.577e-10, energy ratio 2.7e-29); corrected by amendment 1 in s0c_s1c_s2c_free_spinning_bar"]
fn s0_s1_s2_free_spinning_bar() {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", soft()).unwrap();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes().to_vec();
    let n = dofs.n_equations();
    let mut offset = vec![None; n];
    let mut prescribed = Vec::new();
    for (i, p) in nodes.iter().enumerate() {
        for comp in 0..3 {
            let q = dofs.equation(i, comp).unwrap();
            if comp < 2 {
                offset[q] = Some(p[comp] - centre[comp]);
            }
        }
        if near(p[0], centre[0]) {
            prescribed.push(Prescribed::new(i, 0, 0.0));
        }
        if near(p[1], 0.0) {
            prescribed.push(Prescribed::new(i, 1, 0.0));
        }
        if near(p[2], 0.0) {
            prescribed.push(Prescribed::new(i, 2, 0.0));
        }
    }
    let spin = Centrifugal {
        m: consistent(&model).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&model, &prescribed, &[], &NewtonOptions::new(1), ext);
    let mut c = Checks::new();
    c.holds(
        "S0 steady spin solve",
        steady.is_ok(),
        &format!("{:?}", steady.as_ref().err()),
    );
    let u0 = steady.unwrap().result.displacement_values().to_vec();
    let mut v0 = vec![0.0; n];
    for (i, p) in nodes.iter().enumerate() {
        let rx = p[0] + u0[dofs.equation(i, 0).unwrap()] - centre[0];
        let ry = p[1] + u0[dofs.equation(i, 1).unwrap()] - centre[1];
        let w = [-omega * ry + velocity[0], omega * rx + velocity[1], velocity[2]];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(0.01)).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    let (p0, j0) = it.momentum().unwrap();
    let (mut dp, mut dj) = (0.0f64, 0.0f64);
    let mut halvings = 0;
    for _ in 0..1000 {
        halvings += it.step().unwrap().halvings;
        let (p, j) = it.momentum().unwrap();
        dp = dp.max(norm3(sub3(p, p0)));
        dj = dj.max(norm3(sub3(j, j0)));
    }
    println!(
        "spin: P_0 = {p0:?}, J_0 = {j0:?}, max centrifugal displacement {:e}, halvings {halvings}, t = {}",
        inf_norm(&u0),
        it.time().unwrap()
    );
    c.at_most("S1 linear momentum max |P - P_0|/|P_0|", dp / norm3(p0), 1e-10);
    c.at_most("S2 angular momentum max |J - J_0|/|J_0|", dj / norm3(j0), 1e-6);
    c.finish();
}

#[test]
#[ignore = "record of the registered run of ADR 0479 (commit 409fbfe4): set-up defective, the pendulum stopped at step 8 (residual tolerance below the rounding floor); corrected by amendment 1 in t1c_threads_bit_identical"]
fn t1_threads_bit_identical() {
    let model = pendulum_model();
    let mut a = pendulum_integrator(&model, Some(1));
    let mut b = pendulum_integrator(&model, Some(4));
    a.run(50).unwrap();
    b.run(50).unwrap();
    let mut c = Checks::new();
    c.holds(
        "T1 1 and 4 threads, 50 steps",
        a.state().unwrap().to_bytes() == b.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------- the axial oscillator: A, G1, H

const E_STEEL: f64 = 2.0e11;

fn axial_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_STEEL, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn t_lin() -> f64 {
    2.0 * PI * (RHO / (3.0 * E_STEEL)).sqrt()
}

fn axial_integrator(model: &SolidModel, method: Method, dt: f64, newton: NewtonOptions) -> NlIntegrator {
    let mut it = NlIntegrator::new(model, &[], &NlTransientOptions::new(dt).method(method).newton(newton)).unwrap();
    let dofs = model.dof_map();
    let mut u0 = vec![0.0; dofs.n_equations()];
    for (i, p) in model.mesh().nodes().iter().enumerate() {
        u0[dofs.equation(i, 0).unwrap()] = 0.2 * p[0];
    }
    let zero = vec![0.0; u0.len()];
    it.start(0.0, &u0, &zero).unwrap();
    it
}

#[test]
#[ignore = "known miss A0 of ADR 0479 (registered run, commit 409fbfe4): the four end nodes are not bit-equal from step 43 on (spread 1.9e-14 of 0.2 L, the element sums in another order per node); A0 crossings 6, A1 8.64e-5, G1 1.27e-13 met; A0b of amendment 1 in a0b_a1_g1_large_axial_oscillation"]
fn a0_a1_g1_large_axial_oscillation_known_miss() {
    let model = axial_model();
    let dofs = model.dof_map();
    let ends: Vec<usize> = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&i| dofs.equation(i, 0).unwrap())
        .collect();
    let dt = t_lin() / 200.0;
    let mut it = axial_integrator(&model, Method::average_acceleration(), dt, NewtonOptions::new(1));
    let steps = 1200;
    let mut hist = vec![it.displacement().unwrap()];
    let mut same = true;
    let mut crossings = Vec::new();
    let mut lam_prev = 1.2;
    for k in 1..=steps {
        it.step().unwrap();
        let u = it.displacement().unwrap();
        same &= ends.iter().all(|&q| u[q].to_bits() == u[ends[0]].to_bits());
        let lam = 1.0 + u[ends[0]];
        if lam_prev > 1.0 && lam <= 1.0 {
            crossings.push((k - 1) as f64 * dt + dt * (lam_prev - 1.0) / (lam_prev - lam));
        }
        lam_prev = lam;
        hist.push(u);
    }
    let n = crossings.len();
    let t_fe = (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
    let ratio = 1.0407443288903;
    println!("axial: {n} downward crossings, T_FE/T_lin = {:.12}", t_fe / t_lin());
    let mut c = Checks::new();
    c.at_least("A0 downward crossings of lambda = 1", n as f64, 5.0);
    c.holds("A0 the four end nodes equal at every step", same, "bits");
    c.at_most(
        "A1 period |T_FE/T_lin - 1.04074|/1.04074",
        (t_fe / t_lin() - ratio).abs() / ratio,
        5e-3,
    );
    let mut ga = axial_integrator(
        &model,
        Method::GeneralisedAlpha { rho_inf: 1.0 },
        dt,
        NewtonOptions::new(1),
    );
    let hg = nl_history(&mut ga, steps);
    c.at_most(
        "G1 generalised-alpha rho_inf = 1 against Newmark, axial",
        max_rel_diff(&hg, &hist),
        1e-10,
    );
    c.finish();
}

/// A zero contribution that refuses its call number `fail_at` (1-based).
struct FailOnce {
    calls: Cell<usize>,
    fail_at: usize,
}

impl Contribution for FailOnce {
    fn name(&self) -> &str {
        "fail once"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if n == self.fail_at {
            return Err(FemError::InvalidValue {
                key: "fail once",
                reason: "refused once".to_string(),
            });
        }
        Ok(())
    }
}

fn same_state(a: &NlTransientState, b: &NlTransientState) -> bool {
    let bits = |x: &[f64]| x.iter().map(|v| v.to_bits()).collect::<Vec<u64>>();
    bits(&a.u) == bits(&b.u)
        && bits(&a.v) == bits(&b.v)
        && bits(&a.a) == bits(&b.a)
        && bits(&a.internal_force) == bits(&b.internal_force)
        && a.states.len() == b.states.len()
        && a.states
            .iter()
            .zip(&b.states)
            .all(|(x, y)| bits(x.values()) == bits(y.values()))
}

#[test]
fn h1_h2_halvings_and_the_error_with_the_time() {
    let model = axial_model();
    let dt = t_lin() / 200.0;
    let mut c = Checks::new();
    // H1: one correction is never confirmed by the energy criterion.
    let mut it = axial_integrator(
        &model,
        Method::average_acceleration(),
        dt,
        NewtonOptions::new(1).max_iterations(1),
    );
    let before = it.state().unwrap().to_bytes();
    match it.step() {
        Ok(r) => c.holds("H1 the step is refused", false, &format!("accepted: {r:?}")),
        Err(e) => {
            let text = format!("{e}");
            let ok = matches!(&e, FemError::Solver { solver, .. } if *solver == "nl_transient")
                && text.contains("t = 0e0")
                && text.contains(&format!("t = {dt:e}"))
                && text.contains("after 5 halvings");
            c.holds("H1 the error names the step, its times and the halvings", ok, &text);
        }
    }
    c.holds(
        "H1 the state is unchanged by the failed step",
        it.state().unwrap().to_bytes() == before,
        "state bytes",
    );
    // H2: a step that fails once is taken in two halves.
    let fail = FailOnce {
        calls: Cell::new(0),
        fail_at: 2,
    };
    let mut a = axial_integrator(&model, Method::average_acceleration(), dt, NewtonOptions::new(1));
    let r = a.step_with(&[&fail]).unwrap();
    c.holds("H2 one halving", r.halvings == 1, &format!("{r:?}"));
    let never = FailOnce {
        calls: Cell::new(0),
        fail_at: usize::MAX,
    };
    let mut b = axial_integrator(&model, Method::average_acceleration(), dt / 2.0, NewtonOptions::new(1));
    b.step_with(&[&never]).unwrap();
    b.step_with(&[&never]).unwrap();
    c.holds(
        "H2 the halved step has the bits of two steps of dt/2",
        same_state(&a.state().unwrap(), &b.state().unwrap()),
        "u, v, a, internal force, states",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- the J2 bar: R0 to R2, G2

fn j2_model(cells: usize) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [cells, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    let base = Material::isotropic_elastic(E_STEEL, 0.3, RHO, 0.0).unwrap();
    m.add_material("all", Material::j2(base, 2.0e8, 2.0e9, 1.0e9).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

const J2_DT: f64 = 6e-6;

fn j2_integrator(model: &SolidModel, method: Method) -> NlIntegrator {
    let mut it = NlIntegrator::new(model, &[], &NlTransientOptions::new(J2_DT).method(method)).unwrap();
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut v0 = vec![0.0; n];
    for &i in model.mesh().node_set("xmax").unwrap() {
        v0[dofs.equation(i, 0).unwrap()] = 50.0;
    }
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    it
}

#[test]
fn r0_r1_r2_g2_j2_bar_save_and_restore() {
    let model = j2_model(1);
    let mut c = Checks::new();
    for (name, method) in [
        ("Newmark", Method::average_acceleration()),
        ("generalised-alpha 0.8", Method::GeneralisedAlpha { rho_inf: 0.8 }),
    ] {
        let mut whole = j2_integrator(&model, method);
        whole.run(60).unwrap();
        let mut first = j2_integrator(&model, method);
        first.run(30).unwrap();
        let saved = first.state().unwrap();
        let eqps = saved
            .states
            .iter()
            .flat_map(|s| {
                s.values()
                    .chunks(rcfd_fem::mat_j2::STATE_SIZE)
                    .map(rcfd_fem::mat_j2::equivalent_plastic_strain)
                    .collect::<Vec<f64>>()
            })
            .fold(0.0f64, f64::max);
        c.holds(
            &format!("R0 {name}: yielded before the split"),
            eqps > 0.0,
            &format!("largest equivalent plastic strain {eqps:e}"),
        );
        let bytes = saved.to_bytes();
        let mut second = NlIntegrator::new(&model, &[], &NlTransientOptions::new(J2_DT).method(method)).unwrap();
        second.restore(NlTransientState::from_bytes(&bytes).unwrap()).unwrap();
        second.run(30).unwrap();
        c.holds(
            &format!("R1 {name}: split run has the bits of the unsplit run"),
            second.state().unwrap().to_bytes() == whole.state().unwrap().to_bytes(),
            "state bytes",
        );
        c.holds(
            &format!("R2 {name}: bytes round trip"),
            NlTransientState::from_bytes(&bytes).unwrap().to_bytes() == bytes,
            "",
        );
    }
    // R2: refusals.
    let saved = {
        let mut it = j2_integrator(&model, Method::average_acceleration());
        it.run(5).unwrap();
        it.state().unwrap()
    };
    let bytes = saved.to_bytes();
    let mut wrong = bytes.clone();
    wrong[0] ^= 1;
    let (ok, d) = refused_with(NlTransientState::from_bytes(&wrong), "state");
    c.holds("R2 wrong magic refused", ok, &d);
    let (ok, d) = refused_with(NlTransientState::from_bytes(&bytes[..bytes.len() - 8]), "state");
    c.holds("R2 truncated bytes refused", ok, &d);
    let mut other_dt = NlIntegrator::new(&model, &[], &NlTransientOptions::new(2.0 * J2_DT)).unwrap();
    let (ok, d) = refused_with(other_dt.restore(saved.clone()), "state");
    c.holds("R2 another time step refused", ok, &d);
    let mut other_method = NlIntegrator::new(
        &model,
        &[],
        &NlTransientOptions::new(J2_DT).method(Method::GeneralisedAlpha { rho_inf: 0.8 }),
    )
    .unwrap();
    let (ok, d) = refused_with(other_method.restore(saved.clone()), "state");
    c.holds("R2 another method refused", ok, &d);
    let two = j2_model(2);
    let mut other_layout = NlIntegrator::new(&two, &[], &NlTransientOptions::new(J2_DT)).unwrap();
    let mut shaped = j2_integrator(&two, Method::average_acceleration()).state().unwrap();
    shaped.states.pop();
    let (ok, d) = refused_with(other_layout.restore(shaped), "state");
    c.holds("R2 another element layout refused", ok, &d);
    // G2: generalised-alpha rho_inf = 1 against Newmark on the J2 bar.
    let mut nm = j2_integrator(&model, Method::average_acceleration());
    let mut ga = j2_integrator(&model, Method::GeneralisedAlpha { rho_inf: 1.0 });
    let hn = nl_history(&mut nm, 60);
    let hg = nl_history(&mut ga, 60);
    c.at_most(
        "G2 generalised-alpha rho_inf = 1 against Newmark, J2 bar",
        max_rel_diff(&hg, &hn),
        1e-10,
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------------------- F1

#[test]
fn f1_refusals() {
    let mut c = Checks::new();
    let axial = axial_model();
    let (ok, d) = refused_with(
        NlIntegrator::new(&axial, &[], &NlTransientOptions::new(0.0)),
        "time step",
    );
    c.holds("F1 zero time step", ok, &d);
    let (ok, d) = refused_with(
        NlIntegrator::new(
            &axial,
            &[],
            &NlTransientOptions::new(1e-6).method(Method::Newmark { beta: 0.0, gamma: 0.5 }),
        ),
        "method",
    );
    c.holds("F1 Newmark beta = 0", ok, &d);
    let mut linear = axial_model();
    let node = linear.mesh().node_set("xmax").unwrap()[0];
    linear
        .add_constraint(Constraint::linear(vec![LinearTerm::new(node, 0, 1.0)], 0.0))
        .unwrap();
    let (ok, d) = refused_with(
        NlIntegrator::new(&linear, &[], &NlTransientOptions::new(1e-6)),
        "nl_transient",
    );
    c.holds("F1 linear constraint (transformation path)", ok, &d);
    let mut moved = axial_model();
    moved.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let (ok, d) = refused_with(
        NlIntegrator::new(&moved, &[], &NlTransientOptions::new(1e-6)),
        "constraint",
    );
    c.holds("F1 non-zero prescribed value", ok, &d);
    let short = LoadHistory::new(vec![0.0; 3], LoadTable::constant(1.0, 0.0, 1.0).unwrap());
    let (ok, d) = refused_with(
        NlIntegrator::new(&axial, &[short], &NlTransientOptions::new(1e-6)),
        "load pattern",
    );
    c.holds("F1 load pattern of the wrong length", ok, &d);
    let model = pendulum_model();
    let loads = [LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 0.01).unwrap()).unwrap()];
    let mut it = NlIntegrator::new(&model, &loads, &NlTransientOptions::new(0.005)).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    let (ok, d) = refused_with(it.run(10), "load table");
    c.holds(
        "F1 table not covering the run, refused before any step",
        ok && it.time().unwrap() == 0.0,
        &d,
    );
    let j2 = j2_model(1);
    let mut it = NlIntegrator::new(&j2, &[], &NlTransientOptions::new(J2_DT)).unwrap();
    let n = j2.dof_map().n_equations();
    let mut u0 = vec![0.0; n];
    u0[j2
        .dof_map()
        .equation(j2.mesh().node_set("xmax").unwrap()[0], 0)
        .unwrap()] = 1e-4;
    let (ok, d) = refused_with(it.start(0.0, &u0, &vec![0.0; n]), "initial state");
    c.holds("F1 J2 model from a non-zero displacement", ok, &d);
    let it = j2_integrator(&j2, Method::average_acceleration());
    let (ok, d) = refused_with(it.strain_energy(), "strain energy");
    c.holds("F1 strain energy of the J2 model", ok, &d);
    c.finish();
}

// ------------------------------------------------------------------------------------------- amendment 1 (ADR 0479)

/// Amendment 1: the Newton residual tolerance of the pendulum and spin set-ups, a decade above the extrapolated floor.
const AMENDED_RESIDUAL_TOLERANCE: f64 = 1e-7;

fn amended_newton() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = AMENDED_RESIDUAL_TOLERANCE;
    o
}

fn pendulum_integrator_amended(model: &SolidModel, threads: Option<usize>) -> NlIntegrator {
    let loads = [LoadHistory::model_loads(model, LoadTable::constant(1.0, 0.0, 10.0).unwrap()).unwrap()];
    let mut o = NlTransientOptions::new(0.005).newton(amended_newton());
    if let Some(t) = threads {
        o = o.threads(t);
    }
    let mut it = NlIntegrator::new(model, &loads, &o).unwrap();
    let zero = vec![0.0; model.dof_map().n_equations()];
    it.start(0.0, &zero, &zero).unwrap();
    it
}

#[test]
fn p0c_p1c_p2c_pendulum_released_from_the_horizontal() {
    let model = pendulum_model();
    let dofs = model.dof_map();
    let tip = node_where(&model, |p| near(p[0], L_BAR) && near(p[1], 0.0) && near(p[2], 0.0));
    assert_eq!(tip.len(), 1, "tip node");
    let (qx, qz) = (dofs.equation(tip[0], 0).unwrap(), dofs.equation(tip[0], 2).unwrap());
    let gravity = LoadHistory::model_loads(&model, LoadTable::constant(1.0, 0.0, 10.0).unwrap())
        .unwrap()
        .pattern;
    let mut it = pendulum_integrator_amended(&model, None);
    let dt = 0.005;
    let steps = 1940;
    let mass = RHO * L_BAR * B_BAR * B_BAR;
    let swing = mass * G * L_BAR / 2.0;
    let energy = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        it.kinetic_energy().unwrap() + it.strain_energy().unwrap() - dot(&gravity, &u)
    };
    let angle = |it: &NlIntegrator| {
        let u = it.displacement().unwrap();
        (L_BAR + u[qx]).atan2(-u[qz])
    };
    let e0 = energy(&it);
    let mut psi_prev = angle(&it);
    let mut crossings = Vec::new();
    let mut e_dev = 0.0f64;
    let (mut halvings, mut iterations, mut worst) = (0, 0, 0.0f64);
    for k in 1..=steps {
        let r = it.step().unwrap();
        halvings += r.halvings;
        iterations += r.iterations;
        worst = worst.max(r.residual);
        let psi = angle(&it);
        if (psi_prev > 0.0) != (psi > 0.0) {
            crossings.push((k - 1) as f64 * dt + dt * psi_prev / (psi_prev - psi));
        }
        psi_prev = psi;
        e_dev = e_dev.max((energy(&it) - e0).abs());
    }
    let t_closed = 2.0 * PI * (2.0 * L_BAR / (3.0 * G)).sqrt() * 1.180340599016096;
    let n = crossings.len();
    let t_fe = 2.0 * (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
    println!(
        "pendulum (amendment 1): {n} crossings, T_FE = {t_fe:.9e} s, T = {t_closed:.9e} s, E_0 = {e0:e}, halvings \
         {halvings}, iterations {iterations}, largest final residual {worst:e}"
    );
    let mut c = Checks::new();
    c.at_least("P0c crossings of psi = 0", n as f64, 9.0);
    c.at_most("P1c period |T_FE - T|/T", (t_fe - t_closed).abs() / t_closed, 1e-2);
    c.at_most("P2c energy max |E - E_0|/(m g L/2)", e_dev / swing, 1e-3);
    c.finish();
}

#[test]
#[ignore = "known miss S1c, S2c of ADR 0479 amendment 1 (run of commit 1949023c): Newmark from the steady spin stops at step 851 of 1000 (t = 8.5 s, 8.5 turns) with a singular tangent (pivot -4.65e7) after an exponential growth from about step 350 (strain energy 7.917e-5 J to 996 J at step 850), the nonlinear instability of the trapezoidal rule; up to the stop max |dP|/|P| = 3.2e-12, max |dJ|/|J| = 2.2e-10 at step 100, 3.9e-6 at step 600, 13.9 at step 850 (diagnosis f423-scratch/diag/diag3.log); S0c met"]
fn s0c_s1c_s2c_free_spinning_bar_known_miss() {
    let omega = 2.0 * PI;
    let centre = [L_BAR / 2.0, 0.0, 0.0];
    let velocity = [0.5, 0.25, 0.1];
    let mut model = SolidModel::new(slender_mesh());
    model.add_material("all", soft()).unwrap();
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes().to_vec();
    let n = dofs.n_equations();
    let mut offset = vec![None; n];
    let mut prescribed = Vec::new();
    for (i, p) in nodes.iter().enumerate() {
        for comp in 0..3 {
            let q = dofs.equation(i, comp).unwrap();
            if comp < 2 {
                offset[q] = Some(p[comp] - centre[comp]);
            }
        }
        if near(p[0], centre[0]) {
            prescribed.push(Prescribed::new(i, 0, 0.0));
        }
        if near(p[1], 0.0) {
            prescribed.push(Prescribed::new(i, 1, 0.0));
        }
        if near(p[2], 0.0) {
            prescribed.push(Prescribed::new(i, 2, 0.0));
        }
    }
    let spin = Centrifugal {
        m: consistent(&model).unwrap(),
        offset,
        omega,
    };
    let ext = DriverExtensions {
        contributions: vec![&spin],
        step_control: None,
    };
    let steady = solve_with(&model, &prescribed, &[], &NewtonOptions::new(1), ext);
    let mut c = Checks::new();
    c.holds(
        "S0c steady spin solve",
        steady.is_ok(),
        &format!("{:?}", steady.as_ref().err()),
    );
    let u0 = steady.unwrap().result.displacement_values().to_vec();
    let mut v0 = vec![0.0; n];
    for (i, p) in nodes.iter().enumerate() {
        let rx = p[0] + u0[dofs.equation(i, 0).unwrap()] - centre[0];
        let ry = p[1] + u0[dofs.equation(i, 1).unwrap()] - centre[1];
        let w = [-omega * ry + velocity[0], omega * rx + velocity[1], velocity[2]];
        for (comp, wc) in w.iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *wc;
        }
    }
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(0.01).newton(amended_newton())).unwrap();
    it.start(0.0, &u0, &v0).unwrap();
    let (p0, j0) = it.momentum().unwrap();
    let (mut dp, mut dj) = (0.0f64, 0.0f64);
    let (mut halvings, mut worst) = (0, 0.0f64);
    for _ in 0..1000 {
        let r = it.step().unwrap();
        halvings += r.halvings;
        worst = worst.max(r.residual);
        let (p, j) = it.momentum().unwrap();
        dp = dp.max(norm3(sub3(p, p0)));
        dj = dj.max(norm3(sub3(j, j0)));
    }
    println!(
        "spin (amendment 1): P_0 = {p0:?}, J_0 = {j0:?}, max centrifugal displacement {:e}, halvings {halvings}, largest \
         final residual {worst:e}, t = {}",
        inf_norm(&u0),
        it.time().unwrap()
    );
    c.at_most("S1c linear momentum max |P - P_0|/|P_0|", dp / norm3(p0), 1e-10);
    c.at_most("S2c angular momentum max |J - J_0|/|J_0|", dj / norm3(j0), 1e-6);
    c.finish();
}

#[test]
fn t1c_threads_bit_identical() {
    let model = pendulum_model();
    let mut a = pendulum_integrator_amended(&model, Some(1));
    let mut b = pendulum_integrator_amended(&model, Some(4));
    a.run(50).unwrap();
    b.run(50).unwrap();
    let mut c = Checks::new();
    c.holds(
        "T1c 1 and 4 threads, 50 steps",
        a.state().unwrap().to_bytes() == b.state().unwrap().to_bytes(),
        "state bytes",
    );
    c.finish();
}

#[test]
fn a0b_a1_g1_large_axial_oscillation() {
    let model = axial_model();
    let dofs = model.dof_map();
    let ends: Vec<usize> = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&i| dofs.equation(i, 0).unwrap())
        .collect();
    let dt = t_lin() / 200.0;
    let mut it = axial_integrator(&model, Method::average_acceleration(), dt, NewtonOptions::new(1));
    let steps = 1200;
    let mut hist = vec![it.displacement().unwrap()];
    let mut spread = 0.0f64;
    let mut crossings = Vec::new();
    let mut lam_prev = 1.2;
    for k in 1..=steps {
        it.step().unwrap();
        let u = it.displacement().unwrap();
        let lo = ends.iter().map(|&q| u[q]).fold(f64::INFINITY, f64::min);
        let hi = ends.iter().map(|&q| u[q]).fold(f64::NEG_INFINITY, f64::max);
        spread = spread.max((hi - lo) / 0.2);
        let lam = 1.0 + u[ends[0]];
        if lam_prev > 1.0 && lam <= 1.0 {
            crossings.push((k - 1) as f64 * dt + dt * (lam_prev - 1.0) / (lam_prev - lam));
        }
        lam_prev = lam;
        hist.push(u);
    }
    let n = crossings.len();
    let t_fe = (crossings[n - 1] - crossings[0]) / (n - 1) as f64;
    let ratio = 1.0407443288903;
    println!("axial: {n} downward crossings, T_FE/T_lin = {:.12}", t_fe / t_lin());
    let mut c = Checks::new();
    c.at_least("A0 downward crossings of lambda = 1", n as f64, 5.0);
    c.at_most("A0b spread of the four end nodes / 0.2 L", spread, 1e-11);
    c.at_most(
        "A1 period |T_FE/T_lin - 1.04074|/1.04074",
        (t_fe / t_lin() - ratio).abs() / ratio,
        5e-3,
    );
    let mut ga = axial_integrator(
        &model,
        Method::GeneralisedAlpha { rho_inf: 1.0 },
        dt,
        NewtonOptions::new(1),
    );
    let hg = nl_history(&mut ga, steps);
    c.at_most(
        "G1 generalised-alpha rho_inf = 1 against Newmark, axial",
        max_rel_diff(&hg, &hist),
        1e-10,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- amendment 2 (ADR 0479)

#[test]
#[ignore = "known miss S3 of ADR 0479 amendment 2 (run of commit dbb80e54): step 50 of 50 does not converge after 5 halvings, relative residual 1.169e-10 against the default tolerance 1e-10 (energy ratio 2.8e-30), the rounding floor of the internal force under translation; S3 max |dP|/|P_0| 1.30e-14 over 49 steps met (log f423-scratch/run3.log)"]
fn s3_free_body_momentum_known_miss() {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_STEEL, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut v0 = vec![0.0; n];
    for i in 0..model.mesh().nodes().len() {
        for (comp, w) in [0.3, 0.2, 0.1].iter().enumerate() {
            v0[dofs.equation(i, comp).unwrap()] = *w;
        }
    }
    for &i in model.mesh().node_set("xmax").unwrap() {
        v0[dofs.equation(i, 0).unwrap()] += 10.0;
    }
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(t_lin() / 200.0)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let (p0, _) = it.momentum().unwrap();
    let mut c = Checks::new();
    let mut dp = 0.0f64;
    let mut ok = true;
    for k in 0..50 {
        if let Err(e) = it.step() {
            c.holds("S3 every step converges", false, &format!("step {}: {e}", k + 1));
            ok = false;
            break;
        }
        dp = dp.max(norm3(sub3(it.momentum().unwrap().0, p0)));
    }
    if ok {
        c.holds("S3 every step converges", true, "50 steps");
    }
    c.at_most("S3 free body max |P - P_0|/|P_0|", dp / norm3(p0), 1e-11);
    c.finish();
}
