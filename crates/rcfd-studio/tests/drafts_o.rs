//! ADR 0149 (job 48O) O4-O9: unsaved edits are kept per case (the maintainer's answer to ADR 0148 question 2). Leaving a
//! case keeps its editor with its unsaved state (an edit, a field being typed, a conflict); coming back restores it at
//! once and compares the file read again (ADR 0131's rule, a typed field counted as an edit); the Project panel marks
//! the cases with unsaved state; Run and the Start dialog refuse such a case; the project does not close without asking;
//! the number kept is bounded; a field typed and left in the drawn window is not lost. Negative controls: ADR 0148's
//! route (`QuickRun::switches.drop_drafts`). Release builds, `--test-threads 1 --nocapture`.

mod common;
mod lj;
mod pv;

use rcfd_studio::case_edit::EditorState;
use rcfd_studio::quick_run;
use rcfd_studio::schema_model::value_at;
use rcfd_studio::studio::{AfterClose, CloseAnswer, MAX_PARKED};
use rcfd_studio::ui::{CLOSE_CANCEL, CLOSE_DISCARD, CLOSE_SAVE, UNSAVED_HOVER, UNSAVED_MARK};
use rcfd_studio::{Studio, StudioApp};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

const SAVE_FIRST: &str = "has unsaved edits: save it first (Case > Save); a run uses the file on disk";

/// ADR 0138's project (`cases/cavity.toml` 16 x 16, `cases/hex.toml`) with `cases/b.toml` (the cavity at 10 x 10) and
/// `cases/c.toml` (12 x 12).
fn project(tag: &str) -> PathBuf {
    pv::project(
        tag,
        &[
            ("cases/b.toml", lj::cavity(10, 10).as_bytes()),
            ("cases/c.toml", lj::cavity(12, 12).as_bytes()),
        ],
    )
}

/// The Studio model on `root` (watcher and run worker off), with `drop_drafts` (the route before) when asked.
fn studio(root: &Path, drop_drafts: bool) -> Studio {
    let mut st = lj::studio(root, false);
    st.quick.switches.drop_drafts = drop_drafts;
    st
}

/// Drain until nothing is being read and the preview settled.
fn settle(st: &mut Studio) {
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none() && lj::settled(s)),
        "did not settle: pending {:?}, preview {:?}",
        st.pending_case,
        st.preview.status
    );
}

/// Select `rel` (a click in the Project panel) and settle.
fn select(st: &mut Studio, rel: &str) {
    st.open_case(rel);
    settle(st);
}

fn value(st: &Studio, path: &str) -> Option<Value> {
    st.editor.as_ref().and_then(|e| value_at(&e.settings, path).cloned())
}

fn commit(st: &mut Studio, path: &str, v: Value) {
    st.editor.as_mut().expect("a case is open").set_value(path, v).unwrap();
}

/// The editor's case, state, draft and `mesh.nx`.
fn editor_of(st: &Studio) -> EditorOf {
    (
        st.editor.as_ref().map(|e| e.rel.clone()),
        st.editor_state(),
        st.editor.as_ref().and_then(|e| e.draft.clone()),
        value(st, "mesh.nx"),
    )
}

/// The editor's case, state, draft and `mesh.nx` ([`editor_of`]).
type EditorOf = (
    Option<String>,
    Option<EditorState>,
    Option<(String, String)>,
    Option<Value>,
);

/// O8's results: the refusal, the selected case, the cases kept at the refusal, after the cavity, after its save and hex,
/// and the largest kept settings tree in bytes.
type O8 = (
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    usize,
);

// ---------------------------------------------------------------- O4: kept and taken back

/// What O4 saw.
#[derive(Debug, Default)]
struct O4 {
    /// (b) right after b was selected: the cases kept, the editor's case, `editor_blocks` of the cavity.
    b_kept: Vec<String>,
    b_editor: Option<String>,
    b_cavity_blocks: Option<String>,
    b_run: Option<String>,
    /// (d) after hex's save met a conflict and the cavity was selected: the cases with unsaved state.
    d_unsaved: Vec<String>,
    /// (e) the cavity, right after its click (before any drain) and settled.
    e_now: EditorOf,
    e_settled: EditorOf,
    e_run: Option<String>,
    e_cells: Option<usize>,
    e_from_editor: bool,
    e_b_blocks: Option<String>,
    e_hex_blocks: Option<String>,
    /// (f) b taken back.
    f: EditorOf,
    f_run: Option<String>,
    f_cells: Option<usize>,
    /// (g) hex taken back: its state and `physics.nu`.
    g_state: Option<EditorState>,
    g_nu: Option<Value>,
    /// (h) hex clicked again: the editor right after the click.
    h_now: Option<EditorState>,
}

/// O4's procedure: (a) the cavity opened, `mesh.nx` 20 committed; (b) b selected; (c) `mesh.ny` of b typed "12", hex
/// selected; (d) hex's `physics.nu` 0.02 committed, the file replaced on disk, saved (a save conflict), the cavity
/// selected; (e) the cavity's editor right after the click and settled; (f) b selected; (g) hex selected; (h) hex clicked
/// again.
fn o4(tag: &str, drop_drafts: bool) -> O4 {
    let root = project(tag);
    let mut st = studio(&root, drop_drafts);
    let mut r = O4::default();
    // (a)
    select(&mut st, "cases/cavity.toml");
    commit(&mut st, "mesh.nx", json!(20));
    settle(&mut st);
    // (b)
    st.open_case("cases/b.toml");
    r.b_kept = st.parked.keys().cloned().collect();
    r.b_editor = st.editor.as_ref().map(|e| e.rel.clone());
    r.b_cavity_blocks = quick_run::editor_blocks(&st, "cases/cavity.toml");
    settle(&mut st);
    r.b_run = quick_run::blocked(&st);
    // (c)
    lj::type_text(&mut st, "mesh.ny", "12");
    select(&mut st, "cases/hex.toml");
    // (d)
    commit(&mut st, "physics.nu", json!(0.02));
    let disk = pv::HEX_CASE.replace("max_iterations = 5", "max_iterations = 6");
    std::fs::write(root.join("cases/hex.toml"), &disk).unwrap();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| e.in_conflict())),
        "set-up (d): the save met no conflict"
    );
    st.open_case("cases/cavity.toml");
    r.d_unsaved = st.unsaved_cases();
    // (e)
    r.e_now = editor_of(&st);
    settle(&mut st);
    r.e_settled = editor_of(&st);
    r.e_run = quick_run::blocked(&st);
    r.e_cells = st.preview.shown.as_ref().map(|b| b.cells);
    r.e_from_editor = st.preview.shown.as_ref().is_some_and(|b| b.from_editor);
    r.e_b_blocks = quick_run::editor_blocks(&st, "cases/b.toml");
    r.e_hex_blocks = quick_run::editor_blocks(&st, "cases/hex.toml");
    // (f)
    select(&mut st, "cases/b.toml");
    r.f = editor_of(&st);
    r.f_run = quick_run::blocked(&st);
    r.f_cells = st.preview.shown.as_ref().filter(|b| b.from_editor).map(|b| b.cells);
    // (g)
    select(&mut st, "cases/hex.toml");
    r.g_state = st.editor_state();
    r.g_nu = value(&st, "physics.nu");
    // (h)
    st.open_case("cases/hex.toml");
    r.h_now = st.editor_state();
    settle(&mut st);
    eprintln!("O4{}: {r:#?}", if drop_drafts { " control" } else { "" });
    r
}

/// O4: the unsaved state of a case left (an edit, a field being typed, a conflict) is kept and comes back as it was, at
/// once; Run and the Start dialog's rule hold for a kept case; the preview of the case taken back is its editor's text.
#[test]
fn o4_unsaved_state_is_kept_per_case() {
    let r = o4("o4", false);
    let cav = Some("cases/cavity.toml".to_string());
    assert_eq!(r.b_kept, vec!["cases/cavity.toml".to_string()], "(b) kept");
    assert_eq!(r.b_editor, None, "(b) the editor holds a case before b was read");
    assert_eq!(
        r.b_cavity_blocks.as_deref(),
        Some(format!("cases/cavity.toml {SAVE_FIRST}").as_str()),
        "(b) the cavity's refusal"
    );
    assert_eq!(r.b_run, None, "(b) Run of b held up by the cavity");
    assert_eq!(
        r.d_unsaved,
        ["cases/b.toml", "cases/cavity.toml", "cases/hex.toml"].map(String::from),
        "(d) the cases with unsaved state"
    );
    let dirty_20 = (cav.clone(), Some(EditorState::Dirty), None, Some(json!(20)));
    assert_eq!(r.e_now, dirty_20, "(e) at the click");
    assert_eq!(r.e_settled, dirty_20, "(e) settled (the file is unchanged)");
    assert_eq!(
        r.e_run.as_deref(),
        Some(format!("cases/cavity.toml {SAVE_FIRST}").as_str()),
        "(e) Run"
    );
    assert_eq!((r.e_cells, r.e_from_editor), (Some(320), true), "(e) the preview");
    assert!(
        r.e_b_blocks
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/b.toml: finish editing the field mesh.ny")),
        "(e) b's refusal: {:?}",
        r.e_b_blocks
    );
    assert_eq!(
        r.e_hex_blocks.as_deref(),
        Some("cases/hex.toml: resolve the conflict in the case editor first"),
        "(e) hex's refusal"
    );
    assert_eq!(
        r.f,
        (
            Some("cases/b.toml".to_string()),
            Some(EditorState::Clean),
            Some(("mesh.ny".to_string(), "12".to_string())),
            Some(json!(10))
        ),
        "(f) b taken back"
    );
    assert!(
        r.f_run
            .as_deref()
            .is_some_and(|t| t.starts_with("cases/b.toml: finish editing the field mesh.ny")),
        "(f) Run: {:?}",
        r.f_run
    );
    assert_eq!(r.f_cells, Some(120), "(f) the preview of the typed text (10 x 12)");
    assert!(
        matches!(r.g_state, Some(EditorState::Conflict { disk: Some(_) })),
        "(g) {:?}",
        r.g_state
    );
    assert_eq!(r.g_nu, Some(json!(0.02)), "(g) the edit");
    assert!(
        matches!(r.h_now, Some(EditorState::Conflict { .. })),
        "(h) the same case clicked again: {:?}",
        r.h_now
    );
}

/// O4 control: ADR 0148's route (`drop_drafts`): the cavity comes back from its file (16, clean); nothing is kept.
#[test]
fn o4_control_the_edits_were_dropped() {
    let r = o4("o4ctl", true);
    assert!(r.b_kept.is_empty(), "control (b)");
    assert_eq!(r.e_settled.3, Some(json!(16)), "control (e)");
    assert_eq!(r.e_settled.1, Some(EditorState::Clean), "control (e)");
}

// ---------------------------------------------------------------- O5: the file changed while kept

/// O5's procedure: the cavity (`mesh.nx` 20 committed), b (`mesh.ny` typed "12") and c (`mesh.nx` 14 committed) left
/// with their state, hex selected; then the cavity's and b's files replaced on disk and c's removed; each selected again.
/// Returns per case (the editor's state, `mesh.nx`, `mesh.ny`, the draft) and the last message.
#[allow(clippy::type_complexity)]
fn o5(
    tag: &str,
    drop_drafts: bool,
) -> Vec<(
    String,
    Option<EditorState>,
    Option<Value>,
    Option<Value>,
    Option<(String, String)>,
    Option<String>,
)> {
    let root = project(tag);
    let mut st = studio(&root, drop_drafts);
    select(&mut st, "cases/cavity.toml");
    commit(&mut st, "mesh.nx", json!(20));
    select(&mut st, "cases/b.toml");
    lj::type_text(&mut st, "mesh.ny", "12");
    select(&mut st, "cases/c.toml");
    commit(&mut st, "mesh.nx", json!(14));
    select(&mut st, "cases/hex.toml");
    std::fs::write(root.join("cases/cavity.toml"), lj::cavity(24, 16)).unwrap();
    std::fs::write(root.join("cases/b.toml"), lj::cavity(10, 14)).unwrap();
    std::fs::remove_file(root.join("cases/c.toml")).unwrap();
    let mut out = Vec::new();
    for rel in ["cases/cavity.toml", "cases/b.toml", "cases/c.toml"] {
        st.open_case(rel);
        assert!(
            st.drain_until(WAIT, |s| s.pending_case.is_none()),
            "{rel}: the read did not end"
        );
        let ed = st.editor.as_ref();
        let s = (
            rel.to_string(),
            st.editor_state(),
            value(&st, "mesh.nx"),
            value(&st, "mesh.ny"),
            ed.and_then(|e| e.draft.clone()),
            st.last_message().map(String::from),
        );
        eprintln!("O5{} {s:?}", if drop_drafts { " control" } else { "" });
        out.push(s);
    }
    out
}

/// O5: a file changed or removed while its case was kept: when the case is selected again its editor is in ADR 0131's
/// conflict state with the edits and the typed text kept, the disk version offered (none for a removed file).
#[test]
fn o5_a_file_changed_while_kept_is_a_conflict_on_return() {
    let o = o5("o5", false);
    match &o[0].1 {
        Some(EditorState::Conflict { disk: Some(d) }) => {
            assert_eq!(d, lj::cavity(24, 16).as_bytes(), "(cavity) the disk version offered")
        }
        other => panic!("(cavity) not in conflict with the file's bytes: {other:?}"),
    }
    assert_eq!(o[0].2, Some(json!(20)), "(cavity) the edit");
    assert_eq!(
        o[0].5.as_deref(),
        Some("cases/cavity.toml: changed on disk while its edits were kept; resolve the conflict"),
        "(cavity) the message"
    );
    match &o[1].1 {
        Some(EditorState::Conflict { disk: Some(d) }) => {
            assert_eq!(d, lj::cavity(10, 14).as_bytes(), "(b) the disk version offered")
        }
        other => panic!("(b) not in conflict with the file's bytes: {other:?}"),
    }
    assert_eq!(
        o[1].4,
        Some(("mesh.ny".to_string(), "12".to_string())),
        "(b) the typed text"
    );
    assert_eq!(
        o[1].3,
        Some(json!(10)),
        "(b) the settings (the typed text is not committed)"
    );
    assert!(
        matches!(&o[2].1, Some(EditorState::Conflict { disk: None })),
        "(c) not in conflict without a file: {:?}",
        o[2].1
    );
    assert_eq!(o[2].2, Some(json!(14)), "(c) the edit");
}

/// O5 control: ADR 0148's route: the cavity comes back from its new file (24, clean, no conflict).
#[test]
fn o5_control_the_new_file_was_read() {
    let o = o5("o5ctl", true);
    assert_eq!(
        (o[0].1.clone(), o[0].2.clone()),
        (Some(EditorState::Clean), Some(json!(24))),
        "control"
    );
}

/// O5, reported (ADR 0131's rule without a typed field counted as an edit, `CaseEditor::external_change`): a typed field
/// of a clean editor is dropped by the reload of another file. Not a control of the code before (that dropped it at the
/// click already); it shows why a typed field counts as an edit on return.
#[test]
fn o5_report_adr0131_rule_drops_a_typed_field() {
    let root = project("o5rep");
    let mut st = studio(&root, false);
    select(&mut st, "cases/b.toml");
    lj::type_text(&mut st, "mesh.ny", "12");
    let ed = st.editor.as_mut().unwrap();
    let r = ed.external_change(Some(lj::cavity(10, 14).into_bytes()));
    eprintln!("O5 report: external_change of a typed b: {r:?}, draft {:?}", ed.draft);
    assert_eq!(ed.draft, None, "report: ADR 0131's rule keeps the typed text");
}

// ---------------------------------------------------------------- O6: drawn marks, the Start dialog, Discard

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

/// The drawn app (watcher off; run worker on when `runs`; tooltips at once).
fn app_on(root: &Path, runs: bool, drop_drafts: bool) -> (StudioApp, egui::Context) {
    let mut st = lj::studio(root, runs);
    st.quick.switches.drop_drafts = drop_drafts;
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    for _ in 0..3 {
        let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    }
    (app, ctx)
}

/// The Project panel's case labels drawn with a mark after them, in the frame `out`.
fn marked(out: &egui::FullOutput) -> Vec<String> {
    let t = texts_at(out);
    t.iter()
        .filter(|(s, r)| s.starts_with("cases/") && r.center().x < 400.0)
        .filter(|(_, r)| {
            t.iter()
                .any(|(m, mr)| m == UNSAVED_MARK && (mr.center().y - r.center().y).abs() < 6.0 && mr.min.x >= r.max.x)
        })
        .map(|(s, _)| s.clone())
        .collect()
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

fn settle_drawn(app: &mut StudioApp, ctx: &egui::Context) {
    assert!(
        common::frames_until(app, ctx, WAIT, |a| a.st.pending_case.is_none() && lj::settled(&a.st)),
        "did not settle: {:?}",
        app.st.preview.status
    );
}

/// What O6 saw.
#[derive(Debug, Default)]
struct O6 {
    b_marked: Vec<String>,
    b_hover: bool,
    start: Option<String>,
    run_b: Option<String>,
    d_marked: Vec<String>,
    d_source: bool,
    e_marked: Vec<String>,
    e_state: (Option<EditorState>, Option<Value>),
    e_source: bool,
    e_cells: Option<usize>,
}

/// O6's procedure (the drawn app, run worker on): (a) a click on the cavity, `mesh.nx` 20 committed; (b) a click on b:
/// the marks drawn, the hover over the cavity's mark; (c) the Start dialog of the cavity: its Start; (d) a click on the
/// cavity: the marks and the Mesh side's source line; (e) a click on "Discard edits".
fn o6(tag: &str, drop_drafts: bool) -> O6 {
    let root = project(tag);
    let (mut app, ctx) = app_on(&root, true, drop_drafts);
    let mut r = O6::default();
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    commit(&mut app.st, "mesh.nx", json!(20));
    settle_drawn(&mut app, &ctx);
    click_case(&mut app, &ctx, "cases/b.toml");
    settle_drawn(&mut app, &ctx);
    let out = lj::frame_with(&mut app, &ctx, Vec::new());
    r.b_marked = marked(&out);
    if let Some((_, m)) = texts_at(&out).into_iter().find(|(t, _)| t == UNSAVED_MARK) {
        for _ in 0..30 {
            let t = lj::texts(&lj::frame_with(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(m.center())],
            ));
            if t.iter().any(|x| x == UNSAVED_HOVER) {
                r.b_hover = true;
                break;
            }
        }
    }
    // (c) the Start dialog of the cavity while b is selected
    app.ui_state.start_dialog = Some(rcfd_studio::ui::StartDialog {
        case: "cases/cavity.toml".into(),
        ..Default::default()
    });
    app.ui_state.start_button = None;
    assert!(
        common::frames_until(&mut app, &ctx, WAIT, |a| a.ui_state.start_button.is_some()),
        "(c) the dialog's Start was not drawn"
    );
    r.start = app.ui_state.start_button.as_ref().unwrap().1.clone();
    r.run_b = quick_run::blocked(&app.st);
    app.ui_state.start_dialog = None;
    // (d)
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    let out = lj::frame_with(&mut app, &ctx, Vec::new());
    r.d_marked = marked(&out);
    r.d_source = lj::texts(&out)
        .iter()
        .any(|t| t == "(built from the unsaved edits in the case editor; the solver is not run)");
    // (e) Discard edits
    let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    if let Some(rect) = app.ui_state.discard_rect {
        lj::click_at(&mut app, &ctx, rect.center());
        settle_drawn(&mut app, &ctx);
        let out = lj::frame_with(&mut app, &ctx, Vec::new());
        r.e_marked = marked(&out);
        r.e_source = lj::texts(&out)
            .iter()
            .any(|t| t == "(built from the case file on disk; the solver is not run)");
        r.e_state = (app.st.editor_state(), value(&app.st, "mesh.nx"));
        r.e_cells = app.st.preview.shown.as_ref().map(|b| b.cells);
    }
    eprintln!("O6{}: {r:#?}", if drop_drafts { " control" } else { "" });
    r
}

/// O6: the Project panel marks each case with unsaved state (the one kept and the one open), with a hover text; the
/// Start dialog refuses a kept case with Run's text while Run of the selected case is free; the case taken back shows
/// its edits' mesh; "Discard edits" drops them and reads the file.
#[test]
fn o6_marks_the_start_dialog_and_discard() {
    let r = o6("o6", false);
    let cav = vec!["cases/cavity.toml".to_string()];
    assert_eq!(r.b_marked, cav, "(b) the marked cases");
    assert!(r.b_hover, "(b) the mark's hover text not drawn");
    assert_eq!(
        r.start.as_deref(),
        Some(format!("cases/cavity.toml {SAVE_FIRST}").as_str()),
        "(c) the dialog's Start for the kept cavity"
    );
    assert_eq!(r.run_b, None, "(c) Run of b");
    assert_eq!(r.d_marked, cav, "(d) the open case with unsaved edits is marked");
    assert!(r.d_source, "(d) the Mesh side does not say it shows the edits");
    assert!(r.e_marked.is_empty(), "(e) marks after Discard: {:?}", r.e_marked);
    assert_eq!(r.e_state, (Some(EditorState::Clean), Some(json!(16))), "(e)");
    assert!(r.e_source, "(e) the source line is not the file's");
    assert_eq!(r.e_cells, Some(256), "(e) the file's mesh");
}

/// O6 control: ADR 0148's route: no mark after the click on b, and the dialog's Start of the cavity is enabled.
#[test]
fn o6_control_nothing_was_kept() {
    let r = o6("o6ctl", true);
    assert!(r.b_marked.is_empty(), "control (b)");
    assert_eq!(r.start, None, "control (c)");
}

// ---------------------------------------------------------------- O7: closing with unsaved edits

/// The cavity with `mesh.nx` 20 committed and left for b, whose `mesh.ny` is typed "12".
fn two_drafts(st: &mut Studio) {
    select(st, "cases/cavity.toml");
    commit(st, "mesh.nx", json!(20));
    select(st, "cases/b.toml");
    lj::type_text(st, "mesh.ny", "12");
}

fn file_value(root: &Path, rel: &str, path: &str) -> Option<Value> {
    rcfd_api::case::Case::load(root.join(rel))
        .ok()
        .and_then(|c| c.get(path).ok())
}

/// O7 (a)-(c): the question, Cancel and Discard (model).
#[test]
fn o7_close_asks_and_cancel_or_discard() {
    let root = project("o7a");
    let other = pv::project("o7other", &[]);
    let mut st = studio(&root, false);
    two_drafts(&mut st);
    // (a)
    let closed = st.request_close(AfterClose::Nothing);
    let ask = st.close_ask.clone();
    eprintln!("O7 (a): closed {closed}; {ask:?}");
    assert!(!closed, "(a) closed without asking");
    assert_eq!(
        ask.as_ref().map(|a| a.cases.clone()),
        Some(vec!["cases/b.toml".to_string(), "cases/cavity.toml".to_string()]),
        "(a) the cases named"
    );
    assert!(st.browser().is_some(), "(a) the project closed");
    // (b) Cancel
    st.answer_close(CloseAnswer::Cancel);
    assert!(st.close_ask.is_none() && st.browser().is_some(), "(b) Cancel closed");
    assert_eq!(
        st.unsaved_cases(),
        ["cases/b.toml", "cases/cavity.toml"].map(String::from),
        "(b)"
    );
    // (c) another project asked for, Discard
    assert!(!st.request_close(AfterClose::Open(other.clone())), "(c) not asked");
    st.answer_close(CloseAnswer::Discard);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some_and(|b| b.root == other)),
        "(c) the other project did not open"
    );
    assert!(st.unsaved_cases().is_empty() && st.parked.is_empty(), "(c) edits left");
    assert_eq!(
        file_value(&root, "cases/cavity.toml", "mesh.nx"),
        Some(json!(16)),
        "(c) a file was written"
    );
    eprintln!(
        "O7 (b), (c): Cancel kept everything; Discard closed and opened {}",
        other.display()
    );
}

/// O7 (d)-(g): Save all and close (model): the typed field committed and every case saved, then closed; a refused
/// typed field, a case in conflict and a save that meets a conflict each stop the close with its reason.
#[test]
fn o7_save_all_and_its_refusals() {
    let root = project("o7d");
    let mut st = studio(&root, false);
    // (d) the cavity (edit kept), b (typed, kept), hex (`physics.nu` 0.02 committed, open)
    two_drafts(&mut st);
    select(&mut st, "cases/hex.toml");
    commit(&mut st, "physics.nu", json!(0.02));
    assert!(!st.request_close(AfterClose::Nothing));
    st.answer_close(CloseAnswer::SaveAll);
    assert!(
        st.drain_until(WAIT, |s| s.project.is_none()),
        "(d) not closed: {:?}",
        st.close_ask
    );
    let (nx, ny, nu) = (
        file_value(&root, "cases/cavity.toml", "mesh.nx"),
        file_value(&root, "cases/b.toml", "mesh.ny"),
        file_value(&root, "cases/hex.toml", "physics.nu"),
    );
    eprintln!("O7 (d): closed; the files hold cavity mesh.nx {nx:?}, b mesh.ny {ny:?}, hex physics.nu {nu:?}");
    assert_eq!(
        (nx, ny, nu),
        (Some(json!(20)), Some(json!(12)), Some(json!(0.02))),
        "(d)"
    );
    // (e) a refused typed field
    let mut st = studio(&root, false);
    select(&mut st, "cases/c.toml");
    lj::type_text(&mut st, "mesh.nx", "1e");
    select(&mut st, "cases/cavity.toml");
    let before = std::fs::read(root.join("cases/c.toml")).unwrap();
    assert!(!st.request_close(AfterClose::Nothing));
    st.answer_close(CloseAnswer::SaveAll);
    lj::drain_for(&mut st, Duration::from_millis(500));
    let e = st.close_ask.as_ref().and_then(|a| a.error.clone());
    eprintln!("O7 (e): {e:?}");
    assert_eq!(
        e.as_deref(),
        Some("cases/c.toml: mesh.nx: '1e' is not a whole number: correct the field, or close with Discard"),
        "(e)"
    );
    assert!(st.browser().is_some(), "(e) closed");
    assert_eq!(
        std::fs::read(root.join("cases/c.toml")).unwrap(),
        before,
        "(e) c was written"
    );
    st.answer_close(CloseAnswer::Cancel);
    // (f) a case in conflict
    let mut st = studio(&root, false);
    select(&mut st, "cases/hex.toml");
    commit(&mut st, "physics.nu", json!(0.03));
    std::fs::write(root.join("cases/hex.toml"), pv::HEX_CASE).unwrap();
    st.save_case().unwrap();
    assert!(st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| e.in_conflict())));
    assert!(!st.request_close(AfterClose::Nothing));
    st.answer_close(CloseAnswer::SaveAll);
    let f = st.close_ask.as_ref().and_then(|a| a.error.clone());
    eprintln!("O7 (f): {f:?}");
    assert_eq!(
        f.as_deref(),
        Some(
            "cases/hex.toml: resolve the conflict in the case editor first (Keep my edits or Take the disk version), or \
             close with Discard"
        ),
        "(f)"
    );
    assert!(st.browser().is_some(), "(f) closed");
    st.answer_close(CloseAnswer::Cancel);
    // (g) a save that meets a conflict: the cavity's file replaced on disk while its edit is kept (watcher off)
    let mut st = studio(&root, false);
    select(&mut st, "cases/cavity.toml");
    commit(&mut st, "mesh.nx", json!(22));
    select(&mut st, "cases/b.toml");
    commit(&mut st, "mesh.nx", json!(11));
    std::fs::write(root.join("cases/cavity.toml"), lj::cavity(26, 16)).unwrap();
    assert!(!st.request_close(AfterClose::Nothing));
    st.answer_close(CloseAnswer::SaveAll);
    assert!(
        st.drain_until(WAIT, |s| s.close_ask.as_ref().is_some_and(|a| a.saving.is_empty())),
        "(g) the saves did not end"
    );
    let g = st.close_ask.as_ref().and_then(|a| a.error.clone());
    let kept = st.case_editor("cases/cavity.toml").map(|e| e.state());
    eprintln!(
        "O7 (g): {g:?}; the cavity's editor {kept:?}; b on disk mesh.nx {:?}",
        file_value(&root, "cases/b.toml", "mesh.nx")
    );
    assert_eq!(
        g.as_deref(),
        Some("cases/cavity.toml: not saved, the file changed on disk; resolve the conflict"),
        "(g)"
    );
    assert!(st.browser().is_some(), "(g) closed");
    assert!(
        matches!(kept, Some(EditorState::Conflict { .. })),
        "(g) the cavity: {kept:?}"
    );
    assert_eq!(
        file_value(&root, "cases/b.toml", "mesh.nx"),
        Some(json!(11)),
        "(g) b was not saved"
    );
}

/// A frame whose input carries the window's close request (as the window sends it) and `events`; the viewport commands
/// it gave.
fn close_frame_with(
    app: &mut StudioApp,
    ctx: &egui::Context,
    close: bool,
    events: Vec<egui::Event>,
) -> Vec<egui::ViewportCommand> {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        events,
        ..Default::default()
    };
    if close {
        input.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                events: vec![egui::ViewportEvent::Close],
                ..Default::default()
            },
        );
    }
    let out = ctx.run_ui(input, |ui| app.ui(ui));
    out.viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|v| v.commands.clone())
        .unwrap_or_default()
}

fn close_frame(app: &mut StudioApp, ctx: &egui::Context, close: bool) -> Vec<egui::ViewportCommand> {
    close_frame_with(app, ctx, close, Vec::new())
}

/// A click (press, then release) at `pos`; the viewport commands of the release frame and of the next frame.
fn click_commands(app: &mut StudioApp, ctx: &egui::Context, pos: egui::Pos2) -> Vec<egui::ViewportCommand> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = close_frame_with(app, ctx, false, vec![egui::Event::PointerMoved(pos), button(true)]);
    let mut c = close_frame_with(app, ctx, false, vec![button(false)]);
    c.extend(close_frame(app, ctx, false));
    c
}

/// O7 (h): the window's close (drawn): with an unsaved edit the close is cancelled and the question drawn; "Discard and
/// close" lets the window close; the next close request is not cancelled.
fn o7h(
    tag: &str,
    drop_drafts: bool,
) -> (
    Vec<egui::ViewportCommand>,
    Vec<String>,
    bool,
    Vec<egui::ViewportCommand>,
) {
    let root = project(tag);
    let (mut app, ctx) = app_on(&root, false, drop_drafts);
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    commit(&mut app.st, "mesh.nx", json!(20));
    let first = close_frame(&mut app, &ctx, true);
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn.extend(lj::texts(&lj::frame_with(&mut app, &ctx, Vec::new())));
    }
    let mut closing = false;
    if let Some(r) = app.ui_state.close_buttons.get(CLOSE_DISCARD).copied() {
        closing = click_commands(&mut app, &ctx, r.center()).contains(&egui::ViewportCommand::Close);
    }
    let again = close_frame(&mut app, &ctx, true);
    eprintln!(
        "O7 (h){}: first {first:?}; drawn the question {}; Close sent {closing}; again {again:?}",
        if drop_drafts { " control" } else { "" },
        drawn.iter().any(|t| t == "Unsaved edits")
    );
    (first, drawn, closing, again)
}

#[test]
fn o7h_the_window_close_asks() {
    let (first, drawn, closing, again) = o7h("o7h", false);
    assert!(
        first.contains(&egui::ViewportCommand::CancelClose),
        "(h) the close was not cancelled"
    );
    for t in [
        "Unsaved edits",
        "cases/cavity.toml",
        CLOSE_SAVE,
        CLOSE_DISCARD,
        CLOSE_CANCEL,
    ] {
        assert!(drawn.iter().any(|d| d == t), "(h) '{t}' not drawn");
    }
    assert!(closing, "(h) Discard did not close the window");
    assert!(
        !again.contains(&egui::ViewportCommand::CancelClose),
        "(h) the next close was cancelled"
    );
}

/// O7 control: ADR 0148's route (`drop_drafts`): the close is neither asked nor cancelled.
#[test]
fn o7_control_the_close_did_not_ask() {
    let root = project("o7ctl");
    let mut st = studio(&root, true);
    two_drafts(&mut st);
    assert!(st.request_close(AfterClose::Nothing), "control: asked");
    assert!(st.project.is_none(), "control: not closed");
    let (first, ..) = o7h("o7hctl", true);
    assert!(
        !first.contains(&egui::ViewportCommand::CancelClose),
        "control: the window close was cancelled"
    );
}

// ---------------------------------------------------------------- O8: the bound

/// O8's procedure with the bound lowered to 2: the cavity, b and c each edited and left in turn; hex selected (a third
/// would be kept); then the cavity (kept) selected; then the cavity saved and hex selected.
fn o8(tag: &str, drop_drafts: bool) -> O8 {
    let root = project(tag);
    let mut st = studio(&root, drop_drafts);
    st.max_parked = 2;
    for (rel, nx) in [("cases/cavity.toml", 20), ("cases/b.toml", 11), ("cases/c.toml", 13)] {
        select(&mut st, rel);
        commit(&mut st, "mesh.nx", json!(nx));
    }
    let n = st.messages.len();
    st.open_case("cases/hex.toml");
    let refused = st.messages.get(n).cloned();
    let selected = st.selected_case.clone();
    let kept = st.parked.keys().cloned().collect();
    select(&mut st, "cases/cavity.toml");
    let kept_after = st.parked.keys().cloned().collect();
    // amendment 2: saved when it holds unsaved edits (in ADR 0148's route it comes back clean from its file)
    if st.editor.as_ref().is_some_and(|e| e.is_dirty()) {
        st.save_case().unwrap();
        assert!(st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| !e.saving && !e.is_dirty())));
    }
    select(&mut st, "cases/hex.toml");
    let kept_last = st.parked.keys().cloned().collect();
    let size = st
        .parked
        .values()
        .map(|e| serde_json::to_string(&e.settings).map_or(0, |s| s.len()))
        .max()
        .unwrap_or(0);
    eprintln!(
        "O8{}: refused {refused:?}, selected {selected:?}, kept {kept:?}; after the cavity {kept_after:?}; after its save \
         and hex {kept_last:?}; the largest kept settings tree {size} bytes as JSON",
        if drop_drafts { " control" } else { "" }
    );
    (refused, selected, kept, kept_after, kept_last, size)
}

/// O8: at most `max_parked` cases are kept; a switch that would keep one more is refused with a message and changes
/// nothing; selecting a kept case, or leaving a saved one, is not refused. The bound in the app is 32.
#[test]
fn o8_the_number_kept_is_bounded() {
    assert_eq!(MAX_PARKED, 32);
    let (refused, selected, kept, kept_after, kept_last, _) = o8("o8", false);
    assert_eq!(
        refused.as_deref(),
        Some(
            "cases/hex.toml was not opened: 2 other cases hold unsaved edits, the most the Studio keeps; save or \
             discard the edits of one of them first (marked * in the Project panel)"
        ),
        "the refusal"
    );
    assert_eq!(selected.as_deref(), Some("cases/c.toml"), "the selection changed");
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(kept, v(&["cases/b.toml", "cases/cavity.toml"]), "kept at the refusal");
    assert_eq!(
        kept_after,
        v(&["cases/b.toml", "cases/c.toml"]),
        "kept after the cavity"
    );
    assert_eq!(
        kept_last,
        v(&["cases/b.toml", "cases/c.toml"]),
        "kept after the save and hex"
    );
}

/// O8 control: ADR 0148's route: nothing is kept, so nothing is refused.
#[test]
fn o8_control_nothing_kept_nothing_refused() {
    let (refused, selected, kept, ..) = o8("o8ctl", true);
    assert!(
        refused.as_deref().is_none_or(|m| !m.contains("was not opened")),
        "control: refused {refused:?}"
    );
    assert_eq!(selected.as_deref(), Some("cases/hex.toml"), "control");
    assert!(kept.is_empty(), "control");
}

// ---------------------------------------------------------------- O9: typed in the window, left and taken back

/// Click into the `mesh.nx` field (the cursor after its text) and type `keys`.
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
            t => vec![egui::Event::Text(t.to_string())],
        };
        let _ = lj::frame_with(app, ctx, ev);
    }
}

/// What the `mesh.nx` field shows in a frame (the text drawn inside its rectangle).
fn field_text(app: &mut StudioApp, ctx: &egui::Context) -> Option<String> {
    let out = lj::frame_with(app, ctx, Vec::new());
    let field = app.ui_state.field_rects.get("mesh.nx").copied()?;
    texts_at(&out)
        .into_iter()
        .find(|(_, r)| field.contains(r.center()))
        .map(|(t, _)| t)
}

/// O9's procedure (the drawn app, the editor filtered to `mesh.nx`): the cavity clicked; "20" typed into `mesh.nx` (two
/// Backspaces, `2`, `0`), the field still focused; a click on b's label; settled; a click on the cavity's label;
/// settled. Returns (the draft, `mesh.nx`, the state, the field's drawn text, Run's refusal, the preview's cells from
/// the editor).
#[allow(clippy::type_complexity)]
fn o9(
    tag: &str,
    drop_drafts: bool,
) -> (
    Option<(String, String)>,
    Option<Value>,
    Option<EditorState>,
    Option<String>,
    Option<String>,
    Option<usize>,
) {
    let root = project(tag);
    let (mut app, ctx) = app_on(&root, false, drop_drafts);
    app.ui_state.filter = "mesh.nx".into();
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    type_into_nx(&mut app, &ctx, &["BS", "BS", "2", "0"]);
    assert_eq!(
        app.st.editor.as_ref().unwrap().draft,
        Some(("mesh.nx".to_string(), "20".to_string())),
        "set-up: the keystrokes did not reach the field"
    );
    click_case(&mut app, &ctx, "cases/b.toml");
    settle_drawn(&mut app, &ctx);
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    let shown = field_text(&mut app, &ctx);
    let ed = app.st.editor.as_ref().unwrap();
    let r = (
        ed.draft.clone(),
        value_at(&ed.settings, "mesh.nx").cloned(),
        Some(ed.state()),
        shown,
        quick_run::blocked(&app.st),
        app.st.preview.shown.as_ref().filter(|b| b.from_editor).map(|b| b.cells),
    );
    eprintln!(
        "O9{}: {r:?} (the typed text came back {})",
        if drop_drafts { " control" } else { "" },
        match (&r.0, &r.1) {
            (Some(_), _) => "as the field being typed",
            (None, Some(v)) if *v == json!(20) => "committed (the click took the focus first)",
            _ => "not at all",
        }
    );
    r
}

/// O9: a field typed in the window and left by a click on another case is not lost: back on the case, either the field
/// being typed or the committed edit holds 20 (which one is reported), the field shows 20, Run refuses, and the mesh
/// shown is the edit's (20 x 16).
#[test]
fn o9_a_field_typed_and_left_comes_back() {
    let (draft, nx, state, shown, run, cells) = o9("o9", false);
    let typed = draft == Some(("mesh.nx".to_string(), "20".to_string()));
    let committed = draft.is_none() && nx == Some(json!(20)) && state == Some(EditorState::Dirty);
    assert!(typed || committed, "the text 20 was lost: {draft:?} {nx:?} {state:?}");
    assert_eq!(shown.as_deref(), Some("20"), "the field's text");
    assert!(run.is_some(), "Run enabled");
    assert_eq!(cells, Some(320), "the preview");
}

/// O9b (amendment 1, a guard without a control): b kept (`mesh.ny` 12 committed and left); in the cavity "20" typed
/// into `mesh.nx` with the field focused; a drawn click on b's label (b's editor comes back in the frame of the click);
/// settled. Limits: b's `mesh.nx` is 10 (its own) and b has no field being typed; the cavity is kept with its 20, typed
/// or committed (which one is reported).
#[test]
fn o9b_a_typed_text_stays_with_its_case() {
    let root = project("o9b");
    let (mut app, ctx) = app_on(&root, false, false);
    app.ui_state.filter = "mesh.nx".into();
    click_case(&mut app, &ctx, "cases/b.toml");
    settle_drawn(&mut app, &ctx);
    commit(&mut app.st, "mesh.ny", json!(12));
    click_case(&mut app, &ctx, "cases/cavity.toml");
    settle_drawn(&mut app, &ctx);
    assert_eq!(
        app.st.parked.keys().cloned().collect::<Vec<_>>(),
        vec!["cases/b.toml"],
        "set-up: b kept"
    );
    type_into_nx(&mut app, &ctx, &["BS", "BS", "2", "0"]);
    assert_eq!(
        app.st.editor.as_ref().unwrap().draft,
        Some(("mesh.nx".to_string(), "20".to_string())),
        "set-up: the keystrokes did not reach the field"
    );
    click_case(&mut app, &ctx, "cases/b.toml");
    settle_drawn(&mut app, &ctx);
    let b = app.st.editor.as_ref().unwrap();
    let cav = app.st.parked.get("cases/cavity.toml");
    let cav_state = cav.map(|c| (value_at(&c.settings, "mesh.nx").cloned(), c.draft.clone(), c.is_dirty()));
    eprintln!(
        "O9b: b ({}) mesh.nx {:?}, typed {:?}; the cavity kept {cav_state:?} (the 20 {})",
        b.rel,
        value_at(&b.settings, "mesh.nx"),
        b.draft,
        match &cav_state {
            Some((_, Some(_), _)) => "as the field being typed",
            Some((Some(v), None, true)) if *v == json!(20) => "committed",
            _ => "lost",
        }
    );
    assert_eq!(b.rel, "cases/b.toml");
    assert_eq!(
        value_at(&b.settings, "mesh.nx").cloned(),
        Some(json!(10)),
        "b's mesh.nx"
    );
    assert_eq!(b.draft, None, "b has a field being typed");
    let typed = cav_state
        .as_ref()
        .is_some_and(|(_, d, _)| *d == Some(("mesh.nx".to_string(), "20".to_string())));
    let committed = cav_state
        .as_ref()
        .is_some_and(|(v, d, dirty)| *v == Some(json!(20)) && d.is_none() && *dirty);
    assert!(typed || committed, "the cavity's 20: {cav_state:?}");
}

/// O9 control: ADR 0148's route: back on the cavity, 16 from its file, clean, nothing typed.
#[test]
fn o9_control_the_typed_text_was_lost() {
    let (draft, nx, state, shown, ..) = o9("o9ctl", true);
    assert_eq!(
        (draft, nx, state),
        (None, Some(json!(16)), Some(EditorState::Clean)),
        "control"
    );
    assert_eq!(shown.as_deref(), Some("16"), "control: the field's text");
}
