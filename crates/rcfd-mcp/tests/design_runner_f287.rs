//! ADR 0200 C-CH, C-MS and C-MAP (with amendment 1): through `rcfd-mcp --project`, a chained study with several conditions,
//! a study with its own step cap and an operating map are evaluated on the project's runner; a chained condition continues
//! from the run of the condition before, a map point from the run of the converged point before; the records equal those of
//! the in-process evaluator (the same tools on a server without a project) within the registered limits.
//! Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// ADR 0200: a run that starts as the in-process one does (cold).
const REL_EXACT: f64 = 1e-12;
/// ADR 0200: a run continued on the runner against the same condition seeded in process.
const REL_CHAIN: f64 = 1e-4;

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

/// The built-in `lid_driven_cavity` template at `n` x `n` cells with `max_iterations = 3000` (ADR 0189).
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
    std::fs::write(dir.join("cases/small.toml"), cavity_case(16)).unwrap();
}

/// The study of ADR 0189 C4-C: two levels of `nu` on the small case, the conditions `base` and `slow`.
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

fn lines(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

fn record_bin(m: &mut Mcp, dir: &str, index: usize) -> Value {
    ok(&m.call("design_get_metrics", json!({"dir": dir, "index": index})))
}

fn record_in(s: &McpServer, dir: &str, index: usize) -> Value {
    ok(&call(s, "design_get_metrics", json!({"dir": dir, "index": index})))
}

/// The largest relative metric difference of each kind between a runner and an in-process record: (cold conditions,
/// continued conditions), with `continued` the labels of the continued conditions; panics when a field other than the
/// iterations of a continued condition differs, or a limit is missed. `usable`: the in-process record must be usable.
fn compare(runner: &Value, process: &Value, what: &str, continued: &[&str], usable: bool) -> (f64, f64) {
    if usable {
        assert_eq!(
            process["ok"], true,
            "{what}: the in-process record is not usable: {process}"
        );
    }
    assert_eq!(runner["params"], process["params"], "{what}");
    assert_eq!(runner["ok"], process["ok"], "{what}: {runner}");
    let (rc, pc) = (
        runner["conditions"].as_array().unwrap(),
        process["conditions"].as_array().unwrap(),
    );
    assert_eq!(rc.len(), pc.len(), "{what}");
    let (mut exact, mut chain) = (0.0f64, 0.0f64);
    for (r, p) in rc.iter().zip(pc) {
        let label = p["label"].as_str().unwrap();
        let cont = continued.contains(&label);
        for k in ["label", "ok", "status"] {
            assert_eq!(r[k], p[k], "{what}: condition {k}: runner {r} process {p}");
        }
        if !cont {
            assert_eq!(r["iterations"], p["iterations"], "{what}: {label} iterations");
        }
        let (rm, pm) = (r["metrics"].as_object().unwrap(), p["metrics"].as_object().unwrap());
        if usable {
            assert!(!pm.is_empty(), "{what}: no metric to compare");
        }
        let mut rk: Vec<&String> = rm.keys().collect();
        let mut pk: Vec<&String> = pm.keys().collect();
        rk.sort();
        pk.sort();
        assert_eq!(rk, pk, "{what}: metric names");
        for (k, pv) in pm {
            let (a, b) = (rm[k].as_f64().unwrap(), pv.as_f64().unwrap());
            let d = rel(a, b);
            let tol = if cont { REL_CHAIN } else { REL_EXACT };
            assert!(
                d <= tol,
                "{what}: {label} {k}: runner {a:e} process {b:e} (relative {d:e}, limit {tol:e})"
            );
            if cont {
                chain = chain.max(d);
            } else {
                exact = exact.max(d);
            }
        }
        eprintln!(
            "  {what} {label}: iterations runner {} process {}, lid_drag runner {} process {}",
            r["iterations"], p["iterations"], r["metrics"]["lid_drag"], p["metrics"]["lid_drag"]
        );
    }
    (exact, chain)
}

/// The study's runs listed in a result, as (run, variant, condition, continue_from).
fn listed(v: &Value) -> Vec<(String, String, String, Option<String>)> {
    v["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["run"].as_str().unwrap().to_string(),
                r["variant"].as_str().unwrap_or("").to_string(),
                r["condition"].as_str().unwrap_or("").to_string(),
                r["continue_from"].as_str().map(String::from),
            )
        })
        .collect()
}

fn same_file(a: &str, b: &Path) -> bool {
    match (Path::new(a).canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

#[test]
fn c_ch_c_ms_c_map_chains_step_caps_and_maps_on_the_runner() {
    binaries();
    let root = scratch("f287-c");
    let dir = root.join("proj");
    Project::create(&dir, "f287 c", "test").unwrap();
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
    let refdir = root.join("ref");
    write_cases(&refdir);
    let s = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(refdir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );
    let run_dir = |r: &str| dir.join("runs").join(r);

    // C-CH: a chained study (chain left to its default) on the runner
    ok(&m.call("design_create_study", two_args("designs/two-chain", json!({}))));
    ok(&call(
        &s,
        "design_create_study",
        two_args("designs/two-chain", json!({})),
    ));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-chain"})));
    eprintln!("C-CH: {v}");
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["evaluations_run"], 2);
    assert_eq!(v["runs_total"], 4);
    let runs = listed(&v);
    assert_eq!(runs.len(), 4);
    for (run, variant, cond, cont) in &runs {
        let rec = read_json(&run_dir(run).join("run.json"));
        assert_eq!(rec["condition"], json!(cond), "{rec}");
        assert_eq!(rec["variant"], json!(variant), "{rec}");
        match cond.as_str() {
            "base" => {
                assert!(rec["settings"].get("continue_from").is_none(), "{rec}");
                assert!(cont.is_none());
            }
            _ => {
                let (base, ..) = runs.iter().find(|(_, va, c, _)| va == variant && c == "base").unwrap();
                assert_eq!(rec["settings"]["continue_from"], json!(base), "{rec}");
                assert_eq!(cont.as_deref(), Some(base.as_str()));
                // amendment 1: the restart file's iteration is the base run's last, the slow run's first step follows it
                let restart = run_dir(base).join("output").join("restart.rcfd");
                let it = rcfd_workspace::design::restart_iteration(&restart).expect("the base run's restart file");
                let base_fin = read_json(&run_dir(base).join("metrics.json"))["solver_finish"]["iterations"].clone();
                assert_eq!(json!(it), base_fin, "{base}");
                let first = lines(&run_dir(run).join("residuals.jsonl"))[0]["step"]
                    .as_u64()
                    .unwrap();
                assert_eq!(first, it + 1, "{run}");
                let start = lines(&run_dir(run).join("solver-status.jsonl"))
                    .into_iter()
                    .find(|e| e["event"] == "start")
                    .unwrap();
                assert!(
                    same_file(start["info"]["continue_from"].as_str().unwrap(), &restart),
                    "{start}"
                );
                // S7: the record's iterations plus the restart's iteration are the run's total
                let fin = read_json(&run_dir(run).join("metrics.json"))["solver_finish"]["iterations"]
                    .as_u64()
                    .unwrap();
                let idx = if variant.ends_with("0.01") { 0 } else { 1 };
                let recd = record_bin(&mut m, "designs/two-chain", idx);
                assert_eq!(
                    recd["params"]["nu"].as_f64().map(|x| format!("nu={x}")),
                    Some(variant.clone())
                );
                let slow_it = recd["conditions"][1]["iterations"].as_u64().unwrap();
                assert_eq!(slow_it + it, fin, "{run}: {recd}");
                eprintln!(
                    "C-CH {variant}: base {base} ended at {it}; slow {run} steps {}..{fin}, record {slow_it}",
                    it + 1
                );
            }
        }
    }
    let w = ok(&call(&s, "design_run_study", json!({"dir": "designs/two-chain"})));
    assert_eq!(w["evaluations_run"], 2, "{w}");
    let (mut exact, mut chain) = (0.0f64, 0.0f64);
    for i in 0..2 {
        let (r, p) = (
            record_bin(&mut m, "designs/two-chain", i),
            record_in(&s, "designs/two-chain", i),
        );
        let (e, c) = compare(&r, &p, &format!("C-CH record {i}"), &["slow"], true);
        exact = exact.max(e);
        chain = chain.max(c);
    }
    eprintln!("C-CH: largest relative difference: cold {exact:e} (limit {REL_EXACT:e}), continued {chain:e} (limit {REL_CHAIN:e})");
    // for comparison only: the slow iterations of the same definition cold on the runner
    ok(&m.call(
        "design_create_study",
        two_args("designs/two-cold", json!({"chain": false})),
    ));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-cold"})));
    assert_eq!(v["evaluator"], "runner");
    for i in 0..2 {
        let r = record_bin(&mut m, "designs/two-cold", i);
        eprintln!(
            "C-CH cold reference record {i}: nu {} slow iterations {}",
            r["params"]["nu"], r["conditions"][1]["iterations"]
        );
    }

    // C-MS (i): chain false and max_steps 3000
    let before = run_folders(&dir);
    let a = two_args("designs/two-steps", json!({"chain": false, "max_steps": 3000}));
    ok(&m.call("design_create_study", a.clone()));
    ok(&call(&s, "design_create_study", a));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-steps"})));
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["runs_total"], 4);
    let new: Vec<String> = run_folders(&dir).into_iter().filter(|r| !before.contains(r)).collect();
    assert_eq!(new.len(), 4, "{new:?}");
    for r in &new {
        assert_eq!(
            read_json(&run_dir(r).join("run.json"))["settings"]["max_steps"],
            3000,
            "{r}"
        );
    }
    ok(&call(&s, "design_run_study", json!({"dir": "designs/two-steps"})));
    for i in 0..2 {
        let (r, p) = (
            record_bin(&mut m, "designs/two-steps", i),
            record_in(&s, "designs/two-steps", i),
        );
        let (e, _) = compare(&r, &p, &format!("C-MS (i) record {i}"), &[], true);
        exact = exact.max(e);
    }
    // C-MS (ii): chain left to its default, max_steps 20: every condition capped
    let before = run_folders(&dir);
    let a = two_args("designs/two-cap", json!({"max_steps": 20}));
    ok(&m.call("design_create_study", a.clone()));
    ok(&call(&s, "design_create_study", a));
    let v = ok(&m.call("design_run_study", json!({"dir": "designs/two-cap"})));
    eprintln!("C-MS (ii): {v}");
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["runs_total"], 4);
    for (run, _, _, cont) in listed(&v) {
        assert!(
            cont.is_none(),
            "{run}: a slow run continued from a base run that was not usable"
        );
        let rec = read_json(&run_dir(&run).join("run.json"));
        assert_eq!(rec["settings"]["max_steps"], 20, "{rec}");
        assert!(rec["settings"].get("continue_from").is_none(), "{rec}");
        let fin = read_json(&run_dir(&run).join("metrics.json"))["solver_finish"].clone();
        assert_eq!(fin["state"], "step_limit", "{run}: {fin}");
    }
    assert_eq!(run_folders(&dir).len(), before.len() + 4);
    ok(&call(&s, "design_run_study", json!({"dir": "designs/two-cap"})));
    for i in 0..2 {
        let (r, p) = (
            record_bin(&mut m, "designs/two-cap", i),
            record_in(&s, "designs/two-cap", i),
        );
        for c in r["conditions"].as_array().unwrap() {
            assert_eq!(c["ok"], false, "{c}");
            assert_eq!(c["status"], "not_converged", "{c}");
            assert_eq!(c["iterations"], 20, "{c}");
            assert_eq!(c["metrics"], json!({}), "{c}");
        }
        compare(&r, &p, &format!("C-MS (ii) record {i}"), &[], false);
    }

    // C-MAP: an operating map over the lid speed
    let before = run_folders(&dir);
    let map = json!({"base": "cases/small.toml", "out_dir": "designs/map",
        "axes": [{"path": "boundaries.lid.velocity_value", "values": [[1.0, 0.0, 0.0], [0.75, 0.0, 0.0], [0.5, 0.0, 0.0]]}]});
    let v = ok(&m.call("design_run_operating_map", map.clone()));
    eprintln!("C-MAP: {v}");
    let w = ok(&call(&s, "design_run_operating_map", map.clone()));
    assert_eq!(v["evaluator"], "runner", "{v}");
    assert_eq!(v["runs_total"], 3);
    assert!(w.get("evaluator").is_none(), "{w}");
    let pts = v["runs"].as_array().unwrap();
    assert_eq!(pts.len(), 3);
    for (i, r) in pts.iter().enumerate() {
        assert_eq!(r["point"], i, "{r}");
        assert!(r.get("variant").is_none() && r.get("condition").is_none(), "{r}");
        let rec = read_json(&run_dir(r["run"].as_str().unwrap()).join("run.json"));
        assert!(rec.get("study").is_none(), "a map run is untagged: {rec}");
        if i == 0 {
            assert!(r.get("continue_from").is_none(), "{r}");
        } else {
            assert_eq!(r["continue_from"], pts[i - 1]["run"], "{r}");
            assert_eq!(rec["settings"]["continue_from"], pts[i - 1]["run"], "{rec}");
        }
    }
    let (rows, prow) = (v["points"].as_array().unwrap(), w["points"].as_array().unwrap());
    assert_eq!(rows.len(), 3);
    let mut map_chain = 0.0f64;
    for (i, (r, p)) in rows.iter().zip(prow).enumerate() {
        assert_eq!(r["status"], "converged", "{r}");
        for k in ["status", "restart", "exit_code", "state"] {
            assert_eq!(r[k], p[k], "point {i} {k}: runner {r} process {p}");
        }
        assert_eq!(r["restart"], if i == 0 { "cold" } else { "warm" }, "{r}");
        let (a, b) = (
            r["metrics"]["lid_drag"].as_f64().unwrap(),
            p["metrics"]["lid_drag"].as_f64().unwrap(),
        );
        let d = rel(a, b);
        if i == 0 {
            assert_eq!(r["iterations"], p["iterations"], "point 0");
            assert!(d <= REL_EXACT, "point 0: {a:e} {b:e} ({d:e})");
            exact = exact.max(d);
        } else {
            assert!(d <= REL_CHAIN, "point {i}: {a:e} {b:e} ({d:e})");
            map_chain = map_chain.max(d);
        }
        eprintln!(
            "C-MAP point {i}: restart {} iterations runner {} process {}, lid_drag runner {a} process {b} (relative {d:e})",
            r["restart"], r["iterations"], p["iterations"]
        );
    }
    let again = ok(&m.call("design_run_operating_map", map));
    assert_eq!(again["cached"], true, "{again}");
    assert_eq!(run_folders(&dir).len(), before.len() + 3, "a cached map made runs");
    eprintln!(
        "C: largest relative difference: cold {exact:e} (limit {REL_EXACT:e}), chained study {chain:e}, map {map_chain:e} (limit {REL_CHAIN:e})"
    );
}
