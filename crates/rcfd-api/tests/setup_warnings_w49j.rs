//! ADR 0172 (job 49J), library level: SV301 at the settled step and its run-time watch (LS1 to LS7), the one warning path of
//! `run.headless` and the chain driver (LP1, LP2), SV302 and the wall-cell metric on a synthetic sheared, wall-stretched channel
//! (LW6, LW7 as far as a synthetic mesh shows, LW9) and the guard hint (LH2, LH3). The registered 49H meshes are in
//! `setup_warnings_w49j_meshes.rs`; the command line is in `crates/rcfd-cli/tests/w49j_warnings.rs`. Limits as registered in the
//! ADR before any of these ran; none was changed afterwards unless an amendment says so.

use rcfd_api::case::Case;
use rcfd_api::chain;
use rcfd_api::rules::{self, SimFacts};
use rcfd_api::runctl::{self, LagWatch, RunOptions};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn example_text() -> String {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    std::fs::read_to_string(p).expect("example case readable")
}

fn case_with(text: &str, sets: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(text).expect("case parses");
    for (p, v) in sets {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    c
}

/// ADR 0154 K1 (64 x 256, dt 1/64): the SV301 positive.
fn k1(extra: &[(&str, Value)]) -> Case {
    let mut sets: Vec<(&str, Value)> = vec![
        ("mesh.nx", json!(64)),
        ("mesh.ny", json!(256)),
        ("solver.dt", json!(0.015625)),
    ];
    sets.extend_from_slice(extra);
    case_with(&example_text(), &sets)
}

/// C-small: K1 under the legacy Courant controller with a tiny target.
fn c_small() -> Case {
    k1(&[("solver.max_courant", json!(0.01))])
}

/// C-grow: the first step 1/1024 grows to `max_dt` 1/64 (the Courant criterion never binds).
fn c_grow() -> Case {
    case_with(
        &example_text(),
        &[
            ("mesh.nx", json!(64)),
            ("mesh.ny", json!(256)),
            ("solver.dt", json!(0.0009765625)),
            ("solver.max_dt", json!(0.015625)),
            ("solver.max_courant", json!(1000.0)),
        ],
    )
}

fn sv301(rules: &Value) -> Vec<Value> {
    rules
        .as_array()
        .map(|a| a.iter().filter(|r| r["id"] == "SV301").cloned().collect())
        .unwrap_or_default()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

// ------------------------------------------------------------------------------------------------ SV301 at the settled step

#[test]
fn ls1_a_fixed_step_is_estimated_exactly_as_before() {
    let sim = Simulation::new(&k1(&[])).unwrap();
    assert_eq!(sim.settled_dt_estimate().to_bits(), 0.015625f64.to_bits());
    let a = sim.pressure_lag_preflight().expect("K1 has an estimate");
    let b = sim.pressure_lag_preflight_at(0.015625).unwrap();
    println!("LS1 K1: rho {:.6} at cell {} (settled == solver.dt)", a.rho, a.cell);
    assert_eq!(a.rho.to_bits(), b.rho.to_bits());
    assert_eq!((a.cell, a.step, a.n_correctors), (b.cell, b.step, b.n_correctors));
    assert_eq!(bits(&a.position), bits(&b.position));
    assert!(
        a.rho >= rules::SV301_LEVEL,
        "K1 must stay the SV301 positive: {}",
        a.rho
    );
}

#[test]
fn ls2_a_large_initial_step_that_the_controller_will_cut_does_not_warn() {
    let case = c_small();
    let sim = Simulation::new(&case).unwrap();
    let settled = sim.settled_dt_estimate();
    let at_first = sim.pressure_lag_preflight_at(0.015625).unwrap();
    let at_settled = sim.pressure_lag_preflight().unwrap();
    println!(
        "LS2: settled dt {settled:e} (solver.dt 1.5625e-2); rho at the first step {:.4}, at the settled step {:.4}",
        at_first.rho, at_settled.rho
    );
    assert!(settled <= 0.015625 / 10.0, "settled {settled}");
    assert!(at_first.rho >= rules::SV301_LEVEL, "the old behaviour would warn");
    let (code, ev) = runctl::check_only(&case);
    assert_eq!(code, 0, "{ev}");
    assert!(sv301(&ev["rules"]).is_empty(), "{}", ev["rules"]);
}

#[test]
fn ls3_a_step_that_grows_into_the_risk_warns_before_the_run() {
    let case = c_grow();
    let sim = Simulation::new(&case).unwrap();
    let k = Simulation::new(&k1(&[])).unwrap().pressure_lag_preflight().unwrap();
    let settled = sim.settled_dt_estimate();
    let at_first = sim.pressure_lag_preflight_at(0.0009765625).unwrap();
    let pre = sim.pressure_lag_preflight().unwrap();
    println!(
        "LS3: settled dt {settled:e}; rho at the first step {:.4}, pre-run {:.6} (K1 {:.6})",
        at_first.rho, pre.rho, k.rho
    );
    assert_eq!(settled.to_bits(), 0.015625f64.to_bits());
    assert!(((pre.rho - k.rho) / k.rho).abs() < 1e-9);
    assert!(at_first.rho < rules::SV301_LEVEL);
    let (code, ev) = runctl::check_only(&case);
    assert_eq!(code, 0, "{ev}");
    assert_eq!(sv301(&ev["rules"]).len(), 1, "{}", ev["rules"]);
}

fn warning_rules(events: &[Value]) -> Vec<(usize, String)> {
    events
        .iter()
        .enumerate()
        .filter(|(_, e)| e["event"] == "warning" && e["rule"] == "SV301")
        .map(|(i, e)| (i, e["message"].as_str().unwrap_or("").to_string()))
        .collect()
}

fn drive_events(sim: Simulation, steps: usize, info: Value) -> (Simulation, Vec<Value>) {
    let m = Mutex::new(sim);
    let opts = RunOptions {
        max_steps: Some(steps),
        info,
        ..RunOptions::default()
    };
    let mut ev = Vec::new();
    runctl::drive(&m, &opts, &|| false, &mut |e| ev.push(e.clone()));
    (m.into_inner().unwrap(), ev)
}

#[test]
fn ls4_ls6_the_watch_finds_the_risk_the_pre_run_estimate_did_not_and_changes_no_result() {
    // no pre-run call, `info` without warnings: only the watch can warn
    let (watched, ev) = drive_events(Simulation::new(&c_grow()).unwrap(), 60, json!({}));
    let w = warning_rules(&ev);
    println!("LS4: SV301 events {:?}", w.iter().map(|(i, _)| *i).collect::<Vec<_>>());
    if let Some((_, m)) = w.first() {
        println!("LS4 message: {m}");
    }
    assert_eq!(w.len(), 1, "exactly one SV301 event: {w:?}");
    let start = ev.iter().position(|e| e["event"] == "start").unwrap();
    assert!(w[0].0 > start);
    let first_iter = ev.iter().position(|e| e["event"] == "iteration").unwrap();
    assert!(w[0].0 > first_iter, "found at a step after the first");
    // LS6: the same 60 steps by hand
    let mut plain = Simulation::new(&c_grow()).unwrap();
    for _ in 0..60 {
        plain.step();
    }
    for f in ["ux", "uy", "p"] {
        assert_eq!(
            bits(&watched.field(f).unwrap()),
            bits(&plain.field(f).unwrap()),
            "field {f} differs"
        );
    }
    assert_eq!(watched.time().to_bits(), plain.time().to_bits());
}

#[test]
fn ls5_the_watch_is_silent_after_a_pre_run_sv301() {
    let pre = json!([{"message": "pre-run SV301", "rule": "SV301", "path": "solver.corrector_control", "category": "solver"}]);
    let (_, ev) = drive_events(Simulation::new(&c_grow()).unwrap(), 60, json!({"warnings": pre}));
    let w = warning_rules(&ev);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].1, "pre-run SV301");
}

#[test]
fn ls7_the_watch_is_cheap() {
    let mut sim = Simulation::new(&c_grow()).unwrap();
    let mut watch = LagWatch::new(&sim, false);
    let mut fired = 0;
    for _ in 0..60 {
        sim.step();
        if watch.observe(&sim).is_some() {
            fired += 1;
        }
    }
    println!("LS7: {} evaluations in 60 steps, {fired} warning", watch.evaluations());
    assert!(watch.evaluations() <= 12, "{}", watch.evaluations());
    assert_eq!(fired, 1);
}

// ------------------------------------------------------------------------------------------------------- one warning path

const CAVITY: &str = r#"
title = "w49j cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 50
"#;

#[test]
fn lp1_run_headless_shows_the_static_warnings_too() {
    let svc = Service::new(ServiceConfig::default());
    let session = |toml: &str| {
        svc.call("session.create", &json!({"toml": toml})).unwrap()["session"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let loose = session(&format!("{CAVITY}tolerance_u = 0.1\n"));
    let r = svc
        .call("run.headless", &json!({"session": loose, "max_steps": 2}))
        .unwrap();
    let ev = r["events"].as_array().unwrap().clone();
    let kinds: Vec<String> = ev
        .iter()
        .map(|e| {
            let k = e["event"].as_str().unwrap_or("?");
            match e["rule"].as_str() {
                Some(r) => format!("{k}:{r}"),
                None => k.to_string(),
            }
        })
        .collect();
    println!("LP1 loose: {kinds:?}");
    let start = kinds.iter().position(|k| k == "start").unwrap();
    let warn = kinds
        .iter()
        .position(|k| k == "warning:SV105")
        .expect("SV105 warning event");
    let iter = kinds.iter().position(|k| k == "iteration").unwrap();
    assert!(start < warn && warn < iter, "{kinds:?}");
    let info = ev[start]["info"]["warnings"].as_array().expect("start.info.warnings");
    assert!(info.iter().any(|w| w["rule"] == "SV105"), "{info:?}");
    // the committed example case: nothing, as ADR 0160 P3 requires
    let committed = session(&example_text());
    let r = svc
        .call("run.headless", &json!({"session": committed, "max_steps": 2}))
        .unwrap();
    let ev = r["events"].as_array().unwrap();
    assert!(ev.iter().all(|e| e["event"] != "warning"), "{ev:?}");
    assert!(ev[0]["info"].get("warnings").is_none() || ev[0]["info"]["warnings"].is_null());
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w49j_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_chain(case: &Case, out: &Path, steps: usize) -> Vec<Value> {
    let mut events = Vec::new();
    let base = RunOptions {
        max_steps: Some(steps),
        ..RunOptions::default()
    };
    chain::run_chain(case, &base, out, &|| false, &mut |e| events.push(e.clone()));
    events
}

#[test]
fn lp2_the_chain_driver_shows_the_static_warnings_and_sv301() {
    // a steady stage with a loose tolerance: SV105
    let text = format!("{CAVITY}tolerance_u = 0.1\n[chain]\nsequence = [\"a\"]\n[chain.stages.a]\n");
    let ev = run_chain(&Case::parse_toml(&text).unwrap(), &tmp("lp2a"), 2);
    let warns: Vec<&Value> = ev
        .iter()
        .filter(|e| e["event"] == "warning" && e["rule"] == "SV105")
        .collect();
    println!("LP2 SV105: {} warning events", warns.len());
    assert_eq!(warns.len(), 1, "{ev:?}");
    let start = ev.iter().find(|e| e["event"] == "start").unwrap();
    assert!(start["info"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["rule"] == "SV105"));
    // a K1 transient stage: SV301 (the chain driver did not show it before)
    let mut text = example_text();
    text.push_str("\n[chain]\nsequence = [\"a\"]\n[chain.stages.a]\n");
    let mut case = Case::parse_toml(&text).unwrap();
    for (p, v) in [
        ("mesh.nx", json!(64)),
        ("mesh.ny", json!(256)),
        ("solver.dt", json!(0.015625)),
    ] {
        case.set(p, v).unwrap();
    }
    let ev = run_chain(&case, &tmp("lp2b"), 2);
    let w = warning_rules(&ev);
    println!("LP2 K1: SV301 events {:?}", w.iter().map(|x| x.0).collect::<Vec<_>>());
    assert_eq!(w.len(), 1, "{ev:?}");
}

// ----------------------------------------------------------------------------- SV302 on a synthetic sheared wall-stretched channel

/// A 16 x 20 x 1 channel, 2 x 1 x 1, geometric wall clustering, sheared by `deg` degrees (x += y tan(deg)), written as polyMesh.
fn sheared_channel(dir: &Path, deg: f64) -> (PathBuf, rcfd_mesh::Mesh) {
    use rcfd_mesh::gen::{channel, warp, Grading};
    let m = channel(
        [16, 20, 1],
        [2.0, 1.0, 1.0],
        [Grading::Uniform, Grading::Symmetric(1.0e4), Grading::Uniform],
        true,
    )
    .unwrap();
    let t = deg.to_radians().tan();
    let m = warp(&m, |p| [p[0] + t * p[1], p[1], p[2]]).unwrap();
    let d = dir.join(format!("mesh{deg}"));
    rcfd_io::openfoam::write_dir(&m, &d).unwrap();
    (d, m)
}

fn channel_case(mesh: &Path, solver: &str) -> Case {
    Case::parse_toml(&channel_text(mesh, solver)).unwrap()
}

fn channel_text(mesh: &Path, solver: &str) -> String {
    let file = mesh.display().to_string().replace('\\', "/");
    format!(
        r#"
title = "w49j sheared channel"
[mesh]
source = "file"
file = "{file}"
[physics]
nu = 0.01
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
{solver}
"#
    )
}

const STEADY_LU: &str = "algorithm = \"simplec\"\nconvection = \"linear_upwind\"\nmax_iterations = 3\n";

fn sv302(rules: &Value) -> Vec<Value> {
    rules
        .as_array()
        .map(|a| a.iter().filter(|r| r["id"] == "SV302").cloned().collect())
        .unwrap_or_default()
}

#[test]
fn lw9_the_metric_is_the_hand_value_on_a_sheared_channel_and_zero_unsheared() {
    let dir = tmp("lw9");
    let (_, sheared) = sheared_channel(&dir, 70.0);
    let (_, plain) = sheared_channel(&dir, 0.0);
    let case = channel_case(&dir, STEADY_LU);
    let r = rules::wall_cell_risk(&case, &sheared).expect("wall cells");
    // the hand value: the wall cell's shortest edge is the slanted one, dy0 / cos(70), the longest the span edge 1.0
    let dy0 = rcfd_mesh::gen::axis_coords(20, 1.0, rcfd_mesh::gen::Grading::Symmetric(1.0e4)).unwrap()[1];
    let hand = 70f64.to_radians().sin() / dy0;
    println!(
        "LW9: metric {:.1} (aspect {:.1}, non-orthogonality {:.3}), hand sin(70)/dy0 = {hand:.1}, dy0 = {dy0:e}",
        r.metric, r.aspect, r.non_orth_deg
    );
    assert!(((r.metric - hand) / hand).abs() < 0.01, "{} against {hand}", r.metric);
    assert!(r.metric >= rules::SV302_LEVEL);
    assert!(rules::linear_upwind_wall_rule(&r).is_some());
    let p = rules::wall_cell_risk(&case, &plain).expect("wall cells");
    println!("LW9 unsheared: metric {:.3e}", p.metric);
    assert!(p.metric < 1.0 && rules::linear_upwind_wall_rule(&p).is_none());
}

#[test]
fn lw6_lw7_sv302_fires_only_for_unlimited_linear_upwind_on_the_steady_solver_and_is_a_warning() {
    let dir = tmp("lw6");
    let (mesh, _) = sheared_channel(&dir, 70.0);
    let run = |solver: &str| {
        let case = channel_case(&mesh, solver);
        let (code, ev) = runctl::check_only(&case);
        (code, sv302(&ev["rules"]), ev)
    };
    let (code, fired, ev) = run(STEADY_LU);
    assert_eq!(code, 0, "a warning, not an error: {ev}");
    assert_eq!(fired.len(), 1, "{}", ev["rules"]);
    let r = &fired[0];
    println!("LW7 message: {}", r["message"]);
    assert_eq!(r["severity"], "warning");
    assert_eq!(r["path"], "solver.linear_upwind_limiter");
    assert!(r.get("fix").is_none() || r["fix"].is_null(), "no fix object: {r}");
    let m = r["message"].as_str().unwrap();
    for needle in [
        "solver.linear_upwind_limiter",
        "barth_jespersen",
        "3 %",
        "risk",
        "0 degrees",
    ] {
        assert!(m.contains(needle), "message lacks {needle:?}: {m}");
    }
    // the cause removed, or another solver: silent
    for (name, solver) in [
        ("limiter on", "algorithm = \"simplec\"\nconvection = \"linear_upwind\"\nlinear_upwind_limiter = \"barth_jespersen\"\nmax_iterations = 3\n"),
        ("upwind", "algorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = 3\n"),
        ("transient", "algorithm = \"piso\"\nconvection = \"linear_upwind\"\ndt = 0.001\nend_time = 0.002\n"),
    ] {
        let (code, fired, ev) = run(solver);
        println!("LW6 {name}: exit {code}, SV302 {}", fired.len());
        assert!(code == 0 && fired.is_empty(), "{name}: {}", ev["rules"]);
    }
}

#[test]
fn the_same_list_in_check_run_headless_and_the_chain_driver() {
    let dir = tmp("lp3");
    let (mesh, _) = sheared_channel(&dir, 70.0);
    let case = channel_case(&mesh, STEADY_LU);
    // run.check
    let (_, ev) = runctl::check_only(&case);
    let from_check = sv302(&ev["rules"]);
    assert_eq!(from_check.len(), 1);
    let text = from_check[0]["message"].as_str().unwrap().to_string();
    // run.headless
    let svc = Service::new(ServiceConfig::default());
    let sid = svc
        .call("session.create", &json!({"toml": channel_text(&mesh, STEADY_LU)}))
        .unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc
        .call("run.headless", &json!({"session": sid, "max_steps": 1}))
        .unwrap();
    let ev = r["events"].as_array().unwrap();
    let w = ev
        .iter()
        .find(|e| e["event"] == "warning" && e["rule"] == "SV302")
        .expect("headless SV302");
    assert_eq!(w["message"].as_str().unwrap(), text);
    assert_eq!(w["path"], "solver.linear_upwind_limiter");
    // the chain driver
    let c = Case::parse_toml(&format!(
        "{}
[chain]
sequence = [\"a\"]
[chain.stages.a]
",
        channel_text(&mesh, STEADY_LU)
    ))
    .unwrap();
    let ev = run_chain(&c, &tmp("lp3c"), 1);
    let w = ev
        .iter()
        .find(|e| e["event"] == "warning" && e["rule"] == "SV302")
        .expect("chain SV302");
    assert_eq!(w["message"].as_str().unwrap(), text);
}

// ------------------------------------------------------------------------------------------------------------ the guard hint

#[test]
fn lh2_lh3_the_hint_names_the_limiter_for_a_wall_cell_under_unlimited_linear_upwind_only() {
    let dir = tmp("lh");
    let (mesh, m) = sheared_channel(&dir, 70.0);
    let wall_cell = m.owner()[m.patch("walls").unwrap().start];
    let interior = (0..m.n_cells())
        .find(|&c| {
            !m.cell_faces(c)
                .iter()
                .any(|&f| f >= m.n_internal_faces() && m.patch_of_face(f).is_some_and(|p| p.name != "sides"))
        })
        .expect("an interior cell");
    let sim = |solver: &str| Simulation::new(&channel_case(&mesh, solver)).unwrap();
    let unlimited = sim(STEADY_LU);
    let hint = unlimited.steady_guard_hint(Some(wall_cell)).expect("a wall cell");
    println!("LH2 hint: {hint}");
    assert!(hint.contains("solver.linear_upwind_limiter") && hint.contains("barth_jespersen"));
    assert!(!hint.contains("lower the relaxation factors"));
    // the full message, as steady_divergence_message builds it
    let d = rcfd_fv::simple::SteadyDivergence {
        iteration: 15,
        kind: "velocity_runaway",
        quantity: "velocity",
        cell: Some(wall_cell),
        value: 1.0e4,
        limit: 1.0e3,
        last_good: 3,
        last_good_continuity: 1e-3,
    };
    let msg = d.message_with_hint(Some(unlimited.geom.mesh.cell_centres()), Some(hint));
    assert!(
        msg.contains("barth_jespersen") && !msg.contains("lower the relaxation factors"),
        "{msg}"
    );
    // LH3: elsewhere, the old advice
    assert!(
        unlimited.steady_guard_hint(Some(interior)).is_none(),
        "interior cell {interior}"
    );
    assert!(unlimited.steady_guard_hint(None).is_none());
    let limited = sim("algorithm = \"simplec\"\nconvection = \"linear_upwind\"\nlinear_upwind_limiter = \"barth_jespersen\"\nmax_iterations = 3\n");
    assert!(limited.steady_guard_hint(Some(wall_cell)).is_none());
    let upwind = sim("algorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = 3\n");
    assert!(upwind.steady_guard_hint(Some(wall_cell)).is_none());
    // SimFacts agrees with the simulation
    assert!(SimFacts::of(&unlimited).unlimited_linear_upwind && !SimFacts::of(&upwind).unlimited_linear_upwind);
}
