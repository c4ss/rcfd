//! Checks P1-P5 of ADR 0036: tabulated and piecewise temperature-dependent properties through the settings tree (F-220).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn conduction(law: &str) -> String {
    format!(
        r#"
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
{law}
[reports.q_cold]
type = "heat_flow"
patch = "xmin"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-12
tolerance_p = 1e-12
"#
    )
}

fn couette(law: &str) -> String {
    format!(
        r#"
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
{law}
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
"#
    )
}

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..iterations {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    sim
}

const POLY: &str = "law = \"polynomial\"\ncoefficients = [-2.0, 0.01]";
const PL_LINEAR: &str = "law = \"piecewise_linear\"\ntemperatures = [300.0, 400.0]\nvalues = [1.0, 2.0]";
const PP_LINEAR: &str = "law = \"piecewise_polynomial\"\nbreakpoints = [300.0, 350.0, 400.0]\npiece_order = 2\npiece_coefficients = [-2.0, 0.01, -2.0, 0.01]";

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

#[test]
fn p1_piecewise_linear_equals_the_polynomial_it_samples() {
    let a = run(&conduction(POLY), 200);
    let b = run(&conduction(PL_LINEAR), 200);
    let d = max_diff(&a.field("T").unwrap(), &b.field("T").unwrap());
    let (qa, qb) = (a.monitors.last("q_cold").unwrap(), b.monitors.last("q_cold").unwrap());
    eprintln!(
        "P1: max |dT| {d:.3e} K, heat flow relative difference {:.3e}",
        (qa - qb).abs() / qa.abs()
    );
    assert!(d <= 1e-9);
    assert!((qa - qb).abs() <= 1e-9 * qa.abs());
}

#[test]
fn p3_piecewise_polynomial_equals_p1() {
    let a = run(&conduction(PL_LINEAR), 200);
    let b = run(&conduction(PP_LINEAR), 200);
    let d = max_diff(&a.field("T").unwrap(), &b.field("T").unwrap());
    eprintln!("P3: max |dT| {d:.3e} K");
    assert!(d <= 1e-9);
}

#[test]
fn p2_kinked_conductivity_follows_the_kirchhoff_transform() {
    let law = "law = \"piecewise_linear\"\ntemperatures = [300.0, 350.0, 400.0]\nvalues = [1.0, 2.0, 4.0]";
    let sim = run(&conduction(law), 300);
    let c = sim.geom.mesh.cell_centres();
    let t = sim.field("T").unwrap();
    // K(T) = theta + theta^2/100 (theta <= 50), 75 + 2 s + s^2/50 (s = T - 350); K(400) = 225; K linear in x (L = 1)
    let inv = |kk: f64| {
        if kk <= 75.0 {
            // theta^2/100 + theta - kk = 0
            300.0 + 50.0 * (-1.0 + (1.0 + kk / 25.0).sqrt())
        } else {
            // s^2/50 + 2 s - (kk - 75) = 0
            350.0 + 25.0 * (-2.0 + (4.0 + 4.0 * (kk - 75.0) / 50.0).sqrt())
        }
    };
    let err = (0..c.len())
        .map(|i| (t[i] - inv(225.0 * c[i][0])).abs())
        .fold(0.0, f64::max);
    let q = sim.monitors.last("q_cold").unwrap().abs();
    let a_wall = 0.1 * (1.0 / 40.0);
    let want = 225.0 * a_wall;
    eprintln!(
        "P2: max |T - exact| {err:.3e} K (limit 0.5), heat flow {q:.6e} vs {want:.6e} ({:+.3} %)",
        100.0 * (q / want - 1.0)
    );
    assert!(err <= 0.5);
    assert!((q / want - 1.0).abs() <= 0.01);
}

#[test]
fn p4_vogel_viscosity_in_couette_flow() {
    let law = "law = \"vogel\"\nvogel_a = 0.01\nvogel_b = 300.0\nvogel_c = 100.0";
    let sim = run(&couette(law), 1200);
    let c = sim.geom.mesh.cell_centres();
    let ux = sim.field("ux").unwrap();
    let mu = |y: f64| 0.01 * (300.0 / (300.0 + 100.0 * y - 100.0)).exp();
    let integral = |y1: f64| {
        let n = 2000;
        let h = y1 / n as f64;
        let mut s = 1.0 / mu(0.0) + 1.0 / mu(y1);
        for i in 1..n {
            s += (if i % 2 == 1 { 4.0 } else { 2.0 }) / mu(i as f64 * h);
        }
        s * h / 3.0
    };
    let tau = 1.0 / integral(1.0);
    let e = (0..c.len())
        .map(|i| (ux[i] - tau * integral(c[i][1])).abs())
        .fold(0.0, f64::max);
    eprintln!("P4: max |u - exact| {e:.3e} U (limit 1e-2)");
    assert!(e <= 1e-2);
}

#[test]
fn p5_malformed_tables_are_located() {
    let bad = |law: &str, path: &str, needle: &str| {
        let text = conduction(law);
        let err = match Case::parse_toml(&text) {
            Err(d) => d,
            Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
        };
        assert!(
            err.iter().any(|d| d.path == path && d.message.contains(needle)),
            "{path} / {needle}: {err:?}"
        );
    };
    bad(
        "law = \"piecewise_linear\"\ntemperatures = [300.0, 350.0, 350.0]\nvalues = [1.0, 2.0, 3.0]",
        "properties.conductivity.temperatures",
        "x[2]",
    );
    bad(
        "law = \"piecewise_linear\"\ntemperatures = [300.0, 350.0, 400.0]\nvalues = [1.0, 2.0]",
        "properties.conductivity.values",
        "entries",
    );
    bad(
        "law = \"piecewise_polynomial\"\nbreakpoints = [300.0, 400.0]\npiece_order = 0\npiece_coefficients = [1.0, 0.0]",
        "properties.conductivity.piece_coefficients",
        "piece_order",
    );
    bad(
        "law = \"piecewise_polynomial\"\nbreakpoints = [300.0, 400.0]\npiece_order = 2\npiece_coefficients = [1.0, 0.0, 3.0]",
        "properties.conductivity.piece_coefficients",
        "multiple",
    );
    bad(
        "law = \"vogel\"\nvogel_a = 0.0\nvogel_b = 300.0\nvogel_c = 100.0",
        "properties.conductivity.vogel_a",
        "positive",
    );
}
