//! ADR 0067 M3: restart benefit of an operating map on a real laminar channel, and the case-evaluator path of the map.

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseMapEvaluator, RunLimits};
use rcfd_api::design::opmap::{self, Axis, MapOptions, PointStatus};
use serde_json::json;

fn channel(u: f64) -> Case {
    Case::parse_toml(&format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [{u}, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
"#
    ))
    .unwrap()
}

fn map(cold: bool) -> opmap::MapOutcome {
    let axes = vec![Axis {
        path: "boundaries.inlet.velocity_value".into(),
        values: (0..8).map(|i| json!([0.5 + 0.1 * i as f64, 0.0, 0.0])).collect(),
    }];
    let pts = opmap::grid_points(&axes, false).unwrap();
    let ev = CaseMapEvaluator {
        base: channel(0.5),
        limits: RunLimits::default(),
    };
    opmap::run_map(
        &pts,
        &ev,
        &MapOptions {
            cold,
            ..Default::default()
        },
        &mut |_| {},
    )
}

#[test]
fn m3_warm_chain_needs_at_most_085_of_the_cold_iterations() {
    let warm = map(false);
    let cold = map(true);
    let it = |o: &opmap::MapOutcome| o.rows.iter().map(|r| r.iterations).sum::<usize>();
    let (iw, ic) = (it(&warm), it(&cold));
    println!(
        "M3: warm {iw} iterations, cold {ic}, ratio {:.3}",
        iw as f64 / ic as f64
    );
    for (w, c) in warm.rows.iter().zip(&cold.rows) {
        println!(
            "M3 point {}: warm {} it ({}), cold {} it, dp warm {:.8e} cold {:.8e}",
            w.index, w.iterations, w.restart, c.iterations, w.metrics["dp"], c.metrics["dp"]
        );
        assert_eq!(
            w.status,
            PointStatus::Converged,
            "warm point {}: {}",
            w.index,
            w.message
        );
        assert_eq!(
            c.status,
            PointStatus::Converged,
            "cold point {}: {}",
            c.index,
            c.message
        );
        let rel = (w.metrics["dp"] - c.metrics["dp"]).abs() / c.metrics["dp"].abs();
        assert!(rel <= 5e-4, "point {}: dp differs by {rel:.2e}", w.index);
    }
    assert!(warm.rows.iter().skip(1).all(|r| r.restart == "warm"));
    assert!(
        iw as f64 <= 0.85 * ic as f64,
        "warm {iw} vs cold {ic}: ratio {:.3}",
        iw as f64 / ic as f64
    );
}
