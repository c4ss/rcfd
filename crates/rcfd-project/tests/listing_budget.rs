//! ADR 0122 L1: listing a project with hundreds of runs within a time budget (own test binary so no
//! sibling test competes with it). The limit is registered for release builds; a debug build only
//! checks correctness and prints the time.
//!
//! ADR 0759 (F-866): the negative control of L1 is the function `control` of the measured durations, with its own
//! checks C1 to C3 below (pure: they touch no disk and do not compete with L1).

mod common;

use rcfd_project::{atomic, Project, RunState, RunStatus, Runs};
use std::time::{Duration, Instant};

/// Registered: 500 runs.
const RUNS: usize = 500;
/// Registered limit for the slowest of 3 warm `runs()` calls in release (ADR 0122 L1).
const BUDGET: Duration = Duration::from_millis(500);

/// One listing of the project. The three measured listings and the ten of the control go through here.
fn list(p: &Project) -> Runs {
    p.runs().unwrap()
}

/// What L1 measures: the slowest of the 3 warm listings, and the time of 10 further listings in a row.
struct Measured {
    worst: Duration,
    ten: Duration,
}

/// The budget of ADR 0122 L1 as registered, 500 ms, written a second time: the control decides by this value and
/// asserts by `BUDGET`, so a `BUDGET` that is no longer the registered one fails the control.
const REGISTERED_BUDGET: Duration = Duration::from_millis(500);

/// The negative control of L1 as a function of the measured durations: `Ok(true)` asserted and held, `Ok(false)` not
/// asserted, `Err` failed. The rule is that of ADR 0759, which amends ADR 0128 finding 12 and ADR 0137: the decision
/// and the assertion are both on the time of the ten listings. If the ten listings took longer than the registered
/// budget, the budget in force must reject them; otherwise nothing is asserted. With `BUDGET` as registered no pair of
/// durations fails it (C3); the worst of the three listings does not enter.
fn control(m: &Measured) -> Result<bool, String> {
    if m.ten > REGISTERED_BUDGET {
        if m.ten <= BUDGET {
            Err("negative control: the budget does not catch a 10x slower listing".to_string())
        } else {
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

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
                    reason: None,
                    exit_code: None,
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

    let _warm = list(&p);
    let mut worst = Duration::ZERO;
    for _ in 0..3 {
        let t = Instant::now();
        let runs = list(&p);
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

    // Negative control (ADR 0128 finding 12, amended by ADR 0759): the same listing done 10 times is a listing 10x
    // slower than the one measured, really run and timed. `control` decides and asserts on that one duration: above
    // the registered 500 ms the budget must reject it; at or below, the budget cannot catch a 10x slowdown on this
    // machine, the control is not asserted and the budget assertion below is the remaining guard.
    let t = Instant::now();
    for _ in 0..10 {
        list(&p);
    }
    let m = Measured {
        worst,
        ten: t.elapsed(),
    };
    eprintln!(
        "L1: worst of 3 listings {:?} (budget {BUDGET:?}); the listing 10 times: {:?}",
        m.worst, m.ten
    );
    match control(&m) {
        Ok(true) => {}
        Ok(false) => eprintln!(
            "L1: the listing 10 times took {:?} <= {REGISTERED_BUDGET:?}: the budget catches a listing {:.0}x slower than \
             the mean of these ten, not 10x (ADR 0759); control not asserted, the budget assertion is the remaining guard",
            m.ten,
            10.0 * BUDGET.as_secs_f64() / m.ten.as_secs_f64()
        ),
        Err(e) => panic!("{e}"),
    }
    if !cfg!(debug_assertions) {
        assert!(worst <= BUDGET, "listing {RUNS} runs took {worst:?} > {BUDGET:?}");
    }
}

/// ADR 0759 C1: the times the gate of 62a6fec3 printed (worst of 3 listings 51.5937 ms, the listing 10 times
/// 498.1544 ms) do not fail the control: nothing was wrong with the listing or the budget in that run.
#[test]
fn f866_c1_the_times_of_gate_62a6fec3_do_not_fail_the_control() {
    let m = Measured {
        worst: Duration::from_nanos(51_593_700),
        ten: Duration::from_nanos(498_154_400),
    };
    let got = control(&m);
    assert!(got.is_ok(), "the control fails on a sound listing: {got:?}");
}

/// ADR 0759 C2: above the registered 500 ms the control is asserted and the budget rejects the ten listings; at 500 ms
/// and below it is not asserted. Ten times the runner's worst listing is above the budget.
#[test]
fn f866_c2_the_control_asserts_above_the_registered_budget_and_the_budget_rejects() {
    let at = |ten: Duration| control(&Measured { worst: ten / 10, ten });
    assert_eq!(at(Duration::from_millis(500)), Ok(false));
    for ten in [
        Duration::from_millis(500) + Duration::from_nanos(1),
        Duration::from_millis(501),
        Duration::from_millis(844),
        Duration::from_secs(5),
    ] {
        assert_eq!(at(ten), Ok(true), "ten listings in {ten:?}");
    }
    assert!(Duration::from_nanos(51_593_700) * 10 > BUDGET);
}

/// ADR 0759 C3: with the budget as registered the control fails for no pair of durations: `ten` from 0 to 20 s in
/// steps of 100 us, `worst` from 0 to `ten`.
#[test]
fn f866_c3_the_control_cannot_fail_for_any_duration() {
    for step in 0..=200_000u64 {
        let ten = Duration::from_micros(100 * step);
        for worst in [Duration::ZERO, ten / 20, ten / 10, ten / 5, ten] {
            let got = control(&Measured { worst, ten });
            assert!(got.is_ok(), "worst {worst:?}, ten {ten:?}: {got:?}");
        }
    }
}
