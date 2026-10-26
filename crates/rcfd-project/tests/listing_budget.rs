//! ADR 0122 L1: listing a project with hundreds of runs within a time budget (own test binary so no
//! sibling test competes with it). The limit is registered for release builds; a debug build only
//! checks correctness and prints the time.

mod common;

use rcfd_project::{atomic, Project, RunState, RunStatus};
use std::time::{Duration, Instant};

/// Registered: 500 runs.
const RUNS: usize = 500;
/// Registered limit for the slowest of 3 warm `runs()` calls in release (ADR 0122 L1).
const BUDGET: Duration = Duration::from_millis(500);
/// Negative control: the same listing with 2 ms of extra cost per run must exceed the budget.
const EXTRA: Duration = Duration::from_millis(2);

#[test]
fn l1_listing_hundreds_of_runs_within_budget() {
    let dir = common::scratch("l1");
    let p = Project::create(&dir.join("proj"), "l1", "0").unwrap();
    std::fs::write(p.root().join("cases/a.json"), b"{\"case\": 1}").unwrap();
    std::fs::write(p.root().join("cases/b.json"), b"{\"case\": 2}").unwrap();
    let t = Instant::now();
    let mut last_finished_a = None;
    for i in 0..RUNS {
        let case = if i % 2 == 0 { "cases/a.json" } else { "cases/b.json" };
        let r = p.create_run(case, None, "0", serde_json::json!({"i": i})).unwrap();
        if i % 3 == 0 {
            p.write_status(
                &r.id,
                &RunStatus {
                    state: RunState::Finished,
                    step: i as u64,
                    time: 1.0,
                    pid: None,
                    updated: rcfd_project::time::now(),
                },
            )
            .unwrap();
            atomic::write_json(
                &p.run_dir(&r.id).unwrap().join("metrics.json"),
                &serde_json::json!({"dp": i}),
            )
            .unwrap();
            if case == "cases/a.json" {
                last_finished_a = Some(r.id.clone());
            }
        }
    }
    eprintln!("L1: created {RUNS} runs in {:?}", t.elapsed());

    let _warm = p.runs().unwrap();
    let mut worst = Duration::ZERO;
    for _ in 0..3 {
        let t = Instant::now();
        let runs = p.runs().unwrap();
        worst = worst.max(t.elapsed());
        assert_eq!((runs.runs.len(), runs.skipped.len()), (RUNS, 0));
        assert!(runs
            .runs
            .windows(2)
            .all(|w| (&w[0].created, &w[0].id) <= (&w[1].created, &w[1].id)));
    }
    assert_eq!(
        p.latest_finished("cases/a.json").unwrap().map(|r| r.id),
        last_finished_a
    );
    assert_eq!(p.runs_in_state(RunState::Finished).unwrap().len(), RUNS.div_ceil(3));

    // Negative control: per-run cost of EXTRA in the same listing path (ids, then one summary each).
    let t = Instant::now();
    let ids = p.run_ids().unwrap();
    for id in &ids {
        std::thread::sleep(EXTRA);
        p.run_summary(id).unwrap();
    }
    let slow = t.elapsed();
    eprintln!("L1: worst of 3 listings {worst:?} (budget {BUDGET:?}); with {EXTRA:?}/run: {slow:?}");
    assert!(
        slow > BUDGET,
        "negative control: the budget does not catch a {EXTRA:?}/run regression"
    );
    if !cfg!(debug_assertions) {
        assert!(worst <= BUDGET, "listing {RUNS} runs took {worst:?} > {BUDGET:?}");
    }
}
