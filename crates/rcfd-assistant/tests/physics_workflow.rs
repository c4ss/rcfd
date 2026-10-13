//! Settings-aware assistant helpers for the turbulence and compressible physics (additive).

use rcfd_assistant::workflow::{
    explain_residuals, suggest_boundary_conditions, suggest_boundary_conditions_for, validate_case,
};
use serde_json::{json, Value};

fn patches() -> Value {
    json!([
        {"name": "inlet", "kind": "Inlet"},
        {"name": "outlet", "kind": "Outlet"},
        {"name": "walls", "kind": "Wall"}
    ])
}

fn paths(v: &Value, patch: &str) -> Vec<String> {
    v["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["patch"] == patch)
        .unwrap()["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn laminar_settings_leave_the_suggestions_unchanged() {
    let b = json!({});
    let s = json!({"turbulence": {"model": "laminar"}, "solver": {"algorithm": "simplec"}});
    assert_eq!(
        suggest_boundary_conditions_for(&patches(), &b, &s),
        suggest_boundary_conditions(&patches(), &b)
    );
}

#[test]
fn k_epsilon_inlets_get_turbulence_helpers() {
    let s = json!({"turbulence": {"model": "k_epsilon"}, "solver": {"algorithm": "simplec"}});
    let out = suggest_boundary_conditions_for(&patches(), &json!({}), &s);
    let inlet = paths(&out, "inlet");
    for k in ["turbulence_spec", "turbulence_intensity", "turbulence_length"] {
        assert!(inlet.contains(&format!("boundaries.inlet.{k}")), "{inlet:?}");
    }
    // the incompressible suggestions stay
    assert!(inlet.contains(&"boundaries.inlet.velocity".to_string()));
    // k-omega SST gets the same inlet helpers (intensity + length is model independent)
    let sst = json!({"turbulence": {"model": "k_omega_sst"}, "solver": {"algorithm": "simplec"}});
    let out = suggest_boundary_conditions_for(&patches(), &json!({}), &sst);
    assert!(paths(&out, "inlet").contains(&"boundaries.inlet.turbulence_length".to_string()));
    assert!(!paths(&out, "walls").iter().any(|p| p.contains("turbulence")));
}

#[test]
fn compressible_patches_get_compressible_bc_entries() {
    let s = json!({"solver": {"algorithm": "compressible"}});
    let out = suggest_boundary_conditions_for(&patches(), &json!({}), &s);
    let inlet = paths(&out, "inlet");
    assert!(inlet.contains(&"boundaries.inlet.compressible_bc".to_string()));
    assert!(inlet.contains(&"boundaries.inlet.pressure_value".to_string()));
    assert!(!inlet.contains(&"boundaries.inlet.pressure".to_string()));
    assert_eq!(paths(&out, "outlet"), ["boundaries.outlet.compressible_bc"]);
    assert_eq!(paths(&out, "walls"), ["boundaries.walls.compressible_bc"]);
}

#[test]
fn validate_case_adds_physics_notes_without_the_laminar_reynolds_warning() {
    let mk = |extra: Value| {
        let mut s = json!({
            "physics": {"nu": 1e-6},
            "mesh": {"lx": 1.0, "ly": 1.0},
            "boundaries": {"inlet": {"velocity_value": [10.0, 0.0, 0.0]}},
            "solver": {"algorithm": "simplec"}
        });
        for (k, v) in extra.as_object().unwrap() {
            s[k] = v.clone();
        }
        validate_case(&json!({"diagnostics": []}), None, &s)
    };
    let text = |v: &Value| v["warnings"].to_string();
    assert!(text(&mk(json!({}))).contains("laminar flow only"));
    let ke = mk(json!({"turbulence": {"model": "k_epsilon"}}));
    assert!(text(&ke).contains("y+") && !text(&ke).contains("laminar flow only"));
    // k-omega SST: wall-resolution hint with an estimated first-cell y+
    let sst = mk(json!({
        "turbulence": {"model": "k_omega_sst"},
        "mesh": {"lx": 1.0, "ly": 0.1, "ny": 20}
    }));
    let w = sst["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["path"] == "turbulence.model")
        .unwrap();
    let yp = w["y_plus_estimate"].as_f64().expect("y+ estimate");
    // U = 10 m/s, nu = 1e-6, x = 1: Re_x = 1e7, u_tau about 0.36, h1 = 0.005 -> y+ about 900
    assert!(yp > 500.0 && yp < 1500.0, "{yp}");
    assert!(w["message"].as_str().unwrap().contains("SST") && !text(&sst).contains("laminar flow only"));
    let fine = mk(json!({
        "turbulence": {"model": "k_omega_sst"},
        "mesh": {"lx": 1.0, "ly": 0.1, "ny": 20, "wall_layers": 10, "first_layer_height": 1e-5}
    }));
    let yf = fine["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["path"] == "turbulence.model")
        .unwrap()["y_plus_estimate"]
        .as_f64()
        .unwrap();
    assert!(yf < 3.0, "{yf}");
    let comp = mk(json!({"solver": {"algorithm": "compressible"}, "compressible": {"cfl": 0.95}}));
    assert!(text(&comp).contains("compressible.cfl") && !text(&comp).contains("laminar flow only"));
}

#[test]
fn explain_residuals_handles_turbulence_and_compressible_series() {
    let falling: Vec<f64> = (0..50).map(|i| 10f64.powf(-(i as f64) / 5.0)).collect();
    // a laminar tree: res_nut is still ignored for the verdict, so the result equals the old one
    let base = json!({"solver": {"algorithm": "simplec"}});
    let a = explain_residuals(&[("res_u".into(), falling.clone())], &base);
    let b = explain_residuals(
        &[("res_u".into(), falling.clone()), ("res_nut".into(), falling.clone())],
        &base,
    );
    assert_eq!(a["verdict"], b["verdict"]);
    // a diverging k-epsilon run gets turbulence advice and a relaxation change
    let rising: Vec<f64> = (0..50).map(|i| 1e-3 * 10f64.powf(i as f64 / 5.0)).collect();
    let ke = json!({"solver": {"algorithm": "simplec"}, "turbulence": {"model": "k_epsilon", "relax": 0.7}});
    let out = explain_residuals(&[("res_u".into(), rising.clone()), ("res_nut".into(), rising)], &ke);
    assert!(out["advice"].to_string().contains("res_nut"));
    assert!(out["suggested_changes"].to_string().contains("turbulence.relax"));
    // a transient compressible run has no residual verdict
    let tr = json!({"solver": {"algorithm": "compressible"}, "compressible": {"mode": "transient"}});
    let out = explain_residuals(&[("res_u".into(), falling.clone())], &tr);
    assert_eq!(out["verdict"], "transient compressible run");
    // steady compressible: res_rho is classified against steady_tolerance
    let st =
        json!({"solver": {"algorithm": "compressible"}, "compressible": {"mode": "steady", "steady_tolerance": 1e-6}});
    let out = explain_residuals(&[("res_rho".into(), falling)], &st);
    assert_eq!(out["findings"][0]["series"], "res_rho");
    assert_eq!(out["verdict"], "converged");
}
