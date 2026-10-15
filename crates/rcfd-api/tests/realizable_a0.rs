//! ADR 0017 P6: `turbulence.realizable_a0` through the settings tree (default tests).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};

/// Periodic channel Re_tau = 395, 4 x 14 cells, Realizable.
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
variant = "realizable"
"#;

fn case(edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(CHANNEL).unwrap();
    for (p, v) in edits {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    c
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

fn nut_after(c: &Case, n: usize) -> Vec<f64> {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    s.field("nut").unwrap()
}

const PATH: &str = "turbulence.realizable_a0";

#[test]
fn p6_values_and_located_errors() {
    assert!(diags(&case(&[])).is_empty());
    assert!(diags(&case(&[(PATH, json!(4.0))])).is_empty());
    assert!(diags(&case(&[(PATH, json!(4.04))])).is_empty());
    for bad in [2.9, 5.1] {
        let d = diags(&case(&[(PATH, json!(bad))]));
        assert!(d.iter().any(|e| e.0 == PATH), "{bad}: {d:?}");
    }
    for variant in ["standard", "rng"] {
        let d = diags(&case(&[("turbulence.variant", json!(variant)), (PATH, json!(4.0))]));
        assert!(d.iter().any(|e| e.0 == PATH), "{variant}: {d:?}");
    }
    let d = diags(&case(&[("turbulence.model", json!("k_omega_sst")), (PATH, json!(4.0))]));
    assert!(d.iter().any(|e| e.0 == PATH), "{d:?}");
}

#[test]
fn p6_key_is_applied_and_default_is_bit_identical() {
    let base = nut_after(&case(&[]), 40);
    let explicit = nut_after(&case(&[(PATH, json!(4.04))]), 40);
    let orig = nut_after(&case(&[(PATH, json!(4.0))]), 40);
    assert_eq!(base, explicit, "A0 = 4.04 must reproduce the default bit for bit");
    assert!(
        base != orig,
        "A0 = 4.0 left the eddy viscosity unchanged: the key is not applied"
    );
    assert!(orig.iter().all(|v| v.is_finite()));
}
