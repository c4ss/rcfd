//! Coupled pressure-velocity solver (F-032) through the settings tree.
//!
//! Checks fixed before running: the lid-driven cavity (Re = 100, 20 x 20, linearUpwind) with
//! `solver.algorithm = "coupled"` reaches the residual targets and gives the same velocity field as
//! SIMPLEC within 1e-3 of the lid speed (both solve the same discrete problem up to the
//! pressure-velocity coupling); unsupported combinations are diagnosed with their setting path.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn cavity(alg: &str, extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "cavity_2d"
nx = 20
ny = 20
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "{alg}"
convection = "linear_upwind"
max_iterations = 2000
tolerance_u = 1e-7
tolerance_p = 1e-7
{extra}
"#
    )
}

fn solve(text: &str) -> Simulation {
    let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
    while !sim.is_done() {
        sim.step();
    }
    sim
}

#[test]
fn coupled_cavity_matches_simplec() {
    let a = solve(&cavity("simplec", ""));
    let b = solve(&cavity("coupled", ""));
    assert!(a.is_converged() && b.is_converged());
    let mut worst: f64 = 0.0;
    for f in ["ux", "uy"] {
        let (x, y) = (a.field(f).unwrap(), b.field(f).unwrap());
        worst = x.iter().zip(&y).map(|(p, q)| (p - q).abs()).fold(worst, f64::max);
    }
    eprintln!(
        "simplec {} its, coupled {} its, max |du| = {worst:.2e}",
        a.iteration(),
        b.iteration()
    );
    assert!(worst < 1e-3, "{worst}");
}

#[test]
fn unsupported_combinations_are_diagnosed() {
    let open = "[boundaries.lid]\nvelocity = \"zero_gradient\"\n";
    let _ = open;
    for (extra, path) in [
        ("\n[energy]\nenabled = true\n", "energy.enabled"),
        (
            "\n[sources.s]\ntype = \"acceleration\"\nzone = \"all\"\nacceleration = [1.0, 0.0, 0.0]\n",
            "sources",
        ),
    ] {
        let text = cavity("coupled", "") + extra;
        let err = match Case::parse_toml(&text) {
            Err(d) => d,
            Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
        };
        assert!(err.iter().any(|d| d.path.starts_with(path)), "{path}: {err:?}");
    }
}
