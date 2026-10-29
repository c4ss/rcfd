//! ADR 0160 (job 48Z) item 3 through the run service and the library: P1 (`run.check` gives SV301), P3 (`run.headless`, the
//! `rcfd run` equivalent, emits it after `start`), P5 (the preflight changes no result of a service run), V1 (the pre-run
//! estimate of the power-law channel sees the law's viscosity and warns) and V4 (the preflight changes no result of a
//! rheology or turbulence run). Checks as registered in ADR 0160 before any run; none was changed afterwards. Only the API of
//! the unchanged tree is used; every case text is a `format!` template or the committed example case changed through
//! `settings.set` (ADR 0156 B2a's literal scan finds no case here).

use rcfd_api::case::Case;
use rcfd_api::runctl;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::sync::Arc;

/// The committed example case (`cylinder_ogrid` 64 x 48, PISO, dt 1/16).
fn example_text() -> String {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    std::fs::read_to_string(p).expect("example case readable")
}

/// ADR 0154 K1's wall layer (64 x 256, dt 1/64): the SV301 positive of ADR 0156 B1 and B3.
const K1: [(&str, f64); 3] = [("mesh.nx", 64.0), ("mesh.ny", 256.0), ("solver.dt", 0.015625)];

fn session(svc: &Arc<Service>, sets: &[(&str, Value)]) -> String {
    let sid = svc.call("session.create", &json!({"toml": example_text()})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    for (path, value) in sets {
        svc.call("settings.set", &json!({"session": sid, "path": path, "value": value}))
            .unwrap_or_else(|e| panic!("{path}: {e:?}"));
    }
    sid
}

fn k1_sets() -> Vec<(&'static str, Value)> {
    K1.iter()
        .map(|&(p, v)| {
            (
                p,
                if p.starts_with("mesh.") {
                    json!(v as i64)
                } else {
                    json!(v)
                },
            )
        })
        .collect()
}

fn sv301(rules: &Value) -> Vec<Value> {
    rules
        .as_array()
        .map(|a| a.iter().filter(|r| r["id"] == "SV301").cloned().collect())
        .unwrap_or_default()
}

fn is_sv301_warning(e: &Value) -> bool {
    e["event"] == "warning" && e["rule"] == "SV301"
}

#[test]
fn p1_run_check_gives_sv301_on_the_fine_wall_layer_only() {
    let svc = Service::new(ServiceConfig::default());
    let k1 = session(&svc, &k1_sets());
    let c = svc.call("run.check", &json!({"session": k1})).unwrap();
    let found = sv301(&c["event"]["rules"]);
    println!("P1 K1: exit {}, SV301 {found:?}", c["exit_code"]);
    assert_eq!(c["exit_code"], 0, "{c}");
    assert_eq!(found.len(), 1, "{c}");
    let r = &found[0];
    assert_eq!(r["severity"], "warning");
    assert_eq!(r["category"], "solver");
    assert_eq!(r["path"], "solver.corrector_control");
    assert_eq!(r["fix"]["value"], "adaptive");
    let committed = session(&svc, &[]);
    let c = svc.call("run.check", &json!({"session": committed})).unwrap();
    println!("P1 committed: exit {}, rules {}", c["exit_code"], c["event"]["rules"]);
    assert_eq!(c["exit_code"], 0, "{c}");
    assert!(sv301(&c["event"]["rules"]).is_empty(), "{c}");
}

fn headless(svc: &Arc<Service>, sid: &str, steps: u64) -> Vec<Value> {
    let r = svc
        .call("run.headless", &json!({"session": sid, "max_steps": steps}))
        .unwrap_or_else(|e| panic!("{e:?}"));
    r["events"].as_array().cloned().unwrap_or_default()
}

fn kinds(ev: &[Value]) -> Vec<String> {
    ev.iter()
        .map(|e| {
            let k = e["event"].as_str().unwrap_or("?").to_string();
            if is_sv301_warning(e) {
                format!("{k}:SV301")
            } else {
                k
            }
        })
        .collect()
}

#[test]
fn p3_run_headless_emits_sv301_after_start_once_per_run() {
    let svc = Service::new(ServiceConfig::default());
    let k1 = session(&svc, &k1_sets());
    let first = headless(&svc, &k1, 2);
    let second = headless(&svc, &k1, 1);
    println!("P3 K1 first call: {:?}", kinds(&first));
    println!("P3 K1 second call: {:?}", kinds(&second));
    let pos = |ev: &[Value], f: &dyn Fn(&Value) -> bool| ev.iter().position(f);
    let start = pos(&first, &|e| e["event"] == "start");
    let warn = pos(&first, &is_sv301_warning);
    let iter = pos(&first, &|e| e["event"] == "iteration");
    let mut problems = Vec::new();
    match (start, warn, iter) {
        (Some(s), Some(w), Some(i)) if s < w && w < i => {}
        other => problems.push(format!("K1 first call: start, SV301 warning, iteration at {other:?}")),
    }
    let info_warns = start.map(|s| sv301(&first[s]["info"]["warnings"]).len()).unwrap_or(0);
    if info_warns != 1 {
        problems.push(format!(
            "K1 first call: {info_warns} SV301 entries in start.info.warnings"
        ));
    }
    if second.iter().any(is_sv301_warning) {
        problems.push("K1 second call: SV301 again".into());
    }
    let committed = session(&svc, &[]);
    let ev = headless(&svc, &committed, 2);
    println!("P3 committed: {:?}", kinds(&ev));
    if ev.iter().any(|e| e["event"] == "warning") {
        problems.push(format!("committed: warning events {:?}", kinds(&ev)));
    }
    if ev
        .iter()
        .find(|e| e["event"] == "start")
        .is_some_and(|s| s["info"].get("warnings").is_some())
    {
        problems.push("committed: start.info has a warnings key".into());
    }
    assert!(problems.is_empty(), "P3: {problems:#?}");
}

#[test]
fn p5_the_preflight_changes_no_service_run() {
    let svc = Service::new(ServiceConfig::default());
    let warn = session(&svc, &[]);
    let off = session(&svc, &[("solver.pressure_lag_check", json!("off"))]);
    let keyed = |ev: &[Value]| -> Vec<(u64, u64, u64, u64, u64)> {
        ev.iter()
            .filter(|e| e["event"] == "iteration")
            .map(|e| {
                let b = |k: &str| e[k].as_f64().map_or(u64::MAX, f64::to_bits);
                (
                    e["index"].as_u64().unwrap_or(0),
                    b("time"),
                    b("res_u"),
                    b("res_p"),
                    b("continuity"),
                )
            })
            .collect()
    };
    let (a, b) = (keyed(&headless(&svc, &warn, 20)), keyed(&headless(&svc, &off, 20)));
    println!("P5: {} and {} iteration events", a.len(), b.len());
    assert!(!a.is_empty());
    assert_eq!(a, b, "P5: the iteration events differ");
}

// ------------------------------------------------------------------------------- V1, V4: the first-step viscosity

const H: f64 = 1.0;
const L: f64 = 6.0;
const U: f64 = 1.0;

/// `rheology.rs`'s channel text, made transient exactly as its `transient_runs_use_the_law_too` makes it.
fn channel(physics: &str, rheology: &str) -> String {
    let text = format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 60
ny = 20
lx = {L}
ly = {H}
depth = 0.1
[physics]
rho = 1.0
{physics}
[initial]
velocity = [{U}, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [{U}, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 4000
tolerance_u = 1e-9
tolerance_p = 1e-9

[rheology]
{rheology}
"#
    );
    text.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.02\nend_time = 0.4",
    )
    .replace("nx = 60", "nx = 20")
    .replace("ny = 20", "ny = 8")
}

/// The power-law arm `thin` of `transient_runs_use_the_law_too`.
fn thin() -> String {
    channel(
        "nu = 0.1",
        "model = \"power_law\"\nconsistency_index = 0.1\npower_index = 0.4\nmax_viscosity = 10.0\n",
    )
}

/// The Taylor-Green vortex of `sgs_tree.rs` (Smagorinsky) at step `dt`.
fn sgs_text(dt: f64) -> String {
    format!(
        r#"
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
dt = {dt}
end_time = 1.0
[turbulence]
model = "smagorinsky"
"#
    )
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn v1_the_power_law_channel_preflight_sees_the_law_and_warns() {
    let case = parse(&thin());
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let p = sim
        .pressure_lag_preflight()
        .expect("the preflight applies to the PISO channel");
    let (code, ev) = runctl::check_only(&case);
    let found = sv301(&ev["rules"]);
    println!(
        "V1: rho {:.6} at step {} (cell {}, {:?}); check_only exit {code}, SV301 {}; ADR 0077's in-step estimate 4.17",
        p.rho,
        p.step,
        p.cell,
        p.position,
        found.len()
    );
    assert!(p.rho >= rcfd_api::rules::SV301_LEVEL, "V1: rho {} below 1.3", p.rho);
    assert_eq!(found.len(), 1, "V1: {ev}");
}

fn state_bits(s: &Simulation) -> Vec<Vec<u64>> {
    let b = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    vec![
        b(&s.field("ux").unwrap()),
        b(&s.field("uy").unwrap()),
        b(&s.field("p").unwrap()),
        b(s.flux()),
    ]
}

#[test]
fn v4_the_preflight_changes_no_rheology_or_turbulence_run() {
    for (name, text) in [("thin", thin()), ("sgs", sgs_text(0.01))] {
        let case = parse(&text);
        let mut with = Simulation::new(&case).unwrap();
        let rho = with.pressure_lag_preflight().map(|p| p.rho);
        let mut without = Simulation::new(&case).unwrap();
        for _ in 0..5 {
            with.step();
            without.step();
        }
        println!("V4 {name}: preflight rho {rho:?}; 5 steps each");
        assert!(rho.is_some(), "V4 {name}: the preflight does not apply");
        assert!(
            state_bits(&with) == state_bits(&without),
            "V4 {name}: the preflight changed the run"
        );
    }
}
