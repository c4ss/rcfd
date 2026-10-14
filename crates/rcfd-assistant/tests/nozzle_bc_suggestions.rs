//! Wave-11 bug: `suggest_boundary_conditions` proposed slip walls for the end patches (xmin / xmax,
//! typed Wall by the mesh generator) of a compressible nozzle, which closes the duct. F-062 / ADR 0006:
//! a nozzle needs `total_inlet` at the inlet and `pressure_outlet` at the outlet. Uses the F-094 eval
//! nozzle case (docs/roadmap.json F-094) without editing its registered criteria.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_assistant::*;
use serde_json::{json, Value};

const NOZZLE: &str = "title = \"nozzle\"\n[mesh]\ngenerator = \"nozzle_2d\"\nnx = 40\nny = 8\nlx = 2.0\nly = 0.1\nthroat_height = 0.05\nexit_height = 0.1\n[solver]\nalgorithm = \"compressible\"\nend_time = 0.002\n[compressible]\nflux = \"hllc\"\n[compressible.initial.left]\ndensity = 1.15\nvelocity = [60.0, 0.0, 0.0]\npressure = 98000.0\n";

fn rig() -> Toolbox {
    let svc = Service::new(ServiceConfig::default());
    let tb = Toolbox::new(svc, "cases", ApprovalMode::ApproveWrites);
    let args = json!({"toml": NOZZLE});
    tb.grant("create_case", &args);
    assert!(!tb.execute("create_case", &args).is_error);
    tb
}

fn entries(sug: &Value, patch: &str) -> Vec<(String, Value)> {
    sug["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["patch"] == patch)
        .unwrap_or_else(|| panic!("no suggestion for {patch}"))["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["path"].as_str().unwrap().to_string(), e["value"].clone()))
        .collect()
}

fn value_of(e: &[(String, Value)], key: &str) -> Option<Value> {
    e.iter()
        .find(|(p, _)| p.ends_with(&format!(".{key}")))
        .map(|(_, v)| v.clone())
}

#[test]
fn nozzle_end_patches_get_characteristic_conditions_not_slip_walls() {
    let tb = rig();
    let out = tb.execute("suggest_boundary_conditions", &json!({}));
    assert!(!out.is_error, "{}", out.text);
    let sug = out.structured;
    let inlet = entries(&sug, "xmin");
    assert_eq!(
        value_of(&inlet, "compressible_bc"),
        Some(json!("total_inlet")),
        "{inlet:?}"
    );
    // the placeholder state is derived from the initial state: p0 = p (1 + (g-1)/2 M^2)^(g/(g-1))
    let p0 = value_of(&inlet, "total_pressure_value")
        .and_then(|v| v.as_f64())
        .expect("total pressure");
    assert!(p0 > 98000.0 && p0 < 110000.0, "p0 = {p0}");
    assert!(value_of(&inlet, "temperature_value").and_then(|v| v.as_f64()).unwrap() > 250.0);
    let outlet = entries(&sug, "xmax");
    assert_eq!(
        value_of(&outlet, "compressible_bc"),
        Some(json!("pressure_outlet")),
        "{outlet:?}"
    );
    assert_eq!(value_of(&outlet, "pressure_value"), Some(json!(98000.0)));
    // the centre line and the wall stay slip walls
    for p in ["ymin", "ymax"] {
        assert_eq!(
            value_of(&entries(&sug, p), "compressible_bc"),
            Some(json!("slip")),
            "{p}"
        );
    }
    // the rationale says what was decided and that the values are placeholders
    let text = sug.to_string();
    assert!(text.contains("PLACEHOLDER"), "{text}");
}

#[test]
fn applying_every_nozzle_suggestion_gives_an_open_valid_case() {
    let tb = rig();
    let sug = tb.execute("suggest_boundary_conditions", &json!({})).structured;
    for s in sug["suggestions"].as_array().unwrap() {
        for e in s["settings"].as_array().unwrap() {
            let a = json!({"path": e["path"], "value": e["value"]});
            tb.grant("set_setting", &a);
            let r = tb.execute("set_setting", &a);
            assert!(!r.is_error, "{}: {}", e["path"], r.text);
        }
    }
    let v = tb.execute("validate_case", &json!({}));
    assert!(!v.is_error && v.structured["ok"] == true, "{}", v.text);
    let st = tb.execute("get_setting", &json!({"path": "boundaries.xmin.compressible_bc"}));
    assert!(st.text.contains("total_inlet"), "{}", st.text);
}
