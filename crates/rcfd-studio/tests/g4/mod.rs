//! Shared set-up of the ADR 0141 tests (wave 48G item 4, the one-click path): case texts, clicks on the recorded
//! button rectangles, key presses, drawn text. Test files declare `mod common; mod g4;` (and `mod vp;` for the GPU).
#![allow(dead_code)]

use crate::common::{self, RunnerProject};
use rcfd_studio::bins::Search;
use rcfd_studio::layout::Panel;
use rcfd_studio::{Studio, StudioApp};
use std::path::Path;
use std::time::{Duration, Instant};

pub const WAIT: Duration = Duration::from_secs(30);

/// Release step counts; debug builds use a quarter (as ADR 0133's process tests).
pub fn steps(n: u64) -> u64 {
    if cfg!(debug_assertions) {
        n / 4
    } else {
        n
    }
}

/// The 12 x 12 lid cavity of ADR 0133's tests (with the `lid_drag` report) running `steps` steps: the step count is in
/// the case text, because the one-click path takes no overrides.
pub fn cavity(steps: u64) -> String {
    let t = common::RUN_CASE.replace("max_iterations = 400", &format!("max_iterations = {steps}"));
    assert!(t.contains(&format!("max_iterations = {steps}")));
    t
}

/// A case whose text parses but whose mesh file does not exist: the runner's pre-launch check (`rcfd run
/// --check-only`) refuses it.
pub const MISSING_MESH: &str = r#"title = "w48g missing mesh"
[mesh]
source = "file"
file = "missing.msh"
[physics]
nu = 0.01
[solver]
algorithm = "simplec"
max_iterations = 10
"#;

/// A project with `cases/<name>` holding `text`, runner budgets as ADR 0133's tests (`max_runs` 3); the runner is
/// started directly when `serve` (the ADR 0128 rule), else not at all.
pub fn project(tag: &str, name: &str, text: &str, serve: bool) -> RunnerProject {
    let mut p = RunnerProject::new(tag, 3);
    std::fs::write(p.dir.join("cases").join(name), text).unwrap();
    if serve {
        p.serve();
    }
    p
}

/// The drawn app on project `p`: the Run button searches only `p`'s runner (as `RCFD_RUNNER_BIN` would), the project
/// open, the run worker ticked once and `cases/<case>` loaded in the editor (the selected case).
pub fn app_on(p: &RunnerProject, case: &str, ctx: &egui::Context) -> StudioApp {
    app_with(p, case, ctx, |_| {})
}

/// [`app_on`] with `set` applied to the model before the project opens (switches the workers read at start).
pub fn app_with(p: &RunnerProject, case: &str, ctx: &egui::Context, set: impl FnOnce(&mut Studio)) -> StudioApp {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = Search::explicit(&p.runner);
    set(&mut st);
    st.open_project(&p.dir);
    let mut app = StudioApp::new(st, None);
    assert!(
        common::frames_until(&mut app, ctx, WAIT, |a| a.st.browser().is_some()
            && a.st.runs.last_tick.is_some()),
        "the project did not open"
    );
    app.st.open_case(&format!("cases/{case}"));
    assert!(
        common::frames_until(&mut app, ctx, WAIT, |a| a.st.editor.is_some()),
        "the case did not load"
    );
    app
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

/// Click the button drawn with `label` in the last frame (its recorded rectangle): press and release at its centre
/// in two frames. Returns the time of the release frame.
pub fn click(app: &mut StudioApp, ctx: &egui::Context, label: &str) -> Instant {
    let _ = common::frame(app, ctx); // the rectangles of the current state
    let rect = *app
        .st
        .quick
        .rects
        .get(label)
        .unwrap_or_else(|| panic!("no button '{label}' drawn"));
    let pos = rect.center();
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame_with(app, ctx, vec![egui::Event::PointerMoved(pos), button(true)]);
    let t = Instant::now();
    let _ = frame_with(app, ctx, vec![button(false)]);
    t
}

/// A key press (and release) in one frame.
pub fn key(app: &mut StudioApp, ctx: &egui::Context, k: egui::Key) -> egui::FullOutput {
    let ev = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    frame_with(app, ctx, vec![ev(true), ev(false)])
}

/// The text of every drawn text shape.
pub fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn walk(s: &egui::Shape, v: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => v.push(t.galley.text().to_string()),
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

/// Is `p` the shown tab of its place in the layout?
pub fn in_front(app: &StudioApp, p: Panel) -> bool {
    let d = &app.st.dock;
    d.find_tab(&p)
        .and_then(|t| d.leaf(t.node_path()).ok().map(|l| l.active == t.tab))
        .unwrap_or(false)
}

/// Make `p` the shown tab of its place (the test's starting layout).
pub fn put_in_front(app: &mut StudioApp, p: Panel) {
    let path = app.st.dock.find_tab(&p).expect("panel open");
    app.st.dock.set_active_tab(path).unwrap();
}

/// Committed snapshots of a store folder (0 when it does not exist or cannot be read yet).
pub fn store_snapshots(dir: &Path) -> usize {
    if !dir.join(rcfd_tsstore::STORE_FILE).is_file() {
        return 0;
    }
    rcfd_tsstore::Reader::open(dir).map_or(0, |r| r.snapshots().len())
}

/// The largest residual step plotted.
pub fn max_step(st: &Studio) -> u64 {
    st.plots
        .series
        .values()
        .filter_map(|s| s.x.last().copied())
        .fold(0.0, f64::max) as u64
}
