//! ADR 0695: checks of lessons_drawn (F-776): lessons read the verifier-drawn checks of ADR 0693. Synthetic evaluators only; no
//! CFD, no person, no language model. W5 and the impostor are copied from f745_drawn_sample.rs (not edited).

use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{Entry, Fidelity, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_core::hash::fnv64;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;
const K727: usize = 13;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f776_{tag}_{}", std::process::id()));
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
    fnv64(format!("f776:verifier:{w}").as_bytes())
}

fn query() -> LessonQuery {
    LessonQuery {
        current: vec![("synthetic.w5".into(), "1".into())],
        ..Default::default()
    }
}

fn queries() -> Vec<LessonQuery> {
    vec![
        query(),
        LessonQuery {
            include_stale: true,
            current: Vec::new(),
            ..Default::default()
        },
        query(),
        LessonQuery {
            min_tier: Some(Fidelity::Cfd),
            ..query()
        },
        LessonQuery {
            factor: Some("nothing".into()),
            ..query()
        },
    ]
}

/// The first seed of `fnv64("f776:seed:<i>")` whose k-pair drawn check of `es` has the wanted outcome.
fn seed_with(es: &[Entry], real: &W5, k: usize, reproduced: bool) -> u64 {
    (0u64..100_000)
        .map(|i| fnv64(format!("f776:seed:{i}").as_bytes()))
        .find(|&s| check_drawn(es, "H-1", real, &sample(k, 0.0), s).unwrap().reproduced == reproduced)
        .unwrap()
}

fn drawn_rows(es: &[Entry], real: &W5) -> DrawnLessons {
    lessons_drawn(es, &query(), &[real])
}

// ---------------------------------------------------------------- A0

#[test]
fn a0_no_records_bit_for_bit() {
    let dir = tmp("a0");
    let mut equal = 0;
    let mut total = 0;
    for w in 0..40u64 {
        let seed = 776_000 + w;
        let real = w5(seed);
        for who in [Who::D1, Who::D2, Who::Honest] {
            let b = build(&dir.join("l.jsonl"), &real, seed, who);
            assert_eq!(lessons(&b.es, &query(), &[&real]).rows.len(), 1);
            for (i, q) in queries().iter().enumerate() {
                let none: [&dyn Model; 0] = [];
                let held: [&dyn Model; 1] = [&real];
                let rr: &[&dyn Model] = if i == 2 { &none } else { &held };
                let base = lessons(&b.es, q, rr);
                let d = lessons_drawn(&b.es, q, rr);
                total += 1;
                let counts_zero = d.rows.iter().all(|r| {
                    r.drawn_checks == 0
                        && r.drawn_failed == 0
                        && r.drawn_ignored == 0
                        && r.drawn_unchecked == 0
                        && r.drawn_latest.is_none()
                });
                if d.plain() == base
                    && format!("{:?}", d.plain()) == format!("{base:?}")
                    && counts_zero
                    && d.dropped.is_empty()
                    && d.drawn_flags.is_empty()
                {
                    equal += 1;
                }
            }
        }
    }
    eprintln!("A0: {equal} of {total} equal");
    assert_eq!(total, 600);
    assert_eq!(equal, 600, "A0: 600 of 600");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- A1, A2

#[test]
fn a1_a2_impostor_dropped_honest_kept() {
    let dir = tmp("a1");
    let (mut caught_dropped, mut cases_ok, mut honest_ok) = (0, 0, 0);
    for w in 0..40u64 {
        let seed = 776_000 + w;
        let real = w5(seed);
        // A1: the D1 impostor
        let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
        let l = ledger_at(&b.path);
        let c = verify_drawn(&l, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        let es = entries(&l);
        let d = drawn_rows(&es, &real);
        let caught = !c.problems.is_empty();
        let rec = c.record.unwrap();
        if caught {
            if d.rows.is_empty()
                && d.dropped.len() == 1
                && d.dropped[0].record == rec
                && d.dropped[0].mismatched == c.mismatched
            {
                cases_ok += 1;
                caught_dropped += 1;
            }
        } else if d.rows.len() == 1
            && d.dropped.is_empty()
            && d.rows[0].drawn_checks == 1
            && d.rows[0].drawn_failed == 0
        {
            cases_ok += 1;
        }
        // A2: the honest evaluator
        let hb = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
        let hl = ledger_at(&hb.path);
        let hc = verify_drawn(&hl, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(w))).unwrap();
        let hes = entries(&hl);
        let base = lessons(&hb.es, &query(), &[&real]);
        let hd = drawn_rows(&hes, &real);
        if hd.rows.len() == 1
            && hd.rows[0].lesson == base.rows[0]
            && hd.rows[0].drawn_checks == 1
            && hd.rows[0].drawn_failed == 0
            && hd.rows[0].drawn_latest == hc.record
            && hd.dropped.is_empty()
        {
            honest_ok += 1;
        }
    }
    eprintln!("A1: cases {cases_ok} of 40, caught and dropped {caught_dropped} of 40; A2: {honest_ok} of 40");
    assert_eq!(cases_ok, 40, "A1: 40 of 40");
    assert!(caught_dropped >= 38, "A1: at least 38 of 40, got {caught_dropped}");
    assert_eq!(honest_ok, 40, "A2: 40 of 40");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- A3

#[test]
fn a3_latest_wins() {
    let dir = tmp("a3");
    let seed = 776_900;
    let real = w5(seed);
    let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
    let fail = seed_with(&b.es, &real, 1, false);
    let pass = seed_with(&b.es, &real, 1, true);
    eprintln!("A3: failing seed {fail}, passing seed {pass}");
    // failing then passing
    let l = ledger_at(&b.path);
    let f = verify_drawn(&l, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(fail)).unwrap();
    let p = verify_drawn(&l, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(pass)).unwrap();
    assert!(!f.reproduced && p.reproduced);
    let d = drawn_rows(&entries(&l), &real);
    eprintln!(
        "A3 fail-pass: rows {}, counts {:?}",
        d.rows.len(),
        d.rows.first().map(|r| (r.drawn_checks, r.drawn_failed, r.drawn_latest))
    );
    assert_eq!(d.rows.len(), 1);
    assert!(d.dropped.is_empty());
    assert_eq!(
        (d.rows[0].drawn_checks, d.rows[0].drawn_failed, d.rows[0].drawn_latest),
        (2, 1, p.record)
    );
    // passing then failing
    let b2 = build(&dir.join("d1b.jsonl"), &real, seed, Who::D1);
    let l2 = ledger_at(&b2.path);
    let p2 = verify_drawn(&l2, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(pass)).unwrap();
    let f2 = verify_drawn(&l2, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(fail)).unwrap();
    assert!(p2.reproduced && !f2.reproduced);
    let d2 = drawn_rows(&entries(&l2), &real);
    eprintln!("A3 pass-fail: rows {}, dropped {:?}", d2.rows.len(), d2.dropped);
    assert!(d2.rows.is_empty());
    assert_eq!(d2.dropped.len(), 1);
    assert_eq!(d2.dropped[0].record, f2.record.unwrap());
    assert_eq!(d2.dropped[0].mismatched, f2.mismatched);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- A4

fn set(e: &mut Entry, key: &str, v: serde_json::Value) {
    e.settings[key] = v;
}

#[test]
fn a4_forgeries_are_not_believed() {
    let dir = tmp("a4");
    let seed = 776_901;
    let real = w5(seed);
    let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
    let fail = seed_with(&b.es, &real, 1, false);
    let l = ledger_at(&b.path);
    let f = verify_drawn(&l, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(fail)).unwrap();
    let es = entries(&l);
    let fi = f.record.unwrap();
    // (a) the failing record edited to "reproduced": ignored, not believed
    let mut a = es.clone();
    set(&mut a[fi], "reproduced", true.into());
    let da = drawn_rows(&a, &real);
    eprintln!(
        "A4 (a): rows {} ignored {:?} flags {:?}",
        da.rows.len(),
        da.rows.first().map(|r| r.drawn_ignored),
        da.drawn_flags
    );
    assert_eq!(da.rows.len(), 1);
    assert!(da.dropped.is_empty());
    assert_eq!(
        (
            da.rows[0].drawn_checks,
            da.rows[0].drawn_ignored,
            da.rows[0].drawn_latest
        ),
        (0, 1, None)
    );
    assert!(da
        .drawn_flags
        .iter()
        .any(|x| x.index == fi && x.problem.contains("ignored")));
    // (b) a genuine failing record followed by a forged passing copy
    let mut bb = es.clone();
    let mut forged = bb[fi].clone();
    set(&mut forged, "reproduced", true.into());
    set(&mut forged, "mismatched", serde_json::json!([]));
    bb.push(forged);
    let db = drawn_rows(&bb, &real);
    eprintln!(
        "A4 (b): rows {} dropped {:?} flags {:?}",
        db.rows.len(),
        db.dropped,
        db.drawn_flags
    );
    assert!(db.rows.is_empty());
    assert_eq!(db.dropped.len(), 1);
    assert_eq!(db.dropped[0].record, fi);
    assert!(db
        .drawn_flags
        .iter()
        .any(|x| x.index == fi + 1 && x.problem.contains("ignored")));
    // honest ledger with one record
    let hb = build(&dir.join("h.jsonl"), &real, seed, Who::Honest);
    let hl = ledger_at(&hb.path);
    let hc = verify_drawn(&hl, "H-1", &real, &sample(4, 0.0), DrawSeed::Given(verifier_seed(7))).unwrap();
    let hes = entries(&hl);
    let hi = hc.record.unwrap();
    let base = lessons(&hb.es, &query(), &[&real]);
    // (c) edited to "not reproduced"
    let mut c = hes.clone();
    set(&mut c[hi], "reproduced", false.into());
    let dc = drawn_rows(&c, &real);
    eprintln!(
        "A4 (c): rows {} ignored {:?}",
        dc.rows.len(),
        dc.rows.first().map(|r| r.drawn_ignored)
    );
    assert!(dc.dropped.is_empty());
    assert_eq!(dc.rows.len(), 1);
    assert_eq!(dc.rows[0].lesson, base.rows[0]);
    assert_eq!((dc.rows[0].drawn_checks, dc.rows[0].drawn_ignored), (0, 1));
    // (d) record hash edited
    let mut d = hes.clone();
    set(&mut d[hi], "record_hash", "deadbeef".into());
    let dd = drawn_rows(&d, &real);
    eprintln!(
        "A4 (d): rows {} ignored {:?}",
        dd.rows.len(),
        dd.rows.first().map(|r| r.drawn_ignored)
    );
    assert_eq!(dd.rows.len(), 1);
    assert_eq!((dd.rows[0].drawn_checks, dd.rows[0].drawn_ignored), (0, 1));
    // (e) seed + 1
    let mut e = hes.clone();
    let s = e[hi].settings["seed"].as_u64().unwrap();
    set(&mut e[hi], "seed", s.wrapping_add(1).into());
    let de = drawn_rows(&e, &real);
    eprintln!(
        "A4 (e): rows {} ignored {:?}",
        de.rows.len(),
        de.rows.first().map(|r| r.drawn_ignored)
    );
    assert_eq!(de.rows.len(), 1);
    assert_eq!((de.rows[0].drawn_checks, de.rows[0].drawn_ignored), (0, 1));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- A5, A6

#[test]
fn a5_a6_no_model_no_drop() {
    let dir = tmp("a5");
    let seed = 776_902;
    let real = w5(seed);
    let b = build(&dir.join("d1.jsonl"), &real, seed, Who::D1);
    let fail = seed_with(&b.es, &real, 1, false);
    let l = ledger_at(&b.path);
    let f = verify_drawn(&l, "H-1", &real, &sample(1, 0.0), DrawSeed::Given(fail)).unwrap();
    assert!(!f.reproduced);
    let es = entries(&l);
    let v2 = W5 {
        version: "2",
        ..real.clone()
    };
    let none: [&dyn Model; 0] = [];
    let with_v2: [&dyn Model; 1] = [&v2];
    for (tag, rr) in [("none", &none[..]), ("version 2", &with_v2[..])] {
        let d = lessons_drawn(&es, &query(), rr);
        eprintln!(
            "A5 ({tag}): rows {} unchecked {:?} flags {}",
            d.rows.len(),
            d.rows.first().map(|r| r.drawn_unchecked),
            d.drawn_flags.len()
        );
        assert_eq!(d.rows.len(), 1);
        assert!(d.dropped.is_empty());
        assert_eq!(
            (
                d.rows[0].drawn_unchecked,
                d.rows[0].drawn_checks,
                d.rows[0].drawn_ignored
            ),
            (1, 0, 0)
        );
        assert!(d.drawn_flags.is_empty());
    }
    // A6: lessons of the entries after the record equal those before it; before it nothing is dropped
    let before = &es[..f.record.unwrap()];
    assert_eq!(lessons(&es, &query(), &[&real]), lessons(before, &query(), &[&real]));
    let d0 = drawn_rows(before, &real);
    assert!(d0.dropped.is_empty() && d0.rows.len() == 1 && d0.rows[0].drawn_checks == 0);
    let _ = std::fs::remove_dir_all(&dir);
}
