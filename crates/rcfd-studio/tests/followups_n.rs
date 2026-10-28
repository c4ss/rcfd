//! ADR 0148 (job 48N) N5-N8: the middle drag orbits; data files under their own header (ADR 0147 question 3 (b)); a
//! save of the shown text is not built again (question 2 (b)); the Start run dialog's Start waits for the case editor
//! as Run does (question 4 (b)). Negative controls: the routes before (test switches). Release builds,
//! `--test-threads 1 --nocapture`.

mod common;
mod lj;
mod pv;

use rcfd_studio::preview::Status;
use rcfd_studio::ui::StartDialog;
use rcfd_studio::viewport::{drag_input, DragButton, Input};
use rcfd_studio::{Studio, StudioApp};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(30);

/// Every drawn text with its rectangle.
fn texts_at(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::Shape, v: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => v.push((t.galley.text().to_string(), t.visual_bounding_rect())),
            egui::Shape::Vec(x) => x.iter().for_each(|y| walk(y, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

/// The drawn app on the cavity project of ADR 0138 (watcher off; run worker on when `runs`).
fn app_on(root: &std::path::Path, runs: bool, set: impl FnOnce(&mut Studio)) -> (StudioApp, egui::Context) {
    let mut st = Studio::in_memory();
    st.poll = None;
    if !runs {
        st.run_poll = None;
    }
    set(&mut st);
    st.open_project(root);
    let ctx = egui::Context::default();
    let mut app = StudioApp::new(st, None);
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()
        && (!runs || a.st.runs.last_tick.is_some())));
    (app, ctx)
}

/// Open `rel` and draw frames until it is in the editor and its preview settled.
fn open(app: &mut StudioApp, ctx: &egui::Context, rel: &str) {
    app.st.open_case(rel);
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.st.editor.as_ref().is_some_and(|e| e.rel == rel)
            && a.st.pending_case.is_none()
            && lj::settled(&a.st)),
        "{rel} did not open: {:?}",
        app.st.preview.status
    );
}

// ---------------------------------------------------------------- N5: the middle drag orbits

/// A drag of `button` over the middle of the shown image: pressed, moved 4 times by (12, 7) points (one frame each),
/// released. The camera before and after.
fn drag(
    app: &mut StudioApp,
    ctx: &egui::Context,
    button: egui::PointerButton,
) -> (rcfd_render::Camera, rcfd_render::Camera) {
    let _ = lj::frame_with(app, ctx, Vec::new());
    let rect = app.st.viewport.image_rect.expect("the image is drawn");
    let before = app.st.viewport.camera.expect("a camera").cam;
    let mut pos = rect.center();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(pos), press(pos, true)]);
    for _ in 0..4 {
        pos += egui::vec2(12.0, 7.0);
        let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(pos)]);
    }
    let _ = lj::frame_with(app, ctx, vec![press(pos, false)]);
    let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
    let after = app.st.viewport.camera.expect("a camera").cam;
    (before, after)
}

fn bits(c: &rcfd_render::Camera) -> ([u64; 3], [u64; 2]) {
    (c.target.map(f64::to_bits), [c.yaw.to_bits(), c.pitch.to_bits()])
}

/// What each drag did to the camera: (orbited: yaw or pitch moved, panned: the target moved), for the primary, middle
/// and secondary buttons, on the hex box (3D) and the cavity (2D).
fn n5(tag: &str, middle_pans: bool) -> Vec<(String, bool, bool)> {
    let root = pv::project(tag, &[]);
    let (mut app, ctx) = app_on(&root, false, |st| st.viewport.switches.middle_pans = middle_pans);
    let mut out = Vec::new();
    for case in ["cases/hex.toml", "cases/cavity.toml"] {
        open(&mut app, &ctx, case);
        for (name, b) in [
            ("primary", egui::PointerButton::Primary),
            ("middle", egui::PointerButton::Middle),
            ("secondary", egui::PointerButton::Secondary),
        ] {
            let (x, y) = drag(&mut app, &ctx, b);
            let ((tx, ax), (ty, ay)) = (bits(&x), bits(&y));
            let s = (format!("{case} {name}"), ax != ay, tx != ty);
            eprintln!("N5{}: {s:?}", if middle_pans { " control" } else { "" });
            out.push(s);
        }
    }
    out
}

/// N5: the mapping (pure) and the drawn drags: the primary and the middle drag orbit (the target stays bit for bit),
/// the secondary drag pans (yaw and pitch stay bit for bit), in 3D and in 2D.
#[test]
fn n5_the_middle_drag_orbits() {
    assert_eq!(
        drag_input(DragButton::Primary, 3.0, -4.0, false),
        Input::Orbit(3.0, -4.0)
    );
    assert_eq!(
        drag_input(DragButton::Middle, 3.0, -4.0, false),
        Input::Orbit(3.0, -4.0)
    );
    assert_eq!(
        drag_input(DragButton::Secondary, 3.0, -4.0, false),
        Input::Pan(3.0, -4.0)
    );
    assert_eq!(
        drag_input(DragButton::Middle, 3.0, -4.0, true),
        Input::Pan(3.0, -4.0),
        "ADR 0132's mapping"
    );
    for (name, orbited, panned) in n5("n5", false) {
        let want_orbit = !name.ends_with("secondary");
        assert_eq!((orbited, panned), (want_orbit, !want_orbit), "{name}");
    }
}

/// N5 control: ADR 0132's mapping (`middle_pans`): the middle drag pans.
#[test]
fn n5_control_the_middle_drag_panned() {
    let s = n5("n5ctl", true);
    let middle: Vec<_> = s.iter().filter(|(n, ..)| n.ends_with("middle")).collect();
    assert!(middle.iter().all(|(_, o, p)| !o && *p), "control: {middle:?}");
}

// ---------------------------------------------------------------- N6: data files under their own header

/// N6's drawn panel and Start dialog list. Returns (the drawn texts of the Project panel's headers, the number of times
/// each file is drawn with the dialog's case list open, the selection after a click on the data file's label).
#[allow(clippy::type_complexity)]
fn n6(tag: &str, route_before: bool) -> (Vec<String>, Vec<(String, usize)>, Option<String>, Vec<String>) {
    let root = pv::project(
        tag,
        &[
            (
                "cases/b.json",
                br#"{"mesh": {"generator": "cavity_2d", "nx": 4, "ny": 4}}"#,
            ),
            ("cases/profile_inlet.csv", b"x [m], ux [m/s]\n0, 1\n1, 1\n"),
            ("cases/notes.txt", b"notes\n"),
        ],
    );
    let (mut app, ctx) = app_on(&root, false, |st| st.view.switches.data_files_as_cases = route_before);
    let listed = app.st.browser().unwrap().cases.clone();
    let out = lj::frame_with(&mut app, &ctx, Vec::new());
    let texts = texts_at(&out);
    let headers: Vec<String> = texts
        .iter()
        .filter(|(t, _)| t.starts_with("Cases (") || t.starts_with("Data files ("))
        .map(|(t, _)| t.clone())
        .collect();
    // a click on the data file's label
    let (_, r) = texts
        .iter()
        .find(|(t, r)| t == "cases/profile_inlet.csv" && r.center().x < 400.0)
        .expect("the data file is drawn")
        .clone();
    lj::click_at(&mut app, &ctx, r.center());
    lj::drain_for(&mut app.st, Duration::from_millis(300));
    let selected = app.st.selected_case.clone();
    // the Start run dialog's case list, opened
    app.ui_state.start_dialog = Some(StartDialog::default());
    // a new window is laid out unseen in its first frame (egui 0.36, ADR 0147 deviation 2)
    let mut out = lj::frame_with(&mut app, &ctx, Vec::new());
    for _ in 0..2 {
        out = lj::frame_with(&mut app, &ctx, Vec::new());
    }
    let (_, combo) = texts_at(&out)
        .into_iter()
        .find(|(t, _)| t == "choose a case")
        .expect("the dialog's case list is drawn");
    lj::click_at(&mut app, &ctx, combo.center());
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = texts_at(&lj::frame_with(&mut app, &ctx, Vec::new()));
    }
    let counts = listed
        .iter()
        .map(|f| (f.clone(), drawn.iter().filter(|(t, _)| t == f).count()))
        .collect();
    eprintln!("N6{}: headers {headers:?}; drawn with the list open {counts:?}; selected after the click {selected:?}; Project::cases {listed:?}", if route_before { " control" } else { "" });
    (headers, counts, selected, listed)
}

/// N6: the Project panel lists `.toml` and `.json` files as cases and the other files under "Data files"; a data file
/// is not selectable; the Start run dialog lists the case files only; `Project::cases` still lists every file.
#[test]
fn n6_data_files_are_not_cases() {
    let (headers, counts, selected, listed) = n6("n6", false);
    assert_eq!(headers, vec!["Cases (3)".to_string(), "Data files (2)".to_string()]);
    assert_eq!(selected, None, "a click on a data file selected it");
    for (f, n) in counts {
        let case = f.ends_with(".toml") || f.ends_with(".json");
        assert_eq!(
            n,
            if case { 2 } else { 1 },
            "{f}: drawn {n} times (the panel, and the dialog's list for a case)"
        );
    }
    assert_eq!(listed.len(), 5, "Project::cases");
}

/// N6 control: the route before lists all five files as cases.
#[test]
fn n6_control_every_file_is_a_case() {
    let (headers, counts, ..) = n6("n6ctl", true);
    assert_eq!(headers, vec!["Cases (5)".to_string()]);
    assert!(
        counts.iter().any(|(f, n)| f.ends_with(".csv") && *n == 2),
        "control: {counts:?}"
    );
}

// ---------------------------------------------------------------- N7: a save of the shown text is not built again

/// The preview's counters and shown mesh, to compare across a save.
fn preview_now(st: &Studio) -> (u64, u64, u64, Option<usize>, bool, bool, Option<usize>) {
    let p = &st.preview;
    let b = p.shown.as_ref();
    (
        p.stats.meshes_built,
        p.stats.skipped_saved,
        p.stats.installs,
        b.map(|b| Arc::as_ptr(&b.surface) as usize),
        b.is_some_and(|b| b.from_editor),
        p.stale(),
        b.map(|b| b.cells),
    )
}

fn save_and_wait(app: &mut StudioApp, ctx: &egui::Context) {
    app.st.save_case().unwrap();
    assert!(common::frames_until(app, ctx, WAIT, |a| a
        .st
        .editor
        .as_ref()
        .is_some_and(|e| !e.saving && !e.is_dirty())));
    // the watcher (500 ms) sees the written file: two polls and more
    let end = Instant::now() + Duration::from_millis(2000);
    while Instant::now() < end {
        let _ = lj::frame_with(app, ctx, Vec::new());
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// N7's procedure (watcher on, 500 ms): (a) the cavity: `mesh.nx` typed "20" and built, committed (ADR 0147 skips it),
/// saved; 2 s of frames; (b) the file replaced with other bytes (`nx = 24`); (c) the hex box: `physics.nu` typed and
/// committed, built; the mesh file's modification time moved 10 s ahead; saved. Returns the preview's state before and
/// after each save, (b)'s cells, and whether the source line drawn after (a) is the file's.
#[allow(clippy::type_complexity)]
fn n7(
    tag: &str,
    no_save_skip: bool,
) -> (
    [(u64, u64, u64, Option<usize>, bool, bool, Option<usize>); 4],
    Option<usize>,
    bool,
    Status,
) {
    let root = pv::project(tag, &[]);
    let mut st = Studio::in_memory();
    st.run_poll = None; // the watcher stays on (500 ms)
    st.preview.switches.no_save_skip = no_save_skip;
    st.open_project(&root);
    let ctx = egui::Context::default();
    let mut app = StudioApp::new(st, None);
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()));
    let schema = app.st.schema.clone();
    open(&mut app, &ctx, "cases/cavity.toml");
    lj::type_text(&mut app.st, "mesh.nx", "20");
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.cells == 320)));
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "mesh.nx", "20")
        .unwrap();
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)));
    let a0 = preview_now(&app.st);
    save_and_wait(&mut app, &ctx);
    let a1 = preview_now(&app.st);
    let status = app.st.preview.status.clone();
    let drawn = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()));
    let from_disk = drawn.iter().any(|t| t == rcfd_studio::preview_ui::FROM_DISK);
    // (b) another program writes other bytes
    std::fs::write(root.join("cases/cavity.toml"), pv::CAVITY.replace("nx = 16", "nx = 24")).unwrap();
    let _ = common::frames_until(&mut app, &ctx, Duration::from_secs(5), |a| {
        a.st.preview.shown.as_ref().is_some_and(|b| b.cells == 384)
    });
    let b_cells = app.st.preview.shown.as_ref().map(|b| b.cells);
    // (c) the hex box, its mesh file touched before the save
    open(&mut app, &ctx, "cases/hex.toml");
    lj::type_text(&mut app.st, "physics.nu", "0.02");
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.from_editor)));
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "physics.nu", "0.02")
        .unwrap();
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)));
    let mesh = root.join("meshes/hex.msh");
    let f = std::fs::File::options().write(true).open(&mesh).unwrap();
    f.set_modified(std::fs::metadata(&mesh).unwrap().modified().unwrap() + Duration::from_secs(10))
        .unwrap();
    drop(f);
    let c0 = preview_now(&app.st);
    save_and_wait(&mut app, &ctx);
    let _ = common::frames_until(&mut app, &ctx, Duration::from_secs(5), |a| lj::settled(&a.st));
    let c1 = preview_now(&app.st);
    eprintln!(
        "N7{}: (a) before the save {a0:?}, after {a1:?}, status {status:?}, the file's source line drawn {from_disk}; \
         (b) cells {b_cells:?}; (c) before {c0:?}, after {c1:?}",
        if no_save_skip { " control" } else { "" }
    );
    ([a0, a1, c0, c1], b_cells, from_disk, status)
}

/// N7: (a) the save of the shown text builds nothing (the watcher's notice of it neither): built meshes and installs
/// unchanged, the same surface, the save counted as skipped, ready, not stale, the mesh now the file's (source line);
/// (b) other bytes on disk build; (c) a save with the mesh file changed builds.
#[test]
fn n7_a_save_of_the_shown_text_is_not_built_again() {
    let ([a0, a1, c0, c1], b_cells, from_disk, status) = n7("n7", false);
    assert_eq!(a1.0, a0.0, "(a) built again after the save");
    assert!(a1.1 > a0.1, "(a) the save was not counted as skipped");
    assert_eq!((a1.2, a1.3), (a0.2, a0.3), "(a) a new install or surface");
    assert!(
        a0.4 && !a1.4,
        "(a) the mesh shown is not the file's after the save: {a0:?} {a1:?}"
    );
    assert!(!a1.5 && status == Status::Ready, "(a) stale or not ready: {status:?}");
    assert!(from_disk, "(a) the source line is not the file's");
    assert_eq!(b_cells, Some(384), "(b)");
    assert!(c1.0 > c0.0, "(c) the mesh file changed and nothing was built");
}

/// N7 control: ADR 0147's rule (`no_save_skip`): the save builds again.
#[test]
fn n7_control_the_save_builds_again() {
    let ([a0, a1, ..], ..) = n7("n7ctl", true);
    assert!(a1.0 > a0.0 && a1.1 == 0, "control: {a0:?} {a1:?}");
}

// ---------------------------------------------------------------- N8: the Start run dialog's Start waits too

/// The Start button's refusal once the dialog of `case` drew it with its plan (frames until then).
fn start_refusal(app: &mut StudioApp, ctx: &egui::Context, case: &str) -> Option<String> {
    let reopen = app.ui_state.start_dialog.as_ref().is_none_or(|d| d.case != case);
    if reopen {
        app.ui_state.start_dialog = Some(StartDialog {
            case: case.into(),
            ..Default::default()
        });
        app.ui_state.start_button = None;
    }
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.ui_state.start_button.is_some()
            && a.st
                .runs
                .plan
                .as_ref()
                .is_some_and(|p| p.as_ref().is_ok_and(|p| p.case == case))),
        "{case}: the dialog's Start was not drawn"
    );
    let _ = lj::frame_with(app, ctx, Vec::new());
    app.ui_state.start_button.as_ref().unwrap().1.clone()
}

/// N8's procedure (run worker on, no runner): the cavity open; (a) `mesh.nx` typed "20"; the dialog of the cavity: Start's
/// refusal and Run's; the hover drawn; a click on Start; (b) the field committed; (c) saved; (d) `mesh.nx` set to 24
/// (unsaved) and the dialog switched to the hex box. Returns the refusals of (a) to (d), Run's of (a) and (b), the
/// drawn hover of (a), and whether the click queued a run.
#[allow(clippy::type_complexity)]
fn n8(tag: &str, route_before: bool) -> (Vec<Option<String>>, Vec<Option<String>>, bool, bool) {
    let root = pv::project(tag, &[]);
    let (mut app, ctx) = app_on(&root, true, |st| st.quick.switches.start_ignores_editor = route_before);
    let schema = app.st.schema.clone();
    open(&mut app, &ctx, "cases/cavity.toml");
    let mut refusals = Vec::new();
    let mut run = Vec::new();
    lj::type_text(&mut app.st, "mesh.nx", "20");
    refusals.push(start_refusal(&mut app, &ctx, "cases/cavity.toml"));
    run.push(rcfd_studio::quick_run::blocked(&app.st));
    // the hover text over the greyed Start (frames until egui shows it: the pointer must rest, ADR 0147 amendment 1)
    let rect = app.ui_state.start_button.as_ref().unwrap().0;
    let mut hover = false;
    for _ in 0..30 {
        let drawn = lj::texts(&lj::frame_with(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(rect.center())],
        ));
        if refusals[0].as_ref().is_some_and(|w| drawn.iter().any(|t| t == w)) {
            hover = true;
            break;
        }
    }
    let n = app.st.messages.len();
    lj::click_at(&mut app, &ctx, rect.center());
    lj::drain_for(&mut app.st, Duration::from_millis(1000));
    let queued = app.st.messages[n..].iter().any(|m| m.contains("queued run"))
        || std::fs::read_dir(root.join("runs")).map_or(0, |d| d.count()) > 0;
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "mesh.nx", "20")
        .unwrap();
    refusals.push(start_refusal(&mut app, &ctx, "cases/cavity.toml"));
    run.push(rcfd_studio::quick_run::blocked(&app.st));
    app.st.save_case().unwrap();
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| a
        .st
        .editor
        .as_ref()
        .is_some_and(|e| !e.saving && !e.is_dirty())));
    refusals.push(start_refusal(&mut app, &ctx, "cases/cavity.toml"));
    app.st
        .editor
        .as_mut()
        .unwrap()
        .set_value("mesh.nx", serde_json::json!(24))
        .unwrap();
    refusals.push(start_refusal(&mut app, &ctx, "cases/hex.toml"));
    eprintln!(
        "N8{}: refusals {refusals:?}; Run's {run:?}; hover drawn {hover}; the click queued {queued}",
        if route_before { " control" } else { "" }
    );
    (refusals, run, hover, queued)
}

/// N8: while the case editor holds the dialog's case with a field being typed or unsaved edits, Start is greyed out
/// with Run's refusal as its hover text and a click starts nothing; saved, it is enabled; another case is not held up.
#[test]
fn n8_the_start_dialog_waits_for_the_editor() {
    let (refusals, run, hover, queued) = n8("n8", false);
    assert!(
        refusals[0]
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/cavity.toml: finish editing the field mesh.nx")),
        "(a) {refusals:?}"
    );
    assert_eq!(refusals[0], run[0], "(a) not Run's text");
    assert!(hover, "(a) the hover text was not drawn");
    assert!(!queued, "(a) the click on the greyed Start queued a run");
    assert_eq!(
        refusals[1].as_deref(),
        Some("cases/cavity.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk"),
        "(b)"
    );
    assert_eq!(refusals[1], run[1], "(b) not Run's text");
    assert_eq!(refusals[2], None, "(c) saved");
    assert_eq!(refusals[3], None, "(d) another case");
}

/// N8 control: the route before (`start_ignores_editor`): Start enabled while a field is typed.
#[test]
fn n8_control_the_start_dialog_ignored_the_editor() {
    let (refusals, ..) = n8("n8ctl", true);
    assert_eq!(refusals[0], None, "control");
}
