//! ADR 0081 part P: the default one-corrector pressure-lag warning (ADR 0077 part C) against set-ups whose stability is known from the ADR 0069
//! harness and the ADR 0073 B3 predictions, with the ground truth re-measured here (the check off, stepped to t_end = 10, the ADR 0069 criteria).
//! Checks P-a (ground truth equals the earlier classification) and P-b (warned iff unstable). Limits are the registered ones.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use serde_json::Value;
use std::sync::Mutex;

/// (id, n_outer, time scheme, Co_t, earlier classification STABLE?).
const SETUPS: [(&str, usize, &str, f64, bool); 6] = [
    ("P1", 2, "euler", 1.0, true),
    ("P2", 2, "euler", 2.3, true),
    ("P3", 4, "bdf2", 6.0, true),
    ("P4", 2, "euler", 3.0, false),
    ("P5", 4, "euler", 5.0, false),
    ("P6", 4, "bdf2", 7.5, false),
];

/// Tree form of ADR 0077 C2: cavity 32 x 32, nu 0.01, lid 1, linear_upwind, PIMPLE with one corrector, alpha 0.8/0.5, dt = Co_t / 32.
fn text(n_outer: usize, scheme: &str, co_t: f64, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 32\nny = 32\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nconvection = \"linear_upwind\"\nalgorithm = \"pimple\"\nn_outer = {n_outer}\nn_correctors = 1\nalpha_u = 0.8\nalpha_p = 0.5\ntime_scheme = \"{scheme}\"\ndt = {:?}\nend_time = 10.0\n{extra}",
        co_t / 32.0
    )
}

/// The ADR 0069 classification of a run with the check off, stepped to t_end = 10: (stable, summary).
fn ground_truth(n_outer: usize, scheme: &str, co_t: f64) -> (bool, String) {
    let case = Case::parse_toml(&text(n_outer, scheme, co_t, "pressure_lag_check = \"off\"\n")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let (mut umax, mut pmax, mut cmax) = (0.0f64, 0.0f64, 0.0f64);
    let mut first_violation = None;
    let mut steps = 0;
    while !sim.is_done() && steps < 100_000 {
        let s = sim.step();
        steps += 1;
        let u = sim
            .velocity()
            .values
            .iter()
            .map(|v| vec3::norm(*v))
            .fold(0.0f64, |a, x| if x.is_nan() { f64::INFINITY } else { a.max(x) });
        let p = sim
            .field("p")
            .unwrap()
            .iter()
            .fold(0.0f64, |a, x| if x.is_nan() { f64::INFINITY } else { a.max(x.abs()) });
        let c = if s.continuity.is_nan() {
            f64::INFINITY
        } else {
            s.continuity
        };
        umax = umax.max(u);
        pmax = pmax.max(p);
        cmax = cmax.max(c);
        if first_violation.is_none() && (u > 1.5 || p > 50.0 || c > 1e-3 || sim.diverged()) {
            first_violation = Some((steps, s.time));
        }
        if sim.failure().is_some() {
            break;
        }
    }
    let reached = sim.failure().is_none() && sim.time() >= 10.0 - 1e-9;
    let finite = umax.is_finite() && pmax.is_finite();
    let stable = !sim.diverged() && finite && umax <= 1.5 && pmax <= 50.0 && cmax <= 1e-3 && reached;
    (
        stable,
        format!(
            "{steps} steps to t = {:.4}, max|u| {umax:.4e}, max|p| {pmax:.4e}, max continuity {cmax:.3e}, failure {:?}, first violation {first_violation:?}",
            sim.time(),
            sim.failure()
        ),
    )
}

/// `pressure_lag` warnings of the default key over 3 driven steps.
fn warnings(n_outer: usize, scheme: &str, co_t: f64) -> Vec<Value> {
    let case = Case::parse_toml(&text(n_outer, scheme, co_t, "")).unwrap();
    let sim = Mutex::new(Simulation::new(&case).unwrap());
    let mut warns = Vec::new();
    let opts = RunOptions {
        max_steps: Some(3),
        ..RunOptions::default()
    };
    runctl::drive(&sim, &opts, &|| false, &mut |e| {
        if e["event"] == "warning" && e["kind"] == "pressure_lag" {
            warns.push(e.clone());
        }
    });
    warns
}

#[test]
fn p_one_corrector_warning_iff_unstable() {
    let mut fails = Vec::new();
    for (id, n_outer, scheme, co_t, earlier) in SETUPS {
        let (stable, why) = ground_truth(n_outer, scheme, co_t);
        let w = warnings(n_outer, scheme, co_t);
        let rho: Vec<String> = w
            .iter()
            .map(|e| format!("step {} rho {}", e["step"], e["rho"]))
            .collect();
        println!(
            "{id} PIMPLE-{n_outer} {scheme} Co_t {co_t}: ground truth {} (earlier {}); {why}; warnings {rho:?}",
            if stable { "STABLE" } else { "UNSTABLE" },
            if earlier { "STABLE" } else { "UNSTABLE" }
        );
        if stable != earlier {
            fails.push(format!(
                "P-a {id}: ground truth differs from the earlier classification"
            ));
        }
        let step = if scheme == "euler" { 1 } else { 2 };
        let ok = if stable {
            w.is_empty()
        } else {
            w.len() == 1 && w[0]["step"] == step
        };
        if !ok {
            fails.push(format!(
                "P-b {id}: {} warnings {rho:?} for a {} run",
                w.len(),
                if stable { "STABLE" } else { "UNSTABLE" }
            ));
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
}
