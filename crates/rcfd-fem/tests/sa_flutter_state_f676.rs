//! F-676 (ADR 0600): the complex pencil path about a nonlinear state with the full (unsymmetric) tangents of the
//! contributions. Bar R (Full Hex8 neo-Hookean cantilever, `0.5 x 0.4` section) under a follower end pressure (Beck's load,
//! free-edged), at a compressed state below the Euler load. Buckling: S0, SA, BT, BV (the pairs against the fourth-order
//! central difference of the nonlinear residual with the follower force, vector measure), BE (backward errors), BZ (zero
//! load against `buckling_factors_full`), the negative controls NS, NZ, NG, NR and the refusals R1. Modes: S0, MT, MV, ME,
//! NSM. Every measured value is printed; a non-finite value or a refused call is a miss.

use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::element::ElementState;
use rcfd_fem::follower::{
    buckling_factors_full, tangent_buckling_factors_full_with, tangent_modes, tangent_modes_full, ComplexBuckling,
    FollowerPressure,
};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{initial_states, internal_force, solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::CsrMatrix;

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

// ------------------------------------------------------------------------------------------- set-up (ADR 0600)

/// The follower end pressure, Pa.
const P_F: f64 = 9e3;
/// The step of the fourth-order central difference, m (ADR 0590 D3).
const A_STEP: f64 = 1e-4;
const FACTORS: usize = 6;
const MODES: usize = 4;

/// Bar R: `[0, 2] x [0, 0.5] x [0, 0.4]`, 8 x 2 x 2 Full Hex8, neo-Hookean, held on `xmin` (no load).
fn bar(held: bool) -> SolidModel {
    let mesh = hex_box([0.0; 3], [2.0, 0.5, 0.4], [8, 2, 2]).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material(
            "all",
            Material::hyperelastic(HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap(), 1000.0).unwrap(),
        )
        .unwrap();
    if held {
        for k in 0..3 {
            model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
        }
    }
    model
}

fn dead_prestress(model: &SolidModel) -> Prestress {
    let mut dead = model.clone();
    dead.add_load(Load::pressure("xmax", P_F)).unwrap();
    let s = solve_static(&dead, &StaticOptions::default()).unwrap();
    Prestress::from_static(&dead, &s).unwrap()
}

/// The residual `r(w) = f_int(w) - f_p(w, 1)` and its load-factor derivative `g(w) = -f_p(w, 1)` about `u`.
struct State<'a> {
    model: &'a SolidModel,
    u: Vec<f64>,
    states: Vec<ElementState>,
    fp: &'a FollowerPressure,
}

impl State<'_> {
    fn residual(&self, w: &[f64]) -> Vec<f64> {
        let mut f = internal_force(self.model, w, &self.states).unwrap();
        for (a, b) in f.iter_mut().zip(self.fp.force(w, 1.0).unwrap()) {
            *a -= b;
        }
        f
    }
    fn derivative(&self, w: &[f64]) -> Vec<f64> {
        self.fp.force(w, 1.0).unwrap().iter().map(|x| -x).collect()
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
    /// D4 along a complex direction, by parts.
    fn d4c(&self, x: &[C64], f: impl Fn(&[f64]) -> Vec<f64> + Copy) -> Vec<C64> {
        let re: Vec<f64> = x.iter().map(|z| z.re).collect();
        let im: Vec<f64> = x.iter().map(|z| z.im).collect();
        let dr = self.d4_of(&re, f);
        let di = if im.iter().any(|v| *v != 0.0) {
            self.d4_of(&im, f)
        } else {
            vec![0.0; x.len()]
        };
        dr.iter().zip(&di).map(|(a, b)| C64::new(*a, *b)).collect()
    }
    fn d4r(&self, x: &[C64]) -> Vec<C64> {
        self.d4c(x, |w| self.residual(w))
    }
    fn d4g(&self, x: &[C64]) -> Vec<C64> {
        self.d4c(x, |w| self.derivative(w))
    }
}

fn apply(a: &CsrMatrix, x: &[C64]) -> Vec<C64> {
    (0..a.n)
        .map(|r| {
            let mut s = C64::ZERO;
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                s += x[a.col[k]].scale(a.val[k]);
            }
            s
        })
        .collect()
}

/// `x` divided by its entry of largest modulus.
fn scaled(x: &[C64]) -> Vec<C64> {
    let (mut big, mut at) = (0.0f64, C64::ONE);
    for z in x {
        if z.abs() > big {
            big = z.abs();
            at = *z;
        }
    }
    x.iter().map(|z| *z / at).collect()
}

fn free_mask(model: &SolidModel) -> Vec<bool> {
    let mut free = vec![true; model.dof_map().n_equations()];
    for (eq, _) in prescribed_equations(model, &[]).unwrap() {
        free[eq] = false;
    }
    free
}

/// `max_free |e| / max_free |d|`.
fn ratio(e: &[C64], d: &[C64], free: &[bool]) -> f64 {
    let (mut q, mut m) = (0.0f64, 0.0f64);
    for k in (0..e.len()).filter(|&k| free[k]) {
        q = q.max(e[k].abs());
        m = m.max(d[k].abs());
    }
    q / m
}

/// `V_B` of ADR 0600 arithmetic 2 over the pairs (worst), with or without `D4_g`.
fn buckling_measure(st: &State, kg: &CsrMatrix, free: &[bool], b: &ComplexBuckling, with_g: bool) -> f64 {
    let mut worst = 0.0f64;
    for (x, mu) in b.modes.iter().zip(&b.factors) {
        let x = scaled(x);
        let dr = st.d4r(&x);
        let kx = apply(kg, &x);
        let dg = if with_g { st.d4g(&x) } else { vec![C64::ZERO; x.len()] };
        let e: Vec<C64> = (0..x.len()).map(|k| dr[k] + *mu * (kx[k] + dg[k])).collect();
        let v = ratio(&e, &dr, free);
        worst = if v.is_finite() { worst.max(v) } else { f64::INFINITY };
    }
    worst
}

/// `V_M` over the pairs (worst).
fn modal_measure(st: &State, mass: &CsrMatrix, free: &[bool], pairs: &[(C64, Vec<C64>)]) -> f64 {
    let mut worst = 0.0f64;
    for (w2, x) in pairs {
        let x = scaled(x);
        let dr = st.d4r(&x);
        let mx = apply(mass, &x);
        let e: Vec<C64> = (0..x.len()).map(|k| dr[k] - *w2 * mx[k]).collect();
        let v = ratio(&e, &dr, free);
        worst = if v.is_finite() { worst.max(v) } else { f64::INFINITY };
    }
    worst
}

fn max_of(v: &[f64]) -> f64 {
    v.iter()
        .fold(0.0f64, |m, &x| if x.is_finite() { m.max(x) } else { f64::INFINITY })
}

/// The state under the follower (unsymmetric Newton, ADR 0600 set-up).
fn state(c: &mut Checks, model: &SolidModel, fp: &FollowerPressure) -> Option<(Vec<f64>, Vec<ElementState>)> {
    let ext = DriverExtensions {
        contributions: vec![fp as &dyn Contribution],
        step_control: None,
    };
    match solve_with(model, &[], &[], &NewtonOptions::new(10).unsymmetric(true), ext) {
        Ok(r) => {
            let u = r.result.displacement_values().to_vec();
            let umax = u.iter().fold(0.0f64, |s, x| s.max(x.abs()));
            c.holds(
                "S0",
                true,
                &format!("converged, {} steps, max |u| = {umax:.6e}", r.steps.len()),
            );
            Some((u, r.states))
        }
        Err(e) => {
            c.holds("S0", false, &e.to_string());
            None
        }
    }
}

/// The symmetric part through the full path (NS): `add_full` is the follower's `add`.
struct SymPart<'a>(&'a FollowerPressure);

impl Contribution for SymPart<'_> {
    fn name(&self) -> &str {
        "symmetric part"
    }
    fn add(&self, u: &[f64], l: f64, f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        self.0.add(u, l, f, t)
    }
    fn add_full(&self, u: &[f64], l: f64, f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        self.0.add(u, l, f, t)
    }
}

/// Every tangent of the follower at `u = 0` (NZ).
struct AtZero<'a>(&'a FollowerPressure);

impl Contribution for AtZero<'_> {
    fn name(&self) -> &str {
        "at zero"
    }
    fn add(&self, u: &[f64], l: f64, f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        self.0.add(&vec![0.0; u.len()], l, f, t)
    }
    fn add_full(&self, u: &[f64], l: f64, f: &mut [f64], t: &mut Vec<(usize, usize, f64)>) -> rcfd_fem::Result<()> {
        self.0.add_full(&vec![0.0; u.len()], l, f, t)
    }
}

fn print_pairs(name: &str, b: &ComplexBuckling) {
    println!("{name}: steps {}", b.steps);
    for i in 0..b.factors.len() {
        println!(
            "  mu {:.10} {:+.10} i, residual {:.3e}, eta {:.3e}",
            b.factors[i].re, b.factors[i].im, b.residuals[i], b.backward_errors[i]
        );
    }
}

// ------------------------------------------------------------------------------------------------------ buckling

#[test]
fn flutter_state_buckling() {
    let mut c = Checks::new();
    let model = bar(true);
    let n = model.dof_map().n_equations();
    let fp = FollowerPressure::new(&model, "xmax", P_F).unwrap();
    let Some((u, states)) = state(&mut c, &model, &fp) else {
        c.finish();
        return;
    };
    let free = free_mask(&model);
    let pre = dead_prestress(&model);
    let kg = geometric_stiffness(&model, &pre, None).unwrap();
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fp: &fp,
    };
    let options = BucklingOptions::new(FACTORS);

    // SA: free-edged.
    let kp = CsrMatrix::from_triplets(n, fp.load_stiffness(&u, 1.0).unwrap());
    let mut dense = vec![vec![0.0; n]; n];
    for (i, row) in dense.iter_mut().enumerate() {
        for k in kp.row_ptr[i]..kp.row_ptr[i + 1] {
            row[kp.col[k]] += kp.val[k];
        }
    }
    let (mut anti, mut big) = (0.0f64, 0.0f64);
    for i in (0..n).filter(|&i| free[i]) {
        for j in (0..n).filter(|&j| free[j]) {
            anti = anti.max((dense[i][j] - dense[j][i]).abs());
            big = big.max(dense[i][j].abs());
        }
    }
    c.at_least("SA", anti / big, 1e-1);

    // BT, BV, BE, NG.
    let fpc: &dyn Contribution = &fp;
    match tangent_buckling_factors_full_with(&model, &[], &u, &states, 1.0, &[fpc], &pre, &options) {
        Ok(b) => {
            print_pairs("BT pairs at the state", &b);
            let complex = b.factors.iter().any(|m| m.im.abs() > 1e-6 * m.abs());
            c.holds(
                "BT",
                b.factors.len() >= FACTORS && complex && b.factors.iter().all(|m| m.is_finite()),
                &format!("{} pairs, complex pair present: {complex}", b.factors.len()),
            );
            c.at_most("BV", buckling_measure(&st, &kg, &free, &b, true), 1e-6);
            c.holds(
                "BE lengths",
                b.backward_errors.len() == b.factors.len(),
                &format!("{} backward errors", b.backward_errors.len()),
            );
            c.at_most("BE", max_of(&b.backward_errors), 2e-10);
            c.at_least("NG", buckling_measure(&st, &kg, &free, &b, false), 1e-2);
        }
        Err(e) => c.holds("BT", false, &e.to_string()),
    }

    // BZ, NR.
    let zero = vec![0.0; n];
    let init = initial_states(&model).unwrap();
    let at_rest = tangent_buckling_factors_full_with(&model, &[], &zero, &init, 0.0, &[fpc], &pre, &options);
    let reference = buckling_factors_full(&model, &pre, &[&fp], &options);
    match (at_rest, reference) {
        (Ok(a), Ok(r)) => {
            print_pairs("BZ at zero load", &a);
            print_pairs("BZ reference buckling_factors_full", &r);
            let d = if a.factors.len() == r.factors.len() {
                a.factors
                    .iter()
                    .zip(&r.factors)
                    .map(|(x, y)| (*x - *y).abs() / y.abs())
                    .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY })
            } else {
                f64::INFINITY
            };
            c.at_most("BZ", d, 1e-10);
            c.at_least("NR", buckling_measure(&st, &kg, &free, &r, true), 1e-3);
        }
        (a, r) => c.holds("BZ", false, &format!("{:?} / {:?}", a.err(), r.err())),
    }

    // NS.
    let sym = SymPart(&fp);
    match tangent_buckling_factors_full_with(&model, &[], &u, &states, 1.0, &[&sym], &pre, &options) {
        Ok(b) => {
            print_pairs("NS pairs", &b);
            c.at_least("NS", buckling_measure(&st, &kg, &free, &b, true), 1e-2);
        }
        Err(e) => c.holds("NS", false, &e.to_string()),
    }
    // NZ.
    let az = AtZero(&fp);
    match tangent_buckling_factors_full_with(&model, &[], &u, &states, 1.0, &[&az], &pre, &options) {
        Ok(b) => {
            print_pairs("NZ pairs", &b);
            c.at_least("NZ", buckling_measure(&st, &kg, &free, &b, true), 1e-5);
        }
        Err(e) => c.holds("NZ", false, &e.to_string()),
    }

    // R1.
    let r0 = tangent_buckling_factors_full_with(&model, &[], &u, &states, 1.0, &[fpc], &pre, &BucklingOptions::new(0));
    c.holds(
        "R1 factors 0",
        matches!(&r0, Err(FemError::InvalidValue { key: "factors", .. })),
        &format!("{:?}", r0.as_ref().err()),
    );
    let loose = bar(false);
    let fl = FollowerPressure::new(&loose, "xmax", P_F).unwrap();
    let r1 = tangent_buckling_factors_full_with(&loose, &[], &zero, &init, 0.0, &[&fl], &pre, &options);
    c.holds(
        "R1 rigid-free",
        matches!(&r1, Err(FemError::RigidBodyMotionFree { .. })),
        &format!("{:?}", r1.as_ref().err()),
    );
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- modes

#[test]
fn flutter_state_modes() {
    let mut c = Checks::new();
    let model = bar(true);
    let fp = FollowerPressure::new(&model, "xmax", P_F).unwrap();
    let Some((u, states)) = state(&mut c, &model, &fp) else {
        c.finish();
        return;
    };
    let free = free_mask(&model);
    let mass = consistent(&model).unwrap();
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fp: &fp,
    };
    let fpc: &dyn Contribution = &fp;
    match tangent_modes_full(&model, &[], &u, &states, 1.0, &[fpc], &ModalOptions::new(MODES)) {
        Ok(m) => {
            println!("MT: steps {}", m.steps);
            for i in 0..m.eigenvalues.len() {
                println!(
                    "  omega^2 {:.10} {:+.3e} i, residual {:.3e}, eta {:.3e}",
                    m.eigenvalues[i].re, m.eigenvalues[i].im, m.residuals[i], m.backward_errors[i]
                );
            }
            c.holds(
                "MT",
                (MODES..=MODES + 1).contains(&m.eigenvalues.len()),
                &format!("{} pairs", m.eigenvalues.len()),
            );
            let pairs: Vec<(C64, Vec<C64>)> = m.eigenvalues.iter().cloned().zip(m.modes.iter().cloned()).collect();
            c.at_most("MV", modal_measure(&st, &mass, &free, &pairs), 1e-6);
            c.at_most("ME", max_of(&m.backward_errors), 2e-10);
        }
        Err(e) => c.holds("MT", false, &e.to_string()),
    }
    match tangent_modes(&model, &[], &u, &states, 1.0, &[fpc], &ModalOptions::new(MODES)) {
        Ok(m) => {
            let w2 = m.eigenvalues.clone();
            println!("NSM: symmetric omega^2 {w2:?}");
            let pairs: Vec<(C64, Vec<C64>)> = w2
                .iter()
                .zip(&m.modes)
                .map(|(w, x)| (C64::from_real(*w), x.iter().map(|v| C64::from_real(*v)).collect()))
                .collect();
            c.at_least("NSM", modal_measure(&st, &mass, &free, &pairs), 1e-3);
        }
        Err(e) => c.holds("NSM", false, &e.to_string()),
    }
    c.finish();
}
