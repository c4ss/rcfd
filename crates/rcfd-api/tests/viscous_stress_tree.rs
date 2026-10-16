//! ADR 0032 A8: `solver.viscous_stress` through the settings tree (transient Taylor-Green vortex).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};

const TG: &str = r#"
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
dt = 0.01
end_time = 1.0
[turbulence]
model = "smagorinsky"
"#;

fn case(edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(TG).unwrap();
    for (p, v) in edits {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    c
}

fn steps(c: &Case, n: usize) -> Simulation {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    s
}

fn diags(c: &Case) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = c.semantic_errors().into_iter().map(|d| (d.path, d.message)).collect();
    if out.is_empty() {
        if let Err(e) = Simulation::new(c) {
            out.extend(e.into_iter().map(|d| (d.path, d.message)));
        }
    }
    out
}

fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).fold(0.0, |m, (x, y)| f64::max(m, (x - y).abs()))
}

#[test]
fn a8_default_is_laplacian_and_full_changes_the_subgrid_run() {
    let c = case(&[]);
    assert_eq!(c.get("solver.viscous_stress").unwrap(), json!("laplacian"));
    let lap = steps(&case(&[]), 10).field("ux").unwrap();
    let explicit = steps(&case(&[("solver.viscous_stress", json!("laplacian"))]), 10)
        .field("ux")
        .unwrap();
    assert_eq!(lap, explicit);
    let full = steps(&case(&[("solver.viscous_stress", json!("full"))]), 10)
        .field("ux")
        .unwrap();
    let d = max_abs_diff(&lap, &full);
    println!("A8 Smagorinsky full against Laplacian after 10 steps: max |dux| {d:.3e} (limit > 1e-9)");
    assert!(d > 1e-9);
}

#[test]
fn a8_constant_viscosity_full_agrees_with_laplacian() {
    let lam = [("turbulence.model", json!("laminar"))];
    let lap = steps(&case(&lam), 10).field("ux").unwrap();
    let full = steps(&case(&[lam[0].clone(), ("solver.viscous_stress", json!("full"))]), 10)
        .field("ux")
        .unwrap();
    let d = max_abs_diff(&lap, &full);
    println!("A8 laminar full against Laplacian after 10 steps: max |dux| {d:.3e} (limit 1e-3)");
    assert!(d <= 1e-3);
}

#[test]
fn a8_located_errors() {
    let has = |d: &[(String, String)], path: &str, text: &str| d.iter().any(|e| e.0 == path && e.1.contains(text));
    let d = diags(&case(&[
        ("solver.algorithm", json!("simple")),
        ("turbulence.model", json!("laminar")),
        ("solver.viscous_stress", json!("full")),
    ]));
    assert!(has(&d, "solver.viscous_stress", "piso"), "{d:?}");
    let mut c = case(&[]);
    let e = c.set("solver.viscous_stress", json!("sym")).unwrap_err().to_string();
    assert!(e.contains("solver.viscous_stress"), "{e}");
}
