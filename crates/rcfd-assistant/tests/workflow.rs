//! F-148: setup-workflow analysis on synthetic data and on a real service.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_assistant::workflow::*;
use rcfd_assistant::{ApprovalMode, Toolbox};
use serde_json::{json, Value};

fn decay(n: usize, rate: f64) -> Vec<f64> {
    (0..n).map(|i| (-(i as f64) * rate).exp()).collect()
}

fn verdict(series: Vec<(&str, Vec<f64>)>, settings: Value) -> Value {
    let s: Vec<(String, Vec<f64>)> = series.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    explain_residuals(&s, &settings)
}

fn settings() -> Value {
    json!({"solver": {"algorithm": "simple", "convection": "linear_upwind", "alpha_u": 0.9, "alpha_p": 1.0,
                      "tolerance_u": 1e-6, "tolerance_p": 1e-6, "non_orthogonal_correctors": 0}})
}

fn pat<'a>(v: &'a Value, series: &str) -> &'a str {
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["series"] == series)
        .unwrap()["pattern"]
        .as_str()
        .unwrap()
}

#[test]
fn growth_is_diverging_with_robustness_changes() {
    let y: Vec<f64> = (0..60).map(|i| 0.1 * (1.15f64).powi(i)).collect();
    let v = verdict(vec![("res_u", y.clone()), ("res_p", y)], settings());
    assert_eq!(v["verdict"], "diverging");
    assert_eq!(pat(&v, "res_u"), "diverging");
    let paths: Vec<&str> = v["suggested_changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert!(
        paths.contains(&"solver.alpha_u") && paths.contains(&"solver.alpha_p") && paths.contains(&"solver.convection"),
        "{paths:?}"
    );
    let au = v["suggested_changes"][0]["value"].as_f64().unwrap();
    assert!((0.05..0.9).contains(&au));
}

#[test]
fn nan_is_reported_with_its_location() {
    let mut y = decay(30, 0.2);
    y[20] = f64::NAN;
    let v = verdict(vec![("res_u", y)], settings());
    assert_eq!(pat(&v, "res_u"), "non_finite");
    assert!(v["verdict"].as_str().unwrap().contains("NaN"));
    assert!(v["findings"][0]["evidence"].as_str().unwrap().contains("sample 20"));
}

#[test]
fn oscillation_is_detected() {
    let y: Vec<f64> = (0..80).map(|i| if i % 2 == 0 { 1e-2 } else { 5e-2 }).collect();
    let v = verdict(vec![("res_u", y)], settings());
    assert_eq!(pat(&v, "res_u"), "oscillating");
    assert_eq!(v["verdict"], "oscillating");
}

#[test]
fn stagnation_above_tolerance_is_detected() {
    let mut y = decay(30, 0.3);
    y.extend(std::iter::repeat_n(3e-4, 70));
    let v = verdict(vec![("res_u", y)], settings());
    assert_eq!(pat(&v, "res_u"), "stagnating");
    assert!(v["suggested_changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["path"] == "solver.non_orthogonal_correctors"));
}

#[test]
fn healthy_and_converged_runs_get_no_changes() {
    let v = verdict(vec![("res_u", decay(60, 0.1)), ("res_p", decay(60, 0.1))], settings());
    assert_eq!(v["verdict"], "converging");
    assert!(v["suggested_changes"].as_array().unwrap().is_empty());
    let v = verdict(vec![("res_u", decay(200, 0.2)), ("res_p", decay(200, 0.2))], settings());
    assert_eq!(v["verdict"], "converged");
    let v = verdict(vec![("res_u", vec![1.0, 0.5])], settings());
    assert_eq!(v["verdict"], "not enough data");
}

fn check(report: Value) -> Value {
    json!({"ok": true, "report": report, "patches": []})
}

#[test]
fn mesh_assessment_thresholds() {
    let good = assess_mesh(&check(
        json!({"n_cells": 64, "max_non_orthogonality": 5.0, "max_skewness": 0.2, "max_aspect_ratio": 3.0, "max_volume_ratio": 1.5, "errors": [], "warnings": []}),
    ));
    assert_eq!(good["verdict"], "good");
    let care = assess_mesh(&check(
        json!({"n_cells": 64, "max_non_orthogonality": 55.0, "max_skewness": 0.2, "max_aspect_ratio": 3.0, "errors": [], "warnings": []}),
    ));
    assert_eq!(care["verdict"], "usable with care");
    assert!(care["findings"][0]["advice"]
        .as_str()
        .unwrap()
        .contains("non_orthogonal_correctors"));
    let bad = assess_mesh(&check(
        json!({"n_cells": 64, "max_non_orthogonality": 80.0, "max_skewness": 5.0, "max_aspect_ratio": 2000.0, "errors": ["negative volume in cell 3"], "warnings": []}),
    ));
    assert_eq!(bad["verdict"], "not usable");
    assert_eq!(bad["findings"].as_array().unwrap().len(), 4);
}

#[test]
fn bc_suggestions_by_name_and_kind_only_propose() {
    let patches = json!([
        {"name": "inlet", "kind": "Inlet", "faces": 4},
        {"name": "outlet", "kind": "Outlet", "faces": 4},
        {"name": "top_wall", "kind": "Wall", "faces": 4},
        {"name": "frontAndBack", "kind": "Empty", "faces": 4},
        {"name": "mystery", "kind": "Generic", "faces": 4},
        {"name": "walls", "kind": "Wall", "faces": 4}
    ]);
    let boundaries = json!({"walls": {"velocity": "fixed"}});
    let s = suggest_boundary_conditions(&patches, &boundaries);
    let list = s["suggestions"].as_array().unwrap();
    let by = |n: &str| list.iter().find(|x| x["patch"] == n).unwrap();
    assert_eq!(by("inlet")["role"], "inlet");
    assert!(by("inlet")["rationale"].as_str().unwrap().contains("PLACEHOLDER"));
    assert_eq!(
        by("outlet")["settings"][1],
        json!({"path": "boundaries.outlet.pressure", "value": "fixed"})
    );
    assert_eq!(by("top_wall")["role"], "wall");
    assert_eq!(by("frontAndBack")["role"], "empty");
    assert!(by("frontAndBack")["settings"].as_array().unwrap().is_empty());
    assert_eq!(by("mystery")["role"], "unknown");
    // Already configured patches are left alone.
    assert_eq!(by("walls")["status"], "configured");
    assert!(by("walls")["settings"].as_array().unwrap().is_empty());
}

#[test]
fn validation_finds_mismatches_and_high_reynolds() {
    let mc = json!({"patches": [{"name": "lid", "kind": "Wall"}, {"name": "walls", "kind": "Wall"}, {"name": "frontAndBack", "kind": "Empty"}]});
    let settings = json!({
        "physics": {"nu": 1e-5}, "mesh": {"lx": 1.0, "ly": 1.0},
        "boundaries": {"lid": {"velocity_value": [1.0, 0.0, 0.0]}, "nowhere": {"velocity_value": []}}
    });
    let v = validate_case(&json!({"ok": true, "diagnostics": []}), Some(&mc), &settings);
    assert_eq!(v["ok"], false);
    assert!(v["errors"][0]["message"].as_str().unwrap().contains("nowhere"));
    let w: Vec<String> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["message"].as_str().unwrap().to_string())
        .collect();
    assert!(
        w.iter()
            .any(|m| m.contains("'walls'") && m.contains("no boundary condition")),
        "{w:?}"
    );
    assert!(w.iter().any(|m| m.contains("Re =")), "{w:?}");
    assert!(!w.iter().any(|m| m.contains("frontAndBack")));
}

#[test]
fn workflow_tools_work_against_a_real_service_and_change_nothing() {
    let svc = Service::new(ServiceConfig::default());
    let tb = Toolbox::new(svc.clone(), "cases", ApprovalMode::ApproveWrites);
    tb.grant("create_case", &json!({"template": "lid_driven_cavity"}));
    assert!(
        !tb.execute("create_case", &json!({"template": "lid_driven_cavity"}))
            .is_error
    );
    let hash_before = svc
        .call("journal.get", &json!({"session": tb.session().unwrap()}))
        .unwrap()["case_hash"]
        .clone();
    let m = tb.execute("assess_mesh", &json!({}));
    assert!(!m.is_error, "{}", m.text);
    assert_eq!(m.structured["verdict"], "good");
    let v = tb.execute("validate_case", &json!({}));
    assert!(!v.is_error && v.structured["ok"] == true, "{}", v.text);
    let b = tb.execute("suggest_boundary_conditions", &json!({}));
    assert!(!b.is_error);
    assert_eq!(b.structured["suggestions"].as_array().unwrap().len(), 3);
    // Before any run there is nothing to explain, but it must not fail.
    let e = tb.execute("explain_residuals", &json!({}));
    assert!(!e.is_error, "{}", e.text);
    assert_eq!(e.structured["verdict"], "not enough data");
    let hash_after = svc
        .call("journal.get", &json!({"session": tb.session().unwrap()}))
        .unwrap()["case_hash"]
        .clone();
    assert_eq!(hash_before, hash_after);
    // No session: a clean error.
    let tb2 = Toolbox::new(Service::new(ServiceConfig::default()), "cases", ApprovalMode::ReadOnly);
    assert!(tb2.execute("assess_mesh", &json!({})).is_error);
}
