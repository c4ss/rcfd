//! ADR 0077 part A through the API: checks A2a, A2b (FAN-01 `linear_upwind`, the 34D divergence), A3d (FAN-01 attempt 3 does not trip),
//! A4 (bit identity on/off), A5 (a diverged design-loop candidate is a failed evaluation with its reason), A6 (`off`). Limits are the registered ones.

mod w35b_common;

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseEvaluator, RunLimits};
use rcfd_api::design::store::Store;
use rcfd_api::design::study::{Doe, Method, ObjectiveDef, ParamDef, Robust, Sense, Study, StudyDef, Target};
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::Value;
use std::sync::Mutex;
use std::time::Instant;

/// Drive `text` through `runctl::drive`; (outcome, the finish event).
fn drive(text: &str, max_steps: Option<usize>) -> (runctl::RunOutcome, Value) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let mut finish = Value::Null;
    let opts = RunOptions {
        max_steps,
        ..RunOptions::default()
    };
    let out = runctl::drive(&sim, &opts, &|| false, &mut |e| {
        if e["event"] == "finish" {
            finish = e.clone();
        }
    });
    (out, finish)
}

fn a2(name: &str, text: &str) {
    let t0 = Instant::now();
    let (out, fin) = drive(text, None);
    let secs = t0.elapsed().as_secs_f64();
    println!(
        "{name}: {} after {} iterations, {secs:.1} s: {}",
        out.state, out.iterations, out.reason
    );
    println!("{name}: divergence {}", fin["divergence"]);
    assert_eq!(out.state, "diverged", "{name}: {out:?}");
    assert_eq!(out.exit_code, exit::DIVERGED);
    assert!(
        out.reason.contains("steady divergence guard") && out.reason.contains("last good iteration"),
        "{}",
        out.reason
    );
    let d = &fin["divergence"];
    assert_eq!(d["solver"], "steady", "{d}");
    assert!(d["kind"].is_string(), "{d}");
    let it = d["iteration"].as_u64().expect("divergence.iteration");
    assert!(it <= 15, "{name}: tripped at iteration {it} (limit 15)");
    assert!(secs < 60.0, "{name}: {secs} s (limit 60 s)");
}

#[test]
fn a2a_fan01_laminar_linear_upwind_is_stopped() {
    a2("A2a", &w35b_common::laminar("linear_upwind"));
}

#[test]
fn a2b_fan01_sst_linear_upwind_is_stopped() {
    a2("A2b", &w35b_common::sst("linear_upwind"));
}

/// A3d: the FAN-01 attempt-3 set-up (SST, upwind) does not trip in its first 1000 iterations (heavy: about 2 min).
#[test]
#[ignore = "heavy (FAN-01, 1000 iterations); ADR 0077 A3d"]
fn a3d_fan01_attempt3_does_not_trip() {
    let case = Case::parse_toml(&w35b_common::sst("upwind")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    for _ in 0..1000 {
        sim.step();
        if sim.is_done() {
            break;
        }
    }
    let g = sim.steady_guard().unwrap();
    println!(
        "A3d: {} iterations, failure {:?}, peak ratios K2 {:.3e} K3 {:.3e} K4 {:.3e}",
        sim.iteration(),
        sim.failure(),
        g.peak[0],
        g.peak[1],
        g.peak[2]
    );
    assert!(!sim.diverged() && sim.failure().is_none(), "{:?}", sim.failure());
}

const CAVITY: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simple\"\nconvection = \"linear_upwind\"\nmax_iterations = 60\n";

#[test]
fn a4_guard_on_and_off_are_bit_identical() {
    let run = |extra: &str| {
        let case = Case::parse_toml(&format!("{CAVITY}{extra}")).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        for _ in 0..60 {
            sim.step();
        }
        println!(
            "A4 ({}): trip {:?}",
            if extra.is_empty() { "on" } else { "off" },
            sim.steady_divergence().map(|d| (d.iteration, d.kind))
        );
        let bits = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<_>>();
        (
            bits(sim.field("ux").unwrap()),
            bits(sim.field("uy").unwrap()),
            bits(sim.field("p").unwrap()),
            bits(sim.flux().to_vec()),
        )
    };
    assert!(
        run("") == run("steady_divergence_guard = \"off\"\n"),
        "A4: fields differ"
    );
}

/// The S2 cavity of ADR 0077: 12 x 12, nu 0.01, SIMPLE, upwind, cap 3000, tolerances 1e-6.
const S2: &str = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simple\"\nconvection = \"upwind\"\nmax_iterations = 3000\ntolerance_u = 1e-6\ntolerance_p = 1e-6\n[reports.p_walls]\ntype = \"area_average\"\npatch = \"walls\"\nfield = \"p\"\n";

/// A5 study over the S2 cavity: one parameter x in [lo, hi] with the targets `alpha_u = su x + ou`, `alpha_p = sp x + op`, two levels;
/// checks the registered A5 limits with `x_stable` and `x_diverged` the two levels.
fn a5(lo: f64, hi: f64, (su, ou): (f64, f64), (sp, op): (f64, f64)) {
    let base = Case::parse_toml(S2).unwrap();
    let ev = CaseEvaluator::new(base, RunLimits::default(), false);
    let target = |path: &str, scale: f64, offset: f64| Target {
        path: path.into(),
        scale,
        offset,
    };
    let def = StudyDef {
        name: "relaxation".into(),
        params: vec![ParamDef {
            name: "x".into(),
            lo,
            hi,
            init: None,
            targets: vec![target("solver.alpha_u", su, ou), target("solver.alpha_p", sp, op)],
        }],
        conditions: vec![rcfd_api::design::study::Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: vec![ObjectiveDef {
            metric: "p_walls".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers: 1,
    };
    let dir = std::env::temp_dir().join(format!("rcfd_w35b_a5_{}_{lo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut study = Study::new(def, Store::open(&dir.join("evaluations.jsonl"), "a5").unwrap(), &ev).unwrap();
    let t0 = Instant::now();
    study.run(&Method::Doe(Doe::Factorial { levels: vec![2] })).unwrap();
    println!("A5: study of 2 candidates in {:.2} s", t0.elapsed().as_secs_f64());
    let recs = study.store.records().to_vec();
    assert_eq!(recs.len(), 2);
    let find = |x: f64| {
        recs.iter()
            .find(|r| (study.x_of(r)[0] - x).abs() < 1e-12)
            .expect("record")
            .clone()
    };
    let (good, bad) = (find(lo), find(hi));
    let c = &bad.conditions[0];
    println!(
        "A5: x = {hi}: status {}, ok {}, {} iterations, message: {}",
        c.status, c.ok, c.iterations, c.message
    );
    assert_eq!(c.status, "diverged");
    assert!(!c.ok && !bad.ok());
    assert!(
        c.message.contains("steady divergence guard") && c.message.contains("last good iteration"),
        "{}",
        c.message
    );
    assert!(c.iterations <= 10, "A5: {} iterations (limit 10)", c.iterations);
    assert!(!study.fitness_of(&bad).feasible() && study.fitness_of(&bad).f.is_infinite());
    let g = &good.conditions[0];
    println!("A5: x = {lo}: status {}, {} iterations", g.status, g.iterations);
    assert_eq!(g.status, "converged", "{}", g.message);
    let best = study.best().expect("a best record");
    assert!((study.x_of(&study.store.records()[best])[0] - lo).abs() < 1e-12);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The registered A5 set-up: `alpha_p = (7/3) x - 4/3` is 1.0000000000000002 at x = 1 (outside the schema range): set-up defect (amendment 1).
#[test]
#[ignore = "set-up defect of ADR 0077 A5 (alpha_p rounds above 1.0); corrected test a5_corrected_*; limits unchanged"]
fn a5_registered_setup_defect() {
    a5(0.7, 1.0, (1.0, 0.0), (7.0 / 3.0, -4.0 / 3.0));
}

/// Corrected A5 (amendment 1): x in [0, 1], alpha_u = 0.25 x + 0.75, alpha_p = 0.75 x + 0.25 (exactly 1.0 at x = 1).
#[test]
fn a5_corrected_diverged_candidate_is_a_failed_evaluation() {
    a5(0.0, 1.0, (0.25, 0.75), (0.75, 0.25));
}

#[test]
fn a6_off_lets_the_finite_runaway_run_to_its_cap() {
    let solver = |keys: &str| S2.replace("tolerance_p = 1e-6\n", &format!("tolerance_p = 1e-6\n{keys}"));
    let (out, _) = drive(
        &solver("alpha_u = 1.0\nalpha_p = 1.0\nsteady_divergence_guard = \"off\"\n"),
        None,
    );
    println!("A6: {} after {} iterations: {}", out.state, out.iterations, out.reason);
    assert_eq!(out.state, "not_converged", "{out:?}");
    assert_eq!(out.iterations, 3000);
    let e = Case::parse_toml(&solver("steady_divergence_guard = \"maybe\"\n"))
        .map(|_| ())
        .unwrap_err();
    assert!(e.iter().any(|d| d.path == "solver.steady_divergence_guard"), "{e:?}");
}

/// S-d (amendment 3): a guard trip on a `linear_upwind` run carries the safe-scheme note of ADR 0077 part B.
#[test]
fn s_d_guard_reason_carries_the_scheme_note() {
    let (out, _) = drive(&w35b_common::laminar("linear_upwind"), None);
    assert_eq!(out.state, "diverged");
    assert!(
        out.reason.contains("\"upwind\" and \"tvd_minmod\" were stable"),
        "{}",
        out.reason
    );
}

/// SV-1 (amendment 3): the service background run (`run.start`) ends a steady run whose guard tripped, with the located message.
#[test]
fn sv1_service_background_run_ends_on_a_guard_trip() {
    use rcfd_api::service::{Client, Service, ServiceConfig};
    let svc = Service::new(ServiceConfig::default());
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    let mut c = Client::connect(server.addr).unwrap();
    let text = S2.replace(
        "tolerance_p = 1e-6\n",
        "tolerance_p = 1e-6\nalpha_u = 1.0\nalpha_p = 1.0\n",
    );
    let id = c.call("session.create", serde_json::json!({"toml": text})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let t0 = Instant::now();
    c.call("run.start", serde_json::json!({"session": id})).unwrap();
    let mut st = Value::Null;
    while t0.elapsed().as_secs_f64() < 30.0 {
        st = c
            .call("run.wait", serde_json::json!({"session": id, "timeout_ms": 2000}))
            .unwrap();
        if st["state"] != "running" {
            break;
        }
    }
    println!("SV-1: {st} after {:.2} s", t0.elapsed().as_secs_f64());
    assert_eq!(st["state"], "failed", "{st}");
    assert!(
        st["error"].as_str().unwrap_or("").contains("steady divergence guard"),
        "{st}"
    );
}
