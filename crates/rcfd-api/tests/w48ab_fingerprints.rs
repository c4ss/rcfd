//! ADR 0162 (job 48AB) N4: fingerprints of fixed-dt adaptive runs and of default runs, printed so that a `git archive` copy of
//! 444255c (before this job) and this branch can be compared. Uses only APIs that exist at 444255c, so the file builds there
//! unchanged. `cargo test --release -p rcfd-api --test w48ab_fingerprints -- --ignored --nocapture`

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};

fn case_with(overrides: &[(&str, Value)]) -> Case {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    let mut case = Case::parse_toml(&std::fs::read_to_string(path).expect("example case readable"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    for (k, v) in overrides {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x100000001b3);
    }
}

/// FNV-1a of the bits of ux, uy, p and the face flux, and of positions 0-4 of every corrector-log entry.
fn fingerprint(label: &str, overrides: &[(&str, Value)]) {
    let mut sim = Simulation::new(&case_with(overrides)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
    }
    let mut h = 0xcbf29ce484222325u64;
    for f in ["ux", "uy", "p"] {
        for x in sim.field(f).unwrap() {
            fnv(&mut h, &x.to_bits().to_le_bytes());
        }
    }
    for x in sim.flux() {
        fnv(&mut h, &x.to_bits().to_le_bytes());
    }
    let log: Vec<Value> = sim
        .corrector_summary()
        .and_then(|s| s["log"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|e| json!([e[0], e[1], e[2], e[3], e[4]]))
        .collect();
    let log_text = serde_json::to_string(&log).unwrap();
    fnv(&mut h, log_text.as_bytes());
    let summary = sim.corrector_summary().map_or("fixed".to_string(), |s| {
        format!("mean {} max {} estimates {}", s["mean"], s["max"], s["estimates"])
    });
    println!(
        "FINGERPRINT {label}: {h:016x} ({steps} steps, t = {}, diverged {}, correctors {summary}, log {log_text})",
        sim.time(),
        sim.diverged()
    );
}

fn adaptive() -> (&'static str, Value) {
    ("solver.corrector_control", json!("adaptive"))
}

/// The Studio plan's time entries for N = 3 on the shipped 64 x 48 mesh (ADR 0153: dt0 0.00602).
fn studio_time() -> Vec<(&'static str, Value)> {
    vec![
        ("solver.end_time", json!(3.0)),
        ("solver.dt", json!(0.00602)),
        ("solver.max_courant", json!(0.5)),
        ("solver.max_dt", json!(0.03)),
    ]
}

#[test]
#[ignore = "ADR 0162 N4 printer (heavy: a 64 x 256 run to t = 3)"]
fn w48ab_n4_fingerprints() {
    let only = std::env::var("W48AB_N4").unwrap_or_default();
    let want = |l: &str| only.is_empty() || only.split(',').any(|x| l.starts_with(x));
    if want("F1") {
        fingerprint(
            "F1 adaptive 64x48 dt 1/16 t 10",
            &[adaptive(), ("solver.end_time", json!(10.0))],
        );
    }
    if want("F2") {
        fingerprint(
            "F2 adaptive 64x256 dt 1/64 t 3 (K1)",
            &[
                adaptive(),
                ("mesh.nx", json!(64)),
                ("mesh.ny", json!(256)),
                ("solver.dt", json!(1.0 / 64.0)),
                ("solver.n_correctors", json!(2)),
                ("solver.end_time", json!(3.0)),
            ],
        );
    }
    if want("F3") {
        fingerprint(
            "F3 adaptive 64x48 dt 1/16 end 1.03 (short last step)",
            &[adaptive(), ("solver.end_time", json!(1.03))],
        );
    }
    if want("F4") {
        fingerprint("F4 fixed 64x48 dt 1/16 t 3", &[("solver.end_time", json!(3.0))]);
    }
    if want("F5") {
        fingerprint("F5 fixed 64x48 Studio time entries t 3", &studio_time());
    }
    if want("F6") {
        // expected to differ after ADR 0162 (the step grows threefold under the controller): shows the trigger acts
        let mut v = studio_time();
        v.push(adaptive());
        fingerprint("F6 adaptive 64x48 Studio time entries t 3 (expected to differ)", &v);
    }
}
