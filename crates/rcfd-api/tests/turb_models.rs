//! F-041 / F-044 (ADR 0014): Spalart-Allmaras through the settings tree, coefficient overrides (K1-K4), live model
//! switching through the service (R1-R3) and a registered plugin model (R4). Limits are those of the ADR, written before
//! this file was run.

use rcfd_api::case::Case;
use rcfd_api::schema::case_schema;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_fv::ScalarField;
use rcfd_turb::{CoefficientInfo, ModelContext, TurbulenceModel};
use serde_json::{json, Value};
use std::sync::Arc;

/// Developing flat plate, Re = 5e6 per m, 30 x 30 cells with wall layers (coarse; for the coefficient tests).
const PLATE: &str = r#"
title = "SA flat plate (coarse)"
[mesh]
generator = "box_2d"
nx = 30
ny = 30
lx = 2.0
ly = 1.0
wall_layers = 18
layer_walls = "lower"
layer_growth = 1.2
target_y_plus = 1.0
layer_reference_velocity = 1.0
layer_reference_length = 2.0
[physics]
nu = 2e-7
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
convection = "upwind"
alpha_u = 0.8
max_iterations = 100000
tolerance_u = 1e-14
tolerance_p = 1e-14
[turbulence]
model = "spalart_allmaras"
[reports.yplus]
type = "y_plus"
patch = "ymin"
statistic = "average"
"#;

/// Periodic channel Re_tau = 395, 4 x 14 cells (y+ 28), like cases/channel/kepsilon_channel.toml without the initial
/// k/epsilon values and without a convergence criterion.
const CHANNEL: &str = r#"
title = "channel Re_tau 395"
[mesh]
generator = "box_2d"
nx = 4
ny = 14
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.002531645569620253
body_force = [1.0, 0.0, 0.0]
[initial]
velocity = [17.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
alpha_u = 0.9
max_iterations = 1000000
tolerance_u = 1e-14
tolerance_p = 1e-14
[turbulence]
model = "k_epsilon"
variant = "standard"
"#;

fn case(base: &str, edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(base).unwrap();
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

fn max(v: &[f64]) -> f64 {
    v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
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

#[test]
fn sa_runs_through_the_settings_tree() {
    let c = case(PLATE, &[]);
    let s = steps(&c, 60);
    let nut = s.field("nut").unwrap();
    let nt = s.field("nuTilda").unwrap();
    assert!(nut.iter().chain(&nt).all(|x| x.is_finite()));
    assert!(max(&nut) > 0.0);
    assert!(s.field("y_plus").unwrap().iter().any(|&y| y > 0.0));
    let st = s.status_json();
    assert_eq!(st["turbulence"]["model"], "spalart_allmaras");
    assert_eq!(st["turbulence"]["key"], "spalart_allmaras");
    // k and epsilon do not exist for SA
    assert!(s.field("k").is_err());
}

/// K1
#[test]
fn k1_sa_coefficient_override() {
    let base = steps(&case(PLATE, &[]), 300);
    let explicit = steps(&case(PLATE, &[("turbulence.coefficients.sa.cb1", json!(0.1355))]), 300);
    let raised = steps(&case(PLATE, &[("turbulence.coefficients.sa.cb1", json!(0.1626))]), 300);
    let cb1 = |s: &Simulation| {
        s.turbulence_coefficients()
            .into_iter()
            .find(|c| c.0 == "cb1")
            .unwrap()
            .1
    };
    assert_eq!(cb1(&base), 0.1355);
    assert_eq!(cb1(&raised), 0.1626);
    let (nb, ne, nr) = (
        base.field("nut").unwrap(),
        explicit.field("nut").unwrap(),
        raised.field("nut").unwrap(),
    );
    let diff = nb.iter().zip(&ne).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    assert_eq!(diff, 0.0, "explicit default must reproduce the default run bit for bit");
    let rel = (max(&nr) - max(&nb)).abs() / max(&nb);
    println!(
        "K1: max nut default {:.6e}, cb1 +20 percent {:.6e}, relative change {rel:.4}",
        max(&nb),
        max(&nr)
    );
    assert!(rel > 0.01, "relative change of max nut {rel}");
}

/// K2
#[test]
fn k2_kepsilon_coefficient_override() {
    let base = steps(&case(CHANNEL, &[]), 100);
    let explicit = steps(
        &case(CHANNEL, &[("turbulence.coefficients.k_epsilon.c2", json!(1.92))]),
        100,
    );
    let raised = steps(
        &case(CHANNEL, &[("turbulence.coefficients.k_epsilon.c2", json!(2.112))]),
        100,
    );
    let (nb, ne, nr) = (
        base.field("nut").unwrap(),
        explicit.field("nut").unwrap(),
        raised.field("nut").unwrap(),
    );
    assert_eq!(nb.iter().zip(&ne).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max), 0.0);
    let rel = (max(&nr) - max(&nb)).abs() / max(&nb);
    println!(
        "K2: max nut default {:.6e}, c2 +10 percent {:.6e}, relative change {rel:.5}",
        max(&nb),
        max(&nr)
    );
    assert!(rel > 1e-3, "relative change of max nut {rel}");
    let c2 = raised
        .turbulence_coefficients()
        .into_iter()
        .find(|c| c.0 == "c2")
        .unwrap()
        .1;
    assert_eq!(c2, 2.112);
}

/// K3
#[test]
fn k3_sst_coefficients_are_visible_but_not_overridable() {
    let sst = |extra: &[(&str, Value)]| {
        let mut e = vec![("turbulence.model", json!("k_omega_sst"))];
        e.extend_from_slice(extra);
        case(CHANNEL, &e)
    };
    assert!(diags(&sst(&[("turbulence.coefficients.k_omega_sst.a1", json!(0.31))])).is_empty());
    let d = diags(&sst(&[("turbulence.coefficients.k_omega_sst.a1", json!(0.4))]));
    assert!(
        d.iter().any(|e| e.0 == "turbulence.coefficients.k_omega_sst.a1"),
        "{d:?}"
    );
}

/// K4
#[test]
fn k4_coefficients_are_visible_with_docs_and_located_errors() {
    let c = Case::new();
    let coef = c.get("turbulence.coefficients").unwrap();
    for (group, n) in [("k_epsilon", 5), ("k_omega_sst", 11), ("sa", 10)] {
        assert_eq!(coef[group].as_object().unwrap().len(), n, "{group}");
    }
    // docs and ranges in the exported JSON schema
    let js = case_schema().to_json_schema();
    let props = &js["properties"]["turbulence"]["properties"]["coefficients"]["properties"];
    for (group, n) in [("k_epsilon", 5), ("k_omega_sst", 11), ("sa", 10)] {
        let g = props[group]["properties"].as_object().unwrap();
        assert_eq!(g.len(), n, "{group}");
        for (name, v) in g {
            assert!(
                !v["description"].as_str().unwrap_or("").is_empty(),
                "{group}.{name} has no doc"
            );
            if group != "k_epsilon" {
                assert!(
                    v["minimum"].is_number() && v["maximum"].is_number(),
                    "{group}.{name} has no range"
                );
            }
        }
    }
    // out of range: rejected by set with the path in the message
    let mut c = Case::new();
    let e = c
        .set("turbulence.coefficients.sa.cb1", json!(-1.0))
        .unwrap_err()
        .to_string();
    assert!(e.contains("turbulence.coefficients.sa.cb1"), "{e}");
    // an SA coefficient on a k-epsilon case
    let d = diags(&case(CHANNEL, &[("turbulence.coefficients.sa.cb1", json!(0.2))]));
    assert!(d.iter().any(|e| e.0 == "turbulence.coefficients.sa.cb1"), "{d:?}");
    // c1 with realizable
    let d = diags(&case(
        CHANNEL,
        &[
            ("turbulence.variant", json!("realizable")),
            ("turbulence.coefficients.k_epsilon.c1", json!(1.5)),
        ],
    ));
    assert!(d.iter().any(|e| e.0 == "turbulence.coefficients.k_epsilon.c1"), "{d:?}");
}

// ------------------------------------------------------------------ live switching

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

fn run_steps(svc: &Arc<Service>, id: &str, n: u64) {
    let mut left = n;
    while left > 0 {
        let c = left.min(1000);
        svc.call("run.step", &json!({"session": id, "count": c})).unwrap();
        left -= c;
    }
}

fn iteration(svc: &Arc<Service>, id: &str) -> u64 {
    svc.call("run.status", &json!({"session": id})).unwrap()["simulation"]["iteration"]
        .as_u64()
        .unwrap()
}

fn model_of(svc: &Arc<Service>, id: &str) -> String {
    svc.call("run.status", &json!({"session": id})).unwrap()["simulation"]["turbulence"]["key"]
        .as_str()
        .unwrap()
        .to_string()
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn switch(svc: &Arc<Service>, id: &str, model: &str) {
    let ux0 = field(svc, id, "ux");
    let uy0 = field(svc, id, "uy");
    let p0 = field(svc, id, "p");
    let it0 = iteration(svc, id);
    svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.model", "value": model}),
    )
    .unwrap();
    // R1: nothing of the flow was reset or rebuilt
    assert_eq!(field(svc, id, "ux"), ux0, "ux changed at the switch to {model}");
    assert_eq!(field(svc, id, "uy"), uy0);
    assert_eq!(field(svc, id, "p"), p0);
    assert_eq!(
        iteration(svc, id),
        it0,
        "iteration counter reset at the switch to {model}"
    );
    assert_eq!(model_of(svc, id), model);
}

fn fresh_bulk(model: &str, n: u64) -> f64 {
    let svc = Service::new(ServiceConfig::default());
    let c = case(CHANNEL, &[("turbulence.model", json!(model))]);
    let id = session(&svc, &c.to_toml());
    run_steps(&svc, &id, n);
    mean(&field(&svc, &id, "ux"))
}

/// R1 and R2 (heavy in debug: 24000 + 24000 iterations of a 56-cell case)
#[test]
#[ignore = "heavy: 48000 iterations in total, run in release"]
fn r2_switching_models_through_the_settings_tree() {
    let n = 6000;
    let svc = Service::new(ServiceConfig::default());
    let id = session(&svc, CHANNEL);
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    let mut out = Vec::new();
    for (i, model) in ["k_epsilon", "k_omega_sst", "spalart_allmaras", "k_epsilon"]
        .iter()
        .enumerate()
    {
        if i > 0 {
            switch(&svc, &id, model);
        }
        run_steps(&svc, &id, n);
        let bulk = mean(&field(&svc, &id, "ux"));
        let fresh = fresh_bulk(model, n);
        let rel = (bulk / fresh - 1.0).abs();
        println!("R2 phase {i} {model}: bulk {bulk:.5}, fresh {fresh:.5}, relative difference {rel:.2e}");
        out.push((model.to_string(), rel));
    }
    for (m, rel) in out {
        assert!(rel < 0.01, "{m}: switched run differs from a fresh run by {rel}");
    }
}

/// R1 (quick): the flow field is kept across the switch, the iteration counter continues and the next steps run.
#[test]
fn r1_switch_keeps_the_flow() {
    let svc = Service::new(ServiceConfig::default());
    let id = session(&svc, CHANNEL);
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    run_steps(&svc, &id, 200);
    for m in ["k_omega_sst", "spalart_allmaras", "k_epsilon"] {
        switch(&svc, &id, m);
        run_steps(&svc, &id, 100);
        assert!(field(&svc, &id, "nut").iter().all(|x| x.is_finite()));
    }
    assert_eq!(iteration(&svc, &id), 500);
}

/// R3
#[test]
fn r3_invalid_switches_leave_the_run_untouched() {
    let svc = Service::new(ServiceConfig::default());
    let id = session(&svc, CHANNEL);
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    run_steps(&svc, &id, 50);
    let ux0 = field(&svc, &id, "ux");
    let e = svc
        .call(
            "settings.set",
            &json!({"session": id, "path": "turbulence.model", "value": "plugin"}),
        )
        .unwrap_err();
    let msg = format!("{e:?}");
    assert!(msg.contains("turbulence.plugin"), "{msg}");
    assert_eq!(model_of(&svc, &id), "k_epsilon");
    assert_eq!(
        svc.call("settings.get", &json!({"session": id, "path": "turbulence.model"}))
            .unwrap()["value"],
        "k_epsilon"
    );
    // SST coefficient override rejected, located
    svc.call(
        "settings.set",
        &json!({"session": id, "path": "turbulence.model", "value": "k_omega_sst"}),
    )
    .unwrap();
    let e = svc
        .call(
            "settings.set",
            &json!({"session": id, "path": "turbulence.coefficients.k_omega_sst.a1", "value": 0.5}),
        )
        .unwrap_err();
    assert!(format!("{e:?}").contains("turbulence.coefficients.k_omega_sst.a1"));
    assert_eq!(model_of(&svc, &id), "k_omega_sst");
    // the run continues
    assert_eq!(field(&svc, &id, "ux"), ux0);
    run_steps(&svc, &id, 20);
    assert_eq!(iteration(&svc, &id), 70);
}

// ------------------------------------------------------------------ plugin

/// Constant eddy viscosity `nut = nut_ratio * nu` everywhere (registered as `const_nut`).
struct ConstNut {
    nu: f64,
    ratio: f64,
    nut: Vec<f64>,
    dist: Vec<f64>,
    n_faces: usize,
}

impl TurbulenceModel for ConstNut {
    fn key(&self) -> &str {
        "const_nut"
    }
    fn update(&mut self, _u: &rcfd_fv::VectorField, _phi: &[f64]) -> f64 {
        self.nut.iter_mut().for_each(|x| *x = self.ratio * self.nu);
        0.0
    }
    fn nu_face(&self) -> Vec<f64> {
        vec![self.nu * (1.0 + self.ratio); self.n_faces]
    }
    fn nut(&self) -> &[f64] {
        &self.nut
    }
    fn wall_distance(&self) -> &[f64] {
        &self.dist
    }
    fn scalars(&self) -> Vec<(&'static str, &ScalarField)> {
        Vec::new()
    }
    fn scalar_mut(&mut self, _name: &str) -> Option<&mut ScalarField> {
        None
    }
    fn y_plus_faces(&self) -> Vec<f64> {
        Vec::new()
    }
    fn wall_faces(&self) -> Vec<usize> {
        Vec::new()
    }
    fn coefficients(&self) -> Vec<CoefficientInfo> {
        vec![CoefficientInfo {
            name: "nut_ratio",
            value: self.ratio,
            default: 1.0,
            min: 0.0,
            max: 1e6,
            doc: "constant nut/nu",
        }]
    }
    fn set_coefficient(&mut self, name: &str, value: f64) -> Result<(), String> {
        if name == "nut_ratio" && value >= 0.0 {
            self.ratio = value;
            Ok(())
        } else {
            Err(format!("bad coefficient {name} = {value}"))
        }
    }
    fn initialize_from_nut(&mut self, _nut: &[f64]) {}
}

fn register_const_nut() {
    rcfd_turb::register_model(
        "const_nut",
        "constant eddy viscosity (test plugin)",
        Arc::new(|ctx: &ModelContext| {
            let n = ctx.geom.n_cells();
            Ok(Box::new(ConstNut {
                nu: ctx.nu,
                ratio: 1.0,
                nut: vec![ctx.nu; n],
                dist: vec![1.0; n],
                n_faces: ctx.geom.mesh.n_faces(),
            }) as Box<dyn TurbulenceModel>)
        }),
    );
}

/// R4
#[test]
fn r4_plugin_model_with_coefficient_override() {
    register_const_nut();
    let plugin = case(
        CHANNEL,
        &[
            ("turbulence.model", json!("plugin")),
            ("turbulence.plugin", json!("const_nut")),
        ],
    );
    let mut with_coef = plugin.clone();
    with_coef
        .tree
        .add_named("turbulence.plugin_coefficients", "nut_ratio")
        .unwrap();
    with_coef
        .set("turbulence.plugin_coefficients.nut_ratio.value", json!(9.0))
        .unwrap();
    let mut unknown = plugin.clone();
    unknown
        .tree
        .add_named("turbulence.plugin_coefficients", "bogus")
        .unwrap();
    let d = diags(&unknown);
    assert!(d.iter().any(|e| e.0 == "turbulence.plugin_coefficients.bogus"), "{d:?}");
    let s = steps(&with_coef, 3000);
    let coef = s.turbulence_coefficients();
    assert_eq!(coef, vec![("nut_ratio".to_string(), 9.0)]);
    // laminar run with 10 nu on the same mesh
    let lam = steps(
        &case(
            CHANNEL,
            &[
                ("turbulence.model", json!("laminar")),
                ("physics.nu", json!(10.0 * 0.002531645569620253)),
            ],
        ),
        3000,
    );
    let (a, b) = (mean(&s.field("ux").unwrap()), mean(&lam.field("ux").unwrap()));
    println!(
        "R4: plugin bulk {a:.6}, laminar (10 nu) bulk {b:.6}, relative difference {:.2e}",
        (a / b - 1.0).abs()
    );
    assert!((a / b - 1.0).abs() < 1e-3);
}
