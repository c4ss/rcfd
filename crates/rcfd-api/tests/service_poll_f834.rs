//! F-834: polling a live run of the in-process service (`panel.snapshot` with `since`, what the frozen GUI's monitors
//! panel polled) loses no iteration and does not block, and the residual series grows while the run is in progress
//! (ADR 0025 M5, ADR 0095 M7, F-144). The engine and data part of `crates/rcfd-gui/tests/monitors_plot.rs` (removed by
//! F-449); the GUI's monitor store is restated here as [`Series::merge`].

use rcfd_api::service::{Service, ServiceConfig};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The lid-driven cavity template (`rcfd_workspace::templates::LID_DRIVEN_CAVITY`, the GUI's "new from template"),
/// with `solver.max_iterations` and the tolerances replaced.
fn cavity(max_iterations: u64, tolerance: &str) -> String {
    format!(
        r#"title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = {max_iterations}
tolerance_u = {tolerance}
tolerance_p = {tolerance}
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#
    )
}

/// The monitor series as the GUI merged them (`MonitorStore::merge`): per name, x strictly increasing; a sample at or
/// before the last x is a repeat and dropped; a non-finite residual (null) kept as NaN.
#[derive(Default)]
struct Series(BTreeMap<String, (Vec<f64>, Vec<f64>)>);

impl Series {
    fn merge(&mut self, doc: &Value) -> usize {
        let mut added = 0;
        for (name, s) in doc["series"].as_object().into_iter().flatten() {
            let (Some(x), Some(y)) = (s["x"].as_array(), s["y"].as_array()) else {
                continue;
            };
            let dst = self.0.entry(name.clone()).or_default();
            for (xv, yv) in x.iter().zip(y) {
                let Some(xf) = xv.as_f64() else { continue };
                if dst.0.last().is_some_and(|l| xf <= *l) {
                    continue;
                }
                dst.0.push(xf);
                dst.1.push(yv.as_f64().unwrap_or(f64::NAN));
                added += 1;
            }
        }
        added
    }

    /// The largest x of any series (`MonitorStore::since`).
    fn since(&self) -> Option<f64> {
        self.0
            .values()
            .filter_map(|s| s.0.last().copied())
            .fold(None, |a, x| Some(a.map_or(x, |m: f64| m.max(x))))
    }

    fn len(&self, name: &str) -> usize {
        self.0.get(name).map_or(0, |s| s.0.len())
    }
}

/// A session of `toml`, initialized (the GUI's `auto_init`) and started (`run.start`).
fn started(svc: &Arc<Service>, toml: &str) -> String {
    let id = svc.call("session.create", &json!({"toml": toml})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    svc.call("run.start", &json!({"session": id})).unwrap();
    id
}

/// One poll as the GUI's `Controller::poll` made it: `panel.snapshot` from the last merged x; (state, iteration).
fn poll(svc: &Arc<Service>, id: &str, m: &mut Series) -> (String, u64) {
    let mut p = json!({"session": id, "log_lines": 200});
    if let Some(s) = m.since() {
        p["since"] = json!(s);
    }
    let snap = svc.call("panel.snapshot", &p).unwrap();
    if !snap["monitors"].is_null() {
        m.merge(&snap["monitors"]);
    }
    let st = &snap["status"];
    (
        st["state"].as_str().unwrap_or("none").to_string(),
        st["simulation"]["iteration"].as_u64().unwrap_or(0),
    )
}

/// Replaces `crates/rcfd-gui/tests/monitors_plot.rs::m5_polling_a_running_case_does_not_block_and_loses_no_iteration`
/// (ADR 0025 M5; engine part): a 60-iteration cavity polled while it runs. Every poll returns within 1 s, at least one
/// poll sees the run in progress (the check is not vacuous), and the merged `res_u` series holds exactly one sample per
/// iteration, x = 1 to the last iteration. Not ported: the GUI controller and its field and log views.
#[test]
fn m5_polling_a_running_case_does_not_block_and_loses_no_iteration() {
    let svc = Service::new(ServiceConfig::default());
    let id = started(&svc, &cavity(60, "1e-6"));
    let mut m = Series::default();
    let mut worst = Duration::ZERO;
    let mut saw_running_partial = false;
    let t0 = Instant::now();
    loop {
        let t = Instant::now();
        let (state, iteration) = poll(&svc, &id, &mut m);
        worst = worst.max(t.elapsed());
        if state == "running" && iteration < 60 {
            saw_running_partial = true;
        }
        if state != "running" && state != "paused" && iteration > 0 {
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(120), "the run did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    svc.call("run.wait", &json!({"session": id, "timeout_ms": 60_000}))
        .unwrap();
    let (state, iteration) = poll(&svc, &id, &mut m);
    eprintln!(
        "m5: worst poll {worst:?}, saw running partial: {saw_running_partial}, iteration {iteration}, state {state}"
    );
    assert!(worst < Duration::from_secs(1), "a poll took {worst:?}");
    assert!(
        saw_running_partial,
        "the run finished before any poll saw it running (non-vacuous check failed)"
    );
    let (x, _) = &m.0["res_u"];
    assert_eq!(x.len() as u64, iteration);
    assert!(x.windows(2).all(|w| w[1] > w[0]));
    let want: Vec<f64> = (1..=iteration).map(|i| i as f64).collect();
    assert_eq!(*x, want, "an iteration lost or repeated");
    // control: polling again from the last x adds nothing
    let n = m.len("res_u");
    let _ = poll(&svc, &id, &mut m);
    assert_eq!(m.len("res_u"), n, "a re-poll added samples");
}

/// Replaces `crates/rcfd-gui/tests/monitors_plot.rs::m7_plot_series_grows_and_frames_draw_while_running` (ADR 0095 M7;
/// engine and data part): a long cavity run (200 000 iterations, tolerances that do not stop it) polled every 20 ms;
/// while the state is `running` the `res_u` series takes at least four distinct non-zero lengths, strictly growing, each
/// poll within 1 s, and the run is mid-way when the evidence is in; then it is stopped. Not ported (GUI): the frames
/// drawn while running, their time and shape count, the Monitors panel and its 250 ms frame poll.
#[test]
fn m7_the_series_grows_while_running() {
    let svc = Service::new(ServiceConfig::default());
    let id = started(&svc, &cavity(200_000, "1e-14"));
    let mut m = Series::default();
    let mut lens: Vec<usize> = Vec::new();
    let (mut polls_running, mut worst) = (0usize, Duration::ZERO);
    let t0 = Instant::now();
    let mut last_state = String::new();
    while t0.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
        let t = Instant::now();
        let (state, _) = poll(&svc, &id, &mut m);
        let dt = t.elapsed();
        last_state = state.clone();
        if state != "running" {
            continue;
        }
        polls_running += 1;
        worst = worst.max(dt);
        let n = m.len("res_u");
        if lens.last() != Some(&n) {
            lens.push(n);
        }
        if lens.len() >= 5 {
            break;
        }
    }
    svc.call("run.stop", &json!({"session": id})).unwrap();
    svc.call("run.wait", &json!({"session": id, "timeout_ms": 60_000}))
        .unwrap();
    eprintln!(
        "m7: series lengths while running {lens:?}, polls in running state {polls_running}, worst poll {worst:?}, \
         state at the end of the loop {last_state}"
    );
    // the length before the first sample is 0 and does not count
    let grown: Vec<usize> = lens.into_iter().filter(|&n| n > 0).collect();
    assert!(
        grown.len() >= 4,
        "fewer than four distinct series lengths while running: {grown:?}"
    );
    assert!(
        grown.windows(2).all(|w| w[1] > w[0]),
        "the series did not grow strictly: {grown:?}"
    );
    assert!(*grown.last().unwrap() < 200_000, "the run was not mid-way");
    assert!(polls_running >= 3, "{polls_running} polls in state running");
    assert!(worst < Duration::from_secs(1), "a poll took {worst:?}");
}
