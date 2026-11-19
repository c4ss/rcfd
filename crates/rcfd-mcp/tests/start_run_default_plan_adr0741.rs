//! ADR 0741 (F-271): the field-store plan is the default of `start_run` in project mode (ADR 0173 D7 (a), ADR 0186 Q2
//! (a), approved by the maintainer on 2026-11-18). `c7_restated` is ADR 0186 C-7 again, with its control (ii) in the new
//! form: `start_run {case}` without `store` applies the plan (the old form is the ignored record in
//! `new_simulations_f271.rs`). `default_plan_case_by_case` is the list of ADR 0741: a case with no store setting gets the
//! plan, `store: "none"` is the way back to what such a call did before, a case with its own store and a `set` entry
//! that gives the store keep theirs, the solution does not change with the store, a dry run reports the plan, and a
//! file that is no case is refused before anything is queued. Through the `rcfd-mcp` binary with the real runner; needs
//! `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_project::Project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CAP: Duration = Duration::from_secs(300);

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

/// `wait_run` until the run is terminal (at most `CAP`); the last answer.
fn wait_terminal(m: &mut Mcp, run: &str) -> Value {
    let t0 = Instant::now();
    loop {
        let v = ok(&m.call("wait_run", json!({"run": run, "timeout_s": 60})));
        if v["terminal"] == true {
            return v;
        }
        assert!(t0.elapsed() < CAP, "run {run} not ended after {CAP:?}: {v}");
    }
}

fn keys(v: &Value) -> Vec<&str> {
    let mut k: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    k.sort();
    k
}

fn run_id(v: &Value) -> String {
    v["run"].as_str().unwrap().to_string()
}

fn settings_set(dir: &Path, run: &str) -> Vec<String> {
    let v: Value =
        serde_json::from_slice(&std::fs::read(dir.join("runs").join(run).join("run.json")).unwrap()).unwrap();
    serde_json::from_value(v["settings"]["set"].clone()).unwrap()
}

/// The steps of the snapshots in the run's field store; `None` when the run has no store.
fn snapshot_steps(dir: &Path, run: &str) -> Option<Vec<u64>> {
    let store = dir.join("runs").join(run).join("fields");
    store.join(rcfd_tsstore::STORE_FILE).is_file().then(|| {
        rcfd_tsstore::Reader::open(&store)
            .unwrap()
            .snapshots()
            .iter()
            .map(|s| s.step)
            .collect()
    })
}

fn run_folders(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("runs"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn residual_lines(dir: &Path, run: &str) -> Vec<String> {
    std::fs::read_to_string(dir.join("runs").join(run).join("residuals.jsonl"))
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

fn lid_drag(dir: &Path, run: &str) -> f64 {
    let v: Value =
        serde_json::from_slice(&std::fs::read(dir.join("runs").join(run).join("metrics.json")).unwrap()).unwrap();
    v["reports"]["lid_drag"].as_f64().unwrap()
}

/// ADR 0741 rows 1 and 2: ADR 0186 C-7 with control (ii) restated for the new default.
#[test]
fn c7_restated() {
    binaries();
    let dir = scratch("adr0741-c7").join("proj");
    Project::create(&dir, "adr0741 c7", "test").unwrap();
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let example = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/sod/sod.toml")).unwrap();
    let planned = json!(["output.store_time_interval=0.002", "output.store_precision=f32"]);

    // row 1: one call makes the case from the example and runs it with the start plan
    let v = ok(&m.call(
        "start_run",
        json!({"new_case": {"name": "sod", "example": "sod/sod.toml"}}),
    ));
    eprintln!("row 1 start_run: {v}");
    assert_eq!(
        std::fs::read(dir.join("cases/sod.toml")).unwrap(),
        example,
        "not byte-equal to the example"
    );
    assert_eq!(v["case"], "cases/sod.toml");
    assert_eq!(v["created"]["path"], "cases/sod.toml");
    assert_eq!(v["store"]["applied"], true, "{v}");
    assert_eq!(v["store"]["set"], planned);
    assert_eq!(v["store"]["snapshots"], 100);
    let a = run_id(&v);
    assert_eq!(json!(settings_set(&dir, &a)), planned);
    let t0 = Instant::now();
    assert_eq!(wait_terminal(&mut m, &a)["state"], "finished");
    let n = snapshot_steps(&dir, &a).expect("the run has a field store").len();
    eprintln!("row 1 run {a}: finished after {:?}, {n} snapshots", t0.elapsed());
    assert!((50..=101).contains(&n), "{n} snapshots");
    let f = ok(&m.call("field_summary", json!({"run": a, "field": "p"})));
    eprintln!("row 1 field_summary p: min {} max {} n {}", f["min"], f["max"], f["n"]);
    assert!(
        f["min"].as_f64().is_some_and(f64::is_finite) && f["max"].as_f64().is_some_and(f64::is_finite),
        "{f}"
    );

    // control (i): store "none" stores nothing and adds no entry
    let v = ok(&m.call(
        "start_run",
        json!({"new_case": {"name": "sod-none", "example": "sod/sod.toml"}, "store": "none"}),
    ));
    let b = run_id(&v);
    assert!(v.get("store").is_none(), "{v}");
    assert_eq!(wait_terminal(&mut m, &b)["state"], "finished");
    assert_eq!(snapshot_steps(&dir, &b), None, "store: none wrote a store");
    assert!(settings_set(&dir, &b).is_empty());

    // row 2, control (ii) restated: start_run of a project case applies the plan by default
    let v = ok(&m.call("start_run", json!({"case": "cases/sod.toml"})));
    eprintln!("row 2 start_run: {v}");
    assert_eq!(keys(&v), ["case", "run", "state", "store"], "{v}");
    assert_eq!(v["store"]["applied"], true, "{v}");
    assert_eq!(v["store"]["set"], planned);
    let c = run_id(&v);
    assert_eq!(json!(settings_set(&dir, &c)), planned);
    assert_eq!(wait_terminal(&mut m, &c)["state"], "finished");
    let n = snapshot_steps(&dir, &c).expect("the run has a field store").len();
    eprintln!("row 2 run {c}: {n} snapshots");
    assert!((50..=101).contains(&n), "{n} snapshots");

    // control (iii): a taken name (in another letter case) is refused, nothing written and nothing queued
    let runs = run_folders(&dir);
    for (tool, args) in [
        (
            "start_run",
            json!({"new_case": {"name": "SOD", "example": "sod/sod.toml"}}),
        ),
        ("new_case", json!({"name": "SOD", "example": "sod/sod.toml"})),
    ] {
        let r = m.call(tool, args);
        assert_eq!(err_code(&r), 1004, "{r}");
    }
    assert_eq!(std::fs::read(dir.join("cases/sod.toml")).unwrap(), example);
    assert_eq!(run_folders(&dir), runs);

    // D7's wording: a set entry that gives the store's interval means the caller sets the store, so no plan
    let v = ok(&m.call(
        "start_run",
        json!({"case": "cases/sod.toml", "store": "plan", "set": ["output.store_interval=0"]}),
    ));
    assert_eq!(v["store"]["applied"], false, "{v}");
    let e = run_id(&v);
    assert_eq!(settings_set(&dir, &e), ["output.store_interval=0"]);
    assert_eq!(wait_terminal(&mut m, &e)["state"], "finished");
}

/// ADR 0741 rows 3 to 8: what `start_run {case}` without `store` does now, case by case, and the way back.
#[test]
fn default_plan_case_by_case() {
    binaries();
    let dir = project("adr0741-cases");
    // the fixture's cavity without its [output] table: a case with no store setting
    let plain = cavity(12, 30);
    let plain = plain
        .strip_suffix("[output]\nstore_interval = 10\n")
        .expect("the fixture ends with its [output] table");
    std::fs::write(dir.join("cases/plain.toml"), plain).unwrap();
    std::fs::write(dir.join("cases/bad.toml"), "this is = [not a case\n").unwrap();
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let planned = json!(["output.store_interval=1", "output.store_precision=f32"]);

    // row 3: no store setting, no store argument: the plan
    let v = ok(&m.call("start_run", json!({"case": "cases/plain.toml"})));
    eprintln!("row 3 start_run: {v}");
    assert_eq!(keys(&v), ["case", "run", "state", "store"], "{v}");
    assert_eq!(v["store"]["applied"], true, "{v}");
    assert_eq!(v["store"]["set"], planned);
    assert_eq!(v["store"]["snapshots"], 30);
    assert_eq!(v["store"]["estimated_bytes"], 69_120);
    let a = run_id(&v);
    assert_eq!(json!(settings_set(&dir, &a)), planned);
    let w = wait_terminal(&mut m, &a);
    assert_eq!(w["state"], "finished", "{w}");
    assert_eq!(w["status"]["step"], 30);
    let n = snapshot_steps(&dir, &a).expect("the run has a field store").len();
    eprintln!("row 3 run {a}: {n} snapshots");
    assert!((30..=31).contains(&n), "{n} snapshots");
    let f = ok(&m.call("field_summary", json!({"run": a, "field": "p"})));
    assert!(
        f["min"].as_f64().is_some_and(f64::is_finite) && f["max"].as_f64().is_some_and(f64::is_finite),
        "{f}"
    );

    // row 4, the way back: store "none" is what the call did before ADR 0741
    let v = ok(&m.call("start_run", json!({"case": "cases/plain.toml", "store": "none"})));
    assert_eq!(keys(&v), ["case", "run", "state"], "{v}");
    let b = run_id(&v);
    assert!(settings_set(&dir, &b).is_empty());
    let w = wait_terminal(&mut m, &b);
    assert_eq!(w["state"], "finished", "{w}");
    assert_eq!(w["status"]["step"], 30);
    assert_eq!(snapshot_steps(&dir, &b), None, "store: none wrote a store");

    // row 6: the store does not change the solution
    let (ra, rb) = (residual_lines(&dir, &a), residual_lines(&dir, &b));
    eprintln!("row 6: {} and {} residual lines", ra.len(), rb.len());
    assert_eq!(ra.len(), 30);
    assert_eq!(ra, rb, "the residuals differ between the stored and the unstored run");
    let (da, db) = (lid_drag(&dir, &a), lid_drag(&dir, &b));
    eprintln!("row 6: lid_drag {da:e} and {db:e}");
    assert_eq!(da.to_bits(), db.to_bits());

    // row 5: a case with its own store keeps it; only the reply says so
    let v = ok(&m.call("start_run", json!({"case": "cases/cavity.toml"})));
    eprintln!("row 5 start_run: {v}");
    assert_eq!(keys(&v), ["case", "run", "state", "store"], "{v}");
    assert_eq!(v["store"]["applied"], false, "{v}");
    assert_eq!(v["store"]["set"], json!([]));
    assert!(
        v["store"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("the case sets its own field store")),
        "{v}"
    );
    let c = run_id(&v);
    assert!(settings_set(&dir, &c).is_empty());
    assert_eq!(wait_terminal(&mut m, &c)["state"], "finished");
    let v = ok(&m.call("start_run", json!({"case": "cases/cavity.toml", "store": "none"})));
    let c0 = run_id(&v);
    assert_eq!(wait_terminal(&mut m, &c0)["state"], "finished");
    let (sc, sc0) = (snapshot_steps(&dir, &c), snapshot_steps(&dir, &c0));
    eprintln!("row 5: snapshot steps {sc:?} and {sc0:?}");
    assert!(
        sc.as_ref().is_some_and(|s| !s.is_empty()),
        "the case's own store is missing"
    );
    assert_eq!(sc, sc0);

    // row 7: a set entry that gives the store means no plan; another entry is planned for and follows the plan's
    let v = ok(&m.call(
        "start_run",
        json!({"case": "cases/plain.toml", "set": ["output.store_interval=5"]}),
    ));
    assert_eq!(v["store"]["applied"], false, "{v}");
    let e = run_id(&v);
    assert_eq!(settings_set(&dir, &e), ["output.store_interval=5"]);
    assert_eq!(wait_terminal(&mut m, &e)["state"], "finished");
    let v = ok(&m.call(
        "start_run",
        json!({"case": "cases/plain.toml", "set": ["solver.max_iterations=250"]}),
    ));
    eprintln!("row 7 start_run: {v}");
    assert_eq!(
        v["store"]["set"],
        json!(["output.store_interval=3", "output.store_precision=f32"])
    );
    assert_eq!(v["store"]["snapshots"], 84);
    let g = run_id(&v);
    assert_eq!(
        settings_set(&dir, &g),
        [
            "output.store_interval=3",
            "output.store_precision=f32",
            "solver.max_iterations=250"
        ]
    );
    assert_eq!(wait_terminal(&mut m, &g)["state"], "finished");

    // row 8: a dry run reports the plan and writes nothing
    let runs = run_folders(&dir);
    let v = ok(&m.call("start_run", json!({"case": "cases/plain.toml", "dry_run": true})));
    assert_eq!(v["dry_run"], true, "{v}");
    assert_eq!(v["store"]["applied"], true, "{v}");
    assert_eq!(v["set"], planned);
    assert_eq!(run_folders(&dir), runs, "a dry run made a run folder");
    // row 8: a file that is no case is refused before anything is queued
    let r = m.call("start_run", json!({"case": "cases/bad.toml"}));
    eprintln!("row 8 refusal: {}", r["structuredContent"]["error"]["message"]);
    assert_eq!(err_code(&r), 1003, "{r}");
    assert_eq!(run_folders(&dir), runs, "a refused start_run made a run folder");
    // control: with store "none" the file is queued as before, and the run does not finish
    let v = ok(&m.call("start_run", json!({"case": "cases/bad.toml", "store": "none"})));
    let h = run_id(&v);
    let w = wait_terminal(&mut m, &h);
    eprintln!("row 8 control: run {h} ended {}", w["state"]);
    assert_ne!(w["state"], "finished", "{w}");
}
