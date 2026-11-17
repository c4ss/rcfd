//! F-834: every leaf of the case schema the service publishes (`settings.schema`) has a default that is a valid value of
//! its own kind. The check of the frozen GUI's and the egui Studio's case editors (both removed by F-449), without their
//! settings-tree model: the walk, the kinds and the input rule of `rcfd-gui/src/schema_model.rs` restated here over the
//! JSON Schema.

use rcfd_api::service::{Service, ServiceConfig};
use serde_json::{json, Value};

/// The kind of a leaf, as the editors' settings-tree model read it from the JSON Schema.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Bool,
    Int { min: Option<f64>, max: Option<f64> },
    Float { min: Option<f64>, max: Option<f64> },
    Text,
    Enum(Vec<String>),
    NumberList,
}

/// A leaf: its dotted path, kind and schema default.
struct Leaf {
    path: String,
    kind: Kind,
    default: Value,
}

fn kind_of(schema: &Value) -> Kind {
    let min = schema.get("minimum").and_then(Value::as_f64);
    let max = schema.get("maximum").and_then(Value::as_f64);
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        return Kind::Enum(e.iter().filter_map(|v| v.as_str().map(String::from)).collect());
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("boolean") => Kind::Bool,
        Some("integer") => Kind::Int { min, max },
        Some("number") => Kind::Float { min, max },
        Some("array") => Kind::NumberList,
        _ => Kind::Text,
    }
}

/// The leaves of the fixed part of the tree (named maps, `x-rcfd-named-objects`, are not expanded), in the schema's
/// key order.
fn leaves(schema: &Value, prefix: &str, out: &mut Vec<Leaf>) {
    for (k, v) in schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        if v.get("x-rcfd-named-objects") == Some(&Value::Bool(true)) {
            continue;
        }
        if v.get("properties").and_then(Value::as_object).is_some() {
            leaves(v, &path, out);
            continue;
        }
        out.push(Leaf {
            path,
            kind: kind_of(v),
            default: v.get("default").cloned().unwrap_or(Value::Null),
        });
    }
}

/// The text an edit box showed for a value (`format_value`).
fn format_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(format_value).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

fn in_range(v: f64, min: Option<f64>, max: Option<f64>) -> Result<(), String> {
    if min.is_some_and(|m| v < m) {
        return Err(format!("{v} is below the minimum {min:?}"));
    }
    if max.is_some_and(|m| v > m) {
        return Err(format!("{v} is above the maximum {max:?}"));
    }
    Ok(())
}

/// The editors' input rule (`parse_input`) for the kinds whose defaults were checked: the value `text` stands for.
fn parse_input(kind: &Kind, text: &str) -> Result<Value, String> {
    let t = text.trim();
    match kind {
        Kind::Bool => match t.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(Value::Bool(true)),
            "false" | "no" | "0" => Ok(Value::Bool(false)),
            _ => Err("expected true or false".into()),
        },
        Kind::Int { min, max } => {
            let v: i64 = t.parse().map_err(|_| format!("'{t}' is not a whole number"))?;
            in_range(v as f64, *min, *max)?;
            Ok(Value::from(v))
        }
        Kind::Float { min, max } => {
            let v: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
            if !v.is_finite() {
                return Err("value must be finite".into());
            }
            in_range(v, *min, *max)?;
            Ok(Value::from(v))
        }
        Kind::Enum(allowed) => {
            if allowed.iter().any(|a| a == t) {
                Ok(Value::String(t.to_string()))
            } else {
                Err(format!("must be one of: {}", allowed.join(", ")))
            }
        }
        Kind::Text | Kind::NumberList => Ok(Value::String(text.to_string())),
    }
}

/// Replaces `crates/rcfd-gui/tests/logic.rs::every_schema_leaf_is_reachable_and_parses_its_default` and
/// `crates/rcfd-studio/tests/editor_b5.rs::every_schema_leaf_is_reachable_and_parses_its_default` (ADR 0131 B5): the
/// schema exposes more than 30 leaves, and the default of every boolean, integer, number and enumeration leaf is a
/// valid value of its kind (within its bounds, a member of its enumeration); text and number-list leaves and leaves
/// without a default are skipped, as there. Negative control: the rule refuses out-of-range and garbage input (the
/// engine part of `parse_input_rejects_out_of_range_and_garbage` of both files). Not ported (editor UI): the settings
/// tree's node lookup, field errors, edits, named entries, saves and the controller.
#[test]
fn every_schema_leaf_parses_its_default() {
    let svc = Service::new(ServiceConfig::default());
    let doc = svc.call("settings.schema", &json!({})).unwrap();
    let mut all = Vec::new();
    leaves(&doc["case"], "", &mut all);
    assert!(
        all.len() > 30,
        "the schema should expose many settings, got {}",
        all.len()
    );
    let mut parsed = 0;
    let mut bad = Vec::new();
    for l in &all {
        if matches!(l.kind, Kind::Text | Kind::NumberList) {
            continue;
        }
        let text = format_value(&l.default);
        if text.is_empty() || l.default.is_null() {
            continue;
        }
        match parse_input(&l.kind, &text) {
            Ok(_) => parsed += 1,
            Err(e) => bad.push(format!("default of {} ({text}) does not parse: {e}", l.path)),
        }
    }
    eprintln!("F-834 B5: {} leaves, {parsed} defaults parsed", all.len());
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(parsed > 30, "{parsed} defaults parsed");
    // negative control: the rule refuses what it must
    let k = Kind::Int {
        min: Some(1.0),
        max: Some(10.0),
    };
    assert_eq!(parse_input(&k, "5").unwrap(), json!(5));
    assert!(parse_input(&k, "0").is_err());
    assert!(parse_input(&k, "abc").is_err());
    assert!(parse_input(&Kind::Bool, "maybe").is_err());
    assert!(parse_input(&Kind::Enum(vec!["a".into()]), "b").is_err());
}
