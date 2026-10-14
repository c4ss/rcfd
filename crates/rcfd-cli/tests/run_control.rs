//! Headless launch and run-control contract of `rcfd run` and `rcfd workflow` (F-120, F-121).
//!
//! Pass/fail criteria were fixed before running: documented exit codes (0 ok, 2 usage, 3 invalid,
//! 5 not converged, 7 interrupted, 8 wall-clock limit), a status stream on stdout that is nothing
//! but `rcfd-status` JSON lines, a continued run that converges to the uninterrupted result, and
//! results that do not depend on the thread count.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_runctl_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const CASE: &str = r#"
title = "run-control cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-7
tolerance_p = 1e-7
"#;

fn case_file(dir: &Path) -> String {
    let p = dir.join("case.toml");
    std::fs::write(&p, CASE).unwrap();
    p.display().to_string()
}

fn events(o: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {l}")))
        .collect()
}

#[test]
fn status_json_stdout_is_only_status_events_and_matches_the_exit_code() {
    let d = tmp("stream");
    let case = case_file(&d);
    let out = d.join("out");
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        out.to_str().unwrap(),
        "--status-json",
        "--status-every",
        "25",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let ev = events(&o);
    assert_eq!(ev[0]["event"], "start");
    assert_eq!(ev[0]["info"]["driver"], "cli");
    let fin = ev.last().unwrap();
    assert_eq!(
        (fin["event"].clone(), fin["state"].clone(), fin["exit_code"].clone()),
        ("finish".into(), "converged".into(), 0.into())
    );
    for (i, e) in ev.iter().enumerate() {
        assert_eq!(e["format"], "rcfd-status");
        assert_eq!(e["version"], 1);
        assert_eq!(e["seq"], i as u64 + 1);
    }
    // iterations 1..5, then every 25th, and the last one
    let idx: Vec<u64> = ev
        .iter()
        .filter(|e| e["event"] == "iteration")
        .map(|e| e["index"].as_u64().unwrap())
        .collect();
    assert_eq!(&idx[..6], &[1, 2, 3, 4, 5, 25]);
    assert_eq!(*idx.last().unwrap(), fin["iterations"].as_u64().unwrap());
    // human text went to stderr
    assert!(String::from_utf8_lossy(&o.stderr).contains("results written"));
    assert!(out.join("solution.vtu").exists() && out.join("restart.rcfd").exists());
    // the stream is consumable by the summary used by the MCP server
    let stream = String::from_utf8_lossy(&o.stdout).to_string();
    let sum = rcfd_api::runctl::summarize_stream(&stream).unwrap();
    assert_eq!(sum["state"], "converged");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn exit_codes_check_only_not_converged_stop_file_and_time_limit() {
    let d = tmp("codes");
    let case = case_file(&d);
    let out = d.join("out");
    let outs = out.to_str().unwrap();
    // check-only: ok, then invalid (3)
    let o = rcfd(&["run", &case, "--check-only"]);
    assert_eq!(o.status.code(), Some(0));
    let bad = d.join("bad.toml");
    std::fs::write(&bad, CASE.replace("nu = 0.01", "nu = -1.0")).unwrap();
    assert_eq!(
        rcfd(&["run", bad.to_str().unwrap(), "--check-only"]).status.code(),
        Some(3)
    );
    assert_eq!(rcfd(&["run", bad.to_str().unwrap()]).status.code(), Some(3));
    // iteration cap -> 5, checkpoint written, then continue-from converges (0)
    let o = rcfd(&["run", &case, "--out", outs, "--max-iterations", "20", "--quiet"]);
    assert_eq!(o.status.code(), Some(5), "{}", String::from_utf8_lossy(&o.stderr));
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        outs,
        "--continue-from",
        out.join("restart.rcfd").to_str().unwrap(),
        "--quiet",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    // stop file already present -> 7 before any step
    let stop = d.join("STOP");
    std::fs::write(&stop, "").unwrap();
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        outs,
        "--stop-file",
        stop.to_str().unwrap(),
        "--status-json",
    ]);
    assert_eq!(o.status.code(), Some(7));
    let ev = events(&o);
    assert_eq!(ev.last().unwrap()["state"], "interrupted");
    // zero wall-clock budget is a usage-valid value that stops at once -> 8 is only for > 0 limits
    let o = rcfd(&["run", &case, "--out", outs, "--max-elapsed", "0.000001", "--quiet"]);
    assert_eq!(o.status.code(), Some(8), "{}", String::from_utf8_lossy(&o.stderr));
    // usage errors -> 2, with the reason
    for (args, needle) in [
        (vec!["run", &case, "--threads", "0"], "positive"),
        (vec!["run", &case, "--threads", "many"], "number"),
        (vec!["run", &case, "--precision", "single"], "double"),
        (vec!["run", &case, "--hostfile", "h.txt"], "distributed"),
        (vec!["run"], "exactly one"),
    ] {
        let o = rcfd(&args);
        assert_eq!(o.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains(needle),
            "{args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn continued_run_matches_the_uninterrupted_one_and_threads_do_not_change_results() {
    let d = tmp("cont");
    let case = case_file(&d);
    let full = d.join("full");
    let a = d.join("a");
    let b = d.join("b");
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        full.to_str().unwrap(),
        "--threads",
        "1",
        "--quiet",
        "--status-json",
    ]);
    assert_eq!(o.status.code(), Some(0));
    let full_iters = events(&o).last().unwrap()["iterations"].as_u64().unwrap();
    let o = rcfd(&["run", &case, "--out", b.to_str().unwrap(), "--threads", "4", "--quiet"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        std::fs::read(full.join("solution.vtu")).unwrap(),
        std::fs::read(b.join("solution.vtu")).unwrap(),
        "1 and 4 threads give different results"
    );
    // interrupt through the iteration cap, continue and compare to the uninterrupted file
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        a.to_str().unwrap(),
        "--max-iterations",
        "40",
        "--quiet",
    ]);
    assert_eq!(o.status.code(), Some(5));
    let ck = a.join("restart.rcfd");
    let o = rcfd(&[
        "run",
        &case,
        "--out",
        a.to_str().unwrap(),
        "--continue-from",
        ck.to_str().unwrap(),
        "--status-json",
        "--quiet",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let ev = events(&o);
    assert_eq!(
        ev[0]["start_iteration"], 0,
        "iteration counter is restored by the first step"
    );
    assert_eq!(ev.last().unwrap()["iterations"].as_u64().unwrap(), full_iters);
    assert_eq!(
        std::fs::read(full.join("solution.vtu")).unwrap(),
        std::fs::read(a.join("solution.vtu")).unwrap(),
        "continued run differs from the uninterrupted run"
    );
    // a checkpoint from a different mesh is refused (exit 1)
    let other = d.join("other.toml");
    std::fs::write(&other, CASE.replace("nx = 12", "nx = 10")).unwrap();
    let o = rcfd(&[
        "run",
        other.to_str().unwrap(),
        "--continue-from",
        ck.to_str().unwrap(),
        "--out",
        d.join("x").to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("different mesh"));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn journal_runs_the_recorded_case() {
    let d = tmp("journal");
    let j = d.join("j.json");
    let journal = serde_json::json!({
        "format": "rcfd-journal", "version": 1,
        "entries": [
            {"op": "create", "source": {"toml": CASE}},
            {"op": "set", "path": "mesh.nx", "value": 9},
            {"op": "set", "path": "mesh.ny", "value": 9},
        ],
    });
    std::fs::write(&j, journal.to_string()).unwrap();
    let o = rcfd(&[
        "run",
        "--journal",
        j.to_str().unwrap(),
        "--out",
        d.join("o").to_str().unwrap(),
        "--status-json",
        "--quiet",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(events(&o)[0]["cells"], 9 * 9);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn workflow_command_runs_a_template_and_maps_failures_to_exit_codes() {
    let d = tmp("wf");
    let case = case_file(&d);
    let o = rcfd(&["workflow", "templates"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("mesh_to_results"));
    let o = rcfd(&["workflow", "show", "--template", "channel_flow"]);
    let doc: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(doc["format"], "rcfd-workflow");
    // run the whole template in a scratch directory (export writes ./results)
    let o = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .current_dir(&d)
        .args(["workflow", "run", &case, "--template", "channel_flow", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let r: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(r["ok"], true);
    assert_eq!(r["workflow"]["complete"], true);
    assert!(d.join("results/solution.vtu").exists());
    // iteration cap: the solve task fails and the CLI exits with the run's code (5)
    let o = rcfd(&["workflow", "run", &case, "--task", "solve", "--max-steps", "5"]);
    assert_eq!(
        o.status.code(),
        Some(5),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    // a bad case maps to 3 via the validate task
    let bad = d.join("bad.toml");
    std::fs::write(&bad, CASE.replace("nu = 0.01", "nu = -1.0")).unwrap();
    let o = rcfd(&["workflow", "run", bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(3), "{}", String::from_utf8_lossy(&o.stderr));
    // unknown template -> usage
    assert_eq!(
        rcfd(&["workflow", "run", &case, "--template", "nope"]).status.code(),
        Some(2)
    );
    std::fs::remove_dir_all(&d).ok();
}
