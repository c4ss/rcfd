//! ADR 0148 (job 48N) N1-N4: the selected case is a piece of state of its own. A click in the Project panel sets it;
//! Run (F5), the toolbar's label, the Project panel's highlight, the Mesh side and the case editor follow it, also
//! before the case has been read, when the case editor cannot open it, and after a failed read of another case.
//! Negative controls: the route before (`QuickRun::switches.selection_from_editor`). Release builds,
//! `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd` of the same build (`common::binaries`).

mod common;
mod g4;
mod lj;
mod pv;

use common::RunnerProject;
use rcfd_studio::quick_run;
use rcfd_studio::view_toggle::Side;
use rcfd_studio::{Studio, StudioApp};
use std::path::Path;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

/// The cases of N1, N3 and N4: three cavities whose meshes differ (`nx` 8, 10 and 14), 30 iterations each.
const CASES: [(&str, usize); 3] = [("cases/a.toml", 8), ("cases/b.toml", 10), ("cases/c.toml", 14)];
/// A benchmark descriptor among them (N1 (c)): the case editor cannot open it, a run refuses it.
const DESCRIPTOR: &str = "cases/cav/case.json";

/// A served project with [`CASES`] and [`DESCRIPTOR`] (the runner of this build, budgets of ADR 0133's tests).
fn project(tag: &str) -> RunnerProject {
    let mut p = RunnerProject::new(tag, 3);
    for (rel, nx) in CASES {
        let t = g4::cavity(30).replace("nx = 12", &format!("nx = {nx}"));
        assert!(t.contains(&format!("nx = {nx}")));
        std::fs::write(p.dir.join(rel), t).unwrap();
    }
    std::fs::create_dir_all(p.dir.join("cases/cav")).unwrap();
    std::fs::copy(pv::repo_root().join("cases/cavity/case.json"), p.dir.join(DESCRIPTOR)).unwrap();
    p.serve();
    p
}

/// The drawn app on `p` (watcher and run worker on, as in the window), no case selected yet; the Run button searches
/// only `p`'s runner.
fn app(p: &RunnerProject, ctx: &egui::Context, route_before: bool) -> StudioApp {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.quick.switches.selection_from_editor = route_before;
    st.open_project(&p.dir);
    let mut app = StudioApp::new(st, None);
    assert!(
        common::frames_until(&mut app, ctx, WAIT, |a| a.st.browser().is_some()
            && a.st.runs.last_tick.is_some()),
        "the project did not open"
    );
    app
}

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

/// Every drawn rectangle filled with `fill`.
fn filled(out: &egui::FullOutput, fill: egui::Color32) -> Vec<egui::Rect> {
    fn walk(s: &egui::Shape, fill: egui::Color32, v: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Rect(r) if r.fill == fill => v.push(r.rect),
            egui::Shape::Vec(x) => x.iter().for_each(|y| walk(y, fill, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, fill, &mut v);
    }
    v
}

/// What a drawn frame shows of the selection: the case labels the Project panel highlights (drawn on the selection
/// colour), and the case named right of the Run button.
fn drawn_selection(app: &mut StudioApp, ctx: &egui::Context) -> (Vec<String>, Vec<String>) {
    let out = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(640.0, 795.0))]);
    let sel = ctx.global_style().visuals.selection.bg_fill;
    let marks = filled(&out, sel);
    let texts = texts_at(&out);
    let highlight = texts
        .iter()
        .filter(|(t, r)| t.starts_with("cases/") && marks.iter().any(|m| m.contains(r.center())))
        .map(|(t, _)| t.clone())
        .collect();
    let run = app.st.quick.rects.get("Run").copied().expect("the Run button is drawn");
    let label = texts
        .iter()
        .filter(|(t, r)| t.starts_with("cases/") && (r.center().y - run.center().y).abs() < 8.0 && r.min.x > run.max.x)
        .map(|(t, _)| t.clone())
        .collect();
    (highlight, label)
}

/// A drawn click on the case label `text` of the Project panel (where the last frame drew it: the left third of the
/// window, below the Run toolbar; the toolbar's label and the case editor's header draw the same text elsewhere).
fn click_text(app: &mut StudioApp, ctx: &egui::Context, text: &str) {
    let out = lj::frame_with(app, ctx, Vec::new());
    let run = app.st.quick.rects.get("Run").copied().expect("the Run button is drawn");
    let (_, r) = texts_at(&out)
        .into_iter()
        .find(|(t, r)| t == text && r.center().x < 400.0 && r.min.y > run.max.y + 4.0)
        .unwrap_or_else(|| panic!("'{text}' is not drawn in the Project panel"));
    lj::click_at(app, ctx, r.center());
}

/// Frames until no case is being read and the preview settled.
fn settle(app: &mut StudioApp, ctx: &egui::Context) {
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.st.pending_case.is_none() && lj::settled(&a.st)),
        "did not settle: pending {:?}, preview {:?}",
        app.st.pending_case,
        app.st.preview.status
    );
}

/// `build_mesh` of the case file on disk.
fn own_mesh(root: &Path, rel: &str) -> u64 {
    let case = rcfd_api::case::Case::load(root.join(rel)).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
    rcfd_api::sim::build_mesh(&case)
        .unwrap_or_else(|d| panic!("{rel}: {d:?}"))
        .content_hash()
}

/// The case file whose bytes a run's `case.snapshot` holds.
fn snapshot_case(p: &RunnerProject, id: &str) -> String {
    let snap = std::fs::read(p.run_dir(id).join("case.snapshot")).unwrap_or_default();
    CASES
        .iter()
        .map(|(r, _)| *r)
        .chain([DESCRIPTOR, "cases/cavity.toml"])
        .find(|r| std::fs::read(p.dir.join(r)).is_ok_and(|b| b == snap))
        .unwrap_or("(no case file)")
        .to_string()
}

/// What happened when Run was asked for: the case of the queued run, or the start's error.
#[derive(Debug, Clone, PartialEq)]
enum Ran {
    Run(String),
    Refused(String),
}

/// Wait for the start asked for to end (the run worker of the window reports it); what it did.
fn ran(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject, before: Option<String>) -> Ran {
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.st.quick.starting.is_none()),
        "the start did not end"
    );
    match app.st.quick.run.as_ref().map(|r| r.0.clone()) {
        Some(id) if Some(&id) != before.as_ref() => Ran::Run(snapshot_case(p, &id)),
        _ => Ran::Refused(app.st.quick.error.clone().unwrap_or_default()),
    }
}

/// Run as F5 asks for it (`run_selected_case`, the click-equivalent call), now, before any frame; what it did.
fn run_now(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject) -> Ran {
    let before = app.st.quick.run.as_ref().map(|r| r.0.clone());
    if let Err(m) = quick_run::run_selected_case(&mut app.st) {
        return Ran::Refused(m);
    }
    ran(app, ctx, p, before)
}

/// One step of N1: what the window showed and did.
#[derive(Debug, Clone)]
struct Step {
    name: &'static str,
    selected: Option<String>,
    highlight: Vec<String>,
    label: Vec<String>,
    editor: Option<String>,
    editor_error: Option<String>,
    preview: Option<String>,
    mesh_is_own: Option<bool>,
    side: Side,
    ran: Ran,
}

fn step(app: &mut StudioApp, ctx: &egui::Context, p: &RunnerProject, name: &'static str, ran: Ran) -> Step {
    settle(app, ctx);
    let (highlight, label) = drawn_selection(app, ctx);
    let st = &app.st;
    let mesh_is_own = st.preview.case.as_deref().filter(|c| *c != DESCRIPTOR).map(|c| {
        st.preview
            .shown
            .as_ref()
            .is_some_and(|b| b.rel == c && b.mesh_hash == own_mesh(&p.dir, c))
    });
    let s = Step {
        name,
        selected: quick_run::selected_case(st),
        highlight,
        label,
        editor: st.editor.as_ref().map(|e| e.rel.clone()),
        editor_error: st.editor_error.as_ref().map(|e| e.0.clone()),
        preview: st.preview.case.clone(),
        mesh_is_own,
        side: st.view.side,
        ran,
    };
    eprintln!("  {s:?}");
    s
}

/// N1's procedure: (a) a clicked (drawn), settled, Run clicked (drawn); (b) b selected and Run asked for at once,
/// before b is read; (c) the descriptor clicked (drawn), settled, Run; (d) c selected and Run at once; (e) a, b and a
/// selected in a row without a frame between, then Run.
fn n1(tag: &str, route_before: bool) -> Vec<Step> {
    let p = project(tag);
    let ctx = egui::Context::default();
    let mut app = app(&p, &ctx, route_before);
    let mut steps = Vec::new();
    // (a)
    click_text(&mut app, &ctx, "cases/a.toml");
    settle(&mut app, &ctx);
    let before = app.st.quick.run.as_ref().map(|r| r.0.clone());
    g4::click(&mut app, &ctx, "Run");
    let r = ran(&mut app, &ctx, &p, before);
    steps.push(step(&mut app, &ctx, &p, "(a) a clicked, settled, Run clicked", r));
    // (b)
    app.st.open_case("cases/b.toml");
    eprintln!(
        "  (b) at the Run call: editor {:?}, being read {:?}",
        app.st.editor.as_ref().map(|e| e.rel.clone()),
        app.st.pending_case
    );
    let r = run_now(&mut app, &ctx, &p);
    steps.push(step(&mut app, &ctx, &p, "(b) b selected, Run before it was read", r));
    // (c)
    click_text(&mut app, &ctx, DESCRIPTOR);
    settle(&mut app, &ctx);
    let r = run_now(&mut app, &ctx, &p);
    steps.push(step(&mut app, &ctx, &p, "(c) the descriptor clicked, settled, Run", r));
    // (d)
    app.st.open_case("cases/c.toml");
    let r = run_now(&mut app, &ctx, &p);
    steps.push(step(&mut app, &ctx, &p, "(d) c selected, Run before it was read", r));
    // (e)
    for c in ["cases/a.toml", "cases/b.toml", "cases/a.toml"] {
        app.st.open_case(c);
    }
    let r = run_now(&mut app, &ctx, &p);
    steps.push(step(&mut app, &ctx, &p, "(e) a, b, a selected in a row, Run", r));
    steps
}

/// N1: Run, the Mesh side, the highlight, the toolbar's label and the case editor follow the selected case, for three
/// cases and a benchmark descriptor between them, in both orders, also when Run is asked for before the case was read.
#[test]
fn n1_run_and_the_mesh_follow_the_selected_case() {
    let steps = n1("n1", false);
    let want = [
        ("cases/a.toml", Ran::Run("cases/a.toml".into())),
        ("cases/b.toml", Ran::Run("cases/b.toml".into())),
        (DESCRIPTOR, Ran::Refused(String::new())),
        ("cases/c.toml", Ran::Run("cases/c.toml".into())),
        ("cases/a.toml", Ran::Run("cases/a.toml".into())),
    ];
    for (s, (case, run)) in steps.iter().zip(want) {
        let one = vec![case.to_string()];
        assert_eq!(s.selected.as_deref(), Some(case), "{}: selected", s.name);
        assert_eq!(s.highlight, one, "{}: the Project panel's highlight", s.name);
        assert_eq!(s.label, one, "{}: the toolbar's label", s.name);
        assert_eq!(s.preview.as_deref(), Some(case), "{}: the Mesh side's case", s.name);
        // a run of the selected case takes the Viewport to it (ADR 0141); a refused start leaves the Mesh side
        let side = if matches!(run, Ran::Run(_)) {
            Side::Run
        } else {
            Side::Mesh
        };
        assert_eq!(s.side, side, "{}: the Viewport's side", s.name);
        match run {
            Ran::Run(c) => {
                assert_eq!(s.ran, Ran::Run(c.clone()), "{}: the run", s.name);
                assert_eq!(s.editor.as_deref(), Some(case), "{}: the editor", s.name);
                assert_eq!(s.editor_error, None, "{}", s.name);
                assert_eq!(
                    s.mesh_is_own,
                    Some(true),
                    "{}: the mesh shown is not the case's own",
                    s.name
                );
            }
            Ran::Refused(_) => {
                let Ran::Refused(m) = &s.ran else {
                    panic!("{}: a run was queued: {:?}", s.name, s.ran)
                };
                assert!(m.starts_with(&format!("{case} was not started: ")), "{}: {m}", s.name);
                assert_eq!(s.editor, None, "{}: the editor holds a case", s.name);
                assert_eq!(
                    s.editor_error.as_deref(),
                    Some(case),
                    "{}: the editor's message",
                    s.name
                );
            }
        }
    }
}

/// N1 control: the route before (the selection is the case editor's case): (b) runs a, and (c) runs b.
#[test]
fn n1_control_the_selection_from_the_editor() {
    let steps = n1("n1ctl", true);
    assert_eq!(steps[1].ran, Ran::Run("cases/a.toml".into()), "control (b)");
    assert_eq!(steps[2].ran, Ran::Run("cases/b.toml".into()), "control (c)");
}

// ---------------------------------------------------------------- N2: a failed read does not end the wait for another

/// N2's procedure on the cavity project of ADR 0138 with `cases/b.toml` (the cavity at 10 x 10) and `cases/gone.toml`,
/// watcher and run worker off: (a) the cavity opened; `cases/gone.toml` removed, selected, then b selected before a
/// drain; 1.5 s of drains. (b) `cases/profile_inlet.csv` (a data file) selected through the model, 1.5 s; (c) b again.
/// Returns, after (a), (b) and (c): (selected, editor, the case of the editor's message, the preview's case, the shown
/// mesh is that case's own).
#[allow(clippy::type_complexity)]
fn n2(tag: &str, route_before: bool) -> Vec<(Option<String>, Option<String>, Option<String>, Option<String>, bool)> {
    let root = pv::project(
        tag,
        &[
            ("cases/gone.toml", pv::CAVITY.as_bytes()),
            ("cases/b.toml", lj::cavity(10, 10).as_bytes()),
            ("cases/profile_inlet.csv", b"x [m], ux [m/s]\n0, 1\n1, 1\n"),
        ],
    );
    let mut st = pv::studio(&root);
    st.quick.switches.selection_from_editor = route_before;
    lj::open(&mut st, "cases/cavity.toml");
    let snap = |st: &Studio| {
        let own = st.preview.case.as_deref().is_some_and(|c| {
            c.ends_with(".toml")
                && st
                    .preview
                    .shown
                    .as_ref()
                    .is_some_and(|b| b.rel == c && b.mesh_hash == own_mesh(&root, c))
        });
        let s = (
            quick_run::selected_case(st),
            st.editor.as_ref().map(|e| e.rel.clone()),
            st.editor_error.as_ref().map(|e| e.0.clone()),
            st.preview.case.clone(),
            own,
        );
        eprintln!("  {s:?}; last message {:?}", st.last_message());
        s
    };
    let mut out = Vec::new();
    std::fs::remove_file(root.join("cases/gone.toml")).unwrap();
    st.open_case("cases/gone.toml");
    st.open_case("cases/b.toml");
    lj::drain_for(&mut st, Duration::from_millis(1500));
    out.push(snap(&st));
    st.open_case("cases/profile_inlet.csv");
    lj::drain_for(&mut st, Duration::from_millis(1500));
    out.push(snap(&st));
    st.open_case("cases/b.toml");
    lj::drain_for(&mut st, Duration::from_millis(1500));
    out.push(snap(&st));
    out
}

/// N2: a read that fails for one case (a file removed after it was listed) does not end the wait for the case selected
/// after it; a file the editor cannot open (a data file) leaves the editor empty with that file's message, not the case
/// it held before.
#[test]
fn n2_a_failed_read_does_not_keep_the_old_case() {
    let s = n2("n2", false);
    let b = Some("cases/b.toml".to_string());
    let csv = Some("cases/profile_inlet.csv".to_string());
    assert_eq!(s[0], (b.clone(), b.clone(), None, b.clone(), true), "(a)");
    assert_eq!(s[1], (csv.clone(), None, csv.clone(), csv.clone(), false), "(b)");
    assert_eq!(s[2], (b.clone(), b.clone(), None, b.clone(), true), "(c)");
}

/// N2 control: the route before keeps the cavity in the editor (and as the selection) after (a) while the preview
/// shows b.
#[test]
fn n2_control_the_selection_from_the_editor() {
    let s = n2("n2ctl", true);
    let cavity = Some("cases/cavity.toml".to_string());
    assert_eq!(s[0].0, cavity, "control (a): the selection");
    assert_eq!(s[0].1, cavity, "control (a): the editor");
    assert_eq!(s[0].3.as_deref(), Some("cases/b.toml"), "control (a): the preview");
}

// ---------------------------------------------------------------- N3: unsaved edits of the case left

/// N3's procedure (served project of N1): (a) a opened, `mesh.nx` set to 9 (unsaved): Run's refusal; (b) b selected:
/// Run's refusal at once (before b was read), then Run; (c) a selected again, settled: the editor's `mesh.nx` and state;
/// (d) a field of a typed (`mesh.ny` "11", not committed): Run's refusal; b selected and Run at once.
#[allow(clippy::type_complexity)]
fn n3(
    tag: &str,
    route_before: bool,
) -> (
    Option<String>,
    Option<String>,
    Ran,
    Option<serde_json::Value>,
    bool,
    Option<String>,
    Ran,
) {
    let p = project(tag);
    let ctx = egui::Context::default();
    let mut app = app(&p, &ctx, route_before);
    app.st.open_case("cases/a.toml");
    settle(&mut app, &ctx);
    app.st
        .editor
        .as_mut()
        .unwrap()
        .set_value("mesh.nx", serde_json::json!(9))
        .unwrap();
    let dirty = quick_run::blocked(&app.st);
    app.st.open_case("cases/b.toml");
    let at_switch = quick_run::blocked(&app.st);
    let r_b = run_now(&mut app, &ctx, &p);
    settle(&mut app, &ctx);
    app.st.open_case("cases/a.toml");
    settle(&mut app, &ctx);
    let ed = app.st.editor.as_ref().unwrap();
    let nx = rcfd_studio::schema_model::value_at(&ed.settings, "mesh.nx").cloned();
    let clean = !ed.is_dirty();
    lj::type_text(&mut app.st, "mesh.ny", "11");
    let typed = quick_run::blocked(&app.st);
    app.st.open_case("cases/b.toml");
    let r_typed = run_now(&mut app, &ctx, &p);
    settle(&mut app, &ctx);
    eprintln!(
        "N3: (a) {dirty:?}\n  (b) at the switch {at_switch:?}; Run {r_b:?}\n  (c) a again: mesh.nx {nx:?}, clean {clean}\n  \
         (d) typed {typed:?}; b at once: {r_typed:?}"
    );
    (dirty, at_switch, r_b, nx, clean, typed, r_typed)
}

/// N3: the "save it first" and "finish editing" rules hold for the case being edited; the selection of another case
/// takes Run with it at once (its file on disk); the edits of the case left are dropped (the behaviour before, kept and
/// reported).
#[test]
fn n3_unsaved_edits_and_another_case() {
    let (dirty, at_switch, r_b, nx, clean, typed, r_typed) = n3("n3", false);
    assert_eq!(
        dirty.as_deref(),
        Some("cases/a.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk"),
        "(a)"
    );
    assert_eq!(at_switch, None, "(b) Run refused at the switch");
    assert_eq!(r_b, Ran::Run("cases/b.toml".into()), "(b)");
    assert_eq!(nx, Some(serde_json::json!(8)), "(c) a's file on disk");
    assert!(clean, "(c)");
    assert!(
        typed
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/a.toml: finish editing the field mesh.ny")),
        "(d) {typed:?}"
    );
    assert_eq!(r_typed, Ran::Run("cases/b.toml".into()), "(d)");
}

/// N3 control: the route before refuses Run of b at the switch with a's "save it first".
#[test]
fn n3_control_the_selection_from_the_editor() {
    let (_, at_switch, ..) = n3("n3ctl", true);
    assert!(
        at_switch
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/a.toml has unsaved edits")),
        "control: {at_switch:?}"
    );
}

// ---------------------------------------------------------------- N4: another case selected while Run starts

/// N4's procedure (served project of N1): a selected and settled; Run asked for, and c selected at once (before the
/// run is queued); the start ended. Returns (the selected run is the queued one, its case, the Viewport's side, the run
/// the viewport shows, the preview's case), then the same after Run of c.
#[allow(clippy::type_complexity)]
fn n4(tag: &str, route_before: bool) -> Vec<(bool, String, Side, Option<String>, Option<String>)> {
    let p = project(tag);
    let ctx = egui::Context::default();
    let mut app = app(&p, &ctx, route_before);
    app.st.open_case("cases/a.toml");
    settle(&mut app, &ctx);
    let mut out = Vec::new();
    for then in [Some("cases/c.toml"), None] {
        let before = app.st.quick.run.as_ref().map(|r| r.0.clone());
        quick_run::run_selected_case(&mut app.st).unwrap();
        if let Some(c) = then {
            app.st.open_case(c);
        }
        let r = ran(&mut app, &ctx, &p, before);
        settle(&mut app, &ctx);
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
        let id = app.st.quick.run.as_ref().map(|r| r.0.clone());
        let s = (
            app.st.selected_run == id,
            match r {
                Ran::Run(c) => c,
                Ran::Refused(m) => m,
            },
            app.st.view.side,
            app.st.viewport.run.clone(),
            app.st.preview.case.clone(),
        );
        eprintln!("N4 {then:?}: {s:?}");
        out.push(s);
    }
    out
}

/// N4: a run that Run queued after another case was selected is selected (the plots and the strip follow it) and the
/// Viewport stays on the selected case's mesh; Run of the selected case then shows its run.
#[test]
fn n4_a_case_selected_while_run_starts_keeps_the_mesh() {
    let s = n4("n4", false);
    assert_eq!(
        s[0],
        (
            true,
            "cases/a.toml".into(),
            Side::Mesh,
            None,
            Some("cases/c.toml".into())
        ),
        "(a) c selected while a's run started"
    );
    assert!(
        s[1].0 && s[1].1 == "cases/c.toml" && s[1].2 == Side::Run && s[1].3.is_some(),
        "(b) {:?}",
        s[1]
    );
}

/// N4 control: the route before switches the Viewport to a's run.
#[test]
fn n4_control_the_queued_run_takes_the_viewport() {
    let s = n4("n4ctl", true);
    assert_eq!(s[0].2, Side::Run, "control: {:?}", s[0]);
}
