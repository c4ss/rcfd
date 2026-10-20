//! ADR 0081 part G: the steady divergence guard trips on the coupled solver (`solver.algorithm = "coupled"`) through the run-control
//! driver. No finite runaway of the coupled solver was found (ADR 0081 SM-G), so both tests reach a non-finite state (K1): G1 from a
//! non-finite initial field, G2 the inviscid cavity. Limits are the registered ones.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::Value;
use std::sync::Mutex;

fn drive(text: &str) -> (runctl::RunOutcome, Value) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let mut finish = Value::Null;
    let opts = RunOptions {
        max_steps: Some(50),
        ..RunOptions::default()
    };
    let out = runctl::drive(&sim, &opts, &|| false, &mut |e| {
        if e["event"] == "finish" {
            finish = e.clone();
        }
    });
    (out, finish)
}

fn coupled_cavity(n: usize, nu: f64, conv: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = {n}\nny = {n}\n[physics]\nnu = {nu:?}\n{extra}[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"coupled\"\nconvection = \"{conv}\"\nmax_iterations = 50\n"
    )
}

fn check(name: &str, out: &runctl::RunOutcome, fin: &Value) -> Value {
    println!(
        "{name}: {} after {} iterations: {}",
        out.state, out.iterations, out.reason
    );
    println!("{name}: divergence {}", fin["divergence"]);
    assert_eq!(out.state, "diverged", "{name}: {out:?}");
    assert_eq!(out.exit_code, exit::DIVERGED);
    assert!(out.reason.contains("steady divergence guard"), "{}", out.reason);
    let d = fin["divergence"].clone();
    assert_eq!(d["solver"], "steady", "{d}");
    assert_eq!(d["kind"], "non_finite", "{d}");
    assert_eq!(d["iteration"], 1, "{d}");
    d
}

#[test]
fn g1_coupled_non_finite_initial_field_trips_the_guard() {
    let (out, fin) = drive(&coupled_cavity(
        12,
        0.01,
        "upwind",
        "[initial]\nvelocity_expression_x = \"1 [m^2/s] / (x - x)\"\n",
    ));
    let d = check("G1", &out, &fin);
    assert_eq!(d["quantity"], "velocity", "{d}");
    assert!(d["cell"].is_u64(), "{d}");
}

#[test]
fn g2_coupled_inviscid_cavity_trips_the_guard() {
    let (out, fin) = drive(&coupled_cavity(32, 0.0, "linear_upwind", ""));
    check("G2", &out, &fin);
}
