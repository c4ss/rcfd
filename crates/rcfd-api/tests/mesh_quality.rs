//! Quality report surfaced through the service (F-233, ADR 0018 check Q5).

use rcfd_api::service::{Service, ServiceConfig};
use serde_json::json;

const CASE: &str = r#"
title = "quality"
[mesh]
generator = "box_2d"
nx = 12
ny = 6
lx = 2.0
ly = 0.002
[physics]
nu = 0.01
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
"#;

#[test]
fn mesh_check_returns_old_keys_and_extended_measures() {
    let svc = Service::new(ServiceConfig::default());
    let sid = svc.call("session.create", &json!({"toml": CASE})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc.call("mesh.check", &json!({"session": sid})).unwrap();
    assert_eq!(r["ok"], true);
    let q = &r["report"];
    assert_eq!(q["n_cells"], 72);
    for k in [
        "max_non_orthogonality",
        "max_skewness",
        "max_aspect_ratio",
        "max_volume_ratio",
        "total_volume",
    ] {
        assert!(q[k].is_number(), "old key {k} missing: {q}");
    }
    assert!(q["warnings"].is_array() && q["errors"].as_array().unwrap().is_empty());
    let m = &q["measures"];
    assert!(m["max_warp"].as_f64().unwrap() < 1e-9);
    assert!((m["min_face_angle"].as_f64().unwrap() - 90.0).abs() < 1e-9);
    assert!((m["max_face_angle"].as_f64().unwrap() - 90.0).abs() < 1e-9);
    assert!(m["max_inverse_orthogonal"].as_f64().unwrap() < 1e-9);
    // cells are 1/6 x 1/3000: face edge-length ratio 500, above the default limit of 100
    assert!(m["max_edge_length_ratio"].as_f64().unwrap() > 100.0, "{m}");
    assert!(q["poor_counts"]["edge_length_ratio"].as_u64().unwrap() > 0, "{q}");
    assert!(!q["poor_list"].as_array().unwrap().is_empty());
    assert!(!q["poor_warnings"].as_array().unwrap().is_empty());
}
