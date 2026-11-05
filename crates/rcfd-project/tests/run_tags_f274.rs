//! ADR 0189 C-RR (the record part): the three design-run fields of `run.json` (`study`, `variant`, `condition`) are written
//! only when set, round-trip, and leave records written before them, and readers made before them, as they were.

use rcfd_project::RunRecord;
use serde::Deserialize;
use serde_json::{json, Value};

/// A `run.json` of the form the run service of main eef6c09 writes (`FsStore::create_run`).
const MAIN_RUN_JSON: &str = r#"{
  "id": "r-20261105T120000-0a1f",
  "case": "cases/cavity.toml",
  "case_snapshot_hash": "fnv64:0123456789abcdef",
  "solver_version": "rcfd 0.1.0",
  "settings": {
    "priority": 0,
    "threads": 1,
    "mem_mb": null,
    "set": ["physics.nu=0.015"]
  },
  "created": "2026-11-05T12:00:00.123456789Z",
  "case_source": "E:/p/cases/cavity.toml",
  "case_dir": "E:/p/cases"
}"#;

const TAGS: [&str; 3] = ["study", "variant", "condition"];

/// A reader of the strict kind of `legacy_w48a.rs`: the record as it was before the three fields.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReaderBefore {
    id: String,
    case: String,
    case_snapshot_hash: String,
    solver_version: String,
    settings: Value,
    created: String,
    case_source: Option<String>,
    case_dir: Option<String>,
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn c_rr_tags_are_written_only_when_set_and_old_records_read() {
    // a record of main's form reads with the three absent, and writes back the same JSON value
    let rec: RunRecord = serde_json::from_str(MAIN_RUN_JSON).unwrap();
    assert_eq!((&rec.study, &rec.variant, &rec.condition), (&None, &None, &None));
    let main_value: Value = serde_json::from_str(MAIN_RUN_JSON).unwrap();
    let again = serde_json::to_value(&rec).unwrap();
    assert_eq!(again, main_value, "an untagged record writes the keys of main, no more");
    for k in TAGS {
        assert!(again.get(k).is_none(), "{k} written without a value");
    }
    // tagged: the three keys with their values, and a round trip
    let tagged = RunRecord {
        study: Some("designs/nu3".into()),
        variant: Some("nu=0.015".into()),
        condition: Some("base".into()),
        ..rec.clone()
    };
    let v = serde_json::to_value(&tagged).unwrap();
    assert_eq!(v["study"], json!("designs/nu3"));
    assert_eq!(v["variant"], json!("nu=0.015"));
    assert_eq!(v["condition"], json!("base"));
    let mut want = keys(&main_value);
    want.extend(TAGS.iter().map(|s| s.to_string()));
    want.sort();
    assert_eq!(keys(&v), want);
    let back: RunRecord = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(back, tagged);
    // a reader made before the fields reads the tagged record
    let old: ReaderBefore = serde_json::from_value(v).unwrap();
    assert_eq!(old.case, "cases/cavity.toml");
    // one tag alone is written alone
    let one = RunRecord {
        study: Some("designs/x".into()),
        ..rec
    };
    let v = serde_json::to_value(&one).unwrap();
    assert_eq!(v.get("variant"), None);
    assert_eq!(v.get("condition"), None);
    assert_eq!(serde_json::from_value::<RunRecord>(v).unwrap(), one);
    eprintln!("C-RR record: untagged keys {:?}", keys(&again));
}
