//! ADR 0130 A3 (non-finite values read as NaN) and A4 (a run folder written by the 1248a76 run service is read).
//!
//! Only interfaces that existed before ADR 0130 are used, so this file also runs on the pre-change commit, where it
//! shows the negative controls (the runs are skipped).

mod common;

use rcfd_project::{atomic, jsonl, Project, ResidualLine, RunState};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// `ResidualLine` as it was before ADR 0130 (strict `f64`).
#[derive(Debug, Deserialize)]
struct OldResidualLine {
    step: u64,
    time: f64,
    residuals: BTreeMap<String, f64>,
    #[serde(default)]
    monitors: BTreeMap<String, f64>,
}

/// `RunStatus` as it was before ADR 0130 (strict `f64` time).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldRunStatus {
    state: RunState,
    step: u64,
    time: f64,
    #[serde(default)]
    pid: Option<u32>,
    updated: String,
}

/// `RunRecord` as it was before ADR 0130 (no `case_source` alias, required string `solver_version`).
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OldRunRecord {
    id: String,
    case: String,
    case_snapshot_hash: String,
    solver_version: String,
    settings: serde_json::Value,
    created: String,
}

fn bits(m: &BTreeMap<String, f64>) -> Vec<(String, u64)> {
    m.iter().map(|(k, v)| (k.clone(), v.to_bits())).collect()
}

/// A3: a diverged run's `null` residuals, status time and metrics read as NaN; the run is listed; finite lines read
/// bit-exactly as the old reader read them. Negative control: the old reader refuses the null status and line.
#[test]
fn a3_non_finite_values_read_as_nan() {
    let dir = common::scratch("a3");
    let p = Project::create(&dir.join("proj"), "a3", "0").unwrap();
    std::fs::write(p.root().join("cases/a.json"), b"{}").unwrap();
    let r = p.create_run("cases/a.json", None, "0", serde_json::json!({})).unwrap();
    let rd = p.run_dir(&r.id).unwrap();
    // what a diverged run leaves: serde_json writes NaN and infinity as null
    let status = r#"{"state": "failed", "step": 3, "time": null, "pid": null, "updated": "u", "reason": "diverged"}"#;
    std::fs::write(rd.join("status.json"), status).unwrap();
    std::fs::write(rd.join("metrics.json"), r#"{"dp": null, "eta": 0.5, "label": "x"}"#).unwrap();
    let finite = [
        r#"{"step":1,"time":0.0,"residuals":{"p":1.0,"u":0.30000000000000004},"monitors":{"m":-0.0}}"#,
        r#"{"step":2,"time":5e-324,"residuals":{"p":1e300,"u":2.2250738585072014e-308},"monitors":{}}"#,
    ];
    let lines = [
        finite[0],
        finite[1],
        r#"{"step":3,"time":null,"residuals":{"p":null,"u":1.5},"monitors":{"m":null}}"#,
    ];
    std::fs::write(rd.join("residuals.jsonl"), lines.join("\n") + "\n").unwrap();

    let runs = p.runs().unwrap();
    eprintln!(
        "A3: listed {}, skipped {}: {:?}",
        runs.runs.len(),
        runs.skipped.len(),
        runs.skipped
    );
    assert_eq!((runs.runs.len(), runs.skipped.len()), (1, 0));
    let s = &runs.runs[0];
    assert_eq!((s.state, s.step), (RunState::Failed, 3));
    assert!(s.time.is_nan(), "status time {}", s.time);
    assert!(s.metrics["dp"].is_nan() && s.metrics["eta"] == 0.5, "{:?}", s.metrics);
    assert!(!s.metrics.contains_key("label"));

    let res = jsonl::read::<ResidualLine>(&rd.join("residuals.jsonl")).unwrap();
    assert_eq!(res.items.len(), 3);
    let last = &res.items[2];
    assert!(last.time.is_nan() && last.residuals["p"].is_nan() && last.monitors["m"].is_nan());
    assert_eq!(last.residuals["u"], 1.5);
    for (i, l) in finite.iter().enumerate() {
        let old: OldResidualLine = serde_json::from_str(l).unwrap();
        let new = &res.items[i];
        assert_eq!(new.step, old.step);
        assert_eq!(new.time.to_bits(), old.time.to_bits(), "line {i} time");
        assert_eq!(bits(&new.residuals), bits(&old.residuals), "line {i}");
        assert_eq!(bits(&new.monitors), bits(&old.monitors), "line {i}");
    }
    // negative control: the pre-change reader refuses the null status (the run would be skipped) and the null line
    assert!(serde_json::from_str::<OldRunStatus>(status).is_err());
    assert!(serde_json::from_str::<OldResidualLine>(lines[2]).is_err());
}

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/legacy_runner_1248a76");
const FIXTURE_ID: &str = "r-20261028T010219-cc06";

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

/// A4: a run folder written by main 1248a76's run service (fixture copied from a real run) is listed, `case` comes from
/// `case_source`, `solver_version` is unknown, and `validate()` finds nothing (the `fnv1a64:` hash is accepted).
/// Negative controls: the pre-change record type refuses its `run.json`, and the pre-change hash comparison reports
/// the snapshot.
#[test]
fn a4_legacy_runner_run_is_read() {
    let dir = common::scratch("a4");
    let p = Project::create(&dir.join("proj"), "a4", "0").unwrap();
    let rd = p.root().join("runs").join(FIXTURE_ID);
    copy_dir(&Path::new(FIXTURE).join(FIXTURE_ID), &rd);
    let raw: serde_json::Value = atomic::read_json(&rd.join("run.json")).unwrap();
    let case_source = raw["case_source"].as_str().unwrap().to_string();
    assert!(
        raw.get("case").is_none() && raw["solver_version"].is_null(),
        "fixture: {raw}"
    );

    let runs = p.runs().unwrap();
    let issues = p.validate().unwrap();
    eprintln!(
        "A4: listed {}, skipped {}: {:?}; validate: {} issues {:?}",
        runs.runs.len(),
        runs.skipped.len(),
        runs.skipped,
        issues.len(),
        issues
    );
    assert_eq!((runs.runs.len(), runs.skipped.len()), (1, 0));
    let s = &runs.runs[0];
    assert_eq!((s.id.as_str(), s.state, s.step), (FIXTURE_ID, RunState::Finished, 20));
    assert_eq!(s.case, case_source);
    assert_eq!(p.read_run(FIXTURE_ID).unwrap().solver_version, "unknown");
    assert!(issues.is_empty(), "{issues:?}");
    let ev = p.read_events(FIXTURE_ID).unwrap();
    assert_eq!(ev.items.len(), 8);
    assert!(ev.torn.is_none());

    // negative controls: without the alias the record does not parse (the run was skipped); without the legacy
    // spelling the snapshot hash does not compare equal (one snapshot issue)
    assert!(serde_json::from_value::<OldRunRecord>(raw.clone()).is_err());
    let snap = std::fs::read(rd.join("case.snapshot")).unwrap();
    let recorded = raw["case_snapshot_hash"].as_str().unwrap();
    assert!(recorded.starts_with("fnv1a64:"));
    assert_ne!(rcfd_project::records::hash_bytes(&snap), recorded);
    assert_eq!(
        rcfd_project::records::hash_bytes(&snap).strip_prefix("fnv64:"),
        recorded.strip_prefix("fnv1a64:"),
        "the two spellings name the same FNV-1a 64 value"
    );
}
