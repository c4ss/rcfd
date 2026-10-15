//! Declarative setup-validation rules (F-240, ADR 0031 D6).
//!
//! The rules are data (`rules/setup_rules.json`, format `rcfd-setup-rules` version 1), not code. Each
//! rule is a condition over the resolved settings JSON (`case.tree.to_json()`, so defaults are
//! visible) and, when a mesh is available, a `mesh` context taken from the serialised quality report.
//! A rule that fires yields a categorised diagnostic (error, warning or info) with a located path,
//! before anything is solved. Evaluation is pure and deterministic.
//!
//! Grammar (see ADR 0031): a condition is `{"all": [..]}`, `{"any": [..]}`, `{"not": c}`,
//! `{"path": P, "op": OP, "value": V}` or `{"count": P, "op": OP, "value": N}` with an optional
//! `"where": c` that filters the counted items (`@` is bound to each item). `P` may start with `@`
//! (the item bound by the rule's `each`) or `$mesh.` (the mesh context); `V` may be `{"path": P2}`.
//! Operators: eq ne gt ge lt le in not_in vec_zero vec_nonzero is_empty non_empty name_in.

use crate::case::Case;
use serde::Serialize;
use serde_json::{json, Value};

/// The built-in rule set.
pub const BUILTIN_RULES: &str = include_str!("../rules/setup_rules.json");

/// Severity of a diagnostic, in sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The case cannot run as set up.
    Error,
    /// The case runs but is probably not what was meant.
    Warning,
    /// Worth knowing.
    Info,
}

impl Severity {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
    /// Lower-case name.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

const CATEGORIES: &[&str] = &["mesh", "boundaries", "materials", "models", "solver", "output", "setup"];
const OPS: &[&str] = &[
    "eq",
    "ne",
    "gt",
    "ge",
    "lt",
    "le",
    "in",
    "not_in",
    "vec_zero",
    "vec_nonzero",
    "is_empty",
    "non_empty",
    "name_in",
];

/// One rule diagnostic.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDiagnostic {
    /// Rule id (`SVnnn`).
    pub id: String,
    /// Severity.
    pub severity: Severity,
    /// Category (mesh, boundaries, materials, models, solver, output, setup).
    pub category: String,
    /// Located settings path.
    pub path: String,
    /// What is wrong.
    pub message: String,
    /// Suggested change (`{"path", "value"}`), never applied.
    pub fix: Option<Value>,
}

impl RuleDiagnostic {
    /// JSON object with the keys `id severity category path message` (and `fix` when the rule has one).
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "id": self.id, "severity": self.severity.name(), "category": self.category,
            "path": self.path, "message": self.message,
        });
        if let Some(f) = &self.fix {
            v["fix"] = f.clone();
        }
        v
    }
}

impl std::fmt::Display for RuleDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}/{}] {}: {}",
            self.id,
            self.severity.name(),
            self.category,
            self.path,
            self.message
        )
    }
}

#[derive(Debug, Clone)]
struct Rule {
    id: String,
    severity: Severity,
    category: String,
    each: Option<String>,
    when: Value,
    path: Value,
    message: String,
    fix: Option<Value>,
}

/// A validated rule set.
#[derive(Debug, Clone)]
pub struct RuleSet {
    rules: Vec<Rule>,
}

fn check_keys(v: &Value, allowed: &[&str], what: &str) -> Result<(), String> {
    let Some(o) = v.as_object() else {
        return Err(format!("{what}: expected an object"));
    };
    for k in o.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(format!("{what}: unknown key '{k}'"));
        }
    }
    Ok(())
}

fn check_cond(c: &Value, what: &str) -> Result<(), String> {
    let Some(o) = c.as_object() else {
        return Err(format!("{what}: a condition must be an object"));
    };
    if let Some(l) = o.get("all").or_else(|| o.get("any")) {
        check_keys(c, &["all", "any"], what)?;
        if o.len() != 1 {
            return Err(format!("{what}: 'all' and 'any' stand alone"));
        }
        let Some(a) = l.as_array() else {
            return Err(format!("{what}: 'all'/'any' need a list"));
        };
        for (i, x) in a.iter().enumerate() {
            check_cond(x, &format!("{what}[{i}]"))?;
        }
        return Ok(());
    }
    if let Some(n) = o.get("not") {
        check_keys(c, &["not"], what)?;
        return check_cond(n, &format!("{what}.not"));
    }
    let key = if o.contains_key("count") { "count" } else { "path" };
    check_keys(c, &[key, "op", "value", "where"], what)?;
    if !o.get(key).is_some_and(Value::is_string) {
        return Err(format!("{what}: needs a string '{key}' (or all/any/not)"));
    }
    let Some(op) = o.get("op").and_then(Value::as_str) else {
        return Err(format!("{what}: needs an 'op'"));
    };
    if !OPS.contains(&op) {
        return Err(format!("{what}: unknown operator '{op}'"));
    }
    let needs_value = !matches!(op, "vec_zero" | "vec_nonzero" | "is_empty" | "non_empty");
    if needs_value != o.contains_key("value") {
        return Err(format!(
            "{what}: operator '{op}' {} a 'value'",
            if needs_value { "needs" } else { "takes no" }
        ));
    }
    if key == "count" && !matches!(op, "eq" | "ne" | "gt" | "ge" | "lt" | "le") {
        return Err(format!("{what}: 'count' takes a numeric comparison"));
    }
    if matches!(op, "in" | "not_in") && !o.get("value").is_some_and(Value::is_array) {
        return Err(format!("{what}: '{op}' needs a list"));
    }
    if let Some(w) = o.get("where") {
        if key != "count" {
            return Err(format!("{what}: 'where' belongs to 'count'"));
        }
        check_cond(w, &format!("{what}.where"))?;
    }
    Ok(())
}

impl RuleSet {
    /// Parse and strictly validate a rule file. Unknown keys, operators, severities and categories
    /// are errors.
    pub fn parse(text: &str) -> Result<RuleSet, String> {
        let doc: Value = serde_json::from_str(text).map_err(|e| format!("rules file: {e}"))?;
        check_keys(&doc, &["format", "version", "rules", "comment"], "rules file")?;
        if doc["format"] != "rcfd-setup-rules" {
            return Err("rules file: format must be \"rcfd-setup-rules\"".into());
        }
        if doc["version"] != 1 {
            return Err("rules file: unsupported version (expected 1)".into());
        }
        let list = doc["rules"].as_array().ok_or("rules file: 'rules' must be a list")?;
        let mut rules = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (i, r) in list.iter().enumerate() {
            let what = format!("rules[{i}]");
            check_keys(
                r,
                &[
                    "id", "name", "severity", "category", "each", "when", "path", "message", "fix",
                ],
                &what,
            )?;
            let s = |k: &str| {
                r.get(k)
                    .and_then(Value::as_str)
                    .map(String::from)
                    .ok_or(format!("{what}: needs a string '{k}'"))
            };
            let id = s("id")?;
            if !seen.insert(id.clone()) {
                return Err(format!("{what}: duplicate id {id}"));
            }
            s("name")?;
            let sev = s("severity")?;
            let severity = Severity::parse(&sev).ok_or(format!("{what} ({id}): unknown severity '{sev}'"))?;
            let category = s("category")?;
            if !CATEGORIES.contains(&category.as_str()) {
                return Err(format!("{what} ({id}): unknown category '{category}'"));
            }
            let each = match r.get("each") {
                None => None,
                Some(e) => {
                    check_keys(e, &["over"], &format!("{what}.each"))?;
                    Some(
                        e["over"]
                            .as_str()
                            .ok_or(format!("{what}.each: needs a string 'over'"))?
                            .to_string(),
                    )
                }
            };
            let when = r.get("when").ok_or(format!("{what}: needs 'when'"))?.clone();
            check_cond(&when, &format!("{what} ({id}).when"))?;
            let path = r.get("path").ok_or(format!("{what}: needs 'path'"))?.clone();
            match &path {
                Value::String(_) => {}
                Value::Object(_) => {
                    check_keys(&path, &["first_of"], &format!("{what}.path"))?;
                    let l = path["first_of"]
                        .as_array()
                        .ok_or(format!("{what}.path: 'first_of' needs a list"))?;
                    for c in l {
                        check_keys(c, &["when", "path"], &format!("{what}.path.first_of"))?;
                        if !c["path"].is_string() {
                            return Err(format!("{what}.path.first_of: needs a string 'path'"));
                        }
                        if let Some(w) = c.get("when") {
                            check_cond(w, &format!("{what}.path.first_of.when"))?;
                        }
                    }
                }
                _ => return Err(format!("{what}: 'path' must be a string or {{\"first_of\": [..]}}")),
            }
            let message = s("message")?;
            let fix = match r.get("fix") {
                None => None,
                Some(f) => {
                    check_keys(f, &["path", "value"], &format!("{what}.fix"))?;
                    Some(f.clone())
                }
            };
            rules.push(Rule {
                id,
                severity,
                category,
                each,
                when,
                path,
                message,
                fix,
            });
        }
        Ok(RuleSet { rules })
    }

    /// The built-in rules (`rules/setup_rules.json`).
    pub fn builtin() -> RuleSet {
        RuleSet::parse(BUILTIN_RULES).expect("built-in setup rules are valid")
    }

    /// Rule ids in file order.
    pub fn ids(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.id.as_str()).collect()
    }

    /// Number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// True when there are no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Evaluate every rule against `settings` (and `mesh` when given). Diagnostics are sorted by
    /// (severity, id, path).
    pub fn evaluate(&self, settings: &Value, mesh: Option<&Value>) -> Vec<RuleDiagnostic> {
        let mut out = Vec::new();
        for r in &self.rules {
            let uses_mesh = r.when.to_string().contains("$mesh.");
            if uses_mesh && mesh.is_none() {
                continue;
            }
            let ctx = Ctx {
                settings,
                mesh,
                item: None,
            };
            match &r.each {
                None => r.fire(&ctx, "", &mut out),
                Some(over) => {
                    let Some(coll) = lookup(&ctx, over) else { continue };
                    let items: Vec<(String, Value)> = match coll {
                        Value::Object(o) => {
                            let mut v: Vec<_> = o.iter().map(|(k, x)| (k.clone(), x.clone())).collect();
                            v.sort_by(|a, b| a.0.cmp(&b.0));
                            v
                        }
                        Value::Array(a) => a.iter().enumerate().map(|(i, x)| (i.to_string(), x.clone())).collect(),
                        _ => continue,
                    };
                    for (name, val) in items {
                        let c = Ctx {
                            settings,
                            mesh,
                            item: Some(&val),
                        };
                        r.fire(&c, &name, &mut out);
                    }
                }
            }
        }
        out.sort_by(|a, b| (a.severity, &a.id, &a.path).cmp(&(b.severity, &b.id, &b.path)));
        out
    }
}

impl Rule {
    fn fire(&self, ctx: &Ctx, name: &str, out: &mut Vec<RuleDiagnostic>) {
        if !eval(&self.when, ctx) {
            return;
        }
        let path = match &self.path {
            Value::String(s) => s.clone(),
            other => other["first_of"]
                .as_array()
                .and_then(|l| {
                    l.iter()
                        .find(|c| c.get("when").map_or(true, |w| eval(w, ctx)))
                        .and_then(|c| c["path"].as_str().map(String::from))
                })
                .unwrap_or_default(),
        };
        let sub = |s: &str| s.replace("{name}", name);
        out.push(RuleDiagnostic {
            id: self.id.clone(),
            severity: self.severity,
            category: self.category.clone(),
            path: sub(&path),
            message: sub(&self.message),
            fix: self.fix.as_ref().map(|f| {
                let mut f = f.clone();
                if let Some(p) = f["path"].as_str() {
                    f["path"] = json!(sub(p));
                }
                f
            }),
        });
    }
}

struct Ctx<'a> {
    settings: &'a Value,
    mesh: Option<&'a Value>,
    item: Option<&'a Value>,
}

fn walk<'a>(mut v: &'a Value, rest: &str) -> Option<&'a Value> {
    if rest.is_empty() {
        return Some(v);
    }
    for seg in rest.split('.') {
        v = match v {
            Value::Object(o) => o.get(seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(v)
}

fn lookup<'a>(ctx: &Ctx<'a>, path: &str) -> Option<&'a Value> {
    if path == "@" {
        return ctx.item;
    }
    if let Some(r) = path.strip_prefix("@.") {
        return walk(ctx.item?, r);
    }
    if let Some(r) = path.strip_prefix("$mesh.") {
        return walk(ctx.mesh?, r);
    }
    walk(ctx.settings, path)
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn equal(a: &Value, b: &Value) -> bool {
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn compare(a: &Value, op: &str, b: &Value) -> bool {
    let (Some(x), Some(y)) = (num(a), num(b)) else {
        return false;
    };
    match op {
        "gt" => x > y,
        "ge" => x >= y,
        "lt" => x < y,
        "le" => x <= y,
        _ => false,
    }
}

fn eval(c: &Value, ctx: &Ctx) -> bool {
    if let Some(l) = c.get("all").and_then(Value::as_array) {
        return l.iter().all(|x| eval(x, ctx));
    }
    if let Some(l) = c.get("any").and_then(Value::as_array) {
        return l.iter().any(|x| eval(x, ctx));
    }
    if let Some(n) = c.get("not") {
        return !eval(n, ctx);
    }
    let op = c["op"].as_str().unwrap_or("");
    let operand = |c: &Value| -> Option<Value> {
        let v = c.get("value")?;
        match v.get("path").and_then(Value::as_str) {
            Some(p) if v.as_object().is_some_and(|o| o.len() == 1) => lookup(ctx, p).cloned(),
            _ => Some(v.clone()),
        }
    };
    if let Some(p) = c.get("count").and_then(Value::as_str) {
        let Some(coll) = lookup(ctx, p) else {
            return compare_count(0.0, op, &operand(c));
        };
        let items: Vec<&Value> = match coll {
            Value::Object(o) => o.values().collect(),
            Value::Array(a) => a.iter().collect(),
            _ => Vec::new(),
        };
        let n = items
            .into_iter()
            .filter(|it| {
                c.get("where").map_or(true, |w| {
                    eval(
                        w,
                        &Ctx {
                            settings: ctx.settings,
                            mesh: ctx.mesh,
                            item: Some(it),
                        },
                    )
                })
            })
            .count();
        return compare_count(n as f64, op, &operand(c));
    }
    let Some(p) = c.get("path").and_then(Value::as_str) else {
        return false;
    };
    let Some(v) = lookup(ctx, p) else {
        return false;
    };
    match op {
        "eq" => operand(c).is_some_and(|b| equal(v, &b)),
        "ne" => operand(c).is_some_and(|b| !equal(v, &b)),
        "gt" | "ge" | "lt" | "le" => operand(c).is_some_and(|b| compare(v, op, &b)),
        "in" => c["value"].as_array().is_some_and(|l| l.iter().any(|x| equal(v, x))),
        "not_in" => c["value"].as_array().is_some_and(|l| !l.iter().any(|x| equal(v, x))),
        "vec_zero" => v.as_array().is_some_and(|a| a.iter().all(|x| num(x) == Some(0.0))),
        "vec_nonzero" => v
            .as_array()
            .is_some_and(|a| a.iter().any(|x| num(x).is_some_and(|y| y != 0.0))),
        "is_empty" => match v {
            Value::String(s) => s.is_empty(),
            Value::Array(a) => a.is_empty(),
            Value::Object(o) => o.is_empty(),
            Value::Null => true,
            _ => false,
        },
        "non_empty" => match v {
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            _ => false,
        },
        "name_in" => {
            let (Some(name), Some(coll)) = (v.as_str(), c["value"].as_str().and_then(|p| lookup(ctx, p))) else {
                return false;
            };
            coll.as_object().is_some_and(|o| o.contains_key(name))
        }
        _ => false,
    }
}

fn compare_count(n: f64, op: &str, b: &Option<Value>) -> bool {
    let Some(b) = b else { return false };
    let a = json!(n);
    match op {
        "eq" => equal(&a, b),
        "ne" => !equal(&a, b),
        _ => compare(&a, op, b),
    }
}

/// Evaluate the built-in rules for `case`, with the mesh context when `mesh` is given (a serialised
/// quality report).
pub fn check_case(case: &Case, mesh: Option<&Value>) -> Vec<RuleDiagnostic> {
    RuleSet::builtin().evaluate(&case.tree.to_json(), mesh)
}

/// The mesh context of the rules: the serialised quality report.
pub fn mesh_context(q: &rcfd_mesh::quality::QualityReport) -> Value {
    serde_json::to_value(q).unwrap_or(Value::Null)
}

/// Rule diagnostics as a JSON array.
pub fn to_json(diags: &[RuleDiagnostic]) -> Value {
    Value::Array(diags.iter().map(RuleDiagnostic::to_json).collect())
}

/// True when any diagnostic is an error.
pub fn has_errors(diags: &[RuleDiagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// The warning-level diagnostics as objects for `warning` status events (`message`, `rule`, `path`).
pub fn warnings_json(diags: &[RuleDiagnostic]) -> Value {
    Value::Array(
        diags
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| json!({"message": d.message, "rule": d.id, "path": d.path, "category": d.category}))
            .collect(),
    )
}
