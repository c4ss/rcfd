//! ADR 0141 G4-G7 and the G1 controls: the one-click path through the drawn app, with `rcfd-runner serve` started
//! directly (ADR 0128 rule) except G7, which needs no runner. G4: a start the runner refuses (missing mesh file) shows
//! the runner's reason within 5 s. G5: a run that fails before its first stored snapshot is explained in the viewport
//! and the strip within 5 s. G6: Pause, Resume and Cancel from the strip, with ADR 0133 R3's limits. G7: F5 and the
//! missing-programs message; control: the pre-change start route. G1 controls: no auto-select, no focus.

mod common;
mod g4;

use g4::{app_on, cavity, click, max_step, project, steps, store_snapshots, MISSING_MESH};
use rcfd_project::RunState;
use rcfd_studio::bins::{Search, BUILD};
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::quick_run::{strip_view, Control};
use rcfd_studio::viewport::Status;
use rcfd_studio::StudioApp;
use std::time::{Duration, Instant};

/// G4, G5: registered time limit (ADR 0133 R3's cancel limit).
const FAIL_LIMIT_S: f64 = 5.0;

fn viewport_text(app: &StudioApp) -> String {
    match &app.st.viewport.status {
        Status::NoStore(m) | Status::Failed(m) => m.clone(),
        s => format!("{s:?}"),
    }
}

// ---------------------------------------------------------------- G4: a refused start

/// Click Run on the missing-mesh case; frames until the strip shows the failed run's reason or the limit passes.
/// Returns (seconds, run id, reason, strip text, viewport text, drawn text, Log in front).
fn g4(tag: &str, quiet: bool) -> (Option<f64>, Option<String>, String, String, String, Vec<String>, bool) {
    let p = project(tag, "missing_mesh.toml", MISSING_MESH, true);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "missing_mesh.toml", &ctx);
    app.st.quick.switches.no_explain = quiet;
    let t0 = click(&mut app, &ctx, "Run");
    let shown = common::frames_until(&mut app, &ctx, Duration::from_secs_f64(FAIL_LIMIT_S), |a| {
        let Some(id) = a.st.selected_run.as_deref() else {
            return false;
        };
        let Some(r) = a.st.runs.row(id) else { return false };
        r.state == RunState::Failed
            && r.reason
                .as_ref()
                .is_some_and(|why| strip_view(&a.st).text().contains(why.as_str()))
    });
    let dt = shown.then(|| t0.elapsed().as_secs_f64());
    if quiet {
        // the control: wait as long as the limit for anything to show, then read what is shown
        let _ = common::frames_until(&mut app, &ctx, Duration::from_secs_f64(FAIL_LIMIT_S), |_| false);
    }
    let id = app.st.selected_run.clone();
    let reason = id
        .as_deref()
        .and_then(|i| app.st.runs.row(i))
        .and_then(|r| r.reason.clone())
        .unwrap_or_default();
    // the viewport's message comes after the store check (a control thread): a few frames more
    let _ = common::frames_until(&mut app, &ctx, Duration::from_secs(2), |a| a.st.quick.notice.is_some());
    let drawn = g4::texts(&common::frame(&mut app, &ctx));
    let log_front = g4::in_front(&app, Panel::Log);
    let strip = strip_view(&app.st).text();
    (dt, id, reason, strip, viewport_text(&app), drawn, log_front)
}

#[test]
fn g4_a_refused_start_shows_the_runners_reason_within_5_s() {
    let (dt, id, reason, strip, vp, drawn, log_front) = g4("g4", false);
    eprintln!("G4: shown after {dt:?} s; run {id:?}\n  reason: {reason}\n  strip: {strip}\n  viewport: {vp}\n  Log in front: {log_front}");
    let dt = dt.unwrap_or_else(|| panic!("the failed start was not shown within {FAIL_LIMIT_S} s: strip {strip}"));
    assert!(dt <= FAIL_LIMIT_S);
    assert!(
        reason.contains("pre-launch check failed") && reason.contains("missing.msh"),
        "{reason}"
    );
    assert!(drawn.iter().any(|t| t.contains(&reason)), "the reason is not drawn");
    assert!(
        vp.contains("failed before writing its first stored snapshot") && vp.contains(&reason),
        "viewport: {vp}"
    );
    assert!(log_front, "the Log panel is not in front after the failure");
}

#[test]
fn g4_control_without_the_explanation_the_reason_is_not_shown() {
    let (dt, id, reason, strip, vp, _, _) = g4("g4ctl", true);
    eprintln!("G4 control: shown {dt:?}; run {id:?}; reason {reason}\n  strip: {strip}\n  viewport: {vp}");
    assert!(dt.is_none(), "the check passed without the explanation");
    assert!(!reason.is_empty(), "the run did not fail at all");
    assert!(!vp.contains(&reason));
}

// ---------------------------------------------------------------- G5: failed before the first snapshot

fn g5(tag: &str, quiet: bool) -> (Option<f64>, String, String, String, Vec<String>, String) {
    // 1e6 steps: the default plan stores every 10 000 steps, about 9 s of solving before the first snapshot
    let p = project(tag, "long.toml", &cavity(1_000_000), true);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "long.toml", &ctx);
    app.st.quick.switches.no_explain = quiet;
    click(&mut app, &ctx, "Run");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(60), |a| a
            .st
            .selected_run
            .as_deref()
            .and_then(|id| a.st.runs.row(id))
            .is_some_and(|r| r.state == RunState::Running)
            && max_step(&a.st) >= 20),
        "the run did not show step 20"
    );
    let id = app.st.selected_run.clone().unwrap();
    let store = p.run_dir(&id).join("fields");
    assert_eq!(store_snapshots(&store), 0, "a snapshot exists before the kill");
    let pid = p.status(&id).and_then(|s| s.pid).expect("solver pid");
    // what is drawn while it runs without a snapshot: the lines that speak of snapshots or stored fields
    let waiting = g4::texts(&common::frame(&mut app, &ctx))
        .into_iter()
        .filter(|t| t.contains("snapshot") || t.contains("stored fields"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rcfd_runner::sys::kill(pid), "kill {pid}");
    let t0 = Instant::now();
    let done = common::frames_until(&mut app, &ctx, Duration::from_secs_f64(FAIL_LIMIT_S), |a| {
        let row = a.st.runs.row(&id);
        row.is_some_and(|r| r.state == RunState::Failed)
            && matches!(&a.st.viewport.status, Status::Failed(m)
                if m.contains("before writing its first stored snapshot")
                    && row.and_then(|r| r.reason.as_deref()).is_some_and(|why| m.contains(why)))
    });
    let dt = done.then(|| t0.elapsed().as_secs_f64());
    if quiet {
        let _ = common::frames_until(&mut app, &ctx, Duration::from_secs_f64(FAIL_LIMIT_S), |_| false);
    }
    let reason = app.st.runs.row(&id).and_then(|r| r.reason.clone()).unwrap_or_default();
    let drawn = g4::texts(&common::frame(&mut app, &ctx));
    (
        dt,
        reason,
        viewport_text(&app),
        strip_view(&app.st).text(),
        drawn,
        waiting,
    )
}

/// The 48C viewport text for a run without a store, which ADR 0141 replaces for the selected run.
const GENERIC: &str = "start the run with \"Store fields\" on";

#[test]
fn g5_a_run_failing_before_its_first_snapshot_is_explained_within_5_s() {
    let (dt, reason, vp, strip, drawn, waiting) = g5("g5", false);
    eprintln!(
        "G5: drawn while running: {waiting}\n  explained after {dt:?} s\n  reason: {reason}\n  viewport: {vp}\n  \
         strip: {strip}"
    );
    let dt = dt.unwrap_or_else(|| panic!("not explained within {FAIL_LIMIT_S} s: viewport {vp}"));
    assert!(dt <= FAIL_LIMIT_S);
    assert!(reason.contains("ended without a finish event"), "{reason}");
    assert!(strip.contains(&reason), "the strip lacks the reason");
    assert!(
        drawn.iter().any(|t| t.contains(&vp)),
        "the viewport message is not drawn"
    );
    assert!(
        !drawn.iter().any(|t| t.contains(GENERIC)),
        "the generic 48C text is drawn"
    );
    assert!(
        waiting.contains("no stored snapshot yet") && !waiting.contains(GENERIC),
        "drawn while running: {waiting}"
    );
}

#[test]
fn g5_control_the_48c_viewport_alone_does_not_explain_it() {
    let (dt, reason, vp, strip, _, _) = g5("g5ctl", true);
    eprintln!("G5 control: explained {dt:?}\n  reason: {reason}\n  viewport: {vp}\n  strip: {strip}");
    assert!(dt.is_none(), "the check passed without the explanation");
    assert!(!vp.contains(&reason));
}

// ---------------------------------------------------------------- G6: Pause, Resume, Cancel from the strip

/// Returns (paused after s, S0, largest step during the hold, resumed step, cancelled after s, controls seen).
fn g6(tag: &str, resume_as_pause: bool) -> (f64, u64, u64, Option<u64>, Option<f64>, Vec<String>) {
    let p = project(tag, "long.toml", &cavity(steps(15_000)), true);
    let ctx = egui::Context::default();
    // ADR 0133 R3's control switch, read by the run worker when it starts
    let mut app = g4::app_with(&p, "long.toml", &ctx, |st| {
        st.run_switches.resume_as_pause = resume_as_pause
    });
    let mut seen = Vec::new();
    let mut note = |app: &StudioApp, when: &str| {
        let v = strip_view(&app.st);
        seen.push(format!(
            "{when}: pause {} resume {} cancel {}",
            v.enabled(Control::Pause),
            v.enabled(Control::Resume),
            v.enabled(Control::Cancel)
        ));
    };
    click(&mut app, &ctx, "Run");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(60), |a| a
            .st
            .selected_run
            .as_deref()
            .and_then(|id| a.st.runs.row(id))
            .is_some_and(|r| r.state == RunState::Running)
            && max_step(&a.st) >= 5),
        "the run did not show step 5"
    );
    let id = app.st.selected_run.clone().unwrap();
    note(&app, "running");
    let t0 = click(&mut app, &ctx, "Pause");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(10), |a| a
            .st
            .runs
            .row(&id)
            .is_some_and(|r| r.state == RunState::Paused)),
        "paused not shown"
    );
    let paused_after = t0.elapsed().as_secs_f64();
    note(&app, "paused");
    let s0 = max_step(&app.st);
    let hold = Instant::now();
    let mut held = s0;
    while hold.elapsed() < Duration::from_secs(2) {
        let _ = common::frame(&mut app, &ctx);
        held = held.max(max_step(&app.st));
        std::thread::sleep(Duration::from_millis(20));
    }
    click(&mut app, &ctx, "Resume");
    let resumed = common::frames_until(&mut app, &ctx, Duration::from_secs(5), |a| max_step(&a.st) > s0)
        .then(|| max_step(&app.st));
    // the control's run is still held: Cancel is enabled for any live run
    let t1 = click(&mut app, &ctx, "Cancel");
    let cancelled = common::frames_until(&mut app, &ctx, Duration::from_secs(10), |a| {
        a.st.runs.row(&id).is_some_and(|r| r.state == RunState::Cancelled)
    })
    .then(|| t1.elapsed().as_secs_f64());
    note(&app, "cancelled");
    (paused_after, s0, held, resumed, cancelled, seen)
}

#[test]
fn g6_pause_resume_cancel_from_the_strip() {
    let (paused_after, s0, held, resumed, cancelled, seen) = g6("g6", false);
    eprintln!(
        "G6: paused shown {paused_after:.3} s after the click at step {s0}; largest step after 2 s held {held}; resumed to \
         {resumed:?}; cancelled shown after {cancelled:?} s; controls {seen:?}"
    );
    assert!(paused_after <= 2.0, "paused shown after {paused_after:.3} s");
    assert_eq!(held, s0, "steps shown while paused");
    assert!(resumed.is_some(), "the run did not move after Resume");
    let c = cancelled.expect("cancelled not shown");
    assert!(c <= 5.0, "cancelled shown after {c:.3} s");
    assert_eq!(
        seen,
        [
            "running: pause true resume false cancel true",
            "paused: pause false resume true cancel true",
            "cancelled: pause false resume false cancel false"
        ]
    );
}

#[test]
fn g6_control_resume_mapped_to_pause_fails_the_step_check() {
    let (_, s0, _, resumed, cancelled, _) = g6("g6ctl", true);
    eprintln!("G6 control: S0 {s0}, after the mapped Resume {resumed:?}; cancelled after {cancelled:?} s");
    assert!(resumed.is_none(), "the run moved although Resume sent pause");
}

// ---------------------------------------------------------------- G7: F5 and missing programs

#[test]
fn g7_f5_with_missing_programs_shows_the_folders_and_the_build_command() {
    let p = project("g7", "cavity2.toml", &cavity(100), false);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "cavity2.toml", &ctx);
    let empty = common::temp("g7-empty");
    let (a, b) = (empty.join("studio"), empty.join("on path"));
    for d in [&a, &b] {
        std::fs::create_dir_all(d).unwrap();
    }
    app.st.quick.search = Search {
        exe_dir: Some(a.clone()),
        path: Some(std::env::join_paths([&b]).unwrap()),
        ..Search::default()
    };
    // control first: frames without the key start nothing
    for _ in 0..20 {
        let _ = common::frame(&mut app, &ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    let idle = (app.st.quick.starting.is_none(), app.st.quick.error.is_none());
    let t0 = Instant::now();
    let _ = g4::key(&mut app, &ctx, egui::Key::F5);
    let started = app.st.quick.starting.is_some() || app.st.quick.error.is_some();
    let shown = common::frames_until(&mut app, &ctx, Duration::from_secs(2), |a| a.st.quick.error.is_some());
    let dt = t0.elapsed().as_secs_f64();
    let drawn = g4::texts(&common::frame(&mut app, &ctx));
    let err = app.st.quick.error.clone().unwrap_or_default();
    eprintln!("G7: without F5 idle {idle:?}; F5 started {started}; error shown {shown} after {dt:.3} s:\n{err}");
    assert_eq!(idle, (true, true), "a start without the key");
    assert!(started && shown && dt <= 2.0);
    for want in [BUILD.to_string(), a.display().to_string(), b.display().to_string()] {
        assert!(drawn.iter().any(|t| t.contains(&want)), "'{want}' is not drawn");
    }
    assert!(p.ids().is_empty(), "a run folder was created: {:?}", p.ids());
}

#[test]
fn g7_control_the_pre_change_route_queues_a_run_and_names_no_folder() {
    let p = project("g7ctl", "cavity2.toml", &cavity(100), false);
    let ctx = egui::Context::default();
    // the Start dialog's route of 48D with the runner the Studio had before ADR 0141 when nothing is next to it
    let missing = common::temp("g7ctl-missing").join(rcfd_studio::bins::exe_name("rcfd-runner"));
    let mut app = g4::app_with(&p, "cavity2.toml", &ctx, |st| st.runner_exe = missing.clone());
    let text = std::fs::read_to_string(p.dir.join("cases/cavity2.toml")).unwrap();
    let plan = rcfd_studio::runs::StartPlan::new("cases/cavity2.toml", &text, &p.dir.join("cases")).unwrap();
    let t0 = Instant::now();
    app.st.start_run(plan).unwrap();
    let said = common::frames_until(&mut app, &ctx, Duration::from_secs(20), |a| {
        a.st.messages.iter().any(|m| m.starts_with("the run was not started"))
    });
    let dt = t0.elapsed().as_secs_f64();
    let msgs: Vec<&String> = app.st.messages.iter().filter(|m| m.contains("not started")).collect();
    let ids = p.ids();
    let states: Vec<_> = ids.iter().map(|i| p.status(i).map(|s| s.state)).collect();
    eprintln!("G7 control: message after {dt:.3} s ({said}): {msgs:?}; run folders {ids:?} {states:?}");
    assert_eq!(
        states,
        [Some(RunState::Queued)],
        "the pre-change route left no queued run"
    );
    assert!(
        msgs.iter().all(|m| !m.contains(BUILD)),
        "the pre-change route named the build command"
    );
}

// ---------------------------------------------------------------- G1 controls (no auto-select, no focus)

#[test]
fn g1_control_without_select_and_focus_no_frame_and_no_front() {
    let p = project("g1ctl", "live.toml", &cavity(steps(3000)), true);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "live.toml", &ctx);
    app.st.quick.switches.no_select = true;
    app.st.quick.switches.no_focus = true;
    layout::hide(&mut app.st.dock, Panel::Viewport);
    g4::put_in_front(&mut app, Panel::Outputs);
    click(&mut app, &ctx, "Run");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(60), |_| p.ids().len() == 1),
        "not queued"
    );
    let id = p.ids()[0].clone();
    let store = p.run_dir(&id).join("fields");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(60), |_| store_snapshots(&store) > 0),
        "no snapshot was written"
    );
    let t = Instant::now();
    let framed = common::frames_until(&mut app, &ctx, Duration::from_secs(5), |a| {
        a.st.viewport.frame.is_some()
    });
    eprintln!(
        "G1 control: frame within 5 s of the first snapshot {framed} ({:.3} s); selected {:?}; viewport open {}, \
         Outputs in front {}",
        t.elapsed().as_secs_f64(),
        app.st.selected_run,
        layout::is_open(&app.st.dock, Panel::Viewport),
        g4::in_front(&app, Panel::Outputs)
    );
    assert!(
        !framed && app.st.selected_run.is_none(),
        "a frame without the auto-select"
    );
    assert!(!layout::is_open(&app.st.dock, Panel::Viewport) && g4::in_front(&app, Panel::Outputs));
    p.client.cancel(&id).ok();
}
