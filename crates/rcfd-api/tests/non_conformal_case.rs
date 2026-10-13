//! The `channel_2d_interface` generator through the settings tree (F-114): a non-conformal 20|27 channel carries the
//! Poiseuille flow and balances mass through the interface.
//!
//! Check fixed before running: a parabolic inlet is not expressible in the settings tree, so the inlet is uniform
//! (U = 1), the flow develops over the 2-long channel at nu = 0.1; the inlet and outlet mass flows balance to 1e-9
//! of the inlet flow and the run converges; invalid counts are diagnosed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const CASE: &str = r#"
[mesh]
generator = "channel_2d_interface"
nx = 10
ny = 20
nx_right = 10
ny_right = 27
lx = 2.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.1
rho = 1.0
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
non_orthogonal_correctors = 2
max_iterations = 3000
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
"#;

#[test]
fn non_conformal_channel_runs_and_conserves_mass() {
    let case = Case::parse_toml(CASE).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(3000);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let (qi, qo) = (sim.monitors.last("m_in").unwrap(), sim.monitors.last("m_out").unwrap());
    println!(
        "{n} iterations, inflow {qi:.10}, outflow {qo:.10}, converged {}",
        sim.is_converged()
    );
    assert!(sim.is_converged(), "not converged: {:?}", sim.last());
    assert!((qi + qo).abs() < 1e-9 * qi.abs(), "{qi} {qo}");
    assert!((qi + 0.1).abs() < 1e-9, "inflow {qi}");
}

#[test]
fn zero_block_count_means_same_as_first_block_and_negative_is_rejected() {
    // 0 selects the first block count (a conformal interface); a negative count violates the schema range
    let text = CASE.replace("ny_right = 27", "ny_right = 0");
    let case = Case::parse_toml(&text).unwrap();
    assert!(Simulation::new(&case).is_ok());
    let text = CASE.replace("ny_right = 27", "ny_right = -3");
    assert!(Case::parse_toml(&text).is_err());
}
