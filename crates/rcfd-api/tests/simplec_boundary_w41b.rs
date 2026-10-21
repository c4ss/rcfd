//! SIMPLEC boundary compensation (F-248, ADR 0102). With `solver.simplec_boundary_compensation = true` every face whose flux the
//! pressure sets carries `rAU` at a fixed point, so a converged SIMPLEC run equals a converged SIMPLE run with the same `alpha_u`
//! (the Rhie-Chow fixed point depends on the relaxed diagonal only). B1: a walled duct with a fixed-pressure outlet; B2: the same
//! duct periodic in y with an off-centre porous block, so the seam faces carry a pressure difference. Limits in the ADR.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

/// Duct 2 m x 0.5 m, 40 x 10 cells, inlet 1 m/s at xmin, fixed pressure 0 at xmax; `periodic` makes y periodic (else no-slip
/// walls). A porous block (Darcy, permeability 1e-3) at x in [0.8, 1.2], y in [0.1, 0.3].
fn duct(periodic: bool, alg: &str, extra: &str) -> String {
    let walls = if periodic { "periodic_y = true\n" } else { "" };
    let wall_bc = if periodic {
        ""
    } else {
        "[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\n"
    };
    let alpha_p = if alg == "simple" { 0.3 } else { 1.0 };
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 40
ny = 10
lx = 2.0
ly = 0.5
{walls}
[physics]
nu = 0.02
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
{wall_bc}
[sources.block]
type = "porous"
model = "darcy_forchheimer"
permeability = 1e-3
inertial_resistance = 0.0
zone = "box"
box_min = [0.8, 0.1, -1.0]
box_max = [1.2, 0.3, 1.0]
[solver]
algorithm = "{alg}"
convection = "upwind"
alpha_u = 0.7
alpha_p = {alpha_p}
momentum_rel_tol = 1e-12
pressure_rel_tol = 1e-12
pressure_max_iter = 2000
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
{extra}
"#
    )
}

/// (ux, uy, p, iterations, settled)
type Run = (Vec<f64>, Vec<f64>, Vec<f64>, usize, bool);

/// Iterate in blocks of 100 until the largest change of ux, uy and p over a block, relative to the field's largest magnitude,
/// is below 1e-13 (cap 40000 iterations). Returns (ux, uy, p, iterations, converged).
fn converge(text: &str) -> Run {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let get = |sim: &Simulation| -> Vec<Vec<f64>> { ["ux", "uy", "p"].iter().map(|f| sim.field(f).unwrap()).collect() };
    let mut old = get(&sim);
    let mut it = 0;
    let mut done = false;
    while it < 40000 {
        for _ in 0..100 {
            sim.step();
        }
        it += 100;
        let new = get(&sim);
        assert!(
            new.iter().flatten().all(|v| v.is_finite()),
            "non-finite after {it} iterations"
        );
        let ch = new
            .iter()
            .zip(&old)
            .map(|(a, b)| {
                let s = a.iter().fold(1e-300_f64, |m, v| m.max(v.abs()));
                a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / s
            })
            .fold(0.0, f64::max);
        old = new;
        if ch < 1e-13 {
            done = true;
            break;
        }
    }
    let mut v = old.into_iter();
    let (ux, uy, p) = (v.next().unwrap(), v.next().unwrap(), v.next().unwrap());
    (ux, uy, p, it, done)
}

/// Largest difference of ux and uy relative to max |ux|, and of p relative to the pressure range.
fn diff(a: &Run, b: &Run) -> (f64, f64) {
    let umax = a.0.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let du =
        a.0.iter()
            .zip(&b.0)
            .chain(a.1.iter().zip(&b.1))
            .fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()))
            / umax;
    let (lo, hi) =
        a.2.iter()
            .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let dp = a.2.iter().zip(&b.2).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / (hi - lo);
    (du, dp)
}

fn check(periodic: bool, label: &str) {
    let simple = converge(&duct(periodic, "simple", ""));
    let plain = converge(&duct(periodic, "simplec", ""));
    let comp = converge(&duct(periodic, "simplec", "simplec_boundary_compensation = true\n"));
    let (du0, dp0) = diff(&simple, &plain);
    let (du1, dp1) = diff(&simple, &comp);
    eprintln!(
        "{label}: iterations simple {} ({}), simplec {} ({}), simplec + compensation {} ({}); default simplec vs simple: u {du0:.3e}, p {dp0:.3e} (must exceed 1e-6); with compensation: u {du1:.3e}, p {dp1:.3e} (limit 1e-9)",
        simple.3, simple.4, plain.3, plain.4, comp.3, comp.4
    );
    assert!(simple.4 && plain.4 && comp.4, "{label}: a run did not settle");
    assert!(
        du0.max(dp0) > 1e-6,
        "{label}: the default difference {du0:e} / {dp0:e} does not show the defect"
    );
    assert!(
        du1 <= 1e-9 && dp1 <= 1e-9,
        "{label}: compensated simplec differs from simple: u {du1:e}, p {dp1:e}"
    );
}

#[test]
fn b1_fixed_pressure_outlet_simplec_fixed_point_equals_simple() {
    check(false, "B1 walled duct");
}

#[test]
fn b2_periodic_seam_simplec_fixed_point_equals_simple() {
    check(true, "B2 periodic duct");
}

/// A4: the key with a transient algorithm is a located error.
#[test]
fn a4_key_with_a_transient_algorithm_is_a_located_error() {
    let text = duct(
        false,
        "piso",
        "simplec_boundary_compensation = true\ndt = 0.01\nend_time = 0.1\n",
    );
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let err = Simulation::new(&case).err().expect("the key was accepted with piso");
    assert!(
        err.iter().any(|d| d.path == "solver.simplec_boundary_compensation"),
        "{err:?}"
    );
}
