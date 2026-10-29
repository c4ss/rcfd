//! ADR 0160 (job 48Z), the checks that need this job's new API: S8 (the solver's own time is not snapped), V2 (the pre-run
//! viscosity of a rheology case is the one the first step sets, bit for bit) and V3 (the size of the turbulence
//! approximation, reported). Registered in ADR 0160 before any run; not changed afterwards. Every case text is a `format!`
//! template (ADR 0156 B2a's literal scan skips it).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

/// ADR 0145's PISO cavity at step `dt` to `end`.
fn piso_text(dt: f64, end: f64) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\n\
         velocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"piso\"\n\
         convection = \"linear_upwind\"\ndt = {dt:.17e}\nend_time = {end:.17e}\n"
    )
}

#[test]
fn s8_the_solver_time_is_not_snapped() {
    let dt = 0.01;
    let t3 = dt + dt + dt;
    let end = t3 + 1e-13;
    let mut sim = Simulation::new(&parse(&piso_text(dt, end))).unwrap();
    let mut n = 0;
    while !sim.is_done() {
        sim.step();
        n += 1;
        assert!(n <= 10);
    }
    eprintln!(
        "S8: {n} steps; reported {:.17e}, solver {:.17e}, t_3 {t3:.17e}",
        sim.time(),
        sim.solver_time()
    );
    assert_eq!(n, 3);
    assert_eq!(sim.time().to_bits(), end.to_bits());
    assert_eq!(sim.solver_time().to_bits(), t3.to_bits());
}

const H: f64 = 1.0;
const L: f64 = 6.0;
const U: f64 = 1.0;

/// `rheology.rs`'s channel text, made transient exactly as its `transient_runs_use_the_law_too` makes it.
fn channel(physics: &str, rheology: &str) -> String {
    let text = format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 60
ny = 20
lx = {L}
ly = {H}
depth = 0.1
[physics]
rho = 1.0
{physics}
[initial]
velocity = [{U}, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [{U}, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 4000
tolerance_u = 1e-9
tolerance_p = 1e-9

[rheology]
{rheology}
"#
    );
    text.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.02\nend_time = 0.4",
    )
    .replace("nx = 60", "nx = 20")
    .replace("ny = 20", "ny = 8")
}

/// The Taylor-Green vortex of `sgs_tree.rs` (Smagorinsky), step `dt`.
fn sgs_text(dt: f64) -> String {
    format!(
        r#"
title = "Taylor-Green vortex with a subgrid model"
[mesh]
generator = "box_2d"
nx = 16
ny = 16
lx = 6.283185307179586
ly = 6.283185307179586
periodic_x = true
periodic_y = true
[physics]
nu = 0.05
[initial]
velocity = [0.0, 0.0, 0.0]
velocity_expression_x = "1 [m/s] * sin(x / 1 [m]) * cos(y / 1 [m])"
velocity_expression_y = "-1 [m/s] * cos(x / 1 [m]) * sin(y / 1 [m])"
[solver]
algorithm = "piso"
convection = "central"
dt = {dt}
end_time = 1.0
[turbulence]
model = "smagorinsky"
"#
    )
}

/// The Spalart-Allmaras DDES channel of `des_tree.rs`, step `dt`.
fn des_text(dt: f64) -> String {
    format!(
        r#"
title = "SA DDES channel"
[mesh]
generator = "box_2d"
nx = 8
ny = 20
lx = 2.0
ly = 1.0
periodic_x = true
[physics]
nu = 0.001
[initial]
velocity = [0.0, 0.0, 0.0]
velocity_expression_x = "4 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "upwind"
dt = {dt}
end_time = 1.0
[turbulence]
model = "spalart_allmaras"
[turbulence.sa]
wall_mode = "resolved"
des = "ddes"
"#
    )
}

/// (pre-run viscosity, the solver's viscosity after the first step) of a case.
fn before_and_after(text: &str) -> (Option<Vec<f64>>, Option<Vec<f64>>) {
    let mut sim = Simulation::new(&parse(text)).unwrap_or_else(|e| panic!("{e:?}"));
    let before = sim.next_step_viscosity();
    sim.step();
    (before, sim.solver_face_viscosity())
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn v2_the_pre_run_viscosity_of_a_rheology_case_is_the_first_step_s() {
    let arms = [
        (
            "thin",
            channel(
                "nu = 0.1",
                "model = \"power_law\"\nconsistency_index = 0.1\npower_index = 0.4\nmax_viscosity = 10.0\n",
            ),
        ),
        (
            "limit",
            channel(
                "nu = 0.9",
                "model = \"carreau\"\nzero_shear_viscosity = 0.1\ninfinite_shear_viscosity = 0.0\ntime_constant = 1e-12\nrelax = 1.0\n",
            ),
        ),
    ];
    for (name, text) in arms {
        let (before, after) = before_and_after(&text);
        let (b, a) = (
            before.expect("a law gives a viscosity"),
            after.expect("the step set one"),
        );
        let (lo, hi) = b
            .iter()
            .fold((f64::INFINITY, 0.0_f64), |(l, h), &x| (l.min(x), h.max(x)));
        eprintln!("V2 {name}: {} faces, pre-run viscosity in [{lo:e}, {hi:e}]", b.len());
        assert!(
            bits(&b) == bits(&a),
            "V2 {name}: the pre-run viscosity differs from the first step's"
        );
    }
}

#[test]
fn v3_turbulence_pre_run_viscosity_against_the_first_step_reported() {
    for (name, text) in [("sgs", sgs_text(0.01)), ("des", des_text(0.02))] {
        let (before, after) = before_and_after(&text);
        let (b, a) = (
            before.expect("a model gives a viscosity"),
            after.expect("the step set one"),
        );
        let rel = b
            .iter()
            .zip(&a)
            .map(|(x, y)| (x - y).abs() / y.abs().max(1e-300))
            .fold(0.0_f64, f64::max);
        let (lo_b, hi_b) = b
            .iter()
            .fold((f64::INFINITY, 0.0_f64), |(l, h), &x| (l.min(x), h.max(x)));
        let (lo_a, hi_a) = a
            .iter()
            .fold((f64::INFINITY, 0.0_f64), |(l, h), &x| (l.min(x), h.max(x)));
        eprintln!(
            "V3 {name}: pre-run [{lo_b:e}, {hi_b:e}], after step 1 [{lo_a:e}, {hi_a:e}], largest relative difference {rel:e}"
        );
        assert_eq!(b.len(), a.len());
    }
}
