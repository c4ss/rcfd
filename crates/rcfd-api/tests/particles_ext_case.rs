//! Lagrangian extensions through the settings tree (F-128, ADR 0057 checks L14 and L15). Limits as registered in the ADR.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::particles;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::norm;

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => c.semantic_errors(),
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

/// Frozen Couette flow u = (20 y, 0, 0) on a 12 x 10 box_2d [0,4] x [0,1]; Stokes particle with tau_p = 0.05 s released at
/// rest at (0.5, 0.37).
fn couette_text(extra: &str) -> String {
    format!(
        r#"
title = "particle couette"
[mesh]
generator = "box_2d"
nx = 12
ny = 10
lx = 4.0
ly = 1.0
[physics]
nu = 1e-3
rho = 1.0
[energy]
enabled = true
[initial]
temperature = 300.0
velocity_expression_x = "20 [1/s] * y"
[boundaries.xmin]
velocity = "fixed"
velocity_value = [10.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "fixed"
velocity_value = [10.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [20.0, 0.0, 0.0]
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "stokes"
integration = "analytic"
interpolation = "gradient"
gravity = false
end_time = 0.4
dt = 0.01
[particles.injection.p]
kind = "point"
position = [0.5, 0.37, 0.05]
rate = 1000.0
end_time = 0.001
diameter = 9.486832980505138e-4
{extra}
"#
    )
}

const CONE: &str = "diameter = 1e-4\nmass_flow = 0.02\ncone_half_angle = 20.0\ncone_inner_half_angle = 5.0\ncone_axis = [1.0, 0.0, 0.0]\ncone_speed = 3.0\n";

fn with_cone(extra: &str) -> String {
    couette_text("").replace("diameter = 9.486832980505138e-4", &format!("{CONE}{extra}"))
}

#[test]
#[allow(clippy::vec_init_then_push)]
fn l14_located_errors_for_invalid_input() {
    let mut cases: Vec<(String, &str)> = Vec::new();
    cases.push((
        with_cone("").replace("cone_half_angle = 20.0", "cone_half_angle = 200.0"),
        "particles.injection.p.cone_half_angle",
    ));
    cases.push((
        with_cone("").replace("cone_half_angle = 20.0", "cone_half_angle = 0.0"),
        "particles.injection.p.cone_half_angle",
    ));
    cases.push((
        with_cone("").replace("cone_inner_half_angle = 5.0", "cone_inner_half_angle = 25.0"),
        "particles.injection.p.cone_inner_half_angle",
    ));
    cases.push((
        with_cone("").replace("cone_axis = [1.0, 0.0, 0.0]", "cone_axis = [0.0, 0.0, 0.0]"),
        "particles.injection.p.cone_axis",
    ));
    cases.push((
        with_cone("").replace("cone_axis = [1.0, 0.0, 0.0]", "cone_axis = [1.0, 0.0]"),
        "particles.injection.p.cone_axis",
    ));
    cases.push((
        with_cone("").replace("cone_speed = 3.0", "cone_speed = -1.0"),
        "particles.injection.p.cone_speed",
    ));
    cases.push((
        with_cone("").replace("mass_flow = 0.02", "mass_flow = 0.0"),
        "particles.injection.p.mass_flow",
    ));
    let uni = couette_text("").replace(
        "diameter = 9.486832980505138e-4",
        "distribution = \"uniform\"\nd_min = 2e-4\nd_max = 1e-4",
    );
    cases.push((uni.clone(), "particles.injection.p.d_max"));
    cases.push((uni.replace("d_min = 2e-4\n", ""), "particles.injection.p.d_min"));
    let drw = couette_text("").replace("[particles]\n", "[particles]\ndispersion = \"drw\"\n");
    cases.push((drw.clone(), "particles.dispersion_k"));
    cases.push((
        drw.replace("dispersion = \"drw\"\n", "dispersion = \"drw\"\ndispersion_k = 1.0\n"),
        "particles.dispersion_epsilon",
    ));
    cases.push((
        drw.replace(
            "dispersion = \"drw\"\n",
            "dispersion = \"drw\"\ndispersion_k = 1.0\ndispersion_epsilon = 2.0\ndispersion_cl = 0.0\n",
        ),
        "particles.dispersion_cl",
    ));
    for (text, path) in &cases {
        let errs = errors(text);
        assert!(has(&errs, path), "expected an error at {path}, got {errs:?}");
    }
    // valid input: no diagnostics
    assert!(errors(&with_cone("")).is_empty(), "{:?}", errors(&with_cone("")));
    let ok = drw.replace(
        "dispersion = \"drw\"\n",
        "dispersion = \"drw\"\ndispersion_k = 1.0\ndispersion_epsilon = 2.0\n",
    );
    assert!(errors(&ok).is_empty());
    // a disabled section does not report
    assert!(errors(&drw.replace("enabled = true\ndensity", "enabled = false\ndensity")).is_empty());
}

fn parcels_text(extra_particles: &str) -> String {
    couette_text("")
        .replace("[particles]\n", &format!("[particles]\n{extra_particles}"))
        .replace("rate = 1000.0", "rate = 200000.0")
}

fn final_state(text: &str) -> rcfd_particles::Tracker {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let sim = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    particles::track_frozen(&c, &sim).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn l15_dispersion_through_the_tree() {
    let none = final_state(&parcels_text(""));
    let none2 = final_state(&parcels_text("dispersion = \"none\"\n"));
    assert_eq!(none.particles.len(), 200);
    assert_eq!(
        none.fingerprint(),
        none2.fingerprint(),
        "explicit none equals the default"
    );
    let spread = |t: &rcfd_particles::Tracker| {
        let n = t.particles.len() as f64;
        let m = t.particles.iter().map(|p| p.x[1]).sum::<f64>() / n;
        (t.particles.iter().map(|p| (p.x[1] - m).powi(2)).sum::<f64>() / n).sqrt()
    };
    let drw = "dispersion = \"drw\"\ndispersion_k = 1.0\ndispersion_epsilon = 10.0\n";
    let a = final_state(&parcels_text(drw));
    let b = final_state(&parcels_text(drw));
    let (s0, s1) = (spread(&none), spread(&a));
    println!("L15 transverse position std: none {s0:e}, drw {s1:e}");
    assert!(s0 <= 1e-12, "without dispersion all parcels follow one path");
    assert!(s1 > 1e-3);
    assert_eq!(a.fingerprint(), b.fingerprint(), "reproducible bit for bit");
    let c = final_state(&parcels_text(
        &drw.replace("epsilon = 10.0", "epsilon = 10.0\ndispersion_seed = 7"),
    ));
    assert_ne!(a.fingerprint(), c.fingerprint());
}

#[test]
fn cone_and_mass_flow_through_the_tree() {
    // drag off, reflecting walls: speed is conserved; 200 parcels, mass flow 0.02 kg/s at 200000 parcels/s
    let text = with_cone("")
        .replace("drag = \"stokes\"", "drag = \"none\"")
        .replace("rate = 1000.0", "rate = 200000.0");
    let tr = final_state(&text);
    assert_eq!(tr.particles.len(), 200);
    let mut worst: f64 = 0.0;
    let mut total = 0.0;
    for p in &tr.particles {
        worst = worst.max((norm(p.v) - 3.0).abs() / 3.0);
        total += p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0;
    }
    let expect = 200.0 * 0.02 / 200000.0;
    let e = (total - expect).abs() / expect;
    println!("cone/mass flow: speed error {worst:e}, total mass relative error {e:e}");
    assert!(worst <= 1e-12);
    assert!(e <= 1e-12);
}
