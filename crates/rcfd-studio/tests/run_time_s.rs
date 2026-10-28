//! ADR 0153 (job 48S): the time step is set when a run starts, through the Studio's Run (F5's call) and the real runner:
//! (a) the maintainer's 256 x 256 cylinder runs stably with the automatic step and sheds (heavy, ignored, run on
//! purpose), and its control (the case's own `solver.dt = 0.0625`); (b) the shipped cylinder keeps its shedding band with
//! the automatic step; (c) a run ends exactly at the simulated time asked for, finished, with its last snapshot there; (d)
//! the case file is not touched and the run's settings record the time entries; (g) a case without a speed scale; (h) a
//! JSON case. (f) the toolbar's time row, its hover, the override indicator, the Courant warning and its fix, the Mesh
//! side's line, drawn headless. Negative controls: the route before (`QuickRun::switches.case_time`). Release builds,
//! `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd` of the same build (`common::binaries`).

mod common;
mod lj;
mod pv;
mod ts;

use common::RunnerProject;
use rcfd_project::RunState;
use rcfd_studio::quick_run::{self, TimeView};
use rcfd_studio::quick_run_ui::{ADVANCED, AUTO_BOX, FIX, RUN_HOVER, SIMULATE_FOR};
use rcfd_studio::time_step::{self, StepMode};
use rcfd_studio::{Studio, StudioApp};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(60);

/// A served project holding `cases`.
fn project(tag: &str, cases: &[(&str, String)]) -> RunnerProject {
    let mut p = RunnerProject::new(tag, 3);
    for (rel, text) in cases {
        std::fs::write(p.dir.join(rel), text).unwrap();
    }
    p.serve();
    p
}

/// The Studio model on `p` (watcher and run worker on), the Run button searching `p`'s runner; the route before ADR
/// 0153 when `before`.
fn studio(p: &RunnerProject, before: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.quick.switches.case_time = before;
    st.open_project(&p.dir);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()),
        "the project did not open"
    );
    st
}

/// Select `rel` and drain until its mesh is shown and (unless the route before) its time-step facts are known.
fn select(st: &mut Studio, rel: &str) {
    st.open_case(rel);
    let before = st.quick.switches.case_time;
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none()
            && lj::settled(s)
            && (before || quick_run::facts_of(s, rel).is_some())),
        "{rel}: not settled: {:?}",
        st.preview.status
    );
}

/// Run as F5 asks for it; the id of the run it queued once the start ended, or the refusal.
fn run_now(st: &mut Studio) -> Result<String, String> {
    let before = st.quick.run.as_ref().map(|r| r.0.clone());
    quick_run::run_selected_case(st)?;
    assert!(
        st.drain_until(WAIT, |s| s.quick.starting.is_none()),
        "the start did not end"
    );
    match st.quick.run.as_ref().map(|r| r.0.clone()) {
        Some(id) if Some(&id) != before.as_ref() => Ok(id),
        _ => Err(st.quick.error.clone().unwrap_or_default()),
    }
}

/// Wait until run `id` ended (any terminal state) or `limit` passed; its status.
fn wait_end(p: &RunnerProject, st: &mut Studio, id: &str, limit: Duration) -> rcfd_project::records::RunStatus {
    let ended = |s: &Option<rcfd_project::records::RunStatus>| s.as_ref().is_some_and(|x| x.state.is_terminal());
    assert!(
        st.drain_until(limit, |_| ended(&p.status(id))),
        "run {id} did not end within {limit:?}: {:?}",
        p.status(id).map(|s| (s.state, s.step, s.time))
    );
    p.status(id).unwrap()
}

/// The monitor columns of run `id` (`runs/<id>/output/monitors.csv`).
fn monitors(p: &RunnerProject, id: &str) -> BTreeMap<String, Vec<f64>> {
    ts::monitors(&p.run_dir(id).join("output/monitors.csv"))
}

/// The `set` of run `id`'s `run.json`.
fn run_set(p: &RunnerProject, id: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap();
    v["settings"]["set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

// ---------------------------------------------------------------- (a) the 256 x 256 cylinder

/// What (a) and its control measured (printed whole).
#[derive(Debug)]
#[allow(dead_code)]
struct A {
    set: Vec<String>,
    state: RunState,
    end_bits: u64,
    end: f64,
    steps: usize,
    co_first: f64,
    co_max_after_10: f64,
    co_max: f64,
    finite: bool,
    shedding: ts::Shedding,
    wall_s: f64,
}

/// (a)'s procedure: `cases/cyl256.toml` (the wind tunnel's cylinder at 256 x 256, as the maintainer refined it) selected,
/// "Simulate for" `n` s, the step `mode`, Run, until it ended (`limit`); the lift's shedding over `[from, n]`.
fn a_run(tag: &str, n: f64, from: f64, mode: StepMode, limit: Duration) -> A {
    let p = project(tag, &[("cases/cyl256.toml", ts::cylinder(256, 256))]);
    let mut st = studio(&p, false);
    select(&mut st, "cases/cyl256.toml");
    {
        let rt = quick_run::run_time_mut(&mut st, "cases/cyl256.toml");
        rt.simulate = Some(n);
        rt.mode = mode;
    }
    let t0 = Instant::now();
    let id = run_now(&mut st).unwrap();
    let s = wait_end(&p, &mut st, &id, limit);
    let wall_s = t0.elapsed().as_secs_f64();
    let m = monitors(&p, &id);
    let (t, co, cl) = (&m["time"], &m["courant"], &m["cl"]);
    let a = A {
        set: run_set(&p, &id),
        state: s.state,
        end_bits: s.time.to_bits(),
        end: s.time,
        steps: t.len(),
        co_first: co.first().copied().unwrap_or(f64::NAN),
        co_max_after_10: ts::max_from(co, 10),
        co_max: ts::max_from(co, 0),
        finite: m.values().all(|c| c.iter().all(|x| x.is_finite())),
        shedding: ts::shedding(t, cl, from, n),
        wall_s,
    };
    eprintln!("(a) {tag}: {a:#?}");
    a
}

/// (a): the 256 x 256 cylinder with the automatic step, 40 s: finished at 40 s bit for bit, every monitor finite, the
/// Courant number of the first step at most the target and at most 1.1 times it after step 10; the lift sheds over
/// [10, 40] s (at least 3 upward crossings of its mean, Strouhal number in [0.14, 0.20], amplitude at least 0.1); within 3
/// hours.
#[test]
#[ignore = "heavy: ADR 0153 (a) (60 to 90 minutes)"]
fn a_cylinder_256_automatic_step_is_stable_and_sheds() {
    let a = a_run("a", 40.0, 10.0, StepMode::Adaptive, Duration::from_secs(3 * 3600));
    let c = time_step::COURANT_TARGET;
    assert_eq!(a.state, RunState::Finished, "state");
    assert_eq!(a.end_bits, 40f64.to_bits(), "end {}", a.end);
    assert!(a.finite, "a monitor is not finite");
    assert!(a.co_first <= c, "first Courant number {}", a.co_first);
    assert!(
        a.co_max_after_10 <= 1.1 * c,
        "Courant {} after step 10",
        a.co_max_after_10
    );
    assert!(a.shedding.crossings >= 3, "{:?}", a.shedding);
    let st = a.shedding.strouhal.unwrap();
    assert!((0.14..=0.20).contains(&st), "St {st}");
    assert!(a.shedding.amplitude >= 0.1, "amplitude {}", a.shedding.amplitude);
    assert!(a.wall_s <= 3.0 * 3600.0, "wall {} s", a.wall_s);
}

/// (a) control: the same case with its own `solver.dt = 0.0625` (Advanced: the automatic step off), 5 s: the first
/// Courant number is above the target (predicted 6.3), and (predicted) the Courant number passes 10 within the 5 s.
#[test]
fn a_control_the_case_step_on_256_is_far_above_the_target() {
    let a = a_run("actl", 5.0, 2.5, StepMode::Case, Duration::from_secs(1800));
    assert_eq!(a.set, vec!["solver.end_time=5".to_string()], "the case's own step");
    assert!(
        a.co_first > time_step::COURANT_TARGET,
        "control: first Courant number {}",
        a.co_first
    );
    assert!(
        a.co_max > 10.0,
        "control (predicted): largest Courant number {}",
        a.co_max
    );
}

// ---------------------------------------------------------------- (b) the shipped cylinder

/// (b)'s procedure: the wind tunnel's `cylinder-re100.toml` (64 x 48) selected and Run with its defaults ("Simulate for"
/// its own 80 s).
fn b_run(tag: &str, before: bool) -> A {
    let p = project(tag, &[("cases/cyl.toml", ts::cylinder(64, 48))]);
    let mut st = studio(&p, before);
    select(&mut st, "cases/cyl.toml");
    let t0 = Instant::now();
    let id = run_now(&mut st).unwrap();
    let s = wait_end(&p, &mut st, &id, Duration::from_secs(900));
    let m = monitors(&p, &id);
    let (t, co, cl) = (&m["time"], &m["courant"], &m["cl"]);
    let a = A {
        set: run_set(&p, &id),
        state: s.state,
        end_bits: s.time.to_bits(),
        end: s.time,
        steps: t.len(),
        co_first: co.first().copied().unwrap_or(f64::NAN),
        co_max_after_10: ts::max_from(co, 10),
        co_max: ts::max_from(co, 0),
        finite: m.values().all(|c| c.iter().all(|x| x.is_finite())),
        shedding: ts::shedding(t, cl, 40.0, 80.0),
        wall_s: t0.elapsed().as_secs_f64(),
    };
    eprintln!("(b) {tag}: {a:#?}");
    a
}

/// (b): the shipped cylinder with the automatic step: finished at 80 s bit for bit, the Courant number at most the
/// target at the first step and at most 1.1 times it after step 10, the lift over [40, 80] s in ADR 0142's band of the
/// example (at least 3 upward crossings, Strouhal number in [0.14, 0.20], amplitude at least 0.1).
#[test]
fn b_shipped_cylinder_automatic_step_keeps_the_band() {
    let a = b_run("b", false);
    let c = time_step::COURANT_TARGET;
    assert_eq!(
        (a.state, a.end_bits),
        (RunState::Finished, 80f64.to_bits()),
        "end {}",
        a.end
    );
    assert!(
        a.co_first <= c && a.co_max_after_10 <= 1.1 * c,
        "Courant {} / {}",
        a.co_first,
        a.co_max_after_10
    );
    assert!(a.shedding.crossings >= 3, "{:?}", a.shedding);
    let st = a.shedding.strouhal.unwrap();
    assert!((0.14..=0.20).contains(&st), "St {st}");
    assert!(a.shedding.amplitude >= 0.1, "amplitude {}", a.shedding.amplitude);
}

/// (b) control: the route before (the case's own `solver.dt = 0.0625`): its Courant number is above 1.1 times the target.
#[test]
fn b_control_the_route_before_runs_above_the_target() {
    let a = b_run("bctl", true);
    assert!(
        a.co_max_after_10 > 1.1 * time_step::COURANT_TARGET,
        "control: Courant {} after step 10",
        a.co_max_after_10
    );
}

// ---------------------------------------------------------------- (c), (d) ends at N, the case untouched

/// What (c) and (d) measured.
#[derive(Debug, Default)]
struct Cd {
    queued: String,
    set: Vec<String>,
    plan_set: Vec<String>,
    state: Option<RunState>,
    end_bits: u64,
    reason: Option<String>,
    snapshots: Vec<(u64, f64)>,
    steps: usize,
    last_fraction: f64,
    sane: Vec<(String, bool, String)>,
    p_ratio: f64,
    file_same: bool,
    snapshot_same: bool,
    editor: Option<(bool, f64, f64)>,
    dt_first: f64,
    dt_max: f64,
}

/// (c) and (d)'s procedure: the shipped cylinder (64 x 48, its own store every 1 s) as `cases/cyl.toml`, selected,
/// "Simulate for" 7.3 s (not a whole number of store intervals nor of the first step), Run, until it ended.
fn cd_run(tag: &str, before: bool) -> Cd {
    let p = project(tag, &[("cases/cyl.toml", ts::cylinder(64, 48))]);
    let file = p.dir.join("cases/cyl.toml");
    let bytes = std::fs::read(&file).unwrap();
    let mut st = studio(&p, before);
    select(&mut st, "cases/cyl.toml");
    quick_run::run_time_mut(&mut st, "cases/cyl.toml").simulate = Some(7.3);
    let id = run_now(&mut st).unwrap();
    let plan_set = st.quick.run.as_ref().unwrap().1.set();
    let queued = st
        .messages
        .iter()
        .rev()
        .find(|m| m.starts_with("queued run"))
        .cloned()
        .unwrap_or_default();
    let s = wait_end(&p, &mut st, &id, Duration::from_secs(600));
    let mut r = Cd {
        queued,
        set: run_set(&p, &id),
        plan_set,
        state: Some(s.state),
        end_bits: s.time.to_bits(),
        reason: s.reason.clone(),
        ..Cd::default()
    };
    let m = monitors(&p, &id);
    let (dt, n) = (&m["dt"], m["dt"].len());
    r.steps = n;
    r.last_fraction = dt[n - 1] / dt[n - 2];
    r.dt_first = dt[0];
    r.dt_max = ts::max_from(dt, 0);
    for k in ["cd", "cl"] {
        let v = &m[k];
        let w = &v[n - 51..n - 1];
        let (lo, hi) = w
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| (l.min(*x), h.max(*x)));
        let range = hi - lo;
        let step = w.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0, f64::max);
        let (last, prev) = (v[n - 1], v[n - 2]);
        let ok = last >= lo - 0.1 * range && last <= hi + 0.1 * range && (last - prev).abs() <= 5.0 * step;
        r.sane.push((
            k.to_string(),
            ok,
            format!(
                "last {last}, range [{lo}, {hi}], change {} (limit 5 x {step})",
                (last - prev).abs()
            ),
        ));
    }
    let mut store = rcfd_tsstore::Reader::open(&p.run_dir(&id).join("fields")).unwrap();
    r.snapshots = store.snapshots().iter().map(|s| (s.step, s.time)).collect();
    let k = r.snapshots.len();
    let pmax = |v: Vec<f64>| v.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    r.p_ratio = pmax(store.read_field(k - 1, "p").unwrap()) / pmax(store.read_field(k - 2, "p").unwrap());
    r.file_same = std::fs::read(&file).unwrap() == bytes;
    r.snapshot_same = std::fs::read(p.run_dir(&id).join("case.snapshot")).unwrap() == bytes;
    r.editor = st.editor.as_ref().filter(|e| e.rel == "cases/cyl.toml").map(|e| {
        (
            e.is_dirty(),
            e.settings["solver"]["dt"].as_f64().unwrap_or(f64::NAN),
            e.settings["solver"]["end_time"].as_f64().unwrap_or(f64::NAN),
        )
    });
    eprintln!("(c)(d) {tag}: {r:#?}");
    r
}

/// (c) and (d): the run ends at exactly 7.3 s, finished, and stores its final state there; the last step is an ordinary
/// step (at least 5 % of the one before), its `cd` and `cl` and the last snapshot's pressure are not spoiled (ADR 0145
/// C1's sanity limits). (d): the case file and its run snapshot are byte-equal to the file before the run, the case editor
/// holds the case clean with its own `solver.dt` and `solver.end_time`, the run's `run.json` `set` is the plan's (the
/// time entries, after the case's own store keys), the status bar names them, and the solver's step grew from the first
/// step (the adaptive controller of ADR 0050 ran, not the earlier controller that never steps above `solver.dt`).
#[test]
fn cd_ends_at_the_time_asked_for_and_leaves_the_case_alone() {
    let r = cd_run("cd", false);
    // (c)
    assert_eq!(r.state, Some(RunState::Finished), "(c) state");
    assert_eq!(r.end_bits, 7.3f64.to_bits(), "(c) end time");
    assert_eq!(
        r.reason.as_deref(),
        Some("finished: end time 7.3 reached"),
        "(c) reason"
    );
    let times: Vec<f64> = r.snapshots.iter().map(|s| s.1).collect();
    assert_eq!(r.snapshots.len(), 8, "(c) snapshots {times:?}");
    assert_eq!(r.snapshots[7], (r.steps as u64, 7.3), "(c) the last snapshot");
    for (i, (_, t)) in r.snapshots[..7].iter().enumerate() {
        assert!(
            *t >= (i + 1) as f64 && *t < (i + 1) as f64 + 0.1,
            "(c) snapshot {i} at {t}"
        );
    }
    assert!(
        r.last_fraction >= 0.05,
        "(c) last step {} of the one before",
        r.last_fraction
    );
    for (k, ok, why) in &r.sane {
        assert!(ok, "(c) {k}: {why}");
    }
    assert!(r.p_ratio <= 2.0, "(c) the last snapshot's pressure ratio {}", r.p_ratio);
    // (d)
    assert!(r.file_same, "(d) the case file changed");
    assert!(r.snapshot_same, "(d) case.snapshot is not the case file");
    assert_eq!(r.editor, Some((false, 0.0625, 80.0)), "(d) the case editor");
    let want = vec![
        "solver.end_time=7.3".to_string(),
        "solver.dt=0.00602".to_string(),
        "solver.max_courant=0.5".to_string(),
        "solver.max_dt=0.073".to_string(),
    ];
    assert_eq!(r.plan_set, want, "(d) the plan");
    assert_eq!(r.set, want, "(d) run.json");
    assert!(
        r.queued.ends_with(&format!("(rcfd run --set {})", want.join(" "))),
        "(d) status bar: {}",
        r.queued
    );
    assert_eq!(r.dt_first, 0.00602, "(d) first step");
    assert!(r.dt_max > 1.2 * r.dt_first, "(d) the step did not grow: {}", r.dt_max);
}

/// (c), (d) control: the route before runs the case's own end time (80 s) and adds no time entries.
#[test]
fn cd_control_the_route_before_runs_to_the_case_end() {
    let r = cd_run("cdctl", true);
    assert_ne!(r.end_bits, 7.3f64.to_bits(), "control (c)");
    assert!(
        r.set.iter().all(|e| !e.starts_with("solver.")),
        "control (d): {:?}",
        r.set
    );
}

// ---------------------------------------------------------------- (g) no speed scale, (h) JSON

/// (g)'s procedure: `text` as `cases/g.toml` through Run, "Simulate for" 2 s: the toolbar's hover, the run's `set`, its
/// end, and the largest Courant number after step 10.
fn g_run(tag: &str, text: String) -> (String, Vec<String>, RunState, u64, f64, bool) {
    let p = project(tag, &[("cases/g.toml", text)]);
    let mut st = studio(&p, false);
    select(&mut st, "cases/g.toml");
    quick_run::run_time_mut(&mut st, "cases/g.toml").simulate = Some(2.0);
    let hover = match quick_run::time_view(&st) {
        TimeView::Transient {
            facts, plan: Ok(tp), ..
        } => time_step::hover(&facts, &tp),
        other => panic!("(g) {tag}: {other:?}"),
    };
    let id = run_now(&mut st).unwrap();
    let s = wait_end(&p, &mut st, &id, Duration::from_secs(900));
    let m = monitors(&p, &id);
    let co = ts::max_from(&m["courant"], 10);
    let finite = m.values().all(|c| c.iter().all(|x| x.is_finite()));
    eprintln!(
        "(g) {tag}: set {:?}, state {:?}, end {}, steps {}, Courant after step 10 {co}, first Courant numbers {:?}, \
         first steps {:?}\n  hover: {hover}",
        run_set(&p, &id),
        s.state,
        s.time,
        m["dt"].len(),
        &m["courant"][..m["courant"].len().min(12)],
        &m["dt"][..m["dt"].len().min(12)]
    );
    (hover, run_set(&p, &id), s.state, s.time.to_bits(), co, finite)
}

/// (g1): the heated cavity at rest under PISO (buoyancy only) gets the gravity scale: its run's settings are the
/// registered plan, it finishes at 2 s bit for bit with every monitor finite, and the controller keeps the Courant number
/// at most 1.1 times the target after step 10.
#[test]
fn g1_a_flow_at_rest_under_gravity_gets_the_gravity_scale() {
    let (hover, set, state, end, co, finite) = g_run("g1", ts::heated_piso(10.0));
    assert!(hover.contains(time_step::GRAVITY_SCALE), "(g1) hover: {hover}");
    // the example sets its own store (every 10 steps, f32): no store entry
    assert_eq!(
        set,
        vec![
            "solver.end_time=2",
            "solver.dt=0.000148",
            "solver.max_courant=0.5",
            "solver.max_dt=0.02"
        ]
    );
    assert_eq!((state, end), (RunState::Finished, 2f64.to_bits()));
    assert!(finite, "(g1) a monitor is not finite");
    assert!(co <= 1.1 * time_step::COURANT_TARGET, "(g1) Courant {co}");
}

/// (g2): the cavity with its lid ramped from rest (no speed scale): the toolbar's hover says so, the run starts with the
/// case's `solver.dt` 0.01 s under the controller, finishes at 2 s bit for bit with every monitor finite, and the
/// controller keeps the Courant number at most 1.1 times the target after step 10.
#[test]
fn g2_a_case_without_a_speed_scale_runs_under_the_controller() {
    let (hover, set, state, end, co, finite) = g_run("g2", ts::cavity_ramp_piso(10.0));
    assert!(hover.contains(time_step::NO_SPEED), "(g2) hover: {hover}");
    // the example sets its own store (every 5 steps, f32): no store entry
    assert_eq!(
        set,
        vec![
            "solver.end_time=2",
            "solver.dt=0.01",
            "solver.max_courant=0.5",
            "solver.max_dt=0.02"
        ]
    );
    assert_eq!((state, end), (RunState::Finished, 2f64.to_bits()));
    assert!(finite, "(g2) a monitor is not finite");
    assert!(co <= 1.1 * time_step::COURANT_TARGET, "(g2) Courant {co}");
}

/// (h)'s procedure: the cylinder as `cases/c.toml` and as `cases/c.json` (`Case::to_json_string`); the JSON case
/// selected, "Simulate for" 2.3 s, Run; the TOML case's plan for the same settings.
fn h_run(tag: &str, before: bool) -> (Vec<String>, Vec<String>, Option<RunState>, u64, bool) {
    let toml = ts::cylinder(64, 48);
    let json = rcfd_api::case::Case::parse_toml(&toml).unwrap().to_json_string();
    let p = project(tag, &[("cases/c.toml", toml.clone()), ("cases/c.json", json.clone())]);
    let mut st = studio(&p, before);
    select(&mut st, "cases/c.json");
    quick_run::run_time_mut(&mut st, "cases/c.json").simulate = Some(2.3);
    let id = run_now(&mut st).unwrap();
    let s = wait_end(&p, &mut st, &id, Duration::from_secs(600));
    let f = time_step::facts(&toml, &p.dir.join("cases")).unwrap();
    let toml_plan = time_step::plan(
        &f,
        &time_step::RunTime {
            simulate: Some(2.3),
            ..Default::default()
        },
    )
    .unwrap();
    let same = std::fs::read(p.run_dir(&id).join("case.snapshot")).unwrap() == json.as_bytes();
    let r = (
        run_set(&p, &id),
        toml_plan.entries,
        Some(s.state),
        s.time.to_bits(),
        same,
    );
    eprintln!("(h) {tag}: {r:?}");
    r
}

/// (h): the JSON case runs with the time entries of its TOML twin, finishes at 2.3 s bit for bit, and its run snapshot is
/// the JSON file.
#[test]
fn h_a_json_case_gets_the_time_settings_of_its_toml() {
    let (set, toml_entries, state, end, same) = h_run("h", false);
    assert_eq!(set, toml_entries, "(h) run.json");
    assert_eq!((state, end), (Some(RunState::Finished), 2.3f64.to_bits()));
    assert!(same, "(h) case.snapshot");
}

/// (h) control: the route before adds no time entries to the JSON case's run.
#[test]
fn h_control_the_route_before_adds_none() {
    let (set, ..) = h_run("hctl", true);
    assert!(set.iter().all(|e| !e.starts_with("solver.")), "control: {set:?}");
}

// ---------------------------------------------------------------- (f) the toolbar, drawn

/// What (f) drew.
#[derive(Debug, Default)]
struct F {
    row: Vec<String>,
    hover: Vec<String>,
    after_simulate: Vec<String>,
    popover: Vec<String>,
    warned: Vec<String>,
    fixed: Vec<String>,
    steady: Vec<String>,
    mesh: Vec<String>,
    file_same: bool,
    dirty: bool,
}

fn frame(app: &mut StudioApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<String> {
    lj::texts(&lj::frame_with(app, ctx, events))
}

/// (f)'s procedure: a served project with the cylinder (64 x 48) and the quick start's steady cavity; drawn frames of
/// 1280 x 800, tooltips at once. (i) the cylinder selected, a frame; (ii) the pointer on Run (30 frames at most); (iii)
/// "Simulate for" 7.3 s (the model's call the field makes), a frame; (iv) a drawn click on Advanced, a frame, a drawn click
/// on its checkbox (the automatic step off), Escape (the popover closes), frames; (v) a drawn click on the warning's fix, a
/// frame; (vi) the steady cavity selected, a frame.
fn f_run(tag: &str, before: bool) -> F {
    let cav = std::fs::read_to_string(ts::quick_start_cases().join("cavity-re100.toml")).unwrap();
    let p = project(
        tag,
        &[("cases/cyl.toml", ts::cylinder(64, 48)), ("cases/cav.toml", cav)],
    );
    let file = p.dir.join("cases/cyl.toml");
    let bytes = std::fs::read(&file).unwrap();
    let mut st = studio(&p, before);
    select(&mut st, "cases/cyl.toml");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let mut r = F {
        row: frame(&mut app, &ctx, Vec::new()),
        ..F::default()
    };
    r.mesh = r.row.clone();
    // (ii)
    let pos = app.st.quick.rects["Run"].center();
    for _ in 0..30 {
        r.hover = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        if r.hover.iter().any(|t| t == RUN_HOVER) && r.hover.len() > r.row.len() {
            break;
        }
    }
    let _ = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
    // (iii)
    quick_run::run_time_mut(&mut app.st, "cases/cyl.toml").simulate = Some(7.3);
    r.after_simulate = frame(&mut app, &ctx, Vec::new());
    // (iv)
    if let Some(a) = app.st.quick.rects.get(ADVANCED).copied() {
        lj::click_at(&mut app, &ctx, a.center());
        r.popover = frame(&mut app, &ctx, Vec::new());
        if let Some(b) = app.st.quick.rects.get(AUTO_BOX).copied() {
            lj::click_at(&mut app, &ctx, b.center());
        }
        // Escape closes the popover (it may cover the warning's row below the toolbar)
        let _ = frame(&mut app, &ctx, lj::key(egui::Key::Escape));
        let _ = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
        r.warned = frame(&mut app, &ctx, Vec::new());
        // (v)
        if let Some(f) = app.st.quick.rects.get(FIX).copied() {
            lj::click_at(&mut app, &ctx, f.center());
        }
        r.fixed = frame(&mut app, &ctx, Vec::new());
    }
    r.file_same = std::fs::read(&file).unwrap() == bytes;
    r.dirty = app.st.editor.as_ref().is_some_and(|e| e.is_dirty());
    // (vi)
    select(&mut app.st, "cases/cav.toml");
    r.steady = frame(&mut app, &ctx, Vec::new());
    eprintln!("(f) {tag}: {r:#?}");
    r
}

fn drawn(texts: &[String], t: &str) -> bool {
    texts.iter().any(|x| x == t)
}

/// The facts and plans (f) expects, from the model's own functions on the same file.
fn f_expected() -> (
    time_step::Facts,
    time_step::TimePlan,
    time_step::TimePlan,
    time_step::TimePlan,
) {
    let dir = ts::wind_tunnel_cases();
    let f = time_step::facts(&ts::cylinder(64, 48), &dir).unwrap();
    let rt = time_step::RunTime::default();
    let p80 = time_step::plan(&f, &rt).unwrap();
    let rt73 = time_step::RunTime {
        simulate: Some(7.3),
        ..rt.clone()
    };
    let p73 = time_step::plan(&f, &rt73).unwrap();
    let case = time_step::plan(
        &f,
        &time_step::RunTime {
            mode: StepMode::Case,
            ..rt73
        },
    )
    .unwrap();
    (f, p80, p73, case)
}

/// (f): (i) the row draws "Simulate for", the simulated time 80, "Advanced", the step `dt auto: 0.00602 s at the start,
/// then adaptive (Courant 0.5)` and the indicator `overrides the case's solver.dt 0.0625 s for this run`; the Mesh side
/// draws the time-step line of `time_step::mesh_line`; (ii) Run's hover draws Run's text and the plan's hover text; (iii)
/// after "Simulate for" 7.3 the indicator also names `solver.end_time 80 s`, the case file is unchanged and the editor
/// clean; (iv) the popover draws its checkbox and "Courant target"; with the automatic step off the row draws `dt 0.0625
/// s, the case's own` and the warning of `time_step::warning` with its fix; (v) after the fix, the automatic step again
/// and no warning; (vi) the steady cavity draws `steady (simplec): until converged, at most 2,000 iterations` and no
/// "Simulate for".
#[test]
fn f_the_toolbar_time_row_drawn() {
    let (f, p80, p73, case) = f_expected();
    let r = f_run("f", false);
    // (i)
    for t in [
        SIMULATE_FOR,
        "80",
        ADVANCED,
        "dt auto: 0.00602 s at the start, then adaptive (Courant 0.5)",
        "overrides the case's solver.dt 0.0625 s for this run",
    ] {
        assert!(drawn(&r.row, t), "(i) not drawn: {t}");
    }
    let line = time_step::mesh_line(&f, &time_step::RunTime::default()).unwrap();
    assert_eq!(
        line,
        "Courant estimate 5.184 at the case's solver.dt 0.0625 s (1.116 m/s through the thinnest cell, 0.01346 m); \
         Run sets the time step: 0.00602 s at the start for Courant 0.5, then adaptive, so the case's solver.dt is \
         overridden at run time."
    );
    assert!(drawn(&r.mesh, &line), "(i) the Mesh side's line");
    // (ii)
    assert!(drawn(&r.hover, RUN_HOVER), "(ii) Run's text");
    assert!(drawn(&r.hover, &time_step::hover(&f, &p80)), "(ii) the plan's hover");
    // (iii)
    assert!(
        drawn(
            &r.after_simulate,
            "overrides the case's solver.dt 0.0625 s and solver.end_time 80 s for this run"
        ),
        "(iii) indicator"
    );
    assert!(drawn(&r.after_simulate, &time_step::label(&p73)), "(iii) label");
    assert!(r.file_same && !r.dirty, "(iii) the case file or the editor changed");
    // (iv)
    assert!(
        drawn(&r.popover, AUTO_BOX) && drawn(&r.popover, "Courant target"),
        "(iv) popover"
    );
    assert!(drawn(&r.warned, "dt 0.0625 s, the case's own"), "(iv) label");
    let w = time_step::warning("cases/cyl.toml", &f, &case).unwrap();
    assert_eq!(
        w,
        "cases/cyl.toml: this run takes the case's own solver.dt = 0.0625 s, an estimated Courant number of 5.184 \
         (1.116 m/s through the thinnest cell, 0.01346 m): above 2, the run may become unstable or noisy. Fix: use the \
         automatic time step (Advanced), or a solver.dt of at most 0.0241 s."
    );
    assert!(
        drawn(&r.warned, &w) && drawn(&r.warned, FIX),
        "(iv) the warning and its fix"
    );
    // (v)
    assert!(drawn(&r.fixed, &time_step::label(&p73)), "(v) the automatic step again");
    assert!(!drawn(&r.fixed, &w), "(v) the warning is gone");
    // (vi)
    assert!(
        drawn(&r.steady, "steady (simplec): until converged, at most 2,000 iterations"),
        "(vi) steady"
    );
    assert!(!drawn(&r.steady, SIMULATE_FOR), "(vi) no Simulate for");
}

/// (f) control: the route before draws no time row, no indicator, no Mesh line and no plan hover.
#[test]
fn f_control_the_route_before_draws_none() {
    let (f, p80, ..) = f_expected();
    let r = f_run("fctl", true);
    assert!(!drawn(&r.row, SIMULATE_FOR), "control: Simulate for");
    assert!(!drawn(&r.row, "overrides the case's solver.dt 0.0625 s for this run"));
    assert!(!drawn(&r.hover, &time_step::hover(&f, &p80)), "control: hover");
    let line = time_step::mesh_line(&f, &time_step::RunTime::default()).unwrap();
    assert!(!drawn(&r.mesh, &line), "control: Mesh line");
}

/// Where the test binaries' run folders are (for the record).
#[allow(dead_code)]
fn temp_root() -> PathBuf {
    common::temp("x")
}
