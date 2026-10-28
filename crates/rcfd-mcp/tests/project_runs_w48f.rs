//! ADR 0135 (wave 48F, server part) checks that use the run service through the `rcfd-mcp` binary: F3 result shapes
//! on a project whose runs the tools started through 48A's runner, F11 run tools, F13 server start. They need `rcfd`
//! and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile; amendment 1 item 3).

mod common;

use common::*;
use serde_json::{json, Value};
use std::path::Path;
use std::time::{Duration, Instant};

const GOLDEN: &str = include_str!("golden/project_shapes.json");

/// Opaque and data-keyed paths of each tool (ADR 0135 amendment 2).
fn rules(key: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match key {
        "run_status" => (&["/settings", "/last_residual", "/events/*"], &[]),
        "read_metrics" => (&["/metrics/*"], &["/metrics"]),
        "project_info" => (&[], &["/runs_by_state"]),
        "list_runs" => (&[], &["/runs/*/metrics"]),
        "read_residuals" => (&[], &["/series"]),
        "compare_runs" => (&[], &["/metrics", "/metrics/*", "/differences", "/differences/*"]),
        "edit_case" | "edit_case_dry_run" => (&[], &["/values"]),
        _ => (&[], &[]),
    }
}

struct Shapes(serde_json::Map<String, Value>);

impl Shapes {
    fn record(&mut self, key: &str, result: &Value) {
        assert_eq!(result["isError"], false, "{key}: {result}");
        let (opaque, maps) = rules(key);
        let v = if key == "render_field" {
            result.clone()
        } else {
            result["structuredContent"].clone()
        };
        self.0.insert(key.to_string(), shape(&v, opaque, maps));
    }
}

/// Call `wait_run` (at most 60 s each) until the run is terminal; the last result and the total time.
fn wait_terminal(m: &mut Mcp, run: &str, cap: Duration) -> (Value, Duration) {
    let t0 = Instant::now();
    loop {
        let r = m.call("wait_run", json!({"run": run, "timeout_s": 60}));
        let v = r["structuredContent"].clone();
        assert_eq!(r["isError"], false, "{r}");
        if v["terminal"] == true {
            return (r, t0.elapsed());
        }
        assert!(t0.elapsed() < cap, "run {run} not ended after {cap:?}: {v}");
    }
}

fn wait_state(m: &mut Mcp, run: &str, want: &str, cap: Duration) -> Duration {
    let t0 = Instant::now();
    loop {
        let v = m.call("run_status", json!({"run": run, "events": 0}))["structuredContent"].clone();
        if v["status"]["state"] == want {
            return t0.elapsed();
        }
        assert!(
            t0.elapsed() < cap,
            "run {run} not {want} after {cap:?}: {}",
            v["status"]
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Stops the project's runner (solvers of ended runs are gone; a cancelled long run has stopped) on drop.
struct Runner(std::path::PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

/// F3 (result shapes) and F11: the tools on a project whose runs they start through the run service.
#[test]
fn f3_f11_project_tools_on_runner_made_runs() {
    binaries();
    let dir = project("f3");
    let _runner = Runner(dir.clone());
    std::fs::copy(dir.join("cases/cavity.toml"), dir.join("cases/edit.toml")).unwrap();
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    let mut sh = Shapes(serde_json::Map::new());
    let init = m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("project folder"));
    let list = m.request("tools/list", json!({}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 50);

    // F11 part 1: start_run then wait_run reaches finished on a 30-step cavity
    let r = m.call("start_run", json!({"case": "cases/cavity.toml"}));
    sh.record("start_run", &r);
    let a = r["structuredContent"]["run"].as_str().unwrap().to_string();
    let (w, took) = wait_terminal(&mut m, &a, Duration::from_secs(300));
    sh.record("wait_run", &w);
    let st = &w["structuredContent"];
    eprintln!(
        "F11: run {a} {} at step {} after {took:?}",
        st["state"], st["status"]["step"]
    );
    assert_eq!(st["state"], "finished", "{st}");
    assert_eq!(st["status"]["step"], 30);
    // control: wait_run with a 0 s timeout returns the current state without waiting
    let t0 = Instant::now();
    let z = m.call("wait_run", json!({"run": a, "timeout_s": 0, "from": "finished"}));
    // the default timeout is 30 s: well under that means it did not wait
    assert!(t0.elapsed() < Duration::from_secs(5));
    assert_eq!(z["structuredContent"]["changed"], false);

    let r = m.call(
        "start_run",
        json!({"case": "cases/cavity.toml", "set": ["solver.max_iterations=20"]}),
    );
    let b = r["structuredContent"]["run"].as_str().unwrap().to_string();
    let (wb, _) = wait_terminal(&mut m, &b, Duration::from_secs(300));
    assert_eq!(wb["structuredContent"]["state"], "finished");
    assert_eq!(wb["structuredContent"]["status"]["step"], 20);

    // ADR 0135 amendment 3 M1: the runner's metrics.json carries the final report values; list_runs and compare_runs
    // use them
    let mut finals = std::collections::BTreeMap::new();
    for id in [&a, &b] {
        let rd = dir.join("runs").join(id);
        let met: Value = serde_json::from_str(&std::fs::read_to_string(rd.join("metrics.json")).unwrap()).unwrap();
        let mon: Value =
            serde_json::from_str(&std::fs::read_to_string(rd.join("output/monitors.json")).unwrap()).unwrap();
        let want = mon["series"]["lid_drag"]["y"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_f64()
            .unwrap();
        let got = met["reports"]["lid_drag"].as_f64().unwrap();
        for k in [
            "state",
            "reason",
            "exit_code",
            "steps",
            "time",
            "solver_finish",
            "last_residuals",
        ] {
            assert!(met.get(k).is_some(), "{id}: metrics.json lost {k}");
        }
        assert_eq!(met["reports"].as_object().unwrap().len(), 1, "{}", met["reports"]);
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "{id}: reports.lid_drag {got:e} vs monitors.json {want:e}"
        );
        eprintln!("M1 {id}: reports.lid_drag {got:e} = monitors.json final {want:e}");
        finals.insert(id.clone(), got);
    }
    let lr = m.call("list_runs", json!({}))["structuredContent"].clone();
    for r in lr["runs"].as_array().unwrap() {
        let id = r["id"].as_str().unwrap();
        assert_eq!(r["metrics"], json!({"lid_drag": finals[id]}), "list_runs {id}");
    }
    let cr = m.call("compare_runs", json!({"runs": [a, b]}))["structuredContent"].clone();
    let mut per_run = serde_json::Map::new();
    per_run.insert(a.clone(), json!(finals[&a]));
    per_run.insert(b.clone(), json!(finals[&b]));
    assert_eq!(cr["metrics"], json!({"lid_drag": per_run}));
    assert_eq!(
        cr["differences"]["lid_drag"][&b].as_f64().unwrap().to_bits(),
        (finals[&b] - finals[&a]).to_bits()
    );

    // the read tools on the two finished runs
    sh.record("project_info", &m.call("project_info", json!({})));
    sh.record("list_runs", &m.call("list_runs", json!({})));
    sh.record("run_status", &m.call("run_status", json!({"run": a, "events": 1})));
    sh.record("read_metrics", &m.call("read_metrics", json!({"runs": [a, b]})));
    sh.record(
        "read_residuals",
        &m.call("read_residuals", json!({"run": a, "max_points": 10})),
    );
    sh.record(
        "field_summary",
        &m.call("field_summary", json!({"run": a, "field": "p"})),
    );
    sh.record(
        "render_field",
        &m.call("render_field", json!({"run": a, "field": "p", "width": 128})),
    );
    sh.record("compare_runs", &m.call("compare_runs", json!({"runs": [a, b]})));
    let t = m.call("list_changes", json!({}));
    sh.record("list_changes_token", &t);
    let token = t["structuredContent"]["token"].clone();
    let f = m.call("read_project_file", json!({"path": "cases/edit.toml"}));
    sh.record("read_project_file", &f);
    let h = f["structuredContent"]["hash"].clone();
    sh.record(
        "edit_case_dry_run",
        &m.call(
            "edit_case",
            json!({"path": "cases/edit.toml", "base_hash": h, "set": {"physics.nu": 0.02}, "dry_run": true}),
        ),
    );
    sh.record(
        "edit_case",
        &m.call(
            "edit_case",
            json!({"path": "cases/edit.toml", "base_hash": h, "set": {"physics.nu": 0.02}}),
        ),
    );
    sh.record(
        "write_note",
        &m.call("write_note", json!({"path": "notes/f3.md", "text": "shapes"})),
    );
    let c = m.call("list_changes", json!({"since": token}));
    sh.record("list_changes", &c);
    let changes: Vec<&str> = c["structuredContent"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(
        changes.contains(&"cases/edit.toml") && changes.contains(&"notes/f3.md"),
        "{changes:?}"
    );

    // F11 part 2: the control tools on a long run; cancel_run gives cancelled within 5.0 s
    let r = m.call(
        "start_run",
        json!({"case": "cases/cavity.toml", "set": ["solver.max_iterations=10000000"]}),
    );
    let long = r["structuredContent"]["run"].as_str().unwrap().to_string();
    wait_state(&mut m, &long, "running", Duration::from_secs(120));
    sh.record("pause_run", &m.call("pause_run", json!({"run": long})));
    let paused = wait_state(&mut m, &long, "paused", Duration::from_secs(30));
    sh.record("resume_run", &m.call("resume_run", json!({"run": long})));
    wait_state(&mut m, &long, "running", Duration::from_secs(30));
    let t0 = Instant::now();
    let r = m.call("cancel_run", json!({"run": long}));
    sh.record("cancel_run", &r);
    let (w, _) = wait_terminal(&mut m, &long, Duration::from_secs(60));
    let cancel_s = t0.elapsed().as_secs_f64();
    let st = &w["structuredContent"]["status"];
    eprintln!(
        "F11: pause took {paused:?}; cancelled {} after {cancel_s:.3} s at step {} ({})",
        w["structuredContent"]["state"], st["step"], st["reason"]
    );
    assert_eq!(w["structuredContent"]["state"], "cancelled");
    assert!(cancel_s <= 5.0, "cancel took {cancel_s:.3} s");
    // a run that ended refuses control with 1002
    assert_eq!(err_code(&m.call("pause_run", json!({"run": a}))), 1002);

    // F3: every shape equals the hand-written golden
    let golden: Value = serde_json::from_str(GOLDEN).unwrap();
    let mut bad = Vec::new();
    for (k, want) in golden.as_object().unwrap() {
        match sh.0.get(k) {
            Some(got) if got == want => {}
            Some(got) => bad.push(format!("{k}:\n  got  {got}\n  want {want}")),
            None => bad.push(format!("{k}: not recorded")),
        }
    }
    eprintln!(
        "F3: {} of {} result shapes equal the golden",
        golden.as_object().unwrap().len() - bad.len(),
        golden.as_object().unwrap().len()
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// ADR 0135 amendment 3 R1: `--project` without `--root` confines the existing tools' paths to the project, whatever
/// the working folder; an explicit `--root` wins.
#[test]
fn r1_project_implies_root() {
    let dir = project("r1");
    let elsewhere = scratch("r1-cwd");
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start_in(&["--project", &d], &elsewhere);
    let r = m.call("create_case", json!({"path": "cases/cavity.toml"}));
    assert_eq!(r["isError"], false, "{r}");
    let r = m.call("create_case", json!({"path": "../x.toml"}));
    assert_eq!(err_code(&r), 1004, "{r}");
    // control: an explicit --root is used instead
    let other = elsewhere.to_string_lossy().to_string();
    let mut m2 = Mcp::start_in(&["--project", &d, "--root", &other], &elsewhere);
    let r = m2.call("create_case", json!({"path": "cases/cavity.toml"}));
    assert_eq!(r["isError"], true, "{r}");
    eprintln!("R1: cases/cavity.toml resolved in the project without --root; ../x refused 1004; with --root <other> not found");
}

/// F13: `--project <not a project>` exits non-zero naming the path; with a project the server answers `initialize`.
#[test]
fn f13_server_start() {
    let not = scratch("f13").join("not-a-project");
    std::fs::create_dir_all(&not).unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .arg("--project")
        .arg(&not)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    eprintln!("F13: exit {:?}, stderr: {}", o.status.code(), err.trim());
    assert!(!o.status.success());
    assert!(err.contains(&*not.to_string_lossy()), "{err}");
    let dir = project("f13ok");
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let r = m.request("initialize", json!({}));
    assert_eq!(r["result"]["serverInfo"]["name"], "rcfd");
    assert_eq!(
        m.request("tools/list", json!({}))["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
}
