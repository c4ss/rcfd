//! Wave 45C (ADR 0117): D-b and D-d. The case key `solver.cyclic_partner_in_h1` missed F-a and was removed (rule b); these tests now
//! check the test-only route `SimpleSolver::cyclic_partner_in_h1_test_only` (D-b) and that the removed key is refused (D-d'). Their
//! registered versions passed on the key code (ADR 0117 addendum). The FAN-01 runs and D-a are in `fan01_stall_w43c.rs` (ADR 0117
//! block); R1 / N1 in `rcfd-fv/tests/rotating_seam_h1_w45c.rs`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

/// D-b (after the removal): on the ADR 0008 M2 annulus (no cyclic faces) the route off and on give bit-identical fields after 20
/// iterations.
#[test]
fn db_route_without_cyclic_faces_is_bit_identical() {
    // the text stays a `format!` template so that rules_w19c E2 does not parse it as a standalone case
    let extra = "";
    let text = format!(
        r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 16
nz = 2
r_in = 0.5
r_out = 1.0
lz = 0.4
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_expression_x = "-1.0 [m/s] * y / 1 [m]"
velocity_expression_y = "1.0 [m/s] * x / 1 [m]"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
velocity = "symmetry"
[boundaries.zmax]
velocity = "symmetry"
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 1.0
zone = "all"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 20
{extra}
"#
    );
    let fields = |route: bool| {
        let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
        let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
        sim.simple_solver_mut().unwrap().cyclic_partner_in_h1_test_only = route;
        sim.run(20);
        ["ux", "uy", "uz", "p"].map(|f| sim.field(f).unwrap())
    };
    let base = fields(false);
    let other = fields(true);
    for (a, b) in base.iter().zip(&other) {
        assert!(
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "not bit-identical"
        );
    }
}

/// The diagnostics of `text`: those of the parse, else those of `Simulation::new` (panics if the case is accepted).
fn refusal(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    match Case::parse_toml(text) {
        Err(d) => d,
        Ok(case) => match Simulation::new(&case) {
            Ok(_) => panic!("the case must be refused"),
            Err(d) => d,
        },
    }
}

/// D-d' (ADR 0117 amendment 1): D-d with the cavity's boundary conditions. Registered (passed on the key code): the key with `pimple`
/// or `coupled` is a located error at `solver.cyclic_partner_in_h1`. After the removal: the key is refused there with any algorithm.
#[test]
fn dd2_key_outside_the_steady_solver_is_a_located_error() {
    for alg in ["pimple", "coupled", "simplec"] {
        let d = refusal(&format!(
            "[mesh]\ngenerator = \"cavity_2d\"\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\ncyclic_partner_in_h1 = true\n"
        ));
        assert!(
            d.iter().any(|x| x.path == "solver.cyclic_partner_in_h1"),
            "{alg}: {d:?}"
        );
    }
}

/// D-d as registered: a defective set-up (ADR 0117 amendment 1), the case has no boundary conditions, so it is refused before the
/// algorithm check.
#[test]
#[ignore = "known miss (defective set-up, ADR 0117 amendment 1): refused at boundaries.lid / boundaries.walls before the key check"]
fn dd_key_outside_the_steady_solver_is_a_located_error_known_miss() {
    for alg in ["pimple", "coupled"] {
        let text = format!(
            "[mesh]\ngenerator = \"cavity_2d\"\n[solver]\nalgorithm = \"{alg}\"\ncyclic_partner_in_h1 = true\n"
        );
        let d = match Case::parse_toml(&text) {
            Err(d) => d,
            Ok(case) => match Simulation::new(&case) {
                Ok(_) => panic!("{alg}: the key must be refused"),
                Err(d) => d,
            },
        };
        assert!(
            d.iter().any(|x| x.path == "solver.cyclic_partner_in_h1"),
            "{alg}: {d:?}"
        );
    }
}
