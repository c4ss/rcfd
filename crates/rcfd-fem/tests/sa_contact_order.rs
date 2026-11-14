//! F-600 (ADR 0519): the reading Q(C, F) of ADR 0515 on the contact Newton solves of the total Lagrangian punch of ADR 0507 T2
//! (and the variants M1 to M3 of ADR 0519 D3). Every limit is the one registered in ADR 0519. D1, Q1 to Q3, N3 to N5 run in
//! `d1_q1_q2_q3_n3_n4_n5_consistent_tangent`; N1 and N2 are the ignored test `n1_n2_wrong_tangent_fails_the_reading`, run in the
//! scratch copies with the mutations MU1 and MU2 of the ADR (it fails by design on the real tangent).

use rcfd_fem::builders::hex_box;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactResult, Kinematics, Target};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, Load, Material, SolidModel};

/// Registered constants of ADR 0519 D1.
const C: f64 = 10.0;
const F: f64 = 1e-12;
/// A pair is informative if `r_k` is at least this.
const INFORMATIVE: f64 = 1e-6;

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

/// The punch of ADR 0507 D7 T2 with `steps` load steps, the dead pressure `p` on the punch top and the initial gap `g0` at
/// the four punch nodes with `x > 1.0` (0: none). `Err` carries the error text of a solve that failed.
fn punch(steps: usize, p: f64, g0: f64) -> Result<ContactResult, String> {
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
    m.add_load(Load::pressure("punch.zmax", p)).unwrap();
    let mut pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    if g0 != 0.0 {
        for id in 0..m.mesh().nodes().len() {
            let x = m.mesh().nodes()[id];
            if x[0] > 1.0 && x[0] <= 1.2 + 1e-9 && x[1] <= 0.2 + 1e-9 && (x[2] - 0.25).abs() < 1e-12 {
                pair = pair.initial_gap(id, g0);
            }
        }
    }
    let o = ContactOptions::new(steps, 1e-8)
        .newton(NewtonOptions::new(1).max_iterations(30))
        .kinematics(Kinematics::TotalLagrangian);
    solve(&m, &[], &[], &[pair], &o).map_err(|e| e.to_string())
}

/// The four models of ADR 0519 D3: name, steps, pressure, initial gap.
const MODELS: [(&str, usize, f64, f64); 4] = [
    ("M0", 2, 0.01, 0.0),
    ("M1", 4, 0.01, 0.0),
    ("M2", 2, 0.02, 0.0),
    ("M3", 2, 0.01, -3e-4),
];

#[derive(Default, Debug)]
struct Reading {
    solves: usize,
    read: usize,
    exempt: usize,
    violations: usize,
    /// The largest `r_{k+1}/r_k^2` of the read pairs above `F` (0 if none).
    max_ratio: f64,
    /// Informative read pairs per load step.
    informative: Vec<usize>,
    /// Solves in which the active set changed.
    changed_solves: usize,
    /// Every solve has as many sets as residuals and every residual is finite.
    consistent: bool,
}

/// ADR 0519 D1: `exempt` takes the pairs whose correction changed the active set out of the reading; `squared` is the registered
/// bound (false: the mutation MR2, `r_{k+1} <= max(C r_k, F)`); `all_changed` is the mutation MR1 (every flag true for k >= 1).
fn read(r: &ContactResult, c: f64, f: f64, exempt: bool, squared: bool, all_changed: bool) -> Reading {
    let mut out = Reading {
        consistent: true,
        ..Reading::default()
    };
    for s in &r.steps {
        let mut inf = 0;
        for v in &s.solves {
            out.solves += 1;
            let res = &v.residuals;
            if v.active_sets.len() != res.len() || res.iter().any(|x| !x.is_finite()) {
                out.consistent = false;
                continue;
            }
            let flag = |k: usize| k >= 1 && v.active_sets[k] != v.active_sets[k - 1];
            if (1..res.len()).any(flag) {
                out.changed_solves += 1;
            }
            for k in 1..res.len().saturating_sub(1) {
                let changed = if all_changed { true } else { flag(k + 1) };
                if exempt && changed {
                    out.exempt += 1;
                    continue;
                }
                out.read += 1;
                let (rk, rn) = (res[k], res[k + 1]);
                let bound = if squared { c * rk * rk } else { c * rk };
                if rn > bound.max(f) {
                    out.violations += 1;
                }
                if rn > f && rk > 0.0 {
                    out.max_ratio = out.max_ratio.max(rn / (rk * rk));
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

/// The measure of ADR 0404 (a4) as ADR 0507 T2 applied it: the last three residuals above `1e-12`, set unchanged over them.
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

/// The order of the last three read residuals above `F` in each solve with three or more (N5, report).
fn new_orders(r: &ContactResult) -> Vec<f64> {
    let mut orders = Vec::new();
    for s in &r.steps {
        for v in &s.solves {
            let kept: Vec<usize> = (1..v.residuals.len()).filter(|&k| v.residuals[k] > F).collect();
            if kept.len() >= 3 {
                let (a, b, c) = (kept[kept.len() - 3], kept[kept.len() - 2], kept[kept.len() - 1]);
                let (ra, rb, rc) = (v.residuals[a], v.residuals[b], v.residuals[c]);
                orders.push((rc / rb).ln() / (rb / ra).ln());
            }
        }
    }
    orders
}

#[test]
fn d1_q1_q2_q3_n3_n4_n5_consistent_tangent() {
    let mut c = Checks::new();
    let mut ratios = Vec::new();
    let mut m3_changed = 0;
    let mut other_changed = 0;
    for (name, steps, p, g0) in MODELS {
        let r = punch(steps, p, g0).unwrap_or_else(|e| panic!("{name} did not converge: {e}"));
        let q = read(&r, C, F, true, true, false);
        println!(
            "{name}: solves {}, read {}, exempt {}, violations {}, max ratio {:.4e}, informative per step {:?}, solves with a changed set {}",
            q.solves, q.read, q.exempt, q.violations, q.max_ratio, q.informative, q.changed_solves
        );
        c.holds(
            &format!("D1 {name} one set per residual, residuals finite"),
            q.consistent,
            "",
        );
        c.holds(
            &format!("Q1 {name} Q(10, 1e-12) violations"),
            q.violations == 0,
            &format!("{}", q.violations),
        );
        c.holds(
            &format!("Q2 {name} an informative pair in every step"),
            q.informative.iter().all(|&n| n >= 1),
            &format!("{:?}", q.informative),
        );
        ratios.push(q.max_ratio);
        if name == "M3" {
            m3_changed = q.changed_solves;
        } else {
            other_changed += q.changed_solves;
        }
        if name == "M0" {
            c.holds(
                "Q2 M0 at least 5 informative pairs",
                q.informative.iter().sum::<usize>() >= 5,
                &format!("{}", q.informative.iter().sum::<usize>()),
            );
            let slack = read(&r, 1.0, F, true, true, false);
            c.holds(
                "N3 Q(1, 1e-12) fails on M0",
                slack.violations > 0,
                &format!("{} violations", slack.violations),
            );
            let nofloor = read(&r, C, 0.0, true, true, false);
            c.holds(
                "N4 Q(10, 0) fails on M0",
                nofloor.violations > 0,
                &format!("{} violations", nofloor.violations),
            );
            println!("N5 M0 order of ADR 0404 (a4): {:?}", old_orders(&r));
            println!(
                "N5 M0 order of the last three read residuals above F: {:?}",
                new_orders(&r)
            );
        }
        if name == "M3" {
            let noex = read(&r, C, F, false, true, false);
            println!(
                "N4 M3 without the exemption: {} violations (report); N5 old orders {:?}, new orders {:?}",
                noex.violations,
                old_orders(&r),
                new_orders(&r)
            );
        }
    }
    c.holds(
        "D1 M3 has a solve with a changed set",
        m3_changed > 0,
        &format!("{m3_changed}"),
    );
    println!("D1 solves with a changed set in M0..M2: {other_changed} (report)");
    let worst = ratios
        .iter()
        .fold(0.0f64, |a, &b| if b.is_finite() { a.max(b) } else { f64::INFINITY });
    c.at_most("Q3 largest r_{k+1}/r_k^2 over M0..M3", worst, 10.0);
    c.finish();
}

/// N1 and N2 of ADR 0519: a wrong contact tangent (MU1: the curvature part dropped; MU2: the stiffness term halved) fails the
/// reading on each of the four models. Passes only in the scratch copy with MU1 or MU2 applied to `contact.rs`.
#[test]
#[ignore = "negative controls N1 and N2 of ADR 0519: pass only in the scratch copies with mutation MU1 or MU2 of contact.rs; on the real tangent they fail by design"]
fn n1_n2_wrong_tangent_fails_the_reading() {
    let mut c = Checks::new();
    for (name, steps, p, g0) in MODELS {
        match punch(steps, p, g0) {
            Err(e) => c.holds(&format!("N {name} fails"), true, &format!("did not converge: {e}")),
            Ok(r) => {
                let q = read(&r, C, F, true, true, false);
                println!("N {name}: violations {}, max ratio {:.4e}", q.violations, q.max_ratio);
                c.holds(
                    &format!("N {name} violates Q(10, 1e-12)"),
                    q.violations > 0,
                    &format!("{}", q.violations),
                );
            }
        }
    }
    c.finish();
}
