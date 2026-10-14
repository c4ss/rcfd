//! Transient variable-density solver through the case schema (ADR 0009, check T7). Limits as registered in the ADR
//! before any transient code existed: a closed-box heating case file reproduces the T1 limit `|p/p_analytic - 1| <= 1e-6`,
//! a Couette case file with viscous heating reproduces the T6 A limit (1 % of `mu U^2/(2k)`), and invalid settings are
//! located. The quantitative validation of the kernel is in `rcfd-fv/tests/varden_transient*.rs`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const R: f64 = 287.0;
const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;

fn box_case(mode: &str, solver_extra: &str) -> String {
    let cv = R / (GAMMA - 1.0);
    let rho0 = P0 / (R * 300.0);
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = {R}
gamma = {GAMMA}
mode = "{mode}"
reference_pressure = {P0}
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = {q}
[initial]
temperature = 300.0
pressure = {P0}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = 0.05
end_time = 1.0
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
{solver_extra}
"#,
        q = rho0 * cv * 100.0
    )
}

fn run(case: &Case) -> Simulation {
    let mut sim = Simulation::new(case).unwrap();
    while !sim.is_done() {
        sim.step();
    }
    sim
}

#[test]
fn closed_box_heating_from_a_case_file_matches_the_analytic_pressure() {
    for mode in ["compressible", "low_mach"] {
        let case = Case::parse_toml(&box_case(mode, "")).unwrap();
        assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
        let sim = run(&case);
        assert!(!sim.is_steady());
        assert!((sim.time() - 1.0).abs() < 1e-9, "time {}", sim.time());
        let rho0 = P0 / (R * 300.0);
        let p_an = rho0 * R * 400.0;
        let pa = sim.field("p_abs").unwrap();
        let err = pa.iter().map(|p| (p / p_an - 1.0).abs()).fold(0.0, f64::max);
        let t = sim.field("T").unwrap();
        let t_err = t.iter().map(|x| (x / 400.0 - 1.0).abs()).fold(0.0, f64::max);
        println!(
            "T7 box {mode}: p error {err:.3e}, T error {t_err:.3e}, stop {:?}",
            sim.stop_reason()
        );
        assert!(err <= 1e-6, "{mode}: p error {err}");
        assert!(t_err <= 1e-6, "{mode}: T error {t_err}");
    }
}

fn couette_case(mode: &str) -> String {
    let p_init = if mode == "low_mach" { 0.0 } else { P0 };
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.004
ly = 0.01
[fluid]
density = "ideal_gas"
mode = "{mode}"
gas_constant = {R}
gamma = {GAMMA}
viscosity = 1.8e-5
conductivity = 0.026
reference_pressure = {P0}
inner_iterations = 40
inner_tolerance = 1e-10
[energy]
enabled = true
relax = 1.0
viscous_heating = true
[initial]
velocity_expression_x = "100 [m/s] * y / 0.01 [m]"
temperature = 300.0
pressure = {p_init}
[boundaries.xmin]
velocity = "fixed"
velocity_expression_x = "100 [m/s] * y / 0.01 [m]"
[boundaries.xmax]
velocity = "fixed"
velocity_expression_x = "100 [m/s] * y / 0.01 [m]"
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymax]
velocity = "fixed"
velocity_value = [100.0, 0.0, 0.0]
temperature = "fixed"
temperature_value = 300.0
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = 0.5
end_time = 20.0
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
"#
    )
}

#[test]
fn couette_viscous_heating_from_a_case_file_matches_the_analytic_profile() {
    for mode in ["compressible", "low_mach"] {
        let case = Case::parse_toml(&couette_case(mode)).unwrap();
        assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
        let sim = run(&case);
        let t = sim.field("T").unwrap();
        let scale = 1.8e-5 * 100.0 * 100.0 / (2.0 * 0.026);
        let worst = sim
            .geom
            .mesh
            .cell_centres()
            .iter()
            .zip(&t)
            .map(|(c, tc)| {
                let y = c[1] / 0.01;
                (tc - (300.0 + scale * y * (1.0 - y))).abs() / scale
            })
            .fold(0.0, f64::max);
        println!(
            "T7 Couette {mode}: max |T - T_analytic|/scale {worst:.3e}, stop {:?}",
            sim.stop_reason()
        );
        assert!(worst <= 0.01, "{mode}: profile error {worst}");
    }
}

fn first_error(text: &str) -> (String, String) {
    match Case::parse_toml(text) {
        Err(d) => (d[0].path.clone(), d[0].message.clone()),
        Ok(c) => {
            let e = c.semantic_errors();
            assert!(!e.is_empty(), "no diagnostic");
            (e[0].path.clone(), e[0].message.clone())
        }
    }
}

#[test]
fn invalid_transient_variable_density_settings_are_located() {
    let base = box_case("compressible", "");
    let (p, _) = first_error(&base.replace("algorithm = \"pimple\"", "algorithm = \"piso\""));
    assert_eq!(p, "solver.algorithm");
    let (p, _) = first_error(&base.replace("inner_iterations = 60", "inner_iterations = 0"));
    assert_eq!(p, "fluid.inner_iterations");
    let (p, _) = first_error(&box_case("compressible", "max_courant = 0.5"));
    assert_eq!(p, "solver.max_courant");
    // keys of the variable-density solver without fluid.density
    let constant = base.replace("density = \"ideal_gas\"\n", "");
    let (p, _) = first_error(&couette_case("compressible").replace("density = \"ideal_gas\"\n", ""));
    assert!(
        p.starts_with("fluid.") || p == "energy.viscous_heating" || p == "solver.algorithm",
        "{p}"
    );
    let heated = constant
        .replace("gas_constant = 287\n", "")
        .replace("gamma = 1.4\n", "")
        .replace("mode = \"compressible\"\n", "")
        .replace("reference_pressure = 101325\n", "")
        .replace("inner_iterations = 60\n", "")
        .replace("inner_tolerance = 1e-12\n", "");
    let (p, _) = first_error(&heated);
    assert_eq!(p, "energy.heat_source");
    let viscous = heated.replace("heat_source", "viscous_heating = true\n#heat_source");
    let (p, _) = first_error(&viscous);
    assert_eq!(p, "energy.viscous_heating");
}
