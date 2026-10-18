//! ADR 0067 check G3: the PISO/PIMPLE divergence guard through the case tree and the run-control driver.
//!
//! The registered candidate A does not blow up (ADR 0067 addendum 1) and candidate B needs a relaxation factor outside
//! the schema range, so the tree-level trip is provoked with a small `solver.divergence_courant_limit` on a stable run.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use std::sync::Mutex;

const PISO: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "upwind"
dt = 0.05
end_time = 0.5
"#;

fn run(extra: &str) -> runctl::RunOutcome {
    let case = Case::parse_toml(&(PISO.to_string() + extra)).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Mutex::new(Simulation::new(&case).unwrap());
    runctl::drive(&sim, &RunOptions::default(), &|| false, &mut |_| {})
}

#[test]
fn small_courant_limit_ends_the_run_diverged_with_a_located_reason() {
    let out = run("divergence_courant_limit = 1e-3\n");
    assert_eq!(out.state, "diverged", "{out:?}");
    assert_eq!(out.exit_code, exit::DIVERGED);
    assert!(
        out.reason.contains("divergence guard") && out.reason.contains("step 1"),
        "{}",
        out.reason
    );
    assert!(out.reason.contains("Courant"), "{}", out.reason);
}

#[test]
fn unset_key_runs_a_stable_case_to_the_end_and_a_negative_value_is_rejected() {
    let out = run("");
    assert_eq!(out.state, "finished", "{out:?}");
    assert_eq!(out.exit_code, exit::OK);
    let e = Case::parse_toml(&(PISO.to_string() + "divergence_courant_limit = -1.0\n")).unwrap_err();
    assert!(e.iter().any(|d| d.path == "solver.divergence_courant_limit"), "{e:?}");
    // the resolved default is the sentinel 0
    let c = Case::parse_toml(PISO).unwrap();
    assert_eq!(c.get("solver.divergence_courant_limit").unwrap().as_f64(), Some(0.0));
}
