//! ADR 0826 (F-978): the process view of a case through the `rcfd-mcp` binary. B1: a case built by a client from new
//! to a finished run (`new_case`, a mesh edit, a set-up edit, `start_run`, `wait_run`, a set-up edit after the run),
//! shown as a process panel to an `rcfd-mcp --project --canvas` process (the Studio's reading server), whose
//! `read_view` gives the steps (`process`, D2): the journal's steps are the journal lines of the case, in order, none
//! missing; the run's state steps are its state events; the residuals step's last values are `read_residuals`' last
//! points; a results step; times never decrease; every step's view is accepted by `show_view`. B2: `export_view` of the
//! process view is refused at `panels[0].kind` and writes nothing. B3: `show_view`'s description names the kind, and a
//! view without a process panel gets no `process`.

mod common;

use common::Mcp;
use rcfd_workspace::Workspace;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const CASE: &str = "cases/process-cavity.toml";

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

fn quick_start(tag: &str) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = common::scratch(&format!("f978-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    Workspace::open(&dir).unwrap();
    dir
}

fn d(dir: &Path) -> String {
    dir.display().to_string()
}

fn ok(r: &Value) -> Value {
    assert_eq!(r["isError"], false, "{r}");
    r["structuredContent"].clone()
}

fn journal(dir: &Path) -> Vec<u8> {
    std::fs::read(dir.join(".rcfd/edits.jsonl")).unwrap_or_default()
}

/// The time of a step as this test reads it: (the text before the fraction, the fraction as nanoseconds).
fn t_key(t: &str) -> (String, u64) {
    let body = t.trim_end_matches('Z');
    let (s, f) = body.split_once('.').unwrap_or((body, ""));
    (s.to_string(), format!("{f:0<9}").parse::<u64>().unwrap())
}

/// ADR 0828: ends the project's runner, which `start_run` started detached and which has no idle exit, when the test
/// ends, whether it passed or panicked: `rcfd-runner`'s shutdown, as the guards of `feed_f273.rs` do.
struct EndRunner(PathBuf);

impl Drop for EndRunner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(std::time::Duration::from_secs(10));
    }
}

fn edit(c: &mut Mcp, set: Value, reason: &str) {
    let hash = ok(&c.call("read_project_file", json!({"path": CASE})))["hash"].clone();
    let r = ok(&c.call(
        "edit_case",
        json!({"path": CASE, "base_hash": hash, "set": set, "reason": reason}),
    ));
    assert_eq!(r["written"], true, "{r}");
}

/// ADR 0826 B1, B2, B3.
#[test]
fn b1_b2_b3_the_process_of_a_case_built_from_new_to_a_finished_run() {
    common::binaries();
    let dir = quick_start("b1");
    let runner = EndRunner(dir.clone());
    let mut studio = Mcp::start(&["--project", &d(&dir), "--client", "studio", "--read-only", "--canvas"]);
    let tools = studio.request("tools/list", json!({}));
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    // the registered build
    ok(&c.call(
        "new_case",
        json!({"name": "process-cavity", "copy": "cases/cavity-re100.toml", "reason": "F-978 B1: new"}),
    ));
    edit(&mut c, json!({"mesh.nx": 16, "mesh.ny": 16}), "F-978 B1: mesh");
    edit(&mut c, json!({"physics.nu": 0.02}), "F-978 B1: set-up");
    let run = ok(&c.call(
        "start_run",
        json!({"case": CASE, "max_steps": 40, "reason": "F-978 B1: run"}),
    ))["run"]
        .as_str()
        .unwrap()
        .to_string();
    let mut end = Value::Null;
    for _ in 0..10 {
        end = ok(&c.call("wait_run", json!({"run": run, "timeout_s": 60})));
        if end["terminal"] == true {
            break;
        }
    }
    eprintln!("B1: run {run} ended {}", end["state"]);
    edit(&mut c, json!({"solver.max_iterations": 500}), "F-978 B1: after the run");
    let view = json!({"version": 1, "panels": [{"kind": "process", "case": CASE}]});
    let shown = ok(&c.call("show_view", json!({"view": view, "reason": "F-978 B1"})));
    assert_eq!(shown["studios"], 1, "{shown}");
    let read = ok(&studio.call("read_view", json!({})));
    let p = &read["process"][0];
    let steps = p["steps"].as_array().cloned().unwrap_or_default();
    for s in &steps {
        eprintln!("  {} {} {} {} | {}", s["i"], s["step"], s["seq"], s["t"], s["text"]);
    }
    // the journal, read here: the lines of the case by rules (a) to (c) of D2
    let lines: Vec<Value> = String::from_utf8_lossy(&journal(&dir))
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let runs = [run.clone()];
    let want: Vec<Value> = lines
        .iter()
        .filter(|l| {
            l["path"] == CASE
                || (l["kind"] == "run_submitted" && l["case"] == CASE)
                || l["run"].as_str().is_some_and(|r| runs.iter().any(|x| x == r))
        })
        .map(|l| l["seq"].clone())
        .collect();
    let got: Vec<Value> = steps
        .iter()
        .filter(|s| s["source"] == "journal")
        .map(|s| s["seq"].clone())
        .collect();
    let kinds: Vec<&str> = steps.iter().map(|s| s["step"].as_str().unwrap_or("")).collect();
    // the run's state events, read here
    let events = std::fs::read_to_string(dir.join("runs").join(&run).join("events.jsonl")).unwrap();
    let want_states: Vec<(Value, Value)> = events
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["kind"] == "state")
        .map(|v| (v["from"].clone(), v["to"].clone()))
        .collect();
    let got_states: Vec<(Value, Value)> = steps
        .iter()
        .filter(|s| s["step"] == "state")
        .map(|s| (s["from"].clone(), s["to"].clone()))
        .collect();
    // the residuals step against read_residuals' last points
    let res = ok(&c.call("read_residuals", json!({"run": run})));
    let series = res["series"].as_object().cloned().unwrap_or_default();
    let resid = steps
        .iter()
        .find(|s| s["step"] == "residuals")
        .cloned()
        .unwrap_or(Value::Null);
    let mut compared = 0;
    let mut equal = 0;
    for (k, s) in &series {
        if !(k.starts_with("residuals.") || k.starts_with("monitors.")) {
            continue;
        }
        compared += 1;
        let x = s["x"].as_array().and_then(|a| a.last()).cloned();
        let y = s["y"].as_array().and_then(|a| a.last()).cloned();
        let mine = &resid["last"][k];
        if x.is_some() && mine[0].as_f64() == x.unwrap().as_f64() && mine[1] == y.unwrap() {
            equal += 1;
        }
    }
    let keys: Vec<(String, u64)> = steps.iter().map(|s| t_key(s["t"].as_str().unwrap_or(""))).collect();
    let ordered = keys.windows(2).all(|w| w[0] <= w[1]);
    let results = steps.iter().any(|s| s["step"] == "results" && s["run"] == json!(run));
    let last_setup = steps
        .last()
        .is_some_and(|s| s["step"] == "setup" && s["reason"] == "F-978 B1: after the run");
    // every step's view, shown by the client
    let mut opened = 0;
    for s in &steps {
        if s["opens"].is_null() {
            eprintln!("B1: step {} ({}) opens nothing", s["i"], s["step"]);
            continue;
        }
        let r = c.call("show_view", json!({"view": s["opens"]}));
        if r["isError"] == false {
            opened += 1;
        } else {
            eprintln!("B1: step {} view refused: {r}", s["i"]);
        }
    }
    let terminal = got_states.last().is_some_and(|(_, to)| to == &end["state"]);
    eprintln!(
        "B1: {} steps; journal seq {got:?} against {want:?}; kinds {kinds:?}; states {} against {}, terminal {terminal}; residuals {equal} of {compared} equal; results {results}; ordered {ordered}; after-run set-up last {last_setup}; views shown {opened} of {}",
        steps.len(),
        got_states.len(),
        want_states.len(),
        steps.len()
    );
    // B2
    let before = journal(&dir);
    let r = c.call("export_view", json!({"view": view}));
    let refused = r["isError"] == true && r["structuredContent"]["errors"][0]["key"] == "panels[0].kind"
        || r.to_string().contains("panels[0].kind");
    let nothing = !dir.join("exports").exists() && journal(&dir) == before;
    eprintln!(
        "B2: export_view {}; nothing written {nothing}",
        r.to_string().chars().take(300).collect::<String>()
    );
    // B3
    let desc = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "show_view")
        .map(|t| t["description"].as_str().unwrap_or("").to_string())
        .unwrap_or_default();
    ok(&c.call(
        "show_view",
        json!({"view": {"version": 1, "panels": [{"kind": "case", "case": CASE}]}}),
    ));
    let plain = ok(&studio.call("read_view", json!({})));
    let b3 = desc.contains("process {case}") && plain.get("process").is_none();
    eprintln!(
        "B3: description names process {}; plain read_view keys {:?}",
        desc.contains("process {case}"),
        plain.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())
    );
    drop(c);
    drop(studio);
    drop(runner);
    assert!(!want.is_empty(), "B1: the journal has lines of the case");
    assert_eq!(got, want, "B1: the journal's steps, in order, none missing");
    assert_eq!(kinds.first(), Some(&"created"), "B1: the first step");
    for k in ["mesh", "setup", "run", "state", "residuals", "results"] {
        assert!(kinds.contains(&k), "B1: a step {k}");
    }
    assert_eq!(got_states, want_states, "B1: the run's state steps");
    assert!(terminal, "B1: the run's last state step is its end");
    assert!(
        compared > 0 && equal == compared,
        "B1: the residuals step's last values"
    );
    assert!(results, "B1: a results step");
    assert!(ordered, "B1: times never decrease");
    assert!(last_setup, "B1: the set-up after the run is last");
    assert_eq!(opened, steps.len(), "B1: every step's view is shown");
    assert!(
        refused && nothing,
        "B2: export_view refused at panels[0].kind, nothing written"
    );
    assert!(b3, "B3");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// ADR 0836 (F-978): B1, B2 and B3 of ADR 0826, unchanged, under the load of ADR 0832's arm H: 16 threads spinning in
/// this test process from 500 ms before the build to its end. Ignored so that the file's default run is unchanged; run
/// three times as ADR 0836 registers.
#[test]
#[ignore = "ADR 0836: B1 to B3 under arm H's load, run as registered"]
fn b1_b2_b3_under_the_load_of_arm_h_adr0836() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let stop = Arc::new(AtomicBool::new(false));
    let spin: Vec<_> = (0..16)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut x = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
            })
        })
        .collect();
    eprintln!("ADR 0836: {} spinning threads", spin.len());
    std::thread::sleep(std::time::Duration::from_millis(500));
    let r = std::panic::catch_unwind(b1_b2_b3_the_process_of_a_case_built_from_new_to_a_finished_run);
    stop.store(true, Ordering::Relaxed);
    for t in spin {
        t.join().unwrap();
    }
    if let Err(e) = r {
        std::panic::resume_unwind(e);
    }
}
