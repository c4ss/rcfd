//! ADR 0830 (F-978): the arms that decide why the first `edit_case` after `new_case` is refused under load (B1 of ADR
//! 0826). Each arm, on a copy of `example-projects/quick-start`, keeps every logical processor busy in this process and
//! makes 30 times `new_case` then B1's two `edit_case`, printing each call's start and end (epoch ms) and its result;
//! the trace of the handles on the case files (the scratch tool X of ADR 0830) runs beside it. A: with the `--canvas`
//! server of B1; B: without; A2: A again after the fix F1, which must give no refusal. Then the control TC: this
//! process holds the last case file for 500 ms. Ignored: they load the whole machine on purpose and are run once each.

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

fn short(r: &Value) -> String {
    r.to_string().chars().take(240).collect()
}

/// One call, printed as `ARM <tag> <i> <op> <start> <end> ok|refused <text>`; true when it was refused.
fn step(c: &mut Mcp, tag: &str, i: usize, op: &str, tool: &str, args: Value) -> (bool, Value) {
    let t0 = ms();
    let r = c.call(tool, args);
    let t1 = ms();
    let refused = r["isError"] != false;
    let text = if refused { short(&r) } else { String::new() };
    eprintln!(
        "ARM {tag} {i} {op} {t0} {t1} {} {text}",
        if refused { "refused" } else { "ok" }
    );
    (refused, r)
}

fn arm(tag: &str, canvas: bool) -> usize {
    common::binaries();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir: PathBuf = common::scratch(&format!("f978-arm-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    rcfd_workspace::Workspace::open(&dir).unwrap();
    let d = dir.display().to_string();
    let studio = canvas.then(|| Mcp::start(&["--project", &d, "--client", "studio", "--read-only", "--canvas"]));
    let mut c = Mcp::start(&["--project", &d]);
    eprintln!(
        "ARM {tag} pids: test {} client {} studio {:?}",
        std::process::id(),
        c.pid(),
        studio.as_ref().map(|s| s.pid())
    );
    // the load L: every logical processor busy in this process
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
    let mut last = String::new();
    for i in 0..30 {
        let case = format!("cases/arm-{i}.toml");
        let (r, _) = step(
            &mut c,
            tag,
            i,
            "new_case",
            "new_case",
            json!({"name": format!("arm-{i}"), "copy": "cases/cavity-re100.toml", "reason": "F-978 ADR 0830 arm"}),
        );
        refused += r as usize;
        if r {
            continue;
        }
        last = case.clone();
        for (op, set) in [
            ("edit1", json!({"mesh.nx": 16, "mesh.ny": 16})),
            ("edit2", json!({"physics.nu": 0.02})),
        ] {
            let hash = c.call("read_project_file", json!({"path": case}))["structuredContent"]["hash"].clone();
            let (r, _) = step(
                &mut c,
                tag,
                i,
                op,
                "edit_case",
                json!({"path": case, "base_hash": hash, "set": set, "reason": "F-978 ADR 0830 arm"}),
            );
            refused += r as usize;
        }
    }
    stop.store(true, Ordering::Relaxed);
    for t in spin {
        t.join().unwrap();
    }
    // TC: this process holds the last case file for 500 ms
    if !last.is_empty() {
        let f = std::fs::File::open(dir.join(&last)).unwrap();
        eprintln!("ARM {tag} TC hold {last} from {}", ms());
        std::thread::sleep(Duration::from_millis(500));
        drop(f);
        eprintln!("ARM {tag} TC released at {}", ms());
    }
    eprintln!("ARM {tag} refusals {refused} of 90");
    drop(c);
    drop(studio);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    refused
}

/// ADR 0830 arm A: with the `--canvas` server.
#[test]
#[ignore = "ADR 0830: loads every processor on purpose; run once"]
fn arm_a_with_the_canvas_server_under_load() {
    arm("a", true);
}

/// ADR 0830 arm B: without the `--canvas` server.
#[test]
#[ignore = "ADR 0830: loads every processor on purpose; run once"]
fn arm_b_without_the_canvas_server_under_load() {
    arm("b", false);
}

/// ADR 0830 arm A2: arm A after the fix F1: no refusal.
#[test]
#[ignore = "ADR 0830: loads every processor on purpose; run once"]
fn arm_a2_with_the_canvas_server_under_load_after_the_fix() {
    assert_eq!(arm("a2", true), 0, "A2: refusals of 90");
}

/// ADR 0830 amendment 1, arm A1: arm A again, with the trace at high priority.
#[test]
#[ignore = "ADR 0830: loads every processor on purpose; run once"]
fn arm_a1_with_the_canvas_server_under_load_traced_at_high_priority() {
    arm("a1", true);
}
