//! ADR 0665: checks of the coupled held metrics (flow and speed solved together) and of the held-tolerance floor of a
//! hypothesis (F-728). Synthetic evaluators with closed forms only; no CFD, no person, no language model.

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{entry_hash, resolve, Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_core::hash::fnv64;
use serde_json::Value;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const TOL_Q: f64 = 1e-6;
const TOL_N: f64 = 1e-5;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f728_{tag}_{}", std::process::id()));
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

fn val(params: &[Quantity], name: &str) -> f64 {
    params.iter().find(|q| q.name == name).map_or(f64::NAN, |q| q.value)
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

fn read_bytes(p: &Path) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

// ---------------------------------------------------------------- WC, flow and speed

#[derive(Clone)]
struct Wc {
    beta: f64,
    gamma: f64,
    tier: Fidelity,
}

const WC: Wc = Wc {
    beta: 1.0,
    gamma: 0.0,
    tier: Fidelity::Analytic,
};

impl Model for Wc {
    fn id(&self) -> &str {
        "synthetic.wc"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        self.tier
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let (s, c, a, b) = (val(p, "s"), val(p, "c"), val(p, "a"), val(p, "b"));
        let (aa, bb, cc, gg) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + self.beta * s, 1.0 + self.gamma * c);
        let n = a * bb / (gg * (1.0 + 0.2 * b));
        let q = b * n * aa;
        let y = a * b * cc;
        Outcome::ok(vec![
            Quantity::new("q", q, "m3/s"),
            Quantity::new("n", n, "1/s"),
            Quantity::new("power", y, "W"),
        ])
    }
}

/// The same model under another id (J6).
struct OtherId;

impl Model for OtherId {
    fn id(&self) -> &str {
        "synthetic.other"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        WC.evaluate(p)
    }
}

struct Cf {
    a: f64,
    b: f64,
    y: f64,
}

/// Closed-form joint solution for targets (q, n) at s, c.
fn closed(m: &Wc, s: f64, c: f64, q: f64, n: f64) -> Cf {
    let (aa, bb, cc, gg) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + m.beta * s, 1.0 + m.gamma * c);
    let b = q / (n * aa);
    let a = n * gg * (1.0 + 0.2 * b) / bb;
    Cf { a, b, y: a * b * cc }
}

/// Closed-form sensitivities (S_q, S_n) of the power along the hold.
fn sens(m: &Wc, s: f64, c: f64, q: f64, n: f64) -> (f64, f64) {
    let (aa, bb, cc, gg) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + m.beta * s, 1.0 + m.gamma * c);
    let k = gg * cc / bb;
    (
        k * (1.0 / aa + 0.4 * q / (n * aa * aa)),
        -k * 0.2 * q * q / (n * n * aa * aa),
    )
}

/// First-order bounds of (b, a) from the tolerances.
fn par_bounds(m: &Wc, s: f64, c: f64, q: f64, n: f64) -> (f64, f64) {
    let (aa, bb, gg) = (1.0 + 0.5 * s, 1.0 + 0.3 * s, 1.0 + m.gamma * c);
    let (dbq, dbn) = (1.0 / (n * aa), -q / (n * n * aa));
    let (daq, dan) = (0.2 * gg / (aa * bb), gg / bb);
    (
        dbq.abs() * TOL_Q + dbn.abs() * TOL_N,
        daq.abs() * TOL_Q + dan.abs() * TOL_N,
    )
}

fn phi(m: &Wc, s: f64, c: f64, q: f64, n: f64) -> f64 {
    let (sq, sn) = sens(m, s, c, q, n);
    sq.abs() * TOL_Q + sn.abs() * TOL_N
}

fn hold_q(target: HeldTarget) -> Held {
    Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target,
        tolerance: TOL_Q,
        by: "b".into(),
        by_unit: "1".into(),
        bracket: (0.001, 0.1),
    }
}

fn hold_n(target: HeldTarget, bracket: (f64, f64)) -> Held {
    Held {
        metric: "n".into(),
        unit: "1/s".into(),
        target,
        tolerance: TOL_N,
        by: "a".into(),
        by_unit: "V".into(),
        bracket,
    }
}

fn both() -> Vec<Held> {
    vec![
        hold_q(HeldTarget::Value(1.0)),
        hold_n(HeldTarget::Value(50.0), (10.0, 100.0)),
    ]
}

fn wc_claim(m: &Wc, seed: u64, min_effect: f64, held: Vec<Held>, domain: Vec<Bound>) -> Hypothesis {
    let mut h = Hypothesis::new(
        "raising s raises the power at equal flow and speed",
        Factor {
            name: "s".into(),
            unit: "1".into(),
            low: 0.0,
            high: 1.0,
        },
        Response {
            name: "power".into(),
            unit: "W".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        min_effect,
        domain,
        32,
        seed,
        Scope::of(m, true),
    );
    h.held = held;
    h
}

fn dom_c() -> Vec<Bound> {
    vec![Bound::new("c", "1", 0.0, 1.0)]
}

fn evidence(es: &[Entry]) -> Vec<&Entry> {
    es.iter().filter(|e| e.kind == "evaluation").collect()
}

fn metric(e: &Entry, name: &str) -> f64 {
    e.metric(name).map_or(f64::NAN, |q| q.value)
}

/// The closed-form floor of WC with value targets (1, 50) at the domain midpoint c = 0.5.
fn floor_closed(m: &Wc) -> f64 {
    phi(m, 0.0, 0.5, 1.0, 50.0) + phi(m, 1.0, 0.5, 1.0, 50.0)
}

/// d* = y*(s = 1) - y*(s = 0) at (1, 50), c ignored (gamma 0).
fn d_star() -> f64 {
    closed(&WC, 1.0, 0.0, 1.0, 50.0).y - closed(&WC, 0.0, 0.0, 1.0, 50.0).y
}

fn run_wc(path: &Path, m: &Wc, h: &Hypothesis, workers: usize) -> Verification {
    let l = ledger_at(path);
    let r = register_with_floor(&l, h, m).unwrap();
    let rep = run(
        &l,
        &r.id,
        m,
        &RunOptions {
            workers,
            ..RunOptions::default()
        },
    )
    .unwrap();
    rep.verification
}

#[test]
fn j1_j2_j8_joint_solve_and_floor() {
    let dir = tmp("j1");
    let path = dir.join("l.jsonl");
    let h = wc_claim(&WC, 728_001, 0.01, both(), dom_c());
    let v = run_wc(&path, &WC, &h, 1);
    let es = entries(&ledger_at(&path));
    let ev = evidence(&es);
    // J1 (a) to (c)
    let (mut ok_a, mut ok_b, mut worst_b, mut worst_a, mut max_it) = (0, 0, 0.0f64, 0.0f64, 0);
    for e in &ev {
        let s = val(&e.params, "s");
        let c = val(&e.params, "c");
        let (q, n) = (metric(e, "q"), metric(e, "n"));
        if e.verdict == rcfd_api::design::ledger::Verdict::Ok && (q - 1.0).abs() <= TOL_Q && (n - 50.0).abs() <= TOL_N {
            ok_a += 1;
        }
        let cf = closed(&WC, s, c, 1.0, 50.0);
        let (bb, ba) = par_bounds(&WC, s, c, 1.0, 50.0);
        let (eb, ea) = ((val(&e.params, "b") - cf.b).abs(), (val(&e.params, "a") - cf.a).abs());
        worst_b = worst_b.max(eb / bb);
        worst_a = worst_a.max(ea / ba);
        if eb <= 2.0 * bb && ea <= 2.0 * ba {
            ok_b += 1;
        }
        max_it = max_it.max(e.provenance["iterations"].as_u64().unwrap());
    }
    let res = v.result.clone().unwrap();
    let ds = d_star();
    let fc = floor_closed(&WC);
    let err = (res.stats.mean - ds).abs();
    eprintln!(
        "J1: {} evidence; (a) {ok_a} matched; (b) {ok_b} within 2x bound (largest error / bound: b {worst_b:.3e}, a \
         {worst_a:.3e}); (c) largest evaluations per arm {max_it}; (d) mean {:.15} d* {ds:.15} error {err:.3e} (limit F \
         {fc:.6e}); {}; problems {:?}; genuine {:?}",
        ev.len(),
        res.stats.mean,
        res.finding.as_str(),
        v.problems,
        v.genuine
    );
    assert_eq!(ev.len(), 64);
    assert_eq!(ok_a, 64, "J1 (a)");
    assert_eq!(ok_b, 64, "J1 (b)");
    assert!(max_it <= 40, "J1 (c)");
    assert!(err <= fc, "J1 (d)");
    assert_eq!(res.finding, Finding::Confirmed);
    assert!(v.problems.is_empty() && v.genuine.is_some());
    // J2: the stored floor and held_floor against the closed form
    let st = &es[0].settings["held_floor"];
    let stored = st["value"].as_f64().unwrap();
    let f = held_floor(&h, &WC).unwrap();
    let r2 = rel(stored, fc);
    let mut s_ok = 0;
    let mut by_ok = 0;
    for (arm, s) in f.arms.iter().zip([0.0, 1.0]) {
        let (sq, sn) = sens(&WC, s, 0.5, 1.0, 50.0);
        let scale = sq.abs().max(sn.abs());
        let es_ = [
            (arm.sensitivity[0] - sq).abs() / scale,
            (arm.sensitivity[1] - sn).abs() / scale,
        ];
        eprintln!(
            "J2: arm {:?} S = {:?} closed ({sq:.12e}, {sn:.12e}); errors / max|S| {:.2e} {:.2e}; bound {:.6e} closed {:.6e}",
            arm.arm,
            arm.sensitivity,
            es_[0],
            es_[1],
            arm.bound,
            phi(&WC, s, 0.5, 1.0, 50.0)
        );
        s_ok += es_.iter().filter(|e| **e <= 1e-6).count();
        let cf = closed(&WC, s, 0.5, 1.0, 50.0);
        let (bb, ba) = par_bounds(&WC, s, 0.5, 1.0, 50.0);
        if (arm.by[0] - cf.b).abs() <= 2.0 * bb && (arm.by[1] - cf.a).abs() <= 2.0 * ba {
            by_ok += 1;
        }
    }
    eprintln!(
        "J2: floor stored {stored:.12e}, held_floor {:.12e}, closed {fc:.12e}, relative {r2:.2e}; {} arms; {s_ok} of 4 \
         sensitivities; by {by_ok} of 2",
        f.value,
        f.arms.len()
    );
    assert_eq!(stored.to_bits(), f.value.to_bits());
    assert!(r2 <= 1e-6, "J2 floor");
    assert_eq!(f.arms.len(), 2);
    assert_eq!(s_ok, 4, "J2 sensitivities");
    assert_eq!(by_ok, 2, "J2 by");
    // J8 (workers): the J1 ledger with workers 8
    let path8 = dir.join("w8.jsonl");
    run_wc(&path8, &WC, &h, 8);
    let (b1, b8) = (std::fs::read(&path).unwrap(), std::fs::read(&path8).unwrap());
    eprintln!(
        "J8: workers 1 {} bytes, workers 8 {} bytes, equal {}",
        b1.len(),
        b8.len(),
        b1 == b8
    );
    assert_eq!(b1, b8, "J8 workers");
    // J4 (d) and (e) on copies of the completed J1 ledger
    let ls = lines(&path);
    // (d) 'a' of pair 3 low arm (entry 7) set to 150
    let mut ld = ls.clone();
    let mut e7: Value = serde_json::from_str(&ld[7]).unwrap();
    assert_eq!(e7["params"][3]["name"], "a");
    e7["params"][3]["value"] = Value::from(150.0);
    ld[7] = e7.to_string();
    let pd = dir.join("d.jsonl");
    write_lines(&pd, &ld);
    let vd = verify(&entries(&ledger_at(&pd)), "H-1", None);
    let d_hit = vd.problems.iter().any(|p| p.contains("compensating parameter 'a'"));
    eprintln!("J4 (d): problems {:?}", vd.problems);
    // (e) metric 'n' of pair 3 high arm (entry 8) set to 50.001, the result's parent hash rewritten
    let mut le = ls.clone();
    let old = entry_hash(&es[8]);
    let mut e8: Value = serde_json::from_str(&le[8]).unwrap();
    let mi = e8["metrics"]
        .as_array()
        .unwrap()
        .iter()
        .position(|q| q["name"] == "n")
        .unwrap();
    e8["metrics"][mi]["value"] = Value::from(50.001);
    le[8] = e8.to_string();
    let pe = dir.join("e.jsonl");
    write_lines(&pe, &le);
    let new = entry_hash(&entries(&ledger_at(&pe))[8]);
    let last = le.len() - 1;
    assert!(le[last].contains(&old));
    le[last] = le[last].replace(&old, &new);
    write_lines(&pe, &le);
    let ese = entries(&ledger_at(&pe));
    let resolves = ese.last().unwrap().parents.iter().all(|r| resolve(&ese, r).is_ok());
    let ve = verify(&ese, "H-1", None);
    let why = ve
        .result
        .as_ref()
        .and_then(|r| r.void_reason.clone())
        .unwrap_or_default();
    let void = ve.result.as_ref().map(|r| r.finding) == Some(Finding::Void);
    let e_hit = resolves
        && void
        && why.contains("pair 3 arm high")
        && why.contains("'n'")
        && ve.genuine.is_none()
        && !ve.flagged.is_empty();
    eprintln!(
        "J4 (e): parents resolve {resolves}; void {void} ({why}); genuine {:?}; flagged {:?}; problems {:?}",
        ve.genuine, ve.flagged, ve.problems
    );
    assert!(d_hit, "J4 (d)");
    assert!(e_hit, "J4 (e)");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ncj1_the_evaluator_is_coupled() {
    // one sweep of single-hold bisections: b for q with a = 55, then a for n with that b
    let bis = |f: &dyn Fn(f64) -> f64, mut lo: f64, mut hi: f64, tol: f64| -> f64 {
        let mut flo = f(lo);
        let mut m = lo;
        for _ in 0..200 {
            m = lo + 0.5 * (hi - lo);
            let fm = f(m);
            if fm.abs() <= tol {
                return m;
            }
            if (fm > 0.0) == (flo > 0.0) {
                lo = m;
                flo = fm;
            } else {
                hi = m;
            }
        }
        m
    };
    let ev = |s: f64, a: f64, b: f64| {
        let o = WC.evaluate(&[
            Quantity::new("s", s, "1"),
            Quantity::new("c", 0.5, "1"),
            Quantity::new("b", b, "1"),
            Quantity::new("a", a, "V"),
        ]);
        (o.metrics[0].value, o.metrics[1].value)
    };
    let mut off = 0;
    for s in [0.0, 1.0] {
        let b1 = bis(&|b| ev(s, 55.0, b).0 - 1.0, 0.001, 0.1, TOL_Q);
        let a1 = bis(&|a| ev(s, a, b1).1 - 50.0, 10.0, 100.0, TOL_N);
        let (q, n) = ev(s, a1, b1);
        eprintln!(
            "NCJ1: s {s}: after one sweep q - 1 = {:.4e}, n - 50 = {:.4e}",
            q - 1.0,
            n - 50.0
        );
        off += usize::from((q - 1.0).abs() > TOL_Q);
    }
    assert_eq!(off, 2, "NCJ1: the control must fail J1 (a)");
}

#[test]
fn ncj2_only_the_flow_held() {
    let dir = tmp("ncj2");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let mut dom = dom_c();
    dom.push(Bound::new("a", "V", 45.0, 55.0));
    let h = wc_claim(&WC, 728_002, 0.01, vec![hold_q(HeldTarget::Value(1.0))], dom);
    let r = register(&l, &h).unwrap();
    let rep = run(&l, &r.id, &WC, &RunOptions::default()).unwrap();
    let res = rep.verification.result.unwrap();
    let dist = (res.stats.mean - d_star()).abs();
    let lim = 100.0 * floor_closed(&WC);
    eprintln!(
        "NCJ2: only q held: mean {:.6} against d* {:.6}: distance {dist:.3e} (must exceed {lim:.3e}); {}",
        res.stats.mean,
        d_star(),
        res.finding.as_str()
    );
    assert!(dist > lim, "NCJ2");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- W3s, a single hold (the smoke's set-up)

struct W3 {
    b: f64,
    tier: Fidelity,
}

impl Model for W3 {
    fn id(&self) -> &str {
        "synthetic.w3s"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        self.tier
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let v = |n: &str| p.iter().find(|q| q.name == n).map_or(0.0, |q| q.value);
        let (s, n) = (v("s"), v("n"));
        let q = n * (1.0 + 0.5 * s);
        let tone = q * q * (1.0 + self.b * s);
        Outcome::ok(vec![Quantity::new("q", q, "m3/s"), Quantity::new("tone", tone, "Pa")])
    }
}

const W3S: W3 = W3 {
    b: -0.1,
    tier: Fidelity::ReducedOrder,
};

fn w3_claim(m: &W3, target: HeldTarget, seed: u64) -> Hypothesis {
    let mut h = Hypothesis::new(
        "smoke",
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
        seed,
        Scope::of(m, true),
    );
    h.held = vec![Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target,
        tolerance: 1e-13,
        by: "n".into(),
        by_unit: "1/s".into(),
        bracket: (0.1, 10.0),
    }];
    h
}

const STATEMENT_TAIL: &str = " with `q` held within 0.0000000000001 m3/s by `n` over the registered domain. Confirmed as \
                              hypothesis H-1 of the family (slot 1, level 3.040e-2). Model error of this tier unknown. Not \
                              tested at cfd.";

#[test]
fn j0_single_hold_bytes_of_the_base() {
    let lits = [
        (
            HeldTarget::Value(1.0),
            728_900u64,
            29785usize,
            "aaefeee2393a6515",
            "fnv1a64:8036707366931a92",
            "-1.0000e-1",
        ),
        (
            HeldTarget::AsLowArm { at: 0.8 },
            728_901,
            29351,
            "d0af079ac415b42b",
            "fnv1a64:2ec4743bbbfe4537",
            "-6.4000e-2",
        ),
    ];
    let mut ok = 0;
    for (i, (target, seed, len, fnv, rh, change)) in lits.iter().enumerate() {
        let dir = tmp(&format!("j0_{i}"));
        let path = dir.join("l.jsonl");
        let l = ledger_at(&path);
        let h = w3_claim(&W3S, *target, *seed);
        let r = register(&l, &h).unwrap();
        run(&l, &r.id, &W3S, &RunOptions::default()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let es = entries(&l);
        let v = verify(&es, &r.id, Some(&W3S));
        let q = LessonQuery {
            current: vec![("synthetic.w3s".into(), "1".into())],
            ..Default::default()
        };
        let ls = lessons(&es, &q, &[]);
        let want = format!(
            "reduced_order (synthetic.w3s 1): raising `s` from 0 to 1 1 changed `tone` by {change} Pa on average (one-sided \
             bounds {change}, {change}; 16 of 16 pairs with d > 0){STATEMENT_TAIL}"
        );
        let st_ok = ls.rows.len() == 1 && ls.rows[0].statement == want;
        let text = String::from_utf8_lossy(&bytes);
        let got = format!("{:016x}", fnv64(&bytes));
        eprintln!(
            "J0 {i}: {} entries, {} bytes (base {len}), fnv {got} (base {fnv}), record hash {} (base {rh}), statement equal \
             {st_ok}, problems {}, held_floor key {}",
            es.len(),
            bytes.len(),
            h.hash(),
            v.problems.len(),
            text.contains("held_floor")
        );
        if !st_ok {
            eprintln!(
                "J0 {i} statement: {:?}",
                ls.rows.iter().map(|r| &r.statement).collect::<Vec<_>>()
            );
        }
        if es.len() == 34
            && bytes.len() == *len
            && got == *fnv
            && h.hash() == *rh
            && st_ok
            && v.problems.is_empty()
            && !text.contains("held_floor")
        {
            ok += 1;
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert_eq!(ok, 2, "J0");
}

#[test]
fn j2b_single_hold_floor() {
    let fv = held_floor(&w3_claim(&W3S, HeldTarget::Value(1.0), 728_900), &W3S).unwrap();
    let fa = held_floor(&w3_claim(&W3S, HeldTarget::AsLowArm { at: 0.8 }, 728_901), &W3S).unwrap();
    let (cv, ca) = ((2.0 + 2.0 * 0.9) * 1e-13, 2.0 * 0.8 * 0.9 * 1e-13);
    eprintln!(
        "J2b: value floor {:.12e} closed {cv:.12e} (rel {:.2e}, {} arms); as_low_arm {:.12e} closed {ca:.12e} (rel {:.2e}, {} \
         arms)",
        fv.value,
        rel(fv.value, cv),
        fv.arms.len(),
        fa.value,
        rel(fa.value, ca),
        fa.arms.len()
    );
    assert!(rel(fv.value, cv) <= 1e-6, "J2b value");
    assert!(rel(fa.value, ca) <= 1e-6, "J2b as_low_arm");
}

// ---------------------------------------------------------------- as_low_arm coupled (J2c, J7, J8)

fn aslow() -> Vec<Held> {
    vec![
        hold_q(HeldTarget::AsLowArm { at: 0.02 }),
        hold_n(HeldTarget::AsLowArm { at: 50.0 }, (10.0, 100.0)),
    ]
}

#[test]
fn j2c_j7_j8_as_low_arm_coupled() {
    let dir = tmp("j7");
    let path = dir.join("l.jsonl");
    let h = wc_claim(&WC, 728_007, 0.01, aslow(), dom_c());
    // closed form of the low arm at a = 50, b = 0.02, s = 0
    let nl = 50.0 / (1.0 + 0.2 * 0.02);
    let ql = 0.02 * nl;
    let yl = 50.0 * 0.02;
    let ds = closed(&WC, 1.0, 0.0, ql, nl).y - yl;
    let fl = phi(&WC, 1.0, 0.5, ql, nl);
    // J2c
    let f = held_floor(&h, &WC).unwrap();
    let r2c = rel(f.value, fl);
    eprintln!(
        "J2c: as_low_arm floor {:.12e} closed {fl:.12e} (rel {r2c:.2e}); arms {:?}",
        f.value,
        f.arms.iter().map(|a| a.arm).collect::<Vec<_>>()
    );
    assert!(r2c <= 1e-6, "J2c");
    assert!(f.arms.len() == 1 && f.arms[0].arm == Arm::High);
    // J7
    let v = run_wc(&path, &WC, &h, 1);
    let es = entries(&ledger_at(&path));
    let ev = evidence(&es);
    let mut matched = 0;
    for p in 0..32 {
        let (lo, hi) = (ev[2 * p], ev[2 * p + 1]);
        if (metric(hi, "q") - metric(lo, "q")).abs() <= TOL_Q && (metric(hi, "n") - metric(lo, "n")).abs() <= TOL_N {
            matched += 1;
        }
    }
    let res = v.result.clone().unwrap();
    let err = (res.stats.mean - ds).abs();
    eprintln!(
        "J7: {matched} of 32 high arms matched to their low arm; mean {:.15} d*_low {ds:.15} error {err:.3e} (limit {fl:.6e}); \
         {}; problems {:?}",
        res.stats.mean,
        res.finding.as_str(),
        v.problems
    );
    assert_eq!(matched, 32, "J7 matched");
    assert!(err <= fl, "J7 mean");
    assert_eq!(res.finding, Finding::Confirmed);
    // J8: cuts
    let full = std::fs::read(&path).unwrap();
    let ls = lines(&path);
    let mut same = 0;
    for k in [0usize, 1, 33, 63] {
        let pc = dir.join(format!("cut{k}.jsonl"));
        write_lines(&pc, &ls[..1 + k]);
        let l = ledger_at(&pc);
        run(&l, "H-1", &WC, &RunOptions::default()).unwrap();
        let eq = std::fs::read(&pc).unwrap() == full;
        eprintln!("J8: cut after {k} evidence lines: byte-identical {eq}");
        same += usize::from(eq);
    }
    assert_eq!(same, 4, "J8 cuts");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- J3, J4 (a) to (c): the floor rule and its guards

#[test]
fn j3_j4_floor_rule_and_guards() {
    let dir = tmp("j3");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let low = wc_claim(&WC, 728_003, 2.0e-5, both(), dom_c());
    let fv = held_floor(&low, &WC).unwrap().value;
    let before = read_bytes(&path);
    let r = register_with_floor(&l, &low, &WC);
    let msg = r.as_ref().err().cloned().unwrap_or_default();
    let named = msg.contains("min_effect")
        && msg.contains("held-tolerance floor")
        && msg.contains("0.00002")
        && msg.contains(&format!("{fv}"));
    let unchanged = read_bytes(&path) == before;
    eprintln!("J3: refused: {msg}; both values named {named}; file unchanged {unchanged}");
    assert!(named && unchanged, "J3 refusal");
    let okh = wc_claim(&WC, 728_003, 2.1e-5, both(), dom_c());
    let reg = register_with_floor(&l, &okh, &WC).unwrap();
    eprintln!("J3: min_effect 2.1e-5 accepted as {} slot {}", reg.id, reg.slot);
    assert_eq!(reg.slot, 1);
    assert!(verify(&entries(&l), "H-1", Some(&WC)).problems.is_empty());
    let ls = lines(&path);
    let edit = |tag: &str, f: &dyn Fn(&mut Value)| -> PathBuf {
        let mut v: Value = serde_json::from_str(&ls[0]).unwrap();
        f(&mut v);
        let p = dir.join(format!("{tag}.jsonl"));
        write_lines(&p, &[v.to_string()]);
        p
    };
    // (a) the floor removed
    let pa = edit("a", &|v| {
        v["settings"].as_object_mut().unwrap().remove("held_floor");
    });
    let va = verify(&entries(&ledger_at(&pa)), "H-1", None);
    let ra = run(&ledger_at(&pa), "H-1", &WC, &RunOptions::default());
    let a_hit = va.problems.iter().any(|p| p.contains("held-tolerance floor"))
        && ra.as_ref().err().is_some_and(|e| e.contains("held-tolerance floor"));
    eprintln!("J4 (a): {:?}; run {:?}", va.problems, ra.as_ref().err());
    // (b) the stored floor set to 0.01
    let pb = edit("b", &|v| v["settings"]["held_floor"]["value"] = Value::from(0.01));
    let vb = verify(&entries(&ledger_at(&pb)), "H-1", None);
    let b_hit = vb
        .problems
        .iter()
        .any(|p| p.contains("min_effect") && p.contains("held-tolerance floor"));
    eprintln!("J4 (b): {:?}", vb.problems);
    // (c) the stored floor times 1 + 1e-9
    let st = serde_json::from_str::<Value>(&ls[0]).unwrap()["settings"]["held_floor"]["value"]
        .as_f64()
        .unwrap();
    let pc = edit("c", &|v| {
        v["settings"]["held_floor"]["value"] = Value::from(st * (1.0 + 1e-9))
    });
    let esc = entries(&ledger_at(&pc));
    let vc0 = verify(&esc, "H-1", None);
    let vc1 = verify(&esc, "H-1", Some(&WC));
    let c_hit = vc0.problems.is_empty() && vc1.problems.iter().any(|p| p.contains("not reproduced"));
    eprintln!(
        "J4 (c): without the model {:?}; with it {:?}",
        vc0.problems, vc1.problems
    );
    assert!(a_hit, "J4 (a)");
    assert!(b_hit, "J4 (b)");
    assert!(c_hit, "J4 (c)");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- J5, J6, J9

#[test]
fn j5_unmatched_coupled_hold_is_void() {
    let dir = tmp("j5");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let m = Wc { gamma: 1.0, ..WC };
    let held = vec![
        hold_q(HeldTarget::Value(1.0)),
        hold_n(HeldTarget::Value(50.0), (10.0, 90.0)),
    ];
    let r = register_with_floor(&l, &wc_claim(&m, 728_005, 0.01, held.clone(), dom_c()), &m).unwrap();
    let rep = run(&l, &r.id, &m, &RunOptions::default()).unwrap();
    let res = rep.verification.result.unwrap();
    let why = res.void_reason.clone().unwrap_or_default();
    let next = register_with_floor(&l, &wc_claim(&m, 728_006, 0.01, held, dom_c()), &m).unwrap();
    eprintln!(
        "J5: {} ({why}); next registration {} slot {}",
        res.finding.as_str(),
        next.id,
        next.slot
    );
    assert_eq!(res.finding, Finding::Void);
    assert!(why.contains("pair ") && why.contains("arm low") && (why.contains("'q'") || why.contains("'n'")));
    assert_eq!(next.slot, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn j6_refusals() {
    let dir = tmp("j6");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let base = wc_claim(&WC, 728_010, 0.01, both(), dom_c());
    let before = read_bytes(&path);
    let with = |held: Vec<Held>| {
        let mut h = base.clone();
        h.held = held;
        h
    };
    let mut third = hold_q(HeldTarget::Value(1.0));
    third.metric = "x".into();
    third.by = "z".into();
    let mut q_twice = hold_q(HeldTarget::Value(1.0));
    q_twice.by = "a".into();
    let mut b_twice = hold_n(HeldTarget::Value(50.0), (10.0, 100.0));
    b_twice.by = "b".into();
    let cases: Vec<(&[&str], Result<Registered, String>)> = vec![
        (&["held: 2 held metrics", "register_with_floor"], register(&l, &base)),
        (
            &["held: 3 held metrics"],
            register_with_floor(&l, &with(vec![both()[0].clone(), both()[1].clone(), third]), &WC),
        ),
        (
            &["given twice"],
            register_with_floor(&l, &with(vec![hold_q(HeldTarget::Value(1.0)), q_twice]), &WC),
        ),
        (
            &["compensating parameter 'b'"],
            register_with_floor(&l, &with(vec![hold_q(HeldTarget::Value(1.0)), b_twice]), &WC),
        ),
        (
            &["targets"],
            register_with_floor(
                &l,
                &with(vec![
                    hold_q(HeldTarget::Value(1.0)),
                    hold_n(HeldTarget::AsLowArm { at: 50.0 }, (10.0, 100.0)),
                ]),
                &WC,
            ),
        ),
        (&["no held metric"], register_with_floor(&l, &with(vec![]), &WC)),
        (&["model"], register_with_floor(&l, &base, &OtherId)),
        (
            &["probe"],
            register_with_floor(
                &l,
                &with(vec![
                    hold_q(HeldTarget::Value(1.0)),
                    hold_n(HeldTarget::Value(50.0), (10.0, 20.0)),
                ]),
                &WC,
            ),
        ),
    ];
    let mut ok = 0;
    for (want, r) in &cases {
        let msg = r.as_ref().err().cloned().unwrap_or_default();
        let hit = want.iter().all(|w| msg.contains(w));
        eprintln!("J6 {want:?}: {} -> {msg}", if hit { "refused, named" } else { "MISS" });
        ok += usize::from(hit);
    }
    let unchanged = read_bytes(&path) == before;
    eprintln!(
        "J6: {ok} of {} refused with the field named; file unchanged {unchanged}",
        cases.len()
    );
    assert_eq!(ok, cases.len());
    assert!(unchanged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn j9_rerun_and_statement() {
    let dir = tmp("j9");
    let path = dir.join("l.jsonl");
    let m = Wc {
        tier: Fidelity::ReducedOrder,
        ..WC
    };
    let h = wc_claim(&m, 728_009, 0.01, both(), dom_c());
    run_wc(&path, &m, &h, 1);
    let es = entries(&ledger_at(&path));
    let v = verify(&es, "H-1", Some(&m));
    let q = LessonQuery {
        current: vec![("synthetic.wc".into(), "1".into())],
        ..Default::default()
    };
    let ls = lessons(&es, &q, &[]);
    let st = ls.rows.first().map(|r| r.statement.clone()).unwrap_or_default();
    eprintln!(
        "J9: problems {:?}; evidence_rerun {:?}; {} rows: {st}",
        v.problems,
        v.evidence_rerun,
        ls.rows.len()
    );
    assert!(v.problems.is_empty());
    assert_eq!(v.evidence_rerun, Some(true));
    assert_eq!(ls.rows.len(), 1);
    assert!(st.contains("`q` held within 0.000001 m3/s by `b`") && st.contains("`n` held within 0.00001 1/s by `a`"));
    let _ = std::fs::remove_dir_all(&dir);
}
