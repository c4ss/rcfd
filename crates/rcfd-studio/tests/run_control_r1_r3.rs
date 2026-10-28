//! ADR 0133 R1-R3 (set-up: amendment 1): the Studio model starts and controls runs through `rcfd-runner serve`
//! (started directly); the run worker's snapshots follow `status.json`.

mod common;

use common::{next_tick, studio_on, RunnerProject};
use rcfd_project::RunState;
use rcfd_studio::Studio;
use std::time::{Duration, Instant};

/// Steps of the R1 runs (rule-d smoke: about 0.9 ms per step of this case in release).
const R1_STEPS: u64 = 2000;
/// Steps of the R2 and R3 runs (long enough to outlive the 2 s gap and the pause, resume and cancel sequence).
const LONG_STEPS: u64 = 15_000;

fn steps(n: u64) -> u64 {
    if cfg!(debug_assertions) {
        n / 4
    } else {
        n
    }
}

fn start_n(st: &mut Studio, p: &RunnerProject, n: usize, steps: u64) {
    for _ in 0..n {
        st.start_run(p.plan(steps, false)).unwrap();
    }
    let want = n;
    assert!(
        st.drain_until(Duration::from_secs(60), |_| p.ids().len() >= want),
        "the runs were not queued"
    );
}

fn states(p: &RunnerProject, ids: &[String]) -> Vec<Option<RunState>> {
    ids.iter().map(|id| p.status(id).map(|s| s.state)).collect()
}

/// Is `shown` within the run's state sequence between `a` and `b` (inclusive)?
fn between(seq: &[RunState], a: Option<RunState>, b: Option<RunState>, shown: RunState) -> bool {
    let (Some(a), Some(b)) = (a, b) else { return false };
    if a == b {
        return shown == a;
    }
    let ia = seq.iter().position(|s| *s == a);
    let ib = seq.iter().rposition(|s| *s == b);
    match (ia, ib) {
        (Some(i), Some(j)) if i <= j => seq[i..=j].contains(&shown),
        _ => false,
    }
}

/// The R1 snapshot check over the whole life of 5 runs; returns (snapshots checked, failures, max active shown).
fn r1_loop(st: &mut Studio, p: &RunnerProject, ids: &[String]) -> (usize, Vec<String>, usize) {
    let (mut checked, mut bad, mut max_active) = (0, Vec::new(), 0);
    let t_end = Instant::now() + Duration::from_secs(300);
    loop {
        let t_a = Instant::now();
        let a = states(p, ids);
        next_tick(st, t_a, Duration::from_secs(10));
        let b = states(p, ids);
        max_active = max_active.max(st.runs.active());
        for (i, id) in ids.iter().enumerate() {
            let Some(shown) = st.runs.row(id).map(|r| r.state) else {
                bad.push(format!("{id}: not shown"));
                continue;
            };
            let seq = p.state_sequence(id);
            if !between(&seq, a[i], b[i], shown) {
                bad.push(format!(
                    "{id}: shown {shown:?}, before {:?}, after {:?}, sequence {seq:?}",
                    a[i], b[i]
                ));
            }
        }
        checked += 1;
        if b.iter().all(|s| s.is_some_and(RunState::is_terminal)) && a == b {
            return (checked, bad, max_active);
        }
        if Instant::now() > t_end {
            bad.push("the runs did not end within 300 s".into());
            return (checked, bad, max_active);
        }
    }
}

#[test]
fn r1_model_states_follow_status_json() {
    let mut p = RunnerProject::new("r1", 2);
    p.serve();
    let mut st = studio_on(&p.dir, &p.runner);
    start_n(&mut st, &p, 5, steps(R1_STEPS));
    let ids = p.ids();
    let (checked, bad, max_active) = r1_loop(&mut st, &p, &ids);
    let finals: Vec<_> = states(&p, &ids);
    eprintln!("R1: {checked} snapshots checked, max running or paused shown {max_active}, final states {finals:?}");
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(max_active <= 2, "{max_active} shown running or paused");
    assert!(finals.iter().all(|s| *s == Some(RunState::Finished)), "{finals:?}");
    assert!(checked >= 5, "under-powered: {checked} snapshots");
}

#[test]
fn r1_control_a_model_that_reads_status_once_shows_a_finished_run_as_running() {
    let mut p = RunnerProject::new("r1ctl", 2);
    p.serve();
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.run_switches.status_once = true;
    st.open_project(&p.dir);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    start_n(&mut st, &p, 5, steps(R1_STEPS));
    let ids = p.ids();
    for id in &ids {
        p.client.wait(id, Duration::from_secs(300)).unwrap();
    }
    next_tick(&mut st, Instant::now(), Duration::from_secs(10));
    let stale: Vec<String> = ids
        .iter()
        .filter(|id| st.runs.row(id).is_none_or(|r| !r.state.is_terminal()))
        .cloned()
        .collect();
    eprintln!(
        "R1 control: {} of 5 finished runs shown non-terminal: {:?}",
        stale.len(),
        ids.iter()
            .map(|id| st.runs.row(id).map(|r| r.state))
            .collect::<Vec<_>>()
    );
    assert!(!stale.is_empty(), "the control did not fail the check");
}

#[test]
fn r2_closing_the_model_leaves_the_runs_running_and_reopening_lists_them() {
    let mut p = RunnerProject::new("r2", 3);
    p.serve();
    let mut a = studio_on(&p.dir, &p.runner);
    start_n(&mut a, &p, 2, steps(LONG_STEPS));
    let ids = p.ids();
    assert!(
        a.drain_until(Duration::from_secs(60), |s| ids
            .iter()
            .all(|id| s.runs.row(id).is_some_and(|r| r.state == RunState::Running))),
        "both runs shown running"
    );
    let pids: Vec<u32> = ids.iter().map(|id| p.status(id).unwrap().pid.unwrap()).collect();
    drop(a);
    let t0 = Instant::now();
    let mut probes = 0;
    while t0.elapsed() < Duration::from_secs(2) {
        for pid in &pids {
            assert!(
                rcfd_runner::sys::pid_alive(*pid, "rcfd"),
                "solver {pid} died while the model was closed"
            );
        }
        probes += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut b = studio_on(&p.dir, &p.runner);
    let first: Vec<_> = ids.iter().map(|id| b.runs.row(id).map(|r| r.state)).collect();
    eprintln!("R2: {probes} probes of 2 pids alive over the gap; reopened, first snapshot {first:?}");
    assert!(first.iter().all(|s| *s == Some(RunState::Running)), "{first:?}");
    assert!(
        b.drain_until(Duration::from_secs(300), |s| ids
            .iter()
            .all(|id| s.runs.row(id).is_some_and(|r| r.state == RunState::Finished))),
        "both runs shown finished"
    );
    for id in &ids {
        let n = p.launched(id).len();
        eprintln!("R2: {id} launched {n} time(s)");
        assert_eq!(n, 1, "{id} launched {n} times");
    }
    // negative control: the probe reads a finished solver as not alive
    std::thread::sleep(Duration::from_millis(500));
    for pid in &pids {
        assert!(
            !rcfd_runner::sys::pid_alive(*pid, "rcfd"),
            "solver {pid} reads alive after it finished"
        );
    }
}

fn max_step(st: &Studio) -> u64 {
    st.plots
        .series
        .values()
        .filter_map(|s| s.x.last().copied())
        .fold(0.0, f64::max) as u64
}

/// R3 with an optional mapped-resume model; returns (paused after s, S0, steps after hold, resumed step or None,
/// cancelled after s).
fn r3(name: &str, resume_as_pause: bool) -> (f64, u64, u64, Option<u64>, Option<f64>) {
    let mut p = RunnerProject::new(name, 3);
    p.serve();
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.run_switches.resume_as_pause = resume_as_pause;
    st.open_project(&p.dir);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    start_n(&mut st, &p, 1, steps(LONG_STEPS));
    let id = p.ids()[0].clone();
    st.select_run(&id);
    assert!(
        st.drain_until(Duration::from_secs(60), |s| s
            .runs
            .row(&id)
            .is_some_and(|r| r.state == RunState::Running)
            && max_step(s) >= 5),
        "the run did not show step 5"
    );
    let t0 = Instant::now();
    st.pause_run(&id).unwrap();
    assert!(
        st.drain_until(Duration::from_secs(10), |s| s
            .runs
            .row(&id)
            .is_some_and(|r| r.state == RunState::Paused)),
        "paused not shown"
    );
    let paused_after = t0.elapsed().as_secs_f64();
    let s0 = max_step(&st);
    let hold = Instant::now();
    let mut held = s0;
    while hold.elapsed() < Duration::from_secs(2) {
        st.drain();
        held = held.max(max_step(&st));
        std::thread::sleep(Duration::from_millis(20));
    }
    st.resume_run(&id).unwrap();
    let resumed = st
        .drain_until(Duration::from_secs(5), |s| max_step(s) > s0)
        .then(|| max_step(&st));
    let t1 = Instant::now();
    st.cancel_run(&id).unwrap();
    let cancelled = st
        .drain_until(Duration::from_secs(10), |s| {
            s.runs.row(&id).is_some_and(|r| r.state == RunState::Cancelled)
        })
        .then(|| t1.elapsed().as_secs_f64());
    (paused_after, s0, held, resumed, cancelled)
}

#[test]
fn r3_pause_resume_cancel_through_the_model() {
    let (paused_after, s0, held, resumed, cancelled) = r3("r3", false);
    eprintln!(
        "R3: paused shown after {paused_after:.3} s at step {s0}; largest step after 2 s held {held}; resumed to {resumed:?}; cancelled shown after {cancelled:?} s"
    );
    assert!(paused_after <= 2.0, "paused shown after {paused_after:.3} s");
    assert_eq!(held, s0, "steps shown while paused");
    assert!(resumed.is_some(), "the run did not move after resume");
    let c = cancelled.expect("cancelled not shown");
    assert!(c <= 5.0, "cancelled shown after {c:.3} s");
}

#[test]
fn r3_control_a_model_that_maps_resume_to_pause_fails_the_step_check() {
    let (_, s0, _, resumed, cancelled) = r3("r3ctl", true);
    eprintln!("R3 control: S0 {s0}, after the mapped resume {resumed:?}; cancelled after {cancelled:?} s");
    assert!(resumed.is_none(), "the run moved although resume sent pause");
}
