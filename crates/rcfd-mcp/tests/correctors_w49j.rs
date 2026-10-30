//! ADR 0172 (job 49J) item A3, limit LC1: the MCP `read_residuals` returns the adaptive corrector count and cap
//! (`correctors`, `corrector_cap` of ADR 0160 D4) as series; a run without them returns the keys it returned before.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use rcfd_project::Project;
use serde_json::{json, Value};

fn line(step: u64, n: Option<(u64, u64)>) -> String {
    let mut v = json!({"step": step, "time": step as f64 * 0.1,
        "residuals": {"u": 1e-3 / step as f64, "p": 2e-3 / step as f64, "continuity": 1e-4},
        "monitors": {"cl": 0.1 * step as f64}});
    if let Some((c, cap)) = n {
        v["correctors"] = json!(c);
        v["corrector_cap"] = json!(cap);
    }
    format!("{v}\n")
}

fn run_with(dir: &std::path::Path, lines: &str) -> String {
    let p = Project::open(dir).unwrap();
    let rec = p.create_run("cases/cavity.toml", None, "test", json!({})).unwrap();
    std::fs::write(dir.join("runs").join(&rec.id).join("residuals.jsonl"), lines).unwrap();
    rec.id
}

fn series_of(r: &Value) -> Vec<String> {
    r["series"].as_object().unwrap().keys().cloned().collect()
}

#[test]
fn lc1_read_residuals_returns_the_corrector_series() {
    let dir = project("w49j-lc1");
    let s = server(&dir, Policy::default());
    let counts = [2u64, 2, 3, 3, 4, 4, 3, 3];
    let with: String = counts
        .iter()
        .enumerate()
        .map(|(i, &c)| line(i as u64 + 1, Some((c, i as u64 / 3))))
        .collect();
    let plain: String = (1..=8).map(|i| line(i, None)).collect();
    let a = run_with(&dir, &with);
    let b = run_with(&dir, &plain);
    let ra = ok(&call(&s, "read_residuals", json!({"run": a})));
    let rb = ok(&call(&s, "read_residuals", json!({"run": b})));
    eprintln!("LC1 adaptive: {:?}; plain: {:?}", series_of(&ra), series_of(&rb));
    // the adaptive run: both series, the file's steps and values
    let c = &ra["series"]["correctors"];
    assert_eq!(c["x"], json!([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]), "{ra}");
    assert_eq!(c["y"], json!([2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 3.0, 3.0]), "{ra}");
    assert_eq!(
        ra["series"]["corrector_cap"]["y"],
        json!([0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0]),
        "{ra}"
    );
    // the run without them: exactly the keys it had
    assert_eq!(
        series_of(&rb),
        vec!["monitors.cl", "residuals.continuity", "residuals.p", "residuals.u"],
        "{rb}"
    );
    // `names` picks the series, `since_step` cuts them
    let one = ok(&call(
        &s,
        "read_residuals",
        json!({"run": a, "names": ["correctors"], "since_step": 5}),
    ));
    assert_eq!(series_of(&one), vec!["correctors"], "{one}");
    assert_eq!(one["series"]["correctors"]["y"], json!([4.0, 4.0, 3.0, 3.0]), "{one}");
    assert_eq!(one["unknown_names"], json!([]), "{one}");
}
