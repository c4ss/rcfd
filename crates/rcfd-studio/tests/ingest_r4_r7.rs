//! ADR 0133 R4 (incremental residual ingest with torn tails) and R7 (a diverged run with `null` values), set-up:
//! amendment 1. No runner: fixture runs made with `rcfd-project`, the run worker reads them.

mod common;

use common::{frame, next_tick, temp, CAVITY};
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::monitors::MAX_PLOT_POINTS;
use rcfd_studio::{Studio, StudioApp};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

const LINES: u64 = 20_000;

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s >> 33
}

fn line(step: u64) -> String {
    let r = 1.0 / (step as f64 + 1.0);
    format!(
        "{{\"step\":{step},\"time\":{},\"residuals\":{{\"u\":{r:e},\"p\":{},\"continuity\":{}}},\"monitors\":{{\"lid_drag\":{}}}}}\n",
        step as f64 * 1e-3,
        r * 0.5,
        r * 0.25,
        -1e-3 - r * 1e-4
    )
}

fn fixture(root: &Path, state: RunState) -> String {
    let p = Project::create(root, "w48d fixture", "rcfd test").unwrap();
    std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
    let rec = p
        .create_run("cases/a.toml", None, "rcfd test", serde_json::json!({}))
        .unwrap();
    p.write_status(&rec.id, &RunStatus::new(state)).unwrap();
    rec.id
}

fn studio(root: &Path, period: Duration, drop_torn: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = Some(period);
    st.run_switches.drop_torn_tail = drop_torn;
    st.runner_exe = root.join("no-runner-here");
    st.open_project(root);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    st
}

fn steps(st: &Studio) -> Vec<f64> {
    st.plots.series.get("res_u").map(|s| s.x.clone()).unwrap_or_default()
}

/// Write 20 000 lines in bursts with torn tails; `check` runs after every burst (two ticks later). Returns the number
/// of bursts and torn tails written.
fn write_bursts(st: &mut Studio, path: &Path, mut check: impl FnMut(&Studio, u64, Option<u64>)) -> (usize, usize) {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    let mut s = 48004u64;
    let (mut next, mut bursts, mut torn) = (1u64, 0usize, 0usize);
    // bytes of the pending torn line already written
    let mut pending: Option<(u64, usize)> = None;
    while next <= LINES {
        let mut out = String::new();
        if let Some((step, done)) = pending.take() {
            out.push_str(&line(step)[done..]);
            next = step + 1;
        }
        let n = 1 + lcg(&mut s) % 500;
        let last = (next + n - 1).min(LINES);
        for k in next..=last {
            out.push_str(&line(k));
        }
        next = last + 1;
        bursts += 1;
        let mut torn_step = None;
        if bursts % 2 == 0 && next <= LINES {
            let full = line(next);
            // alternate: a prefix cut at an LCG position, or the whole JSON without its newline
            let cut = if torn % 2 == 0 {
                1 + (lcg(&mut s) as usize) % (full.len() - 2)
            } else {
                full.len() - 1
            };
            out.push_str(&full[..cut]);
            pending = Some((next, cut));
            torn_step = Some(next);
            torn += 1;
        }
        f.write_all(out.as_bytes()).unwrap();
        f.flush().unwrap();
        let t = Instant::now();
        next_tick(st, t, Duration::from_secs(10));
        next_tick(st, Instant::now(), Duration::from_secs(10));
        check(st, last, torn_step);
    }
    (bursts, torn)
}

#[test]
fn r4_incremental_ingest_has_every_step_once_and_holds_torn_tails() {
    let root = temp("r4");
    let id = fixture(&root, RunState::Running);
    let mut st = studio(&root, Duration::from_millis(20), false);
    st.select_run(&id);
    let path = root.join("runs").join(&id).join("residuals.jsonl");
    let mut problems = Vec::new();
    let (bursts, torn) = write_bursts(&mut st, &path, |st, last, torn_step| {
        let x = steps(st);
        let want: Vec<f64> = (1..=last).map(|k| k as f64).collect();
        if x != want {
            problems.push(format!(
                "after step {last}: {} steps shown, first difference at {:?}",
                x.len(),
                x.iter().zip(&want).position(|(a, b)| a != b)
            ));
        }
        if let Some(t) = torn_step {
            if x.contains(&(t as f64)) {
                problems.push(format!("torn step {t} shown before it was completed"));
            }
        }
    });
    let x = steps(&st);
    eprintln!(
        "R4: {bursts} bursts, {torn} torn tails, {} steps shown (first {:?}, last {:?}), lid_drag points {}",
        x.len(),
        x.first(),
        x.last(),
        st.monitors.series.get("lid_drag").map_or(0, |s| s.x.len())
    );
    assert!(problems.is_empty(), "{problems:#?}");
    assert_eq!(x, (1..=LINES).map(|k| k as f64).collect::<Vec<_>>());
    assert!(torn >= 20, "under-powered: {torn} torn tails");
}

#[test]
fn r4_control_a_reader_that_moves_over_the_torn_tail_loses_steps() {
    let root = temp("r4ctl");
    let id = fixture(&root, RunState::Running);
    let mut st = studio(&root, Duration::from_millis(20), true);
    st.select_run(&id);
    let path = root.join("runs").join(&id).join("residuals.jsonl");
    write_bursts(&mut st, &path, |_, _, _| {});
    let x = steps(&st);
    let missing = LINES as usize - x.len();
    eprintln!("R4 control: {} of {LINES} steps shown, {missing} lost", x.len());
    assert!(missing >= 1, "the control lost no step");
}

const R7_LINES: [&str; 10] = [
    r#"{"step":1,"time":0.01,"residuals":{"u":0.5,"p":0.4,"continuity":0.3},"monitors":{}}"#,
    r#"{"step":2,"time":0.02,"residuals":{"u":0.4,"p":0.3,"continuity":0.2},"monitors":{}}"#,
    r#"{"step":3,"time":0.03,"residuals":{"u":0.3,"p":0.2,"continuity":0.1},"monitors":{}}"#,
    r#"{"step":4,"time":0.04,"residuals":{"u":0.2,"p":0.1,"continuity":0.05},"monitors":{}}"#,
    r#"{"step":5,"time":0.05,"residuals":{"u":0.1,"p":0.05,"continuity":0.02},"monitors":{}}"#,
    r#"{"step":6,"time":0.06,"residuals":{"u":null,"p":0.04,"continuity":0.01},"monitors":{}}"#,
    r#"{"step":7,"time":0.07,"residuals":{"u":0.3,"p":0.05,"continuity":0.02},"monitors":{}}"#,
    r#"{"step":8,"time":0.08,"residuals":{"u":0.9,"p":0.08,"continuity":0.04},"monitors":{}}"#,
    r#"{"step":9,"time":null,"residuals":{"u":null,"p":null,"continuity":null},"monitors":{}}"#,
    r#"{"step":10,"time":null,"residuals":{"u":null,"p":null,"continuity":null},"monitors":{}}"#,
];

fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

#[test]
fn r7_a_diverged_run_is_listed_with_a_gap_and_a_warning() {
    let root = temp("r7");
    let id = fixture(&root, RunState::Running);
    let p = Project::open(&root).unwrap();
    let st_failed = RunStatus {
        time: f64::NAN,
        step: 10,
        reason: Some("the solver diverged (non-finite residuals)".into()),
        ..RunStatus::new(RunState::Failed)
    };
    p.write_status(&id, &st_failed).unwrap();
    let text: String = R7_LINES.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(root.join("runs").join(&id).join("residuals.jsonl"), text).unwrap();
    let mut st = studio(&root, Duration::from_millis(50), false);
    st.select_run(&id);
    assert!(st.drain_until(Duration::from_secs(10), |s| steps(s).len() == 10));
    let b = st.browser().unwrap();
    let listed = b.runs.iter().find(|r| r.id == id).map(|r| r.state);
    let row = st.runs.row(&id).map(|r| r.state);
    let segs: Vec<Vec<f64>> = st
        .plots
        .plot_segments("res_u", true, MAX_PLOT_POINTS)
        .into_iter()
        .map(|s| s.iter().map(|p| p[0]).collect())
        .collect();
    eprintln!(
        "R7: browser {listed:?}, worker {row:?}, skipped {}, res_u segments {segs:?}, warnings {:?}",
        b.skipped.len(),
        st.runs.warnings
    );
    assert_eq!(listed, Some(RunState::Failed));
    assert_eq!(row, Some(RunState::Failed));
    assert!(b.skipped.is_empty(), "{:?}", b.skipped);
    assert_eq!(segs, vec![vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![7.0, 8.0]]);
    let warn = st
        .runs
        .warnings
        .iter()
        .find(|w| w.contains("residual u") && w.contains("step 6"))
        .cloned()
        .expect("a warning names step 6 of u");
    let mut app = StudioApp::new(st, None);
    rcfd_studio::layout::float(&mut app.st.dock, rcfd_studio::layout::Panel::Plots);
    let ctx = egui::Context::default();
    let mut drawn = false;
    for _ in 0..3 {
        let out = frame(&mut app, &ctx);
        drawn |= texts(&out.shapes).iter().any(|t| t.contains(&warn));
    }
    assert!(drawn, "the Plots panel did not draw the warning");
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct StrictLine {
    step: u64,
    time: f64,
    residuals: std::collections::BTreeMap<String, f64>,
}

#[test]
fn r7_control_the_pre_48a_reader_refuses_the_null_line() {
    let ok: Result<StrictLine, _> = serde_json::from_str(R7_LINES[4]);
    let bad: Result<StrictLine, _> = serde_json::from_str(R7_LINES[5]);
    eprintln!(
        "R7 control: step 5 {:?}, step 6 {:?}",
        ok.as_ref().map(|l| l.step),
        bad.as_ref().err().map(|e| e.to_string())
    );
    assert!(ok.is_ok());
    assert!(bad.is_err(), "the strict reader read a null residual");
}
