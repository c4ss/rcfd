//! ADR 0133 R8 (set-up: amendment 1): the lid cavity started through the model's start plan, with the store proposal
//! on (R10 end to end). The plotted `lid_drag` series equals the residual lines at every step and its last value
//! equals `output/monitors.json` and `metrics.json` bit-exactly. Negative control: the series of `res_p` differs.

mod common;

use common::{studio_on, RunnerProject};
use rcfd_project::RunState;
use serde_json::Value;
use std::time::Duration;

const STEPS: u64 = 400;

#[test]
fn r8_monitor_plot_equals_the_residual_lines_and_monitors_json() {
    let mut p = RunnerProject::new("r8", 3);
    p.serve();
    let mut st = studio_on(&p.dir, &p.runner);
    let plan = p.plan(STEPS, true);
    eprintln!("R8: rcfd run --set {:?}", plan.set());
    assert_eq!(
        plan.set(),
        [
            "output.store_interval=4",
            "output.store_precision=f32",
            "solver.max_iterations=400"
        ]
    );
    st.start_run(plan).unwrap();
    assert!(st.drain_until(Duration::from_secs(60), |_| p.ids().len() == 1));
    let id = p.ids()[0].clone();
    st.select_run(&id);
    assert!(
        st.drain_until(Duration::from_secs(300), |s| s
            .runs
            .row(&id)
            .is_some_and(|r| r.state.is_terminal())),
        "the run did not end"
    );
    // the last lines are read by the tick after the terminal status at the latest
    let last_step = p.status(&id).unwrap().step;
    assert!(st.drain_until(Duration::from_secs(10), |s| s
        .monitors
        .series
        .get("lid_drag")
        .and_then(|m| m.x.last().copied())
        == Some(last_step as f64)));
    assert_eq!(st.runs.row(&id).unwrap().state, RunState::Finished);
    let dir = p.run_dir(&id);
    let lines: Vec<Value> = std::fs::read_to_string(dir.join("residuals.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let m = &st.monitors.series["lid_drag"];
    let want_x: Vec<f64> = (1..=last_step).map(|k| k as f64).collect();
    assert_eq!(m.x, want_x, "one point per step");
    assert_eq!(lines.len(), m.x.len());
    let mut differ = 0;
    for (i, l) in lines.iter().enumerate() {
        let v = l["monitors"]["lid_drag"].as_f64().unwrap();
        assert_eq!(l["step"].as_u64(), Some(i as u64 + 1));
        if v.to_bits() != m.y[i].to_bits() {
            differ += 1;
        }
    }
    assert_eq!(differ, 0, "{differ} plotted values differ from residuals.jsonl");
    let monitors: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("output/monitors.json")).unwrap()).unwrap();
    let recorded = monitors["series"]["lid_drag"]["y"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_f64()
        .unwrap();
    let metrics: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("metrics.json")).unwrap()).unwrap();
    let in_metrics = metrics["last_residuals"]["monitors"]["lid_drag"].as_f64().unwrap();
    let last = *m.y.last().unwrap();
    eprintln!(
        "R8: {} steps; last lid_drag plotted {last:e}, monitors.json {recorded:e}, metrics.json {in_metrics:e}",
        m.x.len()
    );
    assert_eq!(last.to_bits(), recorded.to_bits());
    assert_eq!(last.to_bits(), in_metrics.to_bits());
    // negative control: a plot fed from res_p
    let res_p = *st.plots.series["res_p"].y.last().unwrap();
    eprintln!("R8 control: last res_p {res_p:e}");
    assert_ne!(res_p.to_bits(), recorded.to_bits());
    // R10 end to end: the store at runs/<id>/fields, at most 100 snapshots, every field f32
    let mut r = rcfd_tsstore::Reader::open(&dir.join("fields")).unwrap();
    let n = r.snapshots().len();
    let mut precisions = std::collections::BTreeSet::new();
    for k in 0..n {
        for f in r.fields(k).unwrap() {
            precisions.insert(format!("{:?}", f.precision));
        }
    }
    eprintln!(
        "R8/R10: store at runs/{id}/fields, {n} snapshots, precisions {precisions:?}, no output/fields: {}",
        !dir.join("output/fields").exists()
    );
    assert!(n > 0 && n <= 100, "{n} snapshots");
    assert_eq!(precisions.into_iter().collect::<Vec<_>>(), ["F32"]);
    assert!(!dir.join("output/fields").exists());
}
