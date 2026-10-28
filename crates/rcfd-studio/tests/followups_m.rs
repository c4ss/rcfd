//! ADR 0147 (job 48M) checks without a running runner: M1 (Run is greyed out while a field of the case editor is being
//! typed), M2 (a rebuild of the same case keeps the camera while the bounds stay, refits when they change), M3 (a build
//! of a text identical to the shown mesh's is skipped, counted), M5 (runs ordered by `created`, then id, with runs of one
//! second), M6 (the status bar's one-line message for a benchmark descriptor) and M7 (a) and (c) (the runner's log tail
//! once in a failed start's text).

mod common;
mod lj;
mod pv;

use rcfd_project::{Project, RunState, RunStatus};
use rcfd_render::{Camera, Projection};
use rcfd_studio::preview::{self, Source, Status};
use rcfd_studio::project_view::{self, Browser};
use rcfd_studio::quick_run::{self, FINISH_EDITING};
use rcfd_studio::view_toggle::Side;
use rcfd_studio::viewport::{clip_planes, fit, CameraState, Input};
use rcfd_studio::{Studio, StudioApp};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

// ---------------------------------------------------------------- M1: Run while a field is being typed

/// The Run button's hover text when it is enabled (`quick_run_ui::toolbar`).
const RUN_HOVER: &str = "start the selected case with the default plan and follow it in the viewport (F5)";

/// The drawn texts of the frames with the pointer on the Run button (tooltip delay 0) until one of `want` is drawn or 30
/// frames passed (amendment 1: egui shows a first tooltip once the pointer is still, about 6 headless frames after it
/// moved); the pointer stays there.
fn hover_run_for(app: &mut StudioApp, ctx: &egui::Context, want: &[&str]) -> Vec<String> {
    let _ = lj::frame_with(app, ctx, Vec::new());
    let pos = app.st.quick.rects["Run"].center();
    let mut seen = lj::texts(&lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(pos)]));
    for _ in 0..30 {
        if seen.iter().any(|t| want.contains(&t.as_str())) {
            break;
        }
        seen.extend(lj::texts(&lj::frame_with(app, ctx, Vec::new())));
    }
    seen
}

/// [`hover_run_for`] waiting for either of Run's hover texts: the start text or the reason it is greyed out.
fn hover_run(app: &mut StudioApp, ctx: &egui::Context) -> Vec<String> {
    let why = quick_run::blocked(&app.st);
    let mut want = vec![RUN_HOVER];
    if let Some(w) = &why {
        want.push(w.as_str());
    }
    hover_run_for(app, ctx, &want)
}

/// Move the pointer away from the toolbar (no click).
fn pointer_away(app: &mut StudioApp, ctx: &egui::Context) {
    let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
}

/// Click into the `mesh.nx` field (the cursor goes after its text) and type `keys` (one key or text per frame).
fn type_into_nx(app: &mut StudioApp, ctx: &egui::Context, keys: &[&str]) {
    let _ = lj::frame_with(app, ctx, Vec::new());
    let field = *app
        .ui_state
        .field_rects
        .get("mesh.nx")
        .expect("set-up: the mesh.nx field was not drawn");
    lj::click_at(app, ctx, field.right_center() - egui::vec2(6.0, 0.0));
    for k in keys {
        let ev = match *k {
            "BS" => lj::key(egui::Key::Backspace),
            "ENTER" => lj::key(egui::Key::Enter),
            t => vec![egui::Event::Text(t.to_string())],
        };
        let _ = lj::frame_with(app, ctx, ev);
    }
    let _ = lj::frame_with(app, ctx, Vec::new());
}

/// A key pressed and released in one frame (not typed into any field).
fn press(app: &mut StudioApp, ctx: &egui::Context, k: egui::Key) {
    let _ = lj::frame_with(app, ctx, lj::key(k));
}

/// M1 limit (g): wherever Run may start, the preview's next build is of the case file on disk (what a run uses).
fn run_uses_what_is_shown(st: &Studio, what: &str, seen: &mut usize) {
    if quick_run::blocked(st).is_none() {
        assert_eq!(
            preview::current_source(st),
            Source::Disk,
            "{what}: Run is enabled while the preview builds the editor's text"
        );
        *seen += 1;
    }
}

/// The cavity open in the drawn app (watcher and run worker off), its editor filtered to `mesh.nx`, tooltips at once.
fn m1_app(tag: &str, run_while_typing: bool) -> (StudioApp, egui::Context) {
    let root = pv::project(tag, &[]);
    let mut st = lj::studio(&root, false);
    st.quick.switches.run_while_typing = run_while_typing;
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    let mut app = StudioApp::new(st, None);
    app.ui_state.filter = "mesh.nx".into();
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    for _ in 0..3 {
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    }
    (app, ctx)
}

/// M1: the Run button's state machine with the case editor's field, through the drawn app (egui events into the
/// `mesh.nx` field of the cavity). (a) Opened, clean: Run enabled. (b) `20` typed, the field still being edited: Run
/// greyed out, its hover text `cases/cavity.toml: finish editing the field mesh.nx ...`; F5 starts nothing and the status
/// bar says why; a click on Run starts nothing. (c) The field committed (Enter, or the click before took the focus): the
/// settings hold 20, no field is being typed, Run refused with ADR 0141's "save it first". (d) Saved: enabled. (e) A
/// digit typed and deleted again (the stored text back), then Enter: greyed out while typed, enabled after, nothing
/// committed. (f) `1e` typed and Enter (refused): the field stays being edited, Run greyed out; corrected to `20` and
/// Enter: enabled. (g) In every state where Run is enabled the preview's next build is of the file on disk. Negative
/// control: the route before (`run_while_typing`): in (b)'s state Run is enabled, its hover is the start text, and the
/// preview builds the editor's text.
#[test]
fn m1_run_is_greyed_out_while_a_field_is_typed() {
    let (mut app, ctx) = m1_app("m1", false);
    let mut enabled_states = 0;
    // (a)
    assert_eq!(quick_run::blocked(&app.st), None, "(a)");
    assert!(hover_run(&mut app, &ctx).iter().any(|t| t == RUN_HOVER), "(a) hover");
    pointer_away(&mut app, &ctx);
    run_uses_what_is_shown(&app.st, "(a)", &mut enabled_states);
    // (b)
    type_into_nx(&mut app, &ctx, &["BS", "BS", "2", "0"]);
    assert_eq!(
        app.st.editor.as_ref().unwrap().draft,
        Some(("mesh.nx".to_string(), "20".to_string())),
        "set-up (b): the keystrokes did not reach the field"
    );
    let why = quick_run::blocked(&app.st).expect("(b) Run not greyed out while the field is typed");
    assert!(
        why.starts_with(&format!("cases/cavity.toml: {FINISH_EDITING} mesh.nx")),
        "(b) {why}"
    );
    let drawn = hover_run(&mut app, &ctx);
    assert!(drawn.contains(&why), "(b) the reason not drawn on hover");
    pointer_away(&mut app, &ctx);
    press(&mut app, &ctx, egui::Key::F5);
    assert!(app.st.quick.starting.is_none(), "(b) F5 started");
    assert_eq!(
        app.st.last_message(),
        Some(format!("Run: {why}").as_str()),
        "(b) F5's refusal"
    );
    click_run(&mut app, &ctx);
    assert!(app.st.quick.starting.is_none(), "(b) the click started");
    let committed_by_click = app.st.editor.as_ref().unwrap().draft.is_none();
    eprintln!(
        "M1 (b): '{why}' drawn on hover; F5 refused ('Run: ...'); a click on Run started nothing (reported: the click \
         {} the field)",
        if committed_by_click {
            "committed"
        } else {
            "did not commit"
        }
    );
    // (c)
    if !committed_by_click {
        type_into_nx(&mut app, &ctx, &["ENTER"]);
    }
    let ed = app.st.editor.as_ref().unwrap();
    assert_eq!(
        (
            ed.draft.clone(),
            rcfd_studio::schema_model::value_at(&ed.settings, "mesh.nx").cloned()
        ),
        (None, Some(json!(20))),
        "(c) not committed"
    );
    let why = quick_run::blocked(&app.st).expect("(c) Run enabled with an unsaved edit");
    assert!(why.contains("save it first"), "(c) {why}");
    eprintln!("M1 (c): committed; '{why}'");
    // (d)
    app.st.save_case().unwrap();
    assert!(
        common::frames_until(&mut app, &ctx, lj::WAIT, |a| a
            .st
            .editor
            .as_ref()
            .is_some_and(|e| !e.saving && !e.is_dirty())),
        "(d) not saved"
    );
    assert_eq!(quick_run::blocked(&app.st), None, "(d)");
    run_uses_what_is_shown(&app.st, "(d)", &mut enabled_states);
    // (e)
    type_into_nx(&mut app, &ctx, &["5", "BS"]);
    let why = quick_run::blocked(&app.st).expect("(e) Run enabled while the field is typed");
    assert!(why.contains(FINISH_EDITING), "(e) {why}");
    type_into_nx(&mut app, &ctx, &["ENTER"]);
    let ed = app.st.editor.as_ref().unwrap();
    assert!(ed.draft.is_none() && !ed.is_dirty(), "(e) something was committed");
    assert_eq!(quick_run::blocked(&app.st), None, "(e) after Enter");
    run_uses_what_is_shown(&app.st, "(e)", &mut enabled_states);
    // (f)
    type_into_nx(&mut app, &ctx, &["BS", "BS", "1", "e", "ENTER"]);
    let ed = app.st.editor.as_ref().unwrap();
    assert_eq!(
        ed.draft,
        Some(("mesh.nx".to_string(), "1e".to_string())),
        "(f) the refused text is not kept as typed"
    );
    let why = quick_run::blocked(&app.st).expect("(f) Run enabled after a refused commit");
    assert!(why.contains(FINISH_EDITING), "(f) {why}");
    type_into_nx(&mut app, &ctx, &["BS", "BS", "2", "0", "ENTER"]);
    assert_eq!(quick_run::blocked(&app.st), None, "(f) after the correction");
    run_uses_what_is_shown(&app.st, "(f)", &mut enabled_states);
    eprintln!("M1 (d)-(g): saved: enabled; typed and reverted: greyed, then enabled; refused '1e': greyed until corrected; Run enabled in {enabled_states} states, each with the preview on the file");
    assert_eq!(enabled_states, 4, "(g) set-up");
}

/// Click the Run button where it was drawn (press and release in two frames).
fn click_run(app: &mut StudioApp, ctx: &egui::Context) {
    let _ = lj::frame_with(app, ctx, Vec::new());
    let pos = app.st.quick.rects["Run"].center();
    lj::click_at(app, ctx, pos);
    let _ = lj::frame_with(app, ctx, Vec::new());
}

/// M1 control: the route before (`run_while_typing`): with `20` typed and the field still being edited, Run is enabled,
/// its hover is the start text, and the preview's next build is of the editor's text, not of the file a run uses.
#[test]
fn m1_control_run_while_typing_route() {
    let (mut app, ctx) = m1_app("m1c", true);
    type_into_nx(&mut app, &ctx, &["BS", "BS", "2", "0"]);
    assert!(app.st.editor.as_ref().unwrap().draft.is_some(), "set-up");
    let blocked = quick_run::blocked(&app.st);
    let hover = hover_run(&mut app, &ctx).iter().any(|t| t == RUN_HOVER);
    let source = preview::current_source(&app.st);
    eprintln!(
        "M1 control (run while typing): blocked {blocked:?}, start hover drawn {hover}, preview source from the editor {}",
        matches!(source, Source::Editor(_))
    );
    assert!(blocked.is_none() && hover, "control: Run greyed out");
    assert!(matches!(source, Source::Editor(_)), "control: set-up");
}

// ---------------------------------------------------------------- M2: the camera of a rebuilt case

/// A full ring (`annulus_3d`, 3D): its x and y extents do not depend on `nz`.
const RING: &str =
    "title = \"ring\"\n[mesh]\ngenerator = \"annulus_3d\"\nnx = 12\nny = 2\nnz = 3\nr_in = 0.5\nr_out = 1.0\n\
                    lz = 1.0\n[physics]\nnu = 0.01\n[solver]\nalgorithm = \"simplec\"\n";

/// Target, distance, yaw, pitch and projection equal bit for bit (near and far may differ).
fn same_view(a: &Camera, b: &Camera) -> bool {
    let bits = |c: &Camera| {
        let p = match c.projection {
            Projection::Perspective { fovy } => (0, fovy.to_bits()),
            Projection::Orthographic { half_height } => (1, half_height.to_bits()),
        };
        (
            c.target.map(f64::to_bits),
            c.distance.to_bits(),
            c.yaw.to_bits(),
            c.pitch.to_bits(),
            p,
        )
    };
    bits(a) == bits(b)
}

/// Type `text` into field `path`, wait until the preview settled and installed what it shows; the shown mesh.
fn retype(st: &mut Studio, path: &str, text: &str) -> Arc<preview::Built> {
    lj::type_text(st, path, text);
    assert_eq!(lj::settle(st), Status::Ready, "{path} = {text}");
    st.drain();
    let b = lj::shown(st);
    assert!(
        Arc::ptr_eq(st.viewport.surface.as_ref().unwrap(), &b.surface),
        "{path} = {text}: not installed"
    );
    b
}

/// Commit field `path` (the text typed before) and let the debounce pass.
fn commit(st: &mut Studio, path: &str, text: &str) {
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, path, text).unwrap();
    lj::settle(st);
    st.drain();
}

fn counters(st: &Studio) -> (u64, u64, u64) {
    let s = st.preview.stats;
    (s.camera_kept, s.camera_refit, s.installs)
}

/// M2: the camera of the Mesh side across rebuilds. (a) The cavity opened: the start camera; then panned and zoomed.
/// (b) `nx` typed `20` (a new surface, the same x and y bounds; the front's z, `lx / nx`, changes): the camera kept
/// (target, distance, yaw, pitch, projection bit for bit; near and far `clip_planes` of the new box), `camera_kept` + 1.
/// (c) `nx` committed, then `lx` typed `2` (wider): refit, the camera `fit` of the one before to the new bounds,
/// `camera_refit` + 1. (d) The 3D ring opened: its start camera; orbited; `nz` typed `4` (same bounds): kept; `nz`
/// committed, `r_out` typed `1.5`: refit (yaw, pitch and the perspective kept). (e) The cavity again, then `mesh.source`
/// and `mesh.file` committed to the hex box (2D to 3D): the start camera of the hex box, neither counter moves.
/// (f) After orbiting, Run then Mesh with a finished run (ADR 0144 question 3 (a)): the start camera. Negative control:
/// ADR 0138's rule (`restart_camera`): after (b) the start camera of the new surface.
#[test]
fn m2_a_rebuild_keeps_the_camera_while_the_bounds_stay() {
    let root = pv::project("m2", &[("cases/ring.toml", RING.as_bytes())]);
    let proj = Project::open(&root).unwrap();
    let rec = proj
        .create_run("cases/cavity.toml", None, "rcfd test", json!({}))
        .unwrap();
    proj.write_status(&rec.id, &RunStatus::new(RunState::Finished)).unwrap();
    let mut st = lj::studio(&root, false);
    // (a)
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    st.drain();
    let b0 = lj::shown(&st);
    let (lo0, hi0) = b0.surface.bounds();
    assert_eq!(
        st.viewport.camera,
        Some(CameraState::start(lo0, hi0, true)),
        "(a) start camera"
    );
    st.viewport.input(Input::Pan(30.0, -20.0));
    st.viewport.input(Input::Dolly(2.0));
    let c1 = st.viewport.camera.unwrap();
    assert!(
        !same_view(&c1.cam, &CameraState::start(lo0, hi0, true).cam),
        "(a) set-up: not moved"
    );
    let k0 = counters(&st);
    // (b)
    let b1 = retype(&mut st, "mesh.nx", "20");
    assert_eq!(b1.cells, 320, "(b)");
    let (lo1, hi1) = b1.surface.bounds();
    let c = st.viewport.camera.unwrap();
    eprintln!(
        "M2 (b): front z {} -> {} (x, y bounds {:?}..{:?} -> {:?}..{:?})",
        hi0[2],
        hi1[2],
        &lo0[..2],
        &hi0[..2],
        &lo1[..2],
        &hi1[..2]
    );
    assert!(
        same_view(&c.cam, &c1.cam),
        "(b) the camera was not kept: {c:?} vs {c1:?}"
    );
    assert_eq!((c.lo, c.hi), (lo1, hi1), "(b) bounds");
    assert_eq!(c.cam, clip_planes(&c1.cam, lo1, hi1), "(b) near and far");
    assert_eq!(counters(&st), (k0.0 + 1, k0.1, k0.2 + 1), "(b) counters");
    // (c)
    commit(&mut st, "mesh.nx", "20");
    let before = st.viewport.camera.unwrap();
    let b2 = retype(&mut st, "mesh.lx", "2");
    let (lo2, hi2) = b2.surface.bounds();
    let c = st.viewport.camera.unwrap();
    assert_eq!(c.cam, fit(&before.cam, lo2, hi2), "(c) not refit");
    assert_eq!((c.lo, c.hi), (lo2, hi2), "(c) bounds");
    assert!(
        !same_view(&c.cam, &before.cam),
        "(c) set-up: the bounds did not change the view"
    );
    assert_eq!(counters(&st), (k0.0 + 1, k0.1 + 1, k0.2 + 2), "(c) counters");
    eprintln!("M2 (a)-(c): start camera; nx 16 -> 20 kept the panned camera; lx 1 -> 2 refit it (orientation kept)");
    // (d) 3D
    assert_eq!(lj::open(&mut st, "cases/ring.toml"), Status::Ready);
    st.drain();
    let r0 = lj::shown(&st);
    let (rlo, rhi) = r0.surface.bounds();
    assert!(!r0.two_d, "(d) set-up: 3D");
    assert_eq!(
        st.viewport.camera,
        Some(CameraState::start(rlo, rhi, false)),
        "(d) start camera"
    );
    st.viewport.input(Input::Orbit(40.0, 25.0));
    let c3 = st.viewport.camera.unwrap();
    let k3 = counters(&st);
    let r1 = retype(&mut st, "mesh.nz", "4");
    assert_ne!(r1.mesh_hash, r0.mesh_hash, "(d) set-up: nz did not change the mesh");
    let c = st.viewport.camera.unwrap();
    assert!(same_view(&c.cam, &c3.cam), "(d) 3D camera not kept");
    assert_eq!(counters(&st), (k3.0 + 1, k3.1, k3.2 + 1), "(d) counters");
    commit(&mut st, "mesh.nz", "4");
    let before = st.viewport.camera.unwrap();
    let r2 = retype(&mut st, "mesh.r_out", "1.5");
    let (lo, hi) = r2.surface.bounds();
    let c = st.viewport.camera.unwrap();
    assert_eq!(c.cam, fit(&before.cam, lo, hi), "(d) 3D not refit");
    assert_eq!(
        (c.cam.yaw, c.cam.pitch),
        (before.cam.yaw, before.cam.pitch),
        "(d) orientation"
    );
    assert!(
        matches!(c.cam.projection, Projection::Perspective { .. }),
        "(d) projection"
    );
    assert_eq!(counters(&st), (k3.0 + 1, k3.1 + 1, k3.2 + 2), "(d) counters");
    eprintln!(
        "M2 (d): the ring's start camera; orbited; nz 3 -> 4 kept; r_out 1 -> 1.5 refit (yaw, pitch, perspective kept)"
    );
    // (e) 2D to 3D in one case
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    st.drain();
    st.viewport.input(Input::Pan(10.0, 10.0));
    let k5 = counters(&st);
    {
        let ed = st.editor.as_mut().unwrap();
        ed.set_value("mesh.source", json!("file")).unwrap();
        ed.set_value("mesh.file", json!("../meshes/hex.msh")).unwrap();
    }
    assert_eq!(lj::settle(&mut st), Status::Ready, "(e)");
    st.drain();
    let h = lj::shown(&st);
    assert!(!h.two_d && h.cells == 64, "(e) set-up: the hex box");
    let (lo, hi) = h.surface.bounds();
    assert_eq!(
        st.viewport.camera,
        Some(CameraState::start(lo, hi, false)),
        "(e) not the start camera"
    );
    assert_eq!(counters(&st), (k5.0, k5.1, k5.2 + 1), "(e) counters");
    // (f) the view starts again after Run and Mesh
    st.viewport.input(Input::Orbit(-30.0, 10.0));
    st.select_run(&rec.id);
    assert_eq!(st.view.side, Side::Run, "(f) set-up");
    st.set_view(Side::Mesh).unwrap();
    assert!(st.drain_until(lj::WAIT, |s| s.viewport.run.is_none() && s.viewport.surface.is_some()));
    let b = lj::shown(&st);
    let (lo, hi) = b.surface.bounds();
    assert_eq!(st.viewport.camera, Some(CameraState::start(lo, hi, b.two_d)), "(f)");
    eprintln!("M2 (e), (f): 2D to 3D: the start camera; after Run and Mesh: the start camera");
}

/// M2 control: ADR 0138's rule (`restart_camera`): after (b)'s rebuild the camera is the start camera of the new
/// surface, not the panned one.
#[test]
fn m2_control_restart_camera_rule() {
    let root = pv::project("m2c", &[]);
    let mut st = lj::studio(&root, false);
    st.preview.switches.restart_camera = true;
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    st.drain();
    st.viewport.input(Input::Pan(30.0, -20.0));
    st.viewport.input(Input::Dolly(2.0));
    let c1 = st.viewport.camera.unwrap();
    let b1 = retype(&mut st, "mesh.nx", "20");
    let (lo, hi) = b1.surface.bounds();
    let c = st.viewport.camera.unwrap();
    eprintln!(
        "M2 control (restart camera): kept {}, start camera {}",
        same_view(&c.cam, &c1.cam),
        c == CameraState::start(lo, hi, true)
    );
    assert!(!same_view(&c.cam, &c1.cam), "control: the camera was kept");
    assert_eq!(st.preview.stats.camera_kept, 0);
}

// ---------------------------------------------------------------- M3: identical text, no build

fn built_and_skipped(st: &Studio) -> (u64, u64) {
    (st.preview.stats.meshes_built, st.preview.stats.skipped_identical)
}

/// M3: a build the debounce asks for of a text byte-identical to the shown mesh's, whose mesh file (if any) is unchanged,
/// is skipped and counted. (a) The cavity: `nx` typed `20`: built (320 cells). (b) The field committed (the same bytes):
/// skipped (`skipped_identical` + 1, no mesh built, ready, not stale, the same mesh shown, no new upload); `2` then `20`
/// typed again within the debounce: skipped. (c) `24` typed: built. (d) "Rebuild" with the same text: built (only the
/// debounce's requests are compared). (e) The hex box (`mesh.file`): `physics.nu` typed `0.02`: built; the mesh file's
/// modification time moved 10 s (bytes unchanged) and the field committed: built (the mesh input changed; the same mesh,
/// no new upload); `0.03` then `0.02` typed within the debounce: skipped. Negative control: ADR 0144's rule (`no_skip`):
/// (b)'s commit builds the mesh again.
#[test]
fn m3_an_identical_text_is_not_built_again() {
    let root = pv::project("m3", &[]);
    let mut st = lj::studio(&root, false);
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    let n0 = built_and_skipped(&st);
    // (a)
    let b = retype(&mut st, "mesh.nx", "20");
    assert_eq!((b.cells, b.from_editor), (320, true), "(a)");
    assert_eq!(built_and_skipped(&st), (n0.0 + 1, n0.1), "(a) counters");
    let installs = st.preview.stats.installs;
    // (b)
    let e = st.preview.stats.editor_requests;
    commit(&mut st, "mesh.nx", "20");
    assert_eq!(
        st.preview.stats.editor_requests,
        e + 1,
        "(b) set-up: the commit asked for no build"
    );
    assert_eq!(built_and_skipped(&st), (n0.0 + 1, n0.1 + 1), "(b) commit");
    assert_eq!(st.preview.status, Status::Ready, "(b)");
    assert!(!st.preview.stale(), "(b) stale");
    assert!(Arc::ptr_eq(&lj::shown(&st), &b), "(b) another mesh shown");
    assert!(
        st.preview.built.as_ref().is_some_and(|x| Arc::ptr_eq(x, &b)),
        "(b) built"
    );
    assert_eq!(st.preview.stats.installs, installs, "(b) a new upload");
    lj::type_text(&mut st, "mesh.nx", "2");
    lj::drain_for(&mut st, Duration::from_millis(100));
    lj::type_text(&mut st, "mesh.nx", "20");
    lj::settle(&mut st);
    assert_eq!(
        built_and_skipped(&st),
        (n0.0 + 1, n0.1 + 2),
        "(b) burst back to the shown text"
    );
    eprintln!("M3 (a), (b): nx 20 built; committed: skipped; 2 then 20: skipped");
    // (c)
    let b = retype(&mut st, "mesh.nx", "24");
    assert_eq!(b.cells, 384, "(c)");
    assert_eq!(built_and_skipped(&st), (n0.0 + 2, n0.1 + 2), "(c) counters");
    // (d)
    st.preview.rebuild();
    st.drain();
    lj::settle(&mut st);
    assert_eq!(built_and_skipped(&st), (n0.0 + 3, n0.1 + 2), "(d) Rebuild was skipped");
    // (e) a mesh file
    assert_eq!(lj::open(&mut st, "cases/hex.toml"), Status::Ready);
    let h0 = built_and_skipped(&st);
    let hb = retype(&mut st, "physics.nu", "0.02");
    assert!(hb.from_editor && hb.mesh_stamp.is_some(), "(e) set-up");
    assert_eq!(built_and_skipped(&st), (h0.0 + 1, h0.1), "(e) typed");
    let msh = root.join("meshes/hex.msh");
    let f = std::fs::OpenOptions::new().write(true).open(&msh).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(10)).unwrap();
    drop(f);
    let installs = st.preview.stats.installs;
    commit(&mut st, "physics.nu", "0.02");
    assert_eq!(
        built_and_skipped(&st),
        (h0.0 + 2, h0.1),
        "(e) the changed mesh file was not read again"
    );
    assert_eq!(st.preview.stats.installs, installs, "(e) the same mesh uploaded again");
    lj::type_text(&mut st, "physics.nu", "0.03");
    lj::drain_for(&mut st, Duration::from_millis(100));
    lj::type_text(&mut st, "physics.nu", "0.02");
    lj::settle(&mut st);
    assert_eq!(
        built_and_skipped(&st),
        (h0.0 + 2, h0.1 + 1),
        "(e) unchanged file: not skipped"
    );
    eprintln!(
        "M3 (c)-(e): nx 24 built; Rebuild built; hex: nu typed built, mtime moved and committed built (no upload), \
         0.03 then 0.02 skipped; skipped in all {}",
        st.preview.stats.skipped_identical
    );
}

/// M3 control: ADR 0144's rule (`no_skip`): the commit of (b) builds the mesh again.
#[test]
fn m3_control_no_skip_rule() {
    let root = pv::project("m3c", &[]);
    let mut st = lj::studio(&root, false);
    st.preview.switches.no_skip = true;
    assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready);
    let _ = retype(&mut st, "mesh.nx", "20");
    let n = built_and_skipped(&st);
    commit(&mut st, "mesh.nx", "20");
    let m = built_and_skipped(&st);
    eprintln!("M3 control (no skip): built {} -> {}, skipped {}", n.0, m.0, m.1);
    assert_eq!((m.0, m.1), (n.0 + 1, 0), "control: not built again");
}

// ---------------------------------------------------------------- M5: run order

/// The runs of M5: `(id, created)`. A and B are of one second, their ids sorted the other way; B and C share `created`;
/// D is a second earlier.
const M5_RUNS: [(&str, &str); 4] = [
    ("r-20261028T120000-ffff", "2026-10-28T12:00:00.100Z"), // A
    ("r-20261028T120000-0000", "2026-10-28T12:00:00.200Z"), // B
    ("r-20261028T120000-aaaa", "2026-10-28T12:00:00.200Z"), // C
    ("r-20261028T115959-9999", "2026-10-28T11:59:59.900Z"), // D
];
/// Expected order: by `created`, then id: D, A, B, C.
const M5_ORDER: [&str; 4] = [
    "r-20261028T115959-9999",
    "r-20261028T120000-ffff",
    "r-20261028T120000-0000",
    "r-20261028T120000-aaaa",
];

/// Write the M5 runs into `runs/` of the project at `root`: each made by `Project::create_run` in a staging project,
/// given its id and `created`, and moved in whole.
fn m5_runs(root: &Path, tag: &str) {
    let stage = common::temp(&format!("{tag}-stage")).join("proj");
    let p = Project::create(&stage, tag, "rcfd test").unwrap();
    std::fs::write(stage.join("cases/a.toml"), common::CAVITY).unwrap();
    for (k, (id, created)) in M5_RUNS.iter().enumerate() {
        let rec = p.create_run("cases/a.toml", None, "rcfd test", json!({})).unwrap();
        let dir = stage.join("runs").join(&rec.id);
        let mut v: Value = serde_json::from_slice(&std::fs::read(dir.join("run.json")).unwrap()).unwrap();
        v["id"] = json!(id);
        v["created"] = json!(created);
        std::fs::write(dir.join("run.json"), v.to_string()).unwrap();
        let st = RunStatus {
            step: k as u64,
            ..RunStatus::new(RunState::Finished)
        };
        std::fs::write(dir.join("status.json"), serde_json::to_vec(&st).unwrap()).unwrap();
        std::fs::rename(&dir, root.join("runs").join(id)).unwrap();
    }
}

fn ids_of(b: &Browser) -> Vec<String> {
    b.runs.iter().map(|r| r.id.clone()).collect()
}

/// M5: runs are ordered by `created` (`run.json`), then id (the maintainer's answer to ADR 0144 question 12), also runs
/// of one second whose ids sort the other way, and runs of one `created` time in one fixed order. (a) `Project::runs`;
/// (b) the Studio's browser and the order of the rows drawn in the Runs panel; (c) the browser after a run's
/// `status.json` changed (`Browser::refresh`); (d) the runs the run worker knows before the browser lists them
/// (`RunsView::extra`); (e) the newest run (`project_view::newest_run`, the launch smoke's pick) is C; (f) five more
/// listings in the same order. Negative control: the order of the ids (the smoke's rule before: the largest id) puts B
/// and C before A and picks A as the newest.
#[test]
fn m5_runs_are_ordered_by_created_then_id() {
    let root = common::temp("m5").join("proj");
    Project::create(&root, "m5", "rcfd test").unwrap();
    std::fs::write(root.join("cases/a.toml"), common::CAVITY).unwrap();
    m5_runs(&root, "m5");
    let p = Project::open(&root).unwrap();
    let want: Vec<String> = M5_ORDER.iter().map(|s| s.to_string()).collect();
    // (a)
    let listed: Vec<String> = p.runs().unwrap().runs.iter().map(|r| r.id.clone()).collect();
    assert_eq!(listed, want, "(a) Project::runs");
    // (b)
    let st = lj::studio(&root, false);
    assert_eq!(ids_of(st.browser().unwrap()), want, "(b) browser");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    }
    let rows: Vec<String> = drawn
        .iter()
        .filter_map(|t| t.split_whitespace().next())
        .filter(|w| M5_ORDER.contains(w))
        .map(String::from)
        .collect();
    assert_eq!(rows, want, "(b) the drawn Runs panel");
    // (c)
    let mut b = Browser::load(&p);
    let changed = vec![format!("runs/{}/status.json", M5_RUNS[0].0)];
    assert!(b.refresh(&p, &changed), "(c) set-up: nothing read");
    assert_eq!(ids_of(&b), want, "(c) after a refresh");
    // (e)
    assert_eq!(project_view::newest_run(&b), Some(M5_ORDER[3]), "(e) newest");
    // (f)
    for k in 0..5 {
        let again: Vec<String> = p.runs().unwrap().runs.iter().map(|r| r.id.clone()).collect();
        assert_eq!(again, want, "(f) listing {k}");
    }
    // (d) runs the worker knows before the browser lists them (watcher off, the runs moved in after the open)
    let root2 = common::temp("m5d").join("proj");
    Project::create(&root2, "m5d", "rcfd test").unwrap();
    std::fs::write(root2.join("cases/a.toml"), common::CAVITY).unwrap();
    let mut st = lj::studio(&root2, true);
    m5_runs(&root2, "m5d");
    assert!(
        st.drain_until(lj::WAIT, |s| s.runs.extra.len() == 4),
        "(d) the run worker did not see the runs: {:?}",
        st.runs.extra
    );
    assert!(
        st.browser().unwrap().runs.is_empty(),
        "(d) set-up: the browser lists them"
    );
    assert_eq!(st.runs.extra, want, "(d) RunsView::extra");
    // control: the order of the ids
    let mut by_id = want.clone();
    by_id.sort();
    let largest = by_id.last().cloned();
    eprintln!(
        "M5: Project::runs, the browser, the drawn rows, a refresh, the run worker's extra list: {want:?}; newest {}; \
         control: by id {by_id:?}, largest id {largest:?}",
        M5_ORDER[3]
    );
    assert_ne!(by_id, want, "control: the id order is the created order");
    assert_ne!(
        largest.as_deref(),
        Some(M5_ORDER[3]),
        "control: the largest id is the newest"
    );
}

// ---------------------------------------------------------------- M6: the status bar for a benchmark descriptor

/// Open `rel` and drain until its load was applied (the editor holds it, or a message about it came); the last message.
fn open_for_message(st: &mut Studio, rel: &str) -> String {
    let n = st.messages.len();
    st.open_case(rel);
    assert!(
        st.drain_until(lj::WAIT, |s| s.pending_case.is_none()
            && (s.editor.as_ref().is_some_and(|e| e.rel == rel) || s.messages.len() > n)),
        "{rel}: not loaded"
    );
    st.last_message().unwrap_or_default().to_string()
}

/// M6: a benchmark descriptor opened in the case editor gives the status bar the preview's one-line message (`<file>:
/// not a case file: this is a benchmark descriptor (id "<id>") ...`), for each of the 6 shipped descriptors; drawn in
/// the status bar. Negative controls: a JSON case with a bad value keeps the editor's located message; the route before
/// (`editor_lists_descriptor_keys`) gives the editor's list of unknown keys.
#[test]
fn m6_the_status_bar_says_not_a_case_file() {
    let repo = pv::repo_root();
    let ids = [
        "backward_step",
        "cavity",
        "channel",
        "flat_plate",
        "heated_cavity",
        "naca0012",
    ];
    let mut extra: Vec<(String, Vec<u8>)> = ids
        .iter()
        .map(|id| {
            let rel = format!("cases/{id}/case.json");
            (rel.clone(), std::fs::read(repo.join(&rel)).unwrap())
        })
        .collect();
    extra.push((
        "cases/bad.json".into(),
        br#"{"title": "x", "mesh": {"nx": "abc"}}"#.to_vec(),
    ));
    let refs: Vec<(&str, &[u8])> = extra.iter().map(|(r, b)| (r.as_str(), b.as_slice())).collect();
    let root = pv::project("m6", &refs);
    let mut st = lj::studio(&root, false);
    for (rel, bytes) in extra.iter().take(ids.len()) {
        let m = open_for_message(&mut st, rel);
        let want = preview::descriptor_message(rel, std::str::from_utf8(bytes).unwrap()).unwrap();
        assert_eq!(m, want, "{rel}");
        assert!(st.editor.is_none(), "{rel}: opened");
        assert_eq!(m.lines().count(), 1, "{rel}");
        eprintln!("M6 {m}");
    }
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let last = app.st.last_message().unwrap().to_string();
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    assert!(drawn.contains(&last), "the status bar does not draw it");
    let mut st = app.st;
    // controls
    let m = open_for_message(&mut st, "cases/bad.json");
    eprintln!("M6 control (a JSON case): {m}");
    assert!(
        m.starts_with("cases/bad.json: ") && m.contains("mesh.nx") && !m.contains("not a case file"),
        "control: {m}"
    );
    st.preview.switches.editor_lists_descriptor_keys = true;
    let rel = "cases/cavity/case.json";
    let m = open_for_message(&mut st, rel);
    let short = preview::descriptor_message(rel, std::str::from_utf8(&extra[1].1).unwrap()).unwrap();
    eprintln!("M6 control (the route before): {m}");
    assert!(m != short && m.contains("unknown setting"), "control: {m}");
}

// ---------------------------------------------------------------- M7: the runner's log tail once

/// Occurrences of `needle` in `hay`.
fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// M7 (a) and (c). (a) An early exit through the library (ADR 0146 E1's set-up: `runner-config.json` holds
/// `{"max_runs": 0}`, `Client::ensure_runner` of the runner of this build): the Studio's text of the failed start
/// (`quick_run::runner_error`) holds the runner's line (`must be positive`) exactly once, and is the client's report.
/// (c) An error that does not carry the runner's output (the 10 s timeout's wording) gets the last lines of
/// `runner.log` appended once. Negative control: the route before (`always`): (a)'s text holds the line twice.
#[test]
fn m7_the_log_tail_once_in_a_failed_start() {
    let (runner, _) = common::binaries();
    let dir = common::temp("m7a").join("proj");
    Project::create(&dir, "m7a", "rcfd test").unwrap();
    std::fs::write(dir.join(".rcfd/runner-config.json"), r#"{"max_runs": 0}"#).unwrap();
    let t = Instant::now();
    let e = rcfd_runner::client::Client::new(&dir, &runner)
        .ensure_runner()
        .expect_err("set-up: the runner started with max_runs 0");
    let dt = t.elapsed().as_secs_f64();
    assert!(
        quick_run::reports_runner_output(&e),
        "set-up: not an early-exit report: {e}"
    );
    let now = quick_run::runner_error(&e, &dir, false);
    let before = quick_run::runner_error(&e, &dir, true);
    eprintln!(
        "M7 (a): early exit reported after {dt:.3} s: '{e}'; the Studio's text has the runner's line {} time(s); the \
         route before {} times",
        count(&now, "must be positive"),
        count(&before, "must be positive")
    );
    assert_eq!(count(&now, "must be positive"), 1, "(a) {now}");
    assert_eq!(now, e, "(a) something appended");
    assert_eq!(count(&before, "must be positive"), 2, "control: {before}");
    // (c)
    let dir = common::temp("m7c").join("proj");
    Project::create(&dir, "m7c", "rcfd test").unwrap();
    std::fs::write(
        dir.join(".rcfd/runner.log"),
        "runner 1 started\nline two\nrcfd-runner: something went wrong\n",
    )
    .unwrap();
    let timeout = format!(
        "the runner did not take the lock in time (see {})",
        dir.join(".rcfd/runner.log").display()
    );
    assert!(!quick_run::reports_runner_output(&timeout), "(c) set-up");
    let t = quick_run::runner_error(&timeout, &dir, false);
    eprintln!("M7 (c): '{t}'");
    assert!(t.starts_with(&timeout), "(c) {t}");
    assert_eq!(count(&t, "; last lines of "), 1, "(c) {t}");
    assert_eq!(count(&t, "something went wrong"), 1, "(c) {t}");
}
