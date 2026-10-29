//! ADR 0122 R1 (record round-trip), M1 (migration hook), V1 (validation), Q1 (query API), I1 (immutability).

mod common;

use rcfd_project::atomic;
use rcfd_project::project::{migrate, Migration, MIGRATIONS};
use rcfd_project::records::{EventKind, SCHEMA_VERSION};
use rcfd_project::{Manifest, Metrics, Project, ProjectError, ResidualLine, RunEvent, RunRecord, RunState, RunStatus};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(v: &T) {
    let text = serde_json::to_string(v).unwrap();
    let back: T = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, v, "{text}");
}

/// A contract literal must parse and re-serialize to the same JSON value.
fn literal<T: Serialize + DeserializeOwned>(v: Value) -> T {
    let t: T = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(&t).unwrap(), v);
    t
}

const STATES: [RunState; 6] = [
    RunState::Queued,
    RunState::Running,
    RunState::Paused,
    RunState::Finished,
    RunState::Failed,
    RunState::Cancelled,
];

#[test]
fn r1_every_record_round_trips() {
    round_trip(&Manifest {
        schema_version: 1,
        name: "fan".into(),
        created: "2026-10-26T10:00:00.000Z".into(),
        solver_version_created: "0.1.0".into(),
    });
    let rec = RunRecord {
        id: "r-20261026T100000-0a1f".into(),
        case: "cases/fan.json".into(),
        case_snapshot_hash: "fnv64:0123456789abcdef".into(),
        mesh: Some("meshes/fan.msh".into()),
        mesh_hash: Some("fnv64:fedcba9876543210".into()),
        solver_version: "0.1.0".into(),
        settings: json!({"threads": 4, "tol": 1e-8, "nested": {"a": [1, 2.5, null]}}),
        created: "2026-10-26T10:00:00.123Z".into(),
        case_source: None,
        case_dir: None,
    };
    round_trip(&rec);
    round_trip(&RunRecord {
        mesh: None,
        mesh_hash: None,
        ..rec
    });
    for (i, s) in STATES.into_iter().enumerate() {
        round_trip(&RunStatus {
            state: s,
            step: i as u64 * 7,
            time: 0.1 + i as f64 / 3.0,
            pid: (i % 2 == 0).then_some(4242),
            updated: "x".into(),
            reason: None,
            exit_code: None,
        });
    }
    for kind in [
        EventKind::State {
            from: None,
            to: RunState::Queued,
            reason: None,
        },
        EventKind::State {
            from: Some(RunState::Running),
            to: RunState::Finished,
            reason: None,
        },
        EventKind::Warning {
            message: "line\nbreak \"quoted\"".into(),
        },
        EventKind::Stop {
            reason: "converged".into(),
        },
    ] {
        round_trip(&RunEvent {
            t: "2026-10-26T10:00:00.000Z".into(),
            kind,
        });
    }
    round_trip(&ResidualLine {
        step: 12,
        time: 1.0 / 3.0,
        residuals: [("p".to_string(), 1.234e-7), ("Ux".to_string(), f64::MIN_POSITIVE)].into(),
        monitors: [("dp".to_string(), -12.5)].into(),
        correctors: None,
        corrector_cap: None,
    });
    // ADR 0160 R3: the corrector count of an adaptive run's line
    round_trip(&ResidualLine {
        step: 13,
        time: 0.5,
        residuals: [("p".to_string(), 1e-6)].into(),
        monitors: Default::default(),
        correctors: Some(5),
        corrector_cap: Some(0),
    });
    round_trip(&Metrics::from([
        ("dp".to_string(), 101.325),
        ("eta".to_string(), 0.1 + 0.2),
    ]));

    // Contract literals (field names fixed by docs/ui-restart-plan.md section 8).
    literal::<Manifest>(json!({"schema_version": 1, "name": "p", "created": "c", "solver_version_created": "v"}));
    literal::<RunStatus>(json!({"state": "cancelled", "step": 3, "time": 0.5, "pid": null, "updated": "u"}));
    literal::<RunEvent>(json!({"t": "t", "kind": "state", "from": "running", "to": "paused"}));
    literal::<RunEvent>(json!({"t": "t", "kind": "stop", "reason": "max steps"}));
    literal::<ResidualLine>(json!({"step": 1, "time": 0.0, "residuals": {"p": 0.5}, "monitors": {}}));
    literal::<ResidualLine>(
        json!({"step": 2, "time": 0.1, "residuals": {"p": 0.5}, "monitors": {}, "correctors": 8, "corrector_cap": 1}),
    );
    // ADR 0160 R3: a line without the count reads with None and writes back byte for byte
    let old = r#"{"step":3,"time":0.25,"residuals":{"continuity":1e-12,"p":0.5,"u":0.25},"monitors":{"cd":1.25}}"#;
    let l: ResidualLine = serde_json::from_str(old).unwrap();
    assert_eq!((l.correctors, l.corrector_cap), (None, None));
    assert_eq!(serde_json::to_string(&l).unwrap(), old);
    // Additive growth: an unknown field is accepted.
    let _: RunStatus = serde_json::from_value(
        json!({"state": "running", "step": 1, "time": 0.0, "pid": 7, "updated": "u", "eta_s": 30}),
    )
    .unwrap();
}

#[test]
fn r1_negative_controls_refuse_broken_records() {
    // Missing required field, unknown state, wrong type, an event line without a kind. (ADR 0130 amendment 1: an
    // unknown event kind is read as `EventKind::Other` now; a line with no tag and a state change to an unknown
    // state are refused instead.)
    assert!(serde_json::from_value::<RunStatus>(json!({"step": 1, "time": 0.0, "pid": null, "updated": "u"})).is_err());
    assert!(serde_json::from_value::<RunStatus>(
        json!({"state": "done", "step": 1, "time": 0.0, "pid": null, "updated": "u"})
    )
    .is_err());
    assert!(serde_json::from_value::<RunStatus>(
        json!({"state": "queued", "step": "1", "time": 0.0, "pid": null, "updated": "u"})
    )
    .is_err());
    assert!(serde_json::from_value::<RunEvent>(json!({"t": "t"})).is_err());
    assert!(
        serde_json::from_value::<RunEvent>(json!({"t": "t", "kind": "state", "from": null, "to": "done"})).is_err()
    );
    assert!(serde_json::from_value::<RunRecord>(
        json!({"id": "x", "case": "c", "solver_version": "v", "settings": {}, "created": "c"})
    )
    .is_err());
    // A located parse error names the file.
    let dir = common::scratch("r1neg");
    let p = dir.join("status.json");
    std::fs::write(&p, b"{\"state\": \"queued\",").unwrap();
    let e = atomic::read_json::<RunStatus>(&p).unwrap_err();
    assert!(matches!(&e, ProjectError::Json { path, .. } if path == &p), "{e}");
    assert!(e.to_string().contains("status.json"), "{e}");
}

#[test]
fn i1_run_json_is_never_replaced() {
    let dir = common::scratch("i1");
    let p = dir.join("run.json");
    atomic::write_json_new(&p, &json!({"a": 1})).unwrap();
    let e = atomic::write_json_new(&p, &json!({"a": 2})).unwrap_err();
    assert!(matches!(&e, ProjectError::Exists { path } if path == &p), "{e}");
    assert_eq!(atomic::read_json::<Value>(&p).unwrap(), json!({"a": 1}));
    // Negative control: the replacing writer does replace.
    atomic::write_json(&p, &json!({"a": 3})).unwrap();
    assert_eq!(atomic::read_json::<Value>(&p).unwrap(), json!({"a": 3}));
    // No temp file is left behind by either path.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

fn v0_fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    // Constructed old schema: no version-0 project was ever written by rcfd.
    std::fs::write(
        dir.join("rcfd-project.json"),
        br#"{"schema_version": 0, "title": "legacy fan", "created_unix": 1700000000, "keep": true}"#,
    )
    .unwrap();
    for d in rcfd_project::project::DIRS {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
}

#[test]
fn m1_migration_hook_upgrades_old_schema_fixture() {
    let dir = common::scratch("m1");
    v0_fixture(&dir);
    let before = std::fs::read(dir.join("rcfd-project.json")).unwrap();
    let mut p = Project::open(&dir).unwrap();
    assert_eq!(p.migrated_from(), Some(0));
    assert_eq!(p.manifest().schema_version, SCHEMA_VERSION);
    assert_eq!(p.manifest().name, "legacy fan");
    assert_eq!(p.manifest().created, "2023-11-14T22:13:20.000Z");
    // Opening does not write; validation reports the unsaved migration.
    assert_eq!(std::fs::read(dir.join("rcfd-project.json")).unwrap(), before);
    assert_eq!(p.validate().unwrap().len(), 1);
    p.save_manifest().unwrap();
    let again = Project::open(&dir).unwrap();
    assert_eq!(again.migrated_from(), None);
    assert_eq!(again.manifest(), p.manifest());
    assert!(again.validate().unwrap().is_empty());
}

#[test]
fn m1_negative_controls() {
    let dir = common::scratch("m1neg");
    v0_fixture(&dir);
    // Without the hook the old fixture does not open.
    let none: [Migration; 0] = [];
    let e = Project::open_with(&dir, &none).unwrap_err();
    assert!(
        e.to_string().contains("no migration from schema_version 0") && e.to_string().contains("rcfd-project.json"),
        "{e}"
    );
    // The fixture is not readable as the current schema directly.
    let raw: Value = serde_json::from_slice(&std::fs::read(dir.join("rcfd-project.json")).unwrap()).unwrap();
    assert!(serde_json::from_value::<Manifest>(raw.clone()).is_err());
    // A newer schema is refused with a located error, never downgraded.
    let newer =
        json!({"schema_version": SCHEMA_VERSION + 1, "name": "x", "created": "c", "solver_version_created": "v"});
    let e = migrate(std::path::Path::new("p/rcfd-project.json"), newer, &MIGRATIONS).unwrap_err();
    assert!(
        e.to_string().contains("newer than supported") && e.to_string().contains("rcfd-project.json"),
        "{e}"
    );
    // A broken v0 fixture fails inside the hook with the step named.
    let e = migrate(
        std::path::Path::new("m.json"),
        json!({"schema_version": 0, "created_unix": 1}),
        &MIGRATIONS,
    )
    .unwrap_err();
    assert!(
        e.to_string().contains("migration 0 -> 1") && e.to_string().contains("title"),
        "{e}"
    );
}

fn project_with_case(name: &str) -> Project {
    let dir = common::scratch(name);
    let p = Project::create(&dir.join("proj"), name, "0.1.0").unwrap();
    std::fs::write(p.root().join("cases/a.json"), br#"{"case": "a"}"#).unwrap();
    std::fs::create_dir_all(p.root().join("cases/sub")).unwrap();
    std::fs::write(p.root().join("cases/sub/b.toml"), b"case = 'b'\n").unwrap();
    std::fs::write(p.root().join("meshes/m.msh"), b"mesh bytes").unwrap();
    std::fs::create_dir_all(p.root().join("designs/ledger1")).unwrap();
    p
}

#[test]
fn v1_validate_finds_each_planted_defect() {
    let p = project_with_case("v1");
    let r = p
        .create_run("cases/a.json", Some("meshes/m.msh"), "0.1.0", json!({}))
        .unwrap();
    assert_eq!(
        r.mesh_hash.as_deref(),
        Some(rcfd_project::records::hash_bytes(b"mesh bytes").as_str())
    );
    assert_eq!(
        std::fs::read(p.run_dir(&r.id).unwrap().join("case.snapshot")).unwrap(),
        br#"{"case": "a"}"#
    );
    assert!(p.validate().unwrap().is_empty(), "{:?}", p.validate());
    assert_eq!(p.cases().unwrap(), ["cases/a.json", "cases/sub/b.toml"]);
    assert_eq!(p.meshes().unwrap(), ["meshes/m.msh"]);
    assert_eq!(p.designs().unwrap(), ["designs/ledger1"]);
    // Negative controls: each planted defect is reported at its file.
    let dir = p.run_dir(&r.id).unwrap();
    std::fs::write(dir.join("case.snapshot"), b"edited").unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("events.jsonl"))
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"{\"t\":"))
        .unwrap();
    std::fs::write(dir.join(".status.json.1.2.tmp"), b"x").unwrap();
    std::fs::remove_dir(p.root().join("notes")).unwrap();
    let issues = p.validate().unwrap();
    let at = |f: &str| issues.iter().any(|i| i.path.ends_with(f));
    assert!(
        at("case.snapshot") && at("events.jsonl") && at(".status.json.1.2.tmp") && at("notes"),
        "{issues:?}"
    );
    assert_eq!(issues.len(), 4, "{issues:?}");
}

#[test]
fn q1_query_api_sorts_filters_and_finds_latest_finished() {
    let p = project_with_case("q1");
    let mut ids = Vec::new();
    for (case, state) in [
        ("cases/a.json", RunState::Finished),
        ("cases/a.json", RunState::Finished),
        ("cases/sub/b.toml", RunState::Finished),
        ("cases/a.json", RunState::Failed),
        ("cases/a.json", RunState::Running),
    ] {
        let r = p.create_run(case, None, "0.1.0", json!({"n": ids.len()})).unwrap();
        p.write_status(
            &r.id,
            &RunStatus {
                state,
                step: 10 * ids.len() as u64,
                time: 0.5,
                pid: None,
                updated: rcfd_project::time::now(),
                reason: None,
                exit_code: None,
            },
        )
        .unwrap();
        atomic::write_json(
            &p.run_dir(&r.id).unwrap().join("metrics.json"),
            &json!({"dp": ids.len() as f64, "label": "skip"}),
        )
        .unwrap();
        ids.push(r.id);
        std::thread::sleep(std::time::Duration::from_millis(3)); // distinct created ms
    }
    let runs = p.runs().unwrap();
    assert!(runs.skipped.is_empty());
    assert_eq!(
        runs.runs.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        ids,
        "oldest first"
    );
    assert_eq!(
        runs.runs[2].metrics,
        [("dp".to_string(), 2.0)].into(),
        "numeric metrics only"
    );
    assert_eq!(runs.runs[4].step, 40);
    assert_eq!(p.runs_in_state(RunState::Finished).unwrap().len(), 3);
    assert_eq!(p.runs_in_state(RunState::Paused).unwrap().len(), 0);
    // Later failed/running runs of a, and a later finished run of b, must not be chosen.
    assert_eq!(p.latest_finished("cases/a.json").unwrap().unwrap().id, ids[1]);
    assert_eq!(p.latest_finished("cases\\sub\\b.toml").unwrap().unwrap().id, ids[2]);
    assert!(p.latest_finished("cases/none.json").unwrap().is_none());
    // A half-created run directory is skipped and reported, not an error.
    std::fs::create_dir(p.root().join("runs/r-20990101T000000-0000")).unwrap();
    let runs = p.runs().unwrap();
    assert_eq!((runs.runs.len(), runs.skipped.len()), (5, 1));
    // Events: creation event then appended ones.
    p.append_event(
        &ids[0],
        &RunEvent {
            t: "t".into(),
            kind: EventKind::Stop { reason: "done".into() },
        },
    )
    .unwrap();
    let ev = p.read_events(&ids[0]).unwrap();
    assert_eq!(ev.items.len(), 2);
    assert_eq!(
        ev.items[0].kind,
        EventKind::State {
            from: None,
            to: RunState::Queued,
            reason: None,
        }
    );
    // A run id cannot be used to name another path.
    assert!(matches!(p.run_dir("../x"), Err(ProjectError::PathRefused { .. })));
}
