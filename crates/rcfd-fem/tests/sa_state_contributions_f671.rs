//! F-671 (ADR 0596): the tangents of the driver's contributions (a follower pressure) on the buckling and direct harmonic
//! paths about a nonlinear state. Ring (quarter ring under external follower pressure, conservative): S0r, BT0r, BFD (the
//! factors against the fourth-order central difference of the residual with the follower force), B0c, P1c, NBc, NB2c and
//! the record BFVr; S0r, HT0r, HFD (the response against the central difference of the residual along it), H0c, NHc, NH2c.
//! No contribution: E0 (bits against the paths of ADR 0590) and the E1 hash. Bar F (free-edged follower end load): records
//! S0b, RNq, RNv, RNa, RNh. Every measured value is printed; a non-finite value or a refused call is a miss (records
//! excepted).

use rcfd_fem::buckling::{BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::element::ElementState;
use rcfd_fem::follower::{
    buckling_factors, tangent_buckling_factors, tangent_buckling_factors_with, tangent_modes, FollowerPressure,
};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_with, Contribution, DriverExtensions, NewtonOptions,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; the test fails at the end if any missed.
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
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not >= {limit:e}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (record)");
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

// ----------------------------------------------------------------------------------------------- set-up (ADR 0596 D4)

const R: f64 = 1.0;
const T: f64 = 0.1;
const BW: f64 = 0.1;
const E: f64 = 2.0e11;
const RHO_STEEL: f64 = 7850.0;
/// The external follower pressure on the ring, Pa.
const P_R: f64 = 2.5e6;
/// The radial harmonic force on each node of `ymin` of the ring, N.
const F_RING: f64 = 1e3;
const RING_RAYLEIGH: (f64, f64) = (1.0, 1e-5);
/// The step of the fourth-order central difference, m (ADR 0590 D3).
const A_STEP: f64 = 1e-4;
const FACTORS: usize = 2;

/// ADR 0480's quarter ring with `T = BW = 0.1`: 1 x 24 x 1 Hex20, the symmetry planes and the plane held.
fn ring_model() -> SolidModel {
    let b = hex_box([R - T / 2.0, 0.0, 0.0], [T, PI / 2.0, BW], [1, 24, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, RHO_STEEL, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

/// The pre-stress of the dead pressure `p` on `set` (the linear static solve of a copy with `Load::pressure`).
fn dead_prestress(model: &SolidModel, set: &str, p: f64) -> (SolidModel, Prestress) {
    let mut dead = model.clone();
    dead.add_load(Load::pressure(set, p)).unwrap();
    let s = solve_static(&dead, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&dead, &s).unwrap();
    (dead, pre)
}

/// A state of the residual `r(w) = f_int(w) - f_p(w, lambda) - f_dead`, and its load-factor derivative `g(w) = -f_p(w, 1)`.
struct State<'a> {
    model: &'a SolidModel,
    u: Vec<f64>,
    states: Vec<ElementState>,
    fp: Option<&'a FollowerPressure>,
    lambda: f64,
    dead: Vec<f64>,
}

impl State<'_> {
    fn residual(&self, w: &[f64]) -> Vec<f64> {
        let mut f = internal_force(self.model, w, &self.states).unwrap();
        if let Some(fp) = self.fp {
            for (a, b) in f.iter_mut().zip(fp.force(w, self.lambda).unwrap()) {
                *a -= b;
            }
        }
        for (a, b) in f.iter_mut().zip(&self.dead) {
            *a -= b;
        }
        f
    }
    fn derivative(&self, w: &[f64]) -> Vec<f64> {
        match self.fp {
            Some(fp) => fp.force(w, 1.0).unwrap().iter().map(|x| -x).collect(),
            None => vec![0.0; w.len()],
        }
    }
    fn d4_of(&self, v: &[f64], f: impl Fn(&[f64]) -> Vec<f64>) -> Vec<f64> {
        let a = A_STEP;
        let at = |t: f64| -> Vec<f64> {
            let w: Vec<f64> = self.u.iter().zip(v).map(|(x, p)| x + t * p).collect();
            f(&w)
        };
        let (p1, m1, p2, m2) = (at(a), at(-a), at(2.0 * a), at(-2.0 * a));
        (0..v.len())
            .map(|k| (8.0 * (p1[k] - m1[k]) - (p2[k] - m2[k])) / (12.0 * a))
            .collect()
    }
    /// D4 of the residual along `v` (ADR 0590 D3, ADR 0596 D3).
    fn d4(&self, v: &[f64]) -> Vec<f64> {
        self.d4_of(v, |w| self.residual(w))
    }
    /// D4 of the load-factor derivative along `v`.
    fn d4g(&self, v: &[f64]) -> Vec<f64> {
        self.d4_of(v, |w| self.derivative(w))
    }
}

fn dense(a: &CsrMatrix) -> Vec<Vec<f64>> {
    let mut d = vec![vec![0.0; a.n]; a.n];
    for (i, row) in d.iter_mut().enumerate() {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            row[a.col[k]] += a.val[k];
        }
    }
    d
}

fn matvec(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    a.iter()
        .map(|row| row.iter().zip(x).map(|(v, xj)| v * xj).sum())
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn unit(v: &[f64]) -> (Vec<f64>, f64) {
    let s = v.iter().fold(0.0f64, |s, x| s.max(x.abs()));
    if s == 0.0 {
        return (v.to_vec(), 0.0);
    }
    (v.iter().map(|x| x / s).collect(), s)
}

/// Worst relative difference of two lists, position by position, relative to `b`; infinite if the lengths differ.
fn worst(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / y.abs())
        .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY })
}

fn free_mask(model: &SolidModel) -> Vec<bool> {
    let mut free = vec![true; model.dof_map().n_equations()];
    for (eq, _) in prescribed_equations(model, &[]).unwrap() {
        free[eq] = false;
    }
    free
}

/// The nonlinear state under the contributions `list` (no dead load in the model).
fn follower_state(
    c: &mut Checks,
    name: &str,
    model: &SolidModel,
    list: &[&dyn Contribution],
    options: &NewtonOptions,
) -> Option<(Vec<f64>, Vec<ElementState>)> {
    let ext = DriverExtensions {
        contributions: list.to_vec(),
        step_control: None,
    };
    match solve_with(model, &[], &[], options, ext) {
        Ok(r) => {
            let u = r.result.displacement_values().to_vec();
            let umax = u.iter().fold(0.0f64, |s, x| s.max(x.abs()));
            c.holds(
                name,
                true,
                &format!("converged, {} steps, max |u| = {umax:.6e}", r.steps.len()),
            );
            Some((u, r.states))
        }
        Err(e) => {
            c.holds(name, false, &e.to_string());
            None
        }
    }
}

fn buckling_ok(c: &mut Checks, name: &str, r: &Result<BucklingResult, String>) -> bool {
    match r {
        Ok(b) => {
            let worst_res = b.residuals.iter().cloned().fold(0.0f64, f64::max);
            let ok = b.factors.len() == FACTORS
                && b.factors.iter().all(|f| f.is_finite() && *f > 0.0)
                && worst_res <= 1e-10
                && b.exhausted.is_none();
            c.holds(
                name,
                ok,
                &format!(
                    "factors {:?}, iterations {}, worst residual {worst_res:.3e}, exhausted {:?}",
                    b.factors, b.iterations, b.exhausted
                ),
            );
            ok
        }
        Err(e) => {
            c.holds(name, false, e);
            false
        }
    }
}

/// `(mu_FD, mu_FD without D4_g, vector measure)` along the modes of `b` (ADR 0596 D3).
fn buckling_measures(st: &State, kg: &[Vec<f64>], free: &[bool], b: &BucklingResult) -> (Vec<f64>, Vec<f64>, f64) {
    let mut mu_fd = Vec::new();
    let mut mu_dead = Vec::new();
    let mut vec_worst = 0.0f64;
    for (x, mu) in b.modes.iter().zip(&b.factors) {
        let (xh, _) = unit(x);
        let dr = st.d4(&xh);
        let dg = st.d4g(&xh);
        let kgx = matvec(kg, &xh);
        let num = dot(&xh, &dr);
        let s = dot(&xh, &kgx);
        mu_fd.push(-num / (s + dot(&xh, &dg)));
        mu_dead.push(-num / s);
        let (mut q, mut m) = (0.0f64, 0.0f64);
        for k in (0..xh.len()).filter(|&k| free[k]) {
            q = q.max((dr[k] + mu * (kgx[k] + dg[k])).abs());
            m = m.max(dr[k].abs());
        }
        vec_worst = vec_worst.max(q / m);
    }
    (mu_fd, mu_dead, vec_worst)
}

// ------------------------------------------------------------------------------------------------------- ring buckling

#[test]
fn ring_buckling_about_the_pressurised_state() {
    let mut c = Checks::new();
    let ring = ring_model();
    let n = ring.dof_map().n_equations();
    let fp = FollowerPressure::new(&ring, "xmax", P_R).unwrap();
    let list: Vec<&dyn Contribution> = vec![&fp];
    let Some((u, states)) = follower_state(&mut c, "S0r: the ring", &ring, &list, &NewtonOptions::new(4)) else {
        c.finish();
        return;
    };
    let (dead, pre) = dead_prestress(&ring, "xmax", P_R);
    let options = BucklingOptions::new(FACTORS);
    let zero = vec![0.0; n];
    let rest = initial_states(&ring).unwrap();
    let e = |r: Result<BucklingResult, FemError>| r.map_err(|e| e.to_string());
    let bs = e(tangent_buckling_factors_with(
        &ring,
        &[],
        &u,
        &states,
        1.0,
        &list,
        &pre,
        &options,
    ));
    let b0 = e(tangent_buckling_factors_with(
        &ring,
        &[],
        &zero,
        &rest,
        0.0,
        &list,
        &pre,
        &options,
    ));
    let bn = e(tangent_buckling_factors(&ring, &[], &u, &states, &pre, &options));
    let bl = e(buckling_factors(&dead, &pre, &[&fp], &options));
    let ok = buckling_ok(&mut c, "BT0r: with the follower at the state", &bs)
        & buckling_ok(&mut c, "BT0r: with the follower at zero load", &b0)
        & buckling_ok(&mut c, "BT0r: without contributions at the state", &bn)
        & buckling_ok(&mut c, "BT0r: follower::buckling_factors", &bl);
    if !ok {
        c.finish();
        return;
    }
    let (bs, b0, bn, bl) = (bs.unwrap(), b0.unwrap(), bn.unwrap(), bl.unwrap());
    let st = State {
        model: &ring,
        u: u.clone(),
        states: states.clone(),
        fp: Some(&fp),
        lambda: 1.0,
        dead: vec![0.0; n],
    };
    let kg = dense(&geometric_stiffness(&ring, &pre, None).unwrap());
    let free = free_mask(&ring);
    let (mu_fd, mu_dead, vec_worst) = buckling_measures(&st, &kg, &free, &bs);
    println!("ring: mu_FD {mu_fd:?}, mu {:?}, linear {:?}", bs.factors, bl.factors);
    c.at_most("BFD", worst(&mu_fd, &bs.factors), 1e-6);
    c.at_most("B0c", worst(&b0.factors, &bl.factors), 1e-6);
    c.at_most(
        "P1c",
        ((1.0 + bs.factors[0]) - bl.factors[0]).abs() / bl.factors[0],
        1e-2,
    );
    c.at_least("NBc", worst(&bn.factors, &bs.factors), 1e-2);
    c.at_least("NB2c", worst(&mu_dead, &bs.factors), 1e-2);
    c.record("BFVr (vector measure, ring)", vec_worst);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- ring harmonic

fn node_load(model: &SolidModel, set: &str, component: usize, value: f64) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &k in model.mesh().node_set(set).unwrap() {
        f[dofs.equation(k, component).unwrap()] += value;
    }
    f
}

/// `e_H` of ADR 0590 D3 on the free equations, the residual of `st`.
fn harmonic_error(
    st: &State,
    free: &[bool],
    mass: &[Vec<f64>],
    f: &[f64],
    x: &HarmonicResponse,
    rayleigh: (f64, f64),
) -> f64 {
    let n = f.len();
    let kx = |v: &[f64]| -> Vec<f64> {
        let (vh, s) = unit(v);
        if s == 0.0 {
            return vec![0.0; n];
        }
        st.d4(&vh).iter().map(|d| d * s).collect()
    };
    let (kre, kim) = (kx(&x.re), kx(&x.im));
    let (mre, mim) = (matvec(mass, &x.re), matvec(mass, &x.im));
    let (alpha, beta) = rayleigh;
    let w = x.omega;
    let mut q_max = 0.0f64;
    let mut k_max = 0.0f64;
    for k in (0..n).filter(|&k| free[k]) {
        let (a, b) = (kre[k] - w * beta * kim[k], kim[k] + w * beta * kre[k]);
        let (cm, dm) = (
            -w * w * mre[k] - w * alpha * mim[k],
            -w * w * mim[k] + w * alpha * mre[k],
        );
        let q = (a + cm - f[k]).hypot(b + dm);
        q_max = q_max.max(q);
        k_max = k_max.max(kre[k].hypot(kim[k]));
    }
    q_max / k_max
}

fn response_difference(a: &HarmonicResponse, b: &HarmonicResponse) -> f64 {
    let mut d = 0.0f64;
    let mut m = 0.0f64;
    for k in 0..a.re.len() {
        d = d.max((a.re[k] - b.re[k]).hypot(a.im[k] - b.im[k]));
        m = m.max(b.re[k].hypot(b.im[k]));
    }
    d / m
}

#[test]
fn ring_harmonic_about_the_pressurised_state() {
    let mut c = Checks::new();
    let ring = ring_model();
    let n = ring.dof_map().n_equations();
    let fp = FollowerPressure::new(&ring, "xmax", P_R).unwrap();
    let list: Vec<&dyn Contribution> = vec![&fp];
    let Some((u, states)) = follower_state(&mut c, "S0r: the ring", &ring, &list, &NewtonOptions::new(4)) else {
        c.finish();
        return;
    };
    let zero = vec![0.0; n];
    let rest = initial_states(&ring).unwrap();
    let ts = tangent_modes(&ring, &[], &u, &states, 1.0, &list, &ModalOptions::new(2));
    let ma = modal_analysis(&ring, &ModalOptions::new(2));
    let (ts, ma) = match (ts, ma) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            c.holds("HT0r: the modes", false, &format!("{:?} {:?}", a.err(), b.err()));
            c.finish();
            return;
        }
    };
    let w1 = 2.0 * PI * ts.frequencies[0];
    let w1r = 2.0 * PI * ma.frequencies[0];
    println!(
        "ring: frequencies at the state {:?} Hz, at rest {:?} Hz",
        ts.frequencies, ma.frequencies
    );
    let f = node_load(&ring, "ymin", 0, F_RING);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let opt_a = DirectHarmonicOptions::new().threads(1);
    let opt_b = DirectHarmonicOptions::new()
        .threads(1)
        .rayleigh(RING_RAYLEIGH.0, RING_RAYLEIGH.1);
    let run = |solver: Result<DirectHarmonic, FemError>, omega: f64| -> Result<HarmonicResponse, String> {
        solver.and_then(|s| s.response(&load, omega)).map_err(|e| e.to_string())
    };
    let hs_a = run(
        DirectHarmonic::about_state_with(&ring, &[], &u, &states, 1.0, &list, &opt_a),
        0.5 * w1,
    );
    let hs_b = run(
        DirectHarmonic::about_state_with(&ring, &[], &u, &states, 1.0, &list, &opt_b),
        1.5 * w1,
    );
    let h0_a = run(
        DirectHarmonic::about_state_with(&ring, &[], &zero, &rest, 0.0, &list, &opt_a),
        0.5 * w1r,
    );
    let h0_b = run(
        DirectHarmonic::about_state_with(&ring, &[], &zero, &rest, 0.0, &list, &opt_b),
        1.5 * w1r,
    );
    let hl_a = run(DirectHarmonic::new(&ring, &opt_a), 0.5 * w1r);
    let hl_b = run(DirectHarmonic::new(&ring, &opt_b), 1.5 * w1r);
    let hn_a = run(DirectHarmonic::about_state(&ring, &[], &u, &states, &opt_a), 0.5 * w1);
    let all = [&hs_a, &hs_b, &h0_a, &h0_b, &hl_a, &hl_b, &hn_a];
    let ok = all.iter().all(|r| r.is_ok());
    c.holds(
        "HT0r",
        ok,
        &format!(
            "{:?}",
            all.iter().map(|r| r.as_ref().err().cloned()).collect::<Vec<_>>()
        ),
    );
    if !ok {
        c.finish();
        return;
    }
    let [hs_a, hs_b, h0_a, h0_b, hl_a, hl_b, hn_a] = [hs_a, hs_b, h0_a, h0_b, hl_a, hl_b, hn_a].map(|r| r.unwrap());
    let mass = dense(&consistent(&ring).unwrap());
    let free = free_mask(&ring);
    let st = State {
        model: &ring,
        u: u.clone(),
        states: states.clone(),
        fp: Some(&fp),
        lambda: 1.0,
        dead: vec![0.0; n],
    };
    c.at_most(
        "HFD omega_a",
        harmonic_error(&st, &free, &mass, &f, &hs_a, (0.0, 0.0)),
        1e-6,
    );
    c.at_most(
        "HFD omega_b",
        harmonic_error(&st, &free, &mass, &f, &hs_b, RING_RAYLEIGH),
        1e-6,
    );
    c.at_most("H0c omega_a", response_difference(&h0_a, &hl_a), 1e-7);
    c.at_most("H0c omega_b", response_difference(&h0_b, &hl_b), 1e-7);
    c.at_least("NHc", response_difference(&hn_a, &hs_a), 1e-2);
    c.at_least("NH2c", harmonic_error(&st, &free, &mass, &f, &hn_a, (0.0, 0.0)), 1e-2);
    c.finish();
}

// ----------------------------------------------------------------------------------------- no contribution (E0, E1)

const RHO_RUBBER: f64 = 1000.0;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}

/// ADR 0590's bar `[0, 2] x [0, 0.5] x [0, 0.5]`, 8 x 2 x 2 Full Hex8, held on `xmin`, `p` in `x` on each node of `xmax`
/// (no load if `p` is `None`).
fn bar(p: Option<f64>) -> SolidModel {
    let mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::hyperelastic(nhc(), RHO_RUBBER).unwrap())
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::nodal_force("xmax", [p, 0.0, 0.0])).unwrap();
    }
    model
}

/// FNV-1a over the bits of a list.
fn fnv(h: &mut u64, v: &[f64]) {
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            *h ^= b as u64;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn no_contribution_equals_the_adr_0590_paths() {
    let mut c = Checks::new();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    // Bar C NH (ADR 0590 D4).
    let model = bar(Some(-250.0));
    let r = solve(&model, &[], &[], &NewtonOptions::new(10)).unwrap();
    let (u, states) = (r.result.displacement_values().to_vec(), r.states);
    let linear = solve_static(&model, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&model, &linear).unwrap();
    let options = BucklingOptions::new(4);
    let old = tangent_buckling_factors(&model, &[], &u, &states, &pre, &options).unwrap();
    let new = tangent_buckling_factors_with(&model, &[], &u, &states, 0.0, &[], &pre, &options).unwrap();
    let same = bits_equal(&old.factors, &new.factors)
        && bits_equal(&old.residuals, &new.residuals)
        && old.modes.len() == new.modes.len()
        && old.modes.iter().zip(&new.modes).all(|(a, b)| bits_equal(a, b))
        && old.iterations == new.iterations;
    c.holds(
        "E0 buckling (Bar C NH)",
        same,
        "factors, residuals, modes, iterations bit-equal",
    );
    fnv(&mut h, &old.factors);
    fnv(&mut h, &old.residuals);
    for m in &old.modes {
        fnv(&mut h, m);
    }
    // Bar S NH (ADR 0590 D4).
    let model = bar(Some(4e3));
    let r = solve(&model, &[], &[], &NewtonOptions::new(10)).unwrap();
    let (u, states) = (r.result.displacement_values().to_vec(), r.states);
    let ts = tangent_modes(&model, &[], &u, &states, 1.0, &[], &ModalOptions::new(6)).unwrap();
    let w1 = 2.0 * PI * ts.frequencies[0];
    let load = HarmonicLoad::real(node_load(&model, "xmax", 2, 100.0)).unwrap();
    let opt_a = DirectHarmonicOptions::new().threads(1);
    let opt_b = DirectHarmonicOptions::new().threads(1).rayleigh(0.5, 1e-3);
    for (name, opt, omega) in [("omega_a", &opt_a, 0.5 * w1), ("omega_b", &opt_b, 1.5 * w1)] {
        let old = DirectHarmonic::about_state(&model, &[], &u, &states, opt)
            .unwrap()
            .response(&load, omega)
            .unwrap();
        let new = DirectHarmonic::about_state_with(&model, &[], &u, &states, 0.0, &[], opt)
            .unwrap()
            .response(&load, omega)
            .unwrap();
        c.holds(
            &format!("E0 harmonic (Bar S NH) {name}"),
            bits_equal(&old.re, &new.re) && bits_equal(&old.im, &new.im),
            "response bit-equal",
        );
        fnv(&mut h, &old.re);
        fnv(&mut h, &old.im);
    }
    println!("E1 hash of the ADR 0590 results: {h:016x}");
    c.finish();
}

// --------------------------------------------------------------------------------------------- Bar F (records)

#[test]
fn free_edged_bar_records() {
    let mut c = Checks::new();
    let model = bar(None);
    let n = model.dof_map().n_equations();
    let fp = FollowerPressure::new(&model, "xmax", 9e3).unwrap();
    let list: Vec<&dyn Contribution> = vec![&fp];
    let mut rc = Checks::new();
    let Some((u, states)) = follower_state(
        &mut rc,
        "S0b (record): Bar F, unsymmetric Newton",
        &model,
        &list,
        &NewtonOptions::new(10).unsymmetric(true),
    ) else {
        c.finish();
        return;
    };
    let (_, pre) = dead_prestress(&model, "xmax", 9e3);
    let free = free_mask(&model);
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fp: Some(&fp),
        lambda: 1.0,
        dead: vec![0.0; n],
    };
    match tangent_buckling_factors_with(
        &model,
        &[],
        &u,
        &states,
        1.0,
        &list,
        &pre,
        &BucklingOptions::new(FACTORS),
    ) {
        Ok(b) => {
            println!("Bar F: factors {:?}, residuals {:?}", b.factors, b.residuals);
            let kg = dense(&geometric_stiffness(&model, &pre, None).unwrap());
            let (mu_fd, _, vec_worst) = buckling_measures(&st, &kg, &free, &b);
            c.record("RNq (Rayleigh measure, Bar F)", worst(&mu_fd, &b.factors));
            c.record("RNv (vector measure, Bar F)", vec_worst);
        }
        Err(e) => println!("RNq, RNv (record): the buckling call failed: {e}"),
    }
    let kp = dense(&CsrMatrix::from_triplets(n, fp.load_stiffness(&u, 1.0).unwrap()));
    let (mut anti, mut big) = (0.0f64, 0.0f64);
    for i in (0..n).filter(|&i| free[i]) {
        for j in (0..n).filter(|&j| free[j]) {
            anti = anti.max((kp[i][j] - kp[j][i]).abs());
            big = big.max(kp[i][j].abs());
        }
    }
    c.record("RNa (reduced antisymmetry of K_p at the state)", anti / big);
    let modes = tangent_modes(&model, &[], &u, &states, 1.0, &list, &ModalOptions::new(2));
    match modes {
        Ok(ts) => {
            let w1 = 2.0 * PI * ts.frequencies[0];
            let f = node_load(&model, "xmax", 2, 100.0);
            let load = HarmonicLoad::real(f.clone()).unwrap();
            let opt = DirectHarmonicOptions::new().threads(1);
            match DirectHarmonic::about_state_with(&model, &[], &u, &states, 1.0, &list, &opt)
                .and_then(|s| s.response(&load, 0.5 * w1))
            {
                Ok(x) => {
                    let mass = dense(&consistent(&model).unwrap());
                    c.record(
                        "RNh (e_H, Bar F, symmetric approximation)",
                        harmonic_error(&st, &free, &mass, &f, &x, (0.0, 0.0)),
                    );
                }
                Err(e) => println!("RNh (record): the harmonic call failed: {e}"),
            }
        }
        Err(e) => println!("RNh (record): tangent_modes failed: {e}"),
    }
    c.finish();
}
