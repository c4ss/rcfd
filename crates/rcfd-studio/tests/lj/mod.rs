//! Shared set-up of the ADR 0144 tests (job 48J: the Mesh / Run toggle and the preview that follows the case editor):
//! typing into the editor, waiting for the preview, clicks and hovers on the drawn app. Test files declare `mod pv;`
//! and `mod lj;` (and `mod common;` where they use its fixtures).
#![allow(dead_code)]

use rcfd_studio::preview::{Built, Status};
use rcfd_studio::{Studio, StudioApp};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const WAIT: Duration = Duration::from_secs(60);

/// The cavity of `pv::CAVITY` with `nx` x `ny` cells.
pub fn cavity(nx: usize, ny: usize) -> String {
    let t = crate::pv::CAVITY
        .replace("nx = 16", &format!("nx = {nx}"))
        .replace("ny = 16", &format!("ny = {ny}"));
    assert!(t.contains(&format!("nx = {nx}")) && t.contains(&format!("ny = {ny}")));
    t
}

/// A Studio (watcher off; run worker on when `runs`) with the project at `root` open (and, with the run worker, its
/// first tick applied).
pub fn studio(root: &Path, runs: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    if !runs {
        st.run_poll = None;
    }
    st.open_project(root);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() && (!runs || s.runs.last_tick.is_some())),
        "the project did not open"
    );
    st
}

/// True when the preview is not building and no editor change waits for its build.
pub fn settled(st: &Studio) -> bool {
    st.preview.due_in().is_none() && !matches!(st.preview.status, Status::Building { .. } | Status::Idle)
}

/// Open case `rel` and drain until its preview settled and the editor holds it; the status.
pub fn open(st: &mut Studio, rel: &str) -> Status {
    st.open_case(rel);
    assert!(
        st.drain_until(WAIT, |s| s.preview.case.as_deref() == Some(rel)
            && settled(s)
            && s.editor.as_ref().is_some_and(|e| e.rel == rel)),
        "{rel}: the preview did not settle or the editor did not load: {:?}",
        st.preview.status
    );
    st.preview.status.clone()
}

/// One keystroke in field `path` of the case editor that leaves `text` in it: the call the editor's text field makes on
/// every change (`CaseEditor::type_text`).
pub fn type_text(st: &mut Studio, path: &str, text: &str) {
    st.editor
        .as_mut()
        .expect("a case is open in the editor")
        .type_text(path, text);
}

/// Drain until the preview settled after an editor change (the debounce has passed and the build ended); the status.
pub fn settle(st: &mut Studio) -> Status {
    assert!(
        st.drain_until(WAIT, settled),
        "the preview did not settle: {:?}",
        st.preview.status
    );
    st.preview.status.clone()
}

/// Drain every 5 ms for `d`.
pub fn drain_for(st: &mut Studio, d: Duration) {
    let end = Instant::now() + d;
    while Instant::now() < end {
        st.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The mesh the viewport shows (panics when none).
pub fn shown(st: &Studio) -> Arc<Built> {
    st.preview
        .shown
        .clone()
        .unwrap_or_else(|| panic!("nothing shown: {:?}", st.preview.status))
}

/// The failure message of the preview (panics on any other status).
pub fn failed(st: &Studio) -> String {
    match &st.preview.status {
        Status::Failed(m) => m.clone(),
        other => panic!("not failed: {other:?}"),
    }
}

/// One headless frame (1280 x 800) with input events.
pub fn frame_with(app: &mut StudioApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.ui(ui))
}

/// The text of every drawn text shape of a frame.
pub fn texts(out: &egui::FullOutput) -> Vec<String> {
    crate::pv::texts(&out.shapes)
}

/// Press and release the primary button at `pos` (two frames, the pointer moved there first).
pub fn click_at(app: &mut StudioApp, ctx: &egui::Context, pos: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame_with(app, ctx, vec![egui::Event::PointerMoved(pos), button(true)]);
    let _ = frame_with(app, ctx, vec![button(false)]);
}

/// Click the toggle's button `label` ("Mesh" or "Run") where it was drawn in the last frame.
pub fn click_toggle(app: &mut StudioApp, ctx: &egui::Context, label: &str) {
    let _ = frame_with(app, ctx, Vec::new()); // the rectangles of the current state
    let rect = *app
        .st
        .view
        .rects
        .get(label)
        .unwrap_or_else(|| panic!("toggle button '{label}' not drawn"));
    click_at(app, ctx, rect.center());
}

/// A key press and release.
pub fn key(k: egui::Key) -> Vec<egui::Event> {
    let ev = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    vec![ev(true), ev(false)]
}

/// The repaint delay a frame asked for (`Duration::MAX`: none).
pub fn repaint_delay(out: &egui::FullOutput) -> Duration {
    out.viewport_output
        .get(&egui::ViewportId::ROOT)
        .map_or(Duration::MAX, |v| v.repaint_delay)
}
