//! ADR 0032 B7: SA-based DES / DDES through the settings tree (transient periodic channel).

use rcfd_api::case::Case;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::sync::Arc;

const CH: &str = r#"
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
dt = 0.02
end_time = 1.0
[turbulence]
model = "spalart_allmaras"
[turbulence.sa]
wall_mode = "resolved"
des = "ddes"
"#;

fn case(edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(CH).unwrap();
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
    a.iter().zip(b).fold(0.0, |m, (x, y)| f64::max(m, (x - y).abs()))
}

#[test]
fn b7_tree_runs_des_and_ddes() {
    for mode in ["des", "ddes"] {
        let s = steps(&case(&[("turbulence.sa.des", json!(mode))]), 10);
        let nut = s.field("nut").unwrap();
        let ux = s.field("ux").unwrap();
        println!(
            "B7 {mode}: max nut {:.3e}, max ux {:.4}",
            nut.iter().cloned().fold(0.0, f64::max),
            ux.iter().cloned().fold(0.0, f64::max)
        );
        assert!(nut.iter().all(|v| v.is_finite() && *v >= 0.0));
        assert!(ux.iter().all(|v| v.is_finite()));
    }
}

#[test]
fn b7_options_change_the_result() {
    let base = steps(&case(&[("turbulence.sa.des", json!("des"))]), 20)
        .field("nut")
        .unwrap();
    let other = steps(
        &case(&[
            ("turbulence.sa.des", json!("des")),
            ("turbulence.sa.des_delta", json!("cbrt_volume")),
        ]),
        20,
    )
    .field("nut")
    .unwrap();
    let c = steps(
        &case(&[("turbulence.sa.des", json!("des")), ("turbulence.sa.c_des", json!(0.3))]),
        20,
    )
    .field("nut")
    .unwrap();
    let (d1, d2) = (max_abs_diff(&base, &other), max_abs_diff(&base, &c));
    println!("B7 teeth: des_delta changes nut by {d1:.3e}, c_des 0.3 by {d2:.3e} (limit > 1e-9)");
    assert!(d1 > 1e-9 && d2 > 1e-9);
}

#[test]
fn b7_located_errors() {
    let has = |d: &[(String, String)], path: &str, text: &str| d.iter().any(|e| e.0 == path && e.1.contains(text));
    // steady algorithm with des
    let d = diags(&case(&[("solver.algorithm", json!("simplec"))]));
    assert!(has(&d, "turbulence.sa.des", "piso"), "{d:?}");
    // transient SA without des stays refused
    let d = diags(&case(&[("turbulence.sa.des", json!("off"))]));
    assert!(has(&d, "turbulence.model", "steady"), "{d:?}");
    // sa.des options with another model
    let d = diags(&case(&[("turbulence.model", json!("smagorinsky"))]));
    assert!(has(&d, "turbulence.sa.des", "spalart_allmaras"), "{d:?}");
    // c_des range
    let mut c = case(&[]);
    let e = c.set("turbulence.sa.c_des", json!(5.0)).unwrap_err().to_string();
    assert!(e.contains("turbulence.sa.c_des"), "{e}");
    let e = c.set("turbulence.sa.des", json!("iddes")).unwrap_err().to_string();
    assert!(e.contains("turbulence.sa.des"), "{e}");
}

fn session(svc: &Arc<Service>, toml: &str) -> String {
    svc.call("session.create", &json!({"toml": toml})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string()
}

fn field(svc: &Arc<Service>, id: &str, name: &str) -> Vec<f64> {
    svc.call(
        "field.slice",
        &json!({"session": id, "name": name, "max_points": 1000000}),
    )
    .unwrap()["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

/// Registered B7 asked for a live switch to smagorinsky. A model switch is refused while `turbulence.sa.*` holds non-default values
/// (the existing rule for the SA options; there is no multi-path set), so the test checks that refusal and the live switch
/// ddes -> des instead (deviation disclosed in ADR 0032 Results).
#[test]
fn b7_live_switch_between_des_modes_keeps_the_flow() {
    let svc = Service::new(ServiceConfig::default());
    let id = session(&svc, CH);
    svc.call("run.step", &json!({"session": id, "count": 10})).unwrap();
    let (ux, uy, p) = (field(&svc, &id, "ux"), field(&svc, &id, "uy"), field(&svc, &id, "p"));
    let nut_a = field(&svc, &id, "nut");
    let r = svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.model", "value": "smagorinsky"}),
    );
    assert!(r.is_err());
    assert_eq!(field(&svc, &id, "ux"), ux);
    svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.sa.des", "value": "des"}),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(field(&svc, &id, "ux"), ux);
    assert_eq!(field(&svc, &id, "uy"), uy);
    assert_eq!(field(&svc, &id, "p"), p);
    svc.call("run.step", &json!({"session": id, "count": 1})).unwrap();
    let nut_b = field(&svc, &id, "nut");
    let d = max_abs_diff(&nut_a, &nut_b);
    println!("B7 nut after the ddes -> des switch and one step differs by {d:.3e}");
    assert!(d > 1e-9);
}
