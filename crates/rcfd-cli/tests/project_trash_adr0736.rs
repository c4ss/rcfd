//! ADR 0736 P1 (G16): `rcfd project trash|trash-list|restore|empty-trash`, the CLI twins of the MCP trash tools, on a
//! project with one finished run and a case. C1 trash a run, list it, restore it byte for byte; C2 a case with a stale
//! base is refused (exit 1, code 1200) and with its hash moved; C3 empty-trash removes for good; C4 usage errors exit 2;
//! the journal lines name the actor `cli`.

use rcfd_project::records::{RunState, RunStatus};
use rcfd_project::Project;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(args)
        .output()
        .expect("rcfd runs")
}

fn json_out(o: &Output) -> Value {
    assert!(
        o.status.success(),
        "exit {:?}: {}",
        o.status,
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-cli-0736-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let p = Project::create(&dir, name, "test").unwrap();
    std::fs::write(dir.join("cases/a.toml"), "title = \"a\"\n").unwrap();
    let rec = p
        .create_run("cases/a.toml", None, "test", serde_json::json!({}))
        .unwrap();
    p.write_status(&rec.id, &RunStatus::new(RunState::Finished)).unwrap();
    std::fs::create_dir_all(dir.join("runs").join(&rec.id).join("fields")).unwrap();
    std::fs::write(dir.join("runs").join(&rec.id).join("fields/x.bin"), b"abc").unwrap();
    dir
}

fn run_id(dir: &Path) -> String {
    Project::open(dir).unwrap().run_ids().unwrap()[0].clone()
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".rcfd" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn c1_trash_list_restore_a_run() {
    let dir = project("c1");
    let d = dir.display().to_string();
    let id = run_id(&dir);
    let before = files(&dir);
    let r = json_out(&rcfd(&["project", "trash", &d, "--run", &id, "--reason", "old"]));
    assert_eq!(r["kind"], "run");
    let token = r["token"].as_str().unwrap().to_string();
    assert!(!dir.join("runs").join(&id).exists());
    let l = json_out(&rcfd(&["project", "trash-list", &d]));
    assert_eq!(l["items"][0]["token"], token.as_str());
    assert_eq!(l["items"][0]["reason"], "old");
    let j = journal(&dir);
    let last = j.last().unwrap();
    assert_eq!(
        (last["kind"].as_str(), last["who"].as_str()),
        (Some("trashed"), Some("trash_run"))
    );
    assert_eq!(last["actor"]["client"], "cli");
    json_out(&rcfd(&["project", "restore", &d, &token]));
    assert_eq!(files(&dir), before);
    assert!(Project::open(&dir).unwrap().validate().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn c2_case_needs_its_base() {
    let dir = project("c2");
    let d = dir.display().to_string();
    let o = rcfd(&[
        "project",
        "trash",
        &d,
        "--case",
        "cases/a.toml",
        "--base-hash",
        "fnv64:0000000000000000",
    ]);
    assert_eq!(o.status.code(), Some(1));
    let e: Value = serde_json::from_slice(&o.stderr).unwrap();
    assert_eq!(e["code"], 1200);
    assert!(dir.join("cases/a.toml").is_file());
    let h = rcfd_project::records::hash_bytes(&std::fs::read(dir.join("cases/a.toml")).unwrap());
    let r = json_out(&rcfd(&[
        "project",
        "trash",
        &d,
        "--case",
        "cases/a.toml",
        "--base-hash",
        &h,
    ]));
    assert_eq!(r["hash"], h.as_str());
    assert!(!dir.join("cases/a.toml").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn c3_empty_trash_removes_for_good() {
    let dir = project("c3");
    let d = dir.display().to_string();
    let id = run_id(&dir);
    let t = json_out(&rcfd(&["project", "trash", &d, "--results", &id, "--parts", "fields"]))["token"]
        .as_str()
        .unwrap()
        .to_string();
    let o = rcfd(&["project", "empty-trash", &d, "--token", "t-20200101T000000-0000"]);
    assert_eq!(o.status.code(), Some(1));
    let r = json_out(&rcfd(&["project", "empty-trash", &d, "--token", &t]));
    assert_eq!(r["removed"][0], t.as_str());
    assert_eq!(json_out(&rcfd(&["project", "trash-list", &d]))["total"], 0);
    assert!(!dir.join("runs").join(&id).join("fields").exists());
    assert!(dir.join("runs").join(&id).join("run.json").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn c4_usage_errors_and_help() {
    let dir = project("c4");
    let d = dir.display().to_string();
    for args in [
        vec!["project", "trash", d.as_str()],
        vec!["project", "trash", d.as_str(), "--run", "x", "--case", "y"],
        vec!["project", "trash", d.as_str(), "--case", "cases/a.toml"],
        vec!["project", "restore", d.as_str()],
        vec!["project", "trash-list", d.as_str(), "--reason", "x"],
        vec!["project", "empty-trash", d.as_str(), "--older-than-days", "soon"],
    ] {
        assert_eq!(rcfd(&args).status.code(), Some(2), "{args:?}");
    }
    let help = String::from_utf8(rcfd(&["--help"]).stdout).unwrap();
    for c in [
        "project trash <dir>",
        "project trash-list <dir>",
        "project restore <dir> <token>",
        "project empty-trash <dir>",
    ] {
        assert!(help.contains(c), "{c}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
