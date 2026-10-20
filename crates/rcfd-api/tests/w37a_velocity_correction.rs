//! ADR 0084 A6: the opt-in `solver.velocity_correction = "face_flux"` (cell velocity correction reconstructed from the face flux correction).
//! The key absent and `"gradient"` are bit-identical; outside the steady incompressible solver the key is a located error.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const CAVITY_SIMPLE: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simple\"\nconvection = \"linear_upwind\"\nmax_iterations = 60\n";

fn with_solver(text: &str, lines: &str) -> String {
    text.replacen("[solver]\n", &format!("[solver]\n{lines}"), 1)
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn a6_key_absent_and_gradient_are_bit_identical() {
    let run = |text: &str| {
        let mut sim = sim_of(text);
        for _ in 0..60 {
            sim.step();
        }
        let bits = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<_>>();
        (
            bits(sim.field("ux").unwrap()),
            bits(sim.field("uy").unwrap()),
            bits(sim.field("p").unwrap()),
            bits(sim.flux().to_vec()),
        )
    };
    assert!(
        run(CAVITY_SIMPLE) == run(&with_solver(CAVITY_SIMPLE, "velocity_correction = \"gradient\"\n")),
        "A6: fields differ"
    );
}

#[test]
fn a6_face_flux_outside_the_steady_solver_is_a_located_error() {
    let located = |text: &str, what: &str| {
        let err = match Case::parse_toml(text) {
            Err(e) => e,
            Ok(case) => Simulation::new(&case).map(|_| ()).expect_err(what),
        };
        assert!(
            err.iter().any(|d| d.path == "solver.velocity_correction"),
            "A6 {what}: {err:?}"
        );
    };
    let base = with_solver(CAVITY_SIMPLE, "velocity_correction = \"face_flux\"\n");
    located(
        &base.replace(
            "algorithm = \"simple\"",
            "algorithm = \"pimple\"\ndt = 0.01\nend_time = 0.05",
        ),
        "pimple",
    );
    located(
        &base.replace("algorithm = \"simple\"", "algorithm = \"coupled\""),
        "coupled",
    );
    located(&base.replace("face_flux", "flux"), "unknown value");
    // in scope: accepted, and it iterates
    let mut sim = sim_of(&base);
    for _ in 0..5 {
        assert!(sim.step().res_u.is_finite());
    }
}
