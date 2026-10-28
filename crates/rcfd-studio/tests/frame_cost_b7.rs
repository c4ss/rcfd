//! ADR 0131 B7: building the egui frame with 500 runs listed takes a median of at most 8 ms over 100 frames (release, no
//! GPU). Set-up: amendment 1 item 10 (500 runs put into the browser snapshot in memory, wall time around `run_ui`, the run
//! list built with `show_rows`). Negative control: 1 ms of busy CPU per built row exceeds 8 ms.

mod common;

use common::{b1_project, frame, frames_until, temp};
use rcfd_project::{RunState, RunSummary};
use rcfd_studio::{Studio, StudioApp};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const LIMIT_MS: f64 = 8.0;

fn app_with_500_runs() -> (StudioApp, egui::Context) {
    let root = temp("b7");
    b1_project(&root);
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
        .st
        .browser()
        .is_some()));
    app.st.open_case("cases/a.toml");
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
        .st
        .editor
        .is_some()));
    let states = [
        RunState::Queued,
        RunState::Running,
        RunState::Paused,
        RunState::Finished,
        RunState::Failed,
    ];
    let runs: Vec<RunSummary> = (0..500u64)
        .map(|i| RunSummary {
            id: format!("r-20261028T{:06}-{:04x}", 100000 + i, i),
            case: format!("cases/case-{}.toml", i % 7),
            created: format!("2026-10-28T10:{:02}:{:02}.000Z", i / 60, i % 60),
            state: states[(i % 5) as usize],
            step: i * 13,
            time: i as f64 * 0.01,
            metrics: BTreeMap::from([("cd".to_string(), 0.5 + i as f64 * 1e-3), ("dp".to_string(), 12.0)]),
        })
        .collect();
    let b = app.st.project.as_mut().unwrap().browser.as_mut().unwrap();
    b.runs = runs;
    app.st.selected_run = Some(b.runs[250].id.clone());
    (app, ctx)
}

/// Median frame time in ms over 100 frames after 5 warm-up frames, and the run-list rows built per frame.
fn measure(app: &mut StudioApp, ctx: &egui::Context) -> (f64, Vec<f64>, usize) {
    for _ in 0..5 {
        let _ = frame(app, ctx);
    }
    let mut times: Vec<f64> = Vec::with_capacity(100);
    let mut rows = 0;
    for _ in 0..100 {
        let t = Instant::now();
        let _ = frame(app, ctx);
        times.push(t.elapsed().as_secs_f64() * 1e3);
        rows = app.ui_state.rows_built;
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    ((sorted[49] + sorted[50]) / 2.0, sorted, rows)
}

#[test]
fn b7_frame_build_with_500_runs_is_within_8_ms() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP b7 (crates/rcfd-studio/tests/frame_cost_b7.rs): the limit is registered for release builds");
        return;
    }
    let (mut app, ctx) = app_with_500_runs();
    let (median, sorted, rows) = measure(&mut app, &ctx);
    eprintln!(
        "b7: median {median:.3} ms, min {:.3}, p90 {:.3}, max {:.3}; {} runs listed, {rows} rows built per frame",
        sorted[0],
        sorted[89],
        sorted[99],
        app.st.browser().unwrap().runs.len()
    );
    assert_eq!(app.st.browser().unwrap().runs.len(), 500);
    assert!(rows > 0, "the run list was not drawn");
    assert!(median <= LIMIT_MS, "median {median:.3} ms");
}

#[test]
fn b7_control_one_ms_per_built_row_exceeds_8_ms() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP b7 control (crates/rcfd-studio/tests/frame_cost_b7.rs): release builds only");
        return;
    }
    let (mut app, ctx) = app_with_500_runs();
    app.ui_state.row_cost = Duration::from_millis(1);
    let (median, _, rows) = measure(&mut app, &ctx);
    eprintln!("b7 control: median {median:.3} ms with 1 ms per row, {rows} rows built per frame");
    assert!(median > LIMIT_MS, "median {median:.3} ms");
}
