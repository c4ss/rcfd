//! ADR 0840 (F-990): a project run of a solid case file through the `rcfd-mcp` binary, with `rcfd`, `rcfd-runner` and
//! `rcfd-solid-run` beside it. S1: `start_run` of `cases/cantilever-hex8.toml` (a byte copy of
//! `crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml`) in a copy of quick-start makes a finished run with its
//! records, report and stored fields; S2: `show_view` of that run with no layout takes the structural layout; S3: the
//! negative controls of D4 (entries, a dry run, an invalid copy, amendment 1) are refused and make no run.

mod common;

use common::Mcp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const CASE: &str = "cases/cantilever-hex8.toml";
const FIELDS: [&str; 11] = [
    "ux",
    "uy",
    "uz",
    "u_magnitude",
    "von_mises",
    "sxx",
    "syy",
    "szz",
    "sxy",
    "syz",
    "szx",
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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

fn cantilever() -> PathBuf {
    repo().join("crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml")
}

fn bare(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("outputs");
    v
}

fn runs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("runs"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Ends the project's runner (it outlives the server otherwise; ADR 0828).
struct RunnerGuard(PathBuf);
impl Drop for RunnerGuard {
    fn drop(&mut self) {
        let runner = common::binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = std::process::Command::new(runner)
            .args(["shutdown", "--project", &self.0.display().to_string()])
            .output();
    }
}

/// ADR 0840 S1, S2, S3.
#[test]
fn s1_s2_s3_a_solid_case_file_runs_in_a_project_and_shows_the_structural_layout() {
    let mcp = common::binaries();
    let driver = mcp.with_file_name(format!("rcfd-solid-run{}", std::env::consts::EXE_SUFFIX));
    assert!(
        driver.is_file(),
        "{} missing (cargo build -p rcfd-workspace --bins)",
        driver.display()
    );
    let dir = common::scratch("f990-s").join("quick-start");
    copy_dir(&repo().join("example-projects/quick-start"), &dir);
    std::fs::copy(cantilever(), dir.join(CASE)).unwrap();
    let bad = std::fs::read_to_string(cantilever())
        .unwrap()
        .replace("youngs_modulus = 2.0e11", "");
    std::fs::write(dir.join("cases/cantilever-bad.toml"), bad).unwrap();
    let _guard = RunnerGuard(dir.clone());
    let d = dir.display().to_string();
    // the Studio's reading server (as canvas_layout_f977): a canvas is open
    let mut studio = Mcp::start(&["--project", &d, "--client", "studio", "--read-only", "--canvas"]);
    studio.request("tools/list", json!({}));
    let mut c = Mcp::start(&["--project", &d]);
    // S3 first: nothing may be written
    let before = runs(&dir);
    for (args, code) in [
        (json!({"case": CASE, "set": ["a=1"]}), -32602),
        (json!({"case": CASE, "dry_run": true}), -32602),
        (json!({"case": "cases/cantilever-bad.toml"}), 1003),
    ] {
        let r = c.call("start_run", args.clone());
        let e = r["structuredContent"]["error"].clone();
        eprintln!("S3 {args}: {e}");
        assert_eq!(r["isError"], true, "S3 {args}: {r}");
        assert_eq!(e["code"], code, "S3 {args}");
        if code == 1003 {
            assert!(
                e["message"]
                    .as_str()
                    .unwrap()
                    .contains("materials.steel.youngs_modulus"),
                "S3 the key: {e}"
            );
        }
    }
    assert_eq!(runs(&dir), before, "S3 no run folder");
    // S1
    let t0 = std::time::Instant::now();
    let r = c.call("start_run", json!({"case": CASE, "reason": "F-990 S1"}));
    assert_eq!(r["isError"], false, "S1 start_run: {r}");
    let id = r["structuredContent"]["run"].as_str().unwrap().to_string();
    let mut state = String::new();
    while t0.elapsed().as_secs_f64() < 60.0 {
        let w = c.call("wait_run", json!({"run": id, "timeout_s": 10}));
        state = w["structuredContent"]["state"].as_str().unwrap_or("").to_string();
        if w["structuredContent"]["terminal"] == true {
            break;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let run_dir = dir.join("runs").join(&id);
    eprintln!(
        "S1: {id} {state} after {secs:.2} s; status {}",
        std::fs::read_to_string(run_dir.join("status.json")).unwrap_or_default()
    );
    assert_eq!(state, "finished", "S1 state");
    assert!(secs <= 60.0, "S1 time");
    let record: Value = serde_json::from_slice(&std::fs::read(run_dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(record["case"], CASE, "S1 run.json case");
    assert_eq!(
        std::fs::read(run_dir.join("case.snapshot")).unwrap(),
        std::fs::read(cantilever()).unwrap(),
        "S1 case.snapshot"
    );
    let own_dir = common::scratch("f990-own");
    let own_case = own_dir.join("own.toml");
    std::fs::copy(cantilever(), &own_case).unwrap();
    let own = rcfd_api::solid_case::run_solid_case_file(&own_case, Some(&own_dir)).unwrap();
    let want = bare(own.report.clone());
    let report: Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir.join("output/cantilever_hex8.json")).unwrap()).unwrap();
    assert_eq!(bare(report), want, "S1 the report equals the case's own entry's");
    assert!(run_dir.join("output/cantilever_hex8.vtu").is_file(), "S1 vtu");
    let mut store = rcfd_tsstore::Reader::open(&run_dir.join("fields")).unwrap();
    let names: Vec<String> = store.fields(0).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, FIELDS, "S1 stored fields");
    let metrics: Value = serde_json::from_slice(&std::fs::read(run_dir.join("metrics.json")).unwrap()).unwrap();
    eprintln!("S1 metrics reports: {}", metrics["reports"]);
    assert_eq!(
        metrics["reports"],
        json!({"displacement_max_magnitude": want["displacement"]["max_magnitude"],
               "von_mises_max": want["stress"]["von_mises_max"],
               "element_von_mises_max": want["stress"]["element_von_mises_max"]}),
        "S1 metrics"
    );
    let lines = std::fs::read_to_string(run_dir.join("residuals.jsonl")).unwrap();
    assert_eq!(lines.lines().count(), 1, "S1 one residual line");
    // S2
    let r = c.call(
        "show_view",
        json!({"view": {"version": 1, "run": id}, "reason": "F-990 S2"}),
    );
    assert_eq!(r["isError"], false, "S2: {r}");
    let shown = r["structuredContent"].clone();
    eprintln!("S2 show_view: {shown}");
    assert_eq!(shown["category"], "structural");
    assert_eq!(shown["rule"], "a solid case file (docs/solid-case-file.md)");
    assert_eq!(shown["panels"], 6);
    assert_eq!(shown["studios"], 1, "S2 a canvas shows it");
    let read = studio.call("read_view", json!({}))["structuredContent"].clone();
    let f = |n: &str| json!({"kind": "field", "title": n, "run": id, "field": n});
    let m = |n: &str| json!({"kind": "monitor", "title": n, "run": id, "monitor": n});
    assert_eq!(
        read["view"]["panels"],
        json!([
            {"kind": "case", "title": "Case", "case": CASE},
            f("u_magnitude"),
            f("von_mises"),
            m("displacement_max_magnitude"),
            m("von_mises_max"),
            {"kind": "table", "title": "Run", "runs": [id]}
        ]),
        "S2 panels"
    );
}
