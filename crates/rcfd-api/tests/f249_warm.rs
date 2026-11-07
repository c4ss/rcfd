//! ADR 0209 W1 to W4 and WU1 to WU3 (F-249): the extended warm start (temperature and the k-epsilon / k-omega SST fields)
//! against cold starts on three steady SIMPLEC cases, its seeding, its refusals, and the unchanged default path. Limits are
//! those registered in ADR 0209 before the code.

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{
    run_case, run_case_with, CaseEvaluator, ExtendedMapEvaluator, Fields, RunLimits, WarmStart,
};
use rcfd_api::design::opmap::{self, Axis, MapOptions, MapOutcome, PointStatus};
use rcfd_api::design::study::{Condition, Evaluator, ObjectiveDef, ParamDef, Robust, Sense, StudyDef, Target};
use rcfd_api::sim::{Simulation, WarmState};
use serde_json::json;

/// `|a - b| <= REL_WARM max(|a|, |b|)` on every report (ADR 0209).
const REL_WARM: f64 = 1e-4;

const HE: &str = r#"
[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
cp = 1.0
conductivity = 0.05
[energy]
enabled = true
buoyancy = false
[initial]
temperature = 0.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.5, 0.0, 0.0]
temperature = "fixed"
temperature_value = 0.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
temperature = "zero_gradient"
[boundaries.walls]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
[reports.t_out]
type = "area_average"
patch = "outlet"
field = "T"
[reports.q_walls]
type = "heat_flow"
patch = "walls"
"#;

/// The ADR 0067 M3 channel (no energy).
const M3: &str = r#"
[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.5, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
"#;

fn parse(text: &str) -> Case {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let sem = c.semantic_errors();
    assert!(sem.is_empty(), "{sem:?}");
    c
}

fn with(mut c: Case, path: &str, v: serde_json::Value) -> Case {
    rcfd_api::study::apply_value(&mut c, path, &v).unwrap();
    c
}

fn he(u: f64) -> Case {
    with(parse(HE), "boundaries.inlet.velocity_value", json!([u, 0.0, 0.0]))
}

fn shipped(file: &str, g: f64) -> Case {
    let path = format!("{}/../../cases/channel/{file}", env!("CARGO_MANIFEST_DIR"));
    let c = parse(&std::fs::read_to_string(path).unwrap());
    with(c, "physics.body_force", json!([g, 0.0, 0.0]))
}

fn rel(a: f64, b: f64) -> f64 {
    let m = a.abs().max(b.abs());
    if m == 0.0 {
        0.0
    } else {
        (a - b).abs() / m
    }
}

fn map(base: Case, path: &str, values: Vec<serde_json::Value>, cold: bool) -> MapOutcome {
    let pts = opmap::grid_points(
        &[Axis {
            path: path.into(),
            values,
        }],
        false,
    )
    .unwrap();
    let ev = ExtendedMapEvaluator {
        base,
        limits: RunLimits::default(),
    };
    opmap::run_map(
        &pts,
        &ev,
        &MapOptions {
            cold,
            ..Default::default()
        },
        &mut |_| {},
    )
}

/// The W1 to W3 pass rule; returns (warm iterations, cold iterations) of rows 1.. and the largest relative difference.
fn compare(tag: &str, warm: &MapOutcome, cold: &MapOutcome) -> (usize, usize, f64) {
    assert_eq!(warm.rows.len(), cold.rows.len());
    let mut worst = 0.0f64;
    let (mut iw, mut ic) = (0, 0);
    for (w, c) in warm.rows.iter().zip(&cold.rows) {
        println!(
            "{tag} point {}: warm {} it ({}, '{}'), cold {} it ({}); {:?} | {:?}",
            w.index, w.iterations, w.restart, w.message, c.iterations, c.restart, w.metrics, c.metrics
        );
        assert_eq!(
            w.status,
            PointStatus::Converged,
            "{tag} warm point {}: {}",
            w.index,
            w.message
        );
        assert_eq!(
            c.status,
            PointStatus::Converged,
            "{tag} cold point {}: {}",
            c.index,
            c.message
        );
        assert_eq!(c.restart, "cold");
        assert!(!w.metrics.is_empty());
        assert_eq!(
            w.metrics.keys().collect::<Vec<_>>(),
            c.metrics.keys().collect::<Vec<_>>()
        );
        if w.index == 0 {
            assert_eq!(w.restart, "cold");
            assert_eq!(w.iterations, c.iterations, "{tag}: point 0 differs");
            for (k, v) in &w.metrics {
                assert_eq!(v.to_bits(), c.metrics[k].to_bits(), "{tag}: point 0 {k} differs");
            }
            continue;
        }
        assert_eq!(w.restart, "warm", "{tag} point {}: {}", w.index, w.message);
        iw += w.iterations;
        ic += c.iterations;
        for (k, v) in &w.metrics {
            let r = rel(*v, c.metrics[k]);
            worst = worst.max(r);
            assert!(r <= REL_WARM, "{tag} point {} {k}: {r:.3e} (limit {REL_WARM})", w.index);
        }
    }
    println!("{tag}: largest relative difference {worst:.3e}; warm rows 1.. {iw} iterations, cold {ic}");
    (iw, ic, worst)
}

#[test]
fn w1_energy_channel_warm_points_reproduce_cold_points() {
    let v = vec![json!([0.5, 0.0, 0.0]), json!([0.6, 0.0, 0.0]), json!([0.7, 0.0, 0.0])];
    let warm = map(he(0.5), "boundaries.inlet.velocity_value", v.clone(), false);
    let cold = map(he(0.5), "boundaries.inlet.velocity_value", v, true);
    let (iw, ic, _) = compare("W1", &warm, &cold);
    println!("W1b: warm {iw} against cold {ic} iterations");
    assert!(iw < ic, "W1b: warm {iw} iterations, cold {ic}");
}

#[test]
fn w2_k_epsilon_channel_warm_points_reproduce_cold_points() {
    let v = vec![json!([1.0, 0.0, 0.0]), json!([1.1, 0.0, 0.0]), json!([1.2, 0.0, 0.0])];
    let base = shipped("kepsilon_channel.toml", 1.0);
    let warm = map(base.clone(), "physics.body_force", v.clone(), false);
    let cold = map(base, "physics.body_force", v, true);
    compare("W2", &warm, &cold);
}

#[test]
fn w3_sst_channel_warm_points_reproduce_cold_points() {
    let v = vec![json!([1.0, 0.0, 0.0]), json!([1.1, 0.0, 0.0]), json!([1.2, 0.0, 0.0])];
    let base = shipped("sst_channel.toml", 1.0);
    let warm = map(base.clone(), "physics.body_force", v.clone(), false);
    let cold = map(base, "physics.body_force", v, true);
    compare("W3", &warm, &cold);
}

#[test]
fn w4_chained_conditions_with_the_extended_warm_start() {
    let def = StudyDef {
        name: "w4".into(),
        params: vec![ParamDef {
            name: "k".into(),
            lo: 0.04,
            hi: 0.06,
            init: Some(0.05),
            targets: vec![Target {
                path: "physics.conductivity".into(),
                scale: 1.0,
                offset: 0.0,
            }],
        }],
        conditions: vec![
            Condition {
                label: "a".into(),
                set: vec![],
            },
            Condition {
                label: "b".into(),
                set: vec![("boundaries.inlet.velocity_value".into(), json!([0.6, 0.0, 0.0]))],
            },
        ],
        objectives: vec![ObjectiveDef {
            metric: "dp".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers: 1,
    };
    let mut chained = CaseEvaluator::new(he(0.5), RunLimits::default(), true);
    chained.warm = WarmStart::Extended;
    let unchained = CaseEvaluator::new(he(0.5), RunLimits::default(), false);
    let w = chained.evaluate(&def, &[0.05]);
    let c = unchained.evaluate(&def, &[0.05]);
    for (x, y) in w.iter().zip(&c) {
        println!(
            "W4 {}: chained {} it '{}' {:?} | unchained {} it {:?}",
            x.label, x.iterations, x.message, x.metrics, y.iterations, y.metrics
        );
        assert!(x.ok && y.ok, "W4 {}: {} / {}", x.label, x.message, y.message);
    }
    for (k, v) in &w[0].metrics {
        assert_eq!(v.to_bits(), c[0].metrics[k].to_bits(), "W4 a {k}");
    }
    assert!(!w[1].message.contains("cold start"), "W4 b ran cold: {}", w[1].message);
    let mut worst = 0.0f64;
    for (k, v) in &w[1].metrics {
        let r = rel(*v, c[1].metrics[k]);
        worst = worst.max(r);
        assert!(r <= REL_WARM, "W4 b {k}: {r:.3e}");
    }
    println!("W4: largest relative difference {worst:.3e}");
}

/// A converged run's extended warm state.
fn state_of(c: &Case) -> WarmState {
    let r = run_case_with(c, None, &RunLimits::default(), WarmStart::Extended);
    assert_eq!(r.state, "converged", "{}", r.message);
    r.fields
        .and_then(|f| f.state)
        .expect("a converged steady run gives a warm state")
}

const FIELDS: [&str; 9] = ["ux", "uy", "uz", "p", "T", "k", "epsilon", "omega", "nut"];

/// Bits of every field the simulation has among [`FIELDS`].
fn snapshot(sim: &Simulation) -> Vec<(&'static str, Vec<u64>)> {
    FIELDS
        .iter()
        .filter_map(|f| sim.field(f).ok().map(|v| (*f, v.iter().map(|x| x.to_bits()).collect())))
        .collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn seeded_equals(tag: &str, case: &Case, st: &WarmState) {
    let mut sim = Simulation::new(case).unwrap();
    sim.seed_state(st).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let comp = |k: usize| st.u.iter().map(|u| u[k]).collect::<Vec<f64>>();
    assert_eq!(bits(&sim.field("ux").unwrap()), bits(&comp(0)), "{tag} ux");
    assert_eq!(bits(&sim.field("uy").unwrap()), bits(&comp(1)), "{tag} uy");
    assert_eq!(bits(&sim.field("uz").unwrap()), bits(&comp(2)), "{tag} uz");
    assert_eq!(bits(&sim.field("p").unwrap()), bits(&st.p), "{tag} p");
    let mut checked = vec!["ux", "uy", "uz", "p"];
    if let Some(t) = &st.t {
        assert_eq!(bits(&sim.field("T").unwrap()), bits(t), "{tag} T");
        checked.push("T");
    }
    if let Some(tw) = &st.turbulence {
        assert_eq!(bits(&sim.field("nut").unwrap()), bits(&tw.nut), "{tag} nut");
        checked.push("nut");
        for (n, v) in &tw.fields {
            assert_eq!(bits(&sim.field(n).unwrap()), bits(v), "{tag} {n}");
            checked.push(n);
        }
    }
    println!("WU1 {tag}: {checked:?} equal bit for bit");
}

fn refused(tag: &str, case: &Case, st: &WarmState, needle: &str) {
    let mut sim = Simulation::new(case).unwrap();
    let before = snapshot(&sim);
    let e = sim.seed_state(st).expect_err(tag);
    println!("WU2 {tag}: {e}");
    assert!(e.contains(needle), "{tag}: '{e}' does not name '{needle}'");
    assert_eq!(snapshot(&sim), before, "{tag}: the simulation changed");
}

#[test]
fn wu1_wu2_seed_state_copies_every_field_or_refuses_without_change() {
    let he_state = state_of(&he(0.5));
    assert!(he_state.t.is_some() && he_state.turbulence.is_none());
    seeded_equals("HE", &he(0.6), &he_state);
    let ke_state = state_of(&shipped("kepsilon_channel.toml", 1.0));
    assert_eq!(ke_state.turbulence.as_ref().unwrap().model, "k_epsilon");
    seeded_equals("KE", &shipped("kepsilon_channel.toml", 1.1), &ke_state);
    let sst_state = state_of(&shipped("sst_channel.toml", 1.0));
    assert_eq!(sst_state.turbulence.as_ref().unwrap().model, "k_omega_sst");
    seeded_equals("SST", &shipped("sst_channel.toml", 1.1), &sst_state);
    // WU2: refusals leave the simulation as it was
    let piso = HE.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.01\nend_time = 0.1",
    );
    refused("PISO", &parse(&piso), &he_state, "steady SIMPLE and SIMPLEC");
    refused("HE into M3", &parse(M3), &he_state, "temperature");
    let m3_state = state_of(&parse(M3));
    refused("M3 into HE", &he(0.6), &m3_state, "temperature");
    refused("KE into SST", &shipped("sst_channel.toml", 1.0), &ke_state, "k_epsilon");
    refused("mesh", &parse(&HE.replace("nx = 40", "nx = 41")), &he_state, "cells");
    let scalar = format!("{HE}\n[scalars.c]\ndiffusivity = 0.01\n");
    refused("scalar", &parse(&scalar), &he_state, "scalars");
}

#[test]
fn wu3_the_flow_path_is_unchanged() {
    let r = run_case(&he(0.5), None, &RunLimits::default());
    let f: Fields = r.fields.expect("converged");
    assert!(f.state.is_none(), "the Flow path collects no warm state");
    let r2 = run_case(&he(0.6), Some(&f), &RunLimits::default());
    println!("WU3: warm {}, '{}'", r2.warm, r2.message);
    assert!(!r2.warm);
    assert!(r2.message.contains(
        "cold start: warm start carries velocity and pressure only; this case has energy or turbulence fields"
    ));
}
