//! F-834: runs started through the workspace's `start_run` with the start plan and the real runner (`rcfd-runner serve`
//! started directly, ADR 0128 rule): ADR 0140 SF4 (the plan's snapshot count against real stores), SF5 (the store is
//! where it is read), SF8's engine part (the plan's entries before the run's own) and ADR 0186 C-S (a run that
//! `start_run` made from a new case is listed, finishes and its store reads). Release builds; `rcfd-runner` and `rcfd`
//! of the same build (`f834::binaries`).

mod f834;

use f834::{call, RunnerProject, RUN_CASE};
use rcfd_project::RunState;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

const RUN_WAIT: Duration = Duration::from_secs(300);

fn piso_case() -> String {
    RUN_CASE.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.0025\nend_time = 0.5",
    )
}

fn sod_case() -> String {
    std::fs::read_to_string(f834::repo().join("cases/sod/sod.toml")).unwrap()
}

/// The last snapshot's `p` of a store, read as the workspace's `field_summary` reads it: (snapshot index, min, max).
fn last_p(dir: &Path) -> (usize, f64, f64) {
    let mut r = rcfd_tsstore::Reader::open(dir).unwrap();
    let n = r.snapshots().len();
    let p = r.read_field(n - 1, "p").unwrap();
    let min = p
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(f64::INFINITY, f64::min);
    let max = p
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    (n - 1, min, max)
}

/// Replaces `crates/rcfd-studio/tests/store_runs_sf4_sf5_sf8.rs::sf4_sf5_estimates_and_the_viewport_location` (engine
/// part) and the engine part of `sf8_rerun_with_fields_stored`: (a) the steady cavity, (b) the piso cavity and (c) sod,
/// each started by `start_run` with `store: plan` (10 MB declared, as the Studio's test did). SF4: the store holds the
/// plan's snapshot count ((c): CFL steps, at least one and at most the count). SF5: the store is in `runs/<id>/fields/`
/// (none under `output/`), and `field_summary` reads its last snapshot's `p` bit for bit as the store holds it;
/// negative control: with the store moved away `field_summary` says the run has no stored fields. SF8's engine part:
/// (d) the plan's entries come before the run's own (`solver.max_iterations=400`) in `run.json`, and the run stores
/// 100 snapshots. Not ported: the Studio's own size estimate within 10 % (`rcfd_studio::store_plan::estimate`, Studio
/// code, F-837), the viewport's frame, its empty-state text, and the Re-run with fields stored action (F-837).
#[test]
fn sf4_sf5_sf8_planned_stores_of_real_runs() {
    let mut p = RunnerProject::new("sf4", 4);
    std::fs::write(p.dir.join("cases/piso.toml"), piso_case()).unwrap();
    std::fs::write(p.dir.join("cases/sod.toml"), sod_case()).unwrap();
    p.serve();
    let ws = p.workspace();
    let mut runs: Vec<(&str, String, Value)> = Vec::new();
    for (name, rel, set) in [
        ("(a) steady cavity", "cases/cavity.toml", json!([])),
        ("(b) piso cavity", "cases/piso.toml", json!([])),
        ("(c) sod", "cases/sod.toml", json!([])),
        (
            "(d) the run's own entry",
            "cases/cavity.toml",
            json!(["solver.max_iterations=400"]),
        ),
    ] {
        let v = call(
            &ws,
            "start_run",
            json!({"case": rel, "store": "plan", "mem_mb": 10, "set": set}),
        );
        eprintln!("SF4 {name}: {v}");
        assert_eq!(v["store"]["applied"], true, "{name}");
        runs.push((name, v["run"].as_str().unwrap().to_string(), v["store"].clone()));
    }
    for (name, id, _) in &runs {
        let st = p.wait_end(id, RUN_WAIT);
        assert_eq!(st.state, RunState::Finished, "{name}: {st:?}");
    }
    let mut counts = Vec::new();
    for (name, id, store) in &runs {
        let dir = p.run_dir(id).join("fields");
        let n = f834::store_snapshots(&dir) as u64;
        let planned = store["snapshots"].as_u64().unwrap();
        let set = p.record(id)["settings"]["set"].clone();
        eprintln!("SF4 {name}: {n} snapshots (plan {planned}), set {set}");
        if name.starts_with("(c)") {
            assert!(n >= 1 && n <= planned, "{name}: {n} snapshots");
        } else {
            assert_eq!(n, planned, "{name}");
        }
        counts.push(n);
    }
    // SF8's engine part: the plan's entries, then the run's own
    let (_, d, _) = &runs[3];
    assert_eq!(
        p.record(d)["settings"]["set"],
        json!([
            "output.store_interval=4",
            "output.store_precision=f32",
            "solver.max_iterations=400"
        ])
    );
    assert_eq!(counts[3], 100);
    assert_eq!(counts[0], 100);
    // SF5: where the store is, read as the workspace reads it
    for (name, id, _) in &runs {
        let dir = p.run_dir(id).join("fields");
        let (last, min, max) = last_p(&dir);
        let v = call(&ws, "field_summary", json!({"run": id, "field": "p"}));
        eprintln!("SF5 {name}: {v}");
        assert!(!p.run_dir(id).join("output").join("fields").exists(), "{name}");
        assert_eq!(v["snapshot"]["index"].as_u64(), Some(last as u64), "{name}");
        assert_eq!(v["precision"], "f32", "{name}");
        assert_eq!(v["min"].as_f64().map(f64::to_bits), Some(min.to_bits()), "{name}");
        assert_eq!(v["max"].as_f64().map(f64::to_bits), Some(max.to_bits()), "{name}");
    }
    // SF5 negative control: (a)'s store moved away
    let (_, a, _) = &runs[0];
    std::fs::rename(p.run_dir(a).join("fields"), p.run_dir(a).join("fields.moved")).unwrap();
    match ws.call("field_summary", &json!({"run": a, "field": "p"})) {
        Some(Err(e)) => {
            eprintln!("SF5 control: {} {}", e.code, e.message);
            assert!(
                e.message.starts_with(&format!("run {a} has no stored fields")),
                "{}",
                e.message
            );
        }
        other => panic!("control: {other:?}"),
    }
}

/// Replaces `crates/rcfd-studio/tests/new_simulations_f271.rs::cs_the_studio_lists_and_reads_a_run_that_new_case_made`
/// (ADR 0186 C-S, ADR 0173 C7; engine part): a run that `start_run` makes from a new case is listed by `list_runs` at
/// once with its case, listed finished once it ended, and its stored fields read: snapshots, the last one's `p` finite.
/// The Studio's run poller and viewport reader were Studio code.
#[test]
fn cs_a_run_that_new_case_made_is_listed_and_its_store_reads() {
    let mut p = RunnerProject::new("f271-cs", 2);
    p.serve();
    let ws = p.workspace();
    let first = call(&ws, "list_runs", json!({}));
    assert_eq!(first["total"], 0, "no run yet: {first}");
    let v = call(
        &ws,
        "start_run",
        json!({"new_case": {"name": "sod", "example": "sod/sod.toml"}}),
    );
    let id = v["run"].as_str().unwrap().to_string();
    let rows = call(&ws, "list_runs", json!({}));
    let row = rows["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id.as_str())
        .cloned()
        .unwrap_or_else(|| panic!("{id} not listed at once: {rows}"));
    eprintln!("C-S: {id} listed: {row}");
    assert_eq!(row["case"], "cases/sod.toml");
    let st = p.client.wait(&id, RUN_WAIT).unwrap();
    assert_eq!(st.state, RunState::Finished, "{st:?}");
    let done = call(&ws, "list_runs", json!({"state": "finished"}));
    assert_eq!(done["total"], 1, "{done}");
    let dir = p.run_dir(&id).join("fields");
    let mut r = rcfd_tsstore::Reader::open(&dir).unwrap();
    let n = r.snapshots().len();
    let last = r.snapshots().last().map(|s| s.time).expect("snapshots");
    let f = r.read_field(n - 1, "p").unwrap();
    eprintln!("C-S: {n} snapshots, the last at t = {last}, p over {} cells", f.len());
    assert!(!f.is_empty() && f.iter().all(|x| x.is_finite()));
}
