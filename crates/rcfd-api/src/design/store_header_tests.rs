//! ADR 0644 TH1, TH2, SB1: the store header after a kill inside the first append.
//!
//! The LG study and its helpers are copies of those of `study_ledger_tests.rs` (ADR 0623 "Set-ups"), which are private.

use super::*;
use crate::design::ledger::{Fidelity, Ledger};
use crate::design::optim::Stop;
use crate::design::study::{
    Condition, Doe, FnEvaluator, Method, ObjectiveDef, ParamDef, Robust, Sense, Study, StudyDef,
};
use crate::design::study_ledger::{pair_of, LedgerSink};
use std::collections::BTreeSet;

const STUDY: &str = "lg";

fn clock() -> u64 {
    1_700_000_000_000
}

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
    let d = std::env::temp_dir().join(format!("rcfd_f705_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger(dir: &Path) -> Ledger {
    let mut l = Ledger::new(&dir.join("ledger.jsonl"));
    l.clock = clock;
    l
}

fn nm() -> Method {
    Method::NelderMead {
        budget: 16,
        restarts: 0,
        seed: 623_005,
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

fn expected_pairs(recs: &[Record]) -> BTreeSet<(String, String)> {
    recs.iter()
        .flat_map(|r| r.conditions.iter().map(|c| (r.key.clone(), c.label.clone())))
        .collect()
}

/// FNV-1a 64 of `bytes`.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Run the LG5 study in `dir` with the sink, killed at write event `kill` (if any); returns the killed file.
fn lg5_run(dir: &Path, kill: Option<usize>) -> Option<&'static str> {
    let ev = FnEvaluator(lg_eval);
    let l = ledger(dir);
    let mut s = Study::new(def(1), Store::open(&dir.join("store.jsonl"), STUDY).unwrap(), &ev).unwrap();
    s.set_ledger_sink(sink(&l)).unwrap();
    s.test_kill_at = kill;
    let r = s.run(&nm());
    let file = *s.test_killed.lock().unwrap();
    match kill {
        None => {
            r.unwrap();
        }
        Some(e) => assert!(
            matches!(&r, Err(Stop::Error(m)) if m.contains("test kill")),
            "event {e}: {r:?}"
        ),
    }
    file
}

/// Whether the first line of `text` is the store header of signature `sig`.
fn header_ok(text: &str, sig: &str) -> bool {
    let Some(first) = text.lines().next() else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(first) else {
        return false;
    };
    v.get("format").and_then(Value::as_str) == Some(FORMAT)
        && v.get("signature").and_then(Value::as_str) == Some(sig)
        && v.get("version").and_then(Value::as_u64) == Some(1)
}

/// TH1 at every byte (ADR 0644): about 130 s in release, so a heavy record run once; the fast test below covers the same
/// path at the header bytes and the full first append.
#[test]
#[ignore = "heavy record (ADR 0644 TH1, about 130 s in release)"]
fn th1_kill_at_every_byte_of_the_first_append_resumes_with_the_header() {
    th1(&|_h, t| (0..=t).collect(), "TH1");
}

/// TH1 fast: `p` in `0..=|H|` and `p = |T|`.
#[test]
fn th1_fast_kill_in_the_header_and_after_the_first_append_resumes_with_the_header() {
    th1(&|h, t| (0..=h).chain([t]).collect(), "TH1 fast");
}

fn th1(select: &dyn Fn(usize, usize) -> Vec<usize>, tag: &str) {
    // reference: the uninterrupted run
    let rdir = tmp("th1ref");
    assert_eq!(lg5_run(&rdir, None), None);
    let b = std::fs::read(rdir.join("store.jsonl")).unwrap();
    let reference = valid_lines(&ledger(&rdir));
    let ref_records = Store::open_read(&rdir.join("store.jsonl"), STUDY)
        .unwrap()
        .records()
        .to_vec();
    // T: the store after the first store write (killed at the ledger write that follows it)
    let tdir = tmp("th1t");
    assert_eq!(lg5_run(&tdir, Some(2)), Some("ledger"));
    let t = std::fs::read(tdir.join("store.jsonl")).unwrap();
    let t_text = String::from_utf8(t.clone()).unwrap();
    let t_lines = t_text.lines().count();
    assert!(header_ok(&t_text, STUDY) && t_lines >= 2 && t.ends_with(b"\n"));
    assert!(b.starts_with(&t));
    let h = t_text.find('\n').unwrap() + 1;
    let _ = std::fs::remove_dir_all(&tdir);
    let ev = FnEvaluator(lg_eval);
    let mut points = 0;
    let mut bad = Vec::new();
    let ps = select(h, t.len());
    for &p in &ps {
        points += 1;
        let dir = tmp("th1p");
        let store = dir.join("store.jsonl");
        assert_eq!(lg5_run(&dir, Some(1)), Some("store"));
        assert!(!store.exists());
        std::fs::write(&store, &t[..p]).unwrap();
        let l = ledger(&dir);
        let mut why = Vec::new();
        match Store::open(&store, STUDY) {
            Err(e) => why.push(format!("open after kill: {e}")),
            Ok(st) => {
                let mut s = Study::new(def(1), st, &ev).unwrap();
                s.set_ledger_sink(sink(&l)).unwrap();
                if let Err(e) = s.run(&nm()) {
                    why.push(format!("resume: {e:?}"));
                }
            }
        }
        let bytes = std::fs::read(&store).unwrap_or_default();
        if !header_ok(&String::from_utf8_lossy(&bytes), STUDY) {
            why.push("no header".into());
        }
        if bytes != b {
            why.push(format!("bytes differ ({} vs {})", bytes.len(), b.len()));
        }
        match Store::open(&store, STUDY) {
            Err(e) => why.push(format!("reopen: {e}")),
            Ok(st) => {
                if st.records() != ref_records.as_slice() {
                    why.push("records differ".into());
                }
            }
        }
        let c = l.read().unwrap();
        let pairs: Vec<_> = c.entries.iter().filter_map(|e| pair_of(e, STUDY)).collect();
        let unique: BTreeSet<_> = pairs.iter().cloned().collect();
        let want = expected_pairs(&ref_records);
        let (dup, missing, extra) = (
            pairs.len() - unique.len(),
            want.difference(&unique).count(),
            unique.difference(&want).count(),
        );
        if dup + missing + extra != 0 || valid_lines(&l) != reference || !c.skipped.is_empty() {
            why.push(format!(
                "ledger dup {dup} missing {missing} extra {extra} skipped {}",
                c.skipped.len()
            ));
        }
        if !why.is_empty() {
            bad.push((p, why.join("; ")));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    let bad_p: Vec<usize> = bad.iter().map(|x| x.0).collect();
    eprintln!(
        "{tag}: |T| = {} bytes ({} lines, header {h} bytes), {points} kill points, {} failing at p = {bad_p:?}; first: {:?}",
        t.len(),
        t_lines,
        bad.len(),
        bad.first()
    );
    let _ = std::fs::remove_dir_all(&rdir);
    assert_eq!(points, ps.len());
    assert!(bad.is_empty());
}

fn rec(i: usize, x: f64) -> Record {
    let mut m = BTreeMap::new();
    m.insert("f".to_string(), x * x);
    Record {
        index: i,
        key: key_of(&[x]),
        params: vec![("x".into(), x)],
        source: "evaluated".into(),
        conditions: vec![CondResult::success("base", m)],
        message: String::new(),
    }
}

#[test]
fn th2_append_on_a_torn_or_empty_header_writes_the_header() {
    let d = tmp("th2");
    let fresh = d.join("fresh.jsonl");
    Store::open(&fresh, "sig").unwrap().append(vec![rec(0, 0.5)]).unwrap();
    let want = std::fs::read(&fresh).unwrap();
    let text = String::from_utf8(want.clone()).unwrap();
    let hl = text.find('\n').unwrap() + 1;
    let mut bad = Vec::new();
    for p in 0..hl {
        let f = d.join(format!("torn{p}.jsonl"));
        std::fs::write(&f, &want[..p]).unwrap();
        let ok = Store::open(&f, "sig")
            .and_then(|mut s| s.append(vec![rec(0, 0.5)]))
            .is_ok()
            && std::fs::read(&f).unwrap() == want
            && Store::open(&f, "sig").map(|s| s.records().len()) == Ok(1);
        if !ok {
            bad.push(p);
        }
    }
    eprintln!("TH2: header {hl} bytes, {} failing at p = {bad:?}", bad.len());
    let _ = std::fs::remove_dir_all(&d);
    assert!(bad.is_empty());
}

/// ADR 0644 SB1 digests, measured on the library before D1 (commit cb329680, log `f705-scratch/nc1_sb1_unfixed.log`).
const SB1_DIGESTS: [u64; 5] = [
    0x2431_3292_8430_f7ff,
    0xd046_29e2_d4b8_9b60,
    0x62d3_f217_75a7_30f4,
    0x57a5_02a8_837e_6f5f,
    0x57a5_02a8_837e_6f5f,
];

#[test]
fn sb1_store_bytes_of_the_existing_paths_are_unchanged() {
    let mut got = [0u64; 5];
    // S1: the scenario of round_trip_is_exact_and_partial_last_line_is_dropped
    let d1 = tmp("sb1s1");
    let p = d1.join("evaluations.jsonl");
    let xs = [0.1, 1.0 / 3.0, 2.0f64.sqrt(), -1e-300, 123456.789];
    let mut s = Store::open(&p, "sig").unwrap();
    s.append(xs.iter().enumerate().map(|(i, &x)| rec(i, x)).collect())
        .unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, &text[..text.len() - 20]).unwrap();
    let mut s3 = Store::open(&p, "sig").unwrap();
    s3.append(vec![rec(4, xs[4])]).unwrap();
    got[0] = fnv(&std::fs::read(&p).unwrap());
    // S2: the LG study (LHS 32), workers 1, with the sink: store and ledger
    let d2 = tmp("sb1s2");
    {
        let ev = FnEvaluator(lg_eval);
        let l = ledger(&d2);
        let mut s = Study::new(def(1), Store::open(&d2.join("store.jsonl"), STUDY).unwrap(), &ev).unwrap();
        s.set_ledger_sink(sink(&l)).unwrap();
        s.run(&Method::Doe(Doe::Lhs { n: 32, seed: 623_001 })).unwrap();
    }
    got[1] = fnv(&std::fs::read(d2.join("store.jsonl")).unwrap());
    got[2] = fnv(&std::fs::read(d2.join("ledger.jsonl")).unwrap());
    // S3: the uninterrupted LG5 store
    let d3 = tmp("sb1s3");
    lg5_run(&d3, None);
    got[3] = fnv(&std::fs::read(d3.join("store.jsonl")).unwrap());
    // S4: the LG5 store resumed after the kill before write event 3
    let d4 = tmp("sb1s4");
    lg5_run(&d4, Some(3));
    lg5_run(&d4, None);
    got[4] = fnv(&std::fs::read(d4.join("store.jsonl")).unwrap());
    eprintln!("SB1 digests: {got:#018x?}");
    for d in [d1, d2, d3, d4] {
        let _ = std::fs::remove_dir_all(d);
    }
    assert_eq!(got, SB1_DIGESTS);
}
