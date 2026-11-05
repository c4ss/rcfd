//! ADR 0189 C4-R, C4-M, C4-T and C4-C (ADR 0173 C4): through `rcfd-mcp --project`, `design_run_study` and `design_evaluate`
//! evaluate each candidate at each operating condition as one run of the project's runner, tagged in `run.json` with its
//! study, variant and condition, never more than `max_runs` at once; the records equal those of the in-process evaluator
//! (the same tools on a server without a project) within the registered limit; the wall time per evaluation of both is
//! recorded; a chained study with several conditions, and a study with its own step cap, stay in the server process.
//! Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// ADR 0189 C4-M: the largest relative difference of a metric between the two evaluators.
const REL_TOL: f64 = 1e-12;

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

/// The built-in `lid_driven_cavity` template at `n` x `n` cells with `max_iterations = 3000` (ADR 0189 C4-R, C4-C).
fn cavity_case(n: usize) -> String {
    let t = rcfd_mcp::LID_DRIVEN_CAVITY;
    for s in ["nx = 16", "ny = 16", "max_iterations = 600"] {
        assert!(t.contains(s), "the template has no '{s}'");
    }
    t.replace("nx = 16", &format!("nx = {n}"))
        .replace("ny = 16", &format!("ny = {n}"))
        .replace("max_iterations = 600", "max_iterations = 3000")
}

fn write_cases(dir: &Path) {
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::write(dir.join("cases/cavity.toml"), cavity_case(32)).unwrap();
    std::fs::write(dir.join("cases/small.toml"), cavity_case(16)).unwrap();
}

fn nu3_args() -> Value {
    json!({
        "dir": "designs/nu3", "base": "cases/cavity.toml", "name": "nu3",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": {"type": "factorial", "levels": [3]},
        "workers": 3
    })
}

/// The study of C4-C: two levels of `nu` on the small case, two conditions.
fn two_args(dir: &str, extra: Value) -> Value {
    let mut a = json!({
        "dir": dir, "base": "cases/small.toml", "name": "two",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "conditions": [
            {"label": "base"},
            {"label": "slow", "set": {"boundaries.lid.velocity_value": [0.5, 0.0, 0.0]}}
        ],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": {"type": "factorial", "levels": [2]},
        "workers": 2
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn run_folders(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("runs"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("r-"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

/// The state events of a run: (time, from, to).
fn state_events(dir: &Path, run: &str) -> Vec<(String, Value, String)> {
    std::fs::read_to_string(dir.join("runs").join(run).join("events.jsonl"))
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["kind"] == "state")
        .map(|v| {
            (
                v["t"].as_str().unwrap().to_string(),
                v["from"].clone(),
                v["to"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn is_terminal(s: &str) -> bool {
    matches!(s, "finished" | "failed" | "cancelled")
}

/// The largest number of `runs` at one time between their `running` and their terminal state event, and whether every
/// run was created before the first terminal event of any of them.
fn peak_and_queued_together(dir: &Path, runs: &[String]) -> (usize, bool) {
    let mut marks: Vec<(String, i32)> = Vec::new();
    let mut created = Vec::new();
    let mut ended = Vec::new();
    for r in runs {
        for (t, from, to) in state_events(dir, r) {
            if from.is_null() && to == "queued" {
                created.push(t);
            } else if to == "running" {
                marks.push((t, 1));
            } else if is_terminal(&to) {
                ended.push(t.clone());
                marks.push((t, -1));
            }
        }
    }
    // an end and a start at the same instant: the end first
    marks.sort();
    let (mut now, mut peak) = (0i32, 0i32);
    for (_, d) in &marks {
        now += d;
        peak = peak.max(now);
    }
    let first_end = ended.iter().min().cloned().unwrap_or_default();
    let together = created.len() == runs.len() && created.iter().all(|c| *c < first_end);
    (peak as usize, together)
}

/// The record `index` of a study, through `design_get_metrics`.
fn record_bin(m: &mut Mcp, dir: &str, index: usize) -> Value {
    ok(&m.call("design_get_metrics", json!({"dir": dir, "index": index})))
}

fn record_in(s: &McpServer, dir: &str, index: usize) -> Value {
    ok(&call(s, "design_get_metrics", json!({"dir": dir, "index": index})))
}

/// ADR 0189 C4-M on one pair of records: the largest relative difference of a metric (panics when another field
/// differs, a metric name differs, or the in-process record is not usable with at least one metric).
fn compare_records(runner: &Value, process: &Value, what: &str) -> f64 {
    assert_eq!(
        process["ok"], true,
        "{what}: the in-process record is not usable: {process}"
    );
    assert_eq!(runner["params"], process["params"], "{what}");
    assert_eq!(runner["ok"], process["ok"], "{what}: {runner}");
    let (rc, pc) = (
        runner["conditions"].as_array().unwrap(),
        process["conditions"].as_array().unwrap(),
    );
    assert_eq!(rc.len(), pc.len(), "{what}");
    let mut worst = 0.0f64;
    for (r, p) in rc.iter().zip(pc) {
        for k in ["label", "ok", "status", "iterations"] {
            assert_eq!(r[k], p[k], "{what}: condition {k}: runner {r} process {p}");
        }
        let (rm, pm) = (r["metrics"].as_object().unwrap(), p["metrics"].as_object().unwrap());
        assert!(!pm.is_empty(), "{what}: no metric to compare");
        let mut rk: Vec<&String> = rm.keys().collect();
        let mut pk: Vec<&String> = pm.keys().collect();
        rk.sort();
        pk.sort();
        assert_eq!(rk, pk, "{what}: metric names");
        for (k, pv) in pm {
            let (a, b) = (rm[k].as_f64().unwrap(), pv.as_f64().unwrap());
            let rel = if a == b {
                0.0
            } else {
                (a - b).abs() / a.abs().max(b.abs())
            };
            worst = worst.max(rel);
            assert!(
                rel <= REL_TOL,
                "{what}: {} {k}: runner {a:e} process {b:e} (relative {rel:e})",
                r["label"]
            );
        }
    }
    worst
}

/// The run of each runner record condition: its `run.json` tags and `set` entries.
fn check_tags(dir: &Path, run: &str, study: &str, param: f64, label: &str, extra_set: &[&str]) {
    let rec = read_json(&dir.join("runs").join(run).join("run.json"));
    assert_eq!(rec["study"], study, "{rec}");
    assert_eq!(rec["condition"], label, "{rec}");
    let variant = rec["variant"].as_str().unwrap();
    let v: f64 = variant.strip_prefix("nu=").unwrap().parse().unwrap();
    assert_eq!(v.to_bits(), param.to_bits(), "variant {variant} for {param}");
    let set: Vec<String> = serde_json::from_value(rec["settings"]["set"].clone()).unwrap();
    let p: f64 = set[0].strip_prefix("physics.nu=").unwrap().parse().unwrap();
    assert_eq!(p.to_bits(), param.to_bits(), "set {set:?}");
    assert_eq!(&set[1..], extra_set, "{set:?}");
}

#[test]
fn c4_design_studies_run_on_the_runner_in_project_mode() {
    binaries();
    let root = scratch("f274-c4");
    let dir = root.join("proj");
    Project::create(&dir, "f274 c4", "test").unwrap();
    write_cases(&dir);
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    std::fs::write(
        dir.join(".rcfd/runner-config.json"),
        r#"{"max_runs": 2, "cpu_threads": 4, "mem_budget_mb": 65536}"#,
    )
    .unwrap();
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    // the in-process reference: the same tools on a server without a project, over a copy of the cases
    let refdir = root.join("ref");
    write_cases(&refdir);
    let s = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(refdir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );

    // C4-R: three points, three tagged runs, at most max_runs = 2 at once
    ok(&m.call("design_create_study", nu3_args()));
    let t0 = Instant::now();
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/nu3"})));
    let wall_runner = t0.elapsed().as_secs_f64();
    eprintln!("C4-R design_run_study: {v}");
    assert_eq!(v["state"], "complete", "{v}");
    assert_eq!(v["evaluations_run"], 3);
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["runs_total"], 3);
    let runs = run_folders(&dir);
    assert_eq!(runs.len(), 3, "{runs:?}");
    let listed: Vec<&str> = v["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["run"].as_str().unwrap())
        .collect();
    let mut listed_sorted = listed.clone();
    listed_sorted.sort();
    assert_eq!(listed_sorted, runs.iter().map(String::as_str).collect::<Vec<_>>());
    let mut values = Vec::new();
    for i in 0..3 {
        values.push(record_bin(&mut m, "designs/nu3", i)["params"]["nu"].as_f64().unwrap());
    }
    for r in &runs {
        let st = read_json(&dir.join("runs").join(r).join("status.json"));
        assert_eq!(st["state"], "finished", "{r}: {st}");
        let rec = read_json(&dir.join("runs").join(r).join("run.json"));
        assert_eq!(rec["case"], "cases/cavity.toml", "{rec}");
        let variant = rec["variant"].as_str().unwrap();
        let nu: f64 = variant.strip_prefix("nu=").unwrap().parse().unwrap();
        let param = *values
            .iter()
            .find(|x| x.to_bits() == nu.to_bits())
            .unwrap_or_else(|| panic!("variant {variant} is none of the store's values {values:?}"));
        check_tags(&dir, r, "designs/nu3", param, "base", &[]);
    }
    let (peak, together) = peak_and_queued_together(&dir, &runs);
    eprintln!("C4-R: 3 runs {runs:?}, peak running {peak}, queued together {together}, {wall_runner:.2} s");
    assert!(together, "the three runs were not queued together");
    assert_eq!(peak, 2, "max_runs = 2");
    // a second call evaluates nothing and makes no run
    let v2 = ok(&m.call("design_run_study", json!({"dir": "designs/nu3"})));
    assert_eq!(v2["evaluations_run"], 0, "{v2}");
    assert_eq!(v2["runs_total"], 0, "{v2}");
    assert_eq!(run_folders(&dir), runs);

    // C4-M: the same study in process
    let mut a = nu3_args();
    a["workers"] = json!(3);
    ok(&call(&s, "design_create_study", a));
    let t0 = Instant::now();
    let w = ok(&call(&s, "design_run_study", json!({"dir": "designs/nu3"})));
    let wall_process = t0.elapsed().as_secs_f64();
    assert_eq!(w["evaluations_run"], 3, "{w}");
    assert!(
        w.get("evaluator").is_none(),
        "a server without a project adds nothing: {w}"
    );
    let mut worst = 0.0f64;
    for i in 0..3 {
        let (r, p) = (record_bin(&mut m, "designs/nu3", i), record_in(&s, "designs/nu3", i));
        worst = worst.max(compare_records(&r, &p, &format!("record {i}")));
        eprintln!(
            "C4-M record {i}: nu {} iterations {} lid_drag runner {} process {}",
            p["params"]["nu"],
            p["conditions"][0]["iterations"],
            r["conditions"][0]["metrics"]["lid_drag"],
            p["conditions"][0]["metrics"]["lid_drag"]
        );
    }
    eprintln!("C4-M: largest relative difference {worst:e} (limit {REL_TOL:e})");

    // C4-T: one new point each way, and the three-point calls
    let t0 = Instant::now();
    let r1 = ok(&m.call(
        "design_evaluate",
        json!({"dir": "designs/nu3", "params": {"nu": 0.0175}}),
    ));
    let one_runner = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let p1 = ok(&call(
        &s,
        "design_evaluate",
        json!({"dir": "designs/nu3", "params": {"nu": 0.0175}}),
    ));
    let one_process = t0.elapsed().as_secs_f64();
    assert_eq!(r1["evaluator"], "runner", "{r1}");
    assert_eq!(r1["runs_total"], 1, "{r1}");
    let worst1 = compare_records(&r1, &p1, "the C4-T point");
    eprintln!(
        "C4-T: one evaluation {one_runner:.3} s on the runner, {one_process:.3} s in process, extra {:.3} s; \
         three points (workers 3, max_runs 2) {wall_runner:.3} s and {wall_process:.3} s, per evaluation {:.3} s and \
         {:.3} s; relative difference {worst1:e}",
        one_runner - one_process,
        wall_runner / 3.0,
        wall_process / 3.0
    );
    let runs = run_folders(&dir);
    assert_eq!(runs.len(), 4);

    // C4-C (i): two conditions, chain false: four tagged runs, records equal to the in-process ones
    ok(&m.call("design_create_study", two_args("designs/two", json!({"chain": false}))));
    ok(&call(
        &s,
        "design_create_study",
        two_args("designs/two", json!({"chain": false})),
    ));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two"})));
    eprintln!("C4-C (i): {v}");
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["evaluations_run"], 2);
    assert_eq!(v["runs_total"], 4);
    let w = ok(&call(&s, "design_run_study", json!({"dir": "designs/two"})));
    assert_eq!(w["evaluations_run"], 2);
    let after: Vec<String> = run_folders(&dir).into_iter().filter(|r| !runs.contains(r)).collect();
    assert_eq!(after.len(), 4, "{after:?}");
    for i in 0..2 {
        let (r, p) = (record_bin(&mut m, "designs/two", i), record_in(&s, "designs/two", i));
        let d = compare_records(&r, &p, &format!("two record {i}"));
        eprintln!("C4-C (i) record {i}: relative difference {d:e}");
        worst = worst.max(d);
    }
    let mut seen = Vec::new();
    for item in v["runs"].as_array().unwrap() {
        let run = item["run"].as_str().unwrap();
        let label = item["condition"].as_str().unwrap();
        let nu: f64 = item["variant"]
            .as_str()
            .unwrap()
            .strip_prefix("nu=")
            .unwrap()
            .parse()
            .unwrap();
        let extra: &[&str] = if label == "slow" {
            &["boundaries.lid.velocity_value=[0.5,0.0,0.0]"]
        } else {
            &[]
        };
        check_tags(&dir, run, "designs/two", nu, label, extra);
        assert_eq!(item["state"], "finished", "{item}");
        seen.push(format!("{label} {nu}"));
    }
    seen.sort();
    eprintln!("C4-C (i) runs: {seen:?}");
    assert_eq!(seen.len(), 4);
    let before = run_folders(&dir);

    // C4-C (ii): chain left to its default (true) with two conditions: in process, no run
    ok(&m.call("design_create_study", two_args("designs/two-chain", json!({}))));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-chain"})));
    eprintln!("C4-C (ii): evaluator {} note {}", v["evaluator"], v["evaluator_note"]);
    assert_eq!(v["evaluator"], "process", "{v}");
    assert!(v["evaluator_note"].as_str().unwrap().contains("chain"), "{v}");
    assert_eq!(v["evaluations_run"], 2);
    assert!(v.get("runs").is_none());
    assert_eq!(run_folders(&dir), before, "a chained study made runs");

    // C4-C (iii): its own step cap: in process, no run
    ok(&m.call(
        "design_create_study",
        two_args("designs/two-steps", json!({"chain": false, "max_steps": 3000})),
    ));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-steps"})));
    eprintln!("C4-C (iii): evaluator {} note {}", v["evaluator"], v["evaluator_note"]);
    assert_eq!(v["evaluator"], "process", "{v}");
    assert!(v["evaluator_note"].as_str().unwrap().contains("max_steps"), "{v}");
    assert_eq!(run_folders(&dir), before, "a study with max_steps made runs");
    eprintln!("C4: largest relative difference over every compared record {worst:e}");
}
