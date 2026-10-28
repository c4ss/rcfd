//! ADR 0149 (job 48O) O1-O3: the selected case and the selected run are linked (the maintainer's answer to ADR 0148
//! question 1). A click on a case selects its newest run (the Viewport stays on Mesh); a click on a run selects its case
//! (the Viewport goes to Run); a run whose case file is gone keeps the selected case and says so; a run that Run queued
//! after another case was selected is not selected. Negative controls: the route before (ADR 0148's,
//! `QuickRun::switches.case_run_unlinked`). Release builds, `--test-threads 1 --nocapture`; O3 with `rcfd-runner` and
//! `rcfd` of the same build (`common::binaries`).

mod common;
mod g4;
mod lj;
mod pv;

use common::RunnerProject;
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::quick_run;
use rcfd_studio::studio::no_run_of;
use rcfd_studio::view_toggle::{gone_case_text, Side, NO_RUN};
use rcfd_studio::{Studio, StudioApp};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

/// The run ids of the linking fixture, in creation order.
/// One step of O1: its name, what it showed, and the Run side's refusal then (`None`: it can be shown).
type Step = (&'static str, Seen, Option<String>);

struct Runs {
    /// Two runs of `cases/cavity.toml` (r1 older, r2 newer).
    r1: String,
    r2: String,
    /// A run of `cases/b.toml`.
    r3: String,
    /// A run of `cases/gone.toml`, a case removed after its run.
    r4: String,
    /// A run of `cases/old.toml`, a case renamed to `cases/new.toml` after its run.
    r5: String,
}

/// ADR 0138's project (`cases/cavity.toml` 16 x 16, `cases/hex.toml` without runs) with `cases/b.toml` (the cavity at 10
/// x 10), `cases/gone.toml` and `cases/old.toml`, and five finished runs made by `Project::create_run` 10 ms apart (so
/// that their `created` times differ): r1, r2 of the cavity, r3 of b, r4 of gone, r5 of old.
fn fixture(tag: &str) -> (PathBuf, Runs) {
    let root = pv::project(
        tag,
        &[
            ("cases/b.toml", lj::cavity(10, 10).as_bytes()),
            ("cases/gone.toml", pv::CAVITY.as_bytes()),
            ("cases/old.toml", lj::cavity(12, 12).as_bytes()),
        ],
    );
    let p = Project::open(&root).unwrap();
    let mut ids = Vec::new();
    for case in [
        "cases/cavity.toml",
        "cases/cavity.toml",
        "cases/b.toml",
        "cases/gone.toml",
        "cases/old.toml",
    ] {
        std::thread::sleep(Duration::from_millis(10));
        let rec = p.create_run(case, None, "rcfd test", serde_json::json!({})).unwrap();
        let st = RunStatus {
            step: 5,
            time: 0.0,
            ..RunStatus::new(RunState::Finished)
        };
        p.write_status(&rec.id, &st).unwrap();
        ids.push(rec.id);
    }
    let runs = Runs {
        r1: ids[0].clone(),
        r2: ids[1].clone(),
        r3: ids[2].clone(),
        r4: ids[3].clone(),
        r5: ids[4].clone(),
    };
    (root, runs)
}

/// Remove `cases/gone.toml` and rename `cases/old.toml` to `cases/new.toml` (after their runs), then Project > Refresh
/// (the watcher is off) until the listing has them so.
fn delete_and_rename(st: &mut Studio, root: &Path) {
    std::fs::remove_file(root.join("cases/gone.toml")).unwrap();
    std::fs::rename(root.join("cases/old.toml"), root.join("cases/new.toml")).unwrap();
    st.refresh();
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some_and(|b| {
            b.cases.iter().any(|c| c == "cases/new.toml") && !b.cases.iter().any(|c| c == "cases/gone.toml")
        })),
        "set-up: the listing did not follow the removal and the rename"
    );
}

/// Drain until nothing is being read and the preview settled. Amendment 2: with no case asked for the preview is idle,
/// which `lj::settled` does not count as settled; the registered "settled" (nothing read, no build running or waiting)
/// holds then.
fn settle(st: &mut Studio) {
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none()
            && (s.preview.case.is_none() || lj::settled(s))),
        "did not settle: pending {:?}, preview {:?}",
        st.pending_case,
        st.preview.status
    );
}

/// What a state shows of the selection, and where its parts disagree on the case (ADR 0149's rule: the Project panel's
/// highlight and Run's case (`quick_run::selected_case`), the Mesh side's case, the case editor's case and the case of
/// the run selected (the Run side's) are one case; a run whose case is not a listed case file is the one exception, and
/// the Run side's readout then names the missing case).
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    case: Option<String>,
    run: Option<String>,
    side: Side,
    viewport_run: Option<String>,
    preview: Option<String>,
    editor: Option<String>,
    disagree: Vec<String>,
}

fn seen(st: &Studio) -> Seen {
    let case = st.selected_case.clone();
    let mut disagree = Vec::new();
    if quick_run::selected_case(st) != case {
        disagree.push(format!("Run's case {:?}", quick_run::selected_case(st)));
    }
    if st.preview.case != case {
        disagree.push(format!("the Mesh side's case {:?}", st.preview.case));
    }
    if let Some(e) = st.editor.as_ref().filter(|e| Some(&e.rel) != case.as_ref()) {
        disagree.push(format!("the editor's case {}", e.rel));
    }
    if let Some(id) = &st.selected_run {
        let rc = st.run_case(id);
        let listed = rc.as_deref().is_some_and(|c| st.case_listed(c));
        if listed && rc != case {
            disagree.push(format!("the run's case {rc:?}"));
        }
        if !listed && st.view.side == Side::Run {
            let want = gone_case_text(rc.as_deref().unwrap_or_default());
            if !st.run_readout().is_some_and(|r| r.starts_with(&want)) {
                disagree.push(format!(
                    "the readout of a run of a missing case: {:?}",
                    st.run_readout()
                ));
            }
        }
    }
    if st.view.side == Side::Run && st.viewport.run != st.selected_run {
        disagree.push(format!("the Run side shows {:?}", st.viewport.run));
    }
    Seen {
        case,
        run: st.selected_run.clone(),
        side: st.view.side,
        viewport_run: st.viewport.run.clone(),
        preview: st.preview.case.clone(),
        editor: st.editor.as_ref().map(|e| e.rel.clone()),
        disagree,
    }
}

fn s(x: &str) -> Option<String> {
    Some(x.to_string())
}

/// O1's procedure (model; watcher off, run worker on): (a) nothing selected; (b) hex clicked (no run); (c) the cavity
/// clicked; the toggle to Run; (d) r1 clicked in the Runs panel; (e) the toggle to Mesh; (f) r3 (of b) clicked; (g) the
/// toggle to Mesh; (h) the cavity clicked; (i) gone.toml removed and old.toml renamed, refreshed; r4 clicked; (j) r5
/// clicked; (k) new.toml clicked; (l) the project closed. Each step: what is shown, settled.
fn o1(tag: &str, unlinked: bool) -> (Runs, Vec<Step>) {
    let (root, runs) = fixture(tag);
    let mut st = lj::studio(&root, true);
    st.quick.switches.case_run_unlinked = unlinked;
    assert!(
        st.drain_until(WAIT, |s| s.runs.rows.len() == 5),
        "set-up: the run worker did not list the 5 runs"
    );
    let mut out = Vec::new();
    let mut step = |name: &'static str, st: &mut Studio| {
        settle(st);
        let v = seen(st);
        let why_run = st.view_refusal(Side::Run);
        eprintln!(
            "O1{} {name}: {v:?}; Run side refused: {why_run:?}",
            if unlinked { " control" } else { "" }
        );
        out.push((name, v, why_run));
    };
    // amendment 3: a toggle refused (the route before has no run selected at (c')) is reported, not a stop; the limits
    // read the side after each toggle
    let toggle = |st: &mut Studio, side: Side| {
        if let Err(m) = st.set_view(side) {
            eprintln!(
                "O1{} the toggle to {side:?} refused: {m}",
                if unlinked { " control" } else { "" }
            );
        }
    };
    step("(a) nothing selected", &mut st);
    st.open_case("cases/hex.toml");
    step("(b) hex", &mut st);
    st.open_case("cases/cavity.toml");
    step("(c) cavity", &mut st);
    toggle(&mut st, Side::Run);
    step("(c') toggle Run", &mut st);
    st.open_run(&runs.r1);
    // the same case: its editor is not read again
    assert_eq!(st.pending_case, None, "(d) the cavity was read again for a run of it");
    step("(d) r1", &mut st);
    toggle(&mut st, Side::Mesh);
    step("(e) toggle Mesh", &mut st);
    st.open_run(&runs.r3);
    step("(f) r3", &mut st);
    toggle(&mut st, Side::Mesh);
    step("(g) toggle Mesh", &mut st);
    st.open_case("cases/cavity.toml");
    step("(h) cavity", &mut st);
    delete_and_rename(&mut st, &root);
    st.open_run(&runs.r4);
    step("(i) r4 (case removed)", &mut st);
    let msg_i = st.last_message().map(String::from);
    st.open_run(&runs.r5);
    step("(j) r5 (case renamed)", &mut st);
    st.open_case("cases/new.toml");
    step("(k) new.toml", &mut st);
    st.close_project();
    let v = seen(&st);
    eprintln!("O1 (l) closed: {v:?}");
    out.push(("(l) closed", v, st.view_refusal(Side::Run)));
    out.push(("(i) message", seen(&st), msg_i));
    (runs, out)
}

/// O1: the state machine of ADR 0149 through the model, every transition: a case click selects its newest run (none:
/// the Run side refused for that case) and stays on Mesh; a run click selects its case and goes to Run (the same case:
/// not read again); the toggle changes only the side; a run of a removed or renamed case keeps the selected case and the
/// readout names the missing case; at every step the parts agree on the case.
#[test]
fn o1_the_case_and_its_runs_are_one_selection() {
    let (r, out) = o1("o1", false);
    let get = |name: &str| out.iter().find(|(n, ..)| *n == name).cloned().unwrap();
    let want = |name: &str, case: Option<String>, run: Option<String>, side: Side, preview: Option<String>| {
        let (_, v, _) = get(name);
        assert!(v.disagree.is_empty(), "{name}: the parts disagree: {:?}", v.disagree);
        assert_eq!((v.case, v.run, v.side, v.preview), (case, run, side, preview), "{name}");
    };
    // (a)
    let (_, a, why) = get("(a) nothing selected");
    assert_eq!((a.case, a.run, a.side), (None, None, Side::Mesh), "(a)");
    assert_eq!(why.as_deref(), Some(NO_RUN), "(a) the Run side's refusal");
    // (b) a case without a run: the Run side refused for it
    want("(b) hex", s("cases/hex.toml"), None, Side::Mesh, s("cases/hex.toml"));
    assert_eq!(
        get("(b) hex").2,
        Some(no_run_of("cases/hex.toml")),
        "(b) the Run side's refusal"
    );
    // (c) the case's newest run, the Mesh side
    want(
        "(c) cavity",
        s("cases/cavity.toml"),
        Some(r.r2.clone()),
        Side::Mesh,
        s("cases/cavity.toml"),
    );
    assert_eq!(get("(c) cavity").1.viewport_run, None, "(c) the Mesh side shows a run");
    want(
        "(c') toggle Run",
        s("cases/cavity.toml"),
        Some(r.r2.clone()),
        Side::Run,
        s("cases/cavity.toml"),
    );
    // (d) the older run of the same case
    want(
        "(d) r1",
        s("cases/cavity.toml"),
        Some(r.r1.clone()),
        Side::Run,
        s("cases/cavity.toml"),
    );
    assert_eq!(get("(d) r1").1.editor, s("cases/cavity.toml"), "(d) the editor");
    want(
        "(e) toggle Mesh",
        s("cases/cavity.toml"),
        Some(r.r1.clone()),
        Side::Mesh,
        s("cases/cavity.toml"),
    );
    // (f) a run of another case: that case
    want(
        "(f) r3",
        s("cases/b.toml"),
        Some(r.r3.clone()),
        Side::Run,
        s("cases/b.toml"),
    );
    assert_eq!(get("(f) r3").1.editor, s("cases/b.toml"), "(f) the editor");
    want(
        "(g) toggle Mesh",
        s("cases/b.toml"),
        Some(r.r3.clone()),
        Side::Mesh,
        s("cases/b.toml"),
    );
    // (h) back to the cavity: its newest run again
    want(
        "(h) cavity",
        s("cases/cavity.toml"),
        Some(r.r2.clone()),
        Side::Mesh,
        s("cases/cavity.toml"),
    );
    // (i), (j) runs of a removed and of a renamed case: the selected case stays
    want(
        "(i) r4 (case removed)",
        s("cases/cavity.toml"),
        Some(r.r4.clone()),
        Side::Run,
        s("cases/cavity.toml"),
    );
    want(
        "(j) r5 (case renamed)",
        s("cases/cavity.toml"),
        Some(r.r5.clone()),
        Side::Run,
        s("cases/cavity.toml"),
    );
    assert_eq!(
        get("(i) message").2.as_deref(),
        Some(
            format!(
                "run {}: its case cases/gone.toml is not in the project any more (deleted or renamed), so the selected \
                 case is unchanged",
                r.r4
            )
            .as_str()
        ),
        "(i) the status bar"
    );
    // (k) the renamed file has no run of its own
    want(
        "(k) new.toml",
        s("cases/new.toml"),
        None,
        Side::Mesh,
        s("cases/new.toml"),
    );
    assert_eq!(get("(k) new.toml").2, Some(no_run_of("cases/new.toml")), "(k)");
    // (l)
    let (_, l, why) = get("(l) closed");
    assert_eq!((l.case, l.run, l.side), (None, None, Side::Mesh), "(l)");
    assert_eq!(why.as_deref(), Some(NO_RUN), "(l)");
}

/// O1 control: ADR 0148's route (`case_run_unlinked`): (c) the cavity's click keeps the run (none) and (f) the click on
/// a run of b keeps the cavity selected while the Run side shows b's run.
#[test]
fn o1_control_the_selections_were_apart() {
    let (r, out) = o1("o1ctl", true);
    let get = |name: &str| out.iter().find(|(n, ..)| *n == name).cloned().unwrap().1;
    assert_eq!(get("(c) cavity").run, None, "control (c)");
    let f = get("(f) r3");
    assert_eq!(
        (f.case, f.viewport_run),
        (s("cases/cavity.toml"), Some(r.r3.clone())),
        "control (f)"
    );
    assert!(!f.disagree.is_empty(), "control (f): the parts agree");
}

// ---------------------------------------------------------------- O2: drawn

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

/// What a drawn frame shows: the case labels the Project panel highlights, the case named right of the Run button, the
/// toggle's side and whether Run is enabled, and every drawn text.
#[derive(Debug, Clone)]
struct Drawn {
    highlight: Vec<String>,
    label: Vec<String>,
    side: Side,
    run_enabled: bool,
    texts: Vec<String>,
}

fn drawn(app: &mut StudioApp, ctx: &egui::Context) -> Drawn {
    let out = lj::frame_with(app, ctx, vec![egui::Event::PointerMoved(egui::pos2(640.0, 795.0))]);
    let sel = ctx.global_style().visuals.selection.bg_fill;
    let marks = filled(&out, sel);
    let texts = texts_at(&out);
    let highlight = texts
        .iter()
        .filter(|(t, r)| {
            t.starts_with("cases/") && r.center().x < 400.0 && marks.iter().any(|m| m.contains(r.center()))
        })
        .map(|(t, _)| t.clone())
        .collect();
    let run = app.st.quick.rects.get("Run").copied().expect("the Run button is drawn");
    let label = texts
        .iter()
        .filter(|(t, r)| t.starts_with("cases/") && (r.center().y - run.center().y).abs() < 8.0 && r.min.x > run.max.x)
        .map(|(t, _)| t.clone())
        .collect();
    Drawn {
        highlight,
        label,
        side: app.st.view.side,
        run_enabled: app.st.view.enabled.get("Run").copied().unwrap_or(false),
        texts: texts.into_iter().map(|(t, _)| t).collect(),
    }
}

/// A drawn click on the case label `text` of the Project panel.
fn click_case(app: &mut StudioApp, ctx: &egui::Context, text: &str) {
    let out = lj::frame_with(app, ctx, Vec::new());
    let run = app.st.quick.rects.get("Run").copied().expect("the Run button is drawn");
    let (_, r) = texts_at(&out)
        .into_iter()
        .find(|(t, r)| t == text && r.center().x < 400.0 && r.min.y > run.max.y + 4.0)
        .unwrap_or_else(|| panic!("'{text}' is not drawn in the Project panel"));
    lj::click_at(app, ctx, r.center());
}

/// A drawn click on the row of run `id` in the Runs panel (where the last frame drew it).
fn click_run_row(app: &mut StudioApp, ctx: &egui::Context, id: &str) {
    let _ = lj::frame_with(app, ctx, Vec::new());
    let r = *app
        .ui_state
        .run_rects
        .get(id)
        .unwrap_or_else(|| panic!("set-up: the row of run {id} is not drawn"));
    lj::click_at(app, ctx, r.center());
}

/// Frames until nothing is read and the preview settled.
fn settle_drawn(app: &mut StudioApp, ctx: &egui::Context) {
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.st.pending_case.is_none() && lj::settled(&a.st)),
        "did not settle: {:?}",
        app.st.preview.status
    );
}

/// O2's procedure (the drawn app, watcher off, run worker on; the fixture's first three runs only, so that their rows
/// are in view; tooltips at once): (a) a click on the cavity's label; (b) a click on the toggle's Run; (c) a click on
/// r3's row (a run of b); (d) a click on hex's label, then the pointer on the toggle's Run.
fn o2(tag: &str, unlinked: bool) -> (Runs, Vec<Drawn>, bool) {
    let (root, runs) = fixture(tag);
    for id in [&runs.r4, &runs.r5] {
        std::fs::remove_dir_all(root.join("runs").join(id)).unwrap();
    }
    let mut st = lj::studio(&root, true);
    st.quick.switches.case_run_unlinked = unlinked;
    assert!(st.drain_until(WAIT, |s| s.runs.rows.len() == 3), "set-up: 3 runs");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    for _ in 0..3 {
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    }
    let mut out = Vec::new();
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    out.push(drawn(&mut app, &ctx));
    lj::click_toggle(&mut app, &ctx, "Run");
    settle_drawn(&mut app, &ctx);
    out.push(drawn(&mut app, &ctx));
    click_run_row(&mut app, &ctx, &runs.r3);
    settle_drawn(&mut app, &ctx);
    out.push(drawn(&mut app, &ctx));
    click_case(&mut app, &ctx, "cases/hex.toml");
    settle_drawn(&mut app, &ctx);
    out.push(drawn(&mut app, &ctx));
    // the hover over the disabled Run of the toggle
    let pos = app.st.view.rects["Run"].center();
    let want = no_run_of("cases/hex.toml");
    let mut hover = false;
    for _ in 0..30 {
        let t = lj::texts(&lj::frame_with(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]));
        if t.contains(&want) {
            hover = true;
            break;
        }
    }
    for (i, d) in out.iter().enumerate() {
        eprintln!(
            "O2{} step {i}: highlight {:?}, label {:?}, side {:?}, Run enabled {}",
            if unlinked { " control" } else { "" },
            d.highlight,
            d.label,
            d.side,
            d.run_enabled
        );
    }
    eprintln!("O2 (d) the hover drawn: {hover}");
    (runs, out, hover)
}

/// O2: the same through the drawn window: the Project panel's highlight, the case right of Run and the toggle follow a
/// click on a case and a click on a run's row; the Run side's caption names the run; a case without runs leaves the
/// toggle's Run disabled and its hover says so for that case.
#[test]
fn o2_the_drawn_window_agrees_on_the_case() {
    let (r, d, hover) = o2("o2", false);
    let one = |c: &str| vec![c.to_string()];
    let cav = one("cases/cavity.toml");
    assert_eq!(
        (&d[0].highlight, &d[0].label, d[0].side),
        (&cav, &cav, Side::Mesh),
        "(a)"
    );
    assert!(
        d[0].run_enabled,
        "(a) the toggle's Run is disabled with a run of the case"
    );
    assert_eq!(
        (&d[1].highlight, &d[1].label, d[1].side),
        (&cav, &cav, Side::Run),
        "(b)"
    );
    let caption = |id: &str| format!("stored fields of run {id}");
    assert!(d[1].texts.contains(&caption(&r.r2)), "(b) the caption of r2 not drawn");
    let b = one("cases/b.toml");
    assert_eq!((&d[2].highlight, &d[2].label, d[2].side), (&b, &b, Side::Run), "(c)");
    assert!(d[2].texts.contains(&caption(&r.r3)), "(c) the caption of r3 not drawn");
    assert!(
        d[2].texts.iter().any(|t| t.starts_with("case cases/b.toml")),
        "(c) the readout does not name b"
    );
    let hex = one("cases/hex.toml");
    assert_eq!(
        (&d[3].highlight, &d[3].label, d[3].side),
        (&hex, &hex, Side::Mesh),
        "(d)"
    );
    assert!(
        !d[3].run_enabled,
        "(d) the toggle's Run is enabled for a case without a run"
    );
    assert!(hover, "(d) the Run side's refusal for hex not drawn on hover");
}

/// O2 control: ADR 0148's route: after (c) the highlight and the label stay on the cavity while the Run side shows r3.
#[test]
fn o2_control_the_drawn_selections_were_apart() {
    let (r, d, _) = o2("o2ctl", true);
    let cav = vec!["cases/cavity.toml".to_string()];
    assert_eq!((&d[2].highlight, &d[2].label), (&cav, &cav), "control (c)");
    assert!(
        d[2].texts.contains(&format!("stored fields of run {}", r.r3)),
        "control (c): r3 not shown"
    );
}

// ---------------------------------------------------------------- O3: runs started, two runs of one case

/// A served project with `cases/a.toml` and `cases/c.toml` (the 12 x 12 cavity of ADR 0133 at `nx` 8 and 14, 30
/// iterations; ADR 0148's N1 fixture).
fn served(tag: &str) -> RunnerProject {
    let mut p = RunnerProject::new(tag, 3);
    for (rel, nx) in [("cases/a.toml", 8), ("cases/c.toml", 14)] {
        let t = g4::cavity(30).replace("nx = 12", &format!("nx = {nx}"));
        assert!(t.contains(&format!("nx = {nx}")));
        std::fs::write(p.dir.join(rel), t).unwrap();
    }
    p.serve();
    p
}

/// Run as F5 asks for it; the id of the run it queued (after the start ended), or the refusal.
fn run_now(st: &mut Studio, then: Option<&str>) -> Result<String, String> {
    let before = st.quick.run.as_ref().map(|r| r.0.clone());
    quick_run::run_selected_case(st)?;
    if let Some(c) = then {
        st.open_case(c);
    }
    assert!(
        st.drain_until(WAIT, |s| s.quick.starting.is_none()),
        "the start did not end"
    );
    match st.quick.run.as_ref().map(|r| r.0.clone()) {
        Some(id) if Some(&id) != before.as_ref() => Ok(id),
        _ => Err(st.quick.error.clone().unwrap_or_default()),
    }
}

/// O3's procedure (served project, the Studio model with the watcher and the run worker on; the Run button searches the
/// project's runner): (a) a selected, Run: A1; (b) Run again: A2; (c) a clicked; (d) A1 clicked in the Runs panel; (e)
/// Run, and c clicked at once: A3; (f) A3's end; (g) a clicked.
#[allow(clippy::type_complexity)]
fn o3(tag: &str, unlinked: bool) -> (Vec<String>, Vec<(&'static str, Seen)>, bool, Vec<String>) {
    let p = served(tag);
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.quick.switches.case_run_unlinked = unlinked;
    st.open_project(&p.dir);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()));
    let mut out = Vec::new();
    let mut step = |name: &'static str, st: &mut Studio| {
        settle(st);
        let v = seen(st);
        eprintln!("O3{} {name}: {v:?}", if unlinked { " control" } else { "" });
        out.push((name, v));
    };
    st.open_case("cases/a.toml");
    settle(&mut st);
    let a1 = run_now(&mut st, None).expect("(a) Run of a");
    step("(a) Run of a", &mut st);
    let a2 = run_now(&mut st, None).expect("(b) Run of a again");
    step("(b) Run of a again", &mut st);
    // both runs listed by the run worker before the case click picks the newest
    assert!(st.drain_until(WAIT, |s| s.runs.rows.contains_key(&a1) && s.runs.rows.contains_key(&a2)));
    st.open_case("cases/a.toml");
    step("(c) a clicked", &mut st);
    st.open_run(&a1);
    step("(d) A1 clicked", &mut st);
    let n = st.messages.len();
    let a3 = run_now(&mut st, Some("cases/c.toml")).expect("(e) Run of a");
    step("(e) Run of a, c clicked at once", &mut st);
    let said: Vec<String> = st.messages[n..].to_vec();
    let finished = |id: &str| p.status(id).is_some_and(|s| s.state == RunState::Finished);
    let ended = st.drain_until(Duration::from_secs(60), |s| {
        finished(&a3) && s.runs.rows.contains_key(&a3)
    });
    step("(f) A3 ended", &mut st);
    st.open_case("cases/a.toml");
    step("(g) a clicked", &mut st);
    (vec![a1, a2, a3], out, ended, said)
}

/// O3: two runs of one case and a run started while another case was selected: Run's run is selected and shown (ADR
/// 0141); a click on the case selects its newest run; a click on the older run selects it; a run that Run queued after
/// another case was selected is not selected (the Viewport stays on that case's mesh, ADR 0148 R4), runs to its end, and
/// is the case's newest run when the case is clicked.
#[test]
fn o3_runs_of_one_case_and_a_run_started_while_another_case_is_selected() {
    let (ids, out, ended, said) = o3("o3", false);
    let (a1, a2, a3) = (&ids[0], &ids[1], &ids[2]);
    let get = |name: &str| out.iter().find(|(n, _)| *n == name).cloned().unwrap().1;
    for (name, v) in &out {
        assert!(v.disagree.is_empty(), "{name}: the parts disagree: {:?}", v.disagree);
    }
    let want = |name: &str, case: &str, run: Option<&String>, side: Side| {
        let v = get(name);
        assert_eq!(
            (v.case.as_deref(), v.run.as_ref(), v.side),
            (Some(case), run, side),
            "{name}"
        );
    };
    want("(a) Run of a", "cases/a.toml", Some(a1), Side::Run);
    want("(b) Run of a again", "cases/a.toml", Some(a2), Side::Run);
    want("(c) a clicked", "cases/a.toml", Some(a2), Side::Mesh);
    want("(d) A1 clicked", "cases/a.toml", Some(a1), Side::Run);
    want("(e) Run of a, c clicked at once", "cases/c.toml", None, Side::Mesh);
    assert_eq!(
        get("(e) Run of a, c clicked at once").preview.as_deref(),
        Some("cases/c.toml")
    );
    assert!(
        said.iter()
            .any(|m| m.starts_with(&format!("queued run {a3} of cases/a.toml"))),
        "(e) the status bar: {said:?}"
    );
    assert!(ended, "(f) A3 did not finish within 60 s");
    want("(f) A3 ended", "cases/c.toml", None, Side::Mesh);
    want("(g) a clicked", "cases/a.toml", Some(a3), Side::Mesh);
}

/// O3 control: ADR 0148's rule (`case_run_unlinked`): after (e) the run of a is selected while c is the selected case.
#[test]
fn o3_control_the_queued_run_was_selected() {
    let (ids, out, ..) = o3("o3ctl", true);
    let e = out
        .iter()
        .find(|(n, _)| *n == "(e) Run of a, c clicked at once")
        .cloned()
        .unwrap()
        .1;
    assert_eq!(
        (e.case.as_deref(), e.run.as_ref()),
        (Some("cases/c.toml"), Some(&ids[2])),
        "control (e)"
    );
}
