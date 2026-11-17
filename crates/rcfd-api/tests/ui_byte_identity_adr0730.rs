//! ADR 0730 T1, T2 and T6: without the UI switch every reply is the one of the base commit, byte for byte.
//!
//! The replies were recorded from the release binaries of the base commit f86339e8 before any ADR 0730 code was written
//! (`fixtures/adr0730_base_replies.json`): the byte count and the FNV-1a hash (`rcfd_core::hash::fnv64`) of the result of
//! `settings.schema`, the whole result of `service.info`, and the errors of `help.*`. `rcfd schema` is checked through
//! the binary in `crates/rcfd-cli/tests/ui_cli_adr0730.rs`, and the MCP tool lists in
//! `crates/rcfd-mcp/tests/tool_list_adr0730.rs` (M3).

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_core::hash::fnv64;
use serde_json::{json, Value};
use std::sync::Arc;

const BASE: &str = include_str!("fixtures/adr0730_base_replies.json");

fn base() -> Value {
    serde_json::from_str(BASE).unwrap()
}

fn digest(text: &str) -> (u64, String) {
    (text.len() as u64, format!("{:#018x}", fnv64(text.as_bytes())))
}

fn recorded(key: &str) -> (u64, String) {
    let v = base();
    (
        v[key]["bytes"].as_u64().unwrap(),
        v[key]["fnv64"].as_str().unwrap().into(),
    )
}

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

/// T1: `settings.schema` without the switch, with any parameters (it ignores them, as before), and with the switch but
/// without `ui: true`, is the base commit's reply.
#[test]
fn t1_settings_schema_is_the_base_commit_s_without_ui_true() {
    let want = recorded("settings_schema_result");
    let off = plain();
    for p in [
        Value::Null,
        json!({}),
        json!({"ui": false}),
        json!({"ui": true}),
        json!({"ui": "x"}),
    ] {
        assert_eq!(
            digest(&result(&off, "settings.schema", p.clone())),
            want,
            "no --ui, params {p}"
        );
    }
    let on = with_ui();
    for p in [Value::Null, json!({}), json!({"ui": false}), json!({"ui": null})] {
        assert_eq!(
            digest(&result(&on, "settings.schema", p.clone())),
            want,
            "--ui, params {p}"
        );
    }
    eprintln!("T1: settings.schema {} bytes, {} as at f86339e8", want.0, want.1);
}

/// T2: `service.info` without the switch is the base commit's reply, and `help.*` is the same -32601.
#[test]
fn t2_service_info_and_help_without_ui_are_the_base_commit_s() {
    let b = base();
    let svc = plain();
    assert_eq!(
        result(&svc, "service.info", json!({})),
        b["service_info_result"].as_str().unwrap()
    );
    assert_eq!(error(&svc, "help.glossary", json!({})), b["help_glossary_error"]);
    assert_eq!(error(&svc, "help.errors", json!({})), b["help_errors_error"]);
    assert_eq!(rcfd_api::service::METHODS.len(), 69, "the method list of protocol 1.1");
}

/// P4: with the switch, `service.info` adds `ui` and the two help methods and changes nothing else.
#[test]
fn p4_service_info_with_ui_adds_ui_and_two_methods() {
    let b: Value = serde_json::from_str(base()["service_info_result"].as_str().unwrap()).unwrap();
    let mut on: Value = serde_json::from_str(&result(&with_ui(), "service.info", json!({}))).unwrap();
    assert_eq!(on["ui"], "rcfd-ui/1");
    assert_eq!(
        on["protocol_version"], "1.1",
        "the version stays a property of the method families"
    );
    let methods = on["methods"].as_array_mut().unwrap();
    assert_eq!(
        methods.split_off(methods.len() - 2),
        vec![json!("help.glossary"), json!("help.errors")]
    );
    on.as_object_mut().unwrap().remove("ui");
    assert_eq!(on, b);
}

/// P2: `ui` that is not a boolean, or parameters that are not an object, are -32602 once the switch is on; `help.*`
/// takes `{}` only.
#[test]
fn p2_bad_parameters_are_refused_with_ui() {
    let on = with_ui();
    for p in [json!({"ui": "yes"}), json!({"ui": 1}), json!([true]), json!("ui")] {
        assert_eq!(error(&on, "settings.schema", p.clone())["code"], -32602, "{p}");
    }
    for m in ["help.glossary", "help.errors"] {
        assert_eq!(error(&on, m, json!({"x": 1}))["code"], -32602, "{m}");
        assert_eq!(error(&on, m, json!([]))["code"], -32602, "{m}");
        assert!(error(&on, m, Value::Null).is_null(), "{m} without params");
    }
    assert_eq!(error(&on, "help.nothing", json!({}))["code"], -32601);
}

fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| {
                !matches!(
                    k.as_str(),
                    "x-rcfd-ui"
                        | "x-rcfd-label"
                        | "x-rcfd-help"
                        | "x-rcfd-help-from"
                        | "x-rcfd-advanced"
                        | "x-rcfd-recommended"
                        | "x-rcfd-see"
                )
            });
            for x in m.values_mut() {
                strip(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        _ => {}
    }
}

/// T6: the export with `ui: true`, with every key this ADR adds removed (and the `x-rcfd-unit` it adds to nodes that
/// had none), is the default export.
#[test]
fn t6_ui_export_less_its_keys_is_the_default_export() {
    let on = with_ui();
    let default: Value = serde_json::from_str(&result(&on, "settings.schema", json!({}))).unwrap();
    let mut ui: Value = serde_json::from_str(&result(&on, "settings.schema", json!({"ui": true}))).unwrap();
    // the units ADR 0730 adds are the data file's `unit` entries: drop them where the default export has none
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
    assert_eq!(ui, default);
}
