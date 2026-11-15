//! ADR 0623 LG1 to LG8: the opt-in ledger sink of design studies.

use super::*;
use crate::design::ledger::rerun_check;
use crate::design::optim::{OptResult, Stop};
use crate::design::store::Store;
use crate::design::study::{Condition, Doe, FnEvaluator, Method, ObjectiveDef, ParamDef, Robust, Sense, Study};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const STUDY: &str = "lg";

fn clock() -> u64 {
    1_700_000_000_000
}

/// The LG study of ADR 0623 ("Set-ups").
fn def(workers: usize) -> StudyDef {
    let p = |name: &str| ParamDef {
        name: name.into(),
        lo: 0.0,
        hi: 1.0,
        init: None,
        targets: vec![],
    };
    StudyDef {
        name: STUDY.into(),
        params: vec![p("x"), p("y")],
        conditions: [("c_lo", 0.25), ("c_hi", 0.75)]
            .iter()
            .map(|(l, c)| Condition {
                label: (*l).into(),
                set: vec![("c".into(), json!(c))],
            })
            .collect(),
        objectives: vec![ObjectiveDef {
            metric: "f".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers,
    }
}

fn lg_eval(x: &[f64], c: &Condition) -> CondResult {
    let cv = c.set[0].1.as_f64().unwrap();
    let s = (32.0 * x[0]).floor() as i64;
    if s <= 3 {
        CondResult::failure(&c.label, "diverged", "analytic: x < 1/8 diverges")
    } else if s <= 7 {
        CondResult::failure(&c.label, "not_evaluated", "analytic: x in [1/8, 1/4) not evaluated")
    } else {
        let mut m = BTreeMap::new();
        m.insert("f".to_string(), (x[0] - cv).powi(2) + 0.01 * x[1]);
        m.insert("g".to_string(), x[0] * cv);
        CondResult::success(&c.label, m)
    }
}

fn units(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect()
}

fn sink(l: &Ledger) -> LedgerSink<'_> {
    LedgerSink {
        ledger: l,
        evaluator: "analytic.lg".into(),
        evaluator_version: "1".into(),
        fidelity: Fidelity::Analytic,
        param_units: units(&[("x", "m"), ("y", "1")]),
        metric_units: units(&[("f", "m2"), ("g", "m")]),
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f268r_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger(dir: &Path) -> Ledger {
    let mut l = Ledger::new(&dir.join("ledger.jsonl"));
    l.clock = clock;
    l
}

fn doe() -> Method {
    Method::Doe(Doe::Lhs { n: 32, seed: 623_001 })
}

fn nm() -> Method {
    Method::NelderMead {
        budget: 16,
        restarts: 0,
        seed: 623_005,
    }
}

/// Run `method` on the LG study with a file store in `dir`, with or without the sink.
fn run_in(dir: &Path, workers: usize, with_sink: bool, method: &Method) -> (OptResult, Vec<Record>) {
    let ev = FnEvaluator(lg_eval);
    let l = ledger(dir);
    let mut s = Study::new(def(workers), Store::open(&dir.join("store.jsonl"), "lg").unwrap(), &ev).unwrap();
    if with_sink {
        assert_eq!(s.set_ledger_sink(sink(&l)).unwrap(), 0);
    }
    let r = s.run(method).unwrap();
    (r, s.store.records().to_vec())
}

fn pairs_of(c: &crate::design::ledger::Contents) -> Vec<(String, String)> {
    c.entries.iter().filter_map(|e| pair_of(e, STUDY)).collect()
}

fn expected_pairs(recs: &[Record]) -> BTreeSet<(String, String)> {
    recs.iter()
        .flat_map(|r| r.conditions.iter().map(|c| (r.key.clone(), c.label.clone())))
        .collect()
}

#[test]
fn lg1_lg3_lg4_one_entry_per_pair_with_located_failures_and_exact_reruns() {
    let dir = tmp("lg1");
    let (_, recs) = run_in(&dir, 1, true, &doe());
    let l = ledger(&dir);
    let c = l.read().unwrap();
    let d = def(1);
    // LG1
    let failed_candidates = recs.iter().filter(|r| !r.ok()).count();
    let pairs = pairs_of(&c);
    let unique: BTreeSet<_> = pairs.iter().cloned().collect();
    let mut content_problems = Vec::new();
    for e in &c.entries {
        let (key, label) = pair_of(e, STUDY).unwrap();
        let r = recs.iter().find(|r| r.key == key).unwrap();
        let cr = r.conditions.iter().find(|x| x.label == label).unwrap();
        let cond = d.conditions.iter().find(|x| x.label == label).unwrap();
        let want_params: Vec<Quantity> = d
            .params
            .iter()
            .map(|p| {
                let v = r.params.iter().find(|(n, _)| *n == p.name).unwrap().1;
                Quantity::new(&p.name, v, if p.name == "x" { "m" } else { "1" })
            })
            .collect();
        let want_metrics: Vec<Quantity> = cr
            .metrics
            .iter()
            .map(|(k, &v)| Quantity::new(k, v, if k == "f" { "m2" } else { "m" }))
            .collect();
        let ok = e.kind == "evaluation"
            && e.params.len() == 2
            && e.params
                .iter()
                .zip(&want_params)
                .all(|(a, b)| a.name == b.name && a.unit == b.unit && a.value.to_bits() == b.value.to_bits())
            && e.metrics.len() == want_metrics.len()
            && e.metrics
                .iter()
                .zip(&want_metrics)
                .all(|(a, b)| a.name == b.name && a.unit == b.unit && a.value.to_bits() == b.value.to_bits())
            && e.settings["condition"] == json!(label)
            && e.settings["set"] == json!({"c": cond.set[0].1})
            && (cr.ok == (e.verdict == Verdict::Ok))
            && (cr.ok || (e.reason.contains(&format!("'{label}'")) && e.reason.contains(&cr.message)));
        if !ok {
            content_problems.push(e.variant.clone());
        }
    }
    eprintln!(
        "LG1: {} entries, {} unique pairs of {} expected, {} skipped lines, {failed_candidates} failed candidates, content \
         problems {content_problems:?}",
        c.entries.len(),
        unique.len(),
        expected_pairs(&recs).len(),
        c.skipped.len()
    );
    assert_eq!(recs.len(), 32);
    assert_eq!(failed_candidates, 8, "set-up: 8 failing candidates");
    assert_eq!(c.entries.len(), 64);
    assert!(c.skipped.is_empty());
    assert_eq!(unique, expected_pairs(&recs));
    assert_eq!(pairs.len(), 64);
    assert!(content_problems.is_empty());
    // LG4
    let n = |v: Verdict| c.entries.iter().filter(|e| e.verdict == v).count();
    let store_failed = recs.iter().flat_map(|r| &r.conditions).filter(|x| !x.ok).count();
    eprintln!(
        "LG4: diverged {}, not_evaluated {}, store failed pairs {store_failed}",
        n(Verdict::Diverged),
        n(Verdict::NotEvaluated)
    );
    assert_eq!(n(Verdict::Diverged), 8);
    assert_eq!(n(Verdict::NotEvaluated), 8);
    assert_eq!(n(Verdict::Diverged) + n(Verdict::NotEvaluated), store_failed);
    // LG3
    let ev = FnEvaluator(lg_eval);
    let sk = sink(&l);
    let (mut passed, mut ok_entries, mut largest, mut same_verdict) = (0, 0, 0.0f64, 0);
    for e in &c.entries {
        let label = e.provenance["condition"].as_str().unwrap();
        let m = ConditionModel::of(&d, &ev, &sk, label);
        if e.verdict == Verdict::Ok {
            ok_entries += 1;
            let r = rerun_check(e, &m, 0.0, 0.0);
            if r.pass {
                passed += 1;
            }
            for x in &r.metrics {
                largest = largest.max(x.3);
            }
        } else if m.evaluate(&e.params).verdict == e.verdict {
            same_verdict += 1;
        }
    }
    eprintln!("LG3: {passed} of {ok_entries} ok entries pass, largest difference {largest:e}; non-ok same verdict {same_verdict} of 16");
    assert_eq!(ok_entries, 48);
    assert_eq!(passed, 48);
    assert_eq!(largest, 0.0);
    assert_eq!(same_verdict, 16);
    // LG6: the finished study run again on its store adds nothing
    let mut s = Study::new(def(1), Store::open(&dir.join("store.jsonl"), "lg").unwrap(), &ev).unwrap();
    let rec = s.set_ledger_sink(sink(&l)).unwrap();
    s.run(&doe()).unwrap();
    let after = l.read().unwrap().entries.len();
    eprintln!("LG6: reconcile appended {rec}, entries after the rerun {after}");
    assert_eq!(rec, 0);
    assert_eq!(after, 64);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lg2_store_and_result_are_identical_with_and_without_the_sink() {
    for (tag, method) in [("doe", doe()), ("nm", nm())] {
        let (a, b) = (tmp(&format!("lg2a_{tag}")), tmp(&format!("lg2b_{tag}")));
        let (ra, _) = run_in(&a, 1, true, &method);
        let (rb, _) = run_in(&b, 1, false, &method);
        let sa = std::fs::read(a.join("store.jsonl")).unwrap();
        let sb = std::fs::read(b.join("store.jsonl")).unwrap();
        let bits = |r: &OptResult| {
            (
                r.x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                r.fitness.f.to_bits(),
                r.fitness.violation.to_bits(),
                r.evaluations,
                r.converged,
            )
        };
        eprintln!(
            "LG2 {tag}: store {} and {} bytes, equal {}; result equal {}; ledger written {}, not written {}",
            sa.len(),
            sb.len(),
            sa == sb,
            bits(&ra) == bits(&rb),
            a.join("ledger.jsonl").exists(),
            b.join("ledger.jsonl").exists()
        );
        assert_eq!(sa, sb);
        assert_eq!(bits(&ra), bits(&rb));
        assert!(!b.join("ledger.jsonl").exists());
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }
}

fn valid_lines(l: &Ledger) -> Vec<String> {
    let mut v: Vec<String> = l
        .read()
        .unwrap()
        .entries
        .iter()
        .map(|e| e.to_json().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn lg5_resume_after_a_kill_at_every_write_event() {
    let ev = FnEvaluator(lg_eval);
    // the uninterrupted run
    let rdir = tmp("lg5ref");
    let rl = ledger(&rdir);
    let mut s = Study::new(def(1), Store::open(&rdir.join("store.jsonl"), "lg").unwrap(), &ev).unwrap();
    s.set_ledger_sink(sink(&rl)).unwrap();
    s.run(&nm()).unwrap();
    let events = s.test_events.load(Ordering::SeqCst);
    drop(s);
    let reference = valid_lines(&rl);
    let mut points = 0;
    let mut bad = Vec::new();
    for e in 1..=events {
        for torn in [false, true] {
            if torn && e == 1 {
                continue; // the pre-existing store defect of ADR 0623 ("What exists")
            }
            points += 1;
            let dir = tmp(&format!("lg5_{e}_{torn}"));
            let l = ledger(&dir);
            let store = dir.join("store.jsonl");
            let mut s = Study::new(def(1), Store::open(&store, "lg").unwrap(), &ev).unwrap();
            s.set_ledger_sink(sink(&l)).unwrap();
            s.test_kill_at = Some(e);
            let r = s.run(&nm());
            let file = *s.test_killed.lock().unwrap();
            drop(s);
            assert!(
                matches!(&r, Err(Stop::Error(m)) if m.contains("test kill")),
                "event {e}: {r:?}"
            );
            if torn {
                let p = if file == Some("store") {
                    store.clone()
                } else {
                    l.path().to_path_buf()
                };
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).unwrap();
                std::io::Write::write_all(&mut f, b"{\"format\":\"rcfd-").unwrap();
            }
            // resume
            let mut s = Study::new(def(1), Store::open(&store, "lg").unwrap(), &ev).unwrap();
            s.set_ledger_sink(sink(&l)).unwrap();
            s.run(&nm()).unwrap();
            let c = l.read().unwrap();
            let pairs = pairs_of(&c);
            let unique: BTreeSet<_> = pairs.iter().cloned().collect();
            let want = expected_pairs(s.store.records());
            let dup = pairs.len() - unique.len();
            let missing = want.difference(&unique).count();
            let extra = unique.difference(&want).count();
            let same = valid_lines(&l) == reference;
            if dup != 0 || missing != 0 || extra != 0 || !same || c.skipped.len() > 1 {
                bad.push((e, torn, file, dup, missing, extra, same, c.skipped.len()));
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    eprintln!(
        "LG5: {events} write events, {points} kill points, {} reference entries; failing points (event, torn, file, dup, \
         missing, extra, same set, skipped) {bad:?}",
        reference.len()
    );
    assert_eq!(points, 2 * events - 1);
    assert!(bad.is_empty());
    let _ = std::fs::remove_dir_all(&rdir);
}

#[test]
fn lg7_ledger_bytes_do_not_depend_on_the_worker_count() {
    let (a, b) = (tmp("lg7a"), tmp("lg7b"));
    run_in(&a, 1, true, &doe());
    run_in(&b, 4, true, &doe());
    let la = std::fs::read(a.join("ledger.jsonl")).unwrap();
    let lb = std::fs::read(b.join("ledger.jsonl")).unwrap();
    eprintln!("LG7: {} and {} bytes, equal {}", la.len(), lb.len(), la == lb);
    assert_eq!(la, lb);
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn lg8_missing_units_are_refused() {
    let dir = tmp("lg8");
    let ev = FnEvaluator(lg_eval);
    let l = ledger(&dir);
    let mut s = Study::new(def(1), Store::memory("lg"), &ev).unwrap();
    let mut sk = sink(&l);
    sk.param_units.remove("y");
    let e1 = s.set_ledger_sink(sk).unwrap_err();
    let no_file = !l.path().exists();
    let mut sk = sink(&l);
    sk.metric_units.remove("g");
    s.set_ledger_sink(sk).unwrap();
    let r = s.run(&doe());
    let entries = l.read().unwrap().entries.len();
    eprintln!("LG8: '{e1}', no file {no_file}; run {r:?}; entries {entries}");
    assert!(e1.contains("'y'"));
    assert!(no_file);
    assert!(matches!(&r, Err(Stop::Error(m)) if m.contains("'g'")));
    assert_eq!(entries, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
