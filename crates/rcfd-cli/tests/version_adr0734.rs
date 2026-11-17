//! ADR 0734 L2 (F-828 E-828a): `rcfd version --json` parses and names the build; `rcfd --version` and `rcfd version`
//! print the text of before. P4: `rcfd project hash <path>` prints the hash of `hash_project_file` and refuses what it
//! cannot hash.

use serde_json::Value;
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(args)
        .output()
        .expect("rcfd runs")
}

#[test]
fn l2_version_json_and_the_old_text() {
    let want = format!("rcfd {}\n", env!("CARGO_PKG_VERSION"));
    for args in [&["--version"][..], &["-V"], &["version"]] {
        let o = rcfd(args);
        assert!(o.status.success(), "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"),
            want,
            "{args:?}"
        );
    }
    let o = rcfd(&["version", "--json"]);
    assert!(o.status.success());
    let text = String::from_utf8(o.stdout).unwrap();
    assert_eq!(text.trim_end().lines().count(), 1, "one line: {text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    eprintln!("L2: {v}");
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["git_rev", "name", "profile", "target", "version"]);
    assert_eq!(v["name"], "rcfd");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["git_rev"].is_null() || v["git_rev"].is_string());
    assert!(v["profile"] == "release" || v["profile"] == "debug");
    assert_eq!(
        v["target"],
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    );
    let o = rcfd(&["version", "--yaml"]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn p4_project_hash() {
    let d = std::env::temp_dir().join(format!("rcfd-cli-adr0734-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("foam")).unwrap();
    std::fs::write(d.join("a.msh"), b"abc").unwrap();
    std::fs::write(d.join("foam/points"), b"()").unwrap();
    let o = rcfd(&["project", "hash", d.join("a.msh").to_str().unwrap()]);
    assert!(o.status.success());
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["hash"], rcfd_project::records::hash_bytes(b"abc"));
    assert_eq!(
        (v["kind"].as_str(), v["bytes"].as_u64(), v["files"].as_u64()),
        (Some("file"), Some(3), Some(1))
    );
    let o = rcfd(&["project", "hash", d.join("foam").to_str().unwrap()]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let line = format!("points\0{}\n", rcfd_project::records::hash_bytes(b"()"));
    assert_eq!(v["hash"], rcfd_project::records::hash_bytes(line.as_bytes()));
    assert_eq!(v["kind"], "folder");
    let o = rcfd(&["project", "hash", d.join("none").to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no such file or folder"));
    assert_eq!(rcfd(&["project", "hash"]).status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&d);
}
