//! ADR 0161 (job 48AA): Stop in Run's place on the toolbar, through the drawn app and the real runner (`rcfd-runner
//! serve` started directly, ADR 0128 rule, except AA3, which needs no runner alive). AA1: the shipped cylinder (64 x 48,
//! 80 s) started by Run and stopped by the drawn Stop: "stopped" within 5 s, Run refused while it stops, `status.json`,
//! the events and `metrics.json` of a clean stop, the store closed with the final state as its last snapshot, the
//! stored snapshots viewable, the Runs panel's line; controls: a kill instead of Stop, the route before (no Stop), Run
//! drawn while stopping. AA2: Ctrl+. (and neither Escape nor a plain full stop). AA3: Stop before the run is queued and
//! before the runner runs. AA4: Stop after the run ended. AA5: the "stopped" label (model). Release builds,
//! `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd` of the same build (`common::binaries`).

mod common;
mod g4;
mod ts;

use common::RunnerProject;
use rcfd_project::RunState;
use rcfd_studio::stop::{self, View};
use rcfd_studio::stop_ui::{STOP, STOPPING};
use rcfd_studio::{Studio, StudioApp};
use std::time::{Duration, Instant};

const CASE: &str = "cyl.toml";
const REL: &str = "cases/cyl.toml";
/// AA1 (c), (g), AA2: ADR 0133 R3's cancel limit.
const STOP_LIMIT_S: f64 = 5.0;
/// AA3: ADR 0133 R9's runner start budget (15 s) plus the stop limit.
const AA3_LIMIT_S: f64 = 20.0;
const WAIT: Duration = Duration::from_secs(60);

/// A toolbar button's drawn text.
fn label(s: &str) -> String {
    format!("  {s}  ")
}

fn has(t: &[String], s: &str) -> bool {
    t.iter().any(|x| x == s)
}

fn drawn(app: &mut StudioApp, ctx: &egui::Context) -> Vec<String> {
    g4::texts(&common::frame(app, ctx))
}

/// Ctrl+. (Cmd+. on macOS): press and release in one frame.
fn ctrl_period() -> Vec<egui::Event> {
    let ev = |pressed| egui::Event::Key {
        key: egui::Key::Period,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    vec![ev(true), ev(false)]
}

fn row_label(app: &StudioApp, id: &str) -> Option<String> {
    app.st.runs.row(id).map(|r| r.state_label())
}

/// The served (or not) project with the shipped cylinder as `cases/cyl.toml`, and the drawn app on it (tooltips at once).
fn cylinder_app(tag: &str, serve: bool, set: impl FnOnce(&mut Studio)) -> (RunnerProject, StudioApp, egui::Context) {
    let p = g4::project(tag, CASE, &ts::cylinder(64, 48), serve);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let app = g4::app_with(&p, CASE, &ctx, set);
    (p, app, ctx)
}

/// Click Run and wait until the selected run is shown running with at least `snaps` stored snapshots; its id.
fn running(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject, snaps: usize) -> String {
    g4::click(app, ctx, "Run");
    let ok = common::frames_until(app, ctx, WAIT, |a| {
        a.st.selected_run.as_deref().is_some_and(|id| {
            a.st.runs.row(id).is_some_and(|r| r.state == RunState::Running)
                && g4::store_snapshots(&p.run_dir(id).join("fields")) >= snaps
        })
    });
    assert!(
        ok,
        "the run was not shown running with {snaps} stored snapshots within {WAIT:?}"
    );
    app.st.selected_run.clone().unwrap()
}

/// How the run is ended.
#[derive(Debug, Clone, Copy, PartialEq)]
enum How {
    /// The drawn Stop.
    Click,
    /// Ctrl+.
    Shortcut,
    /// Control: the solver's process killed (G5's route).
    Kill,
}

/// What a stop measured (printed whole).
#[derive(Debug)]
#[allow(dead_code)]
struct Stopped {
    id: String,
    snaps_before: usize,
    shown_s: Option<f64>,
    between: usize,
    between_bad: Vec<String>,
    state: RunState,
    step: u64,
    time_bits: u64,
    exit_code: Option<i32>,
    reason: String,
    pid: Option<u32>,
    cancel_requested: usize,
    killed: usize,
    solver_finish: String,
    footer: Option<bool>,
    store_count: usize,
    store_last: Option<(u64, u64)>,
    residual_last: Option<u64>,
    viewable_s: Option<f64>,
    viewport_snaps: usize,
    runs_line: Option<String>,
    new_messages: Vec<String>,
}

fn count(p: &RunnerProject, id: &str, kind: &str) -> usize {
    p.events(id)
        .iter()
        .filter(|e| e["kind"] == kind || e["event"] == kind)
        .count()
}

/// End the running run `id` by `how` and measure everything AA1 registers.
fn end_run(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject, id: &str, how: How) -> Stopped {
    let dir = p.run_dir(id);
    let snaps_before = g4::store_snapshots(&dir.join("fields"));
    let before = app.st.messages.len();
    let t0 = match how {
        How::Click => g4::click(app, ctx, STOP),
        How::Shortcut => {
            let _ = g4::frame_with(app, ctx, ctrl_period());
            Instant::now()
        }
        How::Kill => {
            let pid = p.status(id).and_then(|s| s.pid).expect("solver pid");
            assert!(rcfd_runner::sys::kill(pid), "kill {pid}");
            Instant::now()
        }
    };
    let (mut between, mut bad, mut shown_s) = (0usize, Vec::new(), None);
    let stopping = label(STOPPING);
    let run = label("Run");
    loop {
        let t = drawn(app, ctx);
        if row_label(app, id).as_deref() == Some(stop::STOPPED) && has(&t, &run) {
            shown_s = Some(t0.elapsed().as_secs_f64());
            break;
        }
        between += 1;
        let refused = stop::run_refusal(&app.st).is_some();
        if has(&t, &run) || !has(&t, &stopping) || !refused {
            bad.push(format!(
                "frame {between}: Run drawn {}, Stopping drawn {}, F5 refused {refused}, row {:?}",
                has(&t, &run),
                has(&t, &stopping),
                row_label(app, id)
            ));
        }
        if t0.elapsed().as_secs_f64() > STOP_LIMIT_S {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // the run's end on disk (a control's run may take longer than the limit)
    let ended = common::frames_until(app, ctx, Duration::from_secs(30), |_| {
        p.status(id).is_some_and(|s| s.state.is_terminal())
    });
    assert!(ended, "run {id} did not end");
    let s = p.status(id).unwrap();
    let metrics: serde_json::Value = std::fs::read(dir.join("metrics.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let store = dir.join("fields");
    let reader = rcfd_tsstore::Reader::open(&store).ok();
    let footer = reader
        .as_ref()
        .map(|r| *r.index_source() == rcfd_tsstore::IndexSource::Footer);
    let store_count = reader.as_ref().map_or(0, |r| r.snapshots().len());
    let store_last = reader
        .as_ref()
        .and_then(|r| r.snapshots().last().map(|x| (x.step, x.time.to_bits())));
    let residual_last = std::fs::read_to_string(dir.join("residuals.jsonl"))
        .ok()
        .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(String::from))
        .and_then(|l| serde_json::from_str::<rcfd_project::records::ResidualLine>(&l).ok())
        .map(|r| r.step);
    // the stored snapshots in the viewport, and the Runs panel's line
    let tv = Instant::now();
    let viewable = common::frames_until(app, ctx, Duration::from_secs_f64(STOP_LIMIT_S), |a| {
        let vp = &a.st.viewport;
        vp.run.as_deref() == Some(id) && store_count > 0 && vp.snaps.len() == store_count && vp.frame.is_some()
    });
    let viewable_s = viewable.then(|| tv.elapsed().as_secs_f64());
    let viewport_snaps = app.st.viewport.snaps.len();
    let runs_line = drawn(app, ctx)
        .into_iter()
        .find(|l| l.starts_with(id) && !l.contains('\n'));
    let new_messages = app.st.messages.iter().skip(before).cloned().collect();
    Stopped {
        id: id.to_string(),
        snaps_before,
        shown_s,
        between,
        between_bad: bad,
        state: s.state,
        step: s.step,
        time_bits: s.time.to_bits(),
        exit_code: s.exit_code,
        reason: s.reason.clone().unwrap_or_default(),
        pid: s.pid,
        cancel_requested: count(p, id, "cancel_requested"),
        killed: count(p, id, "killed"),
        solver_finish: metrics["solver_finish"]["state"].as_str().unwrap_or("").to_string(),
        footer,
        store_count,
        store_last,
        residual_last,
        viewable_s,
        viewport_snaps,
        runs_line,
        new_messages,
    }
}

/// AA1 (c) to (h), and AA2's (c), (e), (f).
fn assert_clean_stop(s: &Stopped, frames: bool) {
    let shown = s
        .shown_s
        .unwrap_or_else(|| panic!("(c) 'stopped' and Run not shown within {STOP_LIMIT_S} s"));
    assert!(shown <= STOP_LIMIT_S, "(c) {shown:.3} s");
    if frames {
        assert!(s.between >= 1, "(d) no frame between the click and the stopped row");
        assert!(s.between_bad.is_empty(), "(d) {:?}", s.between_bad);
    }
    assert_eq!(s.state, RunState::Cancelled, "(e) state");
    assert_eq!(s.exit_code, Some(stop::INTERRUPTED), "(e) exit code");
    assert!(s.step >= 1, "(e) step");
    assert_eq!(
        s.reason,
        format!("cancelled on request; the solver stopped cleanly after step {}", s.step),
        "(e) reason"
    );
    assert_eq!(s.pid, None, "(e) pid");
    assert_eq!((s.cancel_requested, s.killed), (1, 0), "(e) events");
    assert_eq!(s.solver_finish, "interrupted", "(e) metrics.json solver_finish");
    assert_eq!(s.footer, Some(true), "(f) the store is not closed with its footer");
    assert!(s.store_count >= 3, "(f) {} snapshots", s.store_count);
    assert_eq!(
        s.store_last,
        Some((s.step, s.time_bits)),
        "(f) last snapshot vs status.json"
    );
    assert_eq!(s.residual_last, Some(s.step), "(f) residuals.jsonl");
}

fn assert_viewable_and_said(s: &Stopped, app: &StudioApp) {
    let v = s
        .viewable_s
        .unwrap_or_else(|| panic!("(g) not viewable: {} of {} snapshots", s.viewport_snaps, s.store_count));
    assert!(v <= STOP_LIMIT_S, "(g) {v:.3} s");
    let line = s
        .runs_line
        .as_deref()
        .unwrap_or_else(|| panic!("(g) the Runs panel draws no line of {}", s.id));
    assert!(line.contains(stop::STOPPED), "(g) Runs panel line: {line}");
    let asked = s.new_messages.iter().filter(|m| m.starts_with(stop::ASKED)).count();
    assert_eq!(asked, 1, "(h) {:?}", s.new_messages);
    assert!(
        !s.new_messages
            .iter()
            .any(|m| m.starts_with(stop::FAILED) || m.starts_with("cancel run")),
        "(h) {:?}",
        s.new_messages
    );
    assert!(app.st.quick.error.is_none(), "(h) {:?}", app.st.quick.error);
}

// ---------------------------------------------------------------- AA1: the drawn Stop

/// AA1 (a), (b) before the stop: what the toolbar draws while the run runs, Stop's hover, F5 refused.
#[derive(Debug)]
#[allow(dead_code)]
struct Before {
    stop_drawn: bool,
    run_drawn: bool,
    hover: String,
    hover_drawn: bool,
    f5_message: Option<String>,
    f5_refusal: Option<String>,
    runs_after_f5: usize,
}

fn before_stop(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject) -> Before {
    let t = drawn(app, ctx);
    let (stop_drawn, run_drawn) = (has(&t, &label(STOP)), has(&t, &label("Run")));
    let hover = stop::hover(&app.st, &stop::view(&app.st));
    let pos = app.st.quick.rects[STOP].center();
    let mut seen = g4::texts(&g4::frame_with(app, ctx, vec![egui::Event::PointerMoved(pos)]));
    for _ in 0..30 {
        if has(&seen, &hover) {
            break;
        }
        seen.extend(drawn(app, ctx));
    }
    let hover_drawn = has(&seen, &hover);
    let _ = g4::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
    let f5_refusal = stop::run_refusal(&app.st);
    let _ = g4::key(app, ctx, egui::Key::F5);
    let f5_message = app.st.last_message().map(String::from);
    let _ = common::frames_until(app, ctx, Duration::from_secs(1), |_| false);
    Before {
        stop_drawn,
        run_drawn,
        hover,
        hover_drawn,
        f5_message,
        f5_refusal,
        runs_after_f5: p.ids().len(),
    }
}

#[test]
fn aa1_stop_by_the_button_stops_cleanly_with_the_final_snapshot() {
    let (p, mut app, ctx) = cylinder_app("aa1", true, |_| {});
    let id = running(&mut app, &ctx, &p, 2);
    let b = before_stop(&mut app, &ctx, &p);
    eprintln!("AA1 before: {b:#?}");
    let s = end_run(&mut app, &ctx, &p, &id, How::Click);
    eprintln!("AA1: {s:#?}");
    assert!(
        b.stop_drawn && !b.run_drawn,
        "(a) toolbar while running: Stop {} Run {}",
        b.stop_drawn,
        b.run_drawn
    );
    assert!(b.hover_drawn, "(a) hover not drawn: {}", b.hover);
    for w in ["last snapshot", "\"stopped\"", "Ctrl+."] {
        assert!(b.hover.contains(w), "(a) hover lacks {w}: {}", b.hover);
    }
    assert!(
        b.f5_refusal.is_some() && b.f5_message == b.f5_refusal,
        "(b) {:?} / {:?}",
        b.f5_message,
        b.f5_refusal
    );
    assert_eq!(b.runs_after_f5, 1, "(b) F5 started a run");
    assert_clean_stop(&s, true);
    assert_viewable_and_said(&s, &app);
}

/// AA1 control C1: a kill instead of Stop: no "stopped" within the limit and the final-snapshot check fails.
#[test]
fn aa1_control_a_kill_is_not_a_clean_stop() {
    let (p, mut app, ctx) = cylinder_app("aa1kill", true, |_| {});
    let id = running(&mut app, &ctx, &p, 2);
    let s = end_run(&mut app, &ctx, &p, &id, How::Kill);
    eprintln!("AA1 control (kill): {s:#?}");
    assert!(s.shown_s.is_none(), "control: 'stopped' shown after a kill");
    assert!(
        s.footer != Some(true) || s.store_last.map(|l| l.0) != Some(s.step),
        "control: the store looks like a clean stop's"
    );
}

/// AA1 control C2: the route before (no Stop): Run stays and its click queues a second run while the first runs.
#[test]
fn aa1_control_the_route_before_has_no_stop() {
    let (p, mut app, ctx) = cylinder_app("aa1before", true, |st| st.stop.switches.no_stop = true);
    let id = running(&mut app, &ctx, &p, 1);
    let t = drawn(&mut app, &ctx);
    let (stop_drawn, run_drawn) = (has(&t, &label(STOP)), has(&t, &label("Run")));
    g4::click(&mut app, &ctx, "Run");
    let second = common::frames_until(&mut app, &ctx, Duration::from_secs(10), |_| p.ids().len() == 2);
    let first = p.status(&id).map(|s| s.state);
    eprintln!("AA1 control (route before): Stop drawn {stop_drawn}, Run drawn {run_drawn}; second run {second}; first {first:?}");
    assert!(!stop_drawn && run_drawn, "control: (a) holds without Stop");
    assert!(
        second && first == Some(RunState::Running),
        "control: the click did not queue a second run"
    );
}

/// AA1 control C3: Run drawn while the run stops: (d) fails.
#[test]
fn aa1_control_run_drawn_while_stopping_fails_d() {
    let (p, mut app, ctx) = cylinder_app("aa1rws", true, |st| st.stop.switches.run_while_stopping = true);
    let id = running(&mut app, &ctx, &p, 2);
    let s = end_run(&mut app, &ctx, &p, &id, How::Click);
    eprintln!(
        "AA1 control (Run while stopping): between {}, bad {}",
        s.between,
        s.between_bad.len()
    );
    assert!(
        !s.between_bad.is_empty(),
        "control: (d) held with Run drawn while stopping"
    );
}

// ---------------------------------------------------------------- AA2: Ctrl+.

#[test]
fn aa2_ctrl_period_stops_and_escape_or_a_full_stop_do_not() {
    let (p, mut app, ctx) = cylinder_app("aa2", true, |_| {});
    let id = running(&mut app, &ctx, &p, 2);
    let key = |k: egui::Key, pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = g4::frame_with(
        &mut app,
        &ctx,
        vec![key(egui::Key::Escape, true), key(egui::Key::Escape, false)],
    );
    let _ = g4::frame_with(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::Period, true),
            egui::Event::Text(".".into()),
            key(egui::Key::Period, false),
        ],
    );
    let _ = common::frames_until(&mut app, &ctx, Duration::from_secs(2), |_| false);
    let still = p.status(&id).map(|s| s.state);
    let cancel_file = p.run_dir(&id).join("cancel").exists();
    eprintln!("AA2: after Escape and '.', 2 s: {still:?}, cancel file {cancel_file}");
    assert_eq!(still, Some(RunState::Running), "Escape or '.' ended the run");
    assert!(!cancel_file, "Escape or '.' asked for a stop");
    let s = end_run(&mut app, &ctx, &p, &id, How::Shortcut);
    eprintln!("AA2: {s:#?}");
    assert_clean_stop(&s, false);
}

// ---------------------------------------------------------------- AA3: before the run is queued, no runner alive

#[test]
fn aa3_stop_before_the_run_is_queued_and_before_the_runner_runs() {
    let (p, mut app, ctx) = cylinder_app("aa3", false, |_| {});
    assert!(!p.client.runner_running(), "a runner is alive before the click");
    let before = app.st.messages.len();
    g4::click(&mut app, &ctx, "Run");
    let (mut preparing, mut prep_bad) = (0usize, Vec::new());
    let deadline = Instant::now() + WAIT;
    let (id, runner_then, queued_then) = loop {
        assert!(Instant::now() < deadline, "the run was not queued");
        match stop::view(&app.st) {
            View::Preparing { .. } => {
                preparing += 1;
                let n = app.st.messages.len();
                if stop::stop(&mut app.st) {
                    prep_bad.push("stop() asked".to_string());
                }
                let _ = g4::frame_with(&mut app, &ctx, ctrl_period());
                if app.st.messages.len() != n || !app.st.stop.stopping.is_empty() {
                    prep_bad.push(format!("Ctrl+. or stop() acted: {:?}", app.st.messages.last()));
                }
            }
            View::Live { id, queued, .. } => break (id, p.client.runner_running(), queued),
            v => {
                let _ = v;
                let _ = drawn(&mut app, &ctx);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let t0 = g4::click(&mut app, &ctx, STOP);
    let ended = common::frames_until(&mut app, &ctx, Duration::from_secs_f64(AA3_LIMIT_S), |a| {
        a.st.runs.row(&id).is_some_and(|r| r.state.is_terminal())
    });
    let dt = t0.elapsed().as_secs_f64();
    let t = drawn(&mut app, &ctx);
    let row = app.st.runs.row(&id).cloned();
    let launched = p.launched(&id).len();
    let msgs: Vec<String> = app.st.messages.iter().skip(before).cloned().collect();
    eprintln!(
        "AA3: {preparing} frames before the run was queued ({prep_bad:?}); Stop clicked with the runner running \
         {runner_then}, queued {queued_then}; ended {ended} after {dt:.3} s: {:?}; launched {launched}; Run drawn {}; \
         messages {msgs:?}",
        row.as_ref().map(|r| (r.state_label(), r.reason.clone())),
        has(&t, &label("Run"))
    );
    assert!(prep_bad.is_empty(), "{prep_bad:?}");
    assert!(ended && dt <= AA3_LIMIT_S, "not ended within {AA3_LIMIT_S} s");
    let row = row.unwrap();
    assert_eq!(row.state, RunState::Cancelled);
    if launched == 0 {
        assert_eq!(row.reason.as_deref(), Some("cancelled before launch"));
        assert_eq!(row.state_label(), "cancelled");
    } else {
        assert_eq!(row.state_label(), stop::STOPPED);
    }
    assert!(app.st.quick.error.is_none(), "{:?}", app.st.quick.error);
    assert!(
        !msgs
            .iter()
            .any(|m| m.starts_with(stop::FAILED) || m.starts_with("cancel run")),
        "{msgs:?}"
    );
    assert!(has(&t, &label("Run")), "Run is not drawn again");
}

// ---------------------------------------------------------------- AA4: after the run ended

#[test]
fn aa4_stop_after_the_run_ended_is_a_no_op() {
    let (p, mut app, ctx) = cylinder_app("aa4", true, |_| {});
    rcfd_studio::quick_run::run_time_mut(&mut app.st, REL).simulate = Some(0.5);
    g4::click(&mut app, &ctx, "Run");
    let ok = common::frames_until(&mut app, &ctx, WAIT, |a| {
        a.st.selected_run
            .as_deref()
            .and_then(|id| a.st.runs.row(id))
            .is_some_and(|r| r.state == RunState::Finished)
    });
    assert!(ok, "the 0.5 s run did not finish");
    let id = app.st.selected_run.clone().unwrap();
    let _ = common::frames_until(&mut app, &ctx, Duration::from_millis(500), |_| false);
    let status = std::fs::read(p.run_dir(&id).join("status.json")).unwrap();
    let msgs = app.st.messages.clone();
    let view = stop::view(&app.st);
    let asked = stop::stop(&mut app.st);
    let _ = g4::frame_with(&mut app, &ctx, ctrl_period());
    let _ = common::frames_until(&mut app, &ctx, Duration::from_secs(1), |_| false);
    let t = drawn(&mut app, &ctx);
    let same = std::fs::read(p.run_dir(&id).join("status.json")).unwrap() == status;
    let cancel_file = p.run_dir(&id).join("cancel").exists();
    eprintln!(
        "AA4: view {view:?}, stop() asked {asked}, messages unchanged {}, status.json unchanged {same}, cancel file \
         {cancel_file}, Run drawn {}, Stop drawn {}",
        app.st.messages == msgs,
        has(&t, &label("Run")),
        has(&t, &label(STOP))
    );
    assert_eq!(view, View::Run);
    assert!(!asked && app.st.messages == msgs && same && !cancel_file);
    assert!(has(&t, &label("Run")) && !has(&t, &label(STOP)));
}

// ---------------------------------------------------------------- AA5: the label (model)

#[test]
fn aa5_stopped_is_a_cancelled_run_with_the_solvers_exit_code_7() {
    let row = |state, exit_code, held| rcfd_studio::runs::RunRow {
        id: "r-20261029T120000-aa05".into(),
        case: REL.into(),
        created: String::new(),
        state,
        step: 10,
        time: 1.0,
        pid: None,
        reason: None,
        priority: 0,
        threads: 1,
        held,
        queue_pos: None,
        last_residuals: Default::default(),
        exit_code,
    };
    let cases = [
        (RunState::Cancelled, Some(7), false, "stopped"),
        (RunState::Cancelled, None, false, "cancelled"),
        (RunState::Cancelled, Some(1), false, "cancelled"),
        (RunState::Failed, Some(7), false, "failed"),
        (RunState::Finished, Some(0), false, "finished"),
        (RunState::Queued, None, true, "held"),
        (RunState::Running, None, false, "running"),
    ];
    for (s, c, h, want) in cases {
        assert_eq!(row(s, c, h).state_label(), want, "{s:?} {c:?} held {h}");
    }
    let mut st = Studio::in_memory();
    assert_eq!(stop::view(&st), View::Run);
    assert!(
        !stop::stop(&mut st) && st.messages.is_empty(),
        "no project: stop() acted"
    );
}
