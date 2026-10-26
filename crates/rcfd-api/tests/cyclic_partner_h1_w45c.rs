//! Wave 45C (ADR 0117): the case key `solver.cyclic_partner_in_h1` at its defaults (D-b) and its refusal outside the steady solver
//! (D-d). The FAN-01 runs and D-a are in `fan01_stall_w43c.rs` (ADR 0117 block); R1 / N1 in `rcfd-fv/tests/rotating_seam_h1_w45c.rs`.
//! Limits registered in ADR 0117 before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

/// D-b: on the ADR 0008 M2 annulus (no cyclic faces) the key absent and true give bit-identical fields after 20 iterations.
#[test]
fn db_key_without_cyclic_faces_is_bit_identical() {
    let text = |extra: &str| {
        format!(
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
        )
    };
    let fields = |extra: &str| {
        let case = Case::parse_toml(&text(extra)).unwrap_or_else(|e| panic!("{e:?}"));
        let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
        sim.run(20);
        ["ux", "uy", "uz", "p"].map(|f| sim.field(f).unwrap())
    };
    let base = fields("");
    let other = fields("cyclic_partner_in_h1 = true");
    for (a, b) in base.iter().zip(&other) {
        assert!(
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "not bit-identical"
        );
    }
}

/// D-d: the key with a non-steady or coupled algorithm is a located error at `solver.cyclic_partner_in_h1`.
#[test]
fn dd_key_outside_the_steady_solver_is_a_located_error() {
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
