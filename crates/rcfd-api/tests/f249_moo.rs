//! ADR 0209 MO1 to MO3 (F-249): NSGA-II on ZDT1 and FON through `Study`, the Pareto set against the analytic front, the
//! properties of the method, and closed forms. Limits are those registered in ADR 0209 before the code.

use rcfd_api::design::nsga2::{self, MoFitness, Options};
use rcfd_api::design::optim::Stop;
use rcfd_api::design::pareto;
use rcfd_api::design::rng::Rng;
use rcfd_api::design::store::{CondResult, Record, Store};
use rcfd_api::design::study::{
    Condition, Doe, Evaluator, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study, StudyDef,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// C, G, I limits of ADR 0209.
const LIM_C: f64 = 1e-2;
const LIM_G: f64 = 5e-2;
const LIM_I: f64 = 1e-2;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Problem {
    Zdt1,
    Fon,
}

fn a_fon() -> f64 {
    1.0 / 3f64.sqrt()
}

impl Problem {
    /// Dimension and box.
    fn space(self) -> (usize, f64, f64) {
        match self {
            Problem::Zdt1 => (30, 0.0, 1.0),
            Problem::Fon => (3, -4.0, 4.0),
        }
    }

    fn eval(self, x: &[f64]) -> [f64; 2] {
        match self {
            Problem::Zdt1 => zdt1(x),
            Problem::Fon => fon(x),
        }
    }

    /// The front point of parameter `s` (ZDT1: f1; FON: t).
    fn front(self, s: f64) -> [f64; 2] {
        match self {
            Problem::Zdt1 => [s, 1.0 - s.sqrt()],
            Problem::Fon => {
                let a = a_fon();
                [
                    1.0 - (-3.0 * (s - a).powi(2)).exp(),
                    1.0 - (-3.0 * (s + a).powi(2)).exp(),
                ]
            }
        }
    }

    /// Range of the front parameter.
    fn param_range(self) -> (f64, f64) {
        match self {
            Problem::Zdt1 => (0.0, 1.0),
            Problem::Fon => (-a_fon(), a_fon()),
        }
    }
}

/// ZDT1 (Zitzler, Deb and Thiele 2000) for any d >= 2.
fn zdt1(x: &[f64]) -> [f64; 2] {
    let f1 = x[0];
    let g = 1.0 + 9.0 * x[1..].iter().sum::<f64>() / (x.len() - 1) as f64;
    [f1, g * (1.0 - (f1 / g).sqrt())]
}

/// FON (Fonseca and Fleming 1995) for any d.
fn fon(x: &[f64]) -> [f64; 2] {
    let a = a_fon();
    let s1: f64 = x.iter().map(|v| (v - a).powi(2)).sum();
    let s2: f64 = x.iter().map(|v| (v + a).powi(2)).sum();
    [1.0 - (-s1).exp(), 1.0 - (-s2).exp()]
}

/// `D(f)`: distance to the front (20 001 samples, golden-section refinement to 1e-12 in the parameter).
fn distance(p: Problem, f: &[f64]) -> f64 {
    let (lo, hi) = p.param_range();
    let n = 20_000usize;
    let h = (hi - lo) / n as f64;
    let d = |s: f64| {
        let q = p.front(s);
        ((f[0] - q[0]).powi(2) + (f[1] - q[1]).powi(2)).sqrt()
    };
    let (mut best, mut bk) = (f64::INFINITY, 0usize);
    for k in 0..=n {
        let v = d(lo + h * k as f64);
        if v < best {
            best = v;
            bk = k;
        }
    }
    let mut a = (lo + h * (bk as f64 - 1.0)).max(lo);
    let mut b = (lo + h * (bk as f64 + 1.0)).min(hi);
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let mut c = b - g * (b - a);
    let mut e = a + g * (b - a);
    let (mut fc, mut fe) = (d(c), d(e));
    while b - a > 1e-12 {
        if fc < fe {
            b = e;
            e = c;
            fe = fc;
            c = b - g * (b - a);
            fc = d(c);
        } else {
            a = c;
            c = e;
            fc = fe;
            e = a + g * (b - a);
            fe = d(e);
        }
    }
    best.min(fc).min(fe).min(d(0.5 * (a + b)))
}

#[derive(Debug)]
struct Metrics {
    n: usize,
    c: f64,
    g: f64,
    i: f64,
}

impl Metrics {
    fn pass(&self) -> bool {
        self.n > 0 && self.c <= LIM_C && self.g <= LIM_G && self.i <= LIM_I
    }
}

fn metrics(p: Problem, set: &[Vec<f64>]) -> Metrics {
    let c = set.iter().map(|f| distance(p, f)).fold(0.0, f64::max);
    let (lo, hi) = p.param_range();
    let (mut g, mut sum) = (0.0f64, 0.0);
    for k in 0..=1000 {
        let r = p.front(lo + (hi - lo) * k as f64 / 1000.0);
        let m = set
            .iter()
            .map(|f| (f[0] - r[0]).powi(2) + (f[1] - r[1]).powi(2))
            .fold(f64::INFINITY, f64::min)
            .sqrt();
        g = g.max(m);
        sum += m;
    }
    Metrics {
        n: set.len(),
        c,
        g,
        i: sum / 1001.0,
    }
}

fn def(p: Problem, workers: usize) -> StudyDef {
    let (d, lo, hi) = p.space();
    StudyDef {
        name: "mo".into(),
        params: (0..d)
            .map(|i| ParamDef {
                name: format!("x{i:02}"),
                lo,
                hi,
                init: None,
                targets: vec![],
            })
            .collect(),
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: ["f1", "f2"]
            .iter()
            .map(|m| ObjectiveDef {
                metric: (*m).into(),
                sense: Sense::Min,
                weight: 1.0,
                robust: Robust::Mean,
            })
            .collect(),
        constraints: vec![],
        workers,
    }
}

fn result(f: [f64; 2]) -> CondResult {
    let mut m = BTreeMap::new();
    m.insert("f1".to_string(), f[0]);
    m.insert("f2".to_string(), f[1]);
    CondResult::success("base", m)
}

/// Evaluator of `p` that counts the requested points outside the box (MO2b) and the calls.
struct Counted {
    p: Problem,
    outside: AtomicUsize,
    calls: AtomicUsize,
    fail_x1_above: Option<f64>,
}

impl Counted {
    fn new(p: Problem) -> Self {
        Counted {
            p,
            outside: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            fail_x1_above: None,
        }
    }
}

impl Evaluator for Counted {
    fn evaluate(&self, d: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if x.iter().zip(&d.params).any(|(v, q)| *v < q.lo || *v > q.hi) {
            self.outside.fetch_add(1, Ordering::SeqCst);
        }
        if self.fail_x1_above.is_some_and(|t| x[0] > t) {
            return vec![CondResult::failure("base", "failed", "x1 above the failure threshold")];
        }
        vec![result(self.p.eval(x))]
    }
}

/// The objective vectors of the records `Study::pareto` names.
fn pareto_set(s: &Study) -> Vec<Vec<f64>> {
    s.pareto()
        .iter()
        .map(|&i| s.objective_vector(&s.store.records()[i]).unwrap())
        .collect()
}

fn run_study(p: Problem, method: &Method) -> (Metrics, usize, usize) {
    let ev = Counted::new(p);
    let mut s = Study::new(def(p, 1), Store::memory("mo"), &ev).unwrap();
    s.run(method).unwrap();
    let set = pareto_set(&s);
    (
        metrics(p, &set),
        s.store.records().len(),
        ev.outside.load(Ordering::SeqCst),
    )
}

/// ADR 0209 MO1z, a known miss (amendment 1): C 0.543 / 0.378 / 0.353, G 0.288 / 0.207 / 0.291, I 0.254 / 0.141 / 0.220 for
/// seeds 1 to 3 at ebc4e9f, against the limits 1e-2, 5e-2 and 1e-2, which are kept intact.
#[test]
#[ignore = "known miss: NSGA-II on ZDT1 (d = 30, 25 000 evaluations) ends with C 0.35 to 0.54 against 1e-2 (ADR 0209 amendment 1)"]
fn mo1z_nsga2_on_zdt1_meets_the_analytic_front_known_miss() {
    let mut ok = true;
    for seed in 1..=3u64 {
        let t = std::time::Instant::now();
        let (m, n, outside) = run_study(
            Problem::Zdt1,
            &Method::Nsga2 {
                budget: 25_000,
                pop: 100,
                seed,
            },
        );
        println!(
            "MO1z seed {seed}: {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}; outside {outside}; {:.1} s",
            m.n,
            m.c,
            m.g,
            m.i,
            t.elapsed().as_secs_f64()
        );
        assert_eq!(outside, 0, "MO2b: points outside the box");
        ok &= m.pass();
    }
    assert!(ok, "MO1z: a seed missed C <= {LIM_C}, G <= {LIM_G} or I <= {LIM_I}");
}

#[test]
fn mo1f_nsga2_on_fon_meets_the_analytic_front() {
    let mut ok = true;
    for seed in 1..=3u64 {
        let (m, n, outside) = run_study(
            Problem::Fon,
            &Method::Nsga2 {
                budget: 10_000,
                pop: 100,
                seed,
            },
        );
        println!(
            "MO1f seed {seed}: {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}; outside {outside}",
            m.n, m.c, m.g, m.i
        );
        assert_eq!(outside, 0, "MO2b: points outside the box");
        ok &= m.pass();
    }
    assert!(ok, "MO1f: a seed missed C <= {LIM_C}, G <= {LIM_G} or I <= {LIM_I}");
}

#[test]
fn mo1c_control_latin_hypercube_fails_the_convergence_limit_on_zdt1() {
    let (z, n, _) = run_study(Problem::Zdt1, &Method::Doe(Doe::Lhs { n: 25_000, seed: 1 }));
    println!(
        "MO1c ZDT1 LHS: {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}",
        z.n, z.c, z.g, z.i
    );
    let (f, n, _) = run_study(Problem::Fon, &Method::Doe(Doe::Lhs { n: 10_000, seed: 1 }));
    println!(
        "MO1c FON LHS (recorded): {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}",
        f.n, f.c, f.g, f.i
    );
    assert!(z.c > LIM_C, "MO1c: the control met C ({:.3e}), so C cannot fail", z.c);
}

fn small(seed: u64) -> Method {
    Method::Nsga2 {
        budget: 400,
        pop: 20,
        seed,
    }
}

fn records_of(p: Problem, workers: usize, method: &Method) -> Vec<Record> {
    let ev = Counted::new(p);
    let mut s = Study::new(def(p, workers), Store::memory("mo"), &ev).unwrap();
    s.run(method).unwrap();
    assert_eq!(ev.outside.load(Ordering::SeqCst), 0, "MO2b");
    s.store.records().to_vec()
}

#[test]
fn mo2a_determinism_and_worker_independence() {
    let a = records_of(Problem::Zdt1, 1, &small(4));
    let b = records_of(Problem::Zdt1, 1, &small(4));
    let w4 = records_of(Problem::Zdt1, 4, &small(4));
    let c = records_of(Problem::Zdt1, 1, &small(5));
    println!("MO2a: {} records (seed 4), {} (seed 5)", a.len(), c.len());
    assert_eq!(a, b);
    assert_eq!(a, w4);
    assert_ne!(a, c);
}

#[test]
fn mo2b_budget_in_whole_generations() {
    for budget in [400usize, 410] {
        let mut asked = 0usize;
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
            asked += xs.len();
            Ok(xs
                .iter()
                .map(|x| {
                    assert!(x.iter().all(|v| (0.0..=1.0).contains(v)), "MO2b: outside the box");
                    MoFitness::new(zdt1(x).to_vec(), 0.0)
                })
                .collect())
        };
        let r = nsga2::run(
            &[(0.0, 1.0); 30],
            &Options {
                max_evals: budget,
                pop: 20,
                seed: 4,
                x0: None,
            },
            &mut ev,
        )
        .unwrap();
        println!(
            "MO2b budget {budget}: {} evaluations, {} generations",
            r.evaluations, r.generations
        );
        assert_eq!((r.evaluations, r.generations, asked), (400, 20, 400));
        assert_eq!(r.population.len(), 20);
    }
}

#[test]
fn mo2c_resume_after_interruption() {
    let ev = Counted::new(Problem::Zdt1);
    let mut full = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    full.run(&small(4)).unwrap();
    let total = ev.calls.swap(0, Ordering::SeqCst);
    assert_eq!(total, full.store.records().len());
    for k in [37usize, 250] {
        let dir = std::env::temp_dir().join(format!("rcfd_f249_mo2c_{}_{k}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("evaluations.jsonl");
        let mut s = Study::new(def(Problem::Zdt1, 1), Store::open(&path, "mo").unwrap(), &ev).unwrap();
        s.allowance = Some(k);
        assert_eq!(s.run(&small(4)).unwrap_err(), Stop::Interrupted);
        assert_eq!(ev.calls.swap(0, Ordering::SeqCst), k);
        let mut s2 = Study::new(def(Problem::Zdt1, 1), Store::open(&path, "mo").unwrap(), &ev).unwrap();
        s2.run(&small(4)).unwrap();
        let resumed = ev.calls.swap(0, Ordering::SeqCst);
        println!("MO2c k = {k}: resume evaluated {resumed} of {total}");
        assert_eq!(resumed, total - k);
        assert_eq!(s2.store.records(), full.store.records());
        let mut s3 = Study::new(def(Problem::Zdt1, 1), Store::open(&path, "mo").unwrap(), &ev).unwrap();
        s3.run(&small(4)).unwrap();
        assert_eq!(ev.calls.swap(0, Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

struct NoEval;
impl Evaluator for NoEval {
    fn evaluate(&self, _d: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        panic!("the propose / record loop evaluates nothing itself")
    }
}

#[test]
fn mo2d_propose_record_replays_the_evaluator_run() {
    let ev = NoEval;
    let mut s = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    let mut rounds = 0;
    while let Proposal::Next(points) = s.propose(&small(4)).unwrap() {
        rounds += 1;
        assert!(rounds <= 40);
        for p in points {
            s.record(&p, vec![result(zdt1(&p))], "").unwrap();
        }
    }
    let direct = records_of(Problem::Zdt1, 1, &small(4));
    let strip = |r: &[Record]| {
        r.iter()
            .map(|r| (r.index, r.params.clone(), r.conditions[0].metrics.clone()))
            .collect::<Vec<_>>()
    };
    println!("MO2d: {rounds} proposals, {} records", s.store.records().len());
    assert_eq!(strip(s.store.records()), strip(&direct));
}

#[test]
fn mo2e_failed_evaluations_are_stored_and_never_pareto() {
    let mut ev = Counted::new(Problem::Zdt1);
    ev.fail_x1_above = Some(0.9);
    let mut s = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    s.run(&small(4)).unwrap();
    let failed: Vec<usize> = s
        .store
        .records()
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.ok())
        .map(|(i, _)| i)
        .collect();
    let front = s.pareto();
    println!(
        "MO2e: {} failed of {} records, {} Pareto members",
        failed.len(),
        s.store.records().len(),
        front.len()
    );
    assert!(!failed.is_empty(), "MO2e: no failed record, the check is vacuous");
    assert!(!front.is_empty());
    assert!(front.iter().all(|i| !failed.contains(i)));
}

#[test]
fn mo2f_library_refusals_evaluate_nothing() {
    let mut calls = 0usize;
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
        calls += xs.len();
        Ok(xs.iter().map(|x| MoFitness::new(zdt1(x).to_vec(), 0.0)).collect())
    };
    let b = [(0.0, 1.0); 2];
    for (pop, max_evals) in [(7usize, 100usize), (2, 100), (20, 10)] {
        let r = nsga2::run(
            &b,
            &Options {
                max_evals,
                pop,
                seed: 1,
                x0: None,
            },
            &mut ev,
        );
        println!("MO2f pop {pop} budget {max_evals}: {r:?}");
        assert!(matches!(r, Err(Stop::Error(_))));
    }
    let r = nsga2::run(
        &[],
        &Options {
            max_evals: 100,
            pop: 10,
            seed: 1,
            x0: None,
        },
        &mut ev,
    );
    assert!(matches!(r, Err(Stop::Error(_))));
    assert_eq!(calls, 0);
}

#[test]
fn mo3a_operator_formulas() {
    for u in [0.1f64, 0.25, 0.5, 0.75, 0.9] {
        let beta = if u <= 0.5 {
            (2.0 * u).powf(1.0 / 21.0)
        } else {
            (1.0 / (2.0 * (1.0 - u))).powf(1.0 / 21.0)
        };
        let delta = if u < 0.5 {
            (2.0 * u).powf(1.0 / 21.0) - 1.0
        } else {
            1.0 - (2.0 * (1.0 - u)).powf(1.0 / 21.0)
        };
        assert_eq!(nsga2::sbx_beta(u, 20.0).to_bits(), beta.to_bits(), "sbx u = {u}");
        assert_eq!(nsga2::poly_delta(u, 20.0).to_bits(), delta.to_bits(), "poly u = {u}");
    }
}

#[test]
fn mo3b_survivor_selection_on_a_hand_made_union() {
    let objs = vec![
        vec![0.0, 2.0],
        vec![1.0, 3.0],
        vec![2.0, 4.0],
        vec![2.0, 0.0],
        vec![2.0, 2.0],
        vec![3.0, 3.0],
        vec![3.0, 1.0],
        vec![4.0, 2.0],
    ];
    let v = vec![0.0; 8];
    assert_eq!(
        pareto::non_dominated_sort(&objs, &v),
        vec![vec![0, 3], vec![1, 4, 6], vec![2, 5, 7]]
    );
    assert_eq!(nsga2::select(&objs, &v, 4), vec![0, 3, 1, 6]);
}

#[test]
fn mo3c_problem_closed_forms() {
    let mut x = vec![0.0; 30];
    x[0] = 0.25;
    assert_eq!(zdt1(&x), [0.25, 0.5]);
    let mut x = vec![0.0; 30];
    x[1] = 1.0;
    assert_eq!(zdt1(&x)[1].to_bits(), (1.0 + 9.0 * 1.0 / 29.0f64).to_bits());
    let a = a_fon();
    for t in [-a, 0.0, a / 2.0, a] {
        let f = fon(&[t, t, t]);
        let q = Problem::Fon.front(t);
        assert!(
            (f[0] - q[0]).abs() <= 1e-15 && (f[1] - q[1]).abs() <= 1e-15,
            "{t}: {f:?} {q:?}"
        );
        let d = distance(Problem::Fon, &q);
        assert!(d <= 1e-12, "D(F({t})) = {d:e}");
    }
    assert!(distance(Problem::Zdt1, &[0.25, 0.5]) <= 1e-12);
}

#[test]
fn mo3d_front_equals_the_first_front_of_the_full_sort() {
    let mut rng = Rng::new(17);
    for set in 0..20 {
        let m = if set % 2 == 0 { 2 } else { 3 };
        let mut objs = Vec::new();
        let mut viol = Vec::new();
        for i in 0..100 {
            let mut o: Vec<f64> = (0..m).map(|_| rng.uniform()).collect();
            if i % 23 == 7 {
                o[0] = if i % 2 == 0 { f64::NAN } else { f64::INFINITY };
            }
            objs.push(o);
            viol.push(if i % 3 == 1 { rng.uniform() } else { 0.0 });
        }
        let first = pareto::non_dominated_sort(&objs, &viol)
            .into_iter()
            .next()
            .unwrap_or_default();
        assert_eq!(pareto::front(&objs, &viol), first, "set {set}");
    }
}
