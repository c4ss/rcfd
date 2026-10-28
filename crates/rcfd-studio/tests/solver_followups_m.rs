//! ADR 0147 amendment 3 (job 48M; the maintainer's answers to ADR 0145 questions 3 and 5), process checks with
//! `rcfd-runner` and `rcfd` of this profile next to the test's `rcfd-studio` (`common::binaries`): M9 (the Studio's
//! step-count estimate equals the stored snapshots of a run whose end time lies a tiny remainder past a whole step) and
//! M10 (a long `short_last_step` warning of the solver shown in full in the event list).

mod common;
mod g4;
mod lj;
mod pv;

use rcfd_project::records::EventKind;
use rcfd_project::RunState;
use rcfd_runner::records::RunSettings;
use rcfd_studio::layout::Panel;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::store_plan::{self, CountKind, OLD_STEP_SLACK};
use rcfd_studio::ui::{event_text, event_text_with};
use serde_json::Value;
use std::time::Duration;

/// A 12 x 12 lid cavity with PISO and a fixed step of 0.01 s to `end`, its store every step (the case's own key).
fn piso(end: &str) -> String {
    format!(
        "title = \"w48m piso\"\n[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n\
         [boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\n\
         velocity = \"fixed\"\n[solver]\nalgorithm = \"piso\"\ndt = 0.01\nend_time = {end}\n[output]\nstore_interval = 1\n"
    )
}

/// Run `cases/<name>` (holding `text`) through the project's runner with `set` and wait for its end; the run id.
fn run(p: &common::RunnerProject, name: &str, set: Vec<String>) -> String {
    let id = p
        .client
        .start(
            &p.dir.join("cases").join(name),
            &RunSettings {
                set,
                ..Default::default()
            },
        )
        .unwrap();
    p.client.wait(&id, Duration::from_secs(120)).unwrap();
    id
}

/// The `short_last_step` warnings the run service recorded for run `id` (`solver` lines of `events.jsonl`).
fn short_last_step(p: &common::RunnerProject, id: &str) -> Vec<Value> {
    p.events(id)
        .into_iter()
        .filter(|e| {
            e["kind"] == "solver" && e["solver"]["event"] == "warning" && e["solver"]["kind"] == "short_last_step"
        })
        .collect()
}

/// M9: `cases/tiny.toml`, the PISO cavity with `dt = 0.01` and `end_time = 0.100000005` (10 steps and a remainder of
/// 5e-7 of a step, which the solver absorbs into its 10th step, ADR 0145 D1), its store every step. The Studio's plan
/// (`StartPlan::new`, as the Start dialog and Run make it) and the run started through the runner with the plan's
/// settings. Limits: the plan proposes no store (the case sets one); the count is exact and equals the run's stored
/// snapshots; the run finished at step 10 with 10 snapshots and no `short_last_step` warning. Negative control: the slack
/// before (`OLD_STEP_SLACK`, 1e-9) counts one more.
#[test]
fn m9_the_step_count_estimate_equals_the_stored_snapshots() {
    let mut p = common::RunnerProject::new("m9", 3);
    let text = piso("0.100000005");
    std::fs::write(p.dir.join("cases/tiny.toml"), &text).unwrap();
    p.serve();
    let plan = StartPlan::new("cases/tiny.toml", &text, &p.dir.join("cases")).unwrap();
    assert!(plan.store.is_none(), "set-up: a store proposal ({:?})", plan.store);
    let e = store_plan::effective(&plan);
    let now = store_plan::count(&e).expect("a count");
    let before = store_plan::count_with(&e, OLD_STEP_SLACK).expect("a count");
    let id = run(&p, "tiny.toml", plan.set());
    let s = p.status(&id).unwrap();
    let stored = rcfd_tsstore::Reader::open(&p.run_dir(&id).join("fields"))
        .unwrap()
        .snapshots()
        .len() as u64;
    let warned = short_last_step(&p, &id).len();
    eprintln!(
        "M9: plan set {:?}; estimate {} ({:?}); run {id} {:?} at step {}, {stored} stored snapshots, {warned} \
         short_last_step warning(s); control (slack 1e-9): {}",
        plan.set(),
        now.n,
        now.kind,
        s.state,
        s.step,
        before.n
    );
    assert_eq!((s.state, s.step), (RunState::Finished, 10), "the run");
    assert_eq!(stored, 10, "the stored snapshots");
    assert_eq!(warned, 0, "a warning for an absorbed remainder");
    assert_eq!(now.kind, CountKind::Exact, "the estimate's kind");
    assert_eq!(now.n, stored, "the estimate");
    assert_eq!(before.n, stored + 1, "control: the slack before");
}

/// Where the drawn text that starts with `prefix` lies (the first one), in points.
fn text_rect(out: &egui::FullOutput, prefix: &str) -> Option<egui::Rect> {
    fn walk(s: &egui::Shape, prefix: &str) -> Option<egui::Rect> {
        match s {
            egui::Shape::Text(t) if t.galley.text().starts_with(prefix) => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
            }
            egui::Shape::Vec(v) => v.iter().find_map(|x| walk(x, prefix)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|c| walk(&c.shape, prefix))
}

/// M10: `cases/short.toml`, the PISO cavity with `dt = 0.01` and `end_time = 0.0302`, so the 4th step is 2e-4 s (0.02 of a
/// step, a separate step that ADR 0145 D4 reports), run through the runner. Limits: the run finished; its `events.jsonl`
/// holds exactly one `short_last_step` warning, whose message is longer than 160 characters; the Studio's event list of
/// the selected run holds it and `ui::event_text` gives exactly `warning: <message>`; with the Log panel in front and its
/// event list opened, that text is drawn. Negative control: the route before (`event_text_with(.., false)`) gives the
/// event's JSON cut at 160 characters (`solver {...`, ending `...`), without the message.
#[test]
fn m10_a_long_solver_warning_is_shown_in_full() {
    let mut p = common::RunnerProject::new("m10", 3);
    std::fs::write(p.dir.join("cases/short.toml"), piso("0.0302")).unwrap();
    p.serve();
    let id = run(&p, "short.toml", Vec::new());
    let s = p.status(&id).unwrap();
    let w = short_last_step(&p, &id);
    assert_eq!(s.state, RunState::Finished, "the run: {:?}", s.reason);
    assert_eq!(w.len(), 1, "set-up: {w:?}");
    let message = w[0]["solver"]["message"].as_str().unwrap().to_string();
    assert!(message.len() > 160, "set-up: a short message ({})", message.len());
    // the Studio's event list
    let ctx = egui::Context::default();
    let mut app = g4::app_on(&p, "short.toml", &ctx);
    app.st.select_run(&id);
    let warning = |k: &EventKind| matches!(k, EventKind::Other { kind, body } if kind == "solver" && body["solver"]["kind"] == "short_last_step");
    assert!(
        common::frames_until(&mut app, &ctx, lj::WAIT, |a| a
            .st
            .runs
            .events
            .iter()
            .any(|e| warning(&e.kind))),
        "the warning did not reach the event list: {:?}",
        app.st.runs.events.len()
    );
    let ev = app.st.runs.events.iter().find(|e| warning(&e.kind)).cloned().unwrap();
    let text = event_text(&ev.kind);
    let before = event_text_with(&ev.kind, false);
    // drawn: the Log panel in front, its event list opened by a click on its header
    g4::put_in_front(&mut app, Panel::Log);
    let out = common::frame(&mut app, &ctx);
    let header = text_rect(&out, "events (").expect("set-up: the event list's header is not drawn");
    lj::click_at(&mut app, &ctx, header.center());
    let drawn: Vec<String> = (0..3).flat_map(|_| g4::texts(&common::frame(&mut app, &ctx))).collect();
    eprintln!(
        "M10: run {id} {:?}; the warning's message ({} characters): '{message}'; event_text: '{text}'; drawn {}; \
         control (before): '{before}'",
        s.state,
        message.len(),
        drawn.contains(&text)
    );
    assert_eq!(text, format!("warning: {message}"), "event_text");
    assert!(drawn.contains(&text), "not drawn in the event list");
    assert!(
        before.starts_with("solver {") && before.ends_with("...") && !before.contains(&message),
        "control: {before}"
    );
}
