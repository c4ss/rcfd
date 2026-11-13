//! F-490 (ADR 0393): the transient integrator on the transformation path (elastic supports, rigid sets) and with prescribed
//! motion histories. Checks B1 to B4, R1, R2, X1 to X7, D1, Q1, Q2 and the negative controls N1 to N3 of the registration.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::transient::{
    Integrator, LoadHistory, LoadTable, Method, MotionHistory, Rayleigh, TransientOptions, TransientState,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

/// The digest of the three ADR 0389 runs of D1, recorded from the unchanged library (ADR 0393 amendment 1).
const REF_DIGEST: u64 = 0xabb6_faeb_ad69_662b;

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// FNV-1a 64.
struct Digest(u64);

impl Digest {
    fn new() -> Digest {
        Digest(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= x as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn floats(&mut self, v: &[f64]) {
        for x in v {
            self.bytes(&x.to_bits().to_le_bytes());
        }
    }
}

// ------------------------------------------------------------------------------------------------- bar B (ADR 0389)

fn bar_b() -> SolidModel {
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
    m: CsrMatrix,
    phi: Vec<f64>,
    omega: f64,
    period: f64,
}

fn bar() -> Bar {
    let model = bar_b();
    let modal = modal_analysis(&model, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let k = stiffness(&model).unwrap();
    let m = consistent(&model).unwrap();
    let omega = (dot(&phi, &mv(&k, &phi)) / dot(&phi, &mv(&m, &phi))).sqrt();
    Bar {
        period: 2.0 * PI / omega,
        model,
        m,
        phi,
        omega,
    }
}

fn damping_of(omega: f64) -> Rayleigh {
    Rayleigh::from_ratio(0.02, omega, omega).unwrap()
}

/// ADR 0389's A10 set-up: from rest, `M phi_1` with the ramp, `rho_inf = 0.8`, Rayleigh 0.02, `dt = T_1/40`.
fn a10_options(b: &Bar, threads: usize) -> (Vec<LoadHistory>, TransientOptions) {
    let table = LoadTable::new(vec![(0.0, 0.0), (b.period, 1.0), (10.0 * b.period, 1.0)]).unwrap();
    let o = TransientOptions::new(b.period / 40.0)
        .method(Method::GeneralisedAlpha { rho_inf: 0.8 })
        .damping(damping_of(b.omega))
        .threads(threads);
    (vec![LoadHistory::new(mv(&b.m, &b.phi), table)], o)
}

fn digest_run(d: &mut Digest, it: &mut Integrator, steps: u64) {
    d.bytes(&it.state().unwrap().to_bytes());
    for _ in 0..steps {
        it.step().unwrap();
        d.bytes(&it.state().unwrap().to_bytes());
    }
    d.floats(&it.displacement().unwrap());
    d.floats(&it.velocity().unwrap());
    d.floats(&it.acceleration().unwrap());
    d.floats(&[it.kinetic_energy().unwrap(), it.strain_energy().unwrap()]);
}

fn model_q(omega_2: f64) -> (CsrMatrix, CsrMatrix) {
    let (s, c) = 0.3f64.sin_cos();
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
    (k, m)
}

/// The digest of D1 through the API of ADR 0389 only.
fn adr_0389_digest(b: &Bar) -> u64 {
    let mut d = Digest::new();
    let zero = vec![0.0; b.phi.len()];
    // (i) free vibration, Newmark, Omega = 0.2.
    let mut it = Integrator::new(&b.model, &[], &TransientOptions::new(0.2 / b.omega)).unwrap();
    it.start(0.0, &b.phi, &zero).unwrap();
    digest_run(&mut d, &mut it, 200);
    // (ii) the A10 set-up.
    let (loads, o) = a10_options(b, 1);
    let mut it = Integrator::new(&b.model, &loads, &o).unwrap();
    it.start(0.0, &zero, &zero).unwrap();
    digest_run(&mut d, &mut it, 200);
    // (iii) model Q.
    let (k, m) = model_q(1e4);
    let o = TransientOptions::new(0.1).method(Method::GeneralisedAlpha { rho_inf: 0.5 });
    let mut it = Integrator::from_matrices(k, m, &[], &o).unwrap();
    it.start(0.0, &[1.0, 0.5], &[0.0, 0.0]).unwrap();
    digest_run(&mut d, &mut it, 50);
    d.0
}

#[test]
fn d1_models_without_the_features_keep_the_bits_of_adr_0389() {
    let b = bar();
    let digest = adr_0389_digest(&b);
    println!("D1 digest {digest:#018x} (reference {REF_DIGEST:#018x})");
    assert_eq!(digest, REF_DIGEST, "D1: the digest of the ADR 0389 runs changed");
}

// ------------------------------------------------------------------------------------------------------------ checks

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

fn inf_norm(x: &[f64]) -> f64 {
    x.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// `max_n |a_n - b_n|_inf / max_n |b_n|_inf` over the first `len` entries of each vector.
fn max_rel_diff(a: &[Vec<f64>], b: &[Vec<f64>], len: usize) -> f64 {
    let scale = b.iter().fold(0.0f64, |m, u| m.max(inf_norm(&u[..len])));
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| {
        m.max(
            x[..len]
                .iter()
                .zip(&y[..len])
                .fold(0.0f64, |mm, (p, q)| mm.max((p - q).abs())),
        )
    });
    d / scale
}

fn same_bits(a: &TransientState, b: &TransientState) -> bool {
    let eq = |x: &[f64], y: &[f64]| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits());
    a.step == b.step && eq(&a.u, &b.u) && eq(&a.v, &b.v) && eq(&a.a, &b.a)
}

/// The displacements and states of a run, step 0 included.
fn history(it: &mut Integrator, steps: u64) -> (Vec<Vec<f64>>, Vec<TransientState>) {
    let mut u = vec![it.displacement().unwrap()];
    let mut s = vec![it.state().unwrap()];
    for _ in 0..steps {
        it.step().unwrap();
        u.push(it.displacement().unwrap());
        s.push(it.state().unwrap());
    }
    (u, s)
}

// ------------------------------------------------------------------------------------------------ KB: B1 to B4, N3

const K_SPRING: f64 = 5e8;

/// KB of ADR 0384: the bar on an end spring, rollers on ymin and zmin.
fn kb() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, K_SPRING / 4.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
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

#[test]
fn b1_to_b4_n3_spring_bar_free_vibration() {
    let model = kb();
    let modal = modal_analysis(&model, &ModalOptions::new(4)).unwrap();
    let phi = modal.modes[0].clone();
    let omega = modal.eigenvalues[0].sqrt();
    let period = 2.0 * PI / omega;
    let m = consistent(&model).unwrap();
    let mp = mv(&m, &phi);
    println!("KB: omega_1 = {omega:.9e} rad/s, f_1 = {:.6} Hz", modal.frequencies[0]);
    let mut ch = Checks::new();
    let mut errors = Vec::new();
    for big_omega in [0.2, 0.1] {
        let dt = big_omega / omega;
        let mut it = Integrator::new(&model, &[], &TransientOptions::new(dt)).unwrap();
        ch.holds(
            "KB is on the transformation path",
            it.transformation().is_some(),
            "transformation",
        );
        it.start(0.0, &phi, &vec![0.0; phi.len()]).unwrap();
        let steps = (100.0 * 2.0 * PI / big_omega).ceil() as u64;
        let (mut q, mut p, mut e) = (Vec::new(), Vec::new(), Vec::new());
        let mut rec = |it: &Integrator| {
            q.push(dot(&mp, &it.displacement().unwrap()));
            p.push(dot(&mp, &it.velocity().unwrap()));
            e.push(it.kinetic_energy().unwrap() + it.strain_energy().unwrap());
        };
        rec(&it);
        for _ in 0..steps {
            it.step().unwrap();
            rec(&it);
        }
        let amp = |i: usize| (q[i] * q[i] + (p[i] / omega) * (p[i] / omega)).sqrt();
        ch.at_most(
            &format!("B1 Omega {big_omega}: |A_N/A_0 - 1| ({steps} steps)"),
            (amp(q.len() - 1) / amp(0) - 1.0).abs(),
            1e-9,
        );
        let e_dev = e.iter().fold(0.0f64, |mm, x| mm.max((x / e[0] - 1.0).abs()));
        ch.at_most(&format!("B3 Omega {big_omega}: max |E_n/E_0 - 1|"), e_dev, 1e-10);
        let err = period_error(&q, dt, period);
        let target = big_omega * big_omega / 12.0;
        ch.at_most(
            &format!("B2 Omega {big_omega}: |e/(Omega^2/12) - 1| (e = {err:.6e})"),
            (err / target - 1.0).abs(),
            0.05,
        );
        let n3 = (err / (big_omega * big_omega / 6.0) - 1.0).abs();
        ch.holds(
            &format!("N3 Omega {big_omega}: against Omega^2/6 {n3:.4e} > 0.05"),
            n3 > 0.05,
            "discriminates",
        );
        errors.push(err);
    }
    ch.at_least("B4 order log2(e(0.2)/e(0.1))", (errors[0] / errors[1]).log2(), 1.9);
    ch.finish();
}

// ------------------------------------------------------------------------------------------ B and RB: R1, R2, N2

/// RB of ADR 0384: bar B with a rigid set on its free end.
fn rb() -> SolidModel {
    let mut m = bar_b();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.05, 0.05]))
        .unwrap();
    m
}

/// A mesh vector of RB padded with the reference unknowns: `t_x` = the tip value, all others 0.
fn padded(it: &Integrator, x: &[f64], tip: f64) -> Vec<f64> {
    let tr = it.transformation().unwrap();
    let mut full = x.to_vec();
    full.resize(tr.n_unknowns(), 0.0);
    full[tr.references()[0].1] = tip;
    full
}

fn tip_eq(model: &SolidModel) -> usize {
    let node = model.mesh().node_set("xmax").unwrap()[0];
    model.dof_map().equation(node, 0).unwrap()
}

/// The section-warping start of N2: `u_x = phi_1 sigma(y, z)` off the tip, 0 at the tip.
fn warped(model: &SolidModel, phi: &[f64]) -> Vec<f64> {
    let mut u = vec![0.0; phi.len()];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if x[0] < 1.0 - 1e-9 {
            let eq = model.dof_map().equation(node, 0).unwrap();
            let odd = ((x[1] > 0.05) as u8 + (x[2] > 0.05) as u8) % 2 == 1;
            u[eq] = if odd { -phi[eq] } else { phi[eq] };
        }
    }
    u
}

#[test]
fn r1_r2_n2_rigid_set_end_against_prescribed_only() {
    let b = bar();
    let rbm = rb();
    let tip = tip_eq(&b.model);
    let n = b.phi.len();
    let mut ch = Checks::new();
    let dt = 0.2 / b.omega;
    let steps = (10.0 * 2.0 * PI / 0.2).ceil() as u64;
    let free = |model: &SolidModel, u0: &[f64], pad: Option<f64>| {
        let mut it = Integrator::new(model, &[], &TransientOptions::new(dt)).unwrap();
        let u = match pad {
            None => u0.to_vec(),
            Some(t) => padded(&it, u0, t),
        };
        let v = vec![0.0; u.len()];
        it.start(0.0, &u, &v).unwrap();
        history(&mut it, steps).0
    };
    let ub = free(&b.model, &b.phi, None);
    let ur = free(&rbm, &b.phi, Some(b.phi[tip]));
    ch.at_most(
        &format!("R1 RB vs B, free vibration, Omega 0.2, {steps} steps"),
        max_rel_diff(&ur, &ub, n),
        1e-12,
    );
    // R2: the A10 set-up.
    let (loads, o) = a10_options(&b, 1);
    let mut itb = Integrator::new(&b.model, &loads, &o).unwrap();
    itb.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    let hb = history(&mut itb, 200).0;
    let mut itr = Integrator::new(&rbm, &loads, &o).unwrap();
    let nn = itr.n_unknowns();
    itr.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    let hr = history(&mut itr, 200).0;
    ch.at_most(
        "R2 RB vs B, ramp load M phi_1, rho 0.8, Rayleigh 0.02, 200 steps",
        max_rel_diff(&hr, &hb, n),
        1e-12,
    );
    // N2: a warping start.
    let w = warped(&b.model, &b.phi);
    let wb = free(&b.model, &w, None);
    let wr = free(&rbm, &w, Some(0.0));
    let d = max_rel_diff(&wr, &wb, n);
    ch.holds(&format!("N2 warping start: {d:.4e} > 1e-6"), d > 1e-6, "differs");
    ch.finish();
}

// ------------------------------------------------------------------------------------- X: the base-excited oscillator

const Y_AMP: f64 = 1e-3;

struct Osc {
    model: SolidModel,
    omega: f64,
    moved: Vec<usize>,
    tip: usize,
    base: usize,
}

fn osc() -> Osc {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let dofs = model.dof_map();
    let moved: Vec<usize> = model.mesh().node_set("xmin").unwrap().to_vec();
    let tip = dofs.equation(model.mesh().node_set("xmax").unwrap()[0], 0).unwrap();
    let base = dofs.equation(moved[0], 0).unwrap();
    Osc {
        omega: (3.0 * E_MOD / RHO).sqrt(),
        moved,
        tip,
        base,
        model,
    }
}

/// The table `1 - cos(Omega_b t)` at `t_j = j dt`, `j = -1 ..= steps + 2`.
fn base_table(omega_b: f64, dt: f64, steps: u64) -> LoadTable {
    let pts = (-1..=(steps as i64 + 2))
        .map(|j| {
            let t = j as f64 * dt;
            (t, 1.0 - (omega_b * t).cos())
        })
        .collect();
    LoadTable::new(pts).unwrap()
}

fn base_motion(x: &Osc, table: LoadTable) -> MotionHistory {
    MotionHistory::new(x.moved.iter().map(|&nd| Prescribed::new(nd, 0, Y_AMP)).collect(), table)
}

/// `z'' + 2 zeta omega z' + omega^2 z = -(3/2) Y (Om^2 cos(Om t) + alpha_R Om sin(Om t))`, `z(0) = z'(0) = 0`.
fn closed(t: f64, omega: f64, om: f64, zeta: f64, alpha_r: f64) -> f64 {
    let fc = -1.5 * Y_AMP * om * om;
    let fs = -1.5 * Y_AMP * alpha_r * om;
    let c2 = 2.0 * zeta * omega;
    let w2 = omega * omega - om * om;
    let d = w2 * w2 + (c2 * om) * (c2 * om);
    let p = (w2 * fc - c2 * om * fs) / d;
    let q = (w2 * fs + c2 * om * fc) / d;
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let s = zeta * omega;
    let c1 = -p;
    let c2h = (-q * om + s * c1) / wd;
    p * (om * t).cos() + q * (om * t).sin() + (-s * t).exp() * (c1 * (wd * t).cos() + c2h * (wd * t).sin())
}

struct OscRun {
    z: Vec<f64>,
    u: Vec<Vec<f64>>,
    dt: f64,
}

fn osc_integrator(x: &Osc, spp: u64, steps: u64, zeta: f64, method: Method, threads: usize) -> (Integrator, LoadTable) {
    let period = 2.0 * PI / x.omega;
    let dt = period / spp as f64;
    let table = base_table(0.5 * x.omega, dt, steps);
    let mut o = TransientOptions::new(dt).method(method).threads(threads);
    if zeta > 0.0 {
        o = o.damping(Rayleigh::from_ratio(zeta, x.omega, x.omega).unwrap());
    }
    let it = Integrator::with_motion(&x.model, &[], &[base_motion(x, table.clone())], &o).unwrap();
    (it, table)
}

fn osc_run(x: &Osc, spp: u64, steps: u64, zeta: f64, method: Method) -> OscRun {
    let (mut it, _) = osc_integrator(x, spp, steps, zeta, method, 1);
    let nn = it.n_unknowns();
    it.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    let dt = it.time_step();
    let (u, _) = history(&mut it, steps);
    let z = u.iter().map(|v| v[x.tip] - v[x.base]).collect();
    OscRun { z, u, dt }
}

/// `e_X` over the steps of the run (step 0 excluded) against `factor` times the closed form.
fn e_x(x: &Osc, r: &OscRun, zeta: f64, factor: f64) -> f64 {
    let om = 0.5 * x.omega;
    let alpha_r = zeta * x.omega;
    let (mut err, mut zmax) = (0.0f64, 0.0f64);
    for n in 1..r.z.len() {
        let zc = factor * closed(n as f64 * r.dt, x.omega, om, zeta, alpha_r);
        err = err.max((r.z[n] - zc).abs());
        zmax = zmax.max(zc.abs());
    }
    err / zmax
}

#[test]
fn x1_to_x5_n1_base_excited_oscillator() {
    let x = osc();
    let mut ch = Checks::new();
    println!("X: omega = {:.7e} rad/s, T = {:.8e} s", x.omega, 2.0 * PI / x.omega);
    let nm = Method::average_acceleration();
    let x1 = osc_run(&x, 40, 40, 0.0, nm);
    let e1 = e_x(&x, &x1, 0.0, 1.0);
    ch.at_most("X1 undamped, 40 steps per period, one period: e_X", e1, 0.01);
    let x2 = osc_run(&x, 40, 40, 0.05, nm);
    let e2 = e_x(&x, &x2, 0.05, 1.0);
    ch.at_most("X2 zeta 0.05, 40 steps per period, one period: e_X", e2, 0.01);
    let e1f = e_x(&x, &osc_run(&x, 80, 80, 0.0, nm), 0.0, 1.0);
    let e2f = e_x(&x, &osc_run(&x, 80, 80, 0.05, nm), 0.05, 1.0);
    println!("X3 at 80 steps per period: undamped {e1f:.6e}, damped {e2f:.6e}");
    ch.at_least("X3 undamped order log2(e(40)/e(80))", (e1 / e1f).log2(), 1.9);
    ch.at_least("X3 damped order log2(e(40)/e(80))", (e2 / e2f).log2(), 1.9);
    let ga = osc_run(&x, 40, 80, 0.05, Method::GeneralisedAlpha { rho_inf: 1.0 });
    let nm80 = osc_run(&x, 40, 80, 0.05, nm);
    let nn = ga.u[0].len();
    ch.at_most(
        "X4 damped, rho 1 vs Newmark, 80 steps",
        max_rel_diff(&ga.u, &nm80.u, nn),
        1e-12,
    );
    let table = base_table(0.5 * x.omega, x1.dt, 40);
    let dofs = x.model.dof_map();
    let bits = x1.u.iter().enumerate().all(|(n, u)| {
        let expected = Y_AMP * table.value(0.0 + n as f64 * x1.dt).unwrap();
        x.moved
            .iter()
            .all(|&nd| u[dofs.equation(nd, 0).unwrap()].to_bits() == expected.to_bits())
    });
    ch.holds("X5 displacement at the moved equations = Y s(t_n)", bits, "bit-equal");
    let n1 = e_x(&x, &x1, 0.0, 1.0 / 1.5);
    ch.holds(
        &format!("N1 participation 1 instead of 3/2: {n1:.4e} > 0.1"),
        n1 > 0.1,
        "discriminates",
    );
    ch.finish();
}

#[test]
fn x6_x7_start_residual_save_restore_threads() {
    let x = osc();
    let mut ch = Checks::new();
    // X6.
    let (mut it, table) = osc_integrator(&x, 40, 40, 0.05, Method::average_acceleration(), 1);
    let nn = it.n_unknowns();
    it.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    let r = it.equilibrium_residual().unwrap();
    let dt = it.time_step();
    let s2 = (table.value(dt).unwrap() - 2.0 * table.value(0.0).unwrap() + table.value(-dt).unwrap()) / (dt * dt);
    let tr = it.transformation().unwrap();
    let m = consistent(&x.model).unwrap();
    let mut g = vec![0.0; tr.n_mesh()];
    for &nd in &x.moved {
        g[x.model.dof_map().equation(nd, 0).unwrap()] = Y_AMP;
    }
    let mut mg = mv(&m, &g);
    mg.resize(tr.n_unknowns(), 0.0);
    let f0 = s2.abs() * inf_norm(&tr.transpose_times(&mg));
    ch.at_most("X6 start residual / |f_eff(t0)|", inf_norm(&r) / f0, 1e-12);
    // X7.
    let ga = Method::GeneralisedAlpha { rho_inf: 0.8 };
    let (mut whole, _) = osc_integrator(&x, 40, 80, 0.05, ga, 1);
    whole.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    let (_, unsplit) = history(&mut whole, 80);
    let (mut first, _) = osc_integrator(&x, 40, 80, 0.05, ga, 1);
    first.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    first.run(29).unwrap();
    let bytes = first.state().unwrap().to_bytes();
    let (mut second, _) = osc_integrator(&x, 40, 80, 0.05, ga, 1);
    second.restore(TransientState::from_bytes(&bytes).unwrap()).unwrap();
    let (_, tail) = history(&mut second, 51);
    let equal = tail.len() == 52 && tail.iter().zip(&unsplit[29..]).all(|(a, b)| same_bits(a, b));
    ch.holds("X7 split after 29 of 80 steps", equal, "bit-equal");
    let (mut eight, _) = osc_integrator(&x, 40, 80, 0.05, ga, 8);
    eight.start(0.0, &vec![0.0; nn], &vec![0.0; nn]).unwrap();
    let (_, s8) = history(&mut eight, 80);
    let equal8 = s8.len() == unsplit.len() && s8.iter().zip(&unsplit).all(|(a, b)| same_bits(a, b));
    ch.holds("X7 threads 1 and 8", equal8, "bit-equal");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------- Q1, Q2

#[test]
fn q1_q2_refusals_and_the_dispatch() {
    let x = osc();
    let mut ch = Checks::new();
    let period = 2.0 * PI / x.omega;
    let dt = period / 40.0;
    let o = TransientOptions::new(dt);
    let table = base_table(0.5 * x.omega, dt, 40);
    let nd = x.moved[0];
    let on = |p: Vec<Prescribed>| MotionHistory::new(p, table.clone());
    let (ok, msg) = refused_with(
        Integrator::with_motion(&x.model, &[], &[on(vec![Prescribed::new(nd, 1, Y_AMP)])], &o),
        "motion history",
    );
    ch.holds("Q1 history on an equation the model prescribes", ok, &msg);
    let one = on(vec![Prescribed::new(nd, 0, Y_AMP)]);
    let (ok, msg) = refused_with(
        Integrator::with_motion(&x.model, &[], &[one.clone(), one], &o),
        "motion history",
    );
    ch.holds("Q1 one equation in two histories", ok, &msg);
    let (ok, msg) = refused_with(
        Integrator::with_motion(&x.model, &[], &[on(vec![Prescribed::new(nd, 0, f64::NAN)])], &o),
        "motion history",
    );
    ch.holds("Q1 NaN amplitude", ok, &msg);
    let late = LoadTable::new(
        (0..=42)
            .map(|j| {
                let t = j as f64 * dt;
                (t, 1.0 - (0.5 * x.omega * t).cos())
            })
            .collect(),
    )
    .unwrap();
    let mut it = Integrator::with_motion(&x.model, &[], &[base_motion(&x, late)], &o).unwrap();
    let nn = it.n_unknowns();
    let zero = vec![0.0; nn];
    let (ok, msg) = refused_with(it.start(0.0, &zero, &zero), "motion table");
    ch.holds("Q1 motion table starting at t0 refused by start", ok, &msg);
    let mut it = Integrator::with_motion(&x.model, &[], &[base_motion(&x, table.clone())], &o).unwrap();
    it.start(0.0, &zero, &zero).unwrap();
    let before = it.state().unwrap();
    let (ok, msg) = refused_with(it.run(42), "motion table");
    ch.holds("Q1 run past the motion table refused", ok, &msg);
    ch.holds(
        "Q1 state unchanged",
        same_bits(&it.state().unwrap(), &before),
        "bit-equal",
    );
    ch.holds("Q1 run(40) accepted", it.run(40).is_ok(), "accepted");
    let mut moved = kb();
    moved.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let (ok, msg) = refused_with(Integrator::new(&moved, &[], &o), "constraint");
    ch.holds("Q1 KB with a non-zero prescribed value", ok, &msg);
    let mut lin = kb();
    let xmax = lin.mesh().node_set("xmax").unwrap().to_vec();
    lin.add_constraint(Constraint::linear(
        vec![LinearTerm::new(xmax[0], 0, 1.0), LinearTerm::new(xmax[1], 0, -1.0)],
        1e-3,
    ))
    .unwrap();
    let (ok, msg) = refused_with(Integrator::new(&lin, &[], &o), "constraint");
    ch.holds("Q1 KB with a non-zero Linear value", ok, &msg);
    let b = bar();
    let rbm = rb();
    let mut it = Integrator::new(&rbm, &[], &o).unwrap();
    let tip = tip_eq(&b.model);
    let mut u0 = padded(&it, &b.phi, b.phi[tip]);
    u0[tip] += 1e-3 * inf_norm(&b.phi);
    let z = vec![0.0; u0.len()];
    let (ok, msg) = refused_with(it.start(0.0, &u0, &z), "initial state");
    ch.holds("Q1 RB with an inadmissible tip", ok, &msg);
    let pattern = LoadHistory::new(vec![1.0; 3], LoadTable::constant(1.0, 0.0, 1.0).unwrap());
    let (ok, msg) = refused_with(Integrator::new(&kb(), &[pattern], &o), "load pattern");
    ch.holds("Q1 KB with a load pattern of length 3", ok, &msg);
    // Q2.
    let (loads, o) = a10_options(&b, 1);
    let zero = vec![0.0; b.phi.len()];
    let mut a = Integrator::with_motion(&b.model, &loads, &[], &o).unwrap();
    a.start(0.0, &zero, &zero).unwrap();
    let mut c = Integrator::new(&b.model, &loads, &o).unwrap();
    c.start(0.0, &zero, &zero).unwrap();
    let (_, sa) = history(&mut a, 200);
    let (_, sc) = history(&mut c, 200);
    let equal =
        a.transformation().is_none() && sa.len() == sc.len() && sa.iter().zip(&sc).all(|(p, q)| same_bits(p, q));
    ch.holds(
        "Q2 with_motion without motions on B is ADR 0389's integrator",
        equal,
        "bit-equal",
    );
    ch.finish();
}
