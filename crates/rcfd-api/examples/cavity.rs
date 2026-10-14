//! Build the lid-driven cavity from code and run 10 SIMPLEC iterations (F-082).
//!
//! Run with `cargo run -p rcfd-api --example cavity`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::json;

fn main() {
    let mut case = Case::new();
    let settings = [
        ("title", json!("cavity from code")),
        ("mesh.generator", json!("cavity_2d")),
        ("mesh.nx", json!(16)),
        ("mesh.ny", json!(16)),
        ("physics.nu", json!(0.01)),
        ("boundaries.lid.velocity", json!("fixed")),
        ("boundaries.lid.velocity_value", json!([1.0, 0.0, 0.0])),
        ("boundaries.walls.velocity", json!("fixed")),
        ("solver.algorithm", json!("simplec")),
        ("solver.convection", json!("upwind")),
        ("reports.lid_drag.type", json!("force")),
        ("reports.lid_drag.patch", json!("lid")),
        ("reports.lid_drag.component", json!("viscous")),
    ];
    for (path, value) in settings {
        case.set(path, value).expect("valid setting");
    }
    let mut sim = Simulation::new(&case).expect("case builds");
    for _ in 0..10 {
        let s = sim.step();
        println!(
            "iteration {:3}  res_u {:.3e}  res_p {:.3e}  continuity {:.1e}",
            s.index, s.res_u, s.res_p, s.continuity
        );
    }
    println!(
        "lid drag after 10 iterations: {:.6e} N",
        sim.monitors.last("lid_drag").unwrap()
    );
}
