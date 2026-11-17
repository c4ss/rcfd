//! ADR 0698: checks of lessons_drawn (F-781): drawn checks must be newer than the evidence they check. Synthetic evaluators only; no
//! CFD, no person, no language model. W5 and the impostor are copied from f745_drawn_sample.rs (not edited).

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{entry_ref, Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_core::hash::fnv64;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const K727: usize = 13;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f781_{tag}_{}", std::process::id()));
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

// ---------------------------------------------------------------- W5 and the file-aware impostor

#[derive(Clone)]
struct W5 {
    seed: u64,
    id: &'static str,
    version: &'static str,
}

fn w5(seed: u64) -> W5 {
    W5 {
        seed,
        id: "synthetic.w5",
        version: "1",
    }
}

impl Model for W5 {
    fn id(&self) -> &str {
        self.id
    }
    fn version(&self) -> &str {
        self.version
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Cfd
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        Outcome::ok(vec![Quantity::new("y", val(p, "x0") + noise(self.seed, 0, p), "1")])
    }
}

/// Same id, version and tier; `y + 0.3` on the high arm of the dishonest points, (`rel` is unused here).
struct Impostor {
    real: W5,
    dishonest: Vec<u64>,
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
        Fidelity::Cfd
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let y = self.real.evaluate(p).metrics[0].value;
        let y = if let Some(r) = self.rel {
            y * (1.0 + r)
        } else if val(p, "x0") == 0.75 && self.dishonest.contains(&val(p, "x1").to_bits()) {
            y + 0.3
        } else {
            y
        };
        Outcome::ok(vec![Quantity::new("y", y, "1")])
    }
}

fn w5_claim(m: &W5, seed: u64, deterministic: bool) -> Hypothesis {
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
        64,
        seed,
        Scope::of(m, deterministic),
    );
    if deterministic {
        h.rerun_sample = Some(sample(K727, 0.0));
    }
    h
}

fn sample(k: usize, rtol: f64) -> RerunSample {
    RerunSample {
        pairs: k,
        rtol,
        atol: 0.0,
    }
}

/// Which evaluator writes the evidence of a world ledger.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
enum Who {
    /// Dishonest on every pair outside the F-727 sample.
    D1,
    /// Dishonest on 16 pairs outside the F-727 sample.
    D2,
    /// The real model.
    Honest,
}

#[allow(dead_code)]
struct Built {
    path: PathBuf,
    es: Vec<Entry>,
    s727: Vec<usize>,
    dishonest: Vec<usize>,
    hash: String,
    seed: u64,
}

/// Register the W5 claim (with ADR 0662's k = 13 sample), compute from the registered record only the F-727 sample, and run
/// with the evaluator `who`.
fn build(path: &Path, real: &W5, seed: u64, who: Who) -> Built {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let h = w5_claim(real, seed, true);
    let reg = register(&l, &h).unwrap();
    let rec = reg.record.clone().unwrap();
    let s727 = rerun_pairs(rec.n_pairs, K727, rerun_seed(&reg.hash, rec.seed));
    let rest: Vec<usize> = (0..rec.n_pairs).filter(|p| !s727.contains(p)).collect();
    let dishonest = match who {
        Who::D1 => rest,
        Who::D2 => {
            let mut r = rest;
            Rng::new(seed ^ 0x0745_0D02).shuffle(&mut r);
            r.truncate(16);
            r
        }
        Who::Honest => Vec::new(),
    };
    let pts = design_points(&rec, reg.derived_seed);
    match who {
        Who::Honest => {
            run(&l, &reg.id, real, &RunOptions::default()).unwrap();
        }
        _ => {
            let imp = Impostor {
                real: real.clone(),
                dishonest: dishonest.iter().map(|&p| pts[p][0].to_bits()).collect(),
                rel: None,
            };
            run(&l, &reg.id, &imp, &RunOptions::default()).unwrap();
        }
    }
    Built {
        path: path.to_path_buf(),
        es: entries(&l),
        s727,
        dishonest,
        hash: reg.hash,
        seed: rec.seed,
    }
}

// ---------------------------------------------------------------- helpers of ADR 0695

fn verifier_seed(w: u64) -> u64 {
    fnv64(format!("f781:verifier:{w}").as_bytes())
}

fn query() -> LessonQuery {
    LessonQuery {
        current: vec![("synthetic.w5".into(), "1".into())],
        ..Default::default()
    }
}

fn drawn_rows(es: &[Entry], real: &W5) -> DrawnLessons {
    lessons_drawn(es, &query(), &[real])
}

fn query_stored() -> LessonQuery {
    LessonQuery {
        trust_stored: true,
        ..query()
    }
}

/// A failing k = 4 seed: the first of `fnv64("f781:seed:<i>")` whose check of H-1 does not reproduce.
fn failing_seed(es: &[Entry], real: &W5) -> u64 {
    (0u64..100_000)
        .map(|i| fnv64(format!("f781:seed:{i}").as_bytes()))
        .find(|&s| !check_drawn(es, "H-1", real, &sample(4, 0.0), s).unwrap().reproduced)
        .unwrap()
}

fn is_ev(e: &Entry, id: &str) -> bool {
    e.kind == "evaluation"
        && e.provenance.get("driver").and_then(|v| v.as_str()) == Some("hypothesis")
        && e.provenance.get("hypothesis").and_then(|v| v.as_str()) == Some(id)
}

/// The impostor's second evidence set of `id`, appended in memory: a copy of the evidence with the high arm raised by 0.3, then a
/// copy of the last result entry whose parents are the registration and the new copies.
fn with_second_set(es: &[Entry], id: &str) -> Vec<Entry> {
    let mut out = es.to_vec();
    let mut refs = Vec::new();
    for e in es.iter().filter(|e| is_ev(e, id)) {
        let mut c = e.clone();
        if val(&c.params, "x0") == 0.75 {
            c.metrics[0].value += 0.3;
        }
        refs.push(entry_ref(out.len(), &c));
        out.push(c);
    }
    let mut r = es
        .iter()
        .rev()
        .find(|e| e.kind == KIND_RESULT && e.variant == id)
        .unwrap()
        .clone();
    let reg = r.parents[0].clone();
    r.parents = std::iter::once(reg).chain(refs).collect();
    out.push(r);
    out
}

fn two_hyp(path: &Path, real: &W5, seed: u64, check_h1: bool, check_h2: bool) -> Vec<Entry> {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let v = |w: u64| DrawSeed::Given(verifier_seed(w));
    let r1 = register(&l, &w5_claim(real, seed, true)).unwrap();
    run(&l, &r1.id, real, &RunOptions::default()).unwrap();
    if check_h1 {
        verify_drawn(&l, "H-1", real, &sample(4, 0.0), v(seed)).unwrap();
    }
    let r2 = register(&l, &w5_claim(real, seed + 1, true)).unwrap();
    run(&l, &r2.id, real, &RunOptions::default()).unwrap();
    if check_h2 {
        verify_drawn(&l, "H-2", real, &sample(4, 0.0), v(seed + 1)).unwrap();
    }
    entries(&l)
}

fn ids(d: &DrawnLessons) -> Vec<&str> {
    d.rows.iter().map(|r| r.lesson.id.as_str()).collect()
}

fn counts(d: &DrawnLessons, id: &str) -> Option<(usize, usize)> {
    d.rows
        .iter()
        .find(|r| r.lesson.id == id)
        .map(|r| (r.drawn_checks, r.drawn_stale))
}

// ---------------------------------------------------------------- B0

#[test]
fn b0_no_records_bit_for_bit() {
    let dir = tmp("b0");
    let (mut equal, mut total) = (0, 0);
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let one = build(&dir.join("one.jsonl"), &real, seed, Who::Honest).es;
        let two = two_hyp(&dir.join("two.jsonl"), &real, seed, false, false);
        for es in [&one, &two] {
            for q in [query(), query_stored()] {
                let held: [&dyn Model; 1] = [&real];
                let base = lessons(es, &q, &held);
                let d = lessons_drawn(es, &q, &held);
                total += 1;
                let zero = d.rows.iter().all(|r| {
                    r.drawn_checks == 0
                        && r.drawn_failed == 0
                        && r.drawn_ignored == 0
                        && r.drawn_unchecked == 0
                        && r.drawn_stale == 0
                        && r.drawn_latest.is_none()
                });
                if d.plain() == base
                    && format!("{:?}", d.plain()) == format!("{base:?}")
                    && zero
                    && d.stale.is_empty()
                    && d.dropped.is_empty()
                    && d.drawn_flags.is_empty()
                {
                    equal += 1;
                }
            }
        }
    }
    eprintln!("B0: {equal} of {total}");
    assert_eq!(total, 160);
    assert_eq!(equal, 160, "B0: 160 of 160");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- B1

#[test]
fn b1_honest_row_kept() {
    let dir = tmp("b1");
    let mut ok = 0;
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let b = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
        let l = ledger_at(&b.path);
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        let es = entries(&l);
        let base = lessons(&b.es, &query(), &[&real]);
        let d = drawn_rows(&es, &real);
        if d.rows.len() == 1
            && d.rows[0].lesson == base.rows[0]
            && (d.rows[0].drawn_checks, d.rows[0].drawn_failed, d.rows[0].drawn_stale) == (1, 0, 0)
            && d.rows[0].drawn_latest == c.record
            && d.stale.is_empty()
            && d.dropped.is_empty()
        {
            ok += 1;
        }
    }
    eprintln!("B1: {ok} of 40");
    assert_eq!(ok, 40, "B1: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- B2

#[test]
fn b2_second_set_after_a_passing_record() {
    let dir = tmp("b2");
    let (mut default_ok, mut stored_ok) = (0, 0);
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let b = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
        let l = ledger_at(&b.path);
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        assert!(c.reproduced);
        let es = entries(&l);
        let first_copy = es.len();
        let es2 = with_second_set(&es, "H-1");
        let held: [&dyn Model; 1] = [&real];
        // default mode
        let base = lessons(&es2, &query(), &held);
        let d = lessons_drawn(&es2, &query(), &held);
        if d.rows.is_empty() && d.plain() == base && !base.flagged.is_empty() {
            default_ok += 1;
        }
        // stored mode
        let d = lessons_drawn(&es2, &query_stored(), &held);
        if d.rows.len() == 1
            && (d.rows[0].drawn_checks, d.rows[0].drawn_stale) == (0, 1)
            && d.rows[0].drawn_latest.is_none()
            && d.stale
                == [DrawnStale {
                    id: "H-1".into(),
                    record: c.record.unwrap(),
                    entries_seen: c.entries_seen,
                    newer: first_copy,
                }]
            && d.drawn_flags.iter().any(|f| f.problem.contains("stale"))
            && d.dropped.is_empty()
        {
            stored_ok += 1;
        }
    }
    eprintln!("B2: default {default_ok} of 40, stored {stored_ok} of 40");
    assert_eq!(default_ok, 40, "B2 default: 40 of 40");
    assert_eq!(stored_ok, 40, "B2 stored: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- B3

#[test]
fn b3_stale_failure_does_not_drop() {
    let dir = tmp("b3");
    let (mut stale_ok, mut control_ok) = (0, 0);
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
        let s = failing_seed(&b.es, &real);
        let l = ledger_at(&b.path);
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(s)).unwrap();
        assert!(!c.reproduced);
        let es = entries(&l);
        let held: [&dyn Model; 1] = [&real];
        let d0 = lessons_drawn(&es, &query_stored(), &held);
        if d0.rows.is_empty() && d0.dropped.len() == 1 && d0.dropped[0].record == c.record.unwrap() {
            control_ok += 1;
        }
        let es2 = with_second_set(&es, "H-1");
        let d = lessons_drawn(&es2, &query_stored(), &held);
        if d.rows.len() == 1
            && (d.rows[0].drawn_checks, d.rows[0].drawn_stale, d.rows[0].drawn_failed) == (0, 1, 0)
            && d.dropped.is_empty()
            && d.stale.len() == 1
        {
            stale_ok += 1;
        }
    }
    eprintln!("B3: stale {stale_ok} of 40, control {control_ok} of 40");
    assert_eq!(stale_ok, 40, "B3: 40 of 40");
    assert_eq!(control_ok, 40, "B3 control: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- B4

#[test]
fn b4_two_hypotheses() {
    let dir = tmp("b4");
    let (mut a_ok, mut b_ok, mut c_ok, mut order) = (0, 0, 0, 0);
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let held: [&dyn Model; 1] = [&real];
        // (a) H-1 checked, then H-2 run: H-2's evidence is newer than H-1's record
        let ea = two_hyp(&dir.join("a.jsonl"), &real, seed, true, false);
        let da = lessons_drawn(&ea, &query(), &held);
        if counts(&da, "H-1") == Some((1, 0)) && counts(&da, "H-2") == Some((0, 0)) && da.stale.is_empty() {
            a_ok += 1;
        }
        if ids(&da) == ["H-1", "H-2"] {
            order += 1;
        }
        // (b) both checked
        let eb = two_hyp(&dir.join("b.jsonl"), &real, seed, true, true);
        let db = lessons_drawn(&eb, &query(), &held);
        if counts(&db, "H-1") == Some((1, 0)) && counts(&db, "H-2") == Some((1, 0)) && db.stale.is_empty() {
            b_ok += 1;
        }
        if ids(&db) == ["H-1", "H-2"] {
            order += 1;
        }
        // (c) a second set of H-1 only
        let ec = with_second_set(&eb, "H-1");
        let dc = lessons_drawn(&ec, &query_stored(), &held);
        if counts(&dc, "H-1") == Some((0, 1))
            && counts(&dc, "H-2") == Some((1, 0))
            && dc.stale.len() == 1
            && dc.stale[0].id == "H-1"
        {
            c_ok += 1;
        }
        if ids(&dc) == ["H-1", "H-2"] {
            order += 1;
        }
    }
    eprintln!("B4: a {a_ok}, b {b_ok}, c {c_ok} of 40, order {order} of 120");
    assert_eq!(a_ok, 40, "B4a: 40 of 40");
    assert_eq!(b_ok, 40, "B4b: 40 of 40");
    assert_eq!(c_ok, 40, "B4c: 40 of 40");
    assert_eq!(order, 120, "B4 order: 120 of 120");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- B5, B6

#[test]
fn b5_evidence_at_entries_seen_is_newer() {
    let dir = tmp("b5");
    let mut ok = 0;
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let b = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
        let l = ledger_at(&b.path);
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        let es = entries(&l);
        let r = c.record.unwrap();
        assert_eq!(c.entries_seen, r);
        let copy = es.iter().find(|e| is_ev(e, "H-1")).unwrap().clone();
        let mut es2 = es[..r].to_vec();
        es2.push(copy);
        es2.push(es[r].clone());
        let d = lessons_drawn(&es2, &query_stored(), &[&real]);
        if d.rows.len() == 1
            && (d.rows[0].drawn_checks, d.rows[0].drawn_stale) == (0, 1)
            && d.stale.len() == 1
            && d.stale[0].record == r + 1
            && d.stale[0].newer == r
        {
            ok += 1;
        }
    }
    eprintln!("B5: {ok} of 40");
    assert_eq!(ok, 40, "B5: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn b6_helper_boundary() {
    let dir = tmp("b6");
    let mut ok = 0;
    for w in 0..40u64 {
        let seed = 781_000 + w;
        let real = w5(seed);
        let b = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
        let last = b.es.iter().rposition(|e| is_ev(e, "H-1")).unwrap();
        let cut = dir.join("cut.jsonl");
        let _ = std::fs::remove_file(&cut);
        let l = ledger_at(&cut);
        l.append(&b.es[..=last]).unwrap();
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        let es = entries(&l);
        let r = c.record.unwrap();
        assert_eq!((c.entries_seen, r), (last + 1, last + 1));
        let none = drawn_newer_evidence(&es, r).is_none();
        let mut es2 = es[..r].to_vec();
        es2.push(es[last].clone());
        es2.push(es[r].clone());
        let some = drawn_newer_evidence(&es2, r + 1) == Some(r);
        let not_a_record = drawn_newer_evidence(&es, 0).is_none() && drawn_newer_evidence(&es, 9999).is_none();
        if none && some && not_a_record {
            ok += 1;
        }
    }
    eprintln!("B6: {ok} of 40");
    assert_eq!(ok, 40, "B6: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}
