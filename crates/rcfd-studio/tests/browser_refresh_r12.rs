//! ADR 0133 R12: with 500 runs in the project, a change of one run's `status.json` makes the project worker
//! re-summarise exactly that run, and the browser it sends equals `Browser::load` of the folder. Negative control: the
//! 48B full reload re-summarises all 500.

mod common;

use common::{temp, CAVITY};
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::project_view::Browser;
use rcfd_studio::Studio;
use std::time::{Duration, Instant};

const RUNS: usize = 500;

fn same_lists(a: &Browser, b: &Browser) -> bool {
    a.cases == b.cases
        && a.meshes == b.meshes
        && a.designs == b.designs
        && a.skipped == b.skipped
        && a.errors == b.errors
        && a.runs.len() == b.runs.len()
        && a.runs.iter().zip(&b.runs).all(|(x, y)| {
            (
                &x.id,
                &x.case,
                &x.created,
                x.state,
                x.step,
                x.time.to_bits(),
                &x.metrics,
            ) == (
                &y.id,
                &y.case,
                &y.created,
                y.state,
                y.step,
                y.time.to_bits(),
                &y.metrics,
            )
        })
}

#[test]
fn r12_a_run_change_re_summarises_only_that_run() {
    let root = temp("r12");
    let p = Project::create(&root, "r12", "rcfd test").unwrap();
    std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
    let t0 = Instant::now();
    let ids: Vec<String> = (0..RUNS)
        .map(|_| {
            p.create_run("cases/a.toml", None, "rcfd test", serde_json::json!({}))
                .unwrap()
                .id
        })
        .collect();
    eprintln!("R12: {RUNS} runs made in {:.1} s", t0.elapsed().as_secs_f64());
    let mut st = Studio::in_memory();
    st.run_poll = None;
    st.open_project(&root);
    assert!(st.drain_until(Duration::from_secs(60), |s| s.browser().is_some()));
    let full = st.browser().unwrap().summaries_read;
    // let the watcher take its first scans
    std::thread::sleep(Duration::from_millis(1500));
    st.drain();
    let target = &ids[RUNS / 2];
    let status = RunStatus {
        step: 77,
        ..RunStatus::new(RunState::Running)
    };
    p.write_status(target, &status).unwrap();
    assert!(
        st.drain_until(Duration::from_secs(10), |s| s
            .browser()
            .unwrap()
            .runs
            .iter()
            .any(|r| &r.id == target && r.step == 77)),
        "the change was not listed"
    );
    let b = st.browser().unwrap().clone();
    let fresh = Browser::load(&Project::open(&root).unwrap());
    eprintln!(
        "R12: open read {full} summaries; after one status change the worker read {}; equal to Browser::load: {}",
        b.summaries_read,
        same_lists(&b, &fresh)
    );
    assert_eq!(b.summaries_read, 1);
    assert!(
        same_lists(&b, &fresh),
        "the refreshed browser differs from Browser::load"
    );
    // negative control: the full reload of 48B
    assert_eq!(fresh.summaries_read, RUNS);
    assert_eq!(full, RUNS);
}
