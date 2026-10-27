//! ADR 0128 finding 9 (W2): an unchanged project is not re-read on every poll; changes inside the 2 s window are
//! still seen.

mod common;

use rcfd_project::{Project, Watcher};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

fn age_all(dir: &Path, t: SystemTime) -> usize {
    let mut n = 0;
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if e.file_type().unwrap().is_dir() {
            n += age_all(&p, t);
        } else {
            fs::OpenOptions::new()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
            n += 1;
        }
    }
    n
}

#[test]
fn w2_unchanged_project_poll_reads_ten_times_fewer_bytes() {
    let dir = common::scratch("w2");
    let p = Project::create(&dir.join("proj"), "w2", "0").unwrap();
    let r = p.root().to_path_buf();
    fs::write(r.join("cases/a.json"), "{\"case\": 1}").unwrap();
    for i in 0..300 {
        p.create_run("cases/a.json", None, "0", serde_json::json!({ "i": i }))
            .unwrap();
    }
    let files = age_all(&r, SystemTime::now() - Duration::from_secs(60));
    let mut w = Watcher::new(&r).unwrap();
    let initial = w.bytes_hashed();
    let t0 = w.token();
    assert_eq!(w.poll().unwrap(), t0, "nothing changed");
    let polled = w.bytes_hashed() - initial;
    // inside the window: a new file, then a same-length rewrite that restores its time
    let f = r.join("notes/fresh.txt");
    fs::write(&f, "abcd").unwrap();
    let t1 = w.poll().unwrap();
    let new_seen = w.changes_since(t0) == ["notes/fresh.txt"];
    let mtime = fs::metadata(&f).unwrap().modified().unwrap();
    fs::write(&f, "abce").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let t2 = w.poll().unwrap();
    let rewrite_seen = t2 > t1 && w.changes_since(t1) == ["notes/fresh.txt"];
    // an old file rewritten now (new time) is seen too
    let old = r.join("cases/a.json");
    fs::write(&old, "{\"case\": 2}").unwrap();
    let t3 = w.poll().unwrap();
    let old_seen = t3 > t2 && w.changes_since(t2) == ["cases/a.json"];
    eprintln!(
        "W2: {files} files, initial scan hashed {initial} bytes, unchanged poll {polled} bytes; new {new_seen}, same-tick rewrite {rewrite_seen}, old rewritten {old_seen}"
    );
    assert!(polled * 10 <= initial, "poll read {polled} of {initial} bytes");
    assert!(new_seen && rewrite_seen && old_seen);
}
