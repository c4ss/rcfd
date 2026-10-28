//! ADR 0133 R9 (set-up: amendment 1): no runner alive; the model shows "runner not running" with the queued run, then
//! its start-runner action (`ensure_runner`, the direct start of ADR 0143) shows the runner and the run running within
//! 15 s. Run this binary with its output going to a file (ADR 0123 S5a): the direct start hands the runner the caller's
//! inheritable handles. The drop guard of `RunnerProject` shuts the runner down and kills only the run's solver.

mod common;

use common::{studio_on, RunnerProject};
use rcfd_project::RunState;
use std::time::{Duration, Instant};

const LIMIT_S: f64 = 15.0;

#[test]
fn r9_start_runner_from_the_model() {
    let p = RunnerProject::new("r9", 3);
    let mut st = studio_on(&p.dir, &p.runner);
    st.queue_run(p.plan(2000, false)).unwrap();
    assert!(st.drain_until(Duration::from_secs(60), |_| p.ids().len() == 1));
    let id = p.ids()[0].clone();
    assert!(st.drain_until(Duration::from_secs(10), |s| s.runs.row(&id).is_some()));
    // negative control: the lock probe reads no runner before the start
    assert!(!p.client.runner_running(), "a runner holds the lock before the start");
    assert_eq!(
        st.runs.runner_running,
        Some(false),
        "the model does not show 'runner not running'"
    );
    assert_eq!(st.runs.row(&id).unwrap().state, RunState::Queued);
    let t0 = Instant::now();
    st.start_runner().unwrap();
    let ok = st.drain_until(Duration::from_secs_f64(LIMIT_S), |s| {
        s.runs.runner_running == Some(true)
            && s.runs
                .row(&id)
                .is_some_and(|r| r.state == RunState::Running || r.state.is_terminal())
    });
    let dt = t0.elapsed().as_secs_f64();
    eprintln!(
        "R9: runner shown {:?}, run {:?} after {dt:.3} s; start result {:?}, error {:?}",
        st.runs.runner_running,
        st.runs.row(&id).map(|r| r.state),
        st.runs.starting_runner,
        st.runs.runner_error
    );
    assert!(ok, "not shown within {LIMIT_S} s");
    assert!(dt <= LIMIT_S);
}
