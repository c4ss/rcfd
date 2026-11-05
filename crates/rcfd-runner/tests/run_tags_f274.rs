//! ADR 0189 C-RR (the runner part): `Client::submit` writes a `run.json` with the keys main eef6c09 writes, and
//! `Client::submit_tagged` writes those keys plus `study`, `variant` and `condition` with the given values; the rest of the
//! run (snapshot, status, creation event) is the same.

use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use rcfd_runner::store::RunTags;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The keys of `run.json` as main eef6c09's `FsStore::create_run` writes them.
const MAIN_KEYS: [&str; 8] = [
    "id",
    "case",
    "case_snapshot_hash",
    "solver_version",
    "settings",
    "created",
    "case_source",
    "case_dir",
];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-runner-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    dir
}

fn run_json(dir: &Path, id: &str) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("runs").join(id).join("run.json")).unwrap()).unwrap()
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn c_rr_submit_writes_main_keys_and_submit_tagged_adds_the_three() {
    let dir = scratch("tags-f274");
    let case = dir.join("cases").join("c.toml");
    std::fs::write(&case, "title = \"t\"\n").unwrap();
    // no runner is started: submit only queues (the runner executable is never run here)
    let client = Client::new(&dir, dir.join("no-runner-here"));
    let settings = RunSettings {
        set: vec!["physics.nu=0.015".into()],
        ..RunSettings::default()
    };
    let plain = client.submit(&case, &settings).unwrap();
    let tags = RunTags {
        study: Some("designs/nu3".into()),
        variant: Some("nu=0.015".into()),
        condition: Some("base".into()),
    };
    let tagged = client.submit_tagged(&case, &settings, &tags).unwrap();
    let (a, b) = (run_json(&dir, &plain), run_json(&dir, &tagged));
    let mut want: Vec<String> = MAIN_KEYS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(keys(&a), want, "submit writes the keys of main");
    let mut want_tagged = want.clone();
    want_tagged.extend(["study", "variant", "condition"].map(String::from));
    want_tagged.sort();
    assert_eq!(keys(&b), want_tagged);
    assert_eq!(b["study"], json!("designs/nu3"));
    assert_eq!(b["variant"], json!("nu=0.015"));
    assert_eq!(b["condition"], json!("base"));
    // everything else is what submit writes
    for k in [
        "case",
        "case_snapshot_hash",
        "solver_version",
        "settings",
        "case_source",
        "case_dir",
    ] {
        assert_eq!(a[k], b[k], "{k}");
    }
    assert_eq!(a["settings"]["set"], json!(["physics.nu=0.015"]));
    for id in [&plain, &tagged] {
        let run = dir.join("runs").join(id);
        assert_eq!(std::fs::read(run.join("case.snapshot")).unwrap(), b"title = \"t\"\n");
        let st = client.status(id).unwrap();
        assert_eq!(rcfd_runner::client::state_name(st.state), "queued");
        let ev = std::fs::read_to_string(run.join("events.jsonl")).unwrap();
        assert_eq!(ev.lines().count(), 1, "one creation event: {ev}");
    }
    // the records read back through the run store, tags included
    let rec: rcfd_project::RunRecord = serde_json::from_value(b).unwrap();
    assert_eq!(rec.variant.as_deref(), Some("nu=0.015"));
    eprintln!("C-RR runner: submit keys {:?}", keys(&a));
    let _ = std::fs::remove_dir_all(&dir);
}
