//! ADR 0835 (F-998): a new case file is written by `atomic::write_new_case`, every other new file by `write_new`.
//! K1: what a writer killed mid-write leaves (a prefix of the case under its name) is read by the project as a malformed
//! case: listed, refused by `case_facts` with its name, the other cases intact, a `new_case` of that name refused.
//! K3 (Windows): while `create_with` makes new cases, the case files never appear as temp files and the data files do
//! (they stay on the temp file and hard link); N4: its negative control, `write_new` seen as temp files.

use rcfd_project::Project;
use rcfd_workspace::{Answer, RpcError, Workspace};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn project(name: &str) -> (PathBuf, Project) {
    let dir = std::env::temp_dir().join(format!("rcfd-ws-f998-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let p = Project::create(&dir, name, "test").unwrap();
    (dir, p)
}

fn call(ws: &Workspace, name: &str, args: Value) -> Result<Value, RpcError> {
    match ws.call(name, &args).expect("a workspace command") {
        Ok(Answer::Value(v)) => Ok(v),
        Ok(other) => panic!("{name}: not a value: {other:?}"),
        Err(e) => Err(e),
    }
}

/// ADR 0835 K1: the first 40 % of a case's bytes (cut mid-line) left as `cases/k1.toml`.
#[test]
fn k1_a_prefix_left_by_a_killed_writer_reads_as_a_malformed_case() {
    let src = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example-projects/quick-start/cases/cavity-re100.toml"),
    )
    .unwrap();
    let mut cut = src.len() * 2 / 5;
    if src[cut - 1] == b'\n' {
        cut += 1;
    }
    let prefix = &src[..cut];
    let (dir, _p) = project("k1");
    std::fs::write(dir.join("cases/intact.toml"), &src).unwrap();
    std::fs::write(dir.join("cases/k1.toml"), prefix).unwrap();
    let ws = Workspace::open(&dir).unwrap();
    let info = call(&ws, "project_info", json!({})).expect("K1: project_info");
    assert_eq!(info["counts"]["cases"], 2, "K1: both cases listed: {info}");
    let e = call(&ws, "case_facts", json!({"case": "cases/k1.toml"})).expect_err("K1: case_facts of the prefix");
    let text = format!("{e:?}");
    assert!(text.contains("cases/k1.toml"), "K1: the refusal names the file: {text}");
    call(&ws, "case_facts", json!({"case": "cases/intact.toml"})).expect("K1: the intact case answers");
    let r = call(&ws, "new_case", json!({"name": "k1", "copy": "cases/intact.toml"}));
    assert!(r.is_err(), "K1: new_case of k1 not refused: {r:?}");
    assert_eq!(
        std::fs::read(dir.join("cases/k1.toml")).unwrap(),
        prefix,
        "K1: the prefix changed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
mod lister {
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// Temp names seen in `dir` while `work` runs: (case-file temps `.k-<i>.toml.`, other temps, listings).
    pub fn watch(dir: &Path, work: impl FnOnce()) -> (usize, usize, usize) {
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(std::collections::BTreeSet::new()));
        let t = {
            let (dir, stop, seen) = (dir.to_path_buf(), stop.clone(), seen.clone());
            std::thread::spawn(move || {
                let mut n = 0;
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(rd) = std::fs::read_dir(&dir) {
                        for e in rd.flatten() {
                            let name = e.file_name().to_string_lossy().into_owned();
                            if rcfd_project::atomic::is_temp_name(&name) {
                                seen.lock().unwrap().insert(name);
                            }
                        }
                        n += 1;
                    }
                }
                n
            })
        };
        work();
        stop.store(true, Ordering::Relaxed);
        let listings = t.join().unwrap();
        let seen = seen.lock().unwrap();
        let case = seen.iter().filter(|n| n.contains(".toml.")).count();
        eprintln!(
            "seen {} temp names ({case} of case files), {listings} listings",
            seen.len()
        );
        (case, seen.len() - case, listings)
    }
}

/// ADR 0835 K3: 40 new cases of 2 MiB, the odd ones with a data file of 2 MiB.
#[cfg(windows)]
#[test]
fn k3_new_case_files_never_appear_as_temp_files_and_data_files_do() {
    let (dir, p) = project("k3");
    let big: Vec<u8> = (0..2usize << 20).map(|i| (i % 251) as u8).collect();
    let (case, other, listings) = lister::watch(&dir.join("cases"), || {
        for i in 0..40 {
            let comp = if i % 2 == 1 {
                vec![(format!("cases/k-{i}.dat"), big.clone())]
            } else {
                Vec::new()
            };
            rcfd_workspace::cases::create_with(&p, &format!("cases/k-{i}.toml"), &big, &comp).unwrap();
        }
    });
    assert_eq!(case, 0, "K3: case files seen as temp files");
    assert!(other >= 1, "K3: no data file seen as a temp file");
    assert!(listings >= 40, "K3: listings {listings}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// ADR 0835 N4: `write_new` of 20 files of 2 MiB is seen as temp files.
#[cfg(windows)]
#[test]
fn n4_negative_control_write_new_is_seen_as_temp_files() {
    let (dir, _p) = project("n4");
    let big: Vec<u8> = (0..2usize << 20).map(|i| (i % 251) as u8).collect();
    let d = dir.join("cases");
    let (case, other, _) = lister::watch(&d, || {
        for i in 0..20 {
            rcfd_project::atomic::write_new(&d.join(format!("n-{i}.bin")), &big).unwrap();
        }
    });
    assert!(case + other >= 1, "N4: no temp file seen");
    let _ = std::fs::remove_dir_all(&dir);
}
