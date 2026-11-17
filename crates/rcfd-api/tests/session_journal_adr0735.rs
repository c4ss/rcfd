//! ADR 0735 (F-828 E-828c): a service session's source, recorded, and journal files on disk, opt-in.
//!
//! Registered in the ADR before this file existed: J1 (the source of each kind), J2 (service half: the file equals
//! `journal.get` after create, three sets and a remove; the run half is `crates/rcfd-cli/tests/journal_files_adr0735.rs`)
//! and J4 (without `--journal-dir` nothing is written and the existing reply members are as before). J3 is in the CLI
//! test. The existing journal tests (`service.rs` unit tests, `service_base_dir_w48j.rs`, `run_control.rs`) run unedited.

use rcfd_api::service::{code, Service, ServiceConfig, METHODS};
use rcfd_api::session_journal::hash_bytes;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_adr0735_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const CASE: &str = r#"
title = "journal cavity"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
max_iterations = 20
[reports.m]
type = "volume_average"
field = "speed"
"#;

fn files_under(d: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn create(svc: &Arc<Service>, p: Value) -> String {
    svc.call("session.create", &p).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string()
}

/// J1: a session made from a file records its resolved path and the hash of its bytes; from `toml`, `settings` or
/// nothing the kind is that and the hash null. `session.list` gives the same `source`.
#[test]
fn j1_source_of_each_kind() {
    let d = tmp("j1");
    let case = d.join("case.toml");
    std::fs::write(&case, CASE).unwrap();
    let svc = Service::new(ServiceConfig::default());
    let from_file = create(&svc, json!({"path": case.to_string_lossy()}));
    let j = svc.call("journal.get", &json!({"session": from_file})).unwrap();
    let want = hash_bytes(CASE.as_bytes());
    println!(
        "J1 path: source {}, create entry {}",
        j["source"], j["entries"][0]["source"]["hash"]
    );
    assert_eq!(
        j["source"],
        json!({"kind": "path", "path": case.to_string_lossy(), "hash": want})
    );
    assert_eq!(j["entries"][0]["source"]["hash"], json!(want));
    assert_eq!(j["entries"][0]["source"]["resolved"], json!(case.to_string_lossy()));
    assert_eq!(j["file"], Value::Null);
    let from_toml = create(&svc, json!({"toml": CASE}));
    let settings: Value = svc.call("settings.get", &json!({"session": from_toml})).unwrap()["value"].clone();
    let from_settings = create(&svc, json!({"settings": settings}));
    let empty = create(&svc, json!({}));
    for (id, kind) in [(&from_toml, "toml"), (&from_settings, "settings"), (&empty, "empty")] {
        let j = svc.call("journal.get", &json!({"session": id})).unwrap();
        println!("J1 {kind}: source {}", j["source"]);
        assert_eq!(j["source"], json!({"kind": kind, "path": null, "hash": null}));
        assert!(j["entries"][0]["source"].get("hash").is_none());
    }
    let list = svc.call("session.list", &json!({})).unwrap();
    let sessions = list["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 4);
    for s in sessions {
        let j = svc.call("journal.get", &json!({"session": s["session"]})).unwrap();
        assert_eq!(s["source"], j["source"], "{s}");
    }
    std::fs::remove_dir_all(&d).ok();
}

/// J2 (service half): with journal files on, after create, three sets and a remove, the session's one file holds what
/// `journal.get` answers, and its name is `<session>-<yyyymmddThhmmss>.journal.json`. A rejected change leaves it as
/// it was. `journal.save` writes the same document to a named path. The file replays to the same case hash.
#[test]
fn j2_journal_file_follows_every_accepted_change() {
    let d = tmp("j2");
    let case = d.join("case.toml");
    std::fs::write(&case, CASE).unwrap();
    let jdir = d.join("journals");
    let svc = Service::new(ServiceConfig::default());
    let on = svc.enable_journal_files(&jdir.to_string_lossy()).unwrap();
    assert_eq!(on, jdir);
    assert_eq!(
        svc.enable_journal_files(&jdir.to_string_lossy()).unwrap_err().code,
        code::BAD_STATE
    );
    let id = create(&svc, json!({"path": case.to_string_lossy()}));
    let names = files_under(&jdir);
    println!("J2 files after create: {names:?}");
    assert_eq!(names.len(), 1);
    let name = &names[0];
    let stamp = name
        .strip_prefix(&format!("{id}-"))
        .and_then(|r| r.strip_suffix(".journal.json"))
        .unwrap_or_else(|| panic!("file name {name}"));
    assert!(
        stamp.len() == 15 && stamp.as_bytes()[8] == b'T' && stamp.chars().filter(char::is_ascii_digit).count() == 14,
        "{stamp}"
    );
    let file = jdir.join(name);
    let read = || -> Value { serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap() };
    let get = || svc.call("journal.get", &json!({"session": id})).unwrap();
    assert_eq!(read(), get());
    for (path, value) in [
        ("mesh.nx", json!(10)),
        ("mesh.ny", json!(10)),
        ("solver.max_iterations", json!(30)),
    ] {
        svc.call("settings.set", &json!({"session": id, "path": path, "value": value}))
            .unwrap();
        assert_eq!(read(), get(), "after set {path}");
    }
    svc.call("settings.remove", &json!({"session": id, "path": "reports.m"}))
        .unwrap();
    let j = get();
    assert_eq!(read(), j, "after the remove");
    assert_eq!(j["entries"].as_array().unwrap().len(), 5);
    assert_eq!(j["file"], json!(file.display().to_string()));
    assert_eq!(j["source"]["hash"], json!(hash_bytes(CASE.as_bytes())));
    // a rejected change is not journaled and does not touch the file
    svc.call(
        "settings.set",
        &json!({"session": id, "path": "solver.alpha_u", "value": 9.0}),
    )
    .unwrap_err();
    assert_eq!(read(), j);
    assert_eq!(files_under(&jdir), vec![name.clone()], "no temporary file left");
    let saved = d.join("saved.journal.json");
    let r = svc
        .call("journal.save", &json!({"session": id, "path": saved.to_string_lossy()}))
        .unwrap();
    assert_eq!(r["path"], json!(saved.display().to_string()));
    let saved_doc: Value = serde_json::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(saved_doc, j);
    let replay = svc.call("journal.replay", &json!({"journal": read()})).unwrap();
    println!(
        "J2 entries {}, case_hash {}, replay {replay}",
        j["entries"].as_array().unwrap().len(),
        j["case_hash"]
    );
    assert_eq!(
        (replay["matches"].clone(), replay["case_hash"].clone()),
        (json!(true), j["case_hash"].clone())
    );
    // the replayed session has its own file
    assert_eq!(files_under(&jdir).len(), 2);
    let info = svc.call("service.info", &json!({})).unwrap();
    assert_eq!(info["journal_dir"], json!(jdir.display().to_string()));
    assert_eq!(info["methods"].as_array().unwrap().last(), Some(&json!("journal.save")));
    std::fs::remove_dir_all(&d).ok();
}

/// J2 (sandbox): the journal folder is a service path like any other.
#[test]
fn j2_journal_dir_is_sandboxed() {
    let d = tmp("j2box");
    let root = d.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..ServiceConfig::default()
    });
    let outside = d.join("elsewhere");
    let e = svc.enable_journal_files(&outside.to_string_lossy()).unwrap_err();
    assert_eq!(e.code, code::LIMIT, "{}", e.message);
    assert!(!outside.exists());
    let inside = svc.enable_journal_files("journals").unwrap();
    assert_eq!(inside, root.join("journals"));
    let id = create(&svc, json!({}));
    assert!(files_under(&inside)[0].starts_with(&format!("{id}-")));
    std::fs::remove_dir_all(&d).ok();
}

/// J4: without journal files nothing is written under the sandbox, `journal.save` is not a method, `service.info` is
/// as before (69 methods, no `journal_dir`), and the existing members of `journal.get` and `session.list` hold the
/// values they held before ADR 0735 (restated here from the code of 14a2d490).
#[test]
fn j4_off_by_default() {
    let d = tmp("j4");
    let root = d.join("root");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("case.toml"), CASE).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..ServiceConfig::default()
    });
    let id = create(&svc, json!({"path": "case.toml"}));
    svc.call("settings.set", &json!({"session": id, "path": "mesh.nx", "value": 9}))
        .unwrap();
    svc.call("settings.remove", &json!({"session": id, "path": "reports.m"}))
        .unwrap();
    assert_eq!(files_under(&root), vec!["case.toml".to_string()]);
    let info = svc.call("service.info", &json!({})).unwrap();
    assert!(info.get("journal_dir").is_none());
    assert_eq!(info["methods"].as_array().unwrap().len(), METHODS.len());
    assert_eq!(METHODS.len(), 69);
    assert!(!METHODS.contains(&"journal.save"));
    let e = svc
        .call("journal.save", &json!({"session": id, "path": "x.json"}))
        .unwrap_err();
    assert_eq!(e.code, code::METHOD_NOT_FOUND, "{}", e.message);
    let j = svc.call("journal.get", &json!({"session": id})).unwrap();
    assert_eq!(
        (j["format"].clone(), j["version"].clone()),
        (json!("rcfd-journal"), json!(1))
    );
    let e0 = &j["entries"][0];
    assert_eq!(e0["op"], "create");
    for (k, v) in [
        ("path", json!("case.toml")),
        ("toml", Value::Null),
        ("settings", Value::Null),
    ] {
        assert_eq!(e0["source"][k], v, "{k}");
    }
    assert_eq!(j["entries"][1], json!({"op": "set", "path": "mesh.nx", "value": 9}));
    assert_eq!(j["entries"][2], json!({"op": "remove", "path": "reports.m"}));
    let s = svc.call("settings.get", &json!({"session": id})).unwrap()["value"].clone();
    let other = create(&svc, json!({"settings": s}));
    let jo = svc.call("journal.get", &json!({"session": other})).unwrap();
    assert_eq!(
        j["case_hash"], jo["case_hash"],
        "case_hash is the tree's hash, as before"
    );
    assert_eq!(j["file"], Value::Null);
    let list = svc.call("session.list", &json!({})).unwrap();
    assert_eq!(list["sessions"][0]["session"], json!(id));
    assert_eq!(list["sessions"][0]["state"], json!("defined"));
    std::fs::remove_dir_all(&d).ok();
}
