//! ADR 0144 (job 48J) checks without a GPU or a runner: J1 (the Mesh / Run toggle follows case and run selection), J3
//! (an editor burst builds once, after the debounce, from a frame that asks for itself), J4 (a newer edit discards an
//! older build), J5 (an invalid edit keeps the last good mesh, marked stale), J6 (the size limits hold for unsaved
//! text), J7 (the editor's text is the save's bytes and the runner's mesh), J8 (a benchmark descriptor is not a case
//! file), J9 (a case made by the case starter is previewed), J11 (the toggle drawn, disabled with its reason, clicked).

mod common;
mod lj;
mod pv;

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::case_starter::EXAMPLES;
use rcfd_studio::preview::{self, Built, Limits, Refusal, Status};
use rcfd_studio::preview_ui::{STALE, STALE_BADGE};
use rcfd_studio::schema_model::value_at;
use rcfd_studio::view_toggle::{Side, NO_CASE, NO_RUN};
use rcfd_studio::viewport::{surface_of, Status as ViewStatus, StoreView};
use rcfd_studio::{Studio, StudioApp};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- J1, J11: the toggle

/// The B1 fixture project (ADR 0131: cases `a`, `b`, `sub/c`, eight runs, no stores), open in a Studio.
fn b1(tag: &str, runs: bool) -> (std::path::PathBuf, Studio) {
    let root = common::temp(tag).join("proj");
    common::b1_project(&root);
    let root = rcfd_project::paths::canonicalize(&root).unwrap();
    let st = lj::studio(&root, runs);
    (root, st)
}

/// A run of the B1 fixture in `state`.
fn run_in(st: &Studio, state: RunState) -> String {
    st.browser()
        .unwrap()
        .runs
        .iter()
        .find(|r| r.state == state)
        .map(|r| r.id.clone())
        .unwrap_or_else(|| panic!("no {state:?} run in the fixture"))
}

/// The Mesh side shows the preview of `rel`: no run in the viewport, the preview's surface installed.
fn mesh_shown(st: &Studio, rel: &str, what: &str) {
    assert_eq!(st.view.side, Side::Mesh, "{what}: side");
    assert_eq!(st.viewport.run, None, "{what}: a run is shown");
    let b = lj::shown(st);
    assert_eq!(b.rel, rel, "{what}: case");
    let s = st
        .viewport
        .surface
        .as_ref()
        .unwrap_or_else(|| panic!("{what}: nothing installed"));
    assert!(
        Arc::ptr_eq(s, &b.surface),
        "{what}: the viewport does not show the preview"
    );
}

/// The Run side shows run `id`.
fn run_shown(st: &Studio, id: &str, what: &str) {
    assert_eq!(st.view.side, Side::Run, "{what}: side");
    assert_eq!(st.viewport.run.as_deref(), Some(id), "{what}: viewport run");
}

/// Drain until the Mesh side has installed the preview (one drain normally).
fn until_mesh(st: &mut Studio) {
    assert!(st.drain_until(lj::WAIT, |s| s.viewport.run.is_none()
        && s.viewport.surface.is_some()
        && s.preview
            .shown
            .as_ref()
            .is_some_and(|b| Arc::ptr_eq(s.viewport.surface.as_ref().unwrap(), &b.surface))));
}

/// J1: the toggle's state machine. Opening a case shows its mesh; selecting a run (also the one already selected, also
/// by writing `selected_run` directly) shows the run; the toggle switches either way while the run stays selected (the
/// run worker and the strip keep following it); a side with nothing to show is refused with its reason; the later of a
/// direct selection and a case click wins; a run explained by ADR 0141 (failed before its first snapshot) shows the
/// same explanation after a Mesh / Run round trip; closing the project goes back to Mesh with both sides empty.
#[test]
fn j1_the_toggle_follows_case_and_run_selection() {
    let (_root, mut st) = b1("j1", true);
    let (fin, running, queued, failed) = (
        run_in(&st, RunState::Finished),
        run_in(&st, RunState::Running),
        run_in(&st, RunState::Queued),
        run_in(&st, RunState::Failed),
    );
    // (a) nothing open
    assert_eq!(st.view.side, Side::Mesh);
    assert_eq!(st.view_unavailable(Side::Mesh), Some(NO_CASE));
    assert_eq!(st.view_unavailable(Side::Run), Some(NO_RUN));
    assert_eq!(st.set_view(Side::Run), Err(NO_RUN.to_string()));
    assert_eq!(st.set_view(Side::Mesh), Err(NO_CASE.to_string()));
    assert_eq!(st.view.side, Side::Mesh);
    eprintln!("J1 (a): nothing open: Mesh, both sides refused ('{NO_CASE}', '{NO_RUN}')");
    // (b) a case opened
    assert_eq!(lj::open(&mut st, "cases/a.toml"), Status::Ready);
    until_mesh(&mut st);
    mesh_shown(&st, "cases/a.toml", "(b)");
    assert_eq!(st.view_unavailable(Side::Run), Some(NO_RUN), "(b)");
    // (c) a run selected: shown at once
    st.select_run(&fin);
    run_shown(&st, &fin, "(c) at once");
    st.drain();
    assert_eq!(
        st.runs.shown.as_deref(),
        Some(fin.as_str()),
        "(c) the run worker follows it"
    );
    // (d) the toggle to Mesh: the run stays selected, its series, log and strip are kept
    st.set_view(Side::Mesh).unwrap();
    assert_eq!(st.viewport.run, None, "(d) at once");
    until_mesh(&mut st);
    mesh_shown(&st, "cases/a.toml", "(d)");
    assert_eq!(st.selected_run.as_deref(), Some(fin.as_str()), "(d) selection");
    assert_eq!(st.runs.shown.as_deref(), Some(fin.as_str()), "(d) run worker");
    assert_eq!(
        rcfd_studio::quick_run::strip_view(&st).run.as_deref(),
        Some(fin.as_str()),
        "(d) strip"
    );
    // (e) the toggle to Run
    st.set_view(Side::Run).unwrap();
    run_shown(&st, &fin, "(e)");
    // (f) another case clicked: Mesh, the run still selected
    assert_eq!(lj::open(&mut st, "cases/b.toml"), Status::Ready);
    until_mesh(&mut st);
    mesh_shown(&st, "cases/b.toml", "(f)");
    assert_eq!(lj::shown(&st).cells, 192, "(f) b.toml is 12 x 16");
    assert_eq!(st.selected_run.as_deref(), Some(fin.as_str()), "(f) selection");
    // (g) the same run selected again: Run
    st.select_run(&fin);
    run_shown(&st, &fin, "(g)");
    // (h) a selection written directly (as earlier tests do): Run at the next drain
    st.set_view(Side::Mesh).unwrap();
    until_mesh(&mut st);
    st.selected_run = Some(running.clone());
    st.drain();
    run_shown(&st, &running, "(h)");
    // (i) a direct selection, then a case click before any drain: the later action (the case) wins
    st.selected_run = Some(queued.clone());
    st.open_case("cases/a.toml");
    st.drain();
    assert_eq!(st.view.side, Side::Mesh, "(i)");
    until_mesh(&mut st);
    mesh_shown(&st, "cases/a.toml", "(i)");
    eprintln!("J1 (b)-(i): as registered");
    // (j) ADR 0141's explanation of a run that failed before its first snapshot survives a Mesh / Run round trip
    let (before, after) = round_trip(&mut st, &failed, false);
    let want = format!("Run {failed} failed before writing its first stored snapshot: test failure");
    assert!(before.starts_with(&want), "(j) set-up: {before}");
    assert_eq!(after, Some(before.clone()), "(j) after Mesh -> Run");
    eprintln!("J1 (j): '{before}' before and after the round trip");
    // (k) the project closed
    st.close_project();
    assert_eq!(st.view.side, Side::Mesh, "(k)");
    assert_eq!(
        (st.view_unavailable(Side::Mesh), st.view_unavailable(Side::Run)),
        (Some(NO_CASE), Some(NO_RUN)),
        "(k)"
    );
    eprintln!("J1 (k): closed: Mesh, both sides refused");
}

/// Select `id` until the viewport shows ADR 0141's explanation (a `Failed` text), then Mesh and Run again; the text
/// before, and the `Failed` text held for 0.5 s after the round trip (`None`: not within 5 s).
fn round_trip(st: &mut Studio, id: &str, no_reexplain: bool) -> (String, Option<String>) {
    st.quick.switches.no_reexplain = no_reexplain;
    st.select_run(id);
    let failed_text = |s: &Studio| match &s.viewport.status {
        ViewStatus::Failed(t) if s.viewport.run.as_deref() == Some(id) => Some(t.clone()),
        _ => None,
    };
    assert!(
        st.drain_until(Duration::from_secs(5), |s| failed_text(s).is_some()),
        "no explanation for {id}: {:?}",
        st.viewport.status
    );
    let before = failed_text(st).unwrap();
    st.set_view(Side::Mesh).unwrap();
    until_mesh(st);
    st.set_view(Side::Run).unwrap();
    let back = st.drain_until(Duration::from_secs(5), |s| {
        failed_text(s).as_deref() == Some(before.as_str())
    });
    if !back {
        eprintln!("round trip of {id}: the viewport says {:?}", st.viewport.status);
        return (before, None);
    }
    lj::drain_for(st, Duration::from_millis(500));
    (before, failed_text(st))
}

/// J1 controls. ADR 0138's rule (the selected run always wins): after the toggle to Mesh the viewport still shows the
/// run, so the mesh cannot be reached while a run is selected. ADR 0141's route without the new explanation (switch
/// `no_reexplain`): after the round trip the failed run's explanation is not the one before.
#[test]
fn j1_controls_run_always_wins_and_no_reexplain() {
    let (_root, mut st) = b1("j1c", true);
    let (fin, failed) = (run_in(&st, RunState::Finished), run_in(&st, RunState::Failed));
    st.view.switches.run_always_wins = true;
    assert_eq!(lj::open(&mut st, "cases/a.toml"), Status::Ready);
    st.select_run(&fin);
    st.set_view(Side::Mesh).unwrap();
    lj::drain_for(&mut st, Duration::from_millis(200));
    eprintln!(
        "J1 control (run always wins): side {:?}, viewport run {:?}",
        st.view.side, st.viewport.run
    );
    assert_eq!(
        st.viewport.run.as_deref(),
        Some(fin.as_str()),
        "control: the mesh was reached"
    );
    st.view.switches.run_always_wins = false;
    let (before, after) = round_trip(&mut st, &failed, true);
    eprintln!(
        "J1 control (no re-explanation): '{before}' before, {after:?} after; viewport {:?}",
        st.viewport.status
    );
    assert_ne!(
        after,
        Some(before),
        "control: the explanation came back without the change"
    );
}

/// J11: the toggle drawn. With nothing open both buttons are disabled and hovering each draws its reason; a click on a
/// disabled button changes nothing (the refusal); with a case open only Mesh is enabled; with a run selected both are,
/// and clicks on Mesh and Run switch the drawn panel.
#[test]
fn j11_the_toggle_is_drawn_disabled_with_its_reason_and_clicked() {
    let (_root, st) = b1("j11", false);
    let fin = run_in(&st, RunState::Finished);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    for _ in 0..3 {
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    }
    let enabled = |app: &StudioApp| -> BTreeMap<&'static str, bool> { app.st.view.enabled.clone() };
    // (a) nothing open: disabled, each with its reason on hover
    assert_eq!(enabled(&app), BTreeMap::from([("Mesh", false), ("Run", false)]), "(a)");
    for (label, why) in [("Mesh", NO_CASE), ("Run", NO_RUN)] {
        let pos = app.st.view.rects[label].center();
        let _ = lj::frame_with(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        let seen = (0..5).any(|_| {
            lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()))
                .iter()
                .any(|t| t == why)
        });
        assert!(seen, "(a) {label}: '{why}' not drawn on hover");
    }
    let away = egui::Event::PointerMoved(egui::pos2(5.0, 795.0));
    let _ = lj::frame_with(&mut app, &ctx, vec![away.clone()]);
    eprintln!("J11 (a): both disabled; hovering draws '{NO_CASE}' and '{NO_RUN}'");
    // (b) a case open: Mesh only; a click on the disabled Run is refused
    app.st.open_case("cases/a.toml");
    assert!(common::frames_until(&mut app, &ctx, lj::WAIT, |a| lj::settled(&a.st)
        && a.st.viewport.surface.is_some()));
    let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    assert_eq!(enabled(&app), BTreeMap::from([("Mesh", true), ("Run", false)]), "(b)");
    lj::click_toggle(&mut app, &ctx, "Run");
    assert_eq!(app.st.view.side, Side::Mesh, "(b) a click on the disabled Run switched");
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, vec![away.clone()]));
    assert!(drawn.iter().any(|t| t == "Mesh of cases/a.toml"), "(b) the mesh panel");
    // (c) a run selected: both enabled, Run shown
    app.st.select_run(&fin);
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    assert_eq!(enabled(&app), BTreeMap::from([("Mesh", true), ("Run", true)]), "(c)");
    assert_eq!(app.st.view.side, Side::Run, "(c)");
    let caption = format!("stored fields of run {fin}");
    assert!(drawn.contains(&caption), "(c) '{caption}' not drawn");
    // (d) click Mesh, (e) click Run
    lj::click_toggle(&mut app, &ctx, "Mesh");
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, vec![away.clone()]));
    assert_eq!(
        (app.st.view.side, app.st.viewport.run.clone()),
        (Side::Mesh, None),
        "(d)"
    );
    assert!(drawn.iter().any(|t| t == "Mesh of cases/a.toml"), "(d) the mesh panel");
    lj::click_toggle(&mut app, &ctx, "Run");
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, vec![away]));
    assert_eq!(app.st.view.side, Side::Run, "(e)");
    assert_eq!(app.st.viewport.run.as_deref(), Some(fin.as_str()), "(e)");
    assert!(drawn.contains(&caption), "(e) the run caption");
    eprintln!("J11 (b)-(e): Mesh only with a case (a click on Run refused); both with a run; clicks switch the panel");
}

// ---------------------------------------------------------------- J3, J4: debounce and stale results

/// J3: two keystrokes 100 ms apart in `mesh.nx` of the cavity ask for one build, of the editor's text, between 0.50 and
/// 0.75 s after the last one; it shows 20 x 16 cells; 1 s of idle drains asks for nothing more. Negative control: with no
/// debounce the same burst asks for two builds.
#[test]
fn j3_an_editor_burst_builds_once_after_the_debounce() {
    let root = pv::project("j3", &[]);
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    assert_eq!(st.preview.debounce, preview::DEBOUNCE);
    let burst = |st: &mut Studio, a: &str, b: &str| -> Instant {
        lj::type_text(st, "mesh.nx", a);
        lj::drain_for(st, Duration::from_millis(100));
        lj::type_text(st, "mesh.nx", b);
        Instant::now()
    };
    let r0 = (st.preview.stats.requests, st.preview.stats.editor_requests);
    let t_last = burst(&mut st, "2", "20");
    let mut t_req = None;
    while t_last.elapsed() < Duration::from_secs(3) {
        st.drain();
        if st.preview.stats.requests > r0.0 {
            t_req = Some(t_last.elapsed().as_secs_f64());
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let t_req = t_req.expect("no build asked for within 3 s of the last keystroke");
    assert_eq!(lj::settle(&mut st), Status::Ready);
    let b = lj::shown(&st);
    lj::drain_for(&mut st, Duration::from_secs(1));
    let r1 = (st.preview.stats.requests, st.preview.stats.editor_requests);
    eprintln!(
        "J3: build asked {t_req:.3} s after the last keystroke; requests {} (of the editor's text {}); shown {} cells, \
         from the editor {}",
        r1.0 - r0.0,
        r1.1 - r0.1,
        b.cells,
        b.from_editor
    );
    assert!(
        (0.5..=0.75).contains(&t_req),
        "asked {t_req:.3} s after the last keystroke"
    );
    assert_eq!(
        (r1.0 - r0.0, r1.1 - r0.1),
        (1, 1),
        "one build of the editor's text for the burst"
    );
    assert_eq!((b.cells, b.from_editor), (320, true));
    // control: no debounce
    st.preview.debounce = Duration::ZERO;
    let r2 = st.preview.stats.requests;
    let _ = burst(&mut st, "3", "30");
    lj::settle(&mut st);
    let n = st.preview.stats.requests - r2;
    eprintln!("J3 control: no debounce: {n} builds for the burst");
    assert!(n >= 2, "control: {n} build(s) without a debounce");
}

/// J3 (b): the frame drawn after a keystroke asks for the next frame at most the debounce later, and the frame drawn
/// then asks for the build (without any input). Control: an idle frame asks for no frame within 1 s.
#[test]
fn j3b_the_frame_after_a_keystroke_asks_for_the_debounced_frame() {
    let root = pv::project("j3b", &[]);
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let mut idle = Duration::ZERO;
    for _ in 0..30 {
        idle = lj::repaint_delay(&lj::frame_with(&mut app, &ctx, Vec::new()));
    }
    lj::type_text(&mut app.st, "mesh.nx", "24");
    let d = lj::repaint_delay(&lj::frame_with(&mut app, &ctx, Vec::new()));
    let r0 = app.st.preview.stats.requests;
    std::thread::sleep(d.min(Duration::from_secs(2)));
    let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    let r1 = app.st.preview.stats.requests;
    eprintln!(
        "J3 (b): idle frame repaint delay {idle:?}; after a keystroke {d:?}; builds asked by the frame then {}",
        r1 - r0
    );
    assert!(
        idle > Duration::from_secs(1),
        "control/set-up: an idle frame asks for a frame after {idle:?}"
    );
    assert!(
        d <= preview::DEBOUNCE,
        "the frame after the keystroke asks for the next after {d:?}"
    );
    assert_eq!(r1, r0 + 1, "the frame at the asked time did not ask for the build");
}

/// J4: an older build of the editor's text still running when a newer edit's build is asked for is discarded. The
/// 500 x 500 box is opened; `nx = 501` (A, a 250,500-cell build) is typed; 50 ms after A's thread started, `nx = 20` (B,
/// 10,000 cells). Limits: A's result never in `built` or `shown` after B was typed; B shown within 2 s of its request;
/// at least one result discarded; never more than 2 build threads. Set-up: A's thread lives at least 0.5 s and is still
/// running when B is typed. Negative control: with the generation check off A's result is applied after B was typed.
#[test]
fn j4_a_newer_edit_discards_the_older_build() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP j4 (crates/rcfd-studio/tests/live_preview_j.rs): registered for release builds");
        return;
    }
    for control in [false, true] {
        let root = pv::project("j4", &[("cases/big.toml", pv::box_case(500, 500).as_bytes())]);
        let mut st = lj::studio(&root, false);
        st.preview.switches.ignore_generation = control;
        let _ = lj::open(&mut st, "cases/big.toml");
        let e0 = st.preview.stats.editor_requests;
        lj::type_text(&mut st, "mesh.nx", "501");
        assert!(st.drain_until(lj::WAIT, |s| s.preview.stats.editor_requests == e0 + 1
            && s.preview.running() == 1));
        let t_a = Instant::now();
        std::thread::sleep(Duration::from_millis(50));
        st.drain();
        assert_eq!(st.preview.running(), 1, "set-up: A still building when B is typed");
        lj::type_text(&mut st, "mesh.nx", "20");
        let has = |b: &Option<Arc<Built>>, cells: usize| b.as_ref().is_some_and(|b| b.cells == cells);
        let (mut b_req, mut b_shown, mut a_done, mut a_after) = (None::<Instant>, None, None, 0usize);
        let end = Instant::now() + lj::WAIT;
        loop {
            st.drain();
            if b_req.is_none() && st.preview.stats.editor_requests == e0 + 2 {
                b_req = Some(Instant::now());
            }
            if has(&st.preview.built, 501 * 500) || has(&st.preview.shown, 501 * 500) {
                a_after += 1;
            }
            if let (Some(t), None) = (b_req, b_shown) {
                if has(&st.preview.shown, 20 * 500) && st.preview.status == Status::Ready {
                    b_shown = Some(t.elapsed().as_secs_f64());
                }
            }
            if a_done.is_none() && st.preview.running() == 0 {
                a_done = Some(t_a.elapsed().as_secs_f64());
            }
            if b_shown.is_some() && a_done.is_some() {
                std::thread::sleep(Duration::from_millis(20));
                st.drain();
                if has(&st.preview.built, 501 * 500) || has(&st.preview.shown, 501 * 500) {
                    a_after += 1;
                }
                break;
            }
            assert!(
                Instant::now() < end,
                "J4: no outcome (B shown {b_shown:?}, A done {a_done:?})"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let (b_s, a_s) = (b_shown.unwrap(), a_done.unwrap());
        eprintln!(
            "J4{}: B shown {b_s:.3} s after its request; A's thread {a_s:.3} s; A applied after B was typed in {a_after} \
             drains; discarded {}, most threads {}",
            if control { " control" } else { "" },
            st.preview.stats.discarded,
            st.preview.stats.most_running
        );
        assert!(a_s >= 0.5, "set-up: A's thread lived {a_s:.3} s");
        assert!(st.preview.stats.most_running <= 2, "more than 2 build threads");
        if control {
            assert!(a_after > 0, "control: A's stale result was not applied");
        } else {
            assert_eq!(a_after, 0, "A applied after B was typed");
            assert!(b_s <= 2.0, "B shown {b_s:.3} s after its request");
            assert!(st.preview.stats.discarded >= 1, "A's result not discarded");
        }
    }
}

// ---------------------------------------------------------------- J5, J6: invalid text and the limits

/// The shown mesh is still `s0` (same Arc, installed, no new upload) and marked stale.
fn still_shown(st: &Studio, s0: &Arc<Built>, installs: u64, what: &str) {
    assert!(Arc::ptr_eq(&lj::shown(st), s0), "{what}: the shown mesh changed");
    assert!(
        Arc::ptr_eq(st.viewport.surface.as_ref().expect("installed"), &s0.surface),
        "{what}: the viewport changed"
    );
    assert_eq!(st.preview.stats.installs, installs, "{what}: a new upload");
    assert!(st.preview.stale(), "{what}: not marked stale");
}

/// J5: half-typed and invalid text shows the located error and keeps the last good mesh, marked stale (the same Arc,
/// no new upload); the editor's settings and the file stay as they were. (a) `nx = 1e`; (b) `nx = 0` (below the
/// schema's minimum); (c) committed edits to a mesh file that does not exist (refused by `build_mesh`); (d) a valid
/// `nx = 12` builds and is no longer stale. Drawn: (a) the message, the stale line and the badge; (d) neither.
#[test]
fn j5_an_invalid_edit_keeps_the_last_good_mesh_marked_stale() {
    let root = pv::project("j5", &[]);
    let file = root.join("cases/cavity.toml");
    let disk = std::fs::read(&file).unwrap();
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    let s0 = lj::shown(&st);
    let (installs, n0) = (st.preview.stats.installs, st.preview.stats.meshes_built);
    // (a)
    lj::type_text(&mut st, "mesh.nx", "1e");
    lj::settle(&mut st);
    let m = lj::failed(&st);
    assert_eq!(m, "cases/cavity.toml: mesh.nx: '1e' is not a whole number");
    still_shown(&st, &s0, installs, "(a)");
    let line = format!("{STALE}: the current text does not build (above).");
    assert_eq!(
        st.preview.stale_reason(),
        Some("the current text does not build (above)")
    );
    assert_eq!(
        value_at(&st.editor.as_ref().unwrap().settings, "mesh.nx"),
        Some(&json!(16)),
        "(a) the settings changed"
    );
    assert_eq!(std::fs::read(&file).unwrap(), disk, "(a) the file changed");
    assert_eq!(st.preview.stats.meshes_built, n0, "(a) a mesh was built");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    for want in [m.as_str(), line.as_str(), STALE_BADGE] {
        assert!(drawn.iter().any(|t| t == want), "(a) '{want}' not drawn");
    }
    eprintln!("J5 (a): '{m}'; the 256-cell mesh kept (same Arc, no upload); drawn: message, '{line}', badge");
    let mut st = app.st;
    // (b)
    lj::type_text(&mut st, "mesh.nx", "0");
    lj::settle(&mut st);
    let m = lj::failed(&st);
    assert_eq!(m, "cases/cavity.toml: mesh.nx: 0 is below the minimum 1");
    still_shown(&st, &s0, installs, "(b)");
    eprintln!("J5 (b): '{m}'; kept");
    // (c)
    lj::type_text(&mut st, "mesh.nx", "16");
    {
        let ed = st.editor.as_mut().unwrap();
        ed.set_value("mesh.source", json!("file")).unwrap();
        ed.set_value("mesh.file", json!("missing.msh")).unwrap();
    }
    lj::settle(&mut st);
    let m = lj::failed(&st);
    assert!(
        m.starts_with("cases/cavity.toml: mesh.file: ") && m.contains("missing.msh"),
        "(c) {m}"
    );
    assert_eq!(st.preview.stats.meshes_built, n0 + 1, "(c) build_mesh was not reached");
    still_shown(&st, &s0, installs, "(c)");
    eprintln!("J5 (c): '{m}'; kept");
    // (d)
    st.editor
        .as_mut()
        .unwrap()
        .set_value("mesh.source", json!("generator"))
        .unwrap();
    lj::type_text(&mut st, "mesh.nx", "12");
    assert_eq!(lj::settle(&mut st), Status::Ready);
    let b = lj::shown(&st);
    assert_eq!((b.cells, b.from_editor, st.preview.stale()), (192, true, false), "(d)");
    let mut app = StudioApp::new(st, None);
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    assert!(
        !drawn.iter().any(|t| t.starts_with(STALE) || t == STALE_BADGE),
        "(d) still marked stale"
    );
    eprintln!("J5 (d): nx = 12: 192 cells from the editor, not stale, no mark drawn");
}

/// J5 control: ADR 0138's rule (a failed build takes the shown mesh away): after `nx = 1e` nothing is shown.
#[test]
fn j5_control_adr0138_rule_drops_the_mesh() {
    let root = pv::project("j5c", &[]);
    let mut st = lj::studio(&root, false);
    st.preview.switches.drop_on_failure = true;
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    lj::type_text(&mut st, "mesh.nx", "1e");
    lj::settle(&mut st);
    st.drain();
    eprintln!(
        "J5 control: status {:?}; viewport surface {}",
        st.preview.status,
        st.viewport.surface.is_some()
    );
    assert!(st.viewport.surface.is_none(), "control: the mesh stayed");
}

/// J6: the size limits hold for unsaved text. (a) `nx = 100000` on a 20 x 1000 `box_2d` (in the schema's range):
/// refused before building as `about 100,000,000 cells`, within 5 s of its request, no mesh built, the last mesh kept;
/// `nx = 1000000` (over the schema's maximum) is a located field error, no mesh built. (b) `mesh.file` typed to a
/// 20,000-byte file with the file limit at 10,000: refused, no mesh built, kept. (c) `nx = 20` on a 4 x 4 cavity with the
/// triangle limit at 100: 160 triangles, not shown (the 4 x 4 kept); "show anyway" shows it without a rebuild.
#[test]
fn j6_the_size_limits_hold_for_unsaved_text() {
    // (a) cells
    let root = pv::project("j6a", &[("cases/tall.toml", pv::box_case(20, 1000).as_bytes())]);
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/tall.toml"), Status::Ready);
    let t0 = lj::shown(&st);
    let (installs, n0, e0) = (
        st.preview.stats.installs,
        st.preview.stats.meshes_built,
        st.preview.stats.editor_requests,
    );
    lj::type_text(&mut st, "mesh.nx", "100000");
    assert!(st.drain_until(lj::WAIT, |s| s.preview.stats.editor_requests == e0 + 1));
    let t_req = Instant::now();
    lj::settle(&mut st);
    let dt = t_req.elapsed().as_secs_f64();
    match &st.preview.status {
        Status::TooLarge { message, rebuild: true } => {
            assert_eq!(
                message,
                "cases/tall.toml: about 100,000,000 cells (from the mesh.n* counts of generator \"box_2d\"), more than \
                 the preview limit of 2,000,000"
            );
            eprintln!("J6 (a): '{message}' {dt:.3} s after its request");
        }
        other => panic!("(a) {other:?}"),
    }
    assert!(dt <= 5.0, "(a) refused after {dt:.3} s");
    assert_eq!(st.preview.stats.meshes_built, n0, "(a) a mesh was built");
    still_shown(&st, &t0, installs, "(a)");
    lj::type_text(&mut st, "mesh.nx", "1000000");
    lj::settle(&mut st);
    let m = lj::failed(&st);
    assert_eq!(m, "cases/tall.toml: mesh.nx: 1000000 is above the maximum 100000");
    assert_eq!(st.preview.stats.meshes_built, n0, "(a) a mesh was built");
    still_shown(&st, &t0, installs, "(a) over the maximum");
    eprintln!("J6 (a): '{m}'; nothing built, the 20,000 cells kept");
    // (b) file size
    let big = vec![b'x'; 20_000];
    let root = pv::project("j6b", &[("meshes/big.msh", big.as_slice())]);
    let mut st = lj::studio(&root, false);
    st.preview.limits.file_bytes = 10_000;
    assert_eq!(lj::open(&mut st, "cases/hex.toml"), Status::Ready);
    let h0 = lj::shown(&st);
    let (installs, n0) = (st.preview.stats.installs, st.preview.stats.meshes_built);
    lj::type_text(&mut st, "mesh.file", "../meshes/big.msh");
    lj::settle(&mut st);
    match &st.preview.status {
        Status::TooLarge { message, rebuild: true } => {
            assert!(
                message.starts_with("cases/hex.toml: mesh.file ")
                    && message.ends_with("big.msh is 20,000 bytes, more than the preview limit of 10,000"),
                "(b) {message}"
            );
            eprintln!("J6 (b): '{message}'");
        }
        other => panic!("(b) {other:?}"),
    }
    assert_eq!(st.preview.stats.meshes_built, n0, "(b) a mesh was built");
    still_shown(&st, &h0, installs, "(b)");
    // (c) triangles
    let root = pv::project("j6c", &[("cases/c4.toml", lj::cavity(4, 4).as_bytes())]);
    let mut st = lj::studio(&root, false);
    st.preview.limits.triangles = 100;
    assert_eq!(lj::open(&mut st, "cases/c4.toml"), Status::Ready);
    let c0 = lj::shown(&st);
    assert_eq!(c0.triangles, 32, "(c) set-up");
    let installs = st.preview.stats.installs;
    lj::type_text(&mut st, "mesh.nx", "20");
    lj::settle(&mut st);
    match &st.preview.status {
        Status::TooLarge {
            message,
            rebuild: false,
        } => {
            assert_eq!(
                message,
                "cases/c4.toml: the surface has 160 triangles, more than the preview limit of 100 (uploading it pauses \
                 the window)"
            );
            eprintln!("J6 (c): '{message}'");
        }
        other => panic!("(c) {other:?}"),
    }
    still_shown(&st, &c0, installs, "(c)");
    let (req, n) = (st.preview.stats.requests, st.preview.stats.meshes_built);
    st.preview.anyway();
    st.drain();
    let b = lj::shown(&st);
    assert_eq!(
        (st.preview.status.clone(), b.cells),
        (Status::Ready, 80),
        "(c) show anyway"
    );
    assert!(
        Arc::ptr_eq(st.viewport.surface.as_ref().unwrap(), &b.surface),
        "(c) not shown"
    );
    assert_eq!(
        (st.preview.stats.requests, st.preview.stats.meshes_built),
        (req, n),
        "(c) rebuilt"
    );
    eprintln!("J6 (c): shown anyway (80 cells) without a rebuild");
}

/// J6 control: ADR 0138's rule (the file on disk only): the same `nx = 100000` keystroke leaves the file's 20,000 cells
/// ready and asks for no build of the text.
#[test]
fn j6_control_adr0138_rule_builds_the_file_not_the_text() {
    let root = pv::project("j6x", &[("cases/tall.toml", pv::box_case(20, 1000).as_bytes())]);
    let mut st = lj::studio(&root, false);
    st.preview.switches.disk_only = true;
    assert_eq!(lj::open(&mut st, "cases/tall.toml"), Status::Ready);
    lj::type_text(&mut st, "mesh.nx", "100000");
    lj::drain_for(&mut st, Duration::from_millis(1500));
    eprintln!(
        "J6 control: status {:?}, shown {} cells, editor builds {}",
        st.preview.status,
        lj::shown(&st).cells,
        st.preview.stats.editor_requests
    );
    assert_eq!(st.preview.status, Status::Ready, "control: refused");
    assert_eq!(lj::shown(&st).cells, 20_000);
}

// ---------------------------------------------------------------- J7: the text and the runner

/// The mesh of a built preview equals `mesh` (hash, counts, patches, surface bit for bit).
fn assert_same(b: &Built, mesh: &rcfd_mesh::Mesh, what: &str) {
    assert_eq!(b.mesh_hash, mesh.content_hash(), "{what}: mesh hash");
    assert_eq!(
        (b.cells, b.faces, b.points),
        (mesh.n_cells(), mesh.n_faces(), mesh.n_points()),
        "{what}: counts"
    );
    let want: Vec<(String, usize)> = mesh.patches().iter().map(|p| (p.name.clone(), p.size)).collect();
    let got: Vec<(String, usize)> = b.patches.iter().map(|p| (p.name.clone(), p.faces)).collect();
    assert_eq!(got, want, "{what}: patches");
    let (s, two_d) = surface_of(mesh).unwrap();
    pv::same_surface(&b.surface, &s).unwrap_or_else(|e| panic!("{what}: surface: {e}"));
    assert_eq!(b.two_d, two_d, "{what}: 2D");
}

/// J7: the preview of unsaved edits builds the bytes a save writes, parsed as the runner parses them with the case's
/// folder: the hex box (`mesh.file = "../meshes/hex.msh"`, relative to `cases/`) with `mesh.scale = 2` committed and
/// `physics.nu` typed. Limits: the hash of the save's bytes (`CaseEditor::to_bytes` after the typed field is committed)
/// equals the hash the preview built; the mesh equals `Simulation::new` of those bytes parsed against `cases/` (hash,
/// counts, patches, surface bit for bit) and differs from the file's mesh before the edit; after the save, `rcfd run`
/// started as the runner starts it (`case.snapshot`, `--case-dir cases`) exits 0 or 5 and stores that mesh and surface.
/// Negative control: the same bytes parsed with base `.` (the editor's service) do not build.
#[test]
fn j7_the_editors_text_is_the_saved_bytes_and_the_runners_mesh() {
    let (_, rcfd) = common::binaries();
    let root = pv::project("j7", &[]);
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/hex.toml"), Status::Ready);
    let h0 = lj::shown(&st).mesh_hash;
    st.editor.as_mut().unwrap().set_value("mesh.scale", json!(2.0)).unwrap();
    lj::type_text(&mut st, "physics.nu", "0.02");
    assert_eq!(lj::settle(&mut st), Status::Ready);
    let b = lj::shown(&st);
    assert!(b.from_editor, "not built from the editor");
    let schema = st.schema.clone();
    let bytes = {
        let ed = st.editor.as_mut().unwrap();
        ed.edit_text(&schema, "physics.nu", "0.02").unwrap();
        assert!(ed.draft.is_none(), "the committed field is still a draft");
        ed.to_bytes().unwrap()
    };
    assert_eq!(
        hash_bytes(&bytes),
        b.case_hash,
        "the preview did not build the save's bytes"
    );
    let text = std::str::from_utf8(&bytes).unwrap();
    let cases = root.join("cases");
    let case = preview::parse_case(text, &cases).unwrap_or_else(|d| panic!("{d:?}"));
    let sim = Simulation::new(&case).unwrap_or_else(|d| panic!("Simulation::new: {d:?}"));
    assert_same(&b, &sim.geom.mesh, "J7 Simulation::new");
    assert_ne!(b.mesh_hash, h0, "the edit (mesh.scale = 2) did not change the mesh");
    // saved, then run as the runner starts it
    st.save_case().unwrap();
    assert!(st.drain_until(lj::WAIT, |s| s
        .editor
        .as_ref()
        .is_some_and(|e| !e.saving && !e.is_dirty())));
    assert_eq!(
        std::fs::read(root.join("cases/hex.toml")).unwrap(),
        bytes,
        "saved bytes"
    );
    let run = pv::temp("j7-run");
    std::fs::write(run.join("case.snapshot"), &bytes).unwrap();
    let out = std::process::Command::new(&rcfd)
        .arg("run")
        .arg(run.join("case.snapshot"))
        .arg("--case-dir")
        .arg(&cases)
        .arg("--out")
        .arg(run.join("output"))
        .arg("--store-dir")
        .arg(run.join("fields"))
        .args([
            "--set",
            "output.store_interval=1",
            "--set",
            "solver.max_iterations=2",
            "--quiet",
        ])
        .output()
        .unwrap();
    let code = out.status.code();
    assert!(
        matches!(code, Some(0) | Some(5)),
        "rcfd run exit {code:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = StoreView::open(&run.join("fields")).unwrap();
    pv::same_surface(&b.surface, &sv.surface).unwrap_or_else(|e| panic!("run store: {e}"));
    let mut r = rcfd_tsstore::Reader::open(&run.join("fields")).unwrap();
    assert_eq!(r.mesh().unwrap().content_hash(), b.mesh_hash, "the run's mesh");
    let dot = rcfd_api::sim::build_mesh(&Case::parse_toml(text).unwrap());
    eprintln!(
        "J7: the save's bytes {} ({} bytes) = the preview's; mesh {:016x} (before the edit {h0:016x}) = Simulation::new = \
         the run's (exit {code:?}); control: base '.': {}",
        b.case_hash,
        bytes.len(),
        b.mesh_hash,
        dot.as_ref().err().map(|d| d[0].to_string()).unwrap_or_default()
    );
    assert!(dot.is_err(), "control: the bytes built with base '.'");
}

// ---------------------------------------------------------------- J8: benchmark descriptors

/// J8: each shipped benchmark descriptor is refused with one line, `<file>: not a case file: this is a benchmark
/// descriptor (id "<id>") ...`, also through the Studio model (nothing shown). Negative controls: a JSON case with a
/// bad value keeps its located diagnostics; a descriptor whose `reference` key is renamed gets the full diagnostics; the
/// pre-change message of the cavity descriptor (ADR 0138) has at least 7 lines.
#[test]
fn j8_a_benchmark_descriptor_is_not_a_case_file() {
    let repo = pv::repo_root();
    for id in [
        "backward_step",
        "cavity",
        "channel",
        "flat_plate",
        "heated_cavity",
        "naca0012",
    ] {
        let rel = format!("cases/{id}/case.json");
        match preview::build(&repo, &rel, &Limits::default(), false, &|| false).result {
            Err(Refusal::Failed(m)) => {
                assert_eq!(m.lines().count(), 1, "{rel}: {m}");
                assert!(
                    m.starts_with(&format!(
                        "{rel}: not a case file: this is a benchmark descriptor (id \"{id}\")"
                    )),
                    "{m}"
                );
                eprintln!("J8 {m}");
            }
            other => panic!("{rel}: {other:?}"),
        }
    }
    let desc = std::fs::read(repo.join("cases/cavity/case.json")).unwrap();
    let renamed = String::from_utf8(desc.clone())
        .unwrap()
        .replacen("\"reference\"", "\"references\"", 1);
    assert!(renamed.contains("\"references\""), "set-up");
    let bad = br#"{"title": "x", "mesh": {"nx": "abc"}}"#;
    let root = pv::project(
        "j8",
        &[
            ("cases/cavity/case.json", desc.as_slice()),
            ("cases/bad.json", bad.as_slice()),
            ("cases/renamed.json", renamed.as_bytes()),
        ],
    );
    let mut st = pv::studio(&root);
    let failed = |s: Status| match s {
        Status::Failed(m) => m,
        other => panic!("not failed: {other:?}"),
    };
    let m = failed(pv::show_case(&mut st, "cases/cavity/case.json"));
    assert!(
        m.starts_with("cases/cavity/case.json: not a case file: ") && m.lines().count() == 1,
        "{m}"
    );
    assert!(st.viewport.surface.is_none(), "something shown");
    let m = failed(pv::show_case(&mut st, "cases/bad.json"));
    assert!(
        m.starts_with("cases/bad.json: ") && !m.contains("not a case file"),
        "control (a JSON case): {m}"
    );
    eprintln!("J8 control (a JSON case with a bad value): {m}");
    let m = failed(pv::show_case(&mut st, "cases/renamed.json"));
    assert!(
        m.lines().count() >= 2 && !m.contains("not a case file"),
        "control (renamed key): {m}"
    );
    eprintln!("J8 control (reference renamed): {} lines", m.lines().count());
    let text = std::str::from_utf8(&desc).unwrap();
    let d = preview::parse_case(text, &repo.join("cases/cavity")).unwrap_err();
    let old = preview::located("cases/cavity/case.json", &d);
    eprintln!("J8 control (ADR 0138's message): {} lines", old.lines().count());
    assert!(old.lines().count() >= 7, "control: {old}");
}

// ---------------------------------------------------------------- J9: the case starter

/// J9: a case the case starter makes and opens is previewed and the view switches to its mesh. With the cavity open
/// and a run selected (Run side), `create_case_from_example(sod, "first")`: within 30 s the preview's case is
/// `cases/first.toml`, ready, 100 cells, 2D, the mesh of `build_mesh(Case::load(file))` (hash), installed, and the view
/// is Mesh. Negative controls: the starter's route before the change (switch) leaves the preview on the previous case
/// while the editor opens the new one; with unsaved edits the new case is created but neither opened nor previewed,
/// and the view does not move.
#[test]
fn j9_a_case_made_by_the_starter_is_previewed() {
    let root = pv::project("j9", &[]);
    let proj = Project::open(&root).unwrap();
    let rec = proj
        .create_run("cases/cavity.toml", None, "rcfd test", json!({}))
        .unwrap();
    proj.write_status(&rec.id, &RunStatus::new(RunState::Finished)).unwrap();
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    st.select_run(&rec.id);
    run_shown(&st, &rec.id, "set-up");
    let sod = EXAMPLES.iter().find(|e| e.source == "sod/sod.toml").unwrap();
    let t = Instant::now();
    assert_eq!(st.create_case_from_example(sod, "first").unwrap(), "cases/first.toml");
    let opened = |s: &Studio, rel: &str| s.editor.as_ref().is_some_and(|e| e.rel == rel);
    assert!(
        st.drain_until(Duration::from_secs(30), |s| s.preview.case.as_deref()
            == Some("cases/first.toml")
            && lj::settled(s)
            && opened(s, "cases/first.toml")),
        "the new case was not previewed: case {:?}, {:?}",
        st.preview.case,
        st.preview.status
    );
    let dt = t.elapsed().as_secs_f64();
    st.drain();
    let b = lj::shown(&st);
    let want = rcfd_api::sim::build_mesh(&Case::load(root.join("cases/first.toml")).unwrap()).unwrap();
    assert_eq!(st.preview.status, Status::Ready);
    assert_eq!((b.cells, b.two_d, b.mesh_hash), (100, true, want.content_hash()));
    mesh_shown(&st, "cases/first.toml", "J9");
    eprintln!("J9: cases/first.toml previewed {dt:.3} s after the call: 100 cells, 2D, the file's mesh; view Mesh");
    // control 1: the route before the change
    st.view.switches.starter_skips_preview = true;
    st.create_case_from_example(sod, "second").unwrap();
    assert!(st.drain_until(Duration::from_secs(30), |s| opened(s, "cases/second.toml")));
    lj::drain_for(&mut st, Duration::from_millis(200));
    eprintln!(
        "J9 control (before the change): editor cases/second.toml, preview {:?}",
        st.preview.case
    );
    assert_eq!(
        st.preview.case.as_deref(),
        Some("cases/first.toml"),
        "control 1: the new case was previewed"
    );
    st.view.switches.starter_skips_preview = false;
    // control 2: unsaved edits
    st.select_run(&rec.id);
    st.editor.as_mut().unwrap().set_value("mesh.nx", json!(50)).unwrap();
    st.create_case_from_example(sod, "third").unwrap();
    assert!(st.drain_until(Duration::from_secs(30), |s| s
        .last_message()
        .is_some_and(|m| m.starts_with("created cases/third.toml; not opened"))));
    lj::drain_for(&mut st, Duration::from_millis(200));
    eprintln!(
        "J9 control (unsaved edits): preview {:?}, side {:?}",
        st.preview.case, st.view.side
    );
    assert_eq!(
        st.preview.case.as_deref(),
        Some("cases/first.toml"),
        "control 2: previewed"
    );
    assert_eq!(st.view.side, Side::Run, "control 2: the view moved");
}
