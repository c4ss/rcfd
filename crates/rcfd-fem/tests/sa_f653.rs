//! F-653 (ADR 0580): flutter of follower loads. Beck's column (a Hex20 cantilever with a follower pressure on its end face)
//! by `tangent_modes_full` and a bisection in the load (F1 to F3), the symmetric part as control (F4), the full solver on a
//! symmetric tangent against `tangent_modes` (F5), the full buckling path (F6 to F8) and the refusals (F9). Each registered
//! run is made once; a miss stays as an ignored `*_known_miss` test with its limit.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::follower::{
    buckling_factors, buckling_factors_full, tangent_modes, tangent_modes_full, ComplexModes, FollowerPressure,
};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{initial_states, Contribution};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, LinearTerm, Load, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::{ResidualTest, Shift};
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.9e} (recorded)");
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const L: f64 = 1.0;
const B: f64 = 0.04;
const H: f64 = 0.02;
/// The derived flutter coefficient and coalescence (ADR 0580 arithmetic 1, `docs/reference-data.md`).
const P_CR: f64 = 20.0509536;
const W_CR: f64 = 121.34251;

fn ei() -> f64 {
    E * B * H * H * H / 12.0
}

fn p_ref() -> f64 {
    20.0 * ei() / (L * L * B * H)
}

fn beck() -> SolidModel {
    let base = hex_box([0.0; 3], [L, B, H], [50, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

struct Beck {
    model: SolidModel,
    dead: SolidModel,
    ulin: Vec<f64>,
    pre: Prestress,
}

fn setup() -> Beck {
    let model = beck();
    let mut dead = beck();
    dead.add_load(Load::pressure("xmax", p_ref())).unwrap();
    let s = solve_static(&dead, &StaticOptions::default()).unwrap();
    let ulin = s.displacement_values().to_vec();
    let pre = Prestress::from_static(&dead, &s).unwrap();
    Beck {
        model,
        dead,
        ulin,
        pre,
    }
}

/// `tangent_modes_full` at the load coefficient `p` (in `E I/L^2`) about `u = (p/20) u_lin`.
fn modes_at(b: &Beck, p: f64, contributions: &[&dyn Contribution], count: usize) -> ComplexModes {
    let lf = p / 20.0;
    let u: Vec<f64> = b.ulin.iter().map(|x| lf * x).collect();
    let states = initial_states(&b.model).unwrap();
    tangent_modes_full(&b.model, &[], &u, &states, lf, contributions, &ModalOptions::new(count)).unwrap()
}

fn is_flutter(r: &ComplexModes) -> bool {
    r.eigenvalues.iter().any(|v| v.im.abs() > 1e-6 * v.abs())
}

fn max_res(r: &[f64]) -> f64 {
    r.iter().fold(0.0f64, |m, &x| m.max(x))
}

#[test]
fn f1_f2_f3_beck_flutter() {
    let mut ck = Checks::new();
    let b = setup();
    let fp = FollowerPressure::new(&b.model, "xmax", p_ref()).unwrap();
    let list: Vec<&dyn Contribution> = vec![&fp];
    let mut res: f64 = 0.0;
    let mut eval = |p: f64| {
        let r = modes_at(&b, p, &list, 6);
        res = res.max(max_res(&r.residuals));
        println!(
            "p = {p:.9}: flutter {}, steps {}, omega^2 m L^4/(E I) = {:?}",
            is_flutter(&r),
            r.steps,
            r.eigenvalues
                .iter()
                .map(|v| (v.re * RHO * B * H / ei(), v.im * RHO * B * H / ei()))
                .collect::<Vec<_>>()
        );
        r
    };
    let lo_r = eval(16.0);
    let hi_r = eval(24.0);
    let bracket = !is_flutter(&lo_r) && is_flutter(&hi_r);
    ck.holds("F1e flutter(16) false, flutter(24) true", bracket, "");
    if bracket {
        let (mut lo, mut hi) = (16.0f64, 24.0f64);
        let mut last_hi = hi_r;
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            let r = eval(mid);
            if is_flutter(&r) {
                hi = mid;
                last_hi = r;
            } else {
                lo = mid;
            }
        }
        let p_meas = 0.5 * (lo + hi);
        println!("bracket [{lo:.12}, {hi:.12}], p_meas = {p_meas:.10}");
        ck.at_most("F1 |p_meas/20.0509536 - 1|", (p_meas / P_CR - 1.0).abs(), 1e-2);
        ck.record("F1 p_meas/20.0509536 - 1", p_meas / P_CR - 1.0);
        let pair = last_hi
            .eigenvalues
            .iter()
            .filter(|v| v.im.abs() > 1e-6 * v.abs())
            .fold(None::<C64>, |m, v| match m {
                Some(x) if x.abs() <= v.abs() => Some(x),
                _ => Some(*v),
            })
            .unwrap();
        let w = pair.re * RHO * B * H * L.powi(4) / ei();
        println!("coalescence Omega^2 = {w:.8} (+/- i {:.3e})", pair.im * RHO * B * H / ei());
        ck.at_most("F2 |Omega^2/121.34251 - 1|", (w / W_CR - 1.0).abs(), 2e-2);
        ck.record("F2 Omega^2/121.34251 - 1", w / W_CR - 1.0);
    }
    ck.at_most("F3 max residual over every evaluation", res, 1e-8);
    ck.finish();
}

/// The follower with its symmetric part as the "full" tangent (the control F4).
struct SymmetricOnly<'a>(&'a FollowerPressure);

impl Contribution for SymmetricOnly<'_> {
    fn name(&self) -> &str {
        "symmetric part"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.0.add(u, load_factor, force, tangent)
    }
    fn add_full(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.0.add(u, load_factor, force, tangent)
    }
}

#[test]
fn f4_symmetric_part_control() {
    let mut ck = Checks::new();
    let b = setup();
    let fp = FollowerPressure::new(&b.model, "xmax", p_ref()).unwrap();
    let sym = SymmetricOnly(&fp);
    let list: Vec<&dyn Contribution> = vec![&sym];
    let r = modes_at(&b, 1.01 * P_CR, &list, 6);
    let scaled: Vec<(f64, f64)> = r
        .eigenvalues
        .iter()
        .map(|v| (v.re * RHO * B * H / ei(), v.im * RHO * B * H / ei()))
        .collect();
    println!("F4 omega^2 m L^4/(E I) = {scaled:?}");
    let im = r.eigenvalues.iter().map(|v| v.im.abs() / v.abs()).fold(0.0f64, f64::max);
    let lowest = r.eigenvalues.iter().map(|v| v.re).fold(f64::INFINITY, f64::min);
    ck.at_most("F4 max |Im|/|lambda| with the symmetric part", im, 1e-8);
    ck.holds(
        "F4 lowest Re lambda < 0",
        lowest < 0.0,
        &format!("{:.6}", lowest * RHO * B * H / ei()),
    );
    ck.finish();
}

#[test]
fn f5_symmetric_tangent() {
    let mut ck = Checks::new();
    let b = setup();
    let lf = 1.0 / 20.0;
    let u: Vec<f64> = b.ulin.iter().map(|x| lf * x).collect();
    let states = initial_states(&b.model).unwrap();
    let full = tangent_modes_full(&b.model, &[], &u, &states, lf, &[], &ModalOptions::new(4)).unwrap();
    let sym = tangent_modes(
        &b.model,
        &[],
        &u,
        &states,
        lf,
        &[],
        &ModalOptions::new(4).residual_test(ResidualTest::Backward),
    )
    .unwrap();
    println!("F5 full {:?}\nF5 symmetric {:?}", full.eigenvalues, sym.eigenvalues);
    let mut rel: f64 = 0.0;
    let mut im: f64 = 0.0;
    for (f, s) in full.eigenvalues.iter().zip(&sym.eigenvalues) {
        rel = rel.max((f.re / s - 1.0).abs());
        im = im.max(f.im.abs() / f.abs());
    }
    ck.holds("F5 four eigenvalues", full.eigenvalues.len() >= 4, "");
    ck.at_most("F5 max |lambda_f/lambda_s - 1|", rel, 1e-8);
    ck.at_most("F5 max |Im|/|lambda|", im, 1e-8);
    ck.finish();
}

#[test]
fn f6_f7_f8_buckling() {
    let mut ck = Checks::new();
    let b = setup();
    let fp = FollowerPressure::new(&b.dead, "xmax", p_ref()).unwrap();
    let dead_full = buckling_factors_full(&b.dead, &b.pre, &[], &BucklingOptions::new(2)).unwrap();
    let dead_sym = factors(&b.dead, &b.pre, &BucklingOptions::new(2)).unwrap();
    println!("F6 full {:?}\nF6 symmetric {:?}", dead_full.factors, dead_sym.factors);
    let mut rel: f64 = 0.0;
    let mut im: f64 = 0.0;
    for (f, s) in dead_full.factors.iter().zip(&dead_sym.factors) {
        rel = rel.max((f.re / s - 1.0).abs());
        im = im.max(f.im.abs() / f.abs());
    }
    ck.holds("F6 two factors", dead_full.factors.len() >= 2, "");
    ck.at_most("F6 max |mu_f/mu_s - 1|", rel, 1e-8);
    ck.at_most("F6 max |Im|/|mu|", im, 1e-8);
    ck.record("F6r 20 mu_1/(pi^2/4) - 1", 20.0 * dead_full.factors[0].re / (PI * PI / 4.0) - 1.0);
    let beck_full = buckling_factors_full(&b.dead, &b.pre, &[&fp], &BucklingOptions::new(4)).unwrap();
    println!(
        "F7 factors (in p_ref) {:?}; times 20: {:?}",
        beck_full.factors,
        beck_full.factors.iter().map(|v| (20.0 * v.re, 20.0 * v.im)).collect::<Vec<_>>()
    );
    let bad = beck_full
        .factors
        .iter()
        .filter(|v| v.im.abs() <= 1e-8 * v.abs() && v.re > 0.0 && v.re <= 2.0 * P_CR / 20.0)
        .count();
    ck.holds(
        "F7 no real positive factor <= 2.005",
        bad == 0,
        &format!("{bad} such factors of {}", beck_full.factors.len()),
    );
    let sym = buckling_factors(&b.dead, &b.pre, &[&fp], &BucklingOptions::new(1)).unwrap();
    println!("F7c symmetric part factor {:?}", sym.factors);
    ck.holds(
        "F7c the symmetric path has a factor in (0, 2.005]",
        sym.factors.first().is_some_and(|&f| f > 0.0 && f <= 2.0 * P_CR / 20.0),
        &format!("{:?}", sym.factors),
    );
    let res = max_res(&dead_full.residuals).max(max_res(&beck_full.residuals));
    ck.at_most("F8 max residual of F6 and F7", res, 1e-8);
    ck.finish();
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } => Some(key),
        _ => None,
    }
}

#[test]
fn f9_refusals() {
    let mut ck = Checks::new();
    let b = setup();
    let base = hex_box([0.0; 3], [L, B, H], [50, 1, 1]).unwrap();
    let mut free = SolidModel::new(upgrade_to_hex20(&base).unwrap().0);
    free.add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    let n = free.dof_map().n_equations();
    let e = tangent_modes_full(
        &free,
        &[],
        &vec![0.0; n],
        &initial_states(&free).unwrap(),
        0.0,
        &[],
        &ModalOptions::new(4),
    )
    .unwrap_err();
    ck.holds("F9 rigid-free, shift None: key shift", key_of(&e) == Some("shift"), &format!("{e}"));
    let nb = b.model.dof_map().n_equations();
    let e = tangent_modes_full(
        &b.model,
        &[],
        &vec![0.0; nb],
        &initial_states(&b.model).unwrap(),
        0.0,
        &[],
        &ModalOptions::new(4).shift(Shift::Auto),
    )
    .unwrap_err();
    ck.holds("F9 Shift::Auto: key shift", key_of(&e) == Some("shift"), &format!("{e}"));
    let mut lin = setup().dead;
    let tip: Vec<usize> = lin
        .mesh()
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, x)| (x[0] - L).abs() < 1e-12)
        .map(|(i, _)| i)
        .collect();
    lin.add_constraint(Constraint::linear(
        vec![LinearTerm::new(tip[0], 1, 1.0), LinearTerm::new(tip[1], 1, -1.0)],
        0.0,
    ))
    .unwrap();
    let fp = FollowerPressure::new(&lin, "xmax", p_ref()).unwrap();
    let e = buckling_factors_full(&lin, &b.pre, &[&fp], &BucklingOptions::new(2)).unwrap_err();
    ck.holds(
        "F9 transformation path: key load stiffness",
        key_of(&e) == Some("load stiffness"),
        &format!("{e}"),
    );
    ck.finish();
}
