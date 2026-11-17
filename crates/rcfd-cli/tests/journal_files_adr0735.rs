//! ADR 0735 (F-828 E-828c): `rcfd run --journal` on a journal file the service kept, and the hash check of its case
//! file.
//!
//! Registered in the ADR before this file existed: J2 (run half: the journal file a service kept after create, three
//! sets and a remove runs under `rcfd run --journal` to the same settings, the same residuals and the same stored
//! solution as the session run with `run.start`), J3 (a case file changed after the journal was written is refused with
//! exit 3 and both hashes; `--journal-ignore-hash` runs it) and `rcfd serve --journal-dir` outside `--root` refused.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::session_journal::hash_bytes;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_adr0735_cli_{}_{name}", std::process::id()));
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
algorithm = "simplec"
convection = "upwind"
max_iterations = 20
[reports.m]
type = "volume_average"
field = "speed"
"#;

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// The journal file of a session made from `case`, after three sets and a remove, by a service with journal files in
/// `jdir`; with the service and the session.
fn recorded(case: &Path, jdir: &Path) -> (std::sync::Arc<Service>, String, PathBuf) {
    let svc = Service::new(ServiceConfig::default());
    svc.enable_journal_files(&jdir.to_string_lossy()).unwrap();
    let id = svc
        .call("session.create", &json!({"path": case.to_string_lossy()}))
        .unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    for (path, value) in [
        ("mesh.nx", json!(10)),
        ("mesh.ny", json!(9)),
        ("solver.max_iterations", json!(25)),
    ] {
        svc.call("settings.set", &json!({"session": id, "path": path, "value": value}))
            .unwrap();
    }
    svc.call("settings.remove", &json!({"session": id, "path": "reports.m"}))
        .unwrap();
    let file = svc.call("journal.get", &json!({"session": id})).unwrap()["file"]
        .as_str()
        .unwrap()
        .to_string();
    (svc, id, PathBuf::from(file))
}

#[test]
fn j2_journal_file_runs_to_the_session_result() {
    let d = tmp("j2");
    let case = d.join("case.toml");
    std::fs::write(&case, CASE).unwrap();
    let (svc, id, file) = recorded(&case, &d.join("journals"));
    // the settings
    let o = rcfd(&["run", "--journal", file.to_str().unwrap(), "--print-settings"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let cli_settings: Value = serde_json::from_slice(&o.stdout).unwrap();
    let svc_settings = svc.call("settings.get", &json!({"session": id})).unwrap()["value"].clone();
    // --print-settings fills in the defaults; every setting the session holds has the same value there
    let mut differ = Vec::new();
    compare(&svc_settings, &cli_settings, "", &mut differ);
    println!("J2 settings: {} differences {differ:?}", differ.len());
    assert!(differ.is_empty(), "{differ:?}");
    // the run
    let out = d.join("cli");
    let o = rcfd(&[
        "run",
        "--journal",
        file.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--status-json",
        "--status-every",
        "1",
        "--quiet",
    ]);
    assert!(matches!(o.status.code(), Some(0) | Some(5)), "{}", stderr(&o));
    let events: Vec<Value> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let last = events.iter().rev().find(|e| e["event"] == "iteration").unwrap();
    assert_eq!(events[0]["cells"], 10 * 9);
    svc.call("run.start", &json!({"session": id})).unwrap();
    let st = svc
        .call("run.wait", &json!({"session": id, "timeout_ms": 120_000}))
        .unwrap();
    let sim = &st["simulation"];
    println!(
        "J2 cli: index {} res_u {} res_p {}; service: iteration {} res_u {} res_p {}",
        last["index"], last["res_u"], last["res_p"], sim["iteration"], sim["res_u"], sim["res_p"]
    );
    assert_eq!(
        (last["index"].clone(), last["res_u"].clone(), last["res_p"].clone()),
        (sim["iteration"].clone(), sim["res_u"].clone(), sim["res_p"].clone())
    );
    let svc_out = d.join("svc");
    svc.call(
        "field.export_vtk",
        &json!({"session": id, "dir": svc_out.to_string_lossy(), "name": "solution"}),
    )
    .unwrap();
    let a = std::fs::read(out.join("solution.vtu")).unwrap();
    let b = std::fs::read(svc_out.join("solution.vtu")).unwrap();
    println!("J2 solution.vtu: {} and {} bytes, equal {}", a.len(), b.len(), a == b);
    assert!(a == b, "the stored solutions differ");
    std::fs::remove_dir_all(&d).ok();
}

fn compare(a: &Value, b: &Value, at: &str, out: &mut Vec<String>) {
    match a {
        Value::Object(m) => {
            for (k, v) in m {
                let p = if at.is_empty() { k.clone() } else { format!("{at}.{k}") };
                compare(v, &b[k], &p, out);
            }
        }
        _ if a != b => out.push(format!("{at}: {a} / {b}")),
        _ => {}
    }
}

#[test]
fn j3_changed_case_file_is_refused_unless_asked() {
    let d = tmp("j3");
    let case = d.join("case.toml");
    std::fs::write(&case, CASE).unwrap();
    let (_svc, _id, file) = recorded(&case, &d.join("journals"));
    let recorded_hash = hash_bytes(CASE.as_bytes());
    // the spelling of the project records (rcfd-project), which rcfd-api does not depend on
    assert_eq!(recorded_hash, rcfd_project::records::hash_bytes(CASE.as_bytes()));
    // unchanged: runs (check only, to be quick)
    let o = rcfd(&["run", "--journal", file.to_str().unwrap(), "--check-only"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let changed = CASE.replace("nu = 0.01", "nu = 0.02");
    std::fs::write(&case, &changed).unwrap();
    let o = rcfd(&["run", "--journal", file.to_str().unwrap(), "--check-only"]);
    let err = stderr(&o);
    println!("J3 changed: exit {:?}: {}", o.status.code(), err.trim());
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(
        err.contains(&recorded_hash) && err.contains(&hash_bytes(changed.as_bytes())),
        "{err}"
    );
    let o = rcfd(&[
        "run",
        "--journal",
        file.to_str().unwrap(),
        "--journal-ignore-hash",
        "--print-settings",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let s: Value = serde_json::from_slice(&o.stdout).unwrap();
    println!("J3 --journal-ignore-hash: runs with physics.nu {}", s["physics"]["nu"]);
    assert_eq!(s["physics"]["nu"], json!(0.02));
    // a journal without a recorded hash (written before ADR 0735) is not checked
    let mut old: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let src = old["entries"][0]["source"].as_object_mut().unwrap();
    src.remove("hash");
    src.remove("resolved");
    let old_file = d.join("journals").join("old.json");
    std::fs::write(&old_file, old.to_string()).unwrap();
    let o = rcfd(&["run", "--journal", old_file.to_str().unwrap(), "--check-only"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn serve_refuses_a_journal_dir_outside_the_root() {
    let d = tmp("serve");
    let root = d.join("root");
    std::fs::create_dir_all(&root).unwrap();
    let outside = d.join("journals");
    let o = rcfd(&[
        "serve",
        "--root",
        root.to_str().unwrap(),
        "--journal-dir",
        outside.to_str().unwrap(),
        "--stdio",
    ]);
    let err = stderr(&o);
    println!(
        "serve --journal-dir outside --root: exit {:?}: {}",
        o.status.code(),
        err.trim()
    );
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("--journal-dir") && !outside.exists(), "{err}");
    std::fs::remove_dir_all(&d).ok();
}
