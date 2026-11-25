//! ADR 0794 (F-274): the journal lines of design runs and of stored design records, through `rcfd-mcp --project`: which lines a
//! cold, a chained and a step-capped study, an operating map, a study resumed after a restart, a failed evaluation and the other
//! design tools write, in which order and by whom; what the feed shows of them; two servers in one project; and that records,
//! tables, run records and replies are the bytes of main (row B, `design_journal_f274.tsv`).
//! Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

/// The built-in `lid_driven_cavity` template at 16 x 16 cells with `max_iterations = 3000`.
fn small_case() -> String {
    let t = rcfd_mcp::LID_DRIVEN_CAVITY;
    assert!(t.contains("max_iterations = 600") && t.contains("nx = 16"));
    t.replace("max_iterations = 600", "max_iterations = 3000")
}

/// A project with `cases/small.toml` and the registered runner configuration.
fn new_project(name: &str) -> (PathBuf, Runner) {
    binaries();
    let dir = scratch(&format!("f274j-{name}")).join("proj");
    Project::create(&dir, name, "test").unwrap();
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::write(dir.join("cases/small.toml"), small_case()).unwrap();
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    std::fs::write(
        dir.join(".rcfd/runner-config.json"),
        r#"{"max_runs": 2, "cpu_threads": 4, "mem_budget_mb": 65536}"#,
    )
    .unwrap();
    (dir.clone(), Runner(dir))
}

fn start(dir: &Path) -> Mcp {
    let mut m = Mcp::start(&["--project", &dir.to_string_lossy()]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    m
}

/// A study on `cases/small.toml` with the parameter `nu` in [0.01, 0.02].
fn study(dir: &str, extra: Value) -> Value {
    let mut a = json!({
        "dir": dir, "base": "cases/small.toml", "name": "s",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "objectives": [{"metric": "lid_drag", "sense": "min"}]
    });
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

fn nu3(dir: &str) -> Value {
    study(
        dir,
        json!({"method": {"type": "factorial", "levels": [3]}, "workers": 3}),
    )
}

fn two(dir: &str) -> Value {
    study(
        dir,
        json!({"method": {"type": "factorial", "levels": [2]}, "workers": 2, "conditions": [
            {"label": "base"}, {"label": "slow", "set": {"boundaries.lid.velocity_value": [0.5, 0.0, 0.0]}}]}),
    )
}

fn cap(dir: &str) -> Value {
    study(
        dir,
        json!({"method": {"type": "factorial", "levels": [2]}, "workers": 2, "max_steps": 5}),
    )
}

/// The map of ADR 0200 C-MAP.
fn map3(out: &str) -> Value {
    json!({"base": "cases/small.toml", "out_dir": out, "axes": [{"path": "boundaries.lid.velocity_value",
        "values": [[1.0, 0.0, 0.0], [0.75, 0.0, 0.0], [0.5, 0.0, 0.0]]}]})
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

/// Every run folder's id and `run.json`.
fn runs_of(dir: &Path) -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = std::fs::read_dir(dir.join("runs"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("r-"))
                .map(|n| (n.clone(), read_json(&dir.join("runs").join(&n).join("run.json"))))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

// ---------------------------------------------------------------- row B

/// Replaces the project's path and the run ids in texts by tokens.
struct Norm {
    roots: Vec<String>,
    runs: Vec<(String, String)>,
}

impl Norm {
    fn new(dir: &Path) -> Norm {
        let slashed = |p: &Path| p.to_string_lossy().replace('\\', "/").replace("//?/", "");
        let mut roots = vec![slashed(&dir.canonicalize().unwrap()), slashed(dir)];
        roots.dedup();
        // the runs in the order of what they are
        let mut runs: Vec<(String, String)> = runs_of(dir)
            .into_iter()
            .map(|(id, r)| {
                let set = &r["settings"]["set"];
                (format!("{}|{}|{}|{set}", r["study"], r["variant"], r["condition"]), id)
            })
            .collect();
        runs.sort();
        let runs = runs.into_iter().map(|(k, id)| (id, format!("<run {k}>"))).collect();
        Norm { roots, runs }
    }

    fn text(&self, s: &str) -> String {
        let mut t = s.replace("\\\\", "/").replace('\\', "/").replace("//?/", "");
        for r in &self.roots {
            t = t.replace(r, "<root>");
        }
        for (id, name) in &self.runs {
            t = t.replace(id, name);
        }
        t
    }

    fn value(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.text(s)),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.value(x)).collect()),
            Value::Object(o) => {
                let mut m: Map<String, Value> = o.iter().map(|(k, x)| (k.clone(), self.value(x))).collect();
                // the order of a result's `runs` is the order in which the workers submitted
                if let Some(Value::Array(runs)) = m.get_mut("runs") {
                    runs.sort_by_key(Value::to_string);
                }
                Value::Object(m)
            }
            other => other.clone(),
        }
    }
}

fn pick(v: &Value, keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .map(|k| (k.to_string(), v.get(*k).cloned().unwrap_or(Value::Null)))
            .collect(),
    )
}

/// The items of row B: name and text. With `whole` also what changes with work that is not F-274 (ADR 0794 amendment 1): the
/// tool list and the text of `solver_version`.
fn items_of_main(name: &str, whole: bool) -> Vec<(String, String)> {
    let (dir, _runner) = new_project(name);
    let mut m = start(&dir);
    let mut replies: Vec<(String, Value)> = Vec::new();
    let tools = m.request("tools/list", json!({}));
    let calls = [
        ("design_create_study", nu3("designs/nu3")),
        ("design_create_study", two("designs/two")),
        ("design_create_study", cap("designs/cap")),
        ("design_run_study", json!({"dir": "designs/nu3"})),
        ("design_run_study", json!({"dir": "designs/nu3"})),
        ("design_run_study", json!({"dir": "designs/two"})),
        ("design_run_study", json!({"dir": "designs/cap"})),
        (
            "design_evaluate",
            json!({"dir": "designs/nu3", "params": {"nu": 0.0175}}),
        ),
        (
            "design_record",
            json!({"dir": "designs/nu3", "params": {"nu": 0.0125}, "metrics": {"lid_drag": 0.25}}),
        ),
        ("design_run_operating_map", map3("designs/map")),
        ("design_run_operating_map", map3("designs/map")),
        (
            "design_create_variant",
            json!({"path": "designs/v.toml", "dir": "designs/nu3", "params": {"nu": 0.015}}),
        ),
    ];
    for (i, (tool, args)) in calls.iter().enumerate() {
        replies.push((format!("reply {i:02} {tool}"), ok(&m.call(tool, args.clone()))));
    }
    drop(m);
    let n = Norm::new(&dir);
    let mut out = Vec::new();
    if whole {
        out.push(("tools/list".to_string(), tools["result"].to_string()));
    }
    for (k, v) in &replies {
        out.push((k.clone(), n.value(v).to_string()));
    }
    for f in [
        "designs/nu3/study.json",
        "designs/nu3/evaluations.jsonl",
        "designs/two/study.json",
        "designs/two/evaluations.jsonl",
        "designs/cap/study.json",
        "designs/cap/evaluations.jsonl",
        "designs/map/map.csv",
        "designs/map/map.json",
        "designs/map/map.meta.json",
        "designs/v.toml",
    ] {
        out.push((f.to_string(), n.text(&std::fs::read_to_string(dir.join(f)).unwrap())));
    }
    for (id, token) in &n.runs {
        let rec = read_json(&dir.join("runs").join(id).join("run.json"));
        let keys: Vec<&String> = rec.as_object().unwrap().keys().collect();
        let mut kept = rec.clone();
        for k in ["id", "created", "case_source", "case_dir"] {
            kept.as_object_mut().unwrap().remove(k);
        }
        if !whole {
            kept.as_object_mut().unwrap().remove("solver_version");
        }
        out.push((format!("{token} run.json"), format!("{keys:?} {}", n.value(&kept))));
        let mj = read_json(&dir.join("runs").join(id).join("metrics.json"));
        let mut kept = pick(
            &mj,
            &["state", "reason", "exit_code", "steps", "reports", "last_residuals"],
        );
        kept["solver_finish"] = pick(&mj["solver_finish"], &["state", "exit_code", "iterations", "reason"]);
        out.push((format!("{token} metrics.json"), n.value(&kept).to_string()));
    }
    out
}

/// Row B: records, tables, run records, metrics and replies are the bytes of main (`design_journal_f274.tsv`, made by this
/// test on the library of 25311db5, Windows x86_64; `RCFD_F274_RECORD=<file>` writes the items in place of comparing,
/// and with `RCFD_F274_WHOLE` whole, with the tool list).
#[test]
fn b_the_bytes_of_main() {
    let items = items_of_main("b", std::env::var_os("RCFD_F274_WHOLE").is_some());
    let text: String = items
        .iter()
        .map(|(k, t)| format!("{k}\t{}\n", hash_bytes(t.as_bytes())))
        .collect();
    if let Some(f) = std::env::var_os("RCFD_F274_RECORD") {
        std::fs::write(&f, &text).unwrap();
        let full: String = items.iter().map(|(k, t)| format!("{k}\t{t}\n")).collect();
        std::fs::write(PathBuf::from(f).with_extension("full.txt"), full).unwrap();
        return;
    }
    let main = include_str!("design_journal_f274.tsv");
    assert_eq!(text.lines().count(), main.lines().count());
    if !cfg!(all(windows, target_arch = "x86_64")) {
        eprintln!("B: the record of main is of Windows x86_64; the names alone are compared here");
        let names = |t: &str| -> Vec<String> { t.lines().map(|l| l.split('\t').next().unwrap().to_string()).collect() };
        assert_eq!(names(&text), names(main));
        return;
    }
    let differ: Vec<(&str, &str)> = text.lines().zip(main.lines()).filter(|(a, b)| a != b).collect();
    eprintln!("B: {} items, {} differ", text.lines().count(), differ.len());
    assert!(differ.is_empty(), "{differ:#?}");
}
