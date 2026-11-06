//! ADR 0201 A-R and A-N: in project mode a study's call adopts a run of the project's runner that already is the evaluation
//! of a candidate at a condition (the same tags, case, case bytes, entries, launch settings and solver), so a server stopped
//! during a study and started again submits no second run for a candidate that has one, and its records equal those of a
//! study that was not interrupted; a run that differs in one item of the key, or that failed or was cancelled, is not
//! adopted. Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::project::ProjectTools;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use rcfd_runner::records::{LaunchSettings, RunSettings, RunState};
use rcfd_runner::store::RunTags;
use rcfd_workspace::runs::RunService;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// ADR 0201: the limit of every metric comparison (records of the same runs, or of cold runs in process).
const REL_EXACT: f64 = 1e-12;

fn runner_exe() -> PathBuf {
    binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX))
}

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let _ = rcfd_runner::client::Client::new(&self.0, runner_exe()).shutdown(Duration::from_secs(10));
    }
}

/// One `rcfd-mcp --project` process that can be killed in the middle of a call.
struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    fn start(project: &Path) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .args(["--project", &project.to_string_lossy()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("rcfd-mcp starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut s = Server {
            child,
            stdin,
            stdout,
            next: 1,
        };
        s.request("initialize", json!({"protocolVersion": "2025-06-18"}));
        s
    }

    fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{req}").unwrap();
        self.stdin.flush().unwrap();
        id
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(v["id"], id, "{v}");
        v
    }

    /// A tool's `structuredContent` (no error).
    fn call(&mut self, tool: &str, args: Value) -> Value {
        let r = self.request("tools/call", json!({"name": tool, "arguments": args}));
        assert!(r.get("error").is_none(), "{tool}: {r}");
        ok(&r["result"])
    }

    fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The built-in `lid_driven_cavity` template at `n` x `n` cells with `max_iterations = 3000` (ADR 0189).
fn cavity_case(n: usize) -> String {
    rcfd_mcp::LID_DRIVEN_CAVITY
        .replace("nx = 16", &format!("nx = {n}"))
        .replace("ny = 16", &format!("ny = {n}"))
        .replace("max_iterations = 600", "max_iterations = 3000")
}

fn new_project(root: &Path, max_runs: usize) -> PathBuf {
    let dir = root.join("proj");
    Project::create(&dir, "f288", "test").unwrap();
    write_cases(&dir);
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    std::fs::write(
        dir.join(".rcfd/runner-config.json"),
        format!(r#"{{"max_runs": {max_runs}, "cpu_threads": 4, "mem_budget_mb": 65536}}"#),
    )
    .unwrap();
    dir
}

fn write_cases(dir: &Path) {
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::write(dir.join("cases/cavity.toml"), cavity_case(32)).unwrap();
    std::fs::write(dir.join("cases/small.toml"), cavity_case(16)).unwrap();
}

/// ADR 0189 C4-R's study, in folder `dir`.
fn nu3_args(dir: &str) -> Value {
    json!({
        "dir": dir, "base": "cases/cavity.toml", "name": "nu3",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": {"type": "factorial", "levels": [3]},
        "workers": 3
    })
}

/// ADR 0200 C-CH's chained study, in folder `dir`.
fn chain_args(dir: &str) -> Value {
    json!({
        "dir": dir, "base": "cases/small.toml", "name": "two",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "conditions": [
            {"label": "base"},
            {"label": "slow", "set": {"boundaries.lid.velocity_value": [0.5, 0.0, 0.0]}}
        ],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": {"type": "factorial", "levels": [2]},
        "workers": 2
    })
}

/// The study `dir` of `nu = 0.015` at `base` only (A-N).
fn nu1_args(dir: &str) -> Value {
    json!({
        "dir": dir, "base": "cases/cavity.toml", "name": "nu1",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": {"type": "factorial", "levels": [2]},
        "workers": 1
    })
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

/// Every run folder, with its `run.json` when it reads.
fn runs_of(dir: &Path) -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = std::fs::read_dir(dir.join("runs"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("r-"))
                .filter_map(|n| {
                    let rec = std::fs::read(dir.join("runs").join(&n).join("run.json")).ok()?;
                    Some((n, serde_json::from_slice(&rec).ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn study_runs(dir: &Path, study: &str) -> Vec<(String, Value)> {
    runs_of(dir).into_iter().filter(|(_, r)| r["study"] == study).collect()
}

fn state_of(dir: &Path, run: &str) -> String {
    read_json(&dir.join("runs").join(run).join("status.json"))["state"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Records `a` and `b` agree in params, ok, status and iterations, metrics within `REL_EXACT`; the largest difference.
fn same_record(a: &Value, b: &Value, what: &str) -> f64 {
    assert_eq!(a["params"], b["params"], "{what}");
    assert_eq!(a["ok"], b["ok"], "{what}");
    let (ac, bc) = (a["conditions"].as_array().unwrap(), b["conditions"].as_array().unwrap());
    assert_eq!(ac.len(), bc.len(), "{what}");
    let mut worst = 0.0f64;
    for (x, y) in ac.iter().zip(bc) {
        for k in ["label", "ok", "status", "iterations"] {
            assert_eq!(x[k], y[k], "{what}: {k}: {x} / {y}");
        }
        let (xm, ym) = (x["metrics"].as_object().unwrap(), y["metrics"].as_object().unwrap());
        assert_eq!(
            xm.keys().collect::<Vec<_>>(),
            ym.keys().collect::<Vec<_>>(),
            "{what}: metric names"
        );
        for (k, v) in ym {
            let d = rel(xm[k].as_f64().unwrap(), v.as_f64().unwrap());
            assert!(d <= REL_EXACT, "{what}: {} {k}: {} / {v} ({d:e})", x["label"], xm[k]);
            worst = worst.max(d);
        }
    }
    worst
}

fn metrics(s: &mut Server, dir: &str, i: usize) -> Value {
    s.call("design_get_metrics", json!({"dir": dir, "index": i}))
}

/// Send `design_run_study` on `s` and kill `s` as soon as the project holds `n` runs of study `study`, before the answer
/// is read; the states of those runs at the kill.
fn kill_during(mut s: Server, dir: &Path, study: &str, n: usize) -> Vec<(String, String)> {
    s.send(
        "tools/call",
        json!({"name": "design_run_study", "arguments": {"dir": study}}),
    );
    let t0 = Instant::now();
    loop {
        let runs = study_runs(dir, study);
        if runs.len() >= n {
            s.kill();
            return runs.iter().map(|(r, _)| (r.clone(), state_of(dir, r))).collect();
        }
        assert!(
            t0.elapsed() < Duration::from_secs(120),
            "{study}: {} runs after 120 s",
            runs.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_r_a_server_stopped_during_a_study_adopts_its_runs() {
    binaries();
    let root = scratch("f288-ar");
    let dir = new_project(&root, 1);
    let _runner = Runner(dir.clone());
    // the in-process reference
    let refdir = root.join("ref");
    write_cases(&refdir);
    let p = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(refdir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );

    // (i) three points, the server killed once the three runs are queued
    let mut s1 = Server::start(&dir);
    s1.call("design_create_study", nu3_args("designs/nu3"));
    let at_kill = kill_during(s1, &dir, "designs/nu3", 3);
    eprintln!("A-R (i): at the kill {at_kill:?}");
    assert_eq!(at_kill.len(), 3);
    assert!(
        at_kill
            .iter()
            .any(|(_, st)| !matches!(st.as_str(), "finished" | "failed" | "cancelled")),
        "set-up: every run had ended at the kill"
    );
    let mut s2 = Server::start(&dir);
    let t0 = Instant::now();
    let v = s2.call("design_run_study", json!({"dir": "designs/nu3"}));
    eprintln!("A-R (i) second server, {:.2} s: {v}", t0.elapsed().as_secs_f64());
    assert_eq!(v["evaluations_run"], 3, "{v}");
    assert_eq!(v["runs_total"], 0, "{v}");
    assert_eq!(v["runs_adopted"], 3, "{v}");
    for r in v["runs"].as_array().unwrap() {
        assert_eq!(r["adopted"], true, "{r}");
    }
    let all = runs_of(&dir);
    assert_eq!(
        all.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),
        at_kill.iter().map(|(r, _)| r.clone()).collect::<Vec<_>>(),
        "a run was added"
    );
    for (r, _) in &all {
        assert_eq!(state_of(&dir, r), "finished", "{r}");
    }
    // the same definition without interruption, and in process
    s2.call("design_create_study", nu3_args("designs/nu3-ref"));
    let w = s2.call("design_run_study", json!({"dir": "designs/nu3-ref"}));
    assert_eq!(w["runs_adopted"], 0, "{w}");
    assert_eq!(w["runs_total"], 3, "{w}");
    assert_eq!(runs_of(&dir).len(), 6);
    ok(&call(&p, "design_create_study", nu3_args("designs/nu3")));
    ok(&call(&p, "design_run_study", json!({"dir": "designs/nu3"})));
    let mut worst = 0.0f64;
    for i in 0..3 {
        let (a, b) = (
            metrics(&mut s2, "designs/nu3", i),
            metrics(&mut s2, "designs/nu3-ref", i),
        );
        let c = ok(&call(
            &p,
            "design_get_metrics",
            json!({"dir": "designs/nu3", "index": i}),
        ));
        assert_eq!(c["ok"], true, "the in-process record is not usable: {c}");
        worst = worst.max(same_record(
            &a,
            &b,
            &format!("A-R (i) record {i} against the uninterrupted study"),
        ));
        worst = worst.max(same_record(&a, &c, &format!("A-R (i) record {i} against in process")));
        eprintln!(
            "A-R (i) record {i}: nu {} iterations {} lid_drag {}",
            a["params"]["nu"], a["conditions"][0]["iterations"], a["conditions"][0]["metrics"]["lid_drag"]
        );
    }
    let t0 = Instant::now();
    let index = rcfd_workspace::design::RunIndex::read(&RunService::new(&dir, runner_exe()));
    eprintln!(
        "A-R (i): index read of {} runs in {:.4} s; largest relative difference {worst:e}",
        index.len(),
        t0.elapsed().as_secs_f64()
    );

    // (ii) the chained study, killed once two of its runs exist
    s2.call("design_create_study", chain_args("designs/two-chain"));
    let at_kill = kill_during(s2, &dir, "designs/two-chain", 2);
    eprintln!("A-R (ii): at the kill {at_kill:?}");
    let mut s3 = Server::start(&dir);
    let v = s3.call("design_run_study", json!({"dir": "designs/two-chain"}));
    eprintln!("A-R (ii) third server: {v}");
    assert_eq!(v["evaluations_run"], 2, "{v}");
    let runs = study_runs(&dir, "designs/two-chain");
    let mut pairs: Vec<(String, String)> = runs
        .iter()
        .map(|(_, r)| {
            (
                r["variant"].as_str().unwrap().to_string(),
                r["condition"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    pairs.sort();
    let mut once = pairs.clone();
    once.dedup();
    assert_eq!(pairs, once, "a (variant, condition) has two runs: {runs:?}");
    assert_eq!(pairs.len(), 4, "{pairs:?}");
    assert_eq!(v["runs_adopted"], json!(at_kill.len()), "{v}");
    assert_eq!(v["runs_total"], json!(4 - at_kill.len()), "{v}");
    for (run, r) in &runs {
        if r["condition"] == "slow" {
            let (base, _) = runs
                .iter()
                .find(|(_, b)| b["condition"] == "base" && b["variant"] == r["variant"])
                .unwrap();
            assert_eq!(r["settings"]["continue_from"], json!(base), "{run}: {r}");
        }
    }
    s3.call("design_create_study", chain_args("designs/two-chain-ref"));
    let w = s3.call("design_run_study", json!({"dir": "designs/two-chain-ref"}));
    assert_eq!(
        (w["runs_total"].clone(), w["runs_adopted"].clone()),
        (json!(4), json!(0)),
        "{w}"
    );
    for i in 0..2 {
        let (a, b) = (
            metrics(&mut s3, "designs/two-chain", i),
            metrics(&mut s3, "designs/two-chain-ref", i),
        );
        worst = worst.max(same_record(&a, &b, &format!("A-R (ii) record {i}")));
        eprintln!(
            "A-R (ii) record {i}: nu {} iterations {} and {}",
            a["params"]["nu"], a["conditions"][0]["iterations"], a["conditions"][1]["iterations"]
        );
    }
    eprintln!("A-R: largest relative difference {worst:e} (limit {REL_EXACT:e})");
}

#[test]
fn a_n_a_run_that_differs_in_one_item_is_not_adopted() {
    binaries();
    let root = scratch("f288-an");
    let dir = new_project(&root, 2);
    std::fs::copy(dir.join("cases/cavity.toml"), dir.join("cases/copy.toml")).unwrap();
    let _runner = Runner(dir.clone());
    let s = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(dir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    )
    .with_project(ProjectTools::open(&dir).unwrap().with_runner_exe(runner_exe()));
    let refdir = root.join("ref");
    write_cases(&refdir);
    let p = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(refdir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );
    ok(&call(&p, "design_create_study", nu1_args("designs/nu1")));
    ok(&call(
        &p,
        "design_evaluate",
        json!({"dir": "designs/nu1", "params": {"nu": 0.015}}),
    ));
    let reference = ok(&call(
        &p,
        "design_get_metrics",
        json!({"dir": "designs/nu1", "index": 0}),
    ));
    assert_eq!(reference["ok"], true, "{reference}");
    ok(&call(&s, "design_create_study", nu1_args("designs/nu1")));
    ok(&call(&s, "design_create_study", nu1_args("designs/pos")));
    let rs = RunService::new(&dir, runner_exe());
    let case = dir.join("cases/cavity.toml");
    let full = RunTags {
        study: Some("designs/nu1".into()),
        variant: Some("nu=0.015".into()),
        condition: Some("base".into()),
    };
    let set = |extra: &[&str]| RunSettings {
        set: ["physics.nu=0.015"]
            .iter()
            .chain(extra)
            .map(|x| x.to_string())
            .collect(),
        ..RunSettings::default()
    };
    let none = LaunchSettings::default();
    let tags = |f: &dyn Fn(&mut RunTags)| {
        let mut t = full.clone();
        f(&mut t);
        t
    };
    let mut decoys: Vec<(&str, String)> = Vec::new();
    let mut submit = |what: &'static str, case: &Path, s: &RunSettings, l: &LaunchSettings, t: &RunTags| -> String {
        let id = rs.submit_with(case, s, l, t).unwrap();
        decoys.push((what, id.clone()));
        id
    };
    submit(
        "(a) variant",
        &case,
        &set(&[]),
        &none,
        &tags(&|t| t.variant = Some("nu=0.0151".into())),
    );
    submit(
        "(b) condition",
        &case,
        &set(&[]),
        &none,
        &tags(&|t| t.condition = Some("other".into())),
    );
    submit(
        "(c) study",
        &case,
        &set(&[]),
        &none,
        &tags(&|t| t.study = Some("designs/other".into())),
    );
    submit("(d) one more entry", &case, &set(&["solver.alpha_p=0.3"]), &none, &full);
    let original = std::fs::read(&case).unwrap();
    std::fs::write(&case, [original.as_slice(), b"# decoy\n"].concat()).unwrap();
    submit("(e) case bytes", &case, &set(&[]), &none, &full);
    std::fs::write(&case, &original).unwrap();
    let steps = LaunchSettings {
        max_steps: Some(5000),
        ..LaunchSettings::default()
    };
    submit("(f) max_steps", &case, &set(&[]), &steps, &full);
    submit("(g) case", &dir.join("cases/copy.toml"), &set(&[]), &none, &full);
    let h = submit("(h) solver_version", &case, &set(&[]), &none, &full);
    let hp = dir.join("runs").join(&h).join("run.json");
    let mut hr = read_json(&hp);
    hr["solver_version"] = json!("rcfd 0.0.0-decoy");
    rcfd_project::atomic::atomic_write(&hp, serde_json::to_string_pretty(&hr).unwrap().as_bytes()).unwrap();
    let i = submit("(i) cancelled", &case, &set(&[]), &none, &full);
    let big = RunSettings {
        mem_mb: Some(10_000_000),
        ..set(&[])
    };
    let j = submit("(j) failed", &case, &big, &none, &full);
    for (what, id) in &decoys {
        if what.starts_with("(i)") || what.starts_with("(j)") {
            continue;
        }
        rs.pause(id).unwrap();
    }
    rs.cancel(&i).unwrap();
    rs.ensure_runner().unwrap();
    assert_eq!(rs.wait(&i, Duration::from_secs(60)).unwrap().state, RunState::Cancelled);
    assert_eq!(rs.wait(&j, Duration::from_secs(60)).unwrap().state, RunState::Failed);
    let before: Vec<String> = runs_of(&dir).into_iter().map(|(r, _)| r).collect();
    let v = ok(&call(
        &s,
        "design_evaluate",
        json!({"dir": "designs/nu1", "params": {"nu": 0.015}}),
    ));
    eprintln!("A-N: {v}");
    assert_eq!(v["runs_total"], 1, "{v}");
    assert_eq!(v["runs_adopted"], 0, "{v}");
    let run = v["runs"][0]["run"].as_str().unwrap().to_string();
    assert!(!before.contains(&run), "the run is not new: {run}");
    assert!(v["runs"][0].get("adopted").is_none(), "{v}");
    for (what, id) in &decoys {
        assert_ne!(id, &run, "{what} was adopted");
        eprintln!("A-N decoy {what}: {id} {}", state_of(&dir, id));
    }
    let worst = same_record(&v, &reference, "A-N against in process");
    // (k) the positive control: a queued run with the full key of designs/pos, adopted and waited for
    rcfd_runner::client::Client::new(&dir, runner_exe())
        .shutdown(Duration::from_secs(10))
        .unwrap();
    let k = rs
        .submit_with(
            &case,
            &set(&[]),
            &none,
            &RunTags {
                study: Some("designs/pos".into()),
                ..full.clone()
            },
        )
        .unwrap();
    assert_eq!(state_of(&dir, &k), "queued");
    let w = ok(&call(
        &s,
        "design_evaluate",
        json!({"dir": "designs/pos", "params": {"nu": 0.015}}),
    ));
    eprintln!("A-N (k): {w}");
    assert_eq!(w["runs_total"], 0, "{w}");
    assert_eq!(w["runs_adopted"], 1, "{w}");
    assert_eq!(w["runs"][0]["run"], json!(k), "{w}");
    assert_eq!(w["runs"][0]["adopted"], true, "{w}");
    assert_eq!(state_of(&dir, &k), "finished");
    let worst = worst.max(same_record(&w, &reference, "A-N (k) against in process"));
    for (_, id) in &decoys {
        let _ = rs.cancel(id);
    }
    eprintln!("A-N: 10 decoys not adopted, the control adopted; largest relative difference {worst:e}");
}
