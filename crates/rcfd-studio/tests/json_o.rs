//! ADR 0149 (job 48O) O10-O11: Run and the Start run dialog read a `.json` case as `rcfd run` reads it (the maintainer's
//! answer to ADR 0148 question 6): a JSON case written from a TOML case by `rcfd-api`'s writer runs from the Studio model
//! with its run snapshot equal to the file; every benchmark descriptor of the repository's `cases/` is refused with ADR
//! 0147's one line by the plan, by Run before and after its read (greyed out, with the line as its hover text) and by
//! the Start run dialog. Negative controls: the route before (`QuickRun::switches.cases_as_toml`,
//! `poll::Switches::cases_as_toml`). Release builds, `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd` of the
//! same build (`common::binaries`).

mod common;
mod g4;
mod lj;
mod pv;

use common::RunnerProject;
use rcfd_api::case::Case;
use rcfd_project::records::ResidualLine;
use rcfd_project::RunState;
use rcfd_studio::preview::{descriptor_message, parse_case};
use rcfd_studio::quick_run;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::{Studio, StudioApp};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

/// The Studio model on `p` (watcher and run worker on), the Run button searching `p`'s runner; the routes before when
/// `toml_only` (Run's and the Start dialog's).
fn studio(p: &RunnerProject, toml_only: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.quick.switches.cases_as_toml = toml_only;
    st.run_switches.cases_as_toml = toml_only;
    st.open_project(&p.dir);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()),
        "the project did not open"
    );
    st
}

fn settle(st: &mut Studio) {
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none() && lj::settled(s)),
        "did not settle: {:?}",
        st.preview.status
    );
}

/// Run as F5 asks for it; the id of the run it queued once the start ended, or the refusal (Run's, or the start's).
fn run_now(st: &mut Studio) -> Result<String, String> {
    let before = st.quick.run.as_ref().map(|r| r.0.clone());
    quick_run::run_selected_case(st)?;
    assert!(
        st.drain_until(WAIT, |s| s.quick.starting.is_none()),
        "the start did not end"
    );
    match st.quick.run.as_ref().map(|r| r.0.clone()) {
        Some(id) if Some(&id) != before.as_ref() => Ok(id),
        _ => Err(st.quick.error.clone().unwrap_or_default()),
    }
}

/// The Start run dialog's plan of `rel` (the run worker reads the file).
fn dialog_plan(st: &mut Studio, rel: &str) -> Result<String, String> {
    st.plan_start(rel).unwrap();
    assert!(st.drain_until(WAIT, |s| s.runs.plan.is_some()), "{rel}: no plan");
    match st.runs.plan.clone().unwrap() {
        Ok(p) => Ok(p.case),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------- O10: a JSON case runs

/// The residual lines of a run.
fn residuals(p: &RunnerProject, id: &str) -> Vec<ResidualLine> {
    std::fs::read_to_string(p.run_dir(id).join("residuals.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// A residual line as bits: step, time, residuals and monitors by name.
type Bits = (u64, u64, Vec<(String, u64)>, Vec<(String, u64)>);

/// Bits of the numbers of a residual line, so that two runs compare bit for bit.
fn line_bits(l: &ResidualLine) -> Bits {
    (
        l.step,
        l.time.to_bits(),
        l.residuals.iter().map(|(k, v)| (k.clone(), v.to_bits())).collect(),
        l.monitors.iter().map(|(k, v)| (k.clone(), v.to_bits())).collect(),
    )
}

/// What O10 saw.
#[derive(Debug, Default)]
struct O10 {
    same_settings: bool,
    plan: Option<Result<String, String>>,
    json_run: Option<Result<String, String>>,
    snapshot_equal: bool,
    finished: bool,
    toml_run: Option<String>,
    same_residuals: Option<bool>,
    lines: usize,
    dialog: Option<Result<String, String>>,
}

/// O10's procedure: `cases/t.toml` (the 12 x 12 cavity, 30 iterations) and `cases/j.json` written from it by
/// `Case::to_json_string`; (a) the settings of both; (b) the plan of the JSON text; (c) the JSON case selected, settled,
/// Run; (d) its run's snapshot and end; then the TOML case selected and Run, and the two runs' residuals; (e) the Start
/// run dialog's plan of the JSON case.
fn o10(tag: &str, toml_only: bool) -> O10 {
    let mut p = RunnerProject::new(tag, 3);
    let toml = g4::cavity(30);
    let json = Case::parse_toml(&toml).unwrap().to_json_string();
    std::fs::write(p.dir.join("cases/t.toml"), &toml).unwrap();
    std::fs::write(p.dir.join("cases/j.json"), &json).unwrap();
    p.serve();
    let dir = p.dir.join("cases");
    let mut r = O10::default();
    // (a)
    let from_toml = Case::load(dir.join("t.toml")).unwrap().to_json_string();
    let from_json = parse_case(&json, &dir).map(|c| c.to_json_string());
    r.same_settings = from_json.as_ref().is_ok_and(|j| *j == from_toml);
    // (b)
    r.plan = Some(StartPlan::new_with("cases/j.json", &json, &dir, toml_only).map(|pl| {
        format!(
            "{} {} iterations, {:?} cells; sets its store {}; --set {:?}",
            pl.facts.algorithm,
            pl.facts.max_iterations,
            pl.facts.cells,
            pl.facts.sets_store,
            pl.set()
        )
    }));
    // (c)
    let mut st = studio(&p, toml_only);
    st.open_case("cases/j.json");
    settle(&mut st);
    let ran = run_now(&mut st);
    r.json_run = Some(ran.clone());
    if let Ok(id) = &ran {
        // (d)
        let snap = std::fs::read(p.run_dir(id).join("case.snapshot")).unwrap_or_default();
        r.snapshot_equal = snap == json.as_bytes();
        let done = |id: &str| p.status(id).is_some_and(|s| s.state == RunState::Finished);
        r.finished = st.drain_until(Duration::from_secs(60), |_| done(id));
        st.open_case("cases/t.toml");
        settle(&mut st);
        if let Ok(t) = run_now(&mut st) {
            assert!(
                st.drain_until(Duration::from_secs(60), |_| done(&t)),
                "the TOML run did not finish"
            );
            let (a, b) = (residuals(&p, id), residuals(&p, &t));
            r.lines = a.len();
            r.same_residuals = Some(
                !a.is_empty()
                    && a.iter().map(line_bits).collect::<Vec<_>>() == b.iter().map(line_bits).collect::<Vec<_>>(),
            );
            r.toml_run = Some(t);
        }
    }
    // (e)
    r.dialog = Some(dialog_plan(&mut st, "cases/j.json"));
    eprintln!("O10{}: {r:#?}", if toml_only { " control" } else { "" });
    r
}

/// O10: a JSON case (written from the TOML case by `rcfd-api`'s writer) reads to the same settings, is planned, runs
/// from the Studio model to its end with its run snapshot equal to the file, solves as the TOML case does (residuals bit
/// for bit), and the Start run dialog plans it.
#[test]
fn o10_a_json_case_runs_from_the_studio() {
    let r = o10("o10", false);
    assert!(
        r.same_settings,
        "(a) the JSON case does not read to the TOML case's settings"
    );
    assert!(
        r.plan.as_ref().is_some_and(|p| p
            .as_ref()
            .is_ok_and(|t| t.starts_with("simplec 30 iterations, Some(144) cells"))),
        "(b) {:?}",
        r.plan
    );
    assert!(
        r.json_run.as_ref().is_some_and(|x| x.is_ok()),
        "(c) not started: {:?}",
        r.json_run
    );
    assert!(r.snapshot_equal, "(d) the run's snapshot is not the JSON file");
    assert!(r.finished, "(d) the run did not finish within 60 s");
    assert_eq!(
        r.same_residuals,
        Some(true),
        "(d) the residuals differ from the TOML run's"
    );
    assert_eq!(r.dialog, Some(Ok("cases/j.json".to_string())), "(e) the dialog's plan");
}

/// O10 control: the route before (`cases_as_toml`): Run refuses the JSON case as not TOML, and so does the dialog.
#[test]
fn o10_control_json_was_read_as_toml() {
    let r = o10("o10ctl", true);
    assert!(
        r.json_run.as_ref().is_some_and(|x| x
            .as_ref()
            .is_err_and(|e| e.starts_with("cases/j.json was not started: cases/j.json: not TOML: "))),
        "control (c): {:?}",
        r.json_run
    );
    assert!(
        r.dialog
            .as_ref()
            .is_some_and(|x| x.as_ref().is_err_and(|e| e.starts_with("cases/j.json: not TOML: "))),
        "control (e): {:?}",
        r.dialog
    );
}

// ---------------------------------------------------------------- O11: benchmark descriptors are not startable

/// The repository's benchmark descriptors: (`cases/<id>/case.json`, text), sorted.
fn descriptors() -> Vec<(String, String)> {
    let cases = pv::repo_root().join("cases");
    let mut v: Vec<(String, String)> = std::fs::read_dir(&cases)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("case.json"))
        .filter(|p| p.is_file())
        .map(|p| {
            let id = p.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            (format!("cases/{id}/case.json"), std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    v.sort();
    v
}

/// One descriptor's refusals: the plan's, Run's before the read, Run's greyed text after it, Run's refusal then (or the
/// start's error when it was not greyed), the Start dialog's plan.
#[derive(Debug, Clone)]
struct Refusals {
    rel: String,
    line: String,
    plan: Result<String, String>,
    before_read: Result<String, String>,
    blocked: Option<String>,
    after_read: Result<String, String>,
    dialog: Result<String, String>,
}

/// O11's procedure (a project with the descriptors copied to `cases/<id>/case.json`, not served; the run worker on, the
/// Run button searching this build's runner): for each, (a) `StartPlan::new`; (b) selected and Run at once (before the
/// read); (c) settled: Run's state and Run; (d) the Start dialog's plan. Then (e) the drawn hover over Run with the
/// cavity's descriptor selected. Returns the refusals, the runs queued, and whether the hover drew the line.
fn o11(tag: &str, toml_only: bool) -> (Vec<Refusals>, Vec<String>, bool) {
    let p = RunnerProject::new(tag, 3);
    let ds = descriptors();
    for (rel, text) in &ds {
        let f = p.dir.join(rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, text).unwrap();
    }
    let mut st = studio(&p, toml_only);
    let mut out = Vec::new();
    for (rel, text) in &ds {
        let dir = p.dir.join(rel).parent().unwrap().to_path_buf();
        let plan = StartPlan::new_with(rel, text, &dir, toml_only).map(|pl| pl.case);
        st.open_case(rel);
        let before_read = run_now(&mut st);
        settle(&mut st);
        let blocked = quick_run::blocked(&st);
        let after_read = run_now(&mut st);
        let dialog = dialog_plan(&mut st, rel);
        let r = Refusals {
            rel: rel.clone(),
            line: descriptor_message(rel, text).unwrap_or_default(),
            plan,
            before_read,
            blocked,
            after_read,
            dialog,
        };
        eprintln!("O11{} {r:#?}", if toml_only { " control" } else { "" });
        out.push(r);
    }
    let runs = p.ids();
    // (e) the drawn hover over the greyed Run, the cavity's descriptor selected
    st.open_case("cases/cavity/case.json");
    settle(&mut st);
    let want = quick_run::blocked(&st);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let _ = lj::frame_with(&mut app, &ctx, Vec::new());
    let pos = app.st.quick.rects["Run"].center();
    let mut hover = false;
    for _ in 0..30 {
        let t = lj::texts(&lj::frame_with(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]));
        if want.as_ref().is_some_and(|w| t.iter().any(|x| x == w)) {
            hover = true;
            break;
        }
    }
    eprintln!("O11 (e): the hover drew {want:?}: {hover}; runs queued {runs:?}");
    (out, runs, hover)
}

/// O11: every benchmark descriptor of the repository's `cases/` is refused with ADR 0147's one line: by the plan, by Run
/// before its read (the start's refusal), by Run after its read (greyed out with the line, `Run: ` and the line on F5 or
/// a click), and by the Start run dialog; no run is queued.
#[test]
fn o11_benchmark_descriptors_are_not_startable() {
    let (out, runs, hover) = o11("o11", false);
    assert_eq!(out.len(), 6, "the repository's descriptors");
    for r in &out {
        assert!(
            r.line.contains(": not a case file: ") && !r.line.contains('\n'),
            "{}: {}",
            r.rel,
            r.line
        );
        assert_eq!(r.plan, Err(r.line.clone()), "{} (a)", r.rel);
        assert_eq!(
            r.before_read,
            Err(format!("{} was not started: {}", r.rel, r.line)),
            "{} (b)",
            r.rel
        );
        assert_eq!(r.blocked.as_deref(), Some(r.line.as_str()), "{} (c) Run's state", r.rel);
        assert_eq!(r.after_read, Err(format!("Run: {}", r.line)), "{} (c) Run", r.rel);
        assert_eq!(r.dialog, Err(r.line.clone()), "{} (d)", r.rel);
    }
    assert!(runs.is_empty(), "runs queued: {runs:?}");
    assert!(hover, "(e) the line not drawn on hover");
}

/// O11 control: the route before (`cases_as_toml`): Run is not greyed out after the read, and the refusals are the TOML
/// parser's (`not TOML: ...`), not the one line.
#[test]
fn o11_control_descriptors_were_refused_as_not_toml() {
    let (out, ..) = o11("o11ctl", true);
    for r in &out {
        assert_eq!(r.blocked, None, "{} control (c)", r.rel);
        let not_toml = format!("{} was not started: {}: not TOML: ", r.rel, r.rel);
        assert!(
            r.after_read.as_ref().is_err_and(|e| e.starts_with(&not_toml)),
            "{} control (c): {:?}",
            r.rel,
            r.after_read
        );
        assert!(
            r.dialog
                .as_ref()
                .is_err_and(|e| e.starts_with(&format!("{}: not TOML: ", r.rel))),
            "{} control (d): {:?}",
            r.rel,
            r.dialog
        );
    }
}
