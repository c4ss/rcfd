//! F-834: a clean stop of a running run (ADR 0161, job 48AA, AA1's engine part), through the workspace's `cancel_run`
//! and the real runner (`rcfd-runner serve` started directly, ADR 0128 rule). Release builds; `rcfd-runner` and `rcfd`
//! of the same build (`f834::binaries`). Its own test binary, so that no other run of these tests shares its time limit.

mod f834;

use f834::{call, RunnerProject};
use rcfd_project::RunState;
use serde_json::json;
use std::time::{Duration, Instant};

/// ADR 0133 R3's cancel limit (AA1 (c)).
const STOP_LIMIT_S: f64 = 5.0;
const WAIT: Duration = Duration::from_secs(60);
/// `rcfd run`'s exit code of a run stopped on request (`rcfd_api::runctl::exit::INTERRUPTED`; the Studio's
/// `stop::INTERRUPTED`).
const INTERRUPTED: i32 = 7;

/// The wind tunnel's `cylinder-re100.toml` as shipped (64 x 48, 80 s; `ts::cylinder(64, 48)` of the deleted tests).
fn cylinder() -> String {
    let path = f834::repo().join("example-projects/wind-tunnel/cases/cylinder-re100.toml");
    let t = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
    assert!(
        t.contains("\nnx = 64\n") && t.contains("\nny = 48\n"),
        "the example's mesh counts moved"
    );
    t
}

fn count(p: &RunnerProject, id: &str, kind: &str) -> usize {
    p.events(id)
        .iter()
        .filter(|e| e["kind"] == kind || e["event"] == kind)
        .count()
}

/// Replaces `crates/rcfd-studio/tests/stop_aa.rs::aa1b_stop_by_the_button_stops_cleanly_with_the_final_snapshot` (ADR
/// 0161 AA1 with amendment 1's events; engine part, the checks of its `assert_clean_stop`): the shipped cylinder is
/// started by `start_run` and, once it runs with two stored snapshots, stopped by `cancel_run`. (c) It ends within 5 s;
/// (e) `status.json` says cancelled, exit code 7, the step it stopped after and the clean-stop reason, no pid; at most
/// one `cancel_requested` event and no `killed`; `metrics.json` says the solver finished `interrupted`; (f) the store
/// is closed with its footer, holds at least three snapshots, the last at the step and time of `status.json`, and the
/// last residual line is that step; (g, engine part) `field_summary` reads the last snapshot. Negative control (in place
/// of the deleted kill control C1, which killed the solver): the store of the running run, before the stop, is not
/// closed with its footer. The Studio ran the case with the time step and correctors of its Run (ADR 0153, ADR 0158;
/// Studio code, F-837); here it runs as shipped, which the stop does not depend on. Not ported (Studio UI): the drawn
/// Stop and Run buttons, Stop's hover, F5 refused while stopping, the frames between click and "stopped", the Runs
/// panel's line and the messages (AA1 (a), (b), (d), (g), (h)); the known-miss variant `aa1_..._known_miss` (ignored).
#[test]
fn aa1_a_clean_stop_writes_the_final_snapshot_status_cancelled_and_exit_7() {
    let mut p = RunnerProject::new("aa1", 3);
    std::fs::write(p.dir.join("cases/cyl.toml"), cylinder()).unwrap();
    p.serve();
    let ws = p.workspace();
    let v = call(&ws, "start_run", json!({"case": "cases/cyl.toml"}));
    let id = v["run"].as_str().unwrap().to_string();
    let dir = p.run_dir(&id);
    let store = dir.join("fields");
    assert!(
        f834::wait_until(WAIT, || p.status(&id).is_some_and(|s| s.state == RunState::Running)
            && f834::store_snapshots(&store) >= 2),
        "the run was not running with 2 stored snapshots within {WAIT:?}: {:?}",
        p.status(&id)
    );
    // negative control: a running run's store is not closed with its footer
    let live = rcfd_tsstore::Reader::open(&store).unwrap();
    let live_source = format!("{:?}", live.index_source());
    eprintln!(
        "AA1 control: the running store: {} snapshots, index {live_source}",
        live.snapshots().len()
    );
    assert_ne!(
        *live.index_source(),
        rcfd_tsstore::IndexSource::Footer,
        "control: footer before the stop"
    );
    drop(live);
    // the stop
    let t0 = Instant::now();
    let r = call(&ws, "cancel_run", json!({"run": id}));
    assert_eq!(r["requested"], "cancel", "{r}");
    let ended = f834::wait_until(Duration::from_secs(30), || {
        p.status(&id).is_some_and(|s| s.state.is_terminal())
    });
    let stopped_s = t0.elapsed().as_secs_f64();
    assert!(ended, "run {id} did not end");
    let s = p.status(&id).unwrap();
    let metrics: serde_json::Value = std::fs::read(dir.join("metrics.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let reader = rcfd_tsstore::Reader::open(&store).unwrap();
    let footer = *reader.index_source() == rcfd_tsstore::IndexSource::Footer;
    let store_count = reader.snapshots().len();
    let store_last = reader.snapshots().last().map(|x| (x.step, x.time.to_bits()));
    drop(reader);
    let residual_last = std::fs::read_to_string(dir.join("residuals.jsonl"))
        .ok()
        .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(String::from))
        .and_then(|l| serde_json::from_str::<rcfd_project::records::ResidualLine>(&l).ok())
        .map(|r| r.step);
    let (cancel_requested, killed) = (count(&p, &id, "cancel_requested"), count(&p, &id, "killed"));
    eprintln!(
        "AA1: ended in {stopped_s:.3} s: {s:?}; events cancel_requested {cancel_requested} killed {killed}; \
         solver_finish {}; store footer {footer}, {store_count} snapshots, last {store_last:?}; last residual step \
         {residual_last:?}",
        metrics["solver_finish"]
    );
    // (c)
    assert!(stopped_s <= STOP_LIMIT_S, "(c) {stopped_s:.3} s");
    // (e)
    assert_eq!(s.state, RunState::Cancelled, "(e) state");
    assert_eq!(s.exit_code, Some(INTERRUPTED), "(e) exit code");
    assert_eq!(i32::from(rcfd_api::runctl::exit::INTERRUPTED), INTERRUPTED);
    assert!(s.step >= 1, "(e) step");
    assert_eq!(
        s.reason.as_deref().unwrap_or_default(),
        format!("cancelled on request; the solver stopped cleanly after step {}", s.step),
        "(e) reason"
    );
    assert_eq!(s.pid, None, "(e) pid");
    assert!(
        cancel_requested <= 1 && killed == 0,
        "(e) events (amendment 1): {cancel_requested} cancel_requested, {killed} killed"
    );
    assert_eq!(
        metrics["solver_finish"]["state"], "interrupted",
        "(e) metrics.json solver_finish"
    );
    // (f)
    assert!(footer, "(f) the store is not closed with its footer");
    assert!(store_count >= 3, "(f) {store_count} snapshots");
    assert_eq!(
        store_last,
        Some((s.step, s.time.to_bits())),
        "(f) last snapshot vs status.json"
    );
    assert_eq!(residual_last, Some(s.step), "(f) residuals.jsonl");
    // (g), engine part: the stored snapshots read
    let v = call(&ws, "field_summary", json!({"run": id, "field": "p"}));
    eprintln!("AA1 (g): {v}");
    assert_eq!(v["snapshot"]["index"].as_u64(), Some(store_count as u64 - 1));
    assert_eq!(v["snapshot"]["step"].as_u64(), Some(s.step));
    assert!(v["finite"].as_u64().is_some_and(|n| n > 0));
}
