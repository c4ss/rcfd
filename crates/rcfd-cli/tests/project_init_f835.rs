//! ADR 0731 P2 (F-835): `rcfd project init <dir> [--name <name>]`. P-T1 a new folder, P-T2 a folder that already holds a
//! manifest, P-T3 a file and bad names, P-T4 the default name, and Q3 (a) a folder that holds other files.

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

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-cli-f835-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

/// Every file under `dir` with its bytes, sorted by path.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.push((p.clone(), Vec::new()));
                stack.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn p_t1_init_makes_a_project_like_project_create() {
    let base = scratch("t1");
    let dir = base.join("fan");
    let o = rcfd(&["project", "init", &s(&dir), "--name", "Fan study"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(out.lines().count(), 1, "one JSON line: {out}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    let p = Project::open(&dir).unwrap();
    for d in rcfd_project::project::DIRS {
        assert!(dir.join(d).is_dir(), "{d} missing");
    }
    let m = p.manifest();
    assert_eq!(v["name"], "Fan study");
    assert_eq!(v["root"], p.root().display().to_string());
    assert_eq!(v["created"], m.created);
    assert_eq!(v["schema_version"], m.schema_version);
    let version = format!("rcfd {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(v["solver_version_created"], version);
    assert_eq!(v["existing"], serde_json::json!([]));
    // the same as Project::create with the same name and version, but for `created`
    let other = base.join("reference");
    let r = Project::create(&other, "Fan study", &version).unwrap();
    let mut a: Value = serde_json::from_slice(&std::fs::read(dir.join("rcfd-project.json")).unwrap()).unwrap();
    let mut b: Value = serde_json::from_slice(&std::fs::read(other.join("rcfd-project.json")).unwrap()).unwrap();
    assert_eq!(b["created"], r.manifest().created);
    a["created"] = Value::Null;
    b["created"] = Value::Null;
    assert_eq!(a, b);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn p_t2_a_folder_with_a_manifest_is_refused_and_left_byte_identical() {
    let base = scratch("t2");
    let dir = base.join("p");
    Project::create(&dir, "first", "test").unwrap();
    std::fs::write(dir.join("notes/n.md"), "keep\n").unwrap();
    let before = snapshot(&dir);
    let o = rcfd(&["project", "init", &s(&dir), "--name", "second"]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.starts_with(&format!("error: {}: ", s(&dir))), "{err}");
    assert!(err.contains("rcfd-project.json"), "{err}");
    assert!(o.stdout.is_empty());
    assert_eq!(snapshot(&dir), before);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn p_t3_a_file_is_refused_and_bad_names_are_usage_errors() {
    let base = scratch("t3");
    let file = base.join("a-file");
    std::fs::write(&file, "x").unwrap();
    let o = rcfd(&["project", "init", &s(&file)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("is a file"));
    assert_eq!(std::fs::read(&file).unwrap(), b"x");
    let dir = base.join("p");
    for name in ["", "a\tb", "a\u{1b}b"] {
        let o = rcfd(&["project", "init", &s(&dir), "--name", name]);
        assert_eq!(o.status.code(), Some(2), "{name:?}");
        assert!(!dir.exists(), "{name:?}: nothing made");
    }
    for args in [
        &["project"][..],
        &["project", "init"],
        &["project", "frob"],
        &["project", "init", "a", "b"],
    ] {
        assert_eq!(rcfd(args).status.code(), Some(2), "{args:?}");
    }
    assert_eq!(rcfd(&["project", "init", &s(&dir), "--name"]).status.code(), Some(2));
    assert!(!dir.exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn p_t4_the_default_name_is_the_folder_name() {
    let base = scratch("t4");
    let dir = base.join("my-fan");
    let o = rcfd(&["project", "init", &s(&dir)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(Project::open(&dir).unwrap().manifest().name, "my-fan");
    // `.` names the folder itself
    let here = base.join("here");
    std::fs::create_dir_all(&here).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["project", "init", "."])
        .current_dir(&here)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(Project::open(&here).unwrap().manifest().name, "here");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn q3_a_folder_with_other_files_keeps_them_and_lists_them() {
    let base = scratch("q3");
    let dir = base.join("p");
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::write(dir.join("readme.txt"), "mine\n").unwrap();
    std::fs::write(dir.join("cases/old.toml"), "title = \"old\"\n").unwrap();
    let o = rcfd(&["project", "init", &s(&dir)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["existing"], serde_json::json!(["cases", "readme.txt"]));
    assert_eq!(std::fs::read(dir.join("readme.txt")).unwrap(), b"mine\n");
    assert_eq!(std::fs::read(dir.join("cases/old.toml")).unwrap(), b"title = \"old\"\n");
    Project::open(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn help_names_project_init() {
    let o = rcfd(&["--help"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("project init <dir> [--name <name>]"));
}
