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
                        .find(|c| c.get("when").is_none_or(|w| eval(w, ctx)))
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
                c.get("where").is_none_or(|w| {
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
    let mut out = RuleSet::builtin().evaluate(&case.tree.to_json(), mesh);
    // ADR 0155 D6: MT101, emitted by code (not a data rule), only while the case rotates its mesh
    let mt = crate::mesh_transform::world_coordinate_warnings(case);
    if !mt.is_empty() {
        out.extend(mt);
        out.sort_by(|a, b| (a.severity, &a.id, &a.path).cmp(&(b.severity, &b.id, &b.path)));
    }
    out
}

/// Level of the computed setup rule SV301 (ADR 0156 D7): a pre-run pressure-lag estimate at or above it warns. It lies between
/// the largest step-1 estimate of a clean run (1.166) and the smallest of a spoky or diverged one (2.418) in ADR 0154's DG1.
pub const SV301_LEVEL: f64 = 1.3;

/// The computed setup rule SV301 (ADR 0156 D7) for the pre-run pressure-lag estimate of a transient case
/// ([`crate::sim::Simulation::pressure_lag_preflight`]): a warning at `solver.corrector_control` with the fix `adaptive` when the
/// estimate is at least [`SV301_LEVEL`]. Not in `setup_rules.json` because it needs a built simulation.
pub fn lag_preflight_rule(p: &crate::sim::LagPreflight) -> Option<RuleDiagnostic> {
    if p.rho.is_nan() || p.rho < SV301_LEVEL {
        return None;
    }
    let x = p.position;
    Some(RuleDiagnostic {
        id: "SV301".into(),
        severity: Severity::Warning,
        category: "solver".into(),
        path: "solver.corrector_control".into(),
        message: format!(
            "with solver.n_correctors = {} the pressure-velocity coupling is likely unstable on this mesh at this solver.dt: \
             a lagged pressure error is amplified by {:.2} per step (pressure-lag estimate, ADR 0073; warning level {}), its \
             largest entry in cell {} at ({:.4e}, {:.4e}, {:.4e}), typically a thin wall cell; the run would develop grid-scale \
             pressure oscillations (spokes) there or diverge. Remedy: set solver.corrector_control = \"adaptive\" (the solver \
             then adds pressure correctors until the estimate is at most 0.5)",
            p.n_correctors, p.rho, SV301_LEVEL, p.cell, x[0], x[1], x[2]
        ),
        fix: Some(json!({"path": "solver.corrector_control", "value": "adaptive"})),
    })
}

/// The computed setup rule SV301 found while the run is going (ADR 0172 D1): the estimate `p` made at the step the controller
/// settled on (`dt`, after `step` steps of this launch). Same id, path and fix as [`lag_preflight_rule`].
pub fn lag_watch_rule(p: &crate::sim::LagPreflight, step: usize, dt: f64) -> Option<RuleDiagnostic> {
    if p.rho.is_nan() || p.rho < SV301_LEVEL {
        return None;
    }
    let x = p.position;
    Some(RuleDiagnostic {
        id: "SV301".into(),
        severity: Severity::Warning,
        category: "solver".into(),
        path: "solver.corrector_control".into(),
        message: format!(
            "with solver.n_correctors = {} the pressure-velocity coupling has become unstable on this mesh at the time step the \
             run has reached (step {} of this launch, dt = {:.4e}): a lagged pressure error is amplified by {:.2} per step \
             (pressure-lag estimate, ADR 0073, on the present state; warning level {}), its largest entry in cell {} at \
             ({:.4e}, {:.4e}, {:.4e}), typically a thin wall cell; the run may develop grid-scale pressure oscillations (spokes) \
             there or diverge. Remedy: set solver.corrector_control = \"adaptive\" (the solver then adds pressure correctors until \
             the estimate is at most 0.5)",
            p.n_correctors, step, dt, p.rho, SV301_LEVEL, p.cell, x[0], x[1], x[2]
        ),
        fix: Some(json!({"path": "solver.corrector_control", "value": "adaptive"})),
    })
}

/// Level of the computed setup rule SV302 (ADR 0172 D3): a wall-cell `aspect ratio x tan(non-orthogonality)` at or above it
/// warns when `linear_upwind` is unlimited. It lies between ADR 0171's values for M1w15 (about 1 600, clean at 6 degrees) and
/// M1w05 (about 6 900, diverged at 6 degrees); a judgement, not a fit.
pub const SV302_LEVEL: f64 = 3000.0;

/// The wall cell of largest `aspect ratio x tan(non-orthogonality)` of a mesh (ADR 0172 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallRisk {
    /// `aspect * tan(non_orth)`.
    pub metric: f64,
    /// The cell.
    pub cell: usize,
    /// Its centre.
    pub position: [f64; 3],
    /// Its aspect ratio (longest over shortest edge, `rcfd_mesh::quality`).
    pub aspect: f64,
    /// Its largest face non-orthogonality, degrees (clamped to 89.9).
    pub non_orth_deg: f64,
}

/// True when the patch `p` is a wall for the case (ADR 0172 D3, amendment 1): the turbulence role `Wall` of its boundary entry
/// (defaults included: a fixed zero velocity), the definition the wall treatments use, on a patch that is not an empty,
/// symmetry or cyclic one.
pub fn is_wall_patch(case: &Case, p: &rcfd_mesh::mesh::Patch) -> bool {
    use rcfd_mesh::mesh::PatchKind;
    !matches!(p.kind, PatchKind::Empty | PatchKind::Symmetry | PatchKind::Cyclic)
        && crate::turb::role(case, &p.name) == crate::turb::Role::Wall
}

/// The wall cell with the largest `aspect x tan(non-orthogonality)` (ADR 0172 D3), `None` without a wall cell. The aspect
/// ratio of a cell and the non-orthogonality of its faces are `rcfd_mesh::quality::compute_metrics`'s; the angle is the largest
/// over all faces of the cell, clamped to 89.9 degrees.
pub fn wall_cell_risk(case: &Case, mesh: &rcfd_mesh::mesh::Mesh) -> Option<WallRisk> {
    let flags: Vec<bool> = mesh.patches().iter().map(|p| is_wall_patch(case, p)).collect();
    if !flags.iter().any(|&w| w) {
        return None;
    }
    let m = rcfd_mesh::quality::compute_metrics(mesh);
    let cc = mesh.cell_centres();
    let mut seen = vec![false; mesh.n_cells()];
    let mut best: Option<WallRisk> = None;
    for (p, _) in mesh.patches().iter().zip(&flags).filter(|(_, w)| **w) {
        for f in p.faces() {
            let c = mesh.owner()[f];
            if std::mem::replace(&mut seen[c], true) {
                continue;
            }
            let theta = mesh
                .cell_faces(c)
                .iter()
                .map(|&g| m.non_orthogonality[g])
                .fold(0.0_f64, f64::max)
                .min(89.9);
            let metric = m.aspect_ratio[c] * theta.to_radians().tan();
            let metric = if metric.is_nan() { 0.0 } else { metric };
            if best.is_none_or(|b| metric > b.metric) {
                best = Some(WallRisk {
                    metric,
                    cell: c,
                    position: cc[c],
                    aspect: m.aspect_ratio[c],
                    non_orth_deg: theta,
                });
            }
        }
    }
    best
}

/// The computed setup rule SV302 (ADR 0172 D3): a risk notice for unlimited `linear_upwind` on thin, strongly skewed wall cells.
/// A warning, no fix (the limiter costs accuracy), and honest about false positives.
pub fn linear_upwind_wall_rule(r: &WallRisk) -> Option<RuleDiagnostic> {
    if r.metric.is_nan() || r.metric < SV302_LEVEL {
        return None;
    }
    let x = r.position;
    Some(RuleDiagnostic {
        id: "SV302".into(),
        severity: Severity::Warning,
        category: "solver".into(),
        path: "solver.linear_upwind_limiter".into(),
        message: format!(
            "solver.convection = \"linear_upwind\" is unlimited (solver.linear_upwind_limiter = \"none\") and the mesh has thin, \
             strongly skewed wall cells: aspect ratio x tan(non-orthogonality) reaches {:.0} (notice level {}) in cell {} at \
             ({:.4e}, {:.4e}, {:.4e}), aspect ratio {:.0}, non-orthogonality {:.1} degrees. This is a risk, not a prediction: in \
             ADR 0171's NACA 0012 runs (wall y+ about 2 to 5, metric 6900 to 21000) the unlimited extrapolation on such cells at \
             the trailing edge made the steady solver diverge within 30 iterations once the flow was off-axis (6 degrees), \
             while the same mesh at 0 degrees, with the same metric, converged, and a mesh with metric 1600 converged at 6 degrees; \
             lowering the relaxation factors did not help. If the run diverges at a wall cell, set \
             solver.linear_upwind_limiter = \"barth_jespersen\" (it limits the extrapolation in the convection term; cost in ADR \
             0171: about +3 % in cd_eff at 0 degrees, all in the pressure drag). The default is unchanged",
            r.metric, SV302_LEVEL, r.cell, x[0], x[1], x[2], r.aspect, r.non_orth_deg
        ),
        fix: None,
    })
}

/// What the dynamic set-up rules (SV301, SV302) need from a built simulation, taken while the simulation is at hand
/// (ADR 0172 D2); [`crate::runctl::check_only`] builds the simulation, takes this and drops it before it builds its mesh.
#[derive(Debug, Clone, Copy)]
pub struct SimFacts {
    /// The pre-run pressure-lag estimate (SV301), at the settled step.
    pub lag: Option<crate::sim::LagPreflight>,
    /// The steady solver runs `linear_upwind` without a limiter (SV302 applies).
    pub unlimited_linear_upwind: bool,
}

impl SimFacts {
    /// The facts of `sim` now (a continued run's restarted state, a fresh run's initial state).
    pub fn of(sim: &crate::sim::Simulation) -> Self {
        SimFacts {
            lag: sim.pressure_lag_preflight(),
            unlimited_linear_upwind: sim.unlimited_linear_upwind(),
        }
    }
}

/// The dynamic set-up diagnostics, SV301 then SV302 (ADR 0172 D2): they need a built simulation or its mesh.
pub fn dynamic_diagnostics(case: &Case, facts: &SimFacts, mesh: &rcfd_mesh::mesh::Mesh) -> Vec<RuleDiagnostic> {
    let mut out = Vec::new();
    out.extend(facts.lag.as_ref().and_then(lag_preflight_rule));
    if facts.unlimited_linear_upwind {
        out.extend(wall_cell_risk(case, mesh).as_ref().and_then(linear_upwind_wall_rule));
    }
    out
}

/// [`dynamic_diagnostics`] of a built simulation and its own mesh.
pub fn dynamic_setup_diagnostics(sim: &crate::sim::Simulation) -> Vec<RuleDiagnostic> {
    dynamic_diagnostics(&sim.case, &SimFacts::of(sim), &sim.geom.mesh)
}

/// Every set-up diagnostic of a case about to run (ADR 0172 D2): the static rules ([`check_case`], with the mesh quality `q`)
/// followed by the dynamic ones ([`dynamic_diagnostics`]). The one list that `run.check`, `rcfd run`, `run.headless` and the
/// chain driver show.
pub fn setup_diagnostics(
    case: &Case,
    facts: &SimFacts,
    mesh: &rcfd_mesh::mesh::Mesh,
    q: &rcfd_mesh::quality::QualityReport,
) -> Vec<RuleDiagnostic> {
    let mut out = check_case(case, Some(&mesh_context(q)));
    out.extend(dynamic_diagnostics(case, facts, mesh));
    out
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
