//! ADR 0730 T1, T2, P2, P4 and T6: without the UI switch every reply is the one the engine gave before the ADR, and with
//! it the reply is the plain one plus exactly the listed additions.
//!
//! Restated 2026-11-17 (ADR 0730, "Result", the fixture note): the tests compare the same build with and without `--ui`,
//! and the plain replies with what the code paths the ADR does not touch produce (`case_schema().to_json_schema()`,
//! `monitor_json_schema()`, `METHODS`), never with a frozen recording, so a merge that adds a method, a tool or a key
//! needs no re-recording. The byte-identity of the plain MCP tool lists is held by the pinned tests
//! `crates/rcfd-mcp/tests/default_f1.rs` and `tool_list_f275.rs`; the case and run hashes by `baseline_hashes.rs` (T9).

use rcfd_api::schema::case_schema;
use rcfd_api::service::{Service, ServiceConfig, METHODS};
use rcfd_api::ui::{UI_METHODS, UI_PROTOCOL};
use rcfd_post::monitors::monitor_json_schema;
use serde_json::{json, Value};
use std::sync::Arc;

/// The result text of a call as `rcfd serve` writes it (the reply line holds `to_string()` of the result).
fn result(svc: &Arc<Service>, method: &str, params: Value) -> String {
    let r = svc
        .handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .unwrap();
    assert!(r.get("error").is_none(), "{method} {params}: {r}");
    serde_json::to_string(&r["result"]).unwrap()
}

fn error(svc: &Arc<Service>, method: &str, params: Value) -> Value {
    let r = svc
        .handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .unwrap();
    r["error"].clone()
}

fn plain() -> Arc<Service> {
    Service::new(ServiceConfig::default())
}

fn with_ui() -> Arc<Service> {
    let s = Service::new(ServiceConfig::default());
    s.enable_ui();
    s
}

/// The `settings.schema` result as the code before ADR 0730 builds it (the arm the ADR leaves as it was).
fn untouched_schema() -> String {
    serde_json::to_string(&json!({"case": case_schema().to_json_schema(), "monitors": monitor_json_schema()})).unwrap()
}

/// T1: `settings.schema` without the switch, with any parameters (it ignores them, as before), and with the switch but
/// without `ui: true`, is the untouched export byte for byte.
#[test]
fn t1_settings_schema_is_the_plain_export_without_ui_true() {
    let want = untouched_schema();
    let off = plain();
    for p in [
        Value::Null,
        json!({}),
        json!({"ui": false}),
        json!({"ui": true}),
        json!({"ui": "x"}),
    ] {
        assert!(
            result(&off, "settings.schema", p.clone()) == want,
            "no --ui, params {p}"
        );
    }
    let on = with_ui();
    for p in [Value::Null, json!({}), json!({"ui": false}), json!({"ui": null})] {
        assert!(result(&on, "settings.schema", p.clone()) == want, "--ui, params {p}");
    }
    // the --ui reply is a different text: the switch is what adds the metadata
    assert_ne!(result(&on, "settings.schema", json!({"ui": true})), want);
    eprintln!(
        "T1: settings.schema {} bytes without ui: true, with and without --ui",
        want.len()
    );
}

/// T2: `service.info` without the switch has no `ui` key and lists exactly `METHODS`; `help.*` is the same -32601 as
/// any unknown method.
#[test]
fn t2_service_info_and_help_without_ui_are_as_before() {
    let svc = plain();
    let info: Value = serde_json::from_str(&result(&svc, "service.info", json!({}))).unwrap();
    assert!(info.get("ui").is_none());
    assert_eq!(info["methods"], json!(METHODS));
    for m in UI_METHODS {
        assert!(!METHODS.contains(&m), "{m} is not a method of the plain service");
        let e = error(&svc, m, json!({}));
        assert_eq!(
            e,
            json!({"code": -32601, "message": format!("unknown method '{m}'")}),
            "{m}"
        );
        // the same shape as a name that never existed
        let other = error(&svc, "nothing.here", json!({}));
        assert_eq!(e["code"], other["code"]);
    }
}

/// P4: with the switch, `service.info` is the plain reply plus `ui` and the two help methods at the end; nothing else.
#[test]
fn p4_service_info_with_ui_is_the_plain_one_plus_ui_and_two_methods() {
    let plain_info: Value = serde_json::from_str(&result(&plain(), "service.info", json!({}))).unwrap();
    let mut on: Value = serde_json::from_str(&result(&with_ui(), "service.info", json!({}))).unwrap();
    assert_eq!(on["ui"], UI_PROTOCOL);
    assert_eq!(
        on["protocol_version"], plain_info["protocol_version"],
        "the version stays a property of the families"
    );
    let methods = on["methods"].as_array_mut().unwrap();
    let tail = methods.split_off(methods.len() - UI_METHODS.len());
    assert_eq!(tail, UI_METHODS.iter().map(|m| json!(m)).collect::<Vec<_>>());
    on.as_object_mut().unwrap().remove("ui");
    assert_eq!(on, plain_info);
}

/// P2: `ui` that is not a boolean, or parameters that are not an object, are -32602 once the switch is on; `help.*`
/// takes `{}` only.
#[test]
fn p2_bad_parameters_are_refused_with_ui() {
    let on = with_ui();
    for p in [json!({"ui": "yes"}), json!({"ui": 1}), json!([true]), json!("ui")] {
        assert_eq!(error(&on, "settings.schema", p.clone())["code"], -32602, "{p}");
    }
    for m in UI_METHODS {
        assert_eq!(error(&on, m, json!({"x": 1}))["code"], -32602, "{m}");
        assert_eq!(error(&on, m, json!([]))["code"], -32602, "{m}");
        assert!(error(&on, m, Value::Null).is_null(), "{m} without params");
    }
    assert_eq!(error(&on, "help.nothing", json!({}))["code"], -32601);
}

/// The keys ADR 0730 adds to schema nodes.
const UI_KEYS: [&str; 7] = [
    "x-rcfd-ui",
    "x-rcfd-label",
    "x-rcfd-help",
    "x-rcfd-help-from",
    "x-rcfd-advanced",
    "x-rcfd-recommended",
    "x-rcfd-see",
];

fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| !UI_KEYS.contains(&k.as_str()));
            for x in m.values_mut() {
                strip(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
}

/// T6: the export with `ui: true`, with every key this ADR adds removed (and the `x-rcfd-unit` it adds to nodes that
/// had none), is the plain export of the same build.
#[test]
fn t6_ui_export_less_its_keys_is_the_plain_export() {
    let on = with_ui();
    let plain_reply: Value = serde_json::from_str(&result(&plain(), "settings.schema", json!({}))).unwrap();
    let mut ui: Value = serde_json::from_str(&result(&on, "settings.schema", json!({"ui": true}))).unwrap();
    // the units ADR 0730 adds are the data file's `unit` entries: drop them where the plain export has none
    let meta = rcfd_api::ui::metadata().unwrap();
    for (path, m) in &meta.keys {
        if m.unit.is_some() {
            let mut n = &mut ui["case"];
            for seg in path.split('.') {
                n = if seg == "*" {
                    &mut n["additionalProperties"]
                } else {
                    &mut n["properties"][seg]
                };
            }
            n.as_object_mut().unwrap().remove("x-rcfd-unit");
        }
    }
    strip(&mut ui);
    assert_eq!(ui, plain_reply);
}
