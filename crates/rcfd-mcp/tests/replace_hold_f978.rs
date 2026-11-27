//! ADR 0832 (F-978): how long a refused case file stays held. H is arm B of ADR 0830 (no `--canvas` server, every logical
//! processor busy in this process, `new_case` then B1's two `edit_case` per round) with 300 rounds, and on each refusal a
//! marker `mark-<i>-<op>.txt` beside the project folder naming the refused file, which the scratch trace X3 of ADR 0832
//! follows until no process but `rcfd*` holds it. H2: H again after the fix F1, which must give no refusal. TC3: the
//! control of X3, a marked file that is not the newest, held by this process for 800 ms. Ignored: H and H2 load the whole
//! machine on purpose, and TC3 needs X3 beside it; each is run once.

mod common;

use common::Mcp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

fn ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

/// The marker X3 follows: `mark-<i>-<op>.txt` in the folder that holds the project, naming the file under `cases/`.
fn mark(dir: &Path, i: usize, op: &str, file: &str) {
    let top = dir.parent().unwrap();
    let tmp = top.join(format!("tmp-{i}-{op}"));
    std::fs::write(&tmp, file).unwrap();
    std::fs::rename(&tmp, top.join(format!("mark-{i}-{op}.txt"))).unwrap();
}

/// One call, printed as `ARM <tag> <i> <op> <start> <end> ok|refused <text>`; a refusal is marked at once.
fn step(c: &mut Mcp, dir: &Path, tag: &str, i: usize, op: &str, tool: &str, args: Value) -> bool {
    let t0 = ms();
    let r = c.call(tool, args);
    let t1 = ms();
    let refused = r["isError"] != false;
    if refused && op != "new_case" {
        mark(dir, i, op, &format!("arm-{i}.toml"));
    }
    let text: String = if refused { r.to_string().chars().take(240).collect() } else { String::new() };
    eprintln!(
        "ARM {tag} {i} {op} {t0} {t1} {} {text}",
        if refused { "refused" } else { "ok" }
    );
    refused
}

/// The arm of the hold with `rounds` rounds; the number of refusals.
fn arm(tag: &str, rounds: usize) -> usize {
    common::binaries();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir: PathBuf = common::scratch(&format!("f978-hold-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    rcfd_workspace::Workspace::open(&dir).unwrap();
    let d = dir.display().to_string();
    let mut c = Mcp::start(&["--project", &d]);
    eprintln!("ARM {tag} pids: test {} client {}", std::process::id(), c.pid());
    let stop = Arc::new(AtomicBool::new(false));
    let n = std::thread::available_parallelism().map_or(1, |n| n.get());
    let spin: Vec<_> = (0..n)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut x = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));
    let mut refused = 0;
    for i in 0..rounds {
        let case = format!("cases/arm-{i}.toml");
        let args = json!({"name": format!("arm-{i}"), "copy": "cases/cavity-re100.toml", "reason": "F-978 ADR 0832 arm"});
        if step(&mut c, &dir, tag, i, "new_case", "new_case", args) {
            refused += 1;
            continue;
        }
        for (op, set) in [
            ("edit1", json!({"mesh.nx": 16, "mesh.ny": 16})),
            ("edit2", json!({"physics.nu": 0.02})),
        ] {
            let hash = c.call("read_project_file", json!({"path": case}))["structuredContent"]["hash"].clone();
            let args = json!({"path": case, "base_hash": hash, "set": set, "reason": "F-978 ADR 0832 arm"});
            refused += step(&mut c, &dir, tag, i, op, "edit_case", args) as usize;
        }
    }
    stop.store(true, Ordering::Relaxed);
    for t in spin {
        t.join().unwrap();
    }
    eprintln!("ARM {tag} refusals {refused} of {}", 3 * rounds);
    drop(c);
    // X3 is given time to see the last marked files free before the folder goes
    std::thread::sleep(Duration::from_secs(3));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    refused
}

/// ADR 0832 H: the arm of the hold, 300 rounds.
#[test]
#[ignore = "ADR 0832: loads every processor on purpose; run once with X3 beside it"]
fn h_the_hold_of_a_refused_file_under_load() {
    arm("h", 300);
}

/// ADR 0832 H2: H after the fix F1: no refusal of 900.
#[test]
#[ignore = "ADR 0832: loads every processor on purpose; run once with X3 beside it"]
fn h2_no_refusal_under_load_after_the_fix() {
    assert_eq!(arm("h2", 300), 0, "H2: refusals of 900");
}

/// ADR 0832 TC3: this process holds a marked file that is not the newest for 800 ms.
#[test]
#[ignore = "ADR 0832: the control of the scratch trace X3, which must run beside it"]
fn tc3_the_trace_follows_a_marked_file_until_it_is_free() {
    let dir = common::scratch("f978-hold-tc").join("quick-start");
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    for f in ["arm-0.toml", "arm-1.toml"] {
        std::fs::write(dir.join("cases").join(f), "x = 1\n").unwrap();
    }
    std::thread::sleep(Duration::from_secs(1));
    let f = std::fs::File::open(dir.join("cases/arm-0.toml")).unwrap();
    eprintln!("TC3 pid {} hold arm-0.toml from {}", std::process::id(), ms());
    mark(&dir, 0, "tc", "arm-0.toml");
    std::thread::sleep(Duration::from_millis(800));
    drop(f);
    eprintln!("TC3 released at {}", ms());
    std::thread::sleep(Duration::from_secs(1));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
