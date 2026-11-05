//! ADR 0186 C-7 (ADR 0173 C7, the MCP part): in a new project, one `start_run` call through the `rcfd-mcp` binary makes
//! `cases/sod.toml` from the shipped example and runs it with the start plan, so its fields are stored; controls: the
//! same with `store: "none"` stores nothing, `start_run {case}` is as before F-271, and a taken name is refused with
//! nothing written. Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

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

/// `wait_run` until the run is terminal (at most `CAP`); its final state.
fn wait_terminal(m: &mut Mcp, run: &str) -> String {
    let t0 = Instant::now();
    loop {
        let v = ok(&m.call("wait_run", json!({"run": run, "timeout_s": 60})));
        if v["terminal"] == true {
            return v["state"].as_str().unwrap().to_string();
        }
        assert!(t0.elapsed() < CAP, "run {run} not ended after {CAP:?}: {v}");
    }
}

fn settings_set(dir: &Path, run: &str) -> Vec<String> {
    let v: Value =
        serde_json::from_slice(&std::fs::read(dir.join("runs").join(run).join("run.json")).unwrap()).unwrap();
    serde_json::from_value(v["settings"]["set"].clone()).unwrap()
}

fn snapshots(dir: &Path, run: &str) -> Option<usize> {
    let store = dir.join("runs").join(run).join("fields");
    store
        .join(rcfd_tsstore::STORE_FILE)
        .is_file()
        .then(|| rcfd_tsstore::Reader::open(&store).unwrap().snapshots().len())
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

#[test]
fn c7_one_start_run_makes_a_case_from_an_example_and_stores_its_fields() {
    binaries();
    let dir = scratch("f271-c7").join("proj");
    Project::create(&dir, "f271 c7", "test").unwrap();
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let example = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/sod/sod.toml")).unwrap();

    // one call: the case made from the example, then run with the start plan
    let v = ok(&m.call(
        "start_run",
        json!({"new_case": {"name": "sod", "example": "sod/sod.toml"}}),
    ));
    eprintln!("C-7 start_run: {v}");
    assert_eq!(
        std::fs::read(dir.join("cases/sod.toml")).unwrap(),
        example,
        "not byte-equal to the example"
    );
    assert_eq!(v["case"], "cases/sod.toml");
    assert_eq!(v["created"]["path"], "cases/sod.toml");
    assert_eq!(v["store"]["applied"], true, "{v}");
    let planned = json!(["output.store_time_interval=0.002", "output.store_precision=f32"]);
    assert_eq!(v["store"]["set"], planned);
    assert_eq!(v["store"]["snapshots"], 100);
    let a = v["run"].as_str().unwrap().to_string();
    assert_eq!(json!(settings_set(&dir, &a)), planned);
    let t0 = Instant::now();
    assert_eq!(wait_terminal(&mut m, &a), "finished");
    let n = snapshots(&dir, &a).expect("the run has a field store");
    eprintln!("C-7 run {a}: finished after {:?}, {n} snapshots", t0.elapsed());
    assert!((50..=101).contains(&n), "{n} snapshots");
    let f = ok(&m.call("field_summary", json!({"run": a, "field": "p"})));
    eprintln!("C-7 field_summary p: min {} max {} n {}", f["min"], f["max"], f["n"]);
    assert!(
        f["min"].as_f64().is_some_and(f64::is_finite) && f["max"].as_f64().is_some_and(f64::is_finite),
        "{f}"
    );

    // control (i): store "none" stores nothing and adds no entry
    let v = ok(&m.call(
        "start_run",
        json!({"new_case": {"name": "sod-none", "example": "sod/sod.toml"}, "store": "none"}),
    ));
    let b = v["run"].as_str().unwrap().to_string();
    assert!(v.get("store").is_none(), "{v}");
    assert_eq!(wait_terminal(&mut m, &b), "finished");
    assert_eq!(snapshots(&dir, &b), None, "store: none wrote a store");
    assert!(settings_set(&dir, &b).is_empty());

    // control (ii): start_run of a project case is as it was before F-271
    let v = ok(&m.call("start_run", json!({"case": "cases/sod.toml"})));
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["case", "run", "state"], "{v}");
    let c = v["run"].as_str().unwrap().to_string();
    assert!(settings_set(&dir, &c).is_empty());
    assert_eq!(wait_terminal(&mut m, &c), "finished");
    assert_eq!(snapshots(&dir, &c), None);

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
        eprintln!("C-7 (iii) {tool}: {}", r["structuredContent"]["error"]["message"]);
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
    let e = v["run"].as_str().unwrap().to_string();
    assert_eq!(settings_set(&dir, &e), ["output.store_interval=0"]);
    assert_eq!(wait_terminal(&mut m, &e), "finished");
}

/// ADR 0186 amendment 1: `new_case` writes files, so a read-only server refuses it (1100) and the folder is unchanged;
/// control: the default policy makes the case.
#[test]
fn new_case_is_refused_read_only() {
    use rcfd_mcp::policy::Policy;
    let dir = project("f271-ro");
    let args = json!({"name": "lid", "template": "lid_driven_cavity"});
    let r = call(&server(&dir, Policy::read_only()), "new_case", args.clone());
    assert_eq!(err_code(&r), 1100, "{r}");
    assert!(!dir.join("cases/lid.toml").exists());
    let v = ok(&call(&server(&dir, Policy::default()), "new_case", args));
    assert_eq!(v["path"], "cases/lid.toml");
    assert_eq!(
        std::fs::read_to_string(dir.join("cases/lid.toml")).unwrap(),
        rcfd_mcp::LID_DRIVEN_CAVITY
    );
}
