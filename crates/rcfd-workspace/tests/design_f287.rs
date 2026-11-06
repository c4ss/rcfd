//! ADR 0200 C-U (with amendment 1): the launch settings of `run.json` (`continue_from`, `max_steps`) are absent unless set
//! and read beside `RunSettings`; the start rule of a chained condition or map point on the runner (`start_from`) on restart
//! files written by the solver; the iterations of a continued run counted from its restart file's iteration.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_runner::records::{LaunchSettings, RunRecord, RunSettings};
use rcfd_workspace::design::{cond_result_from, restart_iteration, start_from, StartFrom};
use rcfd_workspace::templates::LID_DRIVEN_CAVITY;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-ws-f287-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The template (16 x 16) with `max_iterations = 3000`, `edit` applied to its text.
fn case(edit: impl Fn(String) -> String) -> Case {
    let text = edit(LID_DRIVEN_CAVITY.replace("max_iterations = 600", "max_iterations = 3000"));
    Case::parse_toml_with(&text, Path::new("."), None).unwrap()
}

/// The restart file of `steps` steps of `c`.
fn restart_after(c: &Case, steps: usize, file: &Path) {
    let mut sim = Simulation::new(c).unwrap_or_else(|d| panic!("{d:?}"));
    for _ in 0..steps {
        sim.step();
    }
    sim.save_restart(file).unwrap();
}

#[test]
fn cu_launch_settings_are_absent_unless_set() {
    assert_eq!(serde_json::to_value(LaunchSettings::default()).unwrap(), json!({}));
    let set = RunSettings::default();
    let plain = LaunchSettings::default().merged(&set).unwrap();
    assert_eq!(plain, serde_json::to_value(&set).unwrap(), "no key added");
    let l = LaunchSettings {
        continue_from: Some("r-20261106T120000-abcd".into()),
        max_steps: Some(20),
    };
    let both = l.merged(&set).unwrap();
    assert_eq!(both["continue_from"], "r-20261106T120000-abcd");
    assert_eq!(both["max_steps"], 20);
    let rec: RunRecord = serde_json::from_value(json!({
        "id": "r-20261106T120001-abcd", "case": "cases/c.toml", "case_snapshot_hash": "fnv64:0000000000000000",
        "solver_version": "x", "settings": both, "created": "2026-11-06T12:00:01.000000000Z"
    }))
    .unwrap();
    assert_eq!(LaunchSettings::of(&rec).unwrap(), l, "round trip");
    assert_eq!(
        RunSettings::of(&rec).unwrap(),
        set,
        "RunSettings reads the object, the two keys ignored"
    );
    let rec0: RunRecord = serde_json::from_value(json!({
        "id": "r-20261106T120001-abcd", "case": "cases/c.toml", "case_snapshot_hash": "fnv64:0000000000000000",
        "solver_version": "x", "settings": plain, "created": "2026-11-06T12:00:01.000000000Z"
    }))
    .unwrap();
    assert!(LaunchSettings::of(&rec0).unwrap().is_empty());
}

#[test]
fn cu_the_start_rule_and_the_iterations_of_a_continued_run() {
    let d = scratch("start");
    let base = case(|t| t);
    let steady = d.join("steady.rcfd");
    restart_after(&base, 25, &steady);
    assert_eq!(restart_iteration(&steady), Some(25), "the restart file of 25 steps");
    // the same case (another lid speed) continues, from iteration 25
    let slow = case(|t| t.replace("velocity_value = [1.0, 0.0, 0.0]", "velocity_value = [0.5, 0.0, 0.0]"));
    assert_eq!(start_from(&steady, &slow), StartFrom::Continue { iteration: 25 });
    // another mesh: cold, with a note naming the mesh
    let fine = case(|t| t.replace("nx = 16", "nx = 20").replace("ny = 16", "ny = 20"));
    match start_from(&steady, &fine) {
        StartFrom::Cold(Some(n)) => {
            eprintln!("C-U another mesh: {n}");
            assert!(n.starts_with("cold start: ") && n.contains("mesh"), "{n}");
        }
        other => panic!("another mesh: {other:?}"),
    }
    // a file of the transient engine: cold, no note (as in process, a transient run carries no fields)
    let piso = case(|t| {
        t.replace(
            "algorithm = \"simplec\"",
            "algorithm = \"piso\"\ndt = 0.05\nend_time = 1.0",
        )
    });
    let transient = d.join("transient.rcfd");
    restart_after(&piso, 3, &transient);
    assert_eq!(start_from(&transient, &base), StartFrom::Cold(None));
    // no file: cold, with a note
    assert!(matches!(
        start_from(&d.join("none.rcfd"), &base),
        StartFrom::Cold(Some(_))
    ));
    // S7: the steps of the run, from its restart file's iteration
    let m = |iterations: u64| -> Value {
        json!({"state": "finished", "reason": "x", "exit_code": 0, "steps": iterations,
            "solver_finish": {"event": "finish", "state": "converged", "exit_code": 0, "iterations": iterations,
                "reason": "converged"}, "reports": {"lid_drag": 0.25}})
    };
    let r = cond_result_from("slow", &m(130), "", 86);
    assert!(r.ok, "{r:?}");
    assert_eq!(r.iterations, 44);
    assert_eq!(cond_result_from("slow", &m(130), "", 0).iterations, 130);
    eprintln!("C-U: restart iteration 25; 130 - 86 = {}", r.iterations);
    let _ = std::fs::remove_dir_all(&d);
}
