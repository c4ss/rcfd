//! ADR 0201 A-U: the key of a study's run (`RunKey::matches`) and the choice among matches (`choose`) on `run.json` and
//! `status.json` pairs: a full key matches, each of the six items changed alone does not; failed and cancelled runs are not
//! adopted; a finished match before a newer queued one, of two queued ones the newer.

use rcfd_runner::records::{LaunchSettings, RunRecord, RunState, RunStatus};
use rcfd_runner::store::RunTags;
use rcfd_workspace::design::{choose, RunKey};
use serde_json::{json, Value};

fn key() -> RunKey {
    RunKey {
        tags: RunTags {
            study: Some("designs/nu1".into()),
            variant: Some("nu=0.015".into()),
            condition: Some("base".into()),
        },
        case: "cases/cavity.toml".into(),
        case_snapshot_hash: "fnv64:00000000000000aa".into(),
        set: vec!["physics.nu=0.015".into()],
        launch: LaunchSettings::default(),
        solver_version: "rcfd 1.0".into(),
    }
}

/// The `run.json` of the full key with id `id` and `edit` applied.
fn rec(id: &str, edit: impl Fn(&mut Value)) -> RunRecord {
    let mut v = json!({
        "id": id, "case": "cases/cavity.toml", "case_snapshot_hash": "fnv64:00000000000000aa",
        "solver_version": "rcfd 1.0", "created": "2026-11-06T12:00:00.000000000Z",
        "settings": {"priority": 0, "threads": 1, "mem_mb": null, "set": ["physics.nu=0.015"]},
        "case_source": "C:/p/cases/cavity.toml", "case_dir": "C:/p/cases",
        "study": "designs/nu1", "variant": "nu=0.015", "condition": "base"
    });
    edit(&mut v);
    serde_json::from_value(v).unwrap()
}

/// One change of a record.
type Edit = Box<dyn Fn(&mut Value)>;

fn st(state: RunState) -> RunStatus {
    RunStatus::new(state)
}

#[test]
fn au_the_key_and_the_choice() {
    let k = key();
    assert!(k.matches(&rec("r-20261106T120000-0001", |_| {})), "the full key");
    // settings that do not change the result are not compared
    assert!(k.matches(&rec("r-20261106T120000-0001", |v| {
        v["settings"]["threads"] = json!(4);
        v["settings"]["priority"] = json!(9);
        v["settings"]["mem_mb"] = json!(100);
    })));
    let changed: Vec<(&str, Edit)> = vec![
        ("study", Box::new(|v| v["study"] = json!("designs/other"))),
        ("variant", Box::new(|v| v["variant"] = json!("nu=0.0151"))),
        ("condition", Box::new(|v| v["condition"] = json!("other"))),
        (
            "no tags",
            Box::new(|v| {
                for t in ["study", "variant", "condition"] {
                    v.as_object_mut().unwrap().remove(t);
                }
            }),
        ),
        ("case", Box::new(|v| v["case"] = json!("cases/copy.toml"))),
        (
            "case_snapshot_hash",
            Box::new(|v| v["case_snapshot_hash"] = json!("fnv64:00000000000000ab")),
        ),
        (
            "set",
            Box::new(|v| v["settings"]["set"] = json!(["physics.nu=0.015", "solver.alpha_p=0.3"])),
        ),
        ("set empty", Box::new(|v| v["settings"]["set"] = json!([]))),
        (
            "continue_from",
            Box::new(|v| v["settings"]["continue_from"] = json!("r-20261106T110000-0000")),
        ),
        ("max_steps", Box::new(|v| v["settings"]["max_steps"] = json!(5000))),
        (
            "solver_version",
            Box::new(|v| v["solver_version"] = json!("rcfd 0.0.0-decoy")),
        ),
    ];
    for (what, edit) in &changed {
        assert!(
            !k.matches(&rec("r-20261106T120000-0001", edit)),
            "{what} changed, still a match"
        );
    }
    // a key with launch settings matches only a record with them
    let mut kl = key();
    kl.launch.max_steps = Some(20);
    assert!(!kl.matches(&rec("r-20261106T120000-0001", |_| {})));
    assert!(kl.matches(&rec("r-20261106T120000-0001", |v| v["settings"]["max_steps"] = json!(20))));
    // the states: failed and cancelled are never adopted
    for (s, adopted) in [
        (RunState::Queued, true),
        (RunState::Running, true),
        (RunState::Paused, true),
        (RunState::Finished, true),
        (RunState::Failed, false),
        (RunState::Cancelled, false),
    ] {
        let runs = [(rec("r-20261106T120000-0001", |_| {}), st(s))];
        assert_eq!(choose(&k, &runs).is_some(), adopted, "{s:?}");
    }
    // a finished match before a newer queued one; of two queued ones the newer; a non-match is never chosen
    let runs = [
        (rec("r-20261106T120000-0001", |_| {}), st(RunState::Finished)),
        (rec("r-20261106T120005-0002", |_| {}), st(RunState::Queued)),
        (
            rec("r-20261106T120009-0003", |v| v["variant"] = json!("x")),
            st(RunState::Finished),
        ),
    ];
    assert_eq!(choose(&k, &runs).unwrap().id, "r-20261106T120000-0001");
    let runs = [
        (rec("r-20261106T120000-0001", |_| {}), st(RunState::Queued)),
        (rec("r-20261106T120005-0002", |_| {}), st(RunState::Running)),
        (rec("r-20261106T120009-0003", |_| {}), st(RunState::Failed)),
    ];
    assert_eq!(choose(&k, &runs).unwrap().id, "r-20261106T120005-0002");
    eprintln!(
        "A-U: {} single-item changes refused; states and order as registered",
        changed.len()
    );
}
