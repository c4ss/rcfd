//! F-238 case-tree report kinds (ADR 0048 D5): volume_std, area_std, area_min/max, mass_weighted_average, vector_flux.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::json;

fn case(extra: &[(&str, serde_json::Value)]) -> Case {
    let text = r#"
[mesh]
generator = "channel_2d"
nx = 10
ny = 8
lx = 2.0
ly = 0.5
depth = 0.2
[physics]
nu = 0.01
rho = 2.0
[initial]
velocity = [1.5, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.5, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
"#;
    let mut c = Case::parse_toml(text).unwrap();
    for (n, d) in extra {
        c.tree.add_named("reports", n).unwrap();
        for (k, v) in d.as_object().unwrap() {
            c.tree.set(&format!("reports.{n}.{k}"), v.clone()).unwrap();
        }
    }
    c
}

#[test]
fn new_report_kinds_on_a_uniform_flow() {
    let c = case(&[
        ("sd", json!({"type": "volume_std", "field": "ux"})),
        ("sd_in", json!({"type": "area_std", "patch": "inlet", "field": "ux"})),
        ("lo", json!({"type": "area_min", "patch": "inlet", "field": "ux"})),
        ("hi", json!({"type": "area_max", "patch": "outlet", "field": "ux"})),
        (
            "mw",
            json!({"type": "mass_weighted_average", "patch": "outlet", "field": "ux"}),
        ),
        (
            "mw_in",
            json!({"type": "mass_weighted_average", "patch": "inlet", "field": "ux"}),
        ),
        (
            "vf",
            json!({"type": "vector_flux", "patch": "outlet", "direction": [1.0, 0.0, 0.0]}),
        ),
        (
            "vf_in",
            json!({"type": "vector_flux", "patch": "inlet", "direction": [2.0, 0.0, 0.0]}),
        ),
        ("m_out", json!({"type": "mass_flow", "patch": "outlet"})),
    ]);
    let sim = Simulation::new(&c).unwrap();
    let get = |n: &str| {
        sim.report_values()
            .into_iter()
            .find(|(k, _)| k == n)
            .unwrap()
            .1
            .unwrap()
    };
    let q = 1.5 * 0.5 * 0.2;
    eprintln!("m_out {} vf {}", get("m_out").value, get("vf").value);
    assert!(
        get("m_out").value > 0.1,
        "flux must be non-zero for the check to mean something"
    );
    assert!(get("sd").value.abs() <= 1e-14);
    assert!(get("sd_in").value.abs() <= 1e-14);
    assert!((get("lo").value - 1.5).abs() <= 1e-14 && (get("hi").value - 1.5).abs() <= 1e-14);
    assert!((get("mw").value - 1.5).abs() <= 1e-12 && (get("mw_in").value - 1.5).abs() <= 1e-12);
    assert!((get("vf").value - 2.0 * q * 1.5).abs() <= 1e-12);
    assert!((get("vf_in").value + 2.0 * q * 1.5).abs() <= 1e-12);
    assert_eq!(get("vf").unit, "N");
}

#[test]
fn unknown_patch_is_a_located_error() {
    let c = case(&[("bad", json!({"type": "area_std", "patch": "nowhere", "field": "ux"}))]);
    let e = Simulation::new(&c).err().expect("error");
    let text = format!("{e:?}");
    assert!(text.contains("reports.bad.patch") && text.contains("nowhere"), "{text}");
}
