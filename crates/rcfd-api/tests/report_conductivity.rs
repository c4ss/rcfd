//! `reports.<n>.conductivity` must not shadow `physics.conductivity` (wave 15A set-up defect E1/E5: the report key
//! defaulted to 1.0 and any positive value won, so a case with physics.conductivity = 2 reported half the heat flow).
//! The key is optional now: unset (0) uses the physics value, an explicit positive value wins.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const SLAB: &str = r#"
title = "slab"
[mesh]
generator = "box_2d"
nx = 16
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 310.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
tolerance_u = 1e-12
max_iterations = 400
[reports.q_default]
type = "heat_flow"
patch = "xmin"
[reports.q_k2]
type = "heat_flow"
patch = "xmin"
conductivity = 2.0
[reports.q_k1]
type = "heat_flow"
patch = "xmin"
conductivity = 1.0
[reports.q_k5]
type = "heat_flow"
patch = "xmin"
conductivity = 5.0
"#;

fn run(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    s.run(400);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    s
}

fn q(s: &Simulation, name: &str) -> f64 {
    s.report_values()
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap()
        .1
        .unwrap()
        .value
}

#[test]
fn unset_report_conductivity_uses_the_physics_value() {
    let s = run(SLAB);
    let (d, k2) = (q(&s, "q_default"), q(&s, "q_k2"));
    assert!(k2.abs() > 0.0);
    assert_eq!(d, k2, "default report {d} vs explicit 2.0 {k2}");
}

#[test]
fn explicit_report_conductivity_still_overrides() {
    let s = run(SLAB);
    let k2 = q(&s, "q_k2");
    assert!((q(&s, "q_k1") / k2 - 0.5).abs() < 1e-12);
    assert!((q(&s, "q_k5") / k2 - 2.5).abs() < 1e-12);
}
