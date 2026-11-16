//! ADR 0662: checks of the seeded re-run sample of the hypothesis evidence and of the evidence-parameter guard (F-727).
//! Synthetic evaluators only; no CFD, no person, no language model.

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{entry_hash, resolve, Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::study::parallel_map;
use rcfd_core::hash::fnv64;
use serde_json::Value;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const K: usize = 13;
const RERUN: &str = "not reproduced by a re-run";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f727_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger_at(p: &Path) -> Ledger {
    let mut l = Ledger::new(p);
    l.clock = || CLOCK;
    l
}

fn entries(l: &Ledger) -> Vec<Entry> {
    let c = l.read().unwrap();
    assert!(c.skipped.is_empty(), "skipped lines: {:?}", c.skipped);
    c.entries
}

/// ADR 0624's deterministic standard normal of a point.
fn noise(world: u64, metric: u64, params: &[Quantity]) -> f64 {
    let mut bytes = Vec::with_capacity(8 * params.len());
    for q in params {
        bytes.extend_from_slice(&q.value.to_bits().to_le_bytes());
    }
    let mut r = Rng::new(fnv64(&bytes) ^ world ^ (metric + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let (u1, u2) = (r.uniform(), r.uniform());
    (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * PI * u2).cos()
}

fn val(params: &[Quantity], name: &str) -> f64 {
    params.iter().find(|q| q.name == name).map_or(0.0, |q| q.value)
}

// ---------------------------------------------------------------- W5 and the impostor

#[derive(Clone)]
struct W5 {
    seed: u64,
    tier: Fidelity,
    version: &'static str,
}

fn w5(seed: u64, tier: Fidelity) -> W5 {
    W5 {
        seed,
        tier,
        version: "1",
    }
}

impl Model for W5 {
    fn id(&self) -> &str {
        "synthetic.w5"
    }
    fn version(&self) -> &str {
        self.version
    }
    fn fidelity(&self) -> Fidelity {
        self.tier
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        Outcome::ok(vec![Quantity::new("y", val(p, "x0") + noise(self.seed, 0, p), "1")])
    }
}

/// Same id, version and tier; `y + 0.3` on the high arm of the corrupted points (or of every point), or `y (1 + rel)` everywhere.
struct Impostor {
    real: W5,
    corrupt: Vec<u64>,
    all_high: bool,
    rel: Option<f64>,
}

impl Model for Impostor {
    fn id(&self) -> &str {
        self.real.id()
    }
    fn version(&self) -> &str {
        self.real.version()
    }
    fn fidelity(&self) -> Fidelity {
        self.real.tier
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let y = self.real.evaluate(p).metrics[0].value;
        let y = if let Some(r) = self.rel {
            y * (1.0 + r)
        } else if val(p, "x0") == 0.75 && (self.all_high || self.corrupt.contains(&val(p, "x1").to_bits())) {
            y + 0.3
        } else {
            y
        };
        Outcome::ok(vec![Quantity::new("y", y, "1")])
    }
}

fn w5_claim(m: &W5, seed: u64, n: usize, sample: Option<RerunSample>) -> Hypothesis {
    let mut h = Hypothesis::new(
        "raising x0 raises y by at least 0.5",
        Factor {
            name: "x0".into(),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: "y".into(),
            unit: "1".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        0.5,
        vec![Bound::new("x1", "1", 0.0, 1.0)],
        n,
        seed,
        Scope::of(m, true),
    );
    h.rerun_sample = sample;
    h
}

fn sample(k: usize) -> Option<RerunSample> {
    Some(RerunSample {
        pairs: k,
        rtol: 0.0,
        atol: 0.0,
    })
}

/// The corrupted pairs of a world: the first 16 of `0..64` shuffled by `Rng::new(world ^ 0x00C0_FFEE)`.
fn corrupted(world: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..64).collect();
    Rng::new(world ^ 0x00C0_FFEE).shuffle(&mut idx);
    idx.truncate(16);
    idx
}

fn result_settings(es: &[Entry]) -> Value {
    es.iter()
        .rev()
        .find(|e| e.kind == KIND_RESULT)
        .map(|e| e.settings.clone())
        .unwrap_or(Value::Null)
}

fn rerun_problems(v: &Verification) -> usize {
    v.problems.iter().filter(|p| p.contains(RERUN)).count()
}

/// The recorded sample of a result line is k, the derived seed and the recipe's pairs.
fn records_sample(es: &[Entry], reg: &Registered, h: &Hypothesis, k: usize) -> bool {
    let s = result_settings(es);
    let r = &s["rerun_sample"];
    let seed = rerun_seed(&reg.hash, h.seed);
    let want: Vec<Value> = rerun_pairs(h.n_pairs, k, seed).into_iter().map(Value::from).collect();
    r["k"].as_u64() == Some(k as u64) && r["seed"].as_u64() == Some(seed) && r["pairs"].as_array() == Some(&want)
}

/// One ledger: register, run with `evaluator`, verify with the real model. Returns (verification, entries, registration).
fn one(
    path: &Path,
    real: &W5,
    h: &Hypothesis,
    evaluator: Option<(&[usize], bool, Option<f64>)>,
) -> (Verification, Vec<Entry>, Registered) {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let reg = register(&l, h).unwrap();
    match evaluator {
        Some((bad, all_high, rel)) => {
            let pts = design_points(h, reg.derived_seed);
            let imp = Impostor {
                real: real.clone(),
                corrupt: bad.iter().map(|&p| pts[p][0].to_bits()).collect(),
                all_high,
                rel,
            };
            run(&l, &reg.id, &imp, &RunOptions::default()).unwrap();
        }
        None => {
            run(&l, &reg.id, real, &RunOptions::default()).unwrap();
        }
    }
    let es = entries(&l);
    let v = verify(&es, &reg.id, Some(real));
    (v, es, reg)
}

// ---------------------------------------------------------------- K0, K1

fn binom_u128(n: u128, k: u128) -> u128 {
    let mut c: u128 = 1;
    for i in 0..k {
        c = c * (n - i) / (i + 1);
    }
    c
}

#[test]
fn k0_sample_size() {
    assert_eq!(rerun_sample_size(64, 16, 0.02), Some(13));
    assert_eq!(rerun_sample_size(64, 6, 0.02), Some(30));
    assert_eq!(rerun_sample_size(64, 16, 0.05), Some(10));
    assert_eq!(rerun_sample_size(64, 0, 0.02), None);
    assert_eq!(rerun_sample_size(64, 16, 1.0), None);
    let p = rerun_miss_probability(64, 16, 13);
    let want = binom_u128(48, 13) as f64 / binom_u128(64, 13) as f64;
    let rel = ((p - want) / want).abs();
    eprintln!("K0: k 13, 30, 10; P_miss(13) {p:.6e} against {want:.6e}, relative {rel:.2e}");
    assert!(rel <= 1e-12);
}

fn draw_by_hand(n: usize, k: usize, seed: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut r = Rng::new(seed);
    for i in 0..k {
        let j = i + r.below(n - i);
        idx.swap(i, j);
    }
    let mut s: Vec<usize> = idx[..k].to_vec();
    s.sort();
    s
}

#[test]
fn k1_sample_recipe() {
    let mut equal = 0;
    for seed in 0..100u64 {
        for (n, k) in [(64, 1), (64, 13), (64, 64), (16, 5)] {
            let s = rerun_pairs(n, k, seed);
            let distinct = s.windows(2).all(|w| w[0] < w[1]);
            let ok = s == draw_by_hand(n, k, seed)
                && s.len() == k
                && distinct
                && s.iter().all(|&p| p < n)
                && (k != n || s == (0..n).collect::<Vec<_>>());
            equal += usize::from(ok);
        }
    }
    eprintln!("K1: {equal} of 400 equal to the recipe");
    assert_eq!(equal, 400);
}

// ---------------------------------------------------------------- A0, A4, A5, A6

#[test]
fn a0_a6_sample_verify() {
    let dir = tmp("a0");
    let real = w5(727_990, Fidelity::Cfd);
    let h = w5_claim(&real, 727_990, 64, sample(K));
    // (i) every high arm corrupted
    let (v, es, reg) = one(&dir.join("i.jsonl"), &real, &h, Some((&[][..], true, None)));
    let plan = v.rerun_sample.clone().expect("plan");
    let on_high_sampled = v
        .problems
        .iter()
        .filter(|p| p.contains(RERUN))
        .filter(|p| {
            let k: usize = p
                .trim_start_matches("entry ")
                .split(':')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let pr = &es[k].provenance;
            pr["arm"] == "high" && plan.pairs.contains(&(pr["pair"].as_u64().unwrap() as usize))
        })
        .count();
    let q = LessonQuery {
        current: vec![("synthetic.w5".into(), "1".into())],
        ..Default::default()
    };
    let rows_i = lessons(&es, &q, &[&real]).rows.len();
    eprintln!(
        "A0 (i): {} re-run problems, {on_high_sampled} on sampled high arms; evidence_rerun {:?}; lessons {rows_i} rows; sample {:?}",
        rerun_problems(&v),
        v.evidence_rerun,
        plan.pairs
    );
    assert_eq!(rerun_problems(&v), K);
    assert_eq!(v.problems.len(), K);
    assert_eq!(on_high_sampled, K);
    assert_eq!(v.evidence_rerun, Some(false));
    assert_eq!(rows_i, 0);
    assert!(records_sample(&es, &reg, &h, K), "A0 (i) result records the sample");
    // (ii) the real model
    let path2 = dir.join("ii.jsonl");
    let (v2, es2, reg2) = one(&path2, &real, &h, None);
    let rows_ii = lessons(&es2, &q, &[&real]).rows.len();
    eprintln!(
        "A0 (ii): {} problems, evidence_rerun {:?}, genuine {:?}, lessons {rows_ii} rows",
        v2.problems.len(),
        v2.evidence_rerun,
        v2.genuine
    );
    assert!(v2.problems.is_empty());
    assert_eq!(v2.evidence_rerun, Some(true));
    assert!(v2.genuine.is_some());
    assert_eq!(rows_ii, 1);
    // (iii) no model
    let v3 = verify(&es2, "H-1", None);
    assert!(v3.problems.is_empty());
    assert_eq!(v3.evidence_rerun, None);
    assert_eq!(v3.rerun_sample, v2.rerun_sample);
    // (iv) the result line records k, the seed and the pairs of the verification
    let s = result_settings(&es2);
    let p2 = v2.rerun_sample.clone().unwrap();
    let want: Vec<Value> = p2.pairs.iter().map(|&x| Value::from(x)).collect();
    eprintln!("A0 (iv): result settings.rerun_sample = {}", s["rerun_sample"]);
    assert_eq!(s["rerun_sample"]["k"].as_u64(), Some(K as u64));
    assert_eq!(s["rerun_sample"]["seed"].as_u64(), Some(rerun_seed(&reg2.hash, h.seed)));
    assert_eq!(s["rerun_sample"]["pairs"].as_array(), Some(&want));
    assert_eq!(p2.seed, rerun_seed(&reg2.hash, h.seed));
    // A6: a model of another version
    let other = W5 {
        version: "2",
        ..real.clone()
    };
    let v6 = verify(&es2, "H-1", Some(&other));
    eprintln!(
        "A6: {} problems, evidence_rerun {:?}, sample reported {}",
        v6.problems.len(),
        v6.evidence_rerun,
        v6.rerun_sample.is_some()
    );
    assert!(v6.problems.is_empty());
    assert_eq!(v6.evidence_rerun, None);
    assert!(v6.rerun_sample.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a4_tolerances() {
    let dir = tmp("a4");
    let real = w5(727_991, Fidelity::Cfd);
    let tol = |rtol: f64| {
        Some(RerunSample {
            pairs: 4,
            rtol,
            atol: 0.0,
        })
    };
    let h1 = w5_claim(&real, 727_991, 64, tol(1e-9));
    let (v1, _, _) = one(&dir.join("t1.jsonl"), &real, &h1, Some((&[][..], false, Some(1e-10))));
    let h0 = w5_claim(&real, 727_991, 64, tol(0.0));
    let (v0, _, _) = one(&dir.join("t0.jsonl"), &real, &h0, Some((&[][..], false, Some(1e-10))));
    eprintln!(
        "A4: rtol 1e-9: {} problems {:?}; rtol 0: {} problems {:?}",
        v1.problems.len(),
        v1.evidence_rerun,
        rerun_problems(&v0),
        v0.evidence_rerun
    );
    assert!(v1.problems.is_empty());
    assert_eq!(v1.evidence_rerun, Some(true));
    assert_eq!(rerun_problems(&v0), 8);
    assert_eq!(v0.evidence_rerun, Some(false));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a5_refusals() {
    let dir = tmp("a5");
    let path = dir.join("l.jsonl");
    let real = w5(727_992, Fidelity::Cfd);
    let l = ledger_at(&path);
    register(&l, &w5_claim(&real, 727_992, 64, sample(K))).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut cases: Vec<(&str, Hypothesis)> = Vec::new();
    cases.push(("pairs 0", w5_claim(&real, 1, 64, sample(0))));
    cases.push(("pairs 65", w5_claim(&real, 2, 64, sample(65))));
    let mut c = w5_claim(&real, 3, 64, sample(K));
    c.rerun_sample.as_mut().unwrap().rtol = f64::NAN;
    cases.push(("rtol NaN", c));
    let mut c = w5_claim(&real, 4, 64, sample(K));
    c.rerun_sample.as_mut().unwrap().atol = -1.0;
    cases.push(("atol -1", c));
    let mut c = w5_claim(&real, 5, 64, sample(K));
    c.evaluator.tier = Fidelity::Measured;
    cases.push(("measured", c));
    let mut c = w5_claim(&real, 6, 64, sample(K));
    c.evaluator.deterministic = false;
    cases.push(("non-deterministic", c));
    let mut refused = 0;
    for (tag, h) in &cases {
        let r = register(&l, h);
        eprintln!("A5 '{tag}': {r:?}", r = r.as_ref().map(|x| &x.id));
        if r.as_ref().err().is_some_and(|e| e.contains("rerun_sample")) {
            refused += 1;
        }
    }
    let after = std::fs::read(&path).unwrap();
    assert_eq!(refused, 6, "A5: 6 of 6");
    assert_eq!(before, after, "A5: file unchanged");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- A worlds

#[derive(Debug, Default, Clone)]
struct World {
    caught13: bool,
    exact13: bool,
    honest_problems: usize,
    honest_rerun_ok: bool,
    records: usize,
    honest_pairs: Vec<usize>,
    nc6_caught: bool,
    nc6_none: bool,
    nc7_caught: bool,
}

fn world_run(w: u64, dir: &Path) -> World {
    let seed = 727_000 + w;
    let real = w5(seed, Fidelity::Cfd);
    let bad = corrupted(seed);
    let mut out = World::default();
    // A1: impostor, k = 13
    let h = w5_claim(&real, seed, 64, sample(K));
    let (v, es, reg) = one(
        &dir.join(format!("a{w}.jsonl")),
        &real,
        &h,
        Some((&bad[..], false, None)),
    );
    let plan = v.rerun_sample.clone().unwrap();
    let inter = plan.pairs.iter().filter(|p| bad.contains(p)).count();
    out.caught13 = rerun_problems(&v) > 0;
    out.exact13 = rerun_problems(&v) == inter && v.evidence_rerun == Some(inter == 0);
    out.records += usize::from(records_sample(&es, &reg, &h, K));
    // A2: the real model, k = 13
    let (v2, es2, reg2) = one(&dir.join(format!("h{w}.jsonl")), &real, &h, None);
    out.honest_problems = v2.problems.len();
    out.honest_rerun_ok = v2.evidence_rerun == Some(true);
    out.records += usize::from(records_sample(&es2, &reg2, &h, K));
    out.honest_pairs = v2.rerun_sample.unwrap().pairs;
    // NC6: no sample
    let h6 = w5_claim(&real, seed, 64, None);
    let (v6, _, _) = one(
        &dir.join(format!("n{w}.jsonl")),
        &real,
        &h6,
        Some((&bad[..], false, None)),
    );
    out.nc6_caught = rerun_problems(&v6) > 0;
    out.nc6_none = v6.evidence_rerun.is_none();
    // NC7: k = 1
    let h7 = w5_claim(&real, seed, 64, sample(1));
    let (v7, _, _) = one(
        &dir.join(format!("o{w}.jsonl")),
        &real,
        &h7,
        Some((&bad[..], false, None)),
    );
    out.nc7_caught = rerun_problems(&v7) > 0;
    for t in ["a", "h", "n", "o"] {
        let _ = std::fs::remove_file(dir.join(format!("{t}{w}.jsonl")));
    }
    out
}

fn worlds(ws: &[u64], tag: &str) -> Vec<World> {
    let dir = tmp(tag);
    let out = parallel_map(ws, 4, &|&w: &u64| world_run(w, &dir));
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn summary(st: &[World]) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    (
        st.iter().filter(|s| s.caught13).count(),
        st.iter().filter(|s| s.exact13).count(),
        st.iter().filter(|s| s.honest_problems > 0).count(),
        st.iter().filter(|s| s.honest_rerun_ok).count(),
        st.iter().map(|s| s.records).sum(),
        st.iter().filter(|s| s.nc6_caught).count(),
        st.iter().filter(|s| s.nc6_none).count(),
        st.iter().filter(|s| s.nc7_caught).count(),
    )
}

#[test]
#[ignore = "heavy registered record (ADR 0662 A1 to A3, NC6, NC7): 400 worlds, run once"]
fn a1_a3_nc6_nc7_worlds_heavy() {
    let ws: Vec<u64> = (0..400).collect();
    let st = worlds(&ws, "aw");
    let (caught, exact, hp, hok, rec, n6, n6none, n7) = summary(&st);
    let mut counts = [0usize; 64];
    for s in &st {
        for &p in &s.honest_pairs {
            counts[p] += 1;
        }
    }
    let (cmin, cmax) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
    eprintln!(
        "A1: caught {caught} of 400 (limit >= 380); A1b exact {exact} of 400; A2 honest with a problem {hp}, re-run ok {hok}; \
         A3 records {rec} of 800, pair counts {cmin}..{cmax} (limit 50..113); NC6 caught {n6}, None {n6none}; NC7 caught {n7} (66..134)"
    );
    assert!(caught >= 380, "A1");
    assert_eq!(exact, 400, "A1b");
    assert_eq!(hp, 0, "A2");
    assert_eq!(hok, 400, "A2");
    assert_eq!(rec, 800, "A3");
    assert!(cmin >= 50 && cmax <= 113, "A3 uniformity");
    assert_eq!(n6, 0, "NC6");
    assert_eq!(n6none, 400, "NC6");
    assert!((66..=134).contains(&n7), "NC7");
}

/// Fast counterpart: worlds 0 to 15, the assertions that hold world by world.
#[test]
fn a_worlds_fast() {
    let ws: Vec<u64> = (0..16).collect();
    let st = worlds(&ws, "awf");
    let (caught, exact, hp, hok, rec, n6, n6none, n7) = summary(&st);
    eprintln!(
        "A fast (16 worlds): caught {caught}, exact {exact}, honest problems {hp}, re-run ok {hok}, records {rec}, NC6 {n6} / None {n6none}, NC7 {n7}"
    );
    assert_eq!(exact, 16);
    assert_eq!(hp, 0);
    assert_eq!(hok, 16);
    assert_eq!(rec, 32);
    assert_eq!(n6, 0);
    assert_eq!(n6none, 16);
}

// ---------------------------------------------------------------- B1, NCB

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn write_lines(path: &Path, ls: &[String]) {
    std::fs::write(path, ls.iter().map(|s| format!("{s}\n")).collect::<String>()).unwrap();
}

#[derive(Clone)]
struct W3b;

impl Model for W3b {
    fn id(&self) -> &str {
        "synthetic.w3b"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let (s, n) = (val(p, "s"), val(p, "n"));
        let q = n * (1.0 + 0.5 * s);
        Outcome::ok(vec![
            Quantity::new("q", q, "m3/s"),
            Quantity::new("tone", q * q * (1.0 - 0.1 * s), "Pa"),
        ])
    }
}

fn w3b_claim() -> Hypothesis {
    let mut h = Hypothesis::new(
        "at equal flow, raising s lowers the tone",
        Factor {
            name: "s".into(),
            unit: "1".into(),
            low: 0.0,
            high: 1.0,
        },
        Response {
            name: "tone".into(),
            unit: "Pa".into(),
            resolution: 0.0,
        },
        Direction::Decreases,
        0.01,
        vec![Bound::new("c", "1", 0.0, 1.0)],
        16,
        727_503,
        Scope::of(&W3b, true),
    );
    h.held = vec![Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target: HeldTarget::AsLowArm { at: 0.8 },
        tolerance: 1e-13,
        by: "n".into(),
        by_unit: "1/s".into(),
        bracket: (0.1, 10.0),
    }];
    h
}

/// (tag, base ledger, entry, parameter index, new value, the expected text, the model).
type Case<'a> = (&'a str, &'a Path, usize, usize, f64, String, &'a dyn Model);

/// Edit parameter `pi` of entry `k` to `value`; with `rehash`, rewrite the result's parent hash of that entry.
fn edit(src: &Path, dst: &Path, k: usize, pi: usize, value: f64, rehash: bool) {
    let mut ls = lines(src);
    let old = entry_hash(&entries(&ledger_at(src))[k]);
    let mut v: Value = serde_json::from_str(&ls[k]).unwrap();
    v["params"][pi]["value"] = Value::from(value);
    ls[k] = v.to_string();
    write_lines(dst, &ls);
    if rehash {
        let new = entry_hash(&entries(&ledger_at(dst))[k]);
        let last = ls.len() - 1;
        assert!(ls[last].contains(&old));
        ls[last] = ls[last].replace(&old, &new);
        write_lines(dst, &ls);
    }
}

#[test]
fn b1_ncb_parameter_guard() {
    let dir = tmp("b1");
    let real = w5(727_501, Fidelity::ReducedOrder);
    let base5 = dir.join("w5.jsonl");
    {
        let l = ledger_at(&base5);
        let r = register(&l, &w5_claim(&real, 727_501, 16, None)).unwrap();
        run(&l, &r.id, &real, &RunOptions::default()).unwrap();
    }
    let base3 = dir.join("w3b.jsonl");
    {
        let l = ledger_at(&base3);
        let r = register(&l, &w3b_claim()).unwrap();
        run(&l, &r.id, &W3b, &RunOptions::default()).unwrap();
        assert!(verify(&entries(&l), "H-1", None).problems.is_empty());
    }
    let x1_p5 = entries(&ledger_at(&base5))[11].params[1].value;
    assert_eq!(entries(&ledger_at(&base3))[7].params[2].value, 0.8);
    // (tag, base, entry, parameter index, new value, the expected text, the model)
    let cases: Vec<Case> = vec![
        (
            "a",
            base5.as_path(),
            11,
            1,
            x1_p5 + 1e-3,
            "entry 11: evidence of H-1 pair 5 arm low: parameter 'x1'".into(),
            &real as &dyn Model,
        ),
        (
            "b",
            base5.as_path(),
            20,
            0,
            0.7501,
            "entry 20: evidence of H-1 pair 9 arm high: parameter 'x0'".into(),
            &real as &dyn Model,
        ),
        (
            "c",
            base3.as_path(),
            7,
            2,
            0.81,
            "entry 7: evidence of H-1 pair 3 arm low: compensating parameter 'n'".into(),
            &W3b,
        ),
    ];
    let mut ok = 0;
    for (tag, base, k, pi, value, want, model) in &cases {
        let p = dir.join(format!("{tag}.jsonl"));
        edit(base, &p, *k, *pi, *value, true);
        let l = ledger_at(&p);
        let es = entries(&l);
        let res = es.last().unwrap();
        let resolves = res.parents.iter().all(|r| resolve(&es, r).is_ok());
        let v = verify(&es, "H-1", None);
        let tail = if *tag == "c" {
            "outside the registered rule"
        } else {
            "the design has"
        };
        let one_problem =
            v.problems.len() == 1 && v.problems[0].contains(want.as_str()) && v.problems[0].contains(tail);
        let r = run(&l, "H-1", *model, &RunOptions::default());
        let run_refused = r.as_ref().err().is_some_and(|e| e.contains(want.as_str()));
        let q = LessonQuery {
            current: vec![(model.id().to_string(), "1".to_string())],
            ..Default::default()
        };
        let rows = lessons(&es, &q, &[]).rows.len();
        eprintln!(
            "B1 ({tag}): parents resolve {resolves}; problems {:?}; run refused {run_refused}; lessons {rows} rows",
            v.problems
        );
        if resolves && one_problem && run_refused && rows == 0 {
            ok += 1;
        }
    }
    // NCB: (a) without the hash rewrite
    let p = dir.join("ncb.jsonl");
    edit(&base5, &p, 11, 1, x1_p5 + 1e-3, false);
    let es = entries(&ledger_at(&p));
    let res = es.last().unwrap();
    let unresolved = res.parents.iter().filter(|r| resolve(&es, r).is_err()).count();
    let v = verify(&es, "H-1", None);
    let param = v.problems.iter().any(|x| x.contains("parameter 'x1'"));
    eprintln!(
        "NCB: {unresolved} parent reference(s) do not resolve; result flagged {:?}; parameter problem {param}",
        v.flagged
    );
    assert_eq!(ok, 3, "B1: 3 of 3");
    assert_eq!(unresolved, 1);
    assert!(!v.flagged.is_empty() && param, "NCB");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- C2

#[test]
fn c2_old_records_bit_identical() {
    let dir = tmp("c2");
    let path = dir.join("l.jsonl");
    let m = w5(727_900, Fidelity::ReducedOrder);
    let h = w5_claim(&m, 727_900, 16, None);
    {
        let l = ledger_at(&path);
        let r = register(&l, &h).unwrap();
        run(&l, &r.id, &m, &RunOptions::default()).unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    let es = entries(&ledger_at(&path));
    let v = verify(&es, "H-1", None);
    // ADR 0624's G1 record
    struct W1b;
    impl Model for W1b {
        fn id(&self) -> &str {
            "synthetic.w1b"
        }
        fn version(&self) -> &str {
            "1"
        }
        fn fidelity(&self) -> Fidelity {
            Fidelity::ReducedOrder
        }
        fn evaluate(&self, _: &[Quantity]) -> Outcome {
            Outcome::ok(Vec::new())
        }
    }
    let mut g1 = Hypothesis::new(
        "raising x0 raises y by at least 0.5",
        Factor {
            name: "x0".into(),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: "y".into(),
            unit: "1".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        0.5,
        vec![Bound::new("x1", "1", 0.0, 1.0)],
        64,
        701_710,
        Scope::of(&W1b, false),
    );
    g1.predicted_size = Some((0.4, 0.6));
    g1.held = vec![Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target: HeldTarget::AsLowArm { at: 0.8 },
        tolerance: 1e-9,
        by: "n".into(),
        by_unit: "1/s".into(),
        bracket: (0.1, 10.0),
    }];
    g1.statistic = Statistic::Sign;
    eprintln!(
        "C2: {} entries, {} bytes, fnv64 {:016x}, record {}, registration {}, result {}; G1 {}; verify problems {}, genuine {:?}",
        es.len(),
        bytes.len(),
        fnv64(&bytes),
        h.hash(),
        entry_hash(&es[0]),
        entry_hash(es.last().unwrap()),
        g1.hash(),
        v.problems.len(),
        v.genuine
    );
    assert_eq!(es.len(), 34);
    assert_eq!(bytes.len(), 26298);
    assert_eq!(fnv64(&bytes), 0xef3a_dd9a_4349_18c6);
    assert_eq!(h.hash(), "fnv1a64:e2e79db55c67eb38");
    assert_eq!(entry_hash(&es[0]), "fnv1a64:9dd45ad925cd2ce8");
    assert_eq!(entry_hash(es.last().unwrap()), "fnv1a64:46c82e4c69b03471");
    assert_eq!(g1.hash(), "fnv1a64:4b58385209778c10");
    assert!(h.to_json().get("rerun_sample").is_none());
    assert!(v.problems.is_empty() && v.genuine == Some(33));
    let _ = std::fs::remove_dir_all(&dir);
}
