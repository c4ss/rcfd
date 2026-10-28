//! ADR 0140 SF2: every shipped case, with its start plan's `set()` applied, writes a store that the viewport's reader
//! opens: three steps through `runctl::drive` with the output and store folders the runner passes (in process, no
//! runner). Negative control: a store naming a field that does not exist is refused by the solver.

mod common;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::viewport::StoreView;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn shipped() -> Vec<(String, PathBuf)> {
    let root = rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let mut out = Vec::new();
    let mut stack = vec![root.join("cases")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "toml") {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
    }
    out.sort();
    out
}

/// Three steps of `case` with the runner's folders under `dir`; the status events.
fn three_steps(case: &Case, dir: &Path) -> Vec<Value> {
    let sim = Simulation::new(case).unwrap_or_else(|d| panic!("{d:?}"));
    let opts = RunOptions {
        max_steps: Some(3),
        out_dir: Some(dir.join("output")),
        store_dir: Some(dir.join("fields")),
        status_monitors: true,
        ..Default::default()
    };
    let mut events = Vec::new();
    let o = drive(&Mutex::new(sim), &opts, &|| false, &mut |e| events.push(e.clone()));
    eprintln!("  drive: {} after {} steps", o.state, o.iterations);
    events
}

fn store_warnings(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["event"] == "warning")
        .filter_map(|e| e["message"].as_str().map(String::from))
        .filter(|m| m.contains("field store"))
        .collect()
}

#[test]
fn sf2_every_shipped_case_writes_a_readable_store() {
    let cases = shipped();
    assert_eq!(cases.len(), 10);
    let tmp = common::temp("sf2");
    let mut ok = 0;
    for (i, (rel, path)) in cases.iter().enumerate() {
        let text = std::fs::read_to_string(path).unwrap();
        let plan = StartPlan::new(rel, &text, path.parent().unwrap()).unwrap();
        let mut case = Case::load(path).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
        for s in plan.set() {
            apply_set(&mut case, &s).unwrap_or_else(|e| panic!("{rel}: {s}: {e:?}"));
        }
        let dir = tmp.join(format!("run{i}"));
        eprintln!("SF2 {rel}: set {:?}", plan.set());
        let events = three_steps(&case, &dir);
        let warns = store_warnings(&events);
        assert!(warns.is_empty(), "{rel}: {warns:?}");
        let store = dir.join("fields");
        let v = StoreView::open(&store).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let n = v.snaps().len();
        let mut r = rcfd_tsstore::Reader::open(&store).unwrap();
        let mut names = Vec::new();
        let mut precisions = std::collections::BTreeSet::new();
        for k in 0..n {
            let f = r.fields(k).unwrap();
            names = f.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
            for x in &f {
                precisions.insert(format!("{:?}", x.precision));
            }
        }
        eprintln!(
            "SF2 {rel}: {n} snapshot(s) at steps {:?}, fields {names:?}, precisions {precisions:?}, {} faces drawn",
            v.snaps().iter().map(|s| s.step).collect::<Vec<_>>(),
            v.surface.n_faces()
        );
        assert!(n >= 1, "{rel}: no snapshot");
        assert_eq!(names, ["ux", "uy", "uz", "p"], "{rel}");
        assert_eq!(precisions.into_iter().collect::<Vec<_>>(), ["F32"], "{rel}");
        ok += 1;
    }
    eprintln!("SF2: {ok} of {} shipped cases stored a readable store", cases.len());
    assert_eq!(ok, 10);
    // negative control: a field that does not exist
    let cav = tmp.join("control.toml");
    std::fs::write(&cav, common::RUN_CASE).unwrap();
    let mut case = Case::load(&cav).unwrap();
    for s in [
        "output.store_interval=4",
        "output.store_precision=f32",
        "output.store_fields=[\"nosuch\"]",
    ] {
        apply_set(&mut case, s).unwrap();
    }
    let dir = tmp.join("control");
    let warns = store_warnings(&three_steps(&case, &dir));
    eprintln!(
        "SF2 control: {warns:?}; store file: {}",
        dir.join("fields/store.rts").exists()
    );
    assert!(warns.iter().any(|w| w.contains("field store disabled")));
    assert!(!dir.join("fields").join(rcfd_tsstore::STORE_FILE).exists());
}
