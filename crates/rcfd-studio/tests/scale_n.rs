//! ADR 0148 (job 48N) N9-N11, the maintainer's questions on the Run page ("what does 1t represent in time?", "what
//! scale are we at?", "can I increase the resolution?"): every time says what it is, a 2D view has a scale bar and both
//! captions give the domain and the cells, and "Refine x2" doubles a generator's counts as an unsaved edit. Negative
//! controls: the routes before (`viewport::Switches::bare_time`, `no_scale`). Release builds, `--test-threads 1
//! --nocapture`; the runs are the committed runs of `example-projects/wind-tunnel` (a copy).

mod common;
mod lj;
mod pv;

use rcfd_studio::preview::Status;
use rcfd_studio::viewport::{RunKind, Snap};
use rcfd_studio::{Studio, StudioApp};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

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

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type().unwrap().is_dir() {
            copy_dir(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

/// A copy of `example-projects/wind-tunnel` (its committed runs with stored fields).
fn wind_tunnel(tag: &str) -> PathBuf {
    let root = pv::temp(tag).join("proj");
    copy_dir(&pv::repo_root().join("example-projects/wind-tunnel"), &root);
    rcfd_project::paths::canonicalize(&root).unwrap()
}

/// The drawn app on `root` (watcher off, run worker on).
fn app_on(root: &Path, set: impl FnOnce(&mut Studio)) -> (StudioApp, egui::Context) {
    let mut st = Studio::in_memory();
    st.poll = None;
    set(&mut st);
    st.open_project(root);
    let ctx = egui::Context::default();
    let mut app = StudioApp::new(st, None);
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()
        && a.st.runs.last_tick.is_some()));
    (app, ctx)
}

/// The id of the committed run of `case`.
fn run_of(app: &StudioApp, case: &str) -> String {
    app.st
        .browser()
        .unwrap()
        .runs
        .iter()
        .find(|r| r.case == case)
        .unwrap_or_else(|| panic!("no run of {case}"))
        .id
        .clone()
}

/// Select `id` and draw until its newest snapshot (or snapshot `index`) is shown and its residuals are plotted; the
/// texts of the last frame.
fn show_run(app: &mut StudioApp, ctx: &egui::Context, id: &str, index: Option<usize>) -> Vec<(String, egui::Rect)> {
    app.st.select_run(id);
    assert!(common::frames_until(app, ctx, WAIT, |a| a.st.viewport.frame.is_some()
        && !a.st.plots.series.is_empty()
        && a.st.runs.shown.as_deref() == Some(id)));
    if let Some(i) = index {
        app.st.viewport.show_snapshot(i);
        assert!(common::frames_until(app, ctx, WAIT, |a| a
            .st
            .viewport
            .frame
            .as_ref()
            .is_some_and(|f| f.index == i)));
    }
    for _ in 0..2 {
        let _ = lj::frame_with(app, ctx, Vec::new());
    }
    texts_at(&lj::frame_with(
        app,
        ctx,
        vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))],
    ))
}

/// The drawn texts of the frames after the pointer comes to rest on `pos` (up to 30 frames, until `want` is drawn).
fn hover(app: &mut StudioApp, ctx: &egui::Context, pos: egui::Pos2, want: &str) -> bool {
    for _ in 0..30 {
        let drawn = lj::texts(&lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(pos)]));
        if drawn.iter().any(|t| t.contains(want)) {
            let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
            return true;
        }
    }
    let _ = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
    false
}

// ---------------------------------------------------------------- N9: every time says what it is

/// What N9 reads for one run: the slider's label, the snapshot line, the strip's line, the run's row of the Runs panel,
/// the Runs panel's header, the plots' x-axis label, and the drawn texts holding a bare `t`.
#[derive(Debug, Clone, Default)]
struct Words {
    slider: Vec<String>,
    snapshot: Vec<String>,
    strip: Vec<String>,
    row: Vec<String>,
    header: Vec<String>,
    x_axis: Vec<String>,
    bare_t: Vec<String>,
    hover: bool,
}

fn words(app: &mut StudioApp, ctx: &egui::Context, id: &str, index: Option<usize>) -> Words {
    let t = show_run(app, ctx, id, index);
    // the Viewport's time row is above the image, the plots below it (the default layout at 1280 x 800)
    let image = app.st.viewport.image_rect.expect("the image is drawn");
    let has = |f: &dyn Fn(&str, &egui::Rect) -> bool| -> Vec<String> {
        t.iter().filter(|(s, r)| f(s, r)).map(|(s, _)| s.clone()).collect()
    };
    let label = |s: &str| ["simulated time (s)", "time step", "iteration", "time", "step"].contains(&s);
    let mut w = Words {
        slider: has(&|s, r| label(s) && r.max.y < image.min.y),
        snapshot: has(&|s, _| s.starts_with("snapshot ")),
        strip: has(&|s, _| s.starts_with(&format!("run {id}  "))),
        row: has(&|s, _| s.starts_with(id)),
        header: has(&|s, _| s.starts_with("id  ")),
        x_axis: has(&|s, r| label(s) && r.min.y > image.max.y),
        bare_t: has(&|s, _| s.contains(" t = ") || s.contains("  t ")),
        hover: false,
    };
    // the slider's hover text, with the pointer on the slider's bar, 70 points left of its label (amendment 1)
    if let Some((_, r)) = t.iter().find(|(s, r)| label(s) && r.max.y < image.min.y) {
        let want = match app.st.viewport.kind() {
            RunKind::Transient => "Simulated time of the stored snapshots, in seconds",
            RunKind::Steady => "A steady run has no simulated time",
        };
        w.hover = hover(app, ctx, egui::pos2(r.min.x - 70.0, r.center().y), want);
    }
    eprintln!("  {id}: {w:?}");
    w
}

/// N9's procedure on the wind-tunnel copy: the cylinder (transient, 80 snapshots at 1, 2, ..., 80 s, 1280 steps) at its
/// newest and its first snapshot, then the aerofoil at 4 degrees (steady, 10 snapshots, 2000 iterations; amendment 1:
/// in place of the step, whose row of the Runs panel is out of view at 1280 x 800).
fn n9(tag: &str, bare: bool) -> (Words, Words, Words) {
    let root = wind_tunnel(tag);
    let (mut app, ctx) = app_on(&root, |st| st.viewport.switches.bare_time = bare);
    // tooltips at once, as ADR 0147's M1 (amendment 1: egui's default delay is the whole hover window)
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let cyl = run_of(&app, "cases/cylinder-re100.toml");
    let step = run_of(&app, "cases/naca0012-aoa4.toml");
    eprintln!("N9{}:", if bare { " control" } else { "" });
    let last = words(&mut app, &ctx, &cyl, None);
    let first = words(&mut app, &ctx, &cyl, Some(0));
    let steady = words(&mut app, &ctx, &step, None);
    (last, first, steady)
}

/// N9: the slider, the snapshot line, the strip, the Runs panel and the plots name the simulated time in seconds of a
/// transient run and the iterations of a steady one, in the same words, with the hover text; no bare `t` is drawn.
#[test]
fn n9_every_time_says_what_it_is() {
    let (last, first, steady) = n9("n9", false);
    let one = |s: &str| vec![s.to_string()];
    assert_eq!(last.slider, one("simulated time (s)"), "transient: the slider");
    assert_eq!(
        last.snapshot,
        one("snapshot 80/80  time step 1280  simulated time 80 s")
    );
    assert_eq!(first.snapshot, one("snapshot 1/80  time step 16  simulated time 1 s"));
    assert!(
        last.strip.len() == 1 && last.strip[0].contains("  time step 1280  simulated time 80 s  "),
        "transient: the strip {:?}",
        last.strip
    );
    assert!(last.hover && steady.hover, "the slider's hover text");
    assert_eq!(steady.slider, one("iteration"), "steady: the slider");
    // ADR 0169: the steady snapshot line and strip name the stored run's 10 snapshots and 2000 iterations; the aerofoil
    // run regenerated under ADR 0169 has 3 and 409, so those two checks are the known miss below (limits intact)
    assert!(
        steady.strip.len() == 1 && !steady.strip[0].contains("time"),
        "steady: the strip {:?}",
        steady.strip
    );
    assert!(
        last.header.len() == 1 && last.header[0].contains("time (s)"),
        "{:?}",
        last.header
    );
    let col = |w: &Words| {
        w.row
            .first()
            .and_then(|r| r.split_whitespace().nth(3).map(String::from))
    };
    assert_eq!(
        col(&last).as_deref(),
        Some("8.0000e1"),
        "transient: the Runs panel's time"
    );
    assert_eq!(col(&steady).as_deref(), Some("-"), "steady: the Runs panel's time");
    assert!(
        last.x_axis.contains(&"time step".to_string()),
        "transient: the plots' x axis"
    );
    assert!(
        steady.x_axis.contains(&"iteration".to_string()),
        "steady: the plots' x axis"
    );
    for w in [&last, &first, &steady] {
        assert!(w.bare_t.is_empty(), "a bare t is drawn: {:?}", w.bare_t);
    }
}

/// KNOWN MISS (ADR 0169 interim note and addendum): N9's steady snapshot line and strip as registered for the aerofoil run
/// at 4 degrees with 10 snapshots and 2000 iterations. The run regenerated under ADR 0169's far-field default meets the
/// residual targets at iteration 409 with 3 snapshots, and shows `snapshot 3/3  iteration 409`. Limits kept.
#[test]
#[ignore = "KNOWN MISS (ADR 0169): the regenerated aerofoil run has 3 snapshots and 409 iterations, N9 expects 10 and 2000"]
fn n9_steady_words_name_snapshot_10_and_iteration_2000_known_miss() {
    let (_, _, steady) = n9("n9km", false);
    assert_eq!(steady.snapshot, vec!["snapshot 10/10  iteration 2000".to_string()]);
    assert!(
        steady.strip.len() == 1 && steady.strip[0].contains("  iteration 2000  ") && !steady.strip[0].contains("time"),
        "steady: the strip {:?}",
        steady.strip
    );
}

/// N9 control: the words before (`bare_time`): "t = 80" and "step" drawn.
#[test]
fn n9_control_the_bare_t() {
    let (last, _, steady) = n9("n9ctl", true);
    assert_eq!(
        last.snapshot,
        vec!["snapshot 80/80  step 1280  t = 80".to_string()],
        "control"
    );
    assert!(
        !last.bare_t.is_empty() && steady.slider == vec!["step".to_string()],
        "control"
    );
}

/// N9 (pure): the kind of a run from its snapshots and its time.
#[test]
fn n9_kinds() {
    let s = |t: f64| Snap { step: 1, time: t };
    assert_eq!(rcfd_studio::viewport::run_kind(&[s(0.0), s(0.0)]), RunKind::Steady);
    assert_eq!(rcfd_studio::viewport::run_kind(&[s(0.5)]), RunKind::Transient);
    assert_eq!(rcfd_studio::viewport::run_kind_of_time(f64::NAN), RunKind::Steady);
    assert_eq!(rcfd_studio::viewport::fmt_num(80.0), "80");
    assert_eq!(rcfd_studio::viewport::fmt_num(0.0625), "0.0625");
    assert_eq!(rcfd_studio::viewport::fmt_num(1.6666666666666667e-7), "1.667e-7");
}

// ---------------------------------------------------------------- N10: a scale and the domain

/// The scale bar shown, checked against the camera: (its length in metres, its label, drawn over the image).
fn bar_of(app: &mut StudioApp, ctx: &egui::Context) -> Option<(f64, String, bool)> {
    let t = texts_at(&lj::frame_with(app, ctx, Vec::new()));
    let vp = &app.st.viewport;
    let rect = vp.image_rect?;
    let bar = vp.scale_bar(rect.width(), rect.height())?;
    let cam = vp.camera.unwrap().cam;
    let rcfd_render::Projection::Orthographic { half_height } = cam.projection else {
        panic!("a scale bar on a perspective view")
    };
    let per_pt = 2.0 * half_height / rect.height() as f64;
    let decade = 10f64.powf(bar.metres.log10().floor());
    let mantissa = bar.metres / decade;
    assert!(
        [1.0, 2.0, 5.0].iter().any(|k| (mantissa - k).abs() < 1e-9),
        "not 1, 2 or 5 times a power of ten: {}",
        bar.metres
    );
    assert!(
        (bar.points as f64 - bar.metres / per_pt).abs() <= 1e-3 * bar.points as f64,
        "{} points for {} m at {per_pt} m per point",
        bar.points,
        bar.metres
    );
    let w = rect.width();
    assert!(
        bar.points <= 0.25 * w + 1e-3 && bar.points >= 0.1 * w - 1e-3,
        "{} of {w}",
        bar.points
    );
    let drawn = t.iter().any(|(s, r)| *s == bar.label && rect.contains(r.center()));
    Some((bar.metres, bar.label, drawn))
}

/// N10's procedure. Mesh side (ADR 0138's project): the cavity (1 x 1 m, 16 x 16), its bar, then zoomed in 3 notches;
/// the hex box (2 x 1 x 1 m from a mesh file). Run side (the wind-tunnel copy): the cylinder (an O-grid of r_out 15 m,
/// 64 x 48 cells) and the step (15 x 1 m, 200 x 40). Returns the drawn resolution lines and readouts and the bars.
#[allow(clippy::type_complexity)]
fn n10(tag: &str, no_scale: bool) -> (Vec<Vec<String>>, Vec<Option<(f64, String, bool)>>) {
    let set = |st: &mut Studio| st.viewport.switches.no_scale = no_scale;
    let root = pv::project(tag, &[]);
    let (mut app, ctx) = app_on(&root, set);
    let mut lines = Vec::new();
    let mut bars = Vec::new();
    let domain = |t: &[(String, egui::Rect)]| -> Vec<String> {
        t.iter()
            .map(|(s, _)| s.clone())
            .filter(|s| s.contains("domain "))
            .collect()
    };
    for case in ["cases/cavity.toml", "cases/hex.toml"] {
        app.st.open_case(case);
        assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)
            && a.st.viewport.surface.is_some()));
        let t = texts_at(&lj::frame_with(&mut app, &ctx, Vec::new()));
        lines.push(domain(&t));
        bars.push(bar_of(&mut app, &ctx));
        if case == "cases/cavity.toml" {
            app.st.viewport.input(rcfd_studio::viewport::Input::Dolly(3.0));
            bars.push(bar_of(&mut app, &ctx));
        }
    }
    let wt = wind_tunnel(&format!("{tag}-wt"));
    let (mut app, ctx) = app_on(&wt, set);
    for case in ["cases/cylinder-re100.toml", "cases/step-re400.toml"] {
        let id = run_of(&app, case);
        let t = show_run(&mut app, &ctx, &id, None);
        lines.push(domain(&t));
        bars.push(bar_of(&mut app, &ctx));
    }
    eprintln!(
        "N10{}: lines {lines:?}; bars {bars:?}",
        if no_scale { " control" } else { "" }
    );
    (lines, bars)
}

/// N10: the Mesh side's resolution line and the Run side's readout give the domain in metres and the cells; a 2D view
/// has a scale bar of 1, 2 or 5 times a power of ten metres, a tenth to a quarter of the image wide, labelled over the
/// image, that follows the zoom; a 3D view has none.
#[test]
fn n10_scale_and_domain() {
    let (lines, bars) = n10("n10", false);
    let one = |s: &str| vec![s.to_string()];
    assert_eq!(lines[0], one("domain 1 x 1 m; 16 x 16 = 256 cells"), "the cavity");
    assert_eq!(
        lines[1],
        one("domain 2 x 1 x 1 m; 64 cells; the resolution is set by the mesh file"),
        "the hex box"
    );
    assert_eq!(
        lines[2],
        one("case cases/cylinder-re100.toml, domain 30 x 30 m, 3,072 cells"),
        "the cylinder's run"
    );
    assert_eq!(
        lines[3],
        one("case cases/step-re400.toml, domain 15 x 1 m, 8,000 cells"),
        "the step's run"
    );
    // bars: cavity, cavity zoomed, hex, cylinder, step
    assert!(bars[0].as_ref().is_some_and(|b| b.2), "the cavity: {:?}", bars[0]);
    assert!(
        bars[1]
            .as_ref()
            .is_some_and(|b| b.2 && b.0 <= bars[0].as_ref().unwrap().0),
        "zoomed: {:?}",
        bars[1]
    );
    assert_eq!(bars[2], None, "the hex box (3D) has no bar");
    assert!(
        bars[3].as_ref().is_some_and(|b| b.2) && bars[4].as_ref().is_some_and(|b| b.2),
        "{bars:?}"
    );
}

/// N10 control: before (`no_scale`): no domain and no bar.
#[test]
fn n10_control_no_scale() {
    let (lines, bars) = n10("n10ctl", true);
    assert!(
        lines.iter().all(|l| l.is_empty()) && bars.iter().all(|b| b.is_none()),
        "control"
    );
}

// ---------------------------------------------------------------- N11: Refine x2

/// N11's procedure (ADR 0138's project with `cases/ring.toml`, an annulus of 12 x 2 x 3 cells; watcher and run worker
/// off). Returns what each step showed.
#[derive(Debug, Default)]
struct Refined {
    /// (a) the cavity: the hover drawn, the message, the editor's counts, its state, the cells shown, the shown mesh is
    /// the refined case's, Run's refusal, the resolution line drawn after.
    hover: bool,
    message: Option<String>,
    counts: Vec<Option<serde_json::Value>>,
    dirty: bool,
    cells: Option<usize>,
    own: bool,
    run: Option<String>,
    line: Vec<String>,
    /// (b) the ring: cells before and after.
    ring: (Option<usize>, Option<usize>),
    /// (c) the hex box: a Refine button drawn.
    hex_button: bool,
    /// (d) over a cell limit of 1,000: the hover drawn, the preview's state after the click.
    over_hover: bool,
    over_status: Option<Status>,
    /// (e) a field being typed: the button's refusal drawn on hover, the counts after a click.
    typed_hover: bool,
    typed_counts: Vec<Option<serde_json::Value>>,
}

const RING: &str = r#"title = "ring (ADR 0148 fixture)"
[mesh]
generator = "annulus_3d"
nx = 12
ny = 2
nz = 3
r_in = 0.5
r_out = 1.0
lz = 1.0
[physics]
nu = 0.01
[solver]
algorithm = "simplec"
"#;

fn counts(st: &Studio) -> Vec<Option<serde_json::Value>> {
    let ed = st.editor.as_ref().expect("a case is open");
    ["mesh.nx", "mesh.ny", "mesh.nz"]
        .iter()
        .map(|p| rcfd_studio::schema_model::value_at(&ed.settings, p).cloned())
        .collect()
}

fn refine_rect(app: &mut StudioApp, ctx: &egui::Context) -> Option<egui::Rect> {
    app.st.preview.rects.clear();
    let _ = lj::frame_with(app, ctx, Vec::new());
    app.st.preview.rects.get(rcfd_studio::preview_ui::REFINE).copied()
}

fn open_settled(app: &mut StudioApp, ctx: &egui::Context, rel: &str) {
    app.st.open_case(rel);
    assert!(common::frames_until(app, ctx, WAIT, |a| a
        .st
        .editor
        .as_ref()
        .is_some_and(|e| e.rel == rel)
        && lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.rel == rel)));
}

/// ADR 0149 (registered change of N11's set-up): the case opened again with its unsaved edits discarded ("Discard
/// edits"), so that (d) and (e) start from the file as N11 registered; a click no longer drops them (the maintainer's
/// answer to ADR 0148 question 2).
fn open_fresh(app: &mut StudioApp, ctx: &egui::Context, rel: &str) {
    app.st.open_case(rel);
    let kept = app
        .st
        .editor
        .as_ref()
        .is_some_and(|e| e.rel == rel && rcfd_studio::studio::has_unsaved(e));
    if kept {
        app.st.discard_edits().unwrap();
    }
    assert!(common::frames_until(app, ctx, WAIT, |a| a
        .st
        .editor
        .as_ref()
        .is_some_and(|e| e.rel == rel && !rcfd_studio::studio::has_unsaved(e))
        && a.st.pending_case.is_none()
        && lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.rel == rel)));
}

fn n11(tag: &str, no_scale: bool) -> Refined {
    let root = pv::project(tag, &[("cases/ring.toml", RING.as_bytes())]);
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.viewport.switches.no_scale = no_scale;
    st.open_project(&root);
    let ctx = egui::Context::default();
    // tooltips at once, as ADR 0147's M1 (amendment 1: egui's default delay is the whole hover window)
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let mut app = StudioApp::new(st, None);
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()));
    let mut r = Refined::default();
    // (a)
    open_settled(&mut app, &ctx, "cases/cavity.toml");
    let Some(rect) = refine_rect(&mut app, &ctx) else {
        eprintln!("N11: no Refine x2 drawn");
        return r;
    };
    r.hover = hover(
        &mut app,
        &ctx,
        rect.center(),
        "mesh.nx 16 -> 32, mesh.ny 16 -> 32 (256 -> 1,024 cells",
    );
    let n = app.st.messages.len();
    lj::click_at(&mut app, &ctx, rect.center());
    r.message = app.st.messages.get(n).cloned();
    r.counts = counts(&app.st);
    r.dirty = app.st.editor.as_ref().is_some_and(|e| e.is_dirty());
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.from_editor)));
    let b = app.st.preview.shown.clone().unwrap();
    r.cells = Some(b.cells);
    let text = pv::CAVITY.replace("nx = 16", "nx = 32").replace("ny = 16", "ny = 32");
    let case = rcfd_studio::preview::parse_case(&text, &root.join("cases")).unwrap();
    r.own = rcfd_api::sim::build_mesh(&case).unwrap().content_hash() == b.mesh_hash;
    r.run = rcfd_studio::quick_run::blocked(&app.st);
    r.line = lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new()))
        .into_iter()
        .filter(|t| t.starts_with("domain "))
        .collect();
    // (b)
    open_settled(&mut app, &ctx, "cases/ring.toml");
    let before = app.st.preview.shown.as_ref().map(|b| b.cells);
    let rect = refine_rect(&mut app, &ctx).expect("(b) Refine x2 drawn");
    lj::click_at(&mut app, &ctx, rect.center());
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st)
        && a.st.preview.shown.as_ref().is_some_and(|b| b.from_editor)));
    r.ring = (before, app.st.preview.shown.as_ref().map(|b| b.cells));
    // (c)
    open_settled(&mut app, &ctx, "cases/hex.toml");
    r.hex_button = refine_rect(&mut app, &ctx).is_some();
    // (d)
    app.st.preview.limits.cells = 1000;
    open_fresh(&mut app, &ctx, "cases/cavity.toml");
    let rect = refine_rect(&mut app, &ctx).expect("(d) Refine x2 drawn");
    r.over_hover = hover(&mut app, &ctx, rect.center(), "Over the preview limit of 1,000 cells");
    lj::click_at(&mut app, &ctx, rect.center());
    let _ = common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st));
    r.over_status = Some(app.st.preview.status.clone());
    // (e)
    app.st.preview.limits.cells = rcfd_studio::preview::MAX_CELLS;
    open_fresh(&mut app, &ctx, "cases/cavity.toml");
    lj::type_text(&mut app.st, "mesh.ly", "2");
    let _ = common::frames_until(&mut app, &ctx, WAIT, |a| lj::settled(&a.st));
    let rect = refine_rect(&mut app, &ctx).expect("(e) Refine x2 drawn");
    r.typed_hover = hover(
        &mut app,
        &ctx,
        rect.center(),
        "cases/cavity.toml: finish editing the field mesh.ly first",
    );
    lj::click_at(&mut app, &ctx, rect.center());
    r.typed_counts = counts(&app.st);
    eprintln!("N11{}: {r:?}", if no_scale { " control" } else { "" });
    r
}

/// N11: "Refine x2" doubles the counts of the shown generator mesh as an unsaved edit, the preview builds the finer mesh
/// (4 times the cells in 2D, 8 in 3D), Run asks for a save; no button for a mesh file; over the cell limit the hover
/// says so and the preview asks before building; refused while a field is typed.
#[test]
fn n11_refine_x2() {
    let r = n11("n11", false);
    let j = |v: u64| Some(serde_json::json!(v));
    assert!(r.hover, "(a) the hover text");
    assert_eq!(
        r.message.as_deref(),
        Some(
            "cases/cavity.toml: refined x2 (mesh.nx 16 -> 32, mesh.ny 16 -> 32): 256 -> 1,024 cells, unsaved (save the \
             case before Run)"
        ),
        "(a)"
    );
    assert_eq!(r.counts[..2], [j(32), j(32)], "(a) the editor's counts");
    assert!(r.dirty, "(a) not an unsaved edit");
    assert_eq!(r.cells, Some(1024), "(a) the cells shown");
    assert!(r.own, "(a) the mesh shown is not the refined case's");
    assert!(
        r.run
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/cavity.toml has unsaved edits: save it first")),
        "(a) Run: {:?}",
        r.run
    );
    assert_eq!(
        r.line,
        vec!["domain 1 x 1 m; 32 x 32 = 1,024 cells".to_string()],
        "(a) the line"
    );
    assert_eq!(r.ring, (Some(72), Some(576)), "(b) the ring");
    assert!(!r.hex_button, "(c) a Refine x2 for a mesh file");
    assert!(r.over_hover, "(d) the hover over the limit");
    assert!(
        matches!(&r.over_status, Some(Status::TooLarge { rebuild: true, .. })),
        "(d) {:?}",
        r.over_status
    );
    assert!(r.typed_hover, "(e) the refusal while typing");
    assert_eq!(r.typed_counts[..2], [j(16), j(16)], "(e) the counts changed");
}

/// N11 control: before (`no_scale`): no Refine x2 is drawn.
#[test]
fn n11_control_no_refine() {
    let r = n11("n11ctl", true);
    assert!(!r.hover && r.message.is_none(), "control");
}
