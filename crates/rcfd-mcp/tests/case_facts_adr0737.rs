//! ADR 0737 (F-837): the project tools `case_facts` and `refine_case_mesh`, and `start_run {run_time, dry_run}`. C2 a
//! dry run writes nothing and its `set` is the `settings.set` of the same call's run record; C3 Refine x2 through
//! `edit_case`'s rule, refused for a mesh file with nothing written. The tool lists are checked by
//! `project_list_f270.rs`, `tool_list_f275.rs` and `project_w48f.rs`, restated there.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use serde_json::{json, Value};
use std::path::Path;
use std::time::{Duration, Instant};

const PISO: &str = r#"title = "piso cavity" # a comment refine keeps
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = 0.01
end_time = 1.0
"#;

/// Every file of the project but the change watcher's and the runner's own (path, bytes), sorted.
/// Shuts down the project runner a test's start_run started, so no runner outlives the test (and holds its pipes).
struct StopRunner(std::path::PathBuf);

impl Drop for StopRunner {
    fn drop(&mut self) {
        let runner = binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = std::process::Command::new(runner)
            .args(["shutdown", "--quiet", "--project"])
            .arg(&self.0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

/// The error code of a `tools/call` response: a JSON-RPC error (bad arguments) or a tool result with `isError`.
fn code(r: &Value) -> i64 {
    r["error"]["code"]
        .as_i64()
        .or_else(|| r["result"]["structuredContent"]["error"]["code"].as_i64())
        .unwrap_or_else(|| panic!("no error: {r}"))
}

fn hash_of(s: &rcfd_mcp::McpServer, rel: &str) -> String {
    ok(&call(s, "read_project_file", json!({"path": rel, "limit": 1})))["hash"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn case_facts_of_project_cases() {
    let dir = project("adr0737-facts");
    std::fs::write(dir.join("cases/piso.toml"), PISO).unwrap();
    let s = server(&dir, Policy::default());
    let before = files(&dir);
    let f = ok(&call(&s, "case_facts", json!({"case": "cases/piso.toml"})));
    assert_eq!(f["kind"], "transient");
    assert_eq!(
        (f["cells"].clone(), f["u0_from"].clone()),
        (json!(64), json!("boundary lid"))
    );
    assert!((f["h_min"].as_f64().unwrap() - 0.125).abs() < 1e-12);
    assert_eq!(
        (f["dt_in_text"].clone(), f["end_in_text"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(f["memory"]["budgets"], "project");
    assert_eq!(f["run_time"]["entries"][0], "solver.end_time=1");
    let f = ok(&call(
        &s,
        "case_facts",
        json!({"case": "cases/piso.toml", "run_time": {"simulate_s": 0.5, "mode": "fixed"}}),
    ));
    assert_eq!(f["run_time"]["step"]["mode"], "fixed");
    assert_eq!(f["run_time"]["steps_estimate"], 8);
    let f = ok(&call(&s, "case_facts", json!({"case": "cases/cavity.toml"})));
    assert_eq!(f["kind"], "steady");
    assert_eq!(f["run_time"]["refused"], "a steady run has no time step");
    assert_eq!(
        code(&rpc_call(&s, "case_facts", json!({"case": "cases/none.toml"}))),
        1202
    );
    assert_eq!(
        code(&rpc_call(
            &s,
            "case_facts",
            json!({"case": "cases/piso.toml", "run_time": {"mode": "x"}})
        )),
        -32602
    );
    assert_eq!(files(&dir), before, "case_facts writes nothing");
}

#[test]
fn refine_case_mesh_writes_through_edit_case() {
    let dir = project("adr0737-refine");
    std::fs::write(dir.join("cases/piso.toml"), PISO).unwrap();
    let file_case = PISO.replace("nx = 8\nny = 8\n", "source = \"file\"\nfile = \"m.msh\"\n");
    std::fs::write(dir.join("cases/file.toml"), &file_case).unwrap();
    let s = server(&dir, Policy::default());
    let h = hash_of(&s, "cases/piso.toml");
    let r = ok(&call(
        &s,
        "refine_case_mesh",
        json!({"case": "cases/piso.toml", "factor": 2, "base_hash": h, "reason": "a finer mesh"}),
    ));
    assert_eq!((r["cells"].clone(), r["cells_after"].clone()), (json!(64), json!(256)));
    let text = std::fs::read_to_string(dir.join("cases/piso.toml")).unwrap();
    assert!(
        text.contains("nx = 16\nny = 16\n") && text.contains("# a comment refine keeps"),
        "{text}"
    );
    assert_eq!(r["written"]["new_hash"], json!(hash_of(&s, "cases/piso.toml")));
    let journal = std::fs::read_to_string(dir.join(".rcfd/edits.jsonl")).unwrap();
    let last: Value = serde_json::from_str(journal.lines().last().unwrap()).unwrap();
    assert_eq!(
        (last["path"].clone(), last["reason"].clone()),
        (json!("cases/piso.toml"), json!("a finer mesh"))
    );
    // the old hash is stale now: 1200, nothing written
    let r = rpc_call(
        &s,
        "refine_case_mesh",
        json!({"case": "cases/piso.toml", "factor": 2, "base_hash": h}),
    );
    assert_eq!(code(&r), 1200);
    // a mesh file: 1009 MESH_FILE with the reason, nothing written (it was 1004 before ADR 0737's change of 2026-11-17)
    let before = files(&dir);
    let hf = hash_of(&s, "cases/file.toml");
    let r = rpc_call(
        &s,
        "refine_case_mesh",
        json!({"case": "cases/file.toml", "factor": 2, "base_hash": hf}),
    );
    assert_eq!(code(&r), 1009, "{r}");
    assert!(r.to_string().contains("mesh file"), "{r}");
    let r = rpc_call(
        &s,
        "refine_case_mesh",
        json!({"case": "cases/piso.toml", "factor": 3, "base_hash": hf}),
    );
    assert_eq!(code(&r), -32602, "{r}");
    assert_eq!(files(&dir), before);
    // --read-only refuses the write
    let ro = server(&dir, Policy::read_only());
    let r = rpc_call(
        &ro,
        "refine_case_mesh",
        json!({"case": "cases/piso.toml", "factor": 2, "base_hash": hash_of(&s, "cases/piso.toml")}),
    );
    assert_eq!(code(&r), 1100, "{r}");
}

/// C2: `start_run {run_time, dry_run: true}` writes nothing; its `set` is the run record's `settings.set` of the same call
/// without `dry_run` (the run-time entries, then the store plan's, then the call's own); without `run_time` and `dry_run`
/// the reply has no `run_time`.
#[test]
fn c2_dry_run_writes_nothing_and_its_set_is_the_runs() {
    let dir = project("adr0737-dry");
    std::fs::write(dir.join("cases/piso.toml"), PISO).unwrap();
    let d = dir.to_string_lossy().to_string();
    let _ = binaries();
    // start_run starts the project's runner (detached, ADR 0146); shut it down when the test ends, pass or fail
    let _runner = StopRunner(dir.clone());
    let mut m = Mcp::start(&["--project", &d]);
    let _ = m.request("initialize", json!({}));
    let args = json!({"case": "cases/piso.toml", "run_time": {"simulate_s": 0.05}, "store": "plan", "set": ["physics.nu=0.02"]});
    let before = files(&dir);
    let mut dry_args = args.clone();
    dry_args["dry_run"] = json!(true);
    let r = m.call("start_run", dry_args);
    let dry = r["structuredContent"].clone();
    assert_eq!(dry["dry_run"], true, "{r}");
    assert_eq!(files(&dir), before, "a dry run writes nothing");
    let set: Vec<&str> = dry["set"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(set[0], "solver.end_time=0.05");
    assert!(set.contains(&"solver.corrector_control=adaptive"));
    assert_eq!(set.last(), Some(&"physics.nu=0.02"));
    assert_eq!(dry["check"]["ok"], true, "{dry}");
    assert_eq!(dry["check"]["cells"], 64);
    assert!(dry["memory_mb"].as_u64().unwrap() >= 64);
    assert!(dry["runner"]["would_start"].is_boolean());
    assert_eq!(dry["run_time"]["entries"][0], "solver.end_time=0.05");
    // the same call, run
    let r = m.call("start_run", args);
    let st = r["structuredContent"].clone();
    let id = st["run"].as_str().unwrap().to_string();
    assert_eq!(st["run_time"], dry["run_time"]);
    let rec: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("runs").join(&id).join("run.json")).unwrap()).unwrap();
    assert_eq!(rec["settings"]["set"], dry["set"], "{rec}");
    let t0 = Instant::now();
    loop {
        let w = m.call("wait_run", json!({"run": id, "timeout_s": 30}));
        if w["structuredContent"]["terminal"] == true || t0.elapsed() > Duration::from_secs(300) {
            assert_eq!(w["structuredContent"]["state"], "finished", "{w}");
            break;
        }
    }
    // a rerun (ADR 0734 from_run) keeps its run's settings: run_time and dry_run are refused beside it, nothing queued
    for extra in [json!({"run_time": {"simulate_s": 0.1}}), json!({"dry_run": true})] {
        let mut a = json!({"from_run": id});
        a.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let r = m.request("tools/call", json!({"name": "start_run", "arguments": a}));
        assert_eq!(code(&r), -32602, "{r}");
    }
    // without run_time: no run_time in the reply (as before)
    let r = m.call("start_run", json!({"case": "cases/cavity.toml", "dry_run": true}));
    assert!(r["structuredContent"]["run_time"].is_null());
    // a Courant target above 2: the plan's warning names the fix
    let r = m.call(
        "start_run",
        json!({"case": "cases/piso.toml", "dry_run": true, "run_time": {"courant": 3}}),
    );
    let w = r["structuredContent"]["run_time"]["warning"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        w.contains("the Courant target 3 is above 2") && w.contains("Fix:"),
        "{r}"
    );
}
