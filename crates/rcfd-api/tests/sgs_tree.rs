//! ADR 0026 T1 to T5: the subgrid-scale models through the settings tree (transient Taylor-Green vortex).

use rcfd_api::case::Case;
use rcfd_api::schema::case_schema;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_mesh::gen::box_2d;
use rcfd_turb::kepsilon::WallTreatment;
use rcfd_turb::{create_model, KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode};
use serde_json::{json, Value};
use std::f64::consts::PI;
use std::sync::Arc;

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

fn ctx(geom: &Arc<FvGeometry>) -> ModelContext {
    ModelContext {
        geom: geom.clone(),
        nu: 0.05,
        walls: Vec::new(),
        k0: 1.0,
        eps0: 1.0,
        omega0: 1.0,
        nutilde0: 0.15,
        variant: KEpsilonVariant::Standard,
        wall_treatment: WallTreatment::Smooth,
        sa_wall_mode: SaWallMode::Auto,
        sa_limiter: SLimiter::Allmaras,
        sa_negative: true,
        kappa: 0.41,
        e_wall: 9.8,
        convection: ConvScheme::Upwind,
        relax_turb: 0.7,
        relax_nut: 0.5,
        wall_damping: false,
        length_scale: 0.0,
        mixing_profile: MixingProfile::Linear,
        time_accurate: true,
    }
}

#[test]
fn t1_tree_nut_equals_library_function() {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(16, 16, 2.0 * PI, 2.0 * PI, true, true).unwrap())).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    for key in ["smagorinsky", "wale"] {
        let s = steps(&case(&[("turbulence.model", json!(key))]), 1);
        let st = s.status_json();
        assert_eq!(st["turbulence"]["key"], key, "{st}");
        let mut m = create_model(key, &ctx(&geom)).unwrap();
        m.update(&u, &[]);
        let d = max_abs_diff(&s.field("nut").unwrap(), m.nut());
        println!("T1 {key}: tree nut against the library, max abs difference {d:.3e}");
        assert!(d <= 1e-12);
        assert!(m.nut().iter().cloned().fold(0.0, f64::max) > 0.0);
    }
}

#[test]
fn t2_coefficients() {
    for (key, coef, default) in [("smagorinsky", "cs", 0.17), ("wale", "cw", 0.325)] {
        let base = steps(&case(&[("turbulence.model", json!(key))]), 1)
            .field("nut")
            .unwrap();
        let path = format!("turbulence.coefficients.{key}.{coef}");
        let scaled = steps(
            &case(&[("turbulence.model", json!(key)), (path.as_str(), json!(1.2 * default))]),
            1,
        )
        .field("nut")
        .unwrap();
        let top = base.iter().cloned().fold(0.0, f64::max);
        let dev = base
            .iter()
            .zip(&scaled)
            .fold(0.0_f64, |m, (b, s)| m.max((s - 1.44 * b).abs()))
            / top;
        println!("T2 {key}: {coef} x 1.2 scales nut by 1.44, max deviation {dev:.3e}");
        assert!(dev <= 1e-9);
    }
    // explicit defaults equal omitted keys bit for bit
    let a = steps(&case(&[]), 5).field("ux").unwrap();
    let b = steps(
        &case(&[
            ("turbulence.coefficients.smagorinsky.cs", json!(0.17)),
            ("turbulence.coefficients.smagorinsky.a_plus", json!(25.0)),
        ]),
        5,
    )
    .field("ux")
    .unwrap();
    assert_eq!(a, b);
    // cs = 0 equals the laminar run
    let z = steps(&case(&[("turbulence.coefficients.smagorinsky.cs", json!(0.0))]), 20);
    let lam = steps(&case(&[("turbulence.model", json!("laminar"))]), 20);
    for f in ["ux", "uy"] {
        let d = max_abs_diff(&z.field(f).unwrap(), &lam.field(f).unwrap());
        println!("T2 cs = 0 against laminar, {f}: {d:.3e}");
        assert!(d <= 1e-13);
    }
    // the model with the default constant differs from the laminar run
    let s = steps(&case(&[]), 20);
    assert!(max_abs_diff(&s.field("ux").unwrap(), &lam.field("ux").unwrap()) > 1e-6);
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

fn key_of(svc: &Arc<Service>, id: &str) -> String {
    svc.call("run.status", &json!({"session": id})).unwrap()["simulation"]["turbulence"]["key"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn t3_live_switch_smagorinsky_to_wale() {
    let svc = Service::new(ServiceConfig::default());
    let id = session(&svc, TG);
    svc.call("run.step", &json!({"session": id, "count": 10})).unwrap();
    assert_eq!(key_of(&svc, &id), "smagorinsky");
    let (ux, uy, p) = (field(&svc, &id, "ux"), field(&svc, &id, "uy"), field(&svc, &id, "p"));
    let nut_s = field(&svc, &id, "nut");
    svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.model", "value": "wale"}),
    )
    .unwrap();
    assert_eq!(field(&svc, &id, "ux"), ux);
    assert_eq!(field(&svc, &id, "uy"), uy);
    assert_eq!(field(&svc, &id, "p"), p);
    assert_eq!(key_of(&svc, &id), "wale");
    svc.call("run.step", &json!({"session": id, "count": 1})).unwrap();
    let nut_w = field(&svc, &id, "nut");
    let top = nut_s.iter().cloned().fold(0.0, f64::max);
    let d = max_abs_diff(&nut_s, &nut_w) / top;
    println!("T3 nut after the switch differs from the Smagorinsky one by {d:.3e} of its maximum");
    assert!(d > 1e-9);
    // a switch to a steady-only RANS model in a transient session is refused and leaves the run untouched
    let r = svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.model", "value": "k_epsilon"}),
    );
    assert!(r.is_err());
    assert_eq!(key_of(&svc, &id), "wale");
}

#[test]
fn t4_located_errors() {
    let has = |d: &[(String, String)], path: &str, text: &str| d.iter().any(|e| e.0 == path && e.1.contains(text));
    let d = diags(&case(&[("solver.algorithm", json!("simplec"))]));
    assert!(has(&d, "turbulence.model", "piso"), "{d:?}");
    // out-of-range values are refused when set, with the located path
    let mut c = case(&[]);
    let e = c
        .set("turbulence.coefficients.smagorinsky.cs", json!(2.0))
        .unwrap_err()
        .to_string();
    assert!(e.contains("turbulence.coefficients.smagorinsky.cs"), "{e}");
    // a coefficient of another model is a located error
    let d = diags(&case(&[("turbulence.coefficients.wale.cw", json!(0.4))]));
    assert!(has(&d, "turbulence.coefficients.wale.cw", ""), "{d:?}");
    let d = diags(&case(&[("turbulence.wall_damping", json!("van_driest"))]));
    assert!(has(&d, "turbulence.wall_damping", "wall patch"), "{d:?}");
    let d = diags(&case(&[
        ("turbulence.model", json!("wale")),
        ("turbulence.wall_damping", json!("van_driest")),
    ]));
    assert!(has(&d, "turbulence.wall_damping", "WALE"), "{d:?}");
    let d = diags(&case(&[("turbulence.model", json!("mixing_length"))]));
    assert!(has(&d, "turbulence.length_scale", "positive"), "{d:?}");
    let d = diags(&case(&[("turbulence.length_scale", json!(1.0))]));
    assert!(has(&d, "turbulence.length_scale", "mixing_length"), "{d:?}");
    let d = diags(&case(&[("turbulence.model", json!("k_epsilon"))]));
    assert!(has(&d, "turbulence.model", "steady"), "{d:?}");
    let d = diags(&case(&[("energy.enabled", json!(true))]));
    assert!(d.iter().any(|e| e.1.to_lowercase().contains("energy")), "{d:?}");
    // the valid case has no diagnostics
    assert!(diags(&case(&[])).is_empty());
}

#[test]
fn t5_schema_lists_the_models_and_coefficients() {
    let js = case_schema().to_json_schema();
    let t = &js["properties"]["turbulence"]["properties"];
    let models: Vec<&str> = t["model"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for m in ["smagorinsky", "wale", "mixing_length"] {
        assert!(models.contains(&m), "{m} missing from {models:?}");
    }
    let props = &t["coefficients"]["properties"];
    for (group, n) in [("smagorinsky", 2), ("wale", 1), ("mixing_length", 3)] {
        let g = props[group]["properties"].as_object().unwrap();
        assert_eq!(g.len(), n, "{group}");
        for (name, v) in g {
            assert!(
                !v["description"].as_str().unwrap_or("").is_empty(),
                "{group}.{name} has no doc"
            );
            assert!(
                v["minimum"].is_number() && v["maximum"].is_number(),
                "{group}.{name} has no range"
            );
        }
    }
    let keys: Vec<String> = rcfd_turb::model_keys().into_iter().map(|k| k.0).collect();
    for m in ["smagorinsky", "wale", "mixing_length"] {
        assert!(keys.iter().any(|k| k == m), "{m} not registered");
    }
}
