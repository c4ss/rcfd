//! ADR 0077 part C: the pressure-lag check of ADR 0073 B4 through the API (`solver.pressure_lag_check`, default `warn`).
//! Checks C1b, C1c (bit identity warn/off), C2 (warning event), C3 (raise), C5 (cost, reported), C6 (BDF2 handling). Limits are the registered ones.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::Value;
use std::sync::Mutex;
use std::time::Instant;

/// Lid cavity `n` x `n`, nu 0.01, `linear_upwind`, with the given solver lines.
fn cavity(n: usize, solver: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = {n}\nny = {n}\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nconvection = \"linear_upwind\"\n{solver}"
    )
}

/// Tree form of ADR 0073 B4d / B3: 32 x 32, PIMPLE-2 with one corrector (alpha_u 0.8, alpha_p 0.5), Courant number `co_t` (U = 1, h = 1/32).
fn pimple2(scheme: &str, co_t: f64, extra: &str) -> String {
    cavity(
        32,
        &format!(
            "algorithm = \"pimple\"\nn_outer = 2\nn_correctors = 1\nalpha_u = 0.8\nalpha_p = 0.5\ntime_scheme = \"{scheme}\"\ndt = {:?}\nend_time = 100.0\n{extra}",
            co_t / 32.0
        ),
    )
}

/// Drive `steps` steps; (outcome, warning events of kind pressure_lag, finish event).
fn drive(text: &str, steps: usize) -> (runctl::RunOutcome, Vec<Value>, Value, Simulation) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let (mut warns, mut fin) = (Vec::new(), Value::Null);
    let opts = RunOptions {
        max_steps: Some(steps),
        ..RunOptions::default()
    };
    let out = runctl::drive(&sim, &opts, &|| false, &mut |e| {
        if e["event"] == "warning" && e["kind"] == "pressure_lag" {
            warns.push(e.clone());
        }
        if e["event"] == "finish" {
            fin = e.clone();
        }
    });
    (out, warns, fin, sim.into_inner().unwrap())
}

fn bits(sim: &Simulation) -> Vec<u64> {
    let mut v = Vec::new();
    for f in ["ux", "uy", "p"] {
        v.extend(sim.field(f).unwrap().into_iter().map(f64::to_bits));
    }
    v.extend(sim.flux().iter().map(|x| x.to_bits()));
    v
}

fn stepped(text: &str, n: usize) -> Simulation {
    let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
    for _ in 0..n {
        sim.step();
    }
    sim
}

#[test]
fn c1b_warn_and_off_are_bit_identical_with_amg_pressure() {
    let piso = |key: &str| cavity(48, &format!("algorithm = \"piso\"\ndt = 0.01\nend_time = 1.0\n{key}"));
    let (on, off) = (
        stepped(&piso(""), 20),
        stepped(&piso("pressure_lag_check = \"off\"\n"), 20),
    );
    assert_eq!(on.geom.n_cells(), 2304);
    assert!(bits(&on) == bits(&off), "C1b: fields differ");
}

#[test]
fn c1c_warn_and_off_are_bit_identical_on_the_unstable_setup() {
    let (on, off) = (
        stepped(&pimple2("euler", 4.0, ""), 3),
        stepped(&pimple2("euler", 4.0, "pressure_lag_check = \"off\"\n"), 3),
    );
    assert!(bits(&on) == bits(&off), "C1c: fields differ");
}

#[test]
fn c2_default_warns_once_at_step_1() {
    let (out, warns, _, _) = drive(&pimple2("euler", 4.0, ""), 3);
    for w in &warns {
        println!("C2: {}", w["message"]);
    }
    assert_eq!(warns.len(), 1, "C2: {warns:?}");
    let w = &warns[0];
    let rho = w["rho"].as_f64().unwrap();
    assert_eq!(w["step"], 1);
    assert!(rho >= 1.0);
    assert!(w["message"].as_str().unwrap().contains("solver.n_correctors >= 2"));
    assert!(
        (rho / 2.251 - 1.0).abs() <= 0.10,
        "C2: rho {rho} against the fv-level 2.251"
    );
    assert_ne!(out.state, "diverged", "warn must not stop the run: {out:?}");
}

#[test]
fn c3_raise_stops_at_step_1() {
    let (out, _, fin, sim) = drive(&pimple2("euler", 4.0, "pressure_lag_check = \"raise\"\n"), 3);
    println!("C3: {} / {}", out.reason, fin["divergence"]);
    assert_eq!(out.state, "diverged", "{out:?}");
    assert_eq!(out.exit_code, exit::DIVERGED);
    assert_eq!(out.iterations, 1);
    assert!(out.reason.contains("pressure-lag check"), "{}", out.reason);
    assert_eq!(fin["divergence"]["kind"], "lag_amplification");
    for f in ["ux", "uy", "p"] {
        assert!(
            sim.field(f).unwrap().iter().all(|x| x.is_finite()),
            "C3: {f} not finite"
        );
    }
}

#[test]
fn c5_cost_of_one_estimate_is_reported() {
    let piso = |key: &str| cavity(48, &format!("algorithm = \"piso\"\ndt = 0.01\nend_time = 1.0\n{key}"));
    let time = |text: &str| {
        let mut sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
        let t0 = Instant::now();
        sim.step();
        let t1 = t0.elapsed().as_secs_f64();
        let t2 = Instant::now();
        sim.step();
        (t1, t2.elapsed().as_secs_f64())
    };
    let (a1, a2) = time(&piso(""));
    let (b1, b2) = time(&piso("pressure_lag_check = \"off\"\n"));
    println!("C5: step 1 with the estimate {a1:.4} s, without {b1:.4} s; step 2 {a2:.4} / {b2:.4} s");
}

#[test]
fn c6_bdf2_estimates_at_step_2() {
    let (_, stable, _, _) = drive(&pimple2("bdf2", 3.4, ""), 3);
    let (_, unstable, _, _) = drive(&pimple2("bdf2", 4.6, ""), 3);
    for w in stable.iter().chain(&unstable) {
        println!("C6: {}", w["message"]);
    }
    assert!(stable.is_empty(), "C6: Co_t 3.4 warned: {stable:?}");
    assert_eq!(unstable.len(), 1, "C6: {unstable:?}");
    assert_eq!(unstable[0]["step"], 2);
}
