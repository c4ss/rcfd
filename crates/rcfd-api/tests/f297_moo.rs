//! ADR 0220 X1 to X4 (F-297): NSGA-II with the per-variable exchange of the children's values (`Crossover::Exchange`) on ZDT1
//! with 30 parameters and on FON through `Study`, the Pareto set against the analytic front, the properties of the method, and a
//! replica of the operator. Limits are those registered in ADR 0220 before the code (the C, G and I limits of ADR 0209). The
//! problem and metric code is copied unchanged from `f249_moo.rs`.

use rcfd_api::design::nsga2::{self, Crossover, MoFitness, Options};
use rcfd_api::design::optim::{random_in_box, Stop};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::store::{CondResult, Record, Store};
use rcfd_api::design::study::{
    Condition, Evaluator, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study, StudyDef,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// C, G, I limits of ADR 0209, registered again in ADR 0220.
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

/// Evaluator of `p` that counts the requested points outside the box (X3b) and the calls.
struct Counted {
    p: Problem,
    outside: AtomicUsize,
    calls: AtomicUsize,
}

impl Counted {
    fn new(p: Problem) -> Self {
        Counted {
            p,
            outside: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        }
    }
}

impl Evaluator for Counted {
    fn evaluate(&self, d: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if x.iter().zip(&d.params).any(|(v, q)| *v < q.lo || *v > q.hi) {
            self.outside.fetch_add(1, Ordering::SeqCst);
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

fn with(budget: usize, pop: usize, seed: u64, crossover: Crossover) -> Method {
    Method::Nsga2With {
        budget,
        pop,
        seed,
        crossover,
    }
}

/// A record as bits: index, key, params, and each condition's metrics (`label.name`).
type RecordBits = (usize, String, Vec<u64>, Vec<(String, u64)>);

/// Every record as bits.
fn bits(recs: &[Record]) -> Vec<RecordBits> {
    recs.iter()
        .map(|r| {
            (
                r.index,
                r.key.clone(),
                r.params.iter().map(|(_, v)| v.to_bits()).collect(),
                r.conditions
                    .iter()
                    .flat_map(|c| c.metrics.iter().map(|(k, v)| (format!("{}.{k}", c.label), v.to_bits())))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn x1_exchange_on_zdt1_meets_the_analytic_front() {
    let mut ok = true;
    for seed in 1..=3u64 {
        let t = std::time::Instant::now();
        let (m, n, outside) = run_study(Problem::Zdt1, &with(25_000, 100, seed, Crossover::Exchange));
        println!(
            "X1 seed {seed}: {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}; outside {outside}; {:.1} s",
            m.n,
            m.c,
            m.g,
            m.i,
            t.elapsed().as_secs_f64()
        );
        assert_eq!(outside, 0, "X3b: points outside the box");
        ok &= m.pass();
    }
    assert!(ok, "X1: a seed missed C <= {LIM_C}, G <= {LIM_G} or I <= {LIM_I}");
}

#[test]
fn x1n_sbx_variant_is_the_default_path_and_misses_c() {
    let ev = Counted::new(Problem::Zdt1);
    let mut a = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    a.run(&with(25_000, 100, 1, Crossover::Sbx)).unwrap();
    let mut b = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    b.run(&Method::Nsga2 {
        budget: 25_000,
        pop: 100,
        seed: 1,
    })
    .unwrap();
    let m = metrics(Problem::Zdt1, &pareto_set(&a));
    println!(
        "X1n: {} records with Sbx, {} with Nsga2; {} Pareto members; C {:.3e}, G {:.3e}, I {:.3e}",
        a.store.records().len(),
        b.store.records().len(),
        m.n,
        m.c,
        m.g,
        m.i
    );
    assert_eq!(ev.outside.load(Ordering::SeqCst), 0, "X3b");
    assert!(
        bits(a.store.records()) == bits(b.store.records()),
        "X1n: Nsga2With {{ Sbx }} is not the default path"
    );
    assert!(
        m.c > LIM_C,
        "X1n: the default operator met C ({:.3e}), so C cannot fail here",
        m.c
    );
}

#[test]
fn x2_exchange_on_fon_meets_the_analytic_front() {
    let mut ok = true;
    for seed in 1..=3u64 {
        let (m, n, outside) = run_study(Problem::Fon, &with(10_000, 100, seed, Crossover::Exchange));
        println!(
            "X2 seed {seed}: {} Pareto members of {n} records; C {:.3e}, G {:.3e}, I {:.3e}; outside {outside}",
            m.n, m.c, m.g, m.i
        );
        assert_eq!(outside, 0, "X3b: points outside the box");
        ok &= m.pass();
    }
    assert!(ok, "X2: a seed missed C <= {LIM_C}, G <= {LIM_G} or I <= {LIM_I}");
}

fn small(seed: u64, crossover: Crossover) -> Method {
    with(400, 20, seed, crossover)
}

fn records_of(p: Problem, workers: usize, method: &Method) -> Vec<Record> {
    let ev = Counted::new(p);
    let mut s = Study::new(def(p, workers), Store::memory("mo"), &ev).unwrap();
    s.run(method).unwrap();
    assert_eq!(ev.outside.load(Ordering::SeqCst), 0, "X3b");
    s.store.records().to_vec()
}

#[test]
fn x3a_determinism_workers_and_seeds() {
    let a = records_of(Problem::Zdt1, 1, &small(4, Crossover::Exchange));
    let b = records_of(Problem::Zdt1, 1, &small(4, Crossover::Exchange));
    let w4 = records_of(Problem::Zdt1, 4, &small(4, Crossover::Exchange));
    let c = records_of(Problem::Zdt1, 1, &small(5, Crossover::Exchange));
    let s = records_of(Problem::Zdt1, 1, &small(4, Crossover::Sbx));
    println!(
        "X3a: {} records (seed 4), {} (seed 5), {} (Sbx seed 4)",
        a.len(),
        c.len(),
        s.len()
    );
    assert!(bits(&a) == bits(&b), "X3a: two runs differ");
    assert!(bits(&a) == bits(&w4), "X3a: workers 1 and 4 differ");
    assert!(bits(&a) != bits(&c), "X3a: seeds 4 and 5 give the same records");
    assert!(bits(&a) != bits(&s), "X3a: Exchange and Sbx give the same records");
}

#[test]
fn x3b_budget_in_whole_generations() {
    for budget in [400usize, 410] {
        let mut asked = 0usize;
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
            asked += xs.len();
            Ok(xs
                .iter()
                .map(|x| {
                    assert!(x.iter().all(|v| (0.0..=1.0).contains(v)), "X3b: outside the box");
                    MoFitness::new(zdt1(x).to_vec(), 0.0)
                })
                .collect())
        };
        let r = nsga2::run_with(
            &[(0.0, 1.0); 30],
            &Options {
                max_evals: budget,
                pop: 20,
                seed: 4,
                x0: None,
            },
            Crossover::Exchange,
            &mut ev,
        )
        .unwrap();
        println!(
            "X3b budget {budget}: {} evaluations, {} generations",
            r.evaluations, r.generations
        );
        assert_eq!((r.evaluations, r.generations, asked), (400, 20, 400));
        assert_eq!(r.population.len(), 20);
    }
}

struct NoEval;
impl Evaluator for NoEval {
    fn evaluate(&self, _d: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        panic!("the propose / record loop evaluates nothing itself")
    }
}

#[test]
fn x3c_propose_record_replays_the_evaluator_run() {
    let ev = NoEval;
    let m = small(4, Crossover::Exchange);
    let mut s = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    let mut rounds = 0;
    while let Proposal::Next(points) = s.propose(&m).unwrap() {
        rounds += 1;
        assert!(rounds <= 40);
        for p in points {
            s.record(&p, vec![result(zdt1(&p))], "").unwrap();
        }
    }
    let direct = records_of(Problem::Zdt1, 1, &m);
    let strip = |r: &[Record]| {
        r.iter()
            .map(|r| {
                (
                    r.index,
                    r.params.iter().map(|(_, v)| v.to_bits()).collect::<Vec<_>>(),
                    r.conditions[0]
                        .metrics
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    println!("X3c: {rounds} proposals, {} records", s.store.records().len());
    assert!(strip(s.store.records()) == strip(&direct), "X3c");
}

#[test]
fn x3d_resume_after_interruption() {
    let m = small(4, Crossover::Exchange);
    let ev = Counted::new(Problem::Zdt1);
    let mut full = Study::new(def(Problem::Zdt1, 1), Store::memory("mo"), &ev).unwrap();
    full.run(&m).unwrap();
    let total = ev.calls.swap(0, Ordering::SeqCst);
    assert_eq!(total, full.store.records().len());
    let k = 37usize;
    let dir = std::env::temp_dir().join(format!("rcfd_f297_x3d_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("evaluations.jsonl");
    let mut s = Study::new(def(Problem::Zdt1, 1), Store::open(&path, "mo").unwrap(), &ev).unwrap();
    s.allowance = Some(k);
    assert_eq!(s.run(&m).unwrap_err(), Stop::Interrupted);
    assert_eq!(ev.calls.swap(0, Ordering::SeqCst), k);
    let mut s2 = Study::new(def(Problem::Zdt1, 1), Store::open(&path, "mo").unwrap(), &ev).unwrap();
    s2.run(&m).unwrap();
    let resumed = ev.calls.swap(0, Ordering::SeqCst);
    println!("X3d k = {k}: resume evaluated {resumed} of {total}");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(resumed, total - k);
    assert!(bits(s2.store.records()) == bits(full.store.records()), "X3d");
}

/// ADR 0220 D1 written out again: the crossover coin, per variable `u` (and `w` with the exchange), the two values of ADR 0209,
/// the exchange when `w < 0.5`, then the polynomial mutation of each child (child 1 first) and the clamp. `counts`: exchanges,
/// crossed variables without an exchange, pairs not crossed.
fn replica(
    p1: &[f64],
    p2: &[f64],
    b: &[(f64, f64)],
    exchange: bool,
    rng: &mut Rng,
    counts: &mut [usize; 3],
) -> (Vec<f64>, Vec<f64>) {
    let d = p1.len();
    let (mut c1, mut c2) = (p1.to_vec(), p2.to_vec());
    if rng.uniform() < 0.9 {
        for j in 0..d {
            let u = rng.uniform();
            let beta = if u <= 0.5 {
                (2.0 * u).powf(1.0 / 21.0)
            } else {
                (1.0 / (2.0 * (1.0 - u))).powf(1.0 / 21.0)
            };
            c1[j] = 0.5 * ((1.0 + beta) * p1[j] + (1.0 - beta) * p2[j]);
            c2[j] = 0.5 * ((1.0 - beta) * p1[j] + (1.0 + beta) * p2[j]);
            if exchange {
                if rng.uniform() < 0.5 {
                    std::mem::swap(&mut c1[j], &mut c2[j]);
                    counts[0] += 1;
                } else {
                    counts[1] += 1;
                }
            }
        }
    } else {
        counts[2] += 1;
    }
    for child in [&mut c1, &mut c2] {
        for j in 0..d {
            if rng.uniform() < 1.0 / d as f64 {
                let u = rng.uniform();
                let delta = if u < 0.5 {
                    (2.0 * u).powf(1.0 / 21.0) - 1.0
                } else {
                    1.0 - (2.0 * (1.0 - u)).powf(1.0 / 21.0)
                };
                child[j] += delta * (b[j].1 - b[j].0);
            }
        }
        for (v, &(lo, hi)) in child.iter_mut().zip(b) {
            *v = v.clamp(lo, hi);
        }
    }
    (c1, c2)
}

#[test]
fn x4a_vary_pair_equals_the_replica() {
    let b = [(-1.0, 2.0); 5];
    let mut counts = [0usize; 3];
    let mut sbx_counts = [0usize; 3];
    let as_bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    for s in 1..=20u64 {
        let mut pr = Rng::new(1000 + s);
        let p1 = random_in_box(&b, &mut pr);
        let p2 = random_in_box(&b, &mut pr);
        for (exchange, cross, cnt) in [
            (true, Crossover::Exchange, &mut counts),
            (false, Crossover::Sbx, &mut sbx_counts),
        ] {
            let (a1, a2) = nsga2::vary_pair(&p1, &p2, &b, cross, &mut Rng::new(s));
            let (r1, r2) = replica(&p1, &p2, &b, exchange, &mut Rng::new(s), cnt);
            assert_eq!(as_bits(&a1), as_bits(&r1), "X4a seed {s} {cross:?} child 1");
            assert_eq!(as_bits(&a2), as_bits(&r2), "X4a seed {s} {cross:?} child 2");
        }
    }
    println!(
        "X4a: Exchange: {} exchanges, {} crossed variables kept, {} pairs not crossed; Sbx: {} pairs not crossed",
        counts[0], counts[1], counts[2], sbx_counts[2]
    );
    assert!(
        counts[0] > 0 && counts[1] > 0 && counts[2] > 0,
        "X4a: the 20 seeds do not exercise every branch ({counts:?})"
    );
}

#[test]
fn x4b_run_equals_run_with_sbx() {
    let trace = |use_run: bool| {
        let mut asked: Vec<u64> = Vec::new();
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
            Ok(xs
                .iter()
                .map(|x| {
                    let f = zdt1(x);
                    asked.extend(x.iter().map(|v| v.to_bits()));
                    asked.extend(f.iter().map(|v| v.to_bits()));
                    MoFitness::new(f.to_vec(), 0.0)
                })
                .collect())
        };
        let o = Options {
            max_evals: 400,
            pop: 20,
            seed: 4,
            x0: Some(vec![0.5; 30]),
        };
        let b = [(0.0, 1.0); 30];
        let r = if use_run {
            nsga2::run(&b, &o, &mut ev)
        } else {
            nsga2::run_with(&b, &o, Crossover::Sbx, &mut ev)
        }
        .unwrap();
        let mut out = asked;
        for (x, f) in &r.population {
            out.extend(x.iter().map(|v| v.to_bits()));
            out.extend(f.objectives.iter().map(|v| v.to_bits()));
            out.push(f.violation.to_bits());
        }
        out.extend(r.rank.iter().map(|k| *k as u64));
        out.push(r.evaluations as u64);
        out.push(r.generations as u64);
        out
    };
    let a = trace(true);
    let b = trace(false);
    println!("X4b: {} values compared", a.len());
    assert!(a == b, "X4b: run and run_with(Sbx) differ");
}
