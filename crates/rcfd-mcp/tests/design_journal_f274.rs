//! ADR 0794 (F-274): the journal lines of design runs and of stored design records, through `rcfd-mcp --project`: which lines a
//! cold, a chained and a step-capped study, an operating map, a study resumed after a restart, a failed evaluation and the other
//! design tools write, in which order and by whom; what the feed shows of them; two servers in one project; and that records,
//! tables, run records and replies are the bytes of main (row B, `design_journal_f274.tsv`).
//! Needs `rcfd` and `rcfd-runner` next to `rcfd-mcp` (one cargo build profile).

mod common;

use common::*;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_workspace::feed::{Change, Feed};
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

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

// ---------------------------------------------------------------- the journal

/// One `rcfd-mcp --project` process whose requests can be sent without reading the answer, and that can be killed.
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
        s.send("initialize", json!({"protocolVersion": "2025-06-18"}));
        s.receive();
        s
    }

    fn send(&mut self, method: &str, params: Value) {
        let req = json!({"jsonrpc": "2.0", "id": self.next, "method": method, "params": params});
        self.next += 1;
        writeln!(self.stdin, "{req}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn send_call(&mut self, tool: &str, args: Value) {
        self.send("tools/call", json!({"name": tool, "arguments": args}));
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.send_call(tool, args);
        ok(&self.receive()["result"])
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The lines of the project's journal (none when there is no file).
fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

/// The lines after the first `from`, with the whole journal's `seq` checked to be 1, 2, ... in file order.
fn new_lines(dir: &Path, from: usize) -> Vec<Value> {
    let all = journal(dir);
    for (i, l) in all.iter().enumerate() {
        assert_eq!(l["seq"], i + 1, "{l}");
    }
    all[from..].to_vec()
}

fn of_kind(lines: &[Value], kind: &str) -> Vec<Value> {
    lines.iter().filter(|l| l["kind"] == kind).cloned().collect()
}

fn kinds(lines: &[Value]) -> Vec<&str> {
    lines.iter().map(|l| l["kind"].as_str().unwrap()).collect()
}

fn pos(lines: &[Value], l: &Value) -> usize {
    lines.iter().position(|x| x == l).unwrap()
}

fn hash_of(p: &Path) -> String {
    hash_bytes(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
}

/// The keys, the command and the actor of a line.
fn check_head(l: &Value, kind: &str, who: &str, pid: u32, fields: &[&str]) {
    assert_eq!(l["kind"], kind, "{l}");
    assert_eq!(l["who"], who, "{l}");
    assert_eq!(l["actor"]["client"], "mcp", "{l}");
    assert_eq!(l["actor"]["instance"]["pid"], pid, "{l}");
    assert!(l["actor"]["instance"]["started"].is_string(), "{l}");
    assert!(l["t"].is_string(), "{l}");
    let mut want: Vec<&str> = fields
        .iter()
        .copied()
        .chain(["actor", "kind", "seq", "t", "who"])
        .collect();
    want.sort_unstable();
    let have: Vec<&String> = l.as_object().unwrap().keys().collect();
    assert_eq!(have, want, "{l}");
}

/// A `run_submitted` line against the `run.json` of its run: every field of ADR 0794 D1.
fn check_run_line(dir: &Path, l: &Value, who: &str, pid: u32) {
    let rec = read_json(&dir.join("runs").join(l["run"].as_str().unwrap()).join("run.json"));
    let set = &rec["settings"];
    let mut fields = vec!["run", "case", "set", "priority", "threads"];
    for (k, v) in [
        ("continue_from", &set["continue_from"]),
        ("max_steps", &set["max_steps"]),
        ("study", &rec["study"]),
        ("variant", &rec["variant"]),
        ("condition", &rec["condition"]),
    ] {
        if !v.is_null() {
            fields.push(k);
            assert_eq!(&l[k], v, "{k}: {l}");
        }
    }
    check_head(l, "run_submitted", who, pid, &fields);
    assert_eq!(l["case"], rec["case"], "{l}");
    for k in ["set", "priority", "threads"] {
        assert_eq!(l[k], set[k], "{k}: {l}");
    }
}

/// The records of a study's store and the length of the file up to the end of each.
fn store(dir: &Path, study: &str) -> (Vec<u8>, Vec<(Value, usize)>) {
    let bytes = std::fs::read(dir.join(study).join("evaluations.jsonl")).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    let mut at = 0;
    let mut out = Vec::new();
    for l in bytes.split_inclusive(|&b| b == b'\n') {
        at += l.len();
        let v: Value = serde_json::from_slice(l).unwrap();
        if v.get("index").is_some() {
            out.push((v, at));
        }
    }
    (bytes, out)
}

/// A `design_recorded` line against record `i` of the study's store: every field of ADR 0794 D2.
fn check_record_line(dir: &Path, study: &str, i: usize, l: &Value, who: &str, pid: u32) {
    let (bytes, recs) = store(dir, study);
    let (rec, end) = &recs[i];
    check_head(
        l,
        "design_recorded",
        who,
        pid,
        &[
            "path",
            "hash",
            "study",
            "variant",
            "index",
            "source",
            "ok",
            "conditions",
        ],
    );
    assert_eq!(l["path"], format!("{study}/evaluations.jsonl"), "{l}");
    assert_eq!(l["hash"], hash_bytes(&bytes[..*end]), "record {i}: {l}");
    if i + 1 == recs.len() {
        assert_eq!(l["hash"], hash_of(&dir.join(study).join("evaluations.jsonl")), "{l}");
    }
    assert_eq!(l["study"], study, "{l}");
    assert_eq!(l["index"], rec["index"], "{l}");
    assert_eq!(l["index"], i, "{l}");
    assert_eq!(l["source"], rec["source"], "{l}");
    let (name, value) = rec["params"].as_object().unwrap().iter().next().unwrap();
    assert_eq!(l["variant"], format!("{name}={value}"), "{l}");
    let conds = rec["conditions"].as_array().unwrap();
    assert_eq!(l["ok"], conds.iter().all(|c| c["ok"] == true), "{l}");
    let want: Vec<Value> = conds
        .iter()
        .map(|c| json!({"condition": c["label"], "status": c["status"]}))
        .collect();
    assert_eq!(l["conditions"], json!(want), "{l}");
}

/// The lines of one study's run: `n` candidates, `runs` submissions, each run of the project's study with one line and its
/// tags, the records in store order, each after the submissions of its candidate.
fn check_study(dir: &Path, study: &str, lines: &[Value], runs: usize, n: usize, who: &str, pid: u32) {
    let (r, d) = (of_kind(lines, "run_submitted"), of_kind(lines, "design_recorded"));
    assert_eq!((r.len(), d.len(), lines.len()), (runs, n, runs + n), "{lines:#?}");
    for l in &r {
        check_run_line(dir, l, who, pid);
        assert_eq!(l["study"], study, "{l}");
        assert!(l["variant"].is_string() && l["condition"].is_string(), "{l}");
    }
    let mut named: Vec<String> = r.iter().map(|l| l["run"].as_str().unwrap().to_string()).collect();
    named.sort();
    let of_study: Vec<String> = runs_of(dir)
        .into_iter()
        .filter(|(_, v)| v["study"] == study)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(named, of_study, "one line per run");
    for (i, l) in d.iter().enumerate() {
        check_record_line(dir, study, i, l, who, pid);
        for s in r.iter().filter(|s| s["variant"] == l["variant"]) {
            assert!(pos(lines, s) < pos(lines, l), "record {i} before its run: {lines:#?}");
        }
    }
}

const RUN: &str = "design_run_study";

#[test]
fn j1_j2_j3_j6_the_lines_of_a_cold_a_chained_a_capped_and_a_failing_study() {
    let (dir, _runner) = new_project("j1");
    let mut m = start(&dir);
    let pid = m.pid();
    // J1: create
    ok(&m.call("design_create_study", nu3("designs/nu3")));
    let c = new_lines(&dir, 0);
    assert_eq!(c.len(), 1, "{c:#?}");
    check_head(
        &c[0],
        "design_created",
        "design_create_study",
        pid,
        &["path", "hash", "study"],
    );
    assert_eq!(c[0]["path"], "designs/nu3/study.json");
    assert_eq!(c[0]["hash"], hash_of(&dir.join("designs/nu3/study.json")));
    assert_eq!(c[0]["study"], "designs/nu3");
    // J1: run
    let v = ok(&m.call(RUN, json!({"dir": "designs/nu3"})));
    assert_eq!(
        (v["evaluations_run"].clone(), v["runs_total"].clone()),
        (json!(3), json!(3)),
        "{v}"
    );
    let l = new_lines(&dir, 1);
    check_study(&dir, "designs/nu3", &l, 3, 3, RUN, pid);
    let mut variants: Vec<&str> = l[..3].iter().map(|x| x["variant"].as_str().unwrap()).collect();
    variants.sort_unstable();
    assert_eq!(variants, ["nu=0.01", "nu=0.015", "nu=0.02"]);
    for x in &l[..3] {
        assert_eq!(x["condition"], "base", "{x}");
    }
    eprintln!("J1: {:?}; {}; {}", kinds(&l), l[0], l[5]);
    // a second call stores nothing
    ok(&m.call(RUN, json!({"dir": "designs/nu3"})));
    assert_eq!(journal(&dir).len(), 7);

    // J2: chained, two conditions
    ok(&m.call("design_create_study", two("designs/two")));
    let v = ok(&m.call(RUN, json!({"dir": "designs/two"})));
    assert_eq!(v["runs_total"], 4, "{v}");
    let l = new_lines(&dir, 8);
    check_study(&dir, "designs/two", &l, 4, 2, RUN, pid);
    let r = of_kind(&l, "run_submitted");
    let base = |variant: &Value| {
        r.iter()
            .find(|x| x["condition"] == "base" && &x["variant"] == variant)
            .unwrap()
    };
    for x in &r {
        match x["condition"].as_str().unwrap() {
            "base" => assert!(x.get("continue_from").is_none(), "{x}"),
            "slow" => {
                assert_eq!(x["continue_from"], base(&x["variant"])["run"], "{x}");
                assert!(pos(&l, base(&x["variant"])) < pos(&l, x), "{l:#?}");
            }
            other => panic!("{other}"),
        }
    }
    eprintln!("J2: {:?}; {}", kinds(&l), r[3]);

    // J3: a step cap that ends every run before it converges
    ok(&m.call("design_create_study", cap("designs/cap")));
    ok(&m.call(RUN, json!({"dir": "designs/cap"})));
    let l = new_lines(&dir, 15);
    check_study(&dir, "designs/cap", &l, 2, 2, RUN, pid);
    for x in of_kind(&l, "run_submitted") {
        assert_eq!(x["max_steps"], 5, "{x}");
    }
    for (x, (rec, _)) in of_kind(&l, "design_recorded").iter().zip(store(&dir, "designs/cap").1) {
        assert_eq!(x["ok"], false, "{x}");
        assert_eq!(x["conditions"][0]["status"], rec["conditions"][0]["status"], "{x}");
        assert_eq!(x["conditions"][0]["status"], "not_converged", "{x}");
    }
    eprintln!("J3: {:?}; {}", kinds(&l), l[3]);

    // J6: a candidate refused before a run
    let mut bad = study(
        "designs/bad",
        json!({"method": {"type": "factorial", "levels": [2]}, "workers": 1}),
    );
    bad["parameters"] = json!([{"name": "n", "lo": 16, "hi": 16.5, "init": 16, "path": "mesh.nx"}]);
    ok(&m.call("design_create_study", bad));
    let v = ok(&m.call(RUN, json!({"dir": "designs/bad"})));
    assert_eq!(
        (v["evaluations_run"].clone(), v["runs_total"].clone()),
        (json!(2), json!(1)),
        "{v}"
    );
    let l = new_lines(&dir, 20);
    check_study(&dir, "designs/bad", &l, 1, 2, RUN, pid);
    assert_eq!(kinds(&l), ["run_submitted", "design_recorded", "design_recorded"]);
    assert_eq!(l[0]["variant"], "n=16.0", "{}", l[0]);
    assert_eq!((l[1]["ok"].clone(), l[2]["ok"].clone()), (json!(true), json!(false)));
    assert_eq!(l[2]["conditions"], json!([{"condition": "base", "status": "failed"}]));
    eprintln!("J6: {:?}; {}", kinds(&l), l[2]);
    assert_eq!(journal(&dir).len(), 23);
}

#[test]
fn j4_the_lines_of_an_operating_map() {
    let (dir, _runner) = new_project("j4");
    let mut m = start(&dir);
    let pid = m.pid();
    let who = "design_run_operating_map";
    let v = ok(&m.call(who, map3("designs/map")));
    assert_eq!(
        (v["state"].clone(), v["runs_total"].clone()),
        (json!("complete"), json!(3)),
        "{v}"
    );
    let l = new_lines(&dir, 0);
    eprintln!("J4: {:?}; {}", kinds(&l), l.last().unwrap());
    assert_eq!(kinds(&l), ["run_submitted", "design_recorded"].repeat(3));
    let table = read_json(&dir.join("designs/map/map.json"));
    for i in 0..3 {
        let (r, d) = (&l[2 * i], &l[2 * i + 1]);
        check_run_line(&dir, r, who, pid);
        assert!(r.get("study").is_none() && r.get("variant").is_none(), "{r}");
        match i {
            0 => assert!(r.get("continue_from").is_none(), "{r}"),
            _ => assert_eq!(r["continue_from"], l[2 * i - 2]["run"], "{r}"),
        }
        check_head(
            d,
            "design_recorded",
            who,
            pid,
            &["path", "hash", "files", "point", "label", "ok", "status"],
        );
        assert_eq!(d["path"], "designs/map/map.json");
        let row = &table["points"][i];
        assert_eq!(d["point"], i, "{d}");
        for k in ["point", "label", "status"] {
            assert_eq!(d[k], row[k], "{d}");
        }
        assert_eq!(d["ok"], true, "{d}");
        assert_eq!(d["files"].as_array().unwrap().len(), if i == 2 { 2 } else { 1 }, "{d}");
    }
    // the last line has the files as they stand
    let file = |f: &str| json!({"path": format!("designs/map/{f}"), "hash": hash_of(&dir.join("designs/map").join(f))});
    assert_eq!(l[5]["hash"], hash_of(&dir.join("designs/map/map.json")));
    assert_eq!(l[5]["files"], json!([file("map.csv"), file("map.meta.json")]));
    // the same call again returns the stored table
    let again = ok(&m.call(who, map3("designs/map")));
    assert_eq!(again["cached"], true, "{again}");
    assert_eq!(journal(&dir).len(), 6);
}

#[test]
fn j5_an_adopted_run_has_no_second_line() {
    let (dir, _runner) = new_project("j5");
    let mut s1 = Server::start(&dir);
    let first = s1.child.id();
    s1.call("design_create_study", nu3("designs/nu3"));
    s1.send_call(RUN, json!({"dir": "designs/nu3"}));
    let t0 = Instant::now();
    while of_kind(&journal_torn(&dir), "run_submitted").len() < 3 {
        assert!(t0.elapsed() < Duration::from_secs(120), "no three lines after 120 s");
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(s1); // killed
    let at_kill = journal(&dir);
    assert_eq!(
        of_kind(&at_kill, "design_recorded").len(),
        0,
        "set-up: the study had ended at the kill: {at_kill:#?}"
    );
    let mut s2 = Server::start(&dir);
    let second = s2.child.id();
    let v = s2.call(RUN, json!({"dir": "designs/nu3"}));
    assert_eq!(
        (v["runs_total"].clone(), v["runs_adopted"].clone()),
        (json!(0), json!(3)),
        "{v}"
    );
    let l = new_lines(&dir, 0);
    let (r, d) = (of_kind(&l, "run_submitted"), of_kind(&l, "design_recorded"));
    eprintln!("J5: {} lines at the kill; then {:?}", at_kill.len(), kinds(&l));
    assert_eq!((r.len(), d.len(), l.len()), (3, 3, 7), "{l:#?}");
    for x in &r {
        check_run_line(&dir, x, RUN, first);
    }
    for (i, x) in d.iter().enumerate() {
        check_record_line(&dir, "designs/nu3", i, x, RUN, second);
    }
    assert_eq!(runs_of(&dir).len(), 3);
}

/// The complete lines of a journal that another process is writing.
fn journal_torn(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[test]
fn j8_the_records_of_a_batch_come_before_the_runs_of_a_later_one() {
    let (dir, _runner) = new_project("j8");
    let mut m = start(&dir);
    let pid = m.pid();
    ok(&m.call(
        "design_create_study",
        study(
            "designs/nm",
            json!({"method": {"type": "nelder_mead", "budget": 4}, "workers": 1}),
        ),
    ));
    let v = ok(&m.call(RUN, json!({"dir": "designs/nm"})));
    let n = store(&dir, "designs/nm").1.len();
    eprintln!("J8: {} records; state {}", n, v["state"]);
    let l = new_lines(&dir, 1);
    eprintln!("J8: {:?}", kinds(&l));
    check_study(&dir, "designs/nm", &l, n, n, RUN, pid);
    let first = pos(&l, &of_kind(&l, "design_recorded")[0]);
    let last = pos(&l, of_kind(&l, "run_submitted").last().unwrap());
    assert!(
        first < last,
        "the record of the first batch is after the last run: {:?}",
        kinds(&l)
    );
}

#[test]
fn j11_the_lines_of_the_other_design_tools() {
    let (dir, _runner) = new_project("j11");
    let mut m = start(&dir);
    let pid = m.pid();
    ok(&m.call("design_create_study", nu3("designs/nu3")));
    ok(&m.call(RUN, json!({"dir": "designs/nu3"})));
    let mut at = journal(&dir).len();
    assert_eq!(at, 7);
    let rec = json!({"dir": "designs/nu3", "params": {"nu": 0.0125}, "metrics": {"lid_drag": 0.25}});
    ok(&m.call("design_record", rec.clone()));
    let l = new_lines(&dir, at);
    assert_eq!(l.len(), 1, "{l:#?}");
    check_record_line(&dir, "designs/nu3", 3, &l[0], "design_record", pid);
    assert_eq!(l[0]["source"], "recorded");
    eprintln!("J11: {}", l[0]);
    at += 1;
    assert_eq!(m.call("design_record", rec)["isError"], true);
    let cached = ok(&m.call(
        "design_evaluate",
        json!({"dir": "designs/nu3", "params": {"nu": 0.015}}),
    ));
    assert_eq!(cached["cached"], true, "{cached}");
    assert_eq!(
        journal(&dir).len(),
        at,
        "a refused record and a cached evaluation write no line"
    );
    ok(&m.call(
        "design_evaluate",
        json!({"dir": "designs/nu3", "params": {"nu": 0.0175}}),
    ));
    let l = new_lines(&dir, at);
    assert_eq!(kinds(&l), ["run_submitted", "design_recorded"]);
    check_run_line(&dir, &l[0], "design_evaluate", pid);
    check_record_line(&dir, "designs/nu3", 4, &l[1], "design_evaluate", pid);
    at += 2;
    assert_eq!(m.call("design_create_study", nu3("designs/nu3"))["isError"], true);
    assert_eq!(journal(&dir).len(), at, "a refused study writes no line");
    ok(&m.call(
        "design_create_variant",
        json!({"path": "designs/v.toml", "dir": "designs/nu3", "params": {"nu": 0.015}}),
    ));
    let l = new_lines(&dir, at);
    assert_eq!(l.len(), 1, "{l:#?}");
    check_head(&l[0], "design_created", "design_create_variant", pid, &["path", "hash"]);
    assert_eq!(l[0]["path"], "designs/v.toml");
    assert_eq!(l[0]["hash"], hash_of(&dir.join("designs/v.toml")));
    at += 1;
    ok(&m.call("design_get_metrics", json!({"dir": "designs/nu3", "index": 0})));
    ok(&m.call("design_list_study", json!({"dir": "designs/nu3"})));
    assert_eq!(journal(&dir).len(), at, "a read writes no line");
}

/// Poll `feed` three times, 600 ms apart (beyond the wait of an `external` path).
fn settle(feed: &mut Feed, into: &mut Vec<Change>) {
    for k in 0..3 {
        if k > 0 {
            std::thread::sleep(Duration::from_millis(600));
        }
        into.extend(feed.poll().unwrap().changes);
    }
}

#[test]
fn f1_f2_the_feed_shows_each_line_once_and_only_outside_writes_as_external() {
    let (dir, _runner) = new_project("f1");
    let mut m = start(&dir);
    let pid = m.pid();
    let mut feed = Feed::open(&dir.canonicalize().unwrap()).unwrap();
    let mut seen: Vec<Change> = Vec::new();
    for (tool, args) in [
        ("design_create_study", nu3("designs/nu3")),
        (RUN, json!({"dir": "designs/nu3"})),
        ("design_run_operating_map", map3("designs/map")),
        (
            "design_record",
            json!({"dir": "designs/nu3", "params": {"nu": 0.0125}, "metrics": {"lid_drag": 0.25}}),
        ),
        (
            "design_create_variant",
            json!({"path": "designs/v.toml", "dir": "designs/nu3", "params": {"nu": 0.015}}),
        ),
    ] {
        ok(&m.call(tool, args));
        settle(&mut feed, &mut seen);
        let ext: Vec<&Change> = seen.iter().filter(|c| c.source == "external").collect();
        assert!(ext.is_empty(), "{tool}: {ext:#?}");
    }
    let lines = new_lines(&dir, 0);
    let shown: Vec<&Change> = seen.iter().filter(|c| c.source == "journal").collect();
    assert_eq!(
        shown.iter().map(|c| c.seq).collect::<Vec<_>>(),
        lines.iter().map(|l| l["seq"].as_u64()).collect::<Vec<_>>(),
        "each line once, in order"
    );
    for (c, l) in shown.iter().zip(&lines) {
        assert_eq!(Some(c.kind.as_str()), l["kind"].as_str());
        assert_eq!(c.who.as_deref(), l["who"].as_str());
        assert_eq!(
            c.actor().map(|a| (a.client, a.pid)),
            Some(("mcp".to_string(), pid)),
            "{c:?}"
        );
    }
    eprintln!("F1: {} lines shown once, 0 external", shown.len());
    assert_eq!(lines.len(), 15);
    let texts: Vec<&str> = shown.iter().map(|c| c.text.as_str()).collect();
    let by = format!("by mcp (pid {pid}): ");
    let run = lines[1]["run"].as_str().unwrap();
    let variant = lines[1]["variant"].as_str().unwrap();
    for want in [
        format!("design_create_study {by}wrote designs/nu3/study.json"),
        format!("design_run_study {by}run {run} of cases/small.toml queued (designs/nu3 {variant} base)"),
        format!("design_run_study {by}recorded designs/nu3 record 0 (nu=0.01): ok"),
        format!(
            "design_run_operating_map {by}run {} of cases/small.toml queued",
            lines[7]["run"].as_str().unwrap()
        ),
        format!(
            "design_run_operating_map {by}recorded designs/map/map.json point 0 ({}): ok",
            lines[8]["label"].as_str().unwrap()
        ),
        format!("design_record {by}recorded designs/nu3 record 3 (nu=0.0125): ok"),
        format!("design_create_variant {by}wrote designs/v.toml"),
    ] {
        assert!(texts.contains(&want.as_str()), "{want}\n{texts:#?}");
    }
    // another server replays the same lines
    let mut other = start(&dir);
    let r = ok(&other.call("read_feed", json!({"from_seq": 1, "limit": 200})));
    let replayed: Vec<&str> = r["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["text"].as_str().unwrap())
        .collect();
    assert_eq!(replayed, texts);

    // F2: writes made outside the workspace
    let touch = |f: &str| {
        let mut h = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(f))
            .unwrap();
        h.write_all(b" ").unwrap();
    };
    let outside = [
        "designs/new.txt",
        "designs/nu3/evaluations.jsonl",
        "designs/nu3/study.json",
    ];
    outside.iter().for_each(|f| touch(f));
    let mut ext: Vec<Change> = Vec::new();
    let mut polls = 0;
    while polls < 4 {
        std::thread::sleep(Duration::from_millis(250));
        polls += 1;
        ext.extend(feed.poll().unwrap().changes);
        if ext.len() >= 3 {
            break;
        }
    }
    eprintln!("F2: {} entries after {polls} polls of 4", ext.len());
    let mut paths: Vec<&str> = ext.iter().map(|c| c.path.as_deref().unwrap()).collect();
    paths.sort_unstable();
    assert_eq!(paths, outside, "{ext:#?}");
    for c in &ext {
        assert_eq!(c.source, "external", "{c:?}");
        assert!(
            c.actor.is_none() && c.who.is_none() && c.changes.is_none() && c.seq.is_none(),
            "{c:?}"
        );
    }
}

#[test]
fn s_two_servers_in_one_project_number_every_line_once() {
    let (dir, _runner) = new_project("s");
    let mut a = Server::start(&dir);
    let mut b = Server::start(&dir);
    a.send_call("design_create_study", nu3("designs/a"));
    b.send_call("design_create_study", nu3("designs/b"));
    for s in [&mut a, &mut b] {
        ok(&s.receive()["result"]);
    }
    a.send_call(RUN, json!({"dir": "designs/a"}));
    b.send_call(RUN, json!({"dir": "designs/b"}));
    for s in [&mut a, &mut b] {
        let v = ok(&s.receive()["result"]);
        assert_eq!(v["evaluations_run"], 3, "{v}");
    }
    let l = journal(&dir);
    let seqs: Vec<u64> = l.iter().filter_map(|x| x["seq"].as_u64()).collect();
    eprintln!(
        "S: {} lines, {} without seq; {:?}",
        l.len(),
        l.len() - seqs.len(),
        kinds(&l)
    );
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
    assert_eq!(l.len(), 14, "{l:#?}");
    for (s, study) in [(&a, "designs/a"), (&b, "designs/b")] {
        let own: Vec<Value> = l
            .iter()
            .filter(|x| x["actor"]["instance"]["pid"] == s.child.id())
            .cloned()
            .collect();
        assert_eq!(of_kind(&own, "design_created").len(), 1, "{own:#?}");
        check_study(&dir, study, &own[1..], 3, 3, RUN, s.child.id());
    }
}

/// Every file under `dir` with its hash.
fn tree(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.clone(), hash_of(&p)));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn n_without_a_project_nothing_is_written_to_the_journal() {
    let (dir, _runner) = new_project("n");
    let before = tree(&dir.join(".rcfd"));
    let mut m = Mcp::start(&["--root", &dir.to_string_lossy()]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    for (tool, args) in [
        ("design_create_study", nu3("designs/nu3")),
        (RUN, json!({"dir": "designs/nu3"})),
        (
            "design_evaluate",
            json!({"dir": "designs/nu3", "params": {"nu": 0.0175}}),
        ),
        (
            "design_record",
            json!({"dir": "designs/nu3", "params": {"nu": 0.0125}, "metrics": {"lid_drag": 0.25}}),
        ),
        ("design_run_operating_map", map3("designs/map")),
        (
            "design_create_variant",
            json!({"path": "designs/v.toml", "dir": "designs/nu3", "params": {"nu": 0.015}}),
        ),
    ] {
        let v = ok(&m.call(tool, args));
        assert!(v.get("evaluator").is_none() && v.get("runs").is_none(), "{tool}: {v}");
    }
    assert_eq!(tree(&dir.join(".rcfd")), before);
    assert!(runs_of(&dir).is_empty());
    assert_eq!(store(&dir, "designs/nu3").1.len(), 5, "the calls did store");
}
