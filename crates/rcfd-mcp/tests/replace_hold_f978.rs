//! ADR 0832 (F-978): how long a refused case file stays held. H is arm B of ADR 0830 (no `--canvas` server, every logical
//! processor busy in this process, `new_case` then B1's two `edit_case` per round) with 300 rounds, and on each refusal a
//! marker `mark-<i>-<op>.txt` beside the project folder naming the refused file, which the scratch trace X3 of ADR 0832
//! follows until no process but `rcfd*` holds it. H2: H again after the fix F1, which must give no refusal. TC3: the
//! control of X3, a marked file that is not the newest, held by this process for 800 ms. Ignored: H and H2 load the whole
//! machine on purpose, and TC3 needs X3 beside it; each is run once.
//!
//! ADR 0833 (F-998): the arms H, HO (the project outside `%TEMP%`), HP (the case created by a plain write) and HOP
//! (both), each 300 rounds of H's load with the trace X4 of ADR 0833 beside it; TC4, X4's control in the outside root;
//! S1, a fast check of the arms' set-up.
//!
//! ADR 0834 (F-998): the arms H (`write_new` before the plain write) and P (after it), 450 rounds each of H's load with
//! the trace X5 beside them; the measure is the count of new case files the System process holds. Each is built at its
//! commit and run once.

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
    let text: String = if refused {
        r.to_string().chars().take(240).collect()
    } else {
        String::new()
    };
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
        let args =
            json!({"name": format!("arm-{i}"), "copy": "cases/cavity-re100.toml", "reason": "F-978 ADR 0832 arm"});
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

/// ADR 0833: the folder a project of an F-998 arm is made in: `%TEMP%` as ADR 0832, or outside it in
/// `RCFD_F998_OUTSIDE` (else `CARGO_TARGET_TMPDIR`).
fn place(name: &str, outside: bool) -> PathBuf {
    if !outside {
        return common::scratch(name);
    }
    let dir = outside_root().join(format!("rcfd-mcp-w48f-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// ADR 0833: the root of the projects outside `%TEMP%`.
fn outside_root() -> PathBuf {
    std::env::var_os("RCFD_F998_OUTSIDE").map_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from)
}

/// ADR 0833: a plain write of a new case: `create_new`, `write_all`, close; no temp file, no hard link, no sync.
fn create_plain(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)
}

/// ADR 0833: the quick-start project of an arm, in its place.
fn setup(tag: &str, outside: bool) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = place(&format!("f998-hold-{tag}"), outside).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    rcfd_workspace::Workspace::open(&dir).unwrap();
    dir
}

/// ADR 0833: round `i` of an arm: the case created (`new_case`, or a plain write) and B1's two `edit_case`; the
/// refusals of the creation and of the edits.
fn round(c: &mut Mcp, dir: &Path, tag: &str, i: usize, plain: bool, src: &[u8]) -> (usize, usize) {
    let case = format!("cases/arm-{i}.toml");
    if plain {
        let t0 = ms();
        let r = create_plain(&dir.join(&case), src);
        let t1 = ms();
        let text = r.as_ref().err().map_or(String::new(), |e| e.to_string());
        let res = if r.is_ok() { "ok" } else { "refused" };
        eprintln!("ARM {tag} {i} create {t0} {t1} {res} {text}");
        if r.is_err() {
            return (1, 0);
        }
    } else {
        let args =
            json!({"name": format!("arm-{i}"), "copy": "cases/cavity-re100.toml", "reason": "F-998 ADR 0833 arm"});
        if step(c, dir, tag, i, "new_case", "new_case", args) {
            return (1, 0);
        }
    }
    let mut edits = 0;
    for (op, set) in [
        ("edit1", json!({"mesh.nx": 16, "mesh.ny": 16})),
        ("edit2", json!({"physics.nu": 0.02})),
    ] {
        let hash = c.call("read_project_file", json!({"path": case}))["structuredContent"]["hash"].clone();
        let args = json!({"path": case, "base_hash": hash, "set": set, "reason": "F-998 ADR 0833 arm"});
        edits += step(c, dir, tag, i, op, "edit_case", args) as usize;
    }
    (0, edits)
}

/// ADR 0833: an arm of `rounds` rounds under H's load; the refusals of its `3 * rounds` calls.
fn arm_f998(tag: &str, rounds: usize, outside: bool, plain: bool) -> usize {
    common::binaries();
    let dir = setup(tag, outside);
    let src = std::fs::read(dir.join("cases/cavity-re100.toml")).unwrap();
    let d = dir.display().to_string();
    let mut c = Mcp::start(&["--project", &d]);
    eprintln!("ARM {tag} pids: test {} client {} dir {d}", std::process::id(), c.pid());
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
    let (mut created, mut edits) = (0, 0);
    for i in 0..rounds {
        let (a, b) = round(&mut c, &dir, tag, i, plain, &src);
        created += a;
        edits += b;
    }
    stop.store(true, Ordering::Relaxed);
    for t in spin {
        t.join().unwrap();
    }
    let total = created + edits;
    eprintln!(
        "ARM {tag} refusals {total} of {} (creation {created}, edit_case {edits})",
        3 * rounds
    );
    drop(c);
    // X4 is given time to see the last marked files free before the folder goes
    std::thread::sleep(Duration::from_secs(3));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    total
}

/// ADR 0833 H: the control, ADR 0832's H in `%TEMP%` with `new_case`.
#[test]
#[ignore = "ADR 0833: loads every processor on purpose; run once with X4 beside it"]
fn f998_h_control() {
    arm_f998("h", 300, false, false);
}

/// ADR 0833 HO: the project outside `%TEMP%`.
#[test]
#[ignore = "ADR 0833: loads every processor on purpose; run once with X4 beside it"]
fn f998_ho_outside() {
    arm_f998("ho", 300, true, false);
}

/// ADR 0833 HP: the case created by a plain write.
#[test]
#[ignore = "ADR 0833: loads every processor on purpose; run once with X4 beside it"]
fn f998_hp_plain() {
    arm_f998("hp", 300, false, true);
}

/// ADR 0833 HOP: both.
#[test]
#[ignore = "ADR 0833: loads every processor on purpose; run once with X4 beside it"]
fn f998_hop_outside_plain() {
    arm_f998("hop", 300, true, true);
}

/// ADR 0833 TC4: ADR 0832's TC3 in the outside root.
#[test]
#[ignore = "ADR 0833: the control of the scratch trace X4, which must run beside it"]
fn f998_tc4_trace_control() {
    let dir = place("f998-hold-tc", true).join("quick-start");
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    for f in ["arm-0.toml", "arm-1.toml"] {
        std::fs::write(dir.join("cases").join(f), "x = 1\n").unwrap();
    }
    std::thread::sleep(Duration::from_secs(1));
    let f = std::fs::File::open(dir.join("cases/arm-0.toml")).unwrap();
    eprintln!("TC4 pid {} hold arm-0.toml from {}", std::process::id(), ms());
    mark(&dir, 0, "tc", "arm-0.toml");
    std::thread::sleep(Duration::from_millis(800));
    drop(f);
    eprintln!("TC4 released at {}", ms());
    std::thread::sleep(Duration::from_secs(1));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// ADR 0833 S1: the arms' places, the plain write, and one unloaded round of HOP.
#[test]
fn s1_the_arms_place_and_create_as_registered() {
    let tmp = std::env::temp_dir();
    let h = place("f998-s1-h", false);
    assert!(
        h.starts_with(&tmp),
        "S1: H's project {} not under {}",
        h.display(),
        tmp.display()
    );
    let o = place("f998-s1-o", true);
    assert!(
        !o.starts_with(&tmp),
        "S1: HO's project {} under {}",
        o.display(),
        tmp.display()
    );
    let root = outside_root();
    assert!(
        o.starts_with(&root),
        "S1: HO's project {} not under {}",
        o.display(),
        root.display()
    );
    let _ = std::fs::remove_dir_all(&h);
    let _ = std::fs::remove_dir_all(&o);

    common::binaries();
    let dir = setup("s1", true);
    let src = std::fs::read(dir.join("cases/cavity-re100.toml")).unwrap();
    create_plain(&dir.join("cases/arm-0.toml"), &src).unwrap();
    assert_eq!(
        std::fs::read(dir.join("cases/arm-0.toml")).unwrap(),
        src,
        "S1: the plain write's bytes"
    );
    let temps: Vec<_> = std::fs::read_dir(dir.join("cases"))
        .unwrap()
        .flatten()
        .filter(|e| rcfd_project::atomic::is_temp_name(&e.file_name().to_string_lossy()))
        .collect();
    assert!(temps.is_empty(), "S1: temp files left: {temps:?}");
    std::fs::remove_file(dir.join("cases/arm-0.toml")).unwrap();
    let d = dir.display().to_string();
    let mut c = Mcp::start(&["--project", &d]);
    assert_eq!(
        round(&mut c, &dir, "s1", 0, true, &src),
        (0, 0),
        "S1: one unloaded round of HOP"
    );
    let text = std::fs::read_to_string(dir.join("cases/arm-0.toml")).unwrap();
    assert!(
        text.contains("nx = 16") && text.contains("0.02"),
        "S1: both edits in the case: {text}"
    );
    drop(c);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// ADR 0834 H: 450 rounds of H's load, run from the test binary built before the plain write.
#[test]
#[ignore = "ADR 0834: loads every processor on purpose; run once with X5 beside it"]
fn f998b_h_hold_count() {
    arm_f998("bh", 450, false, false);
}

/// ADR 0834 P: 450 rounds of H's load (900 edit_case), run from the test binary built with the plain write.
#[test]
#[ignore = "ADR 0834: loads every processor on purpose; run once with X5 beside it"]
fn f998b_p_hold_count() {
    arm_f998("bp", 450, false, false);
}
