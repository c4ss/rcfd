//! ADR 0158 (job 48X): Run's plan makes the pressure correctors adaptive (`solver.corrector_control=adaptive`, ADR 0156 D8)
//! for transient iterative piso/pimple cases that do not choose the key themselves. X1 the scope against the solver's own
//! rule (ADR 0156 D5) on the examples and registered variants; X2 the order of the `--set` entries and the plan's `Debug`
//! text; X3 the toolbar's corrector line, Run's hover and the Advanced switch, drawn headless; X4 the shipped cylinder
//! through Run and the real runner, on and off; X5 the maintainer's 256 x 256 cylinder through Run (heavy, ignored, run on
//! purpose): (a) the automatic time step, (b) the case's own dt 1/64, each with the switch off as its control. Release
//! builds, `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd` of the same build (`common::binaries`).

mod common;
mod lj;
mod pv;
mod ts;

use common::RunnerProject;
use rcfd_project::RunState;
use rcfd_studio::quick_run;
use rcfd_studio::quick_run_ui::{ADVANCED, CORRECTORS_BOX, CORRECTORS_LINE, RUN_HOVER};
use rcfd_studio::runs::StartPlan;
use rcfd_studio::time_step::{self, CorrectorPlan, FixedWhy, Kind, RunTime, StepMode, ADAPTIVE_ENTRY};
use rcfd_studio::{Studio, StudioApp};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------- set-ups

/// `[solver]` of `text` with `line` added as its first key.
fn with_solver(text: &str, line: &str) -> String {
    assert!(text.contains("\n[solver]\n"), "no [solver] table");
    text.replacen("\n[solver]\n", &format!("\n[solver]\n{line}\n"), 1)
}

/// The cylinder at `nx` x `ny` with its own `solver.dt` replaced by `dt`.
fn cylinder_dt(nx: usize, ny: usize, dt: f64) -> String {
    let t = ts::cylinder(nx, ny);
    let line = t
        .lines()
        .find(|l| l.starts_with("dt = 0.0625"))
        .expect("the example's dt line moved")
        .to_string();
    t.replacen(&line, &format!("dt = {dt:?}"), 1)
}

/// ADR 0156 A9's moving-piston case (`box_2d`, piso, `dynamic_mesh`), without the key.
const PISTON: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// ADR 0156 A9's closed-box heating case (variable density, `low_mach`, pimple), without the key.
const LOW_MACH: &str = r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
gamma = 1.4
mode = "low_mach"
reference_pressure = 101325.0
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = 84437.5
[initial]
temperature = 300.0
pressure = 101325.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = 0.01
end_time = 0.1
"#;

/// What X1 expects of a case.
#[derive(Debug, Clone, PartialEq)]
enum Want {
    /// Run adds the entry; the least count.
    Adaptive(u64),
    /// The case's own `adaptive`.
    CaseAdaptive,
    /// Out of scope with a reason that starts with this text; `true`: the solver refuses the key there (ADR 0156 D5).
    Scope(&'static str, bool),
}

/// The solver's answer to `text` with `solver.corrector_control = "adaptive"`: `Ok` when `Simulation::new` builds it, else
/// the paths of its diagnostics.
fn solver_with_adaptive(text: &str, dir: &Path) -> Result<(), Vec<String>> {
    let mut case = rcfd_studio::preview::parse_case(text, dir)
        .map_err(|d| d.iter().map(|x| x.path.clone()).collect::<Vec<String>>())?;
    case.set("solver.corrector_control", json!("adaptive"))
        .map_err(|e| vec![format!("set: {e}")])?;
    rcfd_api::sim::Simulation::new(&case)
        .map(|_| ())
        .map_err(|d| d.iter().map(|x| format!("{}: {}", x.path, x.message)).collect())
}

// ---------------------------------------------------------------- X1: the scope

/// X1 (model): the corrector plan of the registered set-ups with the default run-time settings, and the solver's own
/// answer to the key on each: where Run adds the entry the solver builds the case with it; where the reason is one of ADR
/// 0156 D5's the solver refuses it at `solver.corrector_control`. Every example case of both example trees and of
/// `cases/` gets a plan; the transient ones the entry.
#[test]
fn x1_scope_of_the_examples_and_variants() {
    let wt = ts::wind_tunnel_cases();
    let cyl = ts::cylinder(64, 48);
    let cyl_json = rcfd_api::case::Case::parse_toml(&cyl).unwrap().to_json_string();
    let sod = std::fs::read_to_string(ts::repo_root().join("cases/sod/sod.toml")).unwrap();
    let cav = std::fs::read_to_string(ts::quick_start_cases().join("cavity-re100.toml")).unwrap();
    let set_ups: Vec<(&str, String, PathBuf, Want)> = vec![
        ("cylinder 64 x 48 (shipped)", cyl.clone(), wt.clone(), Want::Adaptive(2)),
        (
            "cylinder, corrector_control = \"fixed\" spelled out",
            with_solver(&cyl, "corrector_control = \"fixed\""),
            wt.clone(),
            Want::Adaptive(2),
        ),
        (
            "cylinder as JSON (every key spelled out)",
            cyl_json,
            wt.clone(),
            Want::Adaptive(2),
        ),
        (
            "cylinder, corrector_control = \"adaptive\"",
            with_solver(&cyl, "corrector_control = \"adaptive\""),
            wt.clone(),
            Want::CaseAdaptive,
        ),
        (
            "cylinder, n_correctors = 3",
            with_solver(&cyl, "n_correctors = 3"),
            wt.clone(),
            Want::Adaptive(3),
        ),
        (
            "cylinder 256 x 256",
            ts::cylinder(256, 256),
            wt.clone(),
            Want::Adaptive(2),
        ),
        (
            "cylinder, fractional_step",
            with_solver(&cyl, "time_advance = \"fractional_step\""),
            wt.clone(),
            Want::Scope("solver.time_advance = \"fractional_step\"", true),
        ),
        (
            "cylinder, error_tolerance 1e-3",
            with_solver(&cyl, "error_tolerance = 1e-3"),
            wt.clone(),
            Want::Scope("solver.error_tolerance", true),
        ),
        (
            "piston (dynamic_mesh)",
            PISTON.to_string(),
            wt.clone(),
            Want::Scope("dynamic_mesh", true),
        ),
        (
            "closed box, low_mach (variable density)",
            LOW_MACH.to_string(),
            wt.clone(),
            Want::Scope("a variable-density run", true),
        ),
        (
            "cavity as PISO",
            ts::cavity_piso(32, 10.0),
            ts::quick_start_cases(),
            Want::Adaptive(2),
        ),
        (
            "step Re 400 as PISO",
            ts::step_piso(30.0),
            wt.clone(),
            Want::Adaptive(2),
        ),
        (
            "heated cavity as PISO",
            ts::heated_piso(2.0),
            ts::quick_start_cases(),
            Want::Adaptive(2),
        ),
        (
            "ramped-lid cavity as PISO",
            ts::cavity_ramp_piso(2.0),
            ts::quick_start_cases(),
            Want::Adaptive(2),
        ),
        (
            "quick start cavity (steady)",
            cav,
            ts::quick_start_cases(),
            Want::Scope("a steady run has no time steps", true),
        ),
        (
            "cases/sod (compressible)",
            sod,
            ts::repo_root().join("cases/sod"),
            Want::Scope("the compressible solver is explicit", true),
        ),
    ];
    let rt = RunTime::default();
    for (label, text, dir, want) in &set_ups {
        let f = time_step::facts(text, dir).unwrap_or_else(|e| panic!("{label}: {e}"));
        let cp = time_step::corrector_plan(&f, &rt);
        eprintln!("X1 {label}: {:?} -> {cp:?}", f.correctors);
        match want {
            Want::Adaptive(n) => {
                assert_eq!(cp, CorrectorPlan::Adaptive { n: *n }, "{label}");
                assert_eq!(cp.entry().as_deref(), Some(ADAPTIVE_ENTRY), "{label}");
                let s = solver_with_adaptive(text, dir);
                assert!(s.is_ok(), "{label}: the solver refuses the entry: {s:?}");
            }
            Want::CaseAdaptive => {
                assert_eq!(cp, CorrectorPlan::CaseAdaptive { n: 2 }, "{label}");
                assert_eq!(cp.entry(), None, "{label}");
            }
            Want::Scope(why, refused) => {
                match &cp {
                    CorrectorPlan::Fixed {
                        why: FixedWhy::Scope(w),
                        ..
                    } => assert!(w.starts_with(why), "{label}: {w}"),
                    other => panic!("{label}: {other:?}"),
                }
                assert_eq!(cp.entry(), None, "{label}");
                if *refused {
                    let s = solver_with_adaptive(text, dir);
                    assert!(
                        s.as_ref()
                            .err()
                            .is_some_and(|d| d.iter().any(|x| x.starts_with("solver.corrector_control"))),
                        "{label}: not refused at solver.corrector_control: {s:?}"
                    );
                }
            }
        }
    }
    // every example case file of both trees and every case of cases/: transient ones get the entry, the others not
    let mut files = Vec::new();
    for tree in ["example-projects", "example-projects-linux"] {
        for project in ["quick-start", "wind-tunnel"] {
            let d = ts::repo_root().join(tree).join(project).join("cases");
            for e in std::fs::read_dir(&d).unwrap() {
                files.push(e.unwrap().path());
            }
        }
    }
    for e in std::fs::read_dir(ts::repo_root().join("cases")).unwrap() {
        let d = e.unwrap().path();
        if d.is_dir() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.extension().is_some_and(|x| x == "toml") {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    let (mut adaptive, mut other) = (0, 0);
    for p in &files {
        let text = std::fs::read_to_string(p).unwrap();
        let f = time_step::facts(&text, p.parent().unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let cp = time_step::corrector_plan(&f, &rt);
        eprintln!("X1 file {}: {:?} -> {cp:?}", p.display(), f.kind);
        if f.kind == Kind::Transient {
            assert_eq!(cp, CorrectorPlan::Adaptive { n: f.correctors.n }, "{}", p.display());
            let s = solver_with_adaptive(&text, p.parent().unwrap());
            assert!(s.is_ok(), "{}: {s:?}", p.display());
            adaptive += 1;
        } else {
            assert_eq!(cp.entry(), None, "{}", p.display());
            other += 1;
        }
    }
    eprintln!(
        "X1 files: {adaptive} with the entry, {other} without, of {}",
        files.len()
    );
    assert_eq!(adaptive, 4, "the four example cylinders");
}

// ---------------------------------------------------------------- X2: the order of the entries

/// X2 (model): Run's plan of the shipped cylinder: the time entries, then the corrector entry, then the user's overrides
/// (the last entry of a key applies); with the switch off no corrector entry; a plan without Run's settings prints as before
/// (no `time`, no `correctors` in its `Debug` text: ADR 0152 DV6).
#[test]
fn x2_entries_in_order_and_the_plan_text() {
    let (dir, text) = (ts::wind_tunnel_cases(), ts::cylinder(64, 48));
    let f = time_step::facts(&text, &dir).unwrap();
    let rt = RunTime::default();
    let base = StartPlan::new("cases/cyl.toml", &text, &dir).unwrap();
    let dbg = format!("{base:?}");
    assert!(!dbg.contains("correctors") && !dbg.contains("time:"), "{dbg}");
    let mut run = base
        .clone()
        .with_time(time_step::plan(&f, &rt).unwrap())
        .with_correctors(time_step::corrector_plan(&f, &rt));
    let want = [
        "solver.end_time=80",
        "solver.dt=0.00602",
        "solver.max_courant=0.5",
        "solver.max_dt=0.8",
        ADAPTIVE_ENTRY,
    ];
    assert_eq!(run.set(), want, "Run's set");
    run.extra_set = vec!["solver.corrector_control=fixed".into()];
    let mut with_user = want.to_vec();
    with_user.push("solver.corrector_control=fixed");
    assert_eq!(run.set(), with_user, "the user's override after it");
    assert!(format!("{run:?}").contains("correctors: Adaptive { n: 2 }"));
    let off = RunTime {
        adaptive_correctors: false,
        ..RunTime::default()
    };
    let run_off = base
        .with_time(time_step::plan(&f, &off).unwrap())
        .with_correctors(time_step::corrector_plan(&f, &off));
    assert_eq!(run_off.set(), want[..4], "the switch off");
}

// ---------------------------------------------------------------- the served project (as run_time_s.rs)

fn project(tag: &str, cases: &[(&str, String)]) -> RunnerProject {
    let mut p = RunnerProject::new(tag, 3);
    for (rel, text) in cases {
        std::fs::write(p.dir.join(rel), text).unwrap();
    }
    p.serve();
    p
}

fn studio(p: &RunnerProject, before: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.quick.switches.case_time = before;
    st.open_project(&p.dir);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()),
        "the project did not open"
    );
    st
}

fn select(st: &mut Studio, rel: &str) {
    st.open_case(rel);
    let before = st.quick.switches.case_time;
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none()
            && lj::settled(s)
            && (before || quick_run::facts_of(s, rel).is_some())),
        "{rel}: not settled: {:?}",
        st.preview.status
    );
}

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

fn wait_end(p: &RunnerProject, st: &mut Studio, id: &str, limit: Duration) -> rcfd_project::records::RunStatus {
    let ended = |s: &Option<rcfd_project::records::RunStatus>| s.as_ref().is_some_and(|x| x.state.is_terminal());
    assert!(
        st.drain_until(limit, |_| ended(&p.status(id))),
        "run {id} did not end within {limit:?}: {:?}",
        p.status(id).map(|s| (s.state, s.step, s.time))
    );
    p.status(id).unwrap()
}

fn run_json(p: &RunnerProject, id: &str) -> Value {
    serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap()
}

fn run_set(p: &RunnerProject, id: &str) -> Vec<String> {
    run_json(p, id)["settings"]["set"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

/// The lines of a JSON-lines file (unparsable lines left out; a missing file gives none).
fn json_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// ---------------------------------------------------------------- X3: drawn

/// What X3 drew.
#[derive(Debug, Default)]
struct X3 {
    row: Vec<String>,
    hover: Vec<String>,
    popover: Vec<String>,
    off: Vec<String>,
    on_again: Vec<String>,
    rt_off: Option<bool>,
    line_rect: Option<egui::Rect>,
    file_same: bool,
    dirty: bool,
}

fn frame(app: &mut StudioApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<String> {
    lj::texts(&lj::frame_with(app, ctx, events))
}

/// X3's procedure (f3's of ADR 0153 on the corrector line): the shipped cylinder selected, frames of 1280 x 800, tooltips at
/// once; (i) the second frame read; (ii) the pointer on Run; (iii) a drawn click on Advanced, a frame; (iv) a drawn click on
/// the corrector switch, Escape, frames; (v) Advanced and the switch again, Escape, frames.
fn x3_run(tag: &str, before: bool) -> X3 {
    let p = project(tag, &[("cases/cyl.toml", ts::cylinder(64, 48))]);
    let file = p.dir.join("cases/cyl.toml");
    let bytes = std::fs::read(&file).unwrap();
    let mut st = studio(&p, before);
    select(&mut st, "cases/cyl.toml");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let _ = frame(&mut app, &ctx, Vec::new());
    let mut r = X3 {
        row: frame(&mut app, &ctx, Vec::new()),
        ..X3::default()
    };
    let pos = app.st.quick.rects["Run"].center();
    for _ in 0..30 {
        r.hover = frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
        if r.hover.iter().any(|t| t == RUN_HOVER) && r.hover.len() > r.row.len() {
            break;
        }
    }
    let away = vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))];
    let _ = frame(&mut app, &ctx, away.clone());
    let toggle = |app: &mut StudioApp, read: bool| -> Vec<String> {
        let mut seen = Vec::new();
        if let Some(a) = app.st.quick.rects.get(ADVANCED).copied() {
            lj::click_at(app, &ctx, a.center());
            let t = frame(app, &ctx, Vec::new());
            if read {
                seen = t;
            }
            if let Some(b) = app.st.quick.rects.get(CORRECTORS_BOX).copied() {
                lj::click_at(app, &ctx, b.center());
            }
            let _ = frame(app, &ctx, lj::key(egui::Key::Escape));
            let _ = frame(app, &ctx, vec![egui::Event::PointerMoved(egui::pos2(5.0, 795.0))]);
        }
        seen
    };
    r.popover = toggle(&mut app, true);
    r.off = frame(&mut app, &ctx, Vec::new());
    r.rt_off = app
        .st
        .quick
        .run_time
        .get("cases/cyl.toml")
        .map(|rt| rt.adaptive_correctors);
    let _ = toggle(&mut app, false);
    r.on_again = frame(&mut app, &ctx, Vec::new());
    r.file_same = std::fs::read(&file).unwrap() == bytes;
    r.dirty = app.st.editor.as_ref().is_some_and(|e| e.is_dirty());
    r.line_rect = app.st.quick.rects.get(CORRECTORS_LINE).copied();
    eprintln!("X3 {tag}: {r:#?}");
    r
}

fn drawn(texts: &[String], t: &str) -> bool {
    texts.iter().any(|x| x == t)
}

/// X3: (i) the row draws ADR 0153's step label and indicator (unchanged) and the corrector line of an adaptive plan; (ii)
/// Run's hover draws Run's text, the time plan's hover and the corrector hover; (iii) the popover draws the switch; (iv)
/// after a drawn click on it the run-time settings hold the switch off and the line of a fixed plan is drawn (not the
/// adaptive one); (v) a second click draws the adaptive line again; the case file is unchanged and the editor clean.
#[test]
fn x3_toolbar_corrector_line_drawn() {
    let r = x3_run("x3", false);
    let (dir, text) = (ts::wind_tunnel_cases(), ts::cylinder(64, 48));
    let f = time_step::facts(&text, &dir).unwrap();
    let rt = RunTime::default();
    let on = time_step::corrector_plan(&f, &rt);
    let off = time_step::corrector_plan(
        &f,
        &RunTime {
            adaptive_correctors: false,
            ..rt.clone()
        },
    );
    let (on_line, off_line) = (time_step::corrector_label(&on), time_step::corrector_label(&off));
    assert_eq!(
        on_line,
        "pressure correctors: adaptive for this run (at least 2, at most 10; the solver adds them where the mesh needs them)"
    );
    assert_eq!(
        off_line,
        "pressure correctors: fixed, 2 per step (adaptive switched off in Advanced)"
    );
    for t in [
        "dt auto: 0.00602 s at the start, then adaptive (Courant 0.5)",
        "overrides the case's solver.dt 0.0625 s for this run",
        on_line.as_str(),
    ] {
        assert!(drawn(&r.row, t), "(i) not drawn: {t}");
    }
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
    assert!(
        r.line_rect.is_some_and(|x| window.contains_rect(x)),
        "(i) the line outside the window: {:?}",
        r.line_rect
    );
    assert!(drawn(&r.hover, RUN_HOVER), "(ii) Run's text");
    let tp = time_step::plan(&f, &rt).unwrap();
    assert!(
        drawn(&r.hover, &time_step::hover(&f, &tp)),
        "(ii) the time plan's hover"
    );
    assert!(
        drawn(&r.hover, &time_step::corrector_hover(&f, &on)),
        "(ii) the corrector hover"
    );
    assert!(drawn(&r.popover, CORRECTORS_BOX), "(iii) the switch");
    assert_eq!(r.rt_off, Some(false), "(iv) the run-time settings");
    assert!(
        drawn(&r.off, &off_line) && !drawn(&r.off, &on_line),
        "(iv) the fixed line"
    );
    assert!(drawn(&r.on_again, &on_line), "(v) adaptive again");
    assert!(r.file_same && !r.dirty, "the case file or the editor changed");
}

/// X3 control: the route before ADR 0153 (no time row) draws no corrector line and no corrector hover.
#[test]
fn x3_control_route_before_draws_no_corrector_line() {
    let r = x3_run("x3ctl", true);
    let all: Vec<&String> = r.row.iter().chain(&r.hover).collect();
    assert!(
        !all.iter()
            .any(|t| t.starts_with("pressure correctors") || t.starts_with("Pressure correctors")),
        "control: a corrector text drawn"
    );
}

// ---------------------------------------------------------------- runs through Run and the real runner

/// What a run through Run gave.
#[derive(Debug, Default)]
#[allow(dead_code)]
struct R {
    id: String,
    set: Vec<String>,
    state: Option<RunState>,
    end_bits: u64,
    end: f64,
    steps: usize,
    /// `correctors` of every `iteration` event of the solver's stream (none for a fixed run).
    counts: Vec<u64>,
    /// `iteration` events without `correctors`.
    without: usize,
    /// The solver's `finish` event's `correctors` summary.
    summary: Value,
    /// `rule` SV301 in `check.json`'s `rules` (its estimate in the message).
    check_sv301: Option<String>,
    /// SV301 in a `warning` event of `events.jsonl`.
    event_sv301: Option<String>,
    /// (time, M_hf) of the snapshot at the first stored time from 1 s on and of the last one.
    m_hf: Vec<(f64, f64)>,
    file_same: bool,
    snapshot_same: bool,
    monitors: Vec<u8>,
    wall_s: f64,
}

/// A run of `text` as `cases/c.toml` through Run: "Simulate for" `n` s, the step `mode`, adaptive correctors `on`.
fn run_case(tag: &str, text: &str, n: f64, mode: StepMode, on: bool, m_hf: bool, limit: Duration) -> R {
    let rel = "cases/c.toml";
    let p = project(tag, &[(rel, text.to_string())]);
    let file = p.dir.join(rel);
    let bytes = std::fs::read(&file).unwrap();
    let mut st = studio(&p, false);
    select(&mut st, rel);
    {
        let rt = quick_run::run_time_mut(&mut st, rel);
        rt.simulate = Some(n);
        rt.mode = mode;
        rt.adaptive_correctors = on;
    }
    let t0 = Instant::now();
    let id = run_now(&mut st).unwrap();
    let s = wait_end(&p, &mut st, &id, limit);
    let dir = p.run_dir(&id);
    let mut r = R {
        id: id.clone(),
        set: run_set(&p, &id),
        state: Some(s.state),
        end_bits: s.time.to_bits(),
        end: s.time,
        wall_s: t0.elapsed().as_secs_f64(),
        ..R::default()
    };
    for v in json_lines(&dir.join("solver-status.jsonl")) {
        match v["event"].as_str() {
            Some("iteration") => {
                r.steps += 1;
                match v["correctors"].as_u64() {
                    Some(c) => r.counts.push(c),
                    None => r.without += 1,
                }
            }
            Some("finish") => r.summary = v["correctors"].clone(),
            _ => {}
        }
    }
    let check = std::fs::read_to_string(dir.join("check.json")).unwrap_or_default();
    r.check_sv301 = check
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .flat_map(|v| v["rules"].as_array().cloned().unwrap_or_default())
        .find(|x| x["id"] == "SV301" || x["rule"] == "SV301")
        .map(|x| x.to_string());
    r.event_sv301 = std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .find(|l| l.contains("SV301"))
        .map(str::to_string);
    if m_hf {
        r.m_hf = m_hf_of(text, &dir.join("fields"));
    }
    r.file_same = std::fs::read(&file).unwrap() == bytes;
    r.snapshot_same = std::fs::read(dir.join("case.snapshot")).unwrap() == bytes;
    r.monitors = std::fs::read(dir.join("output/monitors.csv")).unwrap_or_default();
    eprintln!("X {tag}: {:#?}", r.clone_meta());
    let (lo, hi) = (r.counts.iter().min(), r.counts.iter().max());
    eprintln!(
        "X {tag}: correctors per step min {lo:?} max {hi:?} over {} events",
        r.counts.len()
    );
    r
}

impl R {
    fn clone_meta(&self) -> R {
        R {
            id: self.id.clone(),
            set: self.set.clone(),
            state: self.state,
            end_bits: self.end_bits,
            end: self.end,
            steps: self.steps,
            counts: Vec::new(),
            without: self.without,
            summary: self.summary.clone(),
            check_sv301: self.check_sv301.clone(),
            event_sv301: self.event_sv301.clone(),
            m_hf: self.m_hf.clone(),
            file_same: self.file_same,
            snapshot_same: self.snapshot_same,
            monitors: Vec::new(),
            wall_s: self.wall_s,
        }
    }
}

// ---------------------------------------------------------------- M_hf (ADR 0154 "Metrics", as w48t_cylinder_spokes.rs)

/// Kinematic dynamic pressure of the free stream, 0.5 rho U^2 with rho = 1, U = 1.
const Q: f64 = 0.5;

/// Cells grouped into rings by centre radius (equal to 1e-9 relative) and ordered by angle in [0, 2 pi); `(radius, cells)`.
fn rings(centres: &[[f64; 3]]) -> Vec<(f64, Vec<usize>)> {
    let r: Vec<f64> = centres.iter().map(|c| c[0].hypot(c[1])).collect();
    let mut order: Vec<usize> = (0..r.len()).collect();
    order.sort_by(|&a, &b| r[a].total_cmp(&r[b]));
    let mut groups: Vec<Vec<usize>> = vec![vec![order[0]]];
    for w in order.windows(2) {
        if r[w[1]] - r[w[0]] > 1e-9 * r[w[1]] {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().push(w[1]);
    }
    let theta = |c: usize| {
        centres[c][1]
            .atan2(centres[c][0])
            .rem_euclid(2.0 * std::f64::consts::PI)
    };
    groups
        .into_iter()
        .map(|mut g| {
            g.sort_by(|&a, &b| theta(a).total_cmp(&theta(b)));
            (g.iter().map(|&c| r[c]).sum::<f64>() / g.len() as f64, g)
        })
        .collect()
}

/// M_hf: RMS over the rings with centre radius in [0.55, 0.70] of the part of p in the circumferential modes n/8 <= k <= n/2,
/// divided by q (by Parseval from the DFT of each ring: (2 sum_{n/8 <= k < n/2} |P_k|^2 + |P_{n/2}|^2) / n per ring).
fn m_hf(p: &[f64], centres: &[[f64; 3]]) -> f64 {
    let (mut sum, mut cnt) = (0.0, 0usize);
    for (rj, g) in rings(centres) {
        if !(0.55..=0.70).contains(&rj) {
            continue;
        }
        let n = g.len();
        assert!(n % 8 == 0, "ring of {n} cells");
        let mut hp = 0.0;
        for k in n / 8..=n / 2 {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &c) in g.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * i % n) as f64 / n as f64;
                re += p[c] * a.cos();
                im += p[c] * a.sin();
            }
            hp += if k == n / 2 { 1.0 } else { 2.0 } * (re * re + im * im);
        }
        sum += hp / n as f64;
        cnt += n;
    }
    assert!(cnt > 0, "no ring in [0.55, 0.70]");
    (sum / cnt as f64).sqrt() / Q
}

/// (time, M_hf) of the store's snapshot at the first stored time from 1 s on and of its last snapshot (the reading rule
/// of ADR 0158); the cell centres from the case's own mesh (`Simulation::new` of the text, as the run builds it).
fn m_hf_of(text: &str, store: &Path) -> Vec<(f64, f64)> {
    let case = rcfd_studio::preview::parse_case(text, &ts::wind_tunnel_cases()).unwrap();
    let sim = rcfd_api::sim::Simulation::new(&case).unwrap();
    let centres = sim.geom.mesh.cell_centres().to_vec();
    let Ok(mut rd) = rcfd_tsstore::Reader::open(store) else {
        return Vec::new();
    };
    let snaps: Vec<(usize, f64)> = rd.snapshots().iter().enumerate().map(|(i, s)| (i, s.time)).collect();
    let mut picks = Vec::new();
    if let Some(&(i, t)) = snaps.iter().find(|(_, t)| *t >= 1.0 - 1e-9) {
        picks.push((i, t));
    }
    if let Some(&(i, t)) = snaps.last() {
        if picks.last().map(|x| x.0) != Some(i) {
            picks.push((i, t));
        }
    }
    picks
        .into_iter()
        .map(|(i, t)| {
            let p = rd.read_field(i, "p").unwrap();
            let m = if p.iter().all(|x| x.is_finite()) {
                m_hf(&p, &centres)
            } else {
                f64::INFINITY
            };
            (t, m)
        })
        .collect()
}

/// L3 of ADR 0154 at the checked times of a run to 3 s: 0.01 at t = 1, 0.0114 at t = 3.
fn clean(m: &[(f64, f64)]) -> bool {
    m.len() == 2 && m[0].1 <= 0.01 && m[1].1 <= 0.0114
}

// ---------------------------------------------------------------- X4: the shipped cylinder, on and off

/// X4: the shipped cylinder (64 x 48) through Run, "Simulate for" 2 s, the automatic step: with adaptive correctors (on)
/// the run's `set` is exactly the time entries and the corrector entry, it finishes at 2 s bit for bit, every `iteration`
/// event has `correctors` 2 and the `finish` summary is adaptive with `max` 2 and `cap_bound` 0, the case file and
/// `case.snapshot` are the file's bytes; with the switch off (the control) the `set` has no corrector entry, no event has
/// `correctors`, the summary is absent; the two runs' `monitors.csv` are byte-identical (the count stays 2).
#[test]
fn x4_shipped_cylinder_adaptive_on_and_off() {
    let text = ts::cylinder(64, 48);
    let on = run_case(
        "x4on",
        &text,
        2.0,
        StepMode::Adaptive,
        true,
        false,
        Duration::from_secs(600),
    );
    let off = run_case(
        "x4off",
        &text,
        2.0,
        StepMode::Adaptive,
        false,
        false,
        Duration::from_secs(600),
    );
    let time = [
        "solver.end_time=2",
        "solver.dt=0.00602",
        "solver.max_courant=0.5",
        "solver.max_dt=0.02",
    ];
    let mut want = time.to_vec();
    want.push(ADAPTIVE_ENTRY);
    assert_eq!(on.set, want, "on: set");
    assert_eq!(
        (on.state, on.end_bits),
        (Some(RunState::Finished), 2f64.to_bits()),
        "on: end {}",
        on.end
    );
    assert!(
        on.steps > 0 && on.without == 0 && on.counts.iter().all(|&c| c == 2),
        "on: correctors per step"
    );
    assert_eq!(on.summary["mode"], "adaptive", "on: {}", on.summary);
    assert_eq!(
        (on.summary["max"].as_u64(), on.summary["cap_bound"].as_u64()),
        (Some(2), Some(0)),
        "on: {}",
        on.summary
    );
    assert!(on.file_same && on.snapshot_same, "on: the case file or its snapshot");
    // the control
    assert_eq!(off.set, time, "off: set");
    assert_eq!(off.state, Some(RunState::Finished), "off: state");
    assert!(
        off.counts.is_empty() && off.summary.is_null(),
        "off: corrector keys in the events"
    );
    assert!(off.file_same && off.snapshot_same, "off: the case file or its snapshot");
    assert!(!on.monitors.is_empty(), "no monitors.csv");
    assert!(on.monitors == off.monitors, "monitors.csv differ between on and off");
}

// ---------------------------------------------------------------- X5: the maintainer's 256 x 256 cylinder (heavy)

/// X5 (a): the 256 x 256 cylinder through Run with the automatic step, "Simulate for" 3 s, adaptive correctors on: the
/// `set` exactly the time entries and the corrector entry, finished at 3 s bit for bit, the `finish` summary adaptive, clean
/// at t = 1 and 3 (L3), no SV301 in `check.json` or the events, the case file and `case.snapshot` its bytes.
#[test]
#[ignore = "heavy: ADR 0158 X5 (a) (about 10 minutes)"]
fn x5a_cylinder_256_automatic_step_adaptive() {
    let r = run_case(
        "x5a",
        &ts::cylinder(256, 256),
        3.0,
        StepMode::Adaptive,
        true,
        true,
        Duration::from_secs(3 * 3600),
    );
    assert_eq!(
        r.set,
        [
            "solver.end_time=3",
            "solver.dt=0.00116",
            "solver.max_courant=0.5",
            "solver.max_dt=0.03",
            ADAPTIVE_ENTRY
        ],
        "set"
    );
    assert_eq!(
        (r.state, r.end_bits),
        (Some(RunState::Finished), 3f64.to_bits()),
        "end {}",
        r.end
    );
    assert_eq!(r.summary["mode"], "adaptive", "{}", r.summary);
    assert!(r.without == 0, "iteration events without correctors");
    assert!(clean(&r.m_hf), "M_hf {:?}", r.m_hf);
    assert!(
        r.check_sv301.is_none() && r.event_sv301.is_none(),
        "SV301: {:?} {:?}",
        r.check_sv301,
        r.event_sv301
    );
    assert!(r.file_same && r.snapshot_same, "the case file or its snapshot");
}

/// X5 (a) control: the same with the switch off: the `set` without the corrector entry, no corrector keys in the events;
/// M_hf and SV301 reported (predicted clean and no SV301 at the automatic step: not a discriminating control; X5 (b)'s is).
#[test]
#[ignore = "heavy: ADR 0158 X5 (a) control (about 10 minutes)"]
fn x5a_control_switch_off() {
    let r = run_case(
        "x5actl",
        &ts::cylinder(256, 256),
        3.0,
        StepMode::Adaptive,
        false,
        true,
        Duration::from_secs(3 * 3600),
    );
    assert_eq!(
        r.set,
        [
            "solver.end_time=3",
            "solver.dt=0.00116",
            "solver.max_courant=0.5",
            "solver.max_dt=0.03"
        ],
        "set"
    );
    assert!(r.counts.is_empty() && r.summary.is_null(), "corrector keys");
    eprintln!(
        "X5 (a) control, reported: M_hf {:?}, SV301 check {:?}, event {:?}",
        r.m_hf, r.check_sv301, r.event_sv301
    );
}

/// X5 (b): the 256 x 256 cylinder with its own `solver.dt = 0.015625` (1/64, the maintainer's step; the automatic step
/// off), "Simulate for" 3 s, adaptive correctors on: the `set` exactly `solver.end_time=3` and the corrector entry,
/// finished at 3 s bit for bit, the count above the case's 2 at every step, clean at t = 1 and 3 (L3), no SV301.
#[test]
#[ignore = "heavy: ADR 0158 X5 (b) (about 10 minutes)"]
fn x5b_cylinder_256_case_step_adaptive() {
    let r = run_case(
        "x5b",
        &cylinder_dt(256, 256, 0.015625),
        3.0,
        StepMode::Case,
        true,
        true,
        Duration::from_secs(3 * 3600),
    );
    assert_eq!(r.set, ["solver.end_time=3", ADAPTIVE_ENTRY], "set");
    assert_eq!(
        (r.state, r.end_bits),
        (Some(RunState::Finished), 3f64.to_bits()),
        "end {}",
        r.end
    );
    assert!(r.without == 0 && r.counts.iter().all(|&c| c > 2), "correctors per step");
    assert!(clean(&r.m_hf), "M_hf {:?}", r.m_hf);
    assert!(
        r.check_sv301.is_none() && r.event_sv301.is_none(),
        "SV301: {:?} {:?}",
        r.check_sv301,
        r.event_sv301
    );
    assert!(r.file_same && r.snapshot_same, "the case file or its snapshot");
}

/// X5 (b) control: the same with the switch off must fail (b): `set` exactly `solver.end_time=3`; SV301 in `check.json`
/// and in a warning event; spoky at t = 3 (M_hf >= 0.1, ADR 0154's spoky).
#[test]
#[ignore = "heavy: ADR 0158 X5 (b) control (about 5 minutes)"]
fn x5b_control_switch_off_is_spoky() {
    let r = run_case(
        "x5bctl",
        &cylinder_dt(256, 256, 0.015625),
        3.0,
        StepMode::Case,
        false,
        true,
        Duration::from_secs(3 * 3600),
    );
    assert_eq!(r.set, ["solver.end_time=3"], "set");
    assert!(r.check_sv301.is_some(), "control: no SV301 in check.json");
    assert!(r.event_sv301.is_some(), "control: no SV301 warning event");
    assert!(
        r.m_hf.last().is_some_and(|m| m.1 >= 0.1),
        "control: not spoky at t = 3: {:?}",
        r.m_hf
    );
}
