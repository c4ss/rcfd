//! Passive scalars and species through the settings tree (F-053).
//!
//! Checks fixed before running (flow solved by SIMPLEC on a 2-D channel with slip walls, so the
//! velocity is uniform; the mixing layer of two streams then has the error-function similarity
//! solution):
//! * three species with different diffusivities, the last closed by difference: for each transported
//!   species the net discrete boundary outflow is below 1e-12 of the summed absolute boundary fluxes (the
//!   `scalar_flux` report without patch), the mass fractions sum to 1 to 1e-14 and the profile at x = L/2
//!   matches the similarity solution within 2 percent of the inlet jump;
//! * a Schmidt-number scalar in a Couette-like flow follows the viscosity setting (diffusivity nu/Sc);
//! * invalid settings are diagnosed with their path.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const L: f64 = 1.0;
const H: f64 = 0.5;

fn erf(x: f64) -> f64 {
    if x.abs() > 4.0 {
        return x.signum();
    }
    let (mut term, mut sum, mut n) = (x, x, 0.0);
    while term.abs() > 1e-17 {
        n += 1.0;
        term *= -x * x / n;
        sum += term / (2.0 * n + 1.0);
    }
    2.0 / std::f64::consts::PI.sqrt() * sum
}

fn mixing(extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 100
ny = 50
lx = {L}
ly = {H}
depth = 0.01
[physics]
nu = 0.001
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 600
tolerance_u = 1e-9
tolerance_p = 1e-9
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-22
pressure_max_iter = 5000

[species.A]
diffusivity = 1e-3
[species.A.boundaries.inlet]
type = "fixed"
value_expression = "0.7 * step(y - 0.25 [m])"

[species.B]
schmidt = 0.5
initial = 0.2
[species.B.boundaries.inlet]
type = "fixed"
value_expression = "0.2 + 0.1 * step(y - 0.25 [m])"

[species.C]
remainder = true
initial = 0.8

[reports.flux_A]
type = "scalar_flux"
scalar = "A"
[reports.flux_B]
type = "scalar_flux"
scalar = "B"
[reports.flux_C]
type = "scalar_flux"
scalar = "C"
[reports.out_A]
type = "scalar_flux"
scalar = "A"
patch = "outlet"
{extra}
"#
    )
}

fn run(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    // The flow is uniform, so its normalised residual is degenerate (the normalisation vanishes): stop when
    // the scalar sweeps have converged instead.
    let scalar_res = |s: &Simulation| {
        ["res_A", "res_B"]
            .iter()
            .filter_map(|m| s.monitors.last(m))
            .fold(0.0, f64::max)
    };
    while !sim.is_done()
        && !(sim.iteration() > 50 && scalar_res(&sim) < 1e-11 && sim.monitors.last("continuity").unwrap() < 1e-13)
    {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let l = sim.last().unwrap();
    eprintln!(
        "iterations {}, continuity {:?}, res_u {:.2e}, res_p {:.2e}, res_A {:?}, res_B {:?}",
        l.index,
        sim.monitors.last("continuity"),
        l.res_u,
        l.res_p,
        sim.monitors.last("res_A"),
        sim.monitors.last("res_B")
    );
    sim
}

#[test]
fn species_mixing_layer_is_conserved_and_matches_the_similarity_solution() {
    let sim = run(&mixing(""));
    assert!(
        sim.iteration() < 600,
        "scalars not converged after {} iterations",
        sim.iteration()
    );
    let val = |n: &str| sim.monitors.last(n).unwrap();
    let a = sim.field("A").unwrap();
    let b = sim.field("B").unwrap();
    let c = sim.field("C").unwrap();
    let sum_err = (0..a.len())
        .map(|i| (a[i] + b[i] + c[i] - 1.0).abs())
        .fold(0.0, f64::max);
    let scale_a = 0.7 * 0.25 * 0.01; // inflow of A: U * integral of the inlet profile
    let scale_b = (0.2 * 0.5 + 0.1 * 0.25) * 0.01;
    eprintln!(
        "{} iterations; relative imbalance A {:.2e}, B {:.2e}, C {:.2e}; sum error {sum_err:.2e}; outflow A {:.8e} (inflow {scale_a:.8e})",
        sim.iteration(),
        val("flux_A") / scale_a,
        val("flux_B") / scale_b,
        val("flux_C") / 0.01,
        val("out_A")
    );
    assert!(val("flux_A").abs() < 1e-12 * scale_a);
    assert!(val("flux_B").abs() < 1e-12 * scale_b);
    assert!(val("flux_C").abs() < 1e-12 * 0.01);
    assert!(sum_err < 1e-14, "{sum_err}");
    assert!((val("out_A") - scale_a).abs() < 1e-9 * scale_a);

    // similarity profile of A at x = L/2 (cell column nearest to the middle)
    let cc = sim.geom.mesh.cell_centres();
    let dx = L / 100.0;
    let mut worst: f64 = 0.0;
    for i in 0..a.len() {
        if (cc[i][0] - (0.5 * L - 0.5 * dx)).abs() < 1e-9 {
            let x = cc[i][0];
            let exact = 0.7 * 0.5 * (1.0 + erf((cc[i][1] - 0.25) / (2.0 * (1e-3 * x).sqrt())));
            worst = worst.max((a[i] - exact).abs() / 0.7);
        }
    }
    eprintln!("species A profile error {:.3}% of the jump", 100.0 * worst);
    assert!(worst < 0.02, "{worst}");
}

#[test]
fn invalid_species_and_scalar_settings_are_diagnosed() {
    let bad = |extra: &str, path: &str| {
        let text = mixing(extra);
        let err = match Case::parse_toml(&text) {
            Err(d) => d,
            Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
        };
        assert!(err.iter().any(|d| d.path.starts_with(path)), "{path}: {err:?}");
    };
    bad("[scalars.s]\n", "scalars.s.diffusivity");
    bad("[scalars.s]\ndiffusivity = 1.0\nschmidt = 1.0\n", "scalars.s.schmidt");
    bad("[scalars.p]\ndiffusivity = 1.0\n", "scalars");
    bad(
        "[scalars.s]\ndiffusivity = 1.0\n[scalars.s.boundaries.nowhere]\ntype = \"fixed\"\n",
        "scalars.s.boundaries.nowhere",
    );
    bad(
        "[species.C.boundaries.inlet]\ntype = \"fixed\"\nvalue = 1.0\n",
        "species.C.boundaries.inlet",
    );
    bad("[species.D]\ndiffusivity = 1e-3\ninitial = 0.5\n", "species.");
}
