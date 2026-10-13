//! Temperature-dependent properties (F-053) through the settings tree.
//!
//! Checks fixed before running:
//! * plane Couette flow (periodic box, 4 x 40 cells, lower wall at rest and 300 K, upper wall at U = 1 and 400 K,
//!   rho = 1, pure conduction so T is linear) with the polynomial viscosity mu = 0.01 (1 + 0.02 (T - 300)):
//!   the constant shear stress gives u/U = ln(1 + b y/H) / ln(1 + b) with b = 2; the maximum deviation must be
//!   below 1 percent of U; a Schmidt-number-1 scalar (0 at the lower wall, 1 at the upper) obeys the same law
//!   because its diffusivity is nu(T)/Sc, so it must equal u/U to 1e-6;
//! * steady conduction between two walls with k = k0 (1 + beta (T - 300)), k0 = 1, beta = 0.01, 300 K and
//!   400 K, fluid at rest (Kirchhoff transform: k0 (theta + beta theta^2 / 2) is linear in x): maximum
//!   temperature error below 0.2 percent of the range and the heat_flow report within 0.5 percent of the exact flux;
//! * conflicts with the rheology, the user-function viscosity and a missing energy equation are diagnosed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const COUETTE: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.4
ly = 1.0
periodic_x = true
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
buoyancy = false
relax = 1.0
[initial]
temperature = 350.0
[boundaries.ymin]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
temperature = "fixed"
temperature_value = 400.0
[properties.viscosity]
law = "polynomial"
coefficients = [-0.05, 0.0002]
[scalars.s]
schmidt = 1.0
[scalars.s.boundaries.ymin]
type = "fixed"
value = 0.0
[scalars.s.boundaries.ymax]
type = "fixed"
value = 1.0
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-10
pressure_rel_tol = 1e-10
pressure_max_iter = 1000
max_iterations = 1500
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..iterations {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    sim
}

#[test]
fn couette_flow_with_temperature_dependent_viscosity() {
    let sim = run(COUETTE, 1200);
    let c = sim.geom.mesh.cell_centres();
    let ux = sim.field("ux").unwrap();
    let s = sim.field("s").unwrap();
    let t = sim.field("T").unwrap();
    let b = 2.0f64;
    let exact = |y: f64| (1.0 + b * y).ln() / (1.0 + b).ln();
    let mut e_u: f64 = 0.0;
    let mut e_s: f64 = 0.0;
    let mut e_t: f64 = 0.0;
    for i in 0..c.len() {
        let y = c[i][1];
        e_u = e_u.max((ux[i] - exact(y)).abs());
        e_s = e_s.max((s[i] - ux[i]).abs());
        e_t = e_t.max((t[i] - (300.0 + 100.0 * y)).abs() / 100.0);
    }
    eprintln!("Couette: max |u - exact| {e_u:.3e} (U), |s - u| {e_s:.3e}, T error {e_t:.3e}");
    assert!(e_t < 1e-6, "temperature is not linear: {e_t}");
    assert!(e_u < 0.01, "velocity error {e_u}");
    assert!(e_s < 1e-6, "scalar differs from the velocity: {e_s}");
}

const CONDUCTION: &str = r#"
[mesh]
generator = "box_2d"
nx = 40
ny = 4
lx = 1.0
ly = 0.1
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
buoyancy = false
relax = 1.0
[initial]
temperature = 350.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 400.0
[boundaries.ymin]
velocity = "fixed"
temperature = "zero_gradient"
[boundaries.ymax]
velocity = "fixed"
temperature = "zero_gradient"
[properties.conductivity]
law = "polynomial"
coefficients = [-2.0, 0.01]
[reports.q_hot]
type = "heat_flow"
patch = "xmax"
[reports.q_cold]
type = "heat_flow"
patch = "xmin"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-12
tolerance_p = 1e-12
"#;

#[test]
fn conduction_with_temperature_dependent_conductivity_follows_the_kirchhoff_transform() {
    let sim = run(CONDUCTION, 200);
    let c = sim.geom.mesh.cell_centres();
    let t = sim.field("T").unwrap();
    let (k0, beta) = (1.0, 0.01);
    // k0 (theta + beta theta^2 / 2) = flux_int * x / L with flux_int = k0 (100 + beta 100^2 / 2)
    let total = k0 * (100.0 + beta * 100.0 * 100.0 / 2.0);
    let exact = |x: f64| {
        let r = total * x / k0;
        300.0 + (-1.0 + (1.0 + 2.0 * beta * r).sqrt()) / beta
    };
    let err = (0..c.len())
        .map(|i| (t[i] - exact(c[i][0])).abs() / 100.0)
        .fold(0.0, f64::max);
    let q_hot = sim.monitors.last("q_hot").unwrap();
    let q_cold = sim.monitors.last("q_cold").unwrap();
    // the heat leaves through the hot wall (negative of the flow entering there is positive out): fluxes balance
    eprintln!("conduction: T error {err:.3e} of the range, q_hot {q_hot:.6e}, q_cold {q_cold:.6e}");
    assert!(err < 0.002, "{err}");
    assert!(
        (q_hot + q_cold).abs() < 1e-9 * q_hot.abs().max(1e-30),
        "heat flows do not balance"
    );
    // exact flux density total / L times the face area of the wall patch
    let depth = 1.0 / 40.0;
    let a_wall = 0.1 * depth;
    let want = total / 1.0 * a_wall;
    assert!(
        (q_cold.abs() / want - 1.0).abs() < 0.005,
        "heat flow {q_cold} vs {want}"
    );
}

#[test]
fn property_conflicts_are_diagnosed() {
    let bad = |text: &str, path: &str| {
        let err = match Case::parse_toml(text) {
            Err(d) => d,
            Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
        };
        assert!(err.iter().any(|d| d.path.starts_with(path)), "{path}: {err:?}");
    };
    bad(
        &COUETTE.replace("[energy]\nenabled = true", "[energy]\nenabled = false"),
        "properties.viscosity.law",
    );
    bad(
        &COUETTE.replace(
            "[solver]",
            "[rheology]\nmodel = \"power_law\"\nconsistency_index = 0.01\npower_index = 1.0\n[solver]",
        ),
        "properties.viscosity.law",
    );
    bad(
        &COUETTE.replace("coefficients = [-0.05, 0.0002]", "coefficients = []"),
        "properties.viscosity.coefficients",
    );
    bad(
        &COUETTE.replace("law = \"polynomial\"", "law = \"sutherland\"\nreference_value = -1.0"),
        "properties.viscosity.reference_value",
    );
    bad(
        &COUETTE.replace("algorithm = \"simplec\"", "algorithm = \"coupled\""),
        "properties.viscosity.law",
    );
}
