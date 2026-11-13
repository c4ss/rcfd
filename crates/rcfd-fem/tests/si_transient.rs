//! F-398 (ADR 0389): linear transient dynamics, Newmark and generalised-alpha, Rayleigh damping, load tables. Checks A1 to
//! A12c, A8c, A8d, C1 to C3 and the negative controls N1 to N4 of the registration. Bar B and two-unknown model Q of ADR 0389
//! D5.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::transient::{
    Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions, TransientState,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const ZETA: f64 = 0.02;

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!("{name}: {value:.6e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!("{name}: {value:.6e} (limit >= {limit:e}) {}", if ok { "met" } else { "MISSED" });
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

// ------------------------------------------------------------------------------------------------------------ Bar B

fn bar_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

struct Bar {
    model: SolidModel,
    k: CsrMatrix,
    m: CsrMatrix,
    phi: Vec<f64>,
    omega: f64,
    period: f64,
    tip: usize,
}

fn bar() -> Bar {
    let model = bar_model();
    let modal = modal_analysis(&model, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let k = stiffness(&model).unwrap();
    let m = consistent(&model).unwrap();
    let omega = (dot(&phi, &mv(&k, &phi)) / dot(&phi, &mv(&m, &phi))).sqrt();
    let node = model.mesh().node_set("xmax").unwrap()[0];
    let tip = model.dof_map().equation(node, 0).unwrap();
    Bar {
        period: 2.0 * PI / omega,
        model,
        k,
        m,
        phi,
        omega,
        tip,
    }
}

/// One recorded run: modal coordinates, energies and the full states of every step (step 0 included).
struct Run {
    q: Vec<f64>,
    p: Vec<f64>,
    energy: Vec<f64>,
    u: Vec<Vec<f64>>,
    states: Vec<TransientState>,
}

fn record(b: &Bar, it: &Integrator, run: &mut Run) {
    let u = it.displacement().unwrap();
    let v = it.velocity().unwrap();
    let mp = mv(&b.m, &b.phi);
    run.q.push(dot(&mp, &u));
    run.p.push(dot(&mp, &v));
    run.energy
        .push(it.kinetic_energy().unwrap() + it.strain_energy().unwrap());
    run.u.push(u);
    run.states.push(it.state().unwrap());
}

fn run_steps(b: &Bar, it: &mut Integrator, steps: u64) -> Run {
    let mut run = Run {
        q: Vec::new(),
        p: Vec::new(),
        energy: Vec::new(),
        u: Vec::new(),
        states: Vec::new(),
    };
    record(b, it, &mut run);
    for _ in 0..steps {
        it.step().unwrap();
        record(b, it, &mut run);
    }
    run
}

/// Free vibration from `u0 = phi_1`, `v0 = 0` at `Omega = omega_1 dt`.
fn free_run(b: &Bar, method: Method, omega_dt: f64, periods: f64, damping: Option<Rayleigh>) -> (Run, f64) {
    let dt = omega_dt / b.omega;
    let mut o = TransientOptions::new(dt).method(method);
    if let Some(d) = damping {
        o = o.damping(d);
    }
    let mut it = Integrator::new(&b.model, &[], &o).unwrap();
    it.start(0.0, &b.phi, &vec![0.0; b.phi.len()]).unwrap();
    let steps = (periods * 2.0 * PI / omega_dt).ceil() as u64;
    (run_steps(b, &mut it, steps), dt)
}

/// The relative period error of `q_n` from its zero crossings (ADR 0389 A3).
fn period_error(q: &[f64], dt: f64, period: f64) -> f64 {
    let mut times = Vec::new();
    for n in 0..q.len() - 1 {
        let (a, c) = (q[n], q[n + 1]);
        if a != 0.0 && (a > 0.0) != (c > 0.0) {
            times.push(n as f64 * dt + dt * a / (a - c));
        }
    }
    let tm = 2.0 * (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64;
    tm / period - 1.0
}

fn amplitude(q: f64, p: f64, omega: f64) -> f64 {
    (q * q + (p / omega) * (p / omega)).sqrt()
}

fn pattern_m_phi(b: &Bar) -> Vec<f64> {
    mv(&b.m, &b.phi)
}

fn damping_of(b: &Bar) -> Rayleigh {
    Rayleigh::from_ratio(ZETA, b.omega, b.omega).unwrap()
}

// ---------------------------------------------------------------------------------------------- A1 to A4, N1, N3, A9

#[test]
fn a1_to_a4_newmark_free_vibration() {
    let b = bar();
    println!("bar: omega_1 = {:.9e} rad/s, T_1 = {:.9e} s", b.omega, b.period);
    let mut ch = Checks::new();
    let mut errs = Vec::new();
    for omega_dt in [0.2, 0.1] {
        let (r, dt) = free_run(&b, Method::average_acceleration(), omega_dt, 100.0, None);
        let n = r.q.len() - 1;
        let a0 = amplitude(r.q[0], r.p[0], b.omega);
        let an = amplitude(r.q[n], r.p[n], b.omega);
        ch.at_most(&format!("A1 Omega {omega_dt}: |A_N/A_0 - 1| ({n} steps)"), (an / a0 - 1.0).abs(), 1e-9);
        let e0 = r.energy[0];
        let de = r.energy.iter().fold(0.0f64, |m, e| m.max((e / e0 - 1.0).abs()));
        ch.at_most(&format!("A2 Omega {omega_dt}: max |E_n/E_0 - 1|"), de, 1e-10);
        let e = period_error(&r.q, dt, b.period);
        let amount = omega_dt * omega_dt / 12.0;
        println!("A3 Omega {omega_dt}: period error {e:.9e}, Omega^2/12 = {amount:.9e}, ratio {:.6}", e / amount);
        ch.at_most(&format!("A3 Omega {omega_dt}: |e/(Omega^2/12) - 1|"), (e / amount - 1.0).abs(), 0.05);
        errs.push(e);
    }
    ch.at_least("A4 observed order", (errs[0] / errs[1]).log2(), 1.9);
    ch.finish();
}

#[test]
fn n1_dissipative_newmark_fails_the_energy_check() {
    let b = bar();
    let (r, _) = free_run(&b, Method::Newmark { beta: 0.3025, gamma: 0.6 }, 0.2, 100.0, None);
    let e0 = r.energy[0];
    let de = r.energy.iter().fold(0.0f64, |m, e| m.max((e / e0 - 1.0).abs()));
    let mut ch = Checks::new();
    ch.holds("N1 energy measure > 1e-10", de > 1e-10, &format!("{de:.6e}"));
    ch.finish();
}

#[test]
fn a9_n3_generalised_alpha_period_order() {
    let b = bar();
    let mut ch = Checks::new();
    for rho in [0.5, 0.8] {
        let mut errs = Vec::new();
        for omega_dt in [0.2, 0.1] {
            let (r, dt) = free_run(&b, Method::GeneralisedAlpha { rho_inf: rho }, omega_dt, 100.0, None);
            let e = period_error(&r.q, dt, b.period);
            let ratio = e / (omega_dt * omega_dt / 12.0);
            println!("A9 rho {rho} Omega {omega_dt}: period error {e:.9e}, ratio to Omega^2/12 {ratio:.6}");
            if rho == 0.5 && omega_dt == 0.1 {
                ch.holds("N3 |ratio - 1| > 0.05", (ratio - 1.0).abs() > 0.05, &format!("{:.6e}", (ratio - 1.0).abs()));
            }
            errs.push(e);
        }
        ch.at_least(&format!("A9 rho {rho}: observed order"), (errs[0] / errs[1]).log2(), 1.9);
    }
    ch.finish();
}

// --------------------------------------------------------------------------------------------------------- A5, A6, C1

#[test]
fn a5_step_load_peak() {
    let b = bar();
    let dt = b.period / 40.0;
    let pattern = pattern_m_phi(&b);
    let table = LoadTable::new(vec![(0.0, 1.0), (10.0 * b.period, 1.0)]).unwrap();
    let mut it = Integrator::new(&b.model, &[LoadHistory::new(pattern.clone(), table)], &TransientOptions::new(dt)).unwrap();
    let zero = vec![0.0; pattern.len()];
    it.start(0.0, &zero, &zero).unwrap();
    let r = run_steps(&b, &mut it, 60);
    let peak = r.u.iter().fold(f64::MIN, |m, u| m.max(u[b.tip]));
    let dofs = b.model.dof_map();
    let forces: Vec<NodalForceAt> = (0..dofs.n_nodes())
        .map(|a| {
            let f = [0, 1, 2].map(|c| pattern[dofs.equation(a, c).unwrap()]);
            NodalForceAt::new(a, f)
        })
        .collect();
    let s = solve_static_with(&b.model, &[], &forces, &StaticOptions::default()).unwrap();
    let us = s.displacement_values()[b.tip];
    println!("A5: peak {peak:.9e}, static {us:.9e}, phi_tip/lambda {:.9e}", b.phi[b.tip] / (b.omega * b.omega));
    let mut ch = Checks::new();
    ch.at_most("A5 |peak/(2 u_s) - 1|", (peak / (2.0 * us) - 1.0).abs(), 0.005);
    ch.finish();
}

fn parabola_peak(q: &[f64], n: usize) -> f64 {
    let (a, c, d) = (q[n - 1], q[n], q[n + 1]);
    c - (d - a) * (d - a) / (8.0 * (d - 2.0 * c + a))
}

#[test]
fn a6_c1_rayleigh_decrement() {
    let b = bar();
    let damping = damping_of(&b);
    println!("A6: alpha_R = {:.9e}, beta_R = {:.9e}", damping.mass, damping.stiffness);
    let mut ch = Checks::new();
    // C1: the initial acceleration.
    let dt = 0.1 / b.omega;
    let mut it = Integrator::new(&b.model, &[], &TransientOptions::new(dt).damping(damping)).unwrap();
    it.start(0.0, &b.phi, &vec![0.0; b.phi.len()]).unwrap();
    let res = it.equilibrium_residual().unwrap();
    let ku = mv(&b.k, &b.phi);
    let ku_free: Vec<f64> = it.free().iter().map(|&e| ku[e]).collect();
    ch.at_most("C1 |M a0 + C v0 + K u0 - f0| / |K u0|", inf_norm(&res) / inf_norm(&ku_free), 1e-12);
    let (r, _) = free_run(&b, Method::average_acceleration(), 0.1, 12.0, Some(damping));
    let mut peaks = Vec::new();
    for n in 1..r.q.len() - 1 {
        if r.q[n] > r.q[n - 1] && r.q[n] >= r.q[n + 1] {
            peaks.push(parabola_peak(&r.q, n));
        }
    }
    let delta = (peaks[0] / peaks[10]).ln() / 10.0;
    let exact = 2.0 * PI * ZETA / (1.0 - ZETA * ZETA).sqrt();
    println!("A6: {} peaks, delta {delta:.9e}, exact {exact:.9e}, relative {:.6e}", peaks.len(), delta / exact - 1.0);
    ch.at_most("A6 |delta/exact - 1|", (delta / exact - 1.0).abs(), 0.01);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------- A7, N2

fn max_rel_diff(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    let scale = b.iter().fold(0.0f64, |m, u| m.max(inf_norm(u)));
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| {
        m.max(x.iter().zip(y).fold(0.0f64, |mm, (p, q)| mm.max((p - q).abs())))
    });
    d / scale
}

fn loaded_damped_run(b: &Bar, method: Method, steps: u64) -> Run {
    let dt = b.period / 40.0;
    let table = LoadTable::new(vec![(0.0, 1.0), (10.0 * b.period, 1.0)]).unwrap();
    let o = TransientOptions::new(dt).method(method).damping(damping_of(b));
    let mut it = Integrator::new(&b.model, &[LoadHistory::new(pattern_m_phi(b), table)], &o).unwrap();
    let zero = vec![0.0; b.phi.len()];
    it.start(0.0, &zero, &zero).unwrap();
    run_steps(b, &mut it, steps)
}

#[test]
fn a7_n2_generalised_alpha_one_is_newmark() {
    let b = bar();
    let mut ch = Checks::new();
    let (nm, _) = free_run(&b, Method::average_acceleration(), 0.2, 10.0, None);
    let (ga, _) = free_run(&b, Method::GeneralisedAlpha { rho_inf: 1.0 }, 0.2, 10.0, None);
    ch.at_most("A7a free vibration, rho 1 vs Newmark", max_rel_diff(&ga.u, &nm.u), 1e-12);
    let (ga8, _) = free_run(&b, Method::GeneralisedAlpha { rho_inf: 0.8 }, 0.2, 10.0, None);
    let d8 = max_rel_diff(&ga8.u, &nm.u);
    ch.holds("N2 rho 0.8 vs Newmark > 1e-12", d8 > 1e-12, &format!("{d8:.6e}"));
    let nm = loaded_damped_run(&b, Method::average_acceleration(), 200);
    let ga = loaded_damped_run(&b, Method::GeneralisedAlpha { rho_inf: 1.0 }, 200);
    ch.at_most("A7b step load and damping, rho 1 vs Newmark", max_rel_diff(&ga.u, &nm.u), 1e-12);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------- A8, A8c, A8d (model Q)

const THETA: f64 = 0.3;
const Q_DT: f64 = 0.1;

fn model_q(omega_2: f64) -> (CsrMatrix, CsrMatrix, [f64; 2]) {
    let (s, c) = THETA.sin_cos();
    let (l1, l2) = (1.0, omega_2 * omega_2);
    let k = CsrMatrix::from_triplets(
        2,
        vec![
            (0, 0, c * c * l1 + s * s * l2),
            (0, 1, c * s * (l1 - l2)),
            (1, 0, c * s * (l1 - l2)),
            (1, 1, s * s * l1 + c * c * l2),
        ],
    );
    let m = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, 1.0)]);
    (k, m, [-s, c])
}

type C = (f64, f64);
fn cmul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}
fn cdiv(a: C, b: C) -> C {
    let d = b.0 * b.0 + b.1 * b.1;
    ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
}
fn csub(a: C, b: C) -> C {
    (a.0 - b.0, a.1 - b.1)
}

/// The spectral radius of a 3 x 3 matrix: the roots of its characteristic cubic by Durand-Kerner (200 sweeps).
fn spectral_radius(a: &[[f64; 3]; 3]) -> f64 {
    let tr = a[0][0] + a[1][1] + a[2][2];
    let minors = a[0][0] * a[1][1] - a[0][1] * a[1][0] + a[0][0] * a[2][2] - a[0][2] * a[2][0] + a[1][1] * a[2][2]
        - a[1][2] * a[2][1];
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    let coeff = [-tr, minors, -det];
    let p = |z: C| {
        let z2 = cmul(z, z);
        let z3 = cmul(z2, z);
        (
            z3.0 + coeff[0] * z2.0 + coeff[1] * z.0 + coeff[2],
            z3.1 + coeff[0] * z2.1 + coeff[1] * z.1,
        )
    };
    let mut r: [C; 3] = [(0.4, 0.9), cmul((0.4, 0.9), (0.4, 0.9)), cmul(cmul((0.4, 0.9), (0.4, 0.9)), (0.4, 0.9))];
    for _ in 0..200 {
        for i in 0..3 {
            let mut den = (1.0, 0.0);
            for j in 0..3 {
                if j != i {
                    den = cmul(den, csub(r[i], r[j]));
                }
            }
            r[i] = csub(r[i], cdiv(p(r[i]), den));
        }
    }
    r.iter().fold(0.0f64, |m, z| m.max((z.0 * z.0 + z.1 * z.1).sqrt()))
}

/// The one-step amplification of mode 2 of Q, measured with the integrator (ADR 0389 D5).
fn amplification(rho: f64, omega_2: f64) -> f64 {
    let (k, m, phi) = model_q(omega_2);
    let o = TransientOptions::new(Q_DT).method(Method::GeneralisedAlpha { rho_inf: rho });
    let mut it = Integrator::from_matrices(k, m, &[], &o).unwrap();
    it.start(0.0, &[0.0, 0.0], &[0.0, 0.0]).unwrap();
    let template = it.state().unwrap();
    let scale = [1.0, omega_2, omega_2 * omega_2];
    let mut a = [[0.0; 3]; 3];
    for j in 0..3 {
        let mut s = template.clone();
        let mut e = [0.0; 3];
        e[j] = 1.0;
        s.u = phi.iter().map(|x| x * e[0] * scale[0]).collect();
        s.v = phi.iter().map(|x| x * e[1] * scale[1]).collect();
        s.a = phi.iter().map(|x| x * e[2] * scale[2]).collect();
        it.restore(s).unwrap();
        it.step().unwrap();
        let t = it.state().unwrap();
        a[0][j] = dot(&phi, &t.u) / scale[0];
        a[1][j] = dot(&phi, &t.v) / scale[1];
        a[2][j] = dot(&phi, &t.a) / scale[2];
    }
    spectral_radius(&a)
}

#[test]
fn a8_decay_per_step_at_omega_dt_1e3() {
    let mut ch = Checks::new();
    for rho in [0.5, 0.8] {
        let r = amplification(rho, 1e3 / Q_DT);
        println!("A8 rho {rho}: measured factor {r:.10}");
        ch.at_most(&format!("A8 rho {rho}: |r - rho_inf| at Omega 1e3"), (r - rho).abs(), 1e-3);
    }
    ch.finish();
}

#[test]
fn a8c_a8d_decay_against_the_closed_form_and_at_omega_dt_1e6() {
    let mut ch = Checks::new();
    for (rho, closed) in [(0.5, 0.507_847_168_3), (0.8, 0.807_721_245_8)] {
        let r = amplification(rho, 1e3 / Q_DT);
        ch.at_most(&format!("A8c rho {rho}: |r - r_closed| at Omega 1e3 (r {r:.10})"), (r - closed).abs(), 1e-6);
        let r6 = amplification(rho, 1e6 / Q_DT);
        ch.at_most(&format!("A8d rho {rho}: |r - rho_inf| at Omega 1e6 (r {r6:.10})"), (r6 - rho).abs(), 1e-3);
    }
    ch.finish();
}

// -------------------------------------------------------------------------------------------------- A10, A11, N4

fn ramp_integrator(b: &Bar, threads: usize, table_end: f64) -> Integrator {
    let dt = b.period / 40.0;
    let table = LoadTable::new(vec![(0.0, 0.0), (b.period, 1.0), (table_end, 1.0)]).unwrap();
    let o = TransientOptions::new(dt)
        .method(Method::GeneralisedAlpha { rho_inf: 0.8 })
        .damping(damping_of(b))
        .threads(threads);
    Integrator::new(&b.model, &[LoadHistory::new(pattern_m_phi(b), table)], &o).unwrap()
}

fn same_bits(a: &TransientState, b: &TransientState) -> bool {
    let eq = |x: &[f64], y: &[f64]| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits());
    a.step == b.step && eq(&a.u, &b.u) && eq(&a.v, &b.v) && eq(&a.a, &b.a)
}

#[test]
fn a10_a11_n4_save_restore_and_threads() {
    let b = bar();
    let mut ch = Checks::new();
    let zero = vec![0.0; b.phi.len()];
    let mut whole = ramp_integrator(&b, 1, 10.0 * b.period);
    whole.start(0.0, &zero, &zero).unwrap();
    let unsplit = run_steps(&b, &mut whole, 200);
    // A10: split after step 77.
    let mut first = ramp_integrator(&b, 1, 10.0 * b.period);
    first.start(0.0, &zero, &zero).unwrap();
    first.run(77).unwrap();
    let bytes = first.state().unwrap().to_bytes();
    let mut second = ramp_integrator(&b, 1, 10.0 * b.period);
    second.restore(TransientState::from_bytes(&bytes).unwrap()).unwrap();
    let tail = run_steps(&b, &mut second, 123);
    let equal = tail.states.iter().zip(&unsplit.states[77..]).all(|(x, y)| same_bits(x, y));
    ch.holds("A10 steps 77 to 200 of the split run equal the unsplit run", equal && tail.states.len() == 124, "bit-equal");
    // N4: the restored step set to 0.
    let mut shifted = TransientState::from_bytes(&bytes).unwrap();
    shifted.step = 0;
    let mut third = ramp_integrator(&b, 1, 10.0 * b.period);
    third.restore(shifted).unwrap();
    third.run(123).unwrap();
    let n4 = !same_bits_ignoring_step(&third.state().unwrap(), &unsplit.states[200]);
    ch.holds("N4 restored with step 0: not bit-equal", n4, "differs");
    // A11: threads 1 and 8.
    let mut eight = ramp_integrator(&b, 8, 10.0 * b.period);
    eight.start(0.0, &zero, &zero).unwrap();
    let r8 = run_steps(&b, &mut eight, 200);
    let equal8 = r8.states.iter().zip(&unsplit.states).all(|(x, y)| same_bits(x, y)) && r8.states.len() == 201;
    ch.holds("A11 threads 1 and 8, steps 0 to 200", equal8, "bit-equal");
    ch.finish();
}

fn same_bits_ignoring_step(a: &TransientState, b: &TransientState) -> bool {
    let eq = |x: &[f64], y: &[f64]| x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits());
    eq(&a.u, &b.u) && eq(&a.v, &b.v) && eq(&a.a, &b.a)
}

// -------------------------------------------------------------------------------------------------------- A12, C2, C3

#[test]
fn a12_refusals() {
    let b = bar();
    let mut ch = Checks::new();
    for dt in [0.0, -1e-4, f64::NAN, f64::INFINITY] {
        let (ok, msg) = refused_with(Integrator::new(&b.model, &[], &TransientOptions::new(dt)), "time step");
        ch.holds(&format!("A12a time step {dt:e}"), ok, &msg);
    }
    let zero = vec![0.0; b.phi.len()];
    let mut it = ramp_integrator(&b, 1, 2.0 * b.period);
    it.start(0.0, &zero, &zero).unwrap();
    let before = it.state().unwrap();
    let (ok, msg) = refused_with(it.run(200), "load table");
    ch.holds("A12b run(200) past the table refused", ok, &msg);
    ch.holds("A12b state unchanged", same_bits(&it.state().unwrap(), &before), "bit-equal");
    ch.holds("A12b run(40) accepted", it.run(40).is_ok(), "accepted");
    let late = LoadTable::new(vec![(1e-3 * b.period, 1.0), (10.0 * b.period, 1.0)]).unwrap();
    let mut it = Integrator::new(
        &b.model,
        &[LoadHistory::new(pattern_m_phi(&b), late)],
        &TransientOptions::new(b.period / 40.0),
    )
    .unwrap();
    let (ok, msg) = refused_with(it.start(0.0, &zero, &zero), "load table");
    ch.holds("A12b table starting after t0 refused by start", ok, &msg);
    let k = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, -4.0)]);
    let m = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, 1.0)]);
    let (ok, msg) = refused_with(Integrator::from_matrices(k, m, &[], &TransientOptions::new(1.0)), "effective matrix");
    ch.holds("A12c singular effective matrix", ok, &msg);
    ch.finish();
}

#[test]
fn c2_c3_tables_and_refusals() {
    let b = bar();
    let mut ch = Checks::new();
    let t = LoadTable::new(vec![(0.0, 0.3), (0.7, 1.9), (1.1, -0.4)]).unwrap();
    let at_points = t.points().iter().all(|&(ti, vi)| t.value(ti).unwrap().to_bits() == vi.to_bits());
    ch.holds("C2 values at the points", at_points, "bit-equal");
    let mid = (t.value(0.35).unwrap() - 1.1).abs().max((t.value(0.9).unwrap() - 0.75).abs());
    ch.at_most("C2 midpoint |value - mean|", mid, 1e-15);
    ch.holds("C2 outside the table", t.value(-1e-9).is_none() && t.value(1.1 + 1e-9).is_none(), "None");
    for (name, pts) in [
        ("one point", vec![(0.0, 1.0)]),
        ("non-increasing times", vec![(0.0, 1.0), (0.0, 2.0)]),
        ("NaN value", vec![(0.0, 1.0), (1.0, f64::NAN)]),
    ] {
        let (ok, msg) = refused_with(LoadTable::new(pts), "load table");
        ch.holds(&format!("C3 table {name}"), ok, &msg);
    }
    let dt = b.period / 40.0;
    let table = LoadTable::constant(1.0, 0.0, 1.0).unwrap();
    let (ok, msg) = refused_with(
        Integrator::new(&b.model, &[LoadHistory::new(vec![1.0; 3], table)], &TransientOptions::new(dt)),
        "load pattern",
    );
    ch.holds("C3 pattern of the wrong length", ok, &msg);
    for (name, method) in [
        ("rho 1.5", Method::GeneralisedAlpha { rho_inf: 1.5 }),
        ("rho -0.1", Method::GeneralisedAlpha { rho_inf: -0.1 }),
        ("Newmark beta -1", Method::Newmark { beta: -1.0, gamma: 0.5 }),
    ] {
        let (ok, msg) = refused_with(Integrator::new(&b.model, &[], &TransientOptions::new(dt).method(method)), "method");
        ch.holds(&format!("C3 method {name}"), ok, &msg);
    }
    let bad = Rayleigh { mass: -1.0, stiffness: 0.0 };
    let (ok, msg) = refused_with(Integrator::new(&b.model, &[], &TransientOptions::new(dt).damping(bad)), "damping");
    ch.holds("C3 Rayleigh -1", ok, &msg);
    let mut moved = bar_model();
    moved.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let (ok, msg) = refused_with(Integrator::new(&moved, &[], &TransientOptions::new(dt)), "constraint");
    ch.holds("C3 non-zero prescribed value", ok, &msg);
    let mut sprung = bar_model();
    sprung.add_support(ElasticSupport::nodal_spring("xmax", 0, 1e6)).unwrap();
    let (ok, msg) = refused_with(Integrator::new(&sprung, &[], &TransientOptions::new(dt)), "constraint");
    ch.holds("C3 nodal spring", ok, &msg);
    let mut it = Integrator::new(&b.model, &[], &TransientOptions::new(dt)).unwrap();
    let (ok, msg) = refused_with(it.step(), "state");
    ch.holds("C3 step before start", ok, &msg);
    let mut u0 = vec![0.0; b.phi.len()];
    let fixed_eq = b.model.dof_map().equation(0, 1).unwrap();
    u0[fixed_eq] = 1e-6;
    let (ok, msg) = refused_with(it.start(0.0, &u0, &vec![0.0; u0.len()]), "initial state");
    ch.holds("C3 initial displacement at a prescribed equation", ok, &msg);
    it.start(0.0, &b.phi, &vec![0.0; u0.len()]).unwrap();
    let bytes = it.state().unwrap().to_bytes();
    let mut wrong = bytes.clone();
    wrong[0] = b'X';
    let (ok, msg) = refused_with(TransientState::from_bytes(&wrong), "state");
    ch.holds("C3 bad magic", ok, &msg);
    let (ok, msg) = refused_with(TransientState::from_bytes(&bytes[..bytes.len() - 1]), "state");
    ch.holds("C3 truncated buffer", ok, &msg);
    let state = TransientState::from_bytes(&bytes).unwrap();
    let mut other_dt = Integrator::new(&b.model, &[], &TransientOptions::new(2.0 * dt)).unwrap();
    let (ok, msg) = refused_with(other_dt.restore(state.clone()), "state");
    ch.holds("C3 restore with another dt", ok, &msg);
    let mut other_method =
        Integrator::new(&b.model, &[], &TransientOptions::new(dt).method(Method::GeneralisedAlpha { rho_inf: 0.8 }))
            .unwrap();
    let (ok, msg) = refused_with(other_method.restore(state), "state");
    ch.holds("C3 restore with another method", ok, &msg);
    ch.finish();
}
