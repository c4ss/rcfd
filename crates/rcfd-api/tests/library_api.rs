//! F-082: build the cavity from code and step the solver programmatically.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::json;

fn cavity() -> Case {
    let mut case = Case::new();
    for (path, value) in [
        ("mesh.generator", json!("cavity_2d")),
        ("mesh.nx", json!(16)),
        ("mesh.ny", json!(16)),
        ("boundaries.lid.velocity", json!("fixed")),
        ("boundaries.lid.velocity_value", json!([1.0, 0.0, 0.0])),
        ("boundaries.walls.velocity", json!("fixed")),
        ("solver.algorithm", json!("simplec")),
    ] {
        case.set(path, value).unwrap();
    }
    case
}

#[test]
fn cavity_built_from_code_runs_ten_iterations() {
    let mut sim = Simulation::new(&cavity()).unwrap();
    let mut last = None;
    for i in 1..=10 {
        let s = sim.step();
        assert_eq!(s.index, i);
        assert!(s.res_u.is_finite() && s.continuity < 1e-2);
        last = Some(s);
    }
    assert_eq!(sim.iteration(), 10);
    assert!(!last.unwrap().done);
    assert_eq!(sim.monitors.get("res_u").unwrap().y.len(), 10);
    // field access
    assert!(sim.field("ux").unwrap().iter().any(|&u| u > 0.05));
}

#[test]
fn setting_errors_surface_with_the_path() {
    let mut case = cavity();
    let e = case.set("mesh.nx", json!("many")).unwrap_err();
    assert!(e.to_string().contains("mesh.nx"), "{e}");
    assert!(case.set("mesh.nope", json!(1)).is_err());
}
