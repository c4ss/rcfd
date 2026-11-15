//! F-608 (ADR 0607): the total Lagrangian contact punch of ADR 0507 T2 with the contactor lifted by `+3e-4` at the four nodes
//! with `x > 1.0`, solved with the convexified tangent on a refused pivot (`ContactOptions::curvature_fallback`, ADR 0570) and
//! the line search on the energy (`NewtonOptions::line_search`, ADR 0607 D1), read by Q(10, 1e-12) of ADR 0519; and the
//! checks of the line search itself (LS0, LS1, LS1n), of the trial force of the contact set (LT), of the refusals (RF) and of
//! the default (NL0). Every limit is the one registered in ADR 0607.

use rcfd_fem::arclength::{trace, ArcLengthOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, ContactSet, Kinematics, Target};
use rcfd_fem::error::Result as FemResult;
use rcfd_fem::nonlinear::{initial_states, solve_from, Contribution, NewtonOptions};
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use std::cell::RefCell;

/// ADR 0519 D1, registered again in ADR 0607 A2.
const C: f64 = 10.0;
const F: f64 = 1e-12;
const INFORMATIVE: f64 = 1e-6;
/// The registered initial gap of L1.
const G0: f64 = 3e-4;
/// The dead pressure on the punch top and the top's area.
const P: f64 = 0.01;
const AREA: f64 = 0.4 * 0.2;

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

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------------------ the punch

/// The punch of ADR 0507 D7 T2 as `sa_contact_order.rs` builds it (2 steps, `p = 0.01`), with the initial gap `g0` at the
/// four punch nodes with `x > 1.0`, the fallback `fb` and the line search `ls`.
fn punch(g0: f64, fb: bool, ls: bool) -> (SolidModel, ContactPair, ContactOptions) {
    let plate = hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap();
    let pn = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &pn)]).unwrap();
    let mut m = SolidModel::new(mesh);
    let mat = || Material::isotropic_elastic(1.0, 0.3, 1.0, 0.0).unwrap();
    m.add_material("plate.all", mat()).unwrap();
    m.add_material("punch.all", mat()).unwrap();
    for s in ["plate.xmin", "plate.xmax"] {
        for cc in 0..3 {
            m.add_constraint(Constraint::displacement(s, cc, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::displacement("plate.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.ymin", 1, 0.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("punch.zmax", 0, 0.0))
        .unwrap();
    m.add_load(Load::pressure("punch.zmax", P)).unwrap();
    let mut pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    if g0 != 0.0 {
        for id in 0..m.mesh().nodes().len() {
            let x = m.mesh().nodes()[id];
            if x[0] > 1.0 && x[0] <= 1.2 + 1e-9 && x[1] <= 0.2 + 1e-9 && (x[2] - 0.25).abs() < 1e-12 {
                pair = pair.initial_gap(id, g0);
            }
        }
    }
    let o = ContactOptions::new(2, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30).line_search(ls))
        .kinematics(Kinematics::TotalLagrangian)
        .curvature_fallback(fb);
    (m, pair, o)
}

fn run(g0: f64, fb: bool, ls: bool) -> Result<ContactResult, FemError> {
    let (m, pair, o) = punch(g0, fb, ls);
    solve(&m, &[], &[], &[pair], &o)
}

#[derive(Default, Debug)]
struct Reading {
    read: usize,
    exempt: usize,
    violations: usize,
    max_ratio: f64,
    informative: Vec<usize>,
    consistent: bool,
}

/// The reader of ADR 0519 D1 (copied from `sa_contact_order.rs`, the registered form only).
fn read(r: &ContactResult) -> Reading {
    let mut out = Reading {
        consistent: true,
        ..Reading::default()
    };
    for s in &r.steps {
        let mut inf = 0;
        for v in &s.solves {
            let res = &v.residuals;
            if v.active_sets.len() != res.len() || res.iter().any(|x| !x.is_finite()) {
                out.consistent = false;
                continue;
            }
            for k in 1..res.len().saturating_sub(1) {
                if v.active_sets[k + 1] != v.active_sets[k] {
                    out.exempt += 1;
                    continue;
                }
                out.read += 1;
                let (rk, rn) = (res[k], res[k + 1]);
                if rn > (C * rk * rk).max(F) {
                    out.violations += 1;
                }
                if rn > F && rk > 0.0 {
                    let q = rn / (rk * rk);
                    assert!(q.is_finite(), "non-finite ratio");
                    out.max_ratio = out.max_ratio.max(q);
                }
                if rk >= INFORMATIVE {
                    inf += 1;
                }
            }
        }
        out.informative.push(inf);
    }
    out
}

/// The order of ADR 0404 (a4) as ADR 0507 T2 applied it (N5 of ADR 0519, a report).
fn old_orders(r: &ContactResult) -> Vec<f64> {
    let mut orders = Vec::new();
    for s in &r.steps {
        for v in &s.solves {
            let kept: Vec<usize> = (0..v.residuals.len()).filter(|&k| v.residuals[k] >= 1e-12).collect();
            if kept.len() < 3 {
                continue;
            }
            let (a, b, c) = (kept[kept.len() - 3], kept[kept.len() - 2], kept[kept.len() - 1]);
            if v.active_sets[a] != v.active_sets[b] || v.active_sets[b] != v.active_sets[c] {
                continue;
            }
            let (ra, rb, rc) = (v.residuals[a], v.residuals[b], v.residuals[c]);
            orders.push((rc / rb).ln() / (rb / ra).ln());
        }
    }
    orders
}

fn print_run(tag: &str, r: &ContactResult) {
    for s in &r.steps {
        for (i, v) in s.solves.iter().enumerate() {
            let sets: Vec<usize> = v.active_sets.iter().map(|a| a.len()).collect();
            println!(
                "{tag} step {} solve {i}: residuals {:?}; active counts {sets:?}; fallback iterations {}; step lengths {:?}; \
                 penetration {:.3e}",
                s.step,
                v.residuals.iter().map(|x| format!("{x:.3e}")).collect::<Vec<_>>(),
                v.convex_iterations,
                v.step_lengths.iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>(),
                v.penetration
            );
        }
    }
    for n in &r.nodes {
        println!(
            "{tag} node {}: gap {:?}, force {:.4e}, active {}",
            n.node, n.gap, n.force, n.active
        );
    }
}

// ------------------------------------------------------------------------------------------------------------ L1

#[test]
fn a1_a2_a3_lsr_lifted_punch() {
    let mut c = Checks::new();
    let r = run(G0, true, true);
    c.holds("A1 L1 converges", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    let Ok(r) = r else {
        c.finish();
        return;
    };
    print_run("L1", &r);
    let q = read(&r);
    println!(
        "L1 reading: read {}, exempt {}, violations {}, max ratio {:.4e}, informative per step {:?}",
        q.read, q.exempt, q.violations, q.max_ratio, q.informative
    );
    c.holds(
        "A2 Q(10, 1e-12) violations",
        q.violations == 0,
        &format!("{}", q.violations),
    );
    c.holds("A2c one active set per residual, residuals finite", q.consistent, "");
    c.holds(
        "A2i an informative read pair in every load step",
        q.informative.iter().all(|&n| n >= 1),
        &format!("{:?}", q.informative),
    );
    c.at_most("A2r largest r_{k+1}/r_k^2 of the read pairs above F", q.max_ratio, 10.0);
    // A3: the contact forces' z components against the dead pressure resultant.
    let mut fz = 0.0;
    for n in &r.nodes {
        assert!(n.force.is_finite() && n.normal[2].is_finite());
        fz += n.force * n.normal[2];
    }
    c.at_most(
        "A3 force balance |sum t n_z| / (p A) - 1",
        (fz.abs() / (P * AREA) - 1.0).abs(),
        1e-8,
    );
    // LSr: the step lengths.
    let mut lengths_ok = true;
    let mut some_damped = false;
    let mut augmentations = Vec::new();
    for s in &r.steps {
        augmentations.push(s.solves.len() - 1);
        for v in &s.solves {
            lengths_ok &= v.step_lengths.len() == v.energies.len();
            lengths_ok &= v.step_lengths.iter().all(|&a| a > 0.0 && a <= 1.0);
            some_damped |= v.step_lengths.iter().any(|&a| a < 1.0);
        }
    }
    println!("L1 augmentations per step {augmentations:?}");
    c.holds("LSr one length per correction, each in (0, 1]", lengths_ok, "");
    c.holds("LSr some length below 1", some_damped, "");
    println!("L1 N5 order of ADR 0404 (a4): {:?}", old_orders(&r));
    c.finish();
}

/// Reports without a limit: L1-f (fallback only), L1-l (line search only), L1-g (other gaps, both options).
#[test]
fn reports_l1f_l1l_l1g() {
    for (tag, g0, fb, ls) in [
        ("L1-f", G0, true, false),
        ("L1-l", G0, false, true),
        ("L1-g 1e-4", 1e-4, true, true),
        ("L1-g 1e-3", 1e-3, true, true),
        ("L1-g 5e-3", 5e-3, true, true),
        ("L1-g 2e-2", 2e-2, true, true),
    ] {
        match run(g0, fb, ls) {
            Ok(r) => {
                let q = read(&r);
                println!(
                    "{tag}: converged; Q violations {} of {} read pairs, max ratio {:.3e}",
                    q.violations, q.read, q.max_ratio
                );
                print_run(tag, &r);
            }
            Err(e) => println!("{tag}: failed: {e}"),
        }
    }
}

#[test]
fn nl0_default_keeps_the_refusal() {
    let mut c = Checks::new();
    let e = run(G0, false, false);
    let text = match &e {
        Err(FemError::Solver { solver, reason }) if *solver == "contact" => reason.clone(),
        other => format!("not the contact solver error: {other:?}"),
    };
    println!("NL0: {text}");
    c.holds(
        "NL0 refused with the texts",
        text.contains("load step 1 of 2, augmentation 0")
            && text.contains("singular tangent: pivot")
            && text.contains("at node 67 component 2"),
        &text,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ LS0, LS1

/// A spring `k u_q` at `q` whose reported tangent is `k` (honest) or 0 (`soft`).
struct Spring {
    q: usize,
    k: f64,
    soft: bool,
}

impl Contribution for Spring {
    fn name(&self) -> &str {
        "spring"
    }
    fn add(&self, u: &[f64], _lf: f64, force: &mut [f64], tangent: &mut Vec<(usize, usize, f64)>) -> FemResult<()> {
        force[self.q] += self.k * u[self.q];
        tangent.push((self.q, self.q, if self.soft { 0.0 } else { self.k }));
        Ok(())
    }
}

/// Records the displacement of every `add` (no force); its trial force records nothing.
struct Recorder(RefCell<Vec<Vec<f64>>>);

impl Contribution for Recorder {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(&self, u: &[f64], _lf: f64, _force: &mut [f64], _tangent: &mut Vec<(usize, usize, f64)>) -> FemResult<()> {
        self.0.borrow_mut().push(u.to_vec());
        Ok(())
    }
    fn add_trial_force(&self, _u: &[f64], _lf: f64, _force: &mut [f64]) -> FemResult<()> {
        Ok(())
    }
}

/// ADR 0570's cube FD: a unit Hex8, `zmin` fixed; `q` the equation of `u_z` at `(1, 1, 1)`.
fn cube() -> (SolidModel, usize) {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let node = mesh.nodes().iter().position(|x| *x == [1.0, 1.0, 1.0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(1.0, 0.3, 1.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    let q = m.dof_map().equation(node, 2).unwrap();
    (m, q)
}

#[test]
fn ls0_full_steps_keep_the_bits() {
    let mut c = Checks::new();
    let (m, q) = cube();
    let n = m.dof_map().n_equations();
    let mut load = vec![0.0; n];
    load[q] = 0.01;
    let start = vec![0.0; n];
    let states = initial_states(&m).unwrap();
    let honest = Spring { q, k: 1.0, soft: false };
    let off = solve_from(&m, &[], &load, &start, &states, &NewtonOptions::new(1), &[&honest]).unwrap();
    let on = solve_from(
        &m,
        &[],
        &load,
        &start,
        &states,
        &NewtonOptions::new(1).line_search(true),
        &[&honest],
    )
    .unwrap();
    println!("LS0 residuals {:?}, lengths {:?}", on.residuals, on.step_lengths);
    c.holds(
        "LS0 displacement, residuals, energies bit for bit",
        bits(&on.u) == bits(&off.u)
            && bits(&on.residuals) == bits(&off.residuals)
            && bits(&on.energies) == bits(&off.energies),
        "",
    );
    c.holds(
        "LS0 lengths all 1 and one per correction",
        on.step_lengths.len() == on.energies.len() && on.step_lengths.iter().all(|&a| a == 1.0),
        &format!("{:?}", on.step_lengths),
    );
    c.holds(
        "LS0 no lengths without the option",
        off.step_lengths.is_empty(),
        &format!("{:?}", off.step_lengths),
    );
    c.finish();
}

#[test]
fn ls1_line_minimum_and_ls1n() {
    let mut c = Checks::new();
    let (m, q) = cube();
    let n = m.dof_map().n_equations();
    let (f, k) = (1e-4, 10.0);
    let mut load = vec![0.0; n];
    load[q] = f;
    let start = vec![0.0; n];
    let states = initial_states(&m).unwrap();
    let soft = Spring { q, k, soft: true };
    let second = |ls: bool| {
        let rec = Recorder(RefCell::new(Vec::new()));
        let o = NewtonOptions::new(1).max_iterations(1).line_search(ls);
        let e = solve_from(&m, &[], &load, &start, &states, &o, &[&soft, &rec]);
        assert!(e.is_err(), "one iteration does not converge");
        let seen = rec.0.borrow();
        assert_eq!(seen.len(), 2, "two evaluations of add");
        seen[1][q]
    };
    let d_q = second(false);
    let a_1 = second(true) / d_q;
    let a_star = f / (f + k * d_q);
    println!("LS1 d_q {d_q:.6e}, alpha_1 {a_1:.6e}, alpha* {a_star:.6e}");
    c.at_most("LS1 |alpha_1/alpha* - 1|", (a_1 / a_star - 1.0).abs(), 1e-2);
    c.holds("LS1 0 < alpha_1 < 1", a_1 > 0.0 && a_1 < 1.0, &format!("{a_1:e}"));
    let o = NewtonOptions::new(1).max_iterations(20);
    let e = solve_from(&m, &[], &load, &start, &states, &o, &[&soft]);
    println!("LS1n: {:?}", e.as_ref().err());
    c.holds("LS1n without the line search the solve fails", e.is_err(), "");
    let o = NewtonOptions::new(1).max_iterations(20).line_search(true);
    let e = solve_from(&m, &[], &load, &start, &states, &o, &[&soft]);
    println!(
        "LS1 (report) with the line search, 20 iterations: {:?}",
        e.as_ref().map(|x| x.residuals.len()).map_err(|x| x.to_string())
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ LT, RF

#[test]
fn lt_trial_force_records_nothing() {
    let mut c = Checks::new();
    let (m, pair, o) = punch(G0, true, true);
    let set = ContactSet::new(&m, &[pair], &o).unwrap();
    let n = m.dof_map().n_equations();
    let u: Vec<f64> = (0..n).map(|i| 1e-3 * (1.7 * i as f64 + 0.3).sin()).collect();
    let before = set.active_sets().len();
    let mut ft = vec![0.0; n];
    set.add_trial_force(&u, 1.0, &mut ft).unwrap();
    let after_trial = set.active_sets().len();
    let mut fa = vec![0.0; n];
    let mut t = Vec::new();
    set.add(&u, 1.0, &mut fa, &mut t).unwrap();
    let after_add = set.active_sets().len();
    let nonzero = fa.iter().filter(|&&x| x != 0.0).count();
    println!("LT sets {before} -> {after_trial} -> {after_add}; non-zero force entries {nonzero}");
    c.holds("LT force bit for bit", bits(&ft) == bits(&fa), "");
    c.holds(
        "LT the trial records nothing, add records one set",
        after_trial == before && after_add == before + 1,
        &format!("{before}, {after_trial}, {after_add}"),
    );
    c.holds("LT the state has contact forces", nonzero > 0, "");
    c.finish();
}

fn key_of<T>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

#[test]
fn rf_refusals() {
    let mut c = Checks::new();
    let (m, q) = cube();
    let n = m.dof_map().n_equations();
    let mut load = vec![0.0; n];
    load[q] = 0.01;
    let start = vec![0.0; n];
    let states = initial_states(&m).unwrap();
    let o = NewtonOptions::new(1).line_search(true).unsymmetric(true);
    let e = solve_from(&m, &[], &load, &start, &states, &o, &[]);
    c.holds(
        "RF unsymmetric with the line search",
        key_of(&e) == Some("line_search"),
        &format!("{:?}", e.err()),
    );
    let ao = ArcLengthOptions::new(0.1).newton(NewtonOptions::new(1).line_search(true));
    let e = trace(&m, &[], &[], &ao, &[], &mut |_| true);
    c.holds(
        "RF arc length with the line search",
        key_of(&e) == Some("line_search"),
        &format!("{:?}", e.err()),
    );
    let (pm, pair, _) = punch(G0, false, false);
    let lo = ContactOptions::new(2, 1e-8)
        .newton(NewtonOptions::new(1).line_search(true))
        .kinematics(Kinematics::Linear);
    let e = solve(&pm, &[], &[], &[pair], &lo);
    c.holds(
        "RF linear contact body with the line search",
        key_of(&e) == Some("line_search"),
        &format!("{:?}", e.err()),
    );
    c.finish();
}
