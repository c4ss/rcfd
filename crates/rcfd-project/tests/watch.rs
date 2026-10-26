//! ADR 0122 W1: watcher change tokens report exactly the changed paths.

mod common;

use rcfd_project::{atomic, Project, Watcher};
use std::fs;

fn sequence(hash_limit: u64, tag: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let dir = common::scratch(tag);
    let p = Project::create(&dir.join("proj"), tag, "0").unwrap();
    let r = p.root().to_path_buf();
    for f in [
        "cases/a.json",
        "cases/c.json",
        "cases/d.json",
        "meshes/m.msh",
        "notes/keep.txt",
    ] {
        fs::write(r.join(f), format!("content of {f}")).unwrap();
    }
    let mut w = Watcher::with_hash_limit(&r, hash_limit).unwrap();
    let t0 = w.token();

    // Modify a (append), create b in a new directory, delete c, rewrite d with the same length and
    // restore its modification time (what a rewrite inside one timestamp tick looks like).
    fs::OpenOptions::new()
        .append(true)
        .open(r.join("cases/a.json"))
        .and_then(|mut f| std::io::Write::write_all(&mut f, b"!"))
        .unwrap();
    fs::create_dir_all(r.join("cases/new")).unwrap();
    fs::write(r.join("cases/new/b.json"), "b").unwrap();
    fs::remove_file(r.join("cases/c.json")).unwrap();
    let d = r.join("cases/d.json");
    let mtime = fs::metadata(&d).unwrap().modified().unwrap();
    let old = fs::read(&d).unwrap();
    let mut new = old.clone();
    new[0] ^= 0x20;
    fs::write(&d, &new).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&d)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(
        fs::metadata(&d).unwrap().modified().unwrap(),
        mtime,
        "set-up: mtime restored"
    );
    let t1 = w.poll().unwrap();
    let first = w.changes_since(t0);
    let none = w.changes_since(t1);

    // Atomic replace (temp file + rename) and runner-private files under .rcfd/.
    atomic::write_json(&r.join("notes/status.json"), &serde_json::json!({"x": 1})).unwrap();
    fs::write(r.join(".rcfd/runner.lock"), "pid").unwrap();
    let t2 = w.poll().unwrap();
    assert_eq!(w.poll().unwrap(), t2, "no change, same token");
    let second = w.changes_since(t1);
    (first, none, second)
}

#[test]
fn w1_tokens_report_exactly_the_changed_paths() {
    let (first, none, second) = sequence(rcfd_project::watch::DEFAULT_HASH_LIMIT, "w1");
    assert_eq!(
        first,
        ["cases/a.json", "cases/c.json", "cases/d.json", "cases/new/b.json"]
    );
    assert!(none.is_empty(), "{none:?}");
    assert_eq!(second, ["notes/status.json"]);
}

#[test]
fn w1_negative_control_length_and_mtime_alone_miss_the_same_tick_rewrite() {
    let (first, _, _) = sequence(0, "w1neg");
    assert_eq!(first, ["cases/a.json", "cases/c.json", "cases/new/b.json"]);
}
