//! Parametric studies (F-095, ADR 0034): sweeps of settings-tree values, one run per design point,
//! a result table of chosen report values (CSV and JSON), failure isolation and resume.
//!
//! ```toml
//! [study]
//! outputs = ["q"]
//! [study.parameters.k]
//! path = "physics.conductivity"
//! kind = "list"
//! values = [1.0, 2.0]
//! ```
//!
//! The table never contains wall-clock times or thread numbers, so it is byte-identical for any
//! `parallel` setting.

use crate::case::{Case, Diagnostic};
use crate::runctl::{self, exit, RunOptions};
use crate::sim::Simulation;
use rcfd_core::settings::Schema;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Largest number of design points a study may define.
pub const MAX_POINTS: usize = 100_000;

/// Schema of the `study` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Parametric study (ADR 0034): sweeps of settings, one run per design point, result table. Empty parameters = no study")
        .field(
            "outputs",
            Schema::list(Schema::string("")).doc("Report names that become table columns; empty = all reports of the case, sorted by name"),
        )
        .field(
            "combine",
            Schema::enumeration("grid", &["grid", "zip"]).doc("grid: cartesian product (first parameter name, alphabetically, slowest); zip: parameters walk together, equal lengths"),
        )
        .field(
            "parallel",
            Schema::int(1).doc("Design points run at the same time (>= 1); the table does not depend on it"),
        )
        .field(
            "write_results",
            Schema::boolean(true).doc("Write each point's result files to <out>/points/pNNNN/"),
        )
        .field(
            "parameters",
            Schema::named(
                Schema::object()
                    .doc("One swept parameter")
                    .field("path", Schema::string("").doc("Settings path the parameter overrides (as for --set)"))
                    .field(
                        "kind",
                        Schema::enumeration("list", &["list", "linspace", "logspace"]).doc("list: the given values; linspace: count values from start to stop; logspace: geometric progression from start to stop (both > 0)"),
                    )
                    .field("values", Schema::list(Schema::float(0.0)).doc("list: numeric values in order"))
                    .field(
                        "strings",
                        Schema::list(Schema::string("")).doc("list: string values (instead of values)"),
                    )
                    .field(
                        "json",
                        Schema::list(Schema::string("")).doc("list: values as JSON texts, for vectors and booleans (for example \"[0.1, 0, 0]\"); instead of values and strings"),
                    )
                    .field("start", Schema::float(0.0).doc("linspace, logspace: first value"))
                    .field("stop", Schema::float(0.0).doc("linspace, logspace: last value"))
                    .field("count", Schema::int(0).doc("linspace, logspace: number of values (>= 1)")),
            )
            .doc("Parameters by name"),
        )
}

/// One swept parameter with its expanded, type-checked values.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Name (key in `study.parameters`).
    pub name: String,
    /// Settings path overridden.
    pub path: String,
    /// The values as the settings tree stores them after applying them (normalised).
    pub values: Vec<Value>,
}

/// A validated study.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Parameters sorted by name.
    pub params: Vec<Parameter>,
    /// `grid` or `zip`.
    pub zip: bool,
    /// Table columns (report names).
    pub outputs: Vec<String>,
    /// Concurrent design points.
    pub parallel: usize,
    /// Write the files of each point.
    pub write_results: bool,
}

/// One design point.
#[derive(Debug, Clone)]
pub struct Point {
    /// Position in the deterministic order.
    pub index: usize,
    /// Parameter name and value, in parameter-name order.
    pub values: Vec<(String, Value)>,
}

fn diag(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn get_f64(case: &Case, p: &str) -> f64 {
    case.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn get_strs(case: &Case, p: &str) -> Vec<String> {
    case.get(p)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Raw (not yet type-checked) values of a sweep, or located errors.
fn expand(case: &Case, name: &str, errs: &mut Vec<Diagnostic>) -> Vec<Value> {
    let base = format!("study.parameters.{name}");
    let kind = case
        .get(&format!("{base}.kind"))
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "list".into());
    let strings = get_strs(case, &format!("{base}.strings"));
    let jsons = get_strs(case, &format!("{base}.json"));
    let nums: Vec<f64> = case
        .get(&format!("{base}.values"))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    match kind.as_str() {
        "list" => {
            if !jsons.is_empty() {
                if !strings.is_empty() || !nums.is_empty() {
                    errs.push(diag(
                        format!("{base}.json"),
                        "json, values and strings are alternatives; use one of them",
                    ));
                    return Vec::new();
                }
                let mut out = Vec::new();
                for (i, t) in jsons.iter().enumerate() {
                    match serde_json::from_str::<Value>(t) {
                        Ok(v) => out.push(v),
                        Err(e) => errs.push(diag(format!("{base}.json.{i}"), format!("not JSON: {e}"))),
                    }
                }
                return out;
            }
            if !strings.is_empty() && !nums.is_empty() {
                errs.push(diag(
                    format!("{base}.strings"),
                    "values and strings are both set; use one of them",
                ));
                return Vec::new();
            }
            if !strings.is_empty() {
                return strings.into_iter().map(Value::String).collect();
            }
            if nums.is_empty() {
                errs.push(diag(
                    format!("{base}.values"),
                    "empty sweep: a list needs at least one value",
                ));
                return Vec::new();
            }
            for (i, x) in nums.iter().enumerate() {
                if !x.is_finite() {
                    errs.push(diag(format!("{base}.values.{i}"), "value must be finite"));
                }
            }
            nums.into_iter().map(number).collect()
        }
        k @ ("linspace" | "logspace") => {
            let count = case
                .get(&format!("{base}.count"))
                .ok()
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if count < 1 {
                errs.push(diag(
                    format!("{base}.count"),
                    format!("{k} needs count >= 1 (got {count})"),
                ));
                return Vec::new();
            }
            if count as usize > MAX_POINTS {
                errs.push(diag(format!("{base}.count"), format!("count exceeds {MAX_POINTS}")));
                return Vec::new();
            }
            if !strings.is_empty() || !nums.is_empty() || !jsons.is_empty() {
                errs.push(diag(
                    format!("{base}.values"),
                    format!("values/strings are for kind list, not {k}"),
                ));
                return Vec::new();
            }
            let (a, b) = (
                get_f64(case, &format!("{base}.start")),
                get_f64(case, &format!("{base}.stop")),
            );
            if !a.is_finite() || !b.is_finite() {
                errs.push(diag(format!("{base}.start"), "start and stop must be finite"));
                return Vec::new();
            }
            if k == "logspace" && (a <= 0.0 || b <= 0.0) {
                errs.push(diag(
                    format!("{base}.start"),
                    format!("logspace needs start > 0 and stop > 0 (got {a}, {b})"),
                ));
                return Vec::new();
            }
            let n = count as usize;
            (0..n)
                .map(|i| {
                    let x = if i + 1 == n && n > 1 {
                        b
                    } else if n == 1 {
                        a
                    } else {
                        let t = i as f64 / (n - 1) as f64;
                        if k == "linspace" {
                            a + (b - a) * t
                        } else {
                            a * (b / a).powf(t)
                        }
                    };
                    number(x)
                })
                .collect()
        }
        other => {
            errs.push(diag(format!("{base}.kind"), format!("unknown kind '{other}'")));
            Vec::new()
        }
    }
}

fn number(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Apply one sweep value to `case` through the settings tree; integral numbers are tried as
/// integers first (integer settings), then as floats.
pub fn apply_value(case: &mut Case, path: &str, v: &Value) -> Result<(), String> {
    if let Some(x) = v.as_f64() {
        if x.fract() == 0.0 && x.abs() < 9.0e15 {
            let first = case.set(path, json!(x as i64));
            let Err(e1) = first else { return Ok(()) };
            // an integer setting that rejects the value (range) reports that reason, not "expected integer"
            return match case.set(path, v.clone()) {
                Ok(()) => Ok(()),
                Err(e2) => {
                    let (m1, m2) = (e1.to_string(), e2.to_string());
                    Err(if m2.contains("expected integer") { m1 } else { m2 })
                }
            };
        }
    }
    case.set(path, v.clone()).map_err(|e| e.to_string())
}

/// Plan the study of `case`. With `require` an empty `study.parameters` is an error (the command
/// `rcfd study`); the static check of a plain case passes `false`.
pub fn plan(case: &Case, require: bool) -> Result<Plan, Vec<Diagnostic>> {
    let mut errs = Vec::new();
    let mut names = case.tree.names("study.parameters").unwrap_or_default();
    names.sort();
    if names.is_empty() {
        if require {
            errs.push(diag("study.parameters", "the study has no parameters"));
        }
        return if errs.is_empty() {
            Ok(Plan {
                params: Vec::new(),
                zip: false,
                outputs: Vec::new(),
                parallel: 1,
                write_results: true,
            })
        } else {
            Err(errs)
        };
    }
    let zip = case
        .get("study.combine")
        .ok()
        .and_then(|v| v.as_str().map(|s| s == "zip"))
        .unwrap_or(false);
    let parallel = case.get("study.parallel").ok().and_then(|v| v.as_i64()).unwrap_or(1);
    if parallel < 1 {
        errs.push(diag(
            "study.parallel",
            format!("parallel must be >= 1 (got {parallel})"),
        ));
    }
    let write_results = case
        .get("study.write_results")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut probe = case.clone();
    let mut params: Vec<Parameter> = Vec::new();
    for n in &names {
        let base = format!("study.parameters.{n}");
        let path = case
            .get(&format!("{base}.path"))
            .ok()
            .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
            .unwrap_or_default();
        if path.is_empty() {
            errs.push(diag(format!("{base}.path"), "empty settings path"));
            continue;
        }
        if path == "study" || path.starts_with("study.") {
            errs.push(diag(format!("{base}.path"), "a study cannot sweep its own section"));
            continue;
        }
        if let Some(prev) = params.iter().find(|p| p.path == path) {
            errs.push(diag(
                format!("{base}.path"),
                format!("'{path}' is already swept by parameter '{}'", prev.name),
            ));
            continue;
        }
        let raw = expand(case, n, &mut errs);
        if raw.is_empty() {
            continue;
        }
        let field = if !get_strs(case, &format!("{base}.json")).is_empty() {
            "json"
        } else if !get_strs(case, &format!("{base}.strings")).is_empty() {
            "strings"
        } else {
            "values"
        };
        let list_kind = case
            .get(&format!("{base}.kind"))
            .ok()
            .and_then(|v| v.as_str().map(|s| s == "list"))
            .unwrap_or(true);
        let mut values = Vec::with_capacity(raw.len());
        let mut ok = true;
        for (i, v) in raw.iter().enumerate() {
            match apply_value(&mut probe, &path, v) {
                Ok(()) => values.push(probe.get(&path).unwrap_or_else(|_| v.clone())),
                Err(e) => {
                    let at = if list_kind {
                        format!("{base}.{field}.{i}")
                    } else {
                        format!("{base}.{}", if i == 0 { "start" } else { "stop" })
                    };
                    // an unknown path is reported once, at the path
                    let at = if probe.get(&path).is_err() {
                        format!("{base}.path")
                    } else {
                        at
                    };
                    errs.push(diag(at, e));
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            params.push(Parameter {
                name: n.clone(),
                path,
                values,
            });
        }
    }
    let all = case.tree.names("reports").map(|mut v| {
        v.sort();
        v
    });
    let all = all.unwrap_or_default();
    let mut outputs = get_strs(case, "study.outputs");
    if outputs.is_empty() {
        outputs = all.clone();
    } else {
        for (i, o) in outputs.iter().enumerate() {
            if !all.contains(o) {
                errs.push(diag(format!("study.outputs.{i}"), format!("no report named '{o}'")));
            }
        }
    }
    if zip && params.len() == names.len() {
        let lens: Vec<usize> = params.iter().map(|p| p.values.len()).collect();
        if lens.iter().any(|&l| l != lens[0]) {
            errs.push(diag(
                "study.combine",
                format!(
                    "zip needs equal lengths, got {}",
                    lens.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    if !zip && params.len() == names.len() {
        let mut total = 1usize;
        for p in &params {
            total = total.saturating_mul(p.values.len());
        }
        if total > MAX_POINTS {
            errs.push(diag(
                "study.parameters",
                format!("{total} design points exceed the limit {MAX_POINTS}"),
            ));
        }
    }
    if errs.is_empty() {
        Ok(Plan {
            params,
            zip,
            outputs,
            parallel: parallel as usize,
            write_results,
        })
    } else {
        Err(errs)
    }
}

/// Static checks of the `study` section (empty for a case without parameters).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    plan(case, false).err().unwrap_or_default()
}

impl Plan {
    /// The design points in their deterministic order.
    pub fn points(&self) -> Vec<Point> {
        if self.params.is_empty() {
            return Vec::new();
        }
        let n = if self.zip {
            self.params[0].values.len()
        } else {
            self.params.iter().map(|p| p.values.len()).product()
        };
        (0..n)
            .map(|index| {
                let mut values = Vec::with_capacity(self.params.len());
                if self.zip {
                    for p in &self.params {
                        values.push((p.name.clone(), p.values[index].clone()));
                    }
                } else {
                    // row-major: the first parameter varies slowest
                    let mut rest = index;
                    let mut idx = vec![0usize; self.params.len()];
                    for (k, p) in self.params.iter().enumerate().rev() {
                        idx[k] = rest % p.values.len();
                        rest /= p.values.len();
                    }
                    for (k, p) in self.params.iter().enumerate() {
                        values.push((p.name.clone(), p.values[idx[k]].clone()));
                    }
                }
                Point { index, values }
            })
            .collect()
    }

    fn path_of(&self, name: &str) -> &str {
        self.params
            .iter()
            .find(|p| p.name == name)
            .map_or("", |p| p.path.as_str())
    }
}

/// 64-bit FNV-1a of the resolved settings of a point without the `study` section.
pub fn fingerprint(case: &Case) -> String {
    let mut j = case.tree.to_json();
    if let Some(o) = j.as_object_mut() {
        o.remove("study");
    }
    let text = j.to_string();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The case of one design point: the base case with the overrides applied.
pub fn point_case(base: &Case, plan: &Plan, point: &Point) -> Result<Case, (u8, String)> {
    let mut c = base.clone();
    for (name, v) in &point.values {
        let path = plan.path_of(name);
        apply_value(&mut c, path, v).map_err(|e| (exit::INVALID, format!("{path}: {e}")))?;
    }
    let sem = c.semantic_errors();
    if !sem.is_empty() {
        return Err((
            exit::INVALID,
            sem.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        ));
    }
    Ok(c)
}

/// One row of the result table.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Point index.
    pub point: usize,
    /// Parameter values by name.
    pub parameters: Vec<(String, Value)>,
    /// `ok` or `failed`.
    pub status: String,
    /// Run-control exit code of the point.
    pub exit_code: u8,
    /// Final state (`converged`, `finished`, `failed`, ...).
    pub state: String,
    /// Iterations or steps.
    pub iterations: usize,
    /// See [`fingerprint`].
    pub fingerprint: String,
    /// Chosen report values (null when unavailable).
    pub reports: Vec<(String, Value)>,
    /// Failure reason or notes.
    pub message: String,
}

impl Row {
    fn to_json(&self) -> Value {
        let obj = |v: &[(String, Value)]| Value::Object(v.iter().cloned().collect::<Map<String, Value>>());
        json!({
            "point": self.point, "parameters": obj(&self.parameters), "status": self.status,
            "exit_code": self.exit_code, "state": self.state, "iterations": self.iterations,
            "fingerprint": self.fingerprint, "reports": obj(&self.reports), "message": self.message,
        })
    }

    fn from_json(v: &Value, plan: &Plan) -> Option<Row> {
        let pairs = |key: &str, names: &[String]| -> Option<Vec<(String, Value)>> {
            let o = v.get(key)?.as_object()?;
            names.iter().map(|n| Some((n.clone(), o.get(n)?.clone()))).collect()
        };
        let pn: Vec<String> = plan.params.iter().map(|p| p.name.clone()).collect();
        Some(Row {
            point: v.get("point")?.as_u64()? as usize,
            parameters: pairs("parameters", &pn)?,
            status: v.get("status")?.as_str()?.to_string(),
            exit_code: v.get("exit_code")?.as_u64()? as u8,
            state: v.get("state")?.as_str()?.to_string(),
            iterations: v.get("iterations")?.as_u64()? as usize,
            fingerprint: v.get("fingerprint")?.as_str()?.to_string(),
            reports: pairs("reports", &plan.outputs)?,
            message: v.get("message")?.as_str()?.to_string(),
        })
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => csv_cell(s),
        Value::Array(_) | Value::Object(_) => csv_cell(&v.to_string()),
        other => other.to_string(),
    }
}

/// The table as JSON text and as CSV text.
pub fn render_table(plan: &Plan, rows: &[Row]) -> (String, String) {
    let doc = json!({
        "format": "rcfd-study-table", "version": 1,
        "parameters": plan.params.iter().map(|p| json!({"name": p.name, "path": p.path})).collect::<Vec<_>>(),
        "outputs": plan.outputs,
        "rows": rows.iter().map(Row::to_json).collect::<Vec<_>>(),
    });
    let mut csv = String::from("point");
    for p in &plan.params {
        csv.push(',');
        csv.push_str(&csv_cell(&p.name));
    }
    csv.push_str(",status,exit_code,state,iterations");
    for o in &plan.outputs {
        csv.push(',');
        csv.push_str(&csv_cell(o));
    }
    csv.push_str(",message\n");
    for r in rows {
        csv.push_str(&r.point.to_string());
        for (_, v) in &r.parameters {
            csv.push(',');
            csv.push_str(&csv_value(v));
        }
        csv.push_str(&format!(
            ",{},{},{},{}",
            r.status,
            r.exit_code,
            csv_cell(&r.state),
            r.iterations
        ));
        for (_, v) in &r.reports {
            csv.push(',');
            csv.push_str(&csv_value(v));
        }
        csv.push(',');
        csv.push_str(&csv_cell(&r.message));
        csv.push('\n');
    }
    (serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n", csv)
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Options of a study run.
#[derive(Debug, Clone, Default)]
pub struct StudyOptions {
    /// Output directory.
    pub out_dir: PathBuf,
    /// Overrides `study.parallel` when set.
    pub parallel: Option<usize>,
    /// Keep completed rows of an existing table.
    pub resume: bool,
    /// Stop after this many newly run points.
    pub max_new_points: Option<usize>,
    /// Do not write per-point result files (overrides `study.write_results`).
    pub no_fields: bool,
}

/// Summary of a study run.
#[derive(Debug, Clone)]
pub struct StudyOutcome {
    /// Design points planned.
    pub points: usize,
    /// Points run in this launch.
    pub run: usize,
    /// Points taken from the existing table.
    pub skipped: usize,
    /// Rows with status `failed`.
    pub failed: usize,
    /// `finished`, `failed` or `partial`.
    pub state: &'static str,
    /// Exit code of the study.
    pub exit_code: u8,
    /// The rows, by point index (only completed ones).
    pub rows: Vec<Row>,
}

impl StudyOutcome {
    /// The closing summary object.
    pub fn to_json(&self, out_dir: &Path) -> Value {
        json!({
            "format": "rcfd-study", "points": self.points, "run": self.run, "skipped": self.skipped,
            "failed": self.failed, "state": self.state, "exit_code": self.exit_code,
            "table": out_dir.join("table.json").display().to_string(),
        })
    }
}

/// Run one design point. Never panics.
pub fn run_point(base: &Case, plan: &Plan, point: &Point, dir: Option<&Path>) -> Row {
    let mut row = Row {
        point: point.index,
        parameters: point.values.clone(),
        status: "failed".into(),
        exit_code: exit::FAIL,
        state: "failed".into(),
        iterations: 0,
        fingerprint: String::new(),
        reports: plan.outputs.iter().map(|o| (o.clone(), Value::Null)).collect(),
        message: String::new(),
    };
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_point_inner(base, plan, point, dir, &mut row)
    }));
    if let Err(p) = caught {
        let msg = p
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_else(|| "panic".into());
        row.status = "failed".into();
        row.exit_code = exit::FAIL;
        row.state = "failed".into();
        row.message = format!("internal error: {msg}");
    }
    row
}

fn run_point_inner(base: &Case, plan: &Plan, point: &Point, dir: Option<&Path>, row: &mut Row) {
    let case = match point_case(base, plan, point) {
        Ok(c) => c,
        Err((code, m)) => {
            row.exit_code = code;
            row.message = m;
            return;
        }
    };
    row.fingerprint = fingerprint(&case);
    let sim = match Simulation::new(&case) {
        Ok(s) => s,
        Err(d) => {
            row.exit_code = exit::INVALID;
            row.message = d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
            return;
        }
    };
    let q = rcfd_mesh::quality::check_mesh(&sim.geom.mesh);
    if !q.is_ok() {
        row.exit_code = exit::BAD_MESH;
        row.message = format!("mesh error: {}", q.errors.join("; "));
        return;
    }
    let rd = crate::rules::check_case(&case, Some(&crate::rules::mesh_context(&q)));
    if crate::rules::has_errors(&rd) {
        row.exit_code = exit::INVALID;
        row.message = rd
            .iter()
            .filter(|d| d.severity == crate::rules::Severity::Error)
            .map(|d| format!("{} {}: {}", d.id, d.path, d.message))
            .collect::<Vec<_>>()
            .join("; ");
        return;
    }
    if let Some(d) = dir {
        if let Err(e) = std::fs::create_dir_all(d) {
            row.message = format!("cannot create {}: {e}", d.display());
            return;
        }
    }
    let opts = RunOptions {
        out_dir: dir.map(Path::to_path_buf),
        info: json!({"driver": "study", "point": point.index}),
        ..RunOptions::default()
    };
    let sim = Mutex::new(sim);
    let outcome = runctl::drive(&sim, &opts, &|| false, &mut |_| {});
    let sim = sim.into_inner().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = dir {
        if let Err(e) = runctl::write_results(&sim, d) {
            row.message = format!("writing results to {}: {e}", d.display());
        }
    }
    let values = sim.report_values();
    let mut notes = Vec::new();
    for (name, slot) in &mut row.reports {
        match values.iter().find(|(n, _)| n == name) {
            Some((_, Ok(v))) => *slot = number(v.value),
            Some((_, Err(e))) => notes.push(format!("report {name}: {e}")),
            None => notes.push(format!("report {name} not found")),
        }
    }
    row.exit_code = outcome.exit_code;
    row.state = outcome.state.to_string();
    row.iterations = outcome.iterations;
    row.status = if outcome.exit_code == exit::OK { "ok" } else { "failed" }.into();
    let mut msg = Vec::new();
    if outcome.exit_code != exit::OK {
        msg.push(outcome.reason.clone());
    }
    msg.extend(notes);
    if !row.message.is_empty() {
        msg.push(std::mem::take(&mut row.message));
    }
    row.message = msg.join("; ");
}

/// Directory name of a point's files.
pub fn point_dir(out_dir: &Path, index: usize) -> PathBuf {
    out_dir.join("points").join(format!("p{index:04}"))
}

fn load_rows(path: &Path, plan: &Plan) -> Vec<Row> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    v.get("rows")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|r| Row::from_json(r, plan)).collect())
        .unwrap_or_default()
}

/// Run the study of `base`. Static errors are returned before anything is written.
pub fn run_study(
    base: &Case,
    opts: &StudyOptions,
    sink: &mut (dyn FnMut(&Value) + Send),
) -> Result<StudyOutcome, Vec<Diagnostic>> {
    let plan = plan(base, true)?;
    let points = plan.points();
    let parallel = opts.parallel.unwrap_or(plan.parallel).max(1);
    let write = plan.write_results && !opts.no_fields;
    std::fs::create_dir_all(&opts.out_dir)
        .map_err(|e| vec![diag("", format!("cannot create {}: {e}", opts.out_dir.display()))])?;
    let json_path = opts.out_dir.join("table.json");
    let csv_path = opts.out_dir.join("table.csv");
    // fingerprints of the planned points (also the validity check of each point)
    let planned: Vec<Option<String>> = points
        .iter()
        .map(|p| point_case(base, &plan, p).ok().map(|c| fingerprint(&c)))
        .collect();
    let old = if opts.resume {
        load_rows(&json_path, &plan)
    } else {
        Vec::new()
    };
    let mut slots: Vec<Option<Row>> = vec![None; points.len()];
    let mut skipped = 0usize;
    for (i, p) in points.iter().enumerate() {
        let hit = old
            .iter()
            .find(|r| r.status == "ok" && Some(&r.fingerprint) == planned[i].as_ref() && r.parameters == p.values);
        if let Some(r) = hit {
            let mut r = r.clone();
            r.point = i;
            slots[i] = Some(r);
            skipped += 1;
        }
    }
    let mut pending: Vec<usize> = (0..points.len()).filter(|&i| slots[i].is_none()).collect();
    let truncated = opts.max_new_points.is_some_and(|m| pending.len() > m);
    if let Some(m) = opts.max_new_points {
        pending.truncate(m);
    }
    let run_n = pending.len();
    let store = Mutex::new((slots, sink));
    let flush = |slots: &[Option<Row>]| {
        let rows: Vec<Row> = slots.iter().flatten().cloned().collect();
        let (j, c) = render_table(&plan, &rows);
        let _ = write_atomic(&json_path, &j);
        let _ = write_atomic(&csv_path, &c);
    };
    flush(&store.lock().unwrap_or_else(|e| e.into_inner()).0);
    let next = AtomicUsize::new(0);
    let worker = || loop {
        let k = next.fetch_add(1, Ordering::SeqCst);
        let Some(&i) = pending.get(k) else { break };
        let dir = write.then(|| point_dir(&opts.out_dir, i));
        let row = run_point(base, &plan, &points[i], dir.as_deref());
        let mut g = store.lock().unwrap_or_else(|e| e.into_inner());
        (g.1)(&json!({
            "event": "point", "point": i, "status": row.status, "exit_code": row.exit_code, "state": row.state,
        }));
        g.0[i] = Some(row);
        flush(&g.0);
    };
    if parallel == 1 || pending.len() <= 1 {
        worker();
    } else {
        std::thread::scope(|s| {
            for _ in 0..parallel.min(pending.len()) {
                s.spawn(worker);
            }
        });
    }
    let (slots, _) = store.into_inner().unwrap_or_else(|e| e.into_inner());
    let rows: Vec<Row> = slots.into_iter().flatten().collect();
    let failed = rows.iter().filter(|r| r.status != "ok").count();
    let first_failed = rows.iter().find(|r| r.status != "ok").map(|r| r.exit_code);
    let exit_code = first_failed.unwrap_or(exit::OK);
    let state = if failed > 0 {
        "failed"
    } else if truncated {
        "partial"
    } else {
        "finished"
    };
    Ok(StudyOutcome {
        points: points.len(),
        run: run_n,
        skipped,
        failed,
        state,
        exit_code,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(extra: &str) -> Case {
        Case::parse_toml(&format!("[mesh]\ngenerator=\"box_2d\"\nnx=4\nny=4\n{extra}"))
            .unwrap_or_else(|e| panic!("{e:?}"))
    }

    #[test]
    fn linspace_logspace_ends_are_exact() {
        let c = case(
            "[study.parameters.a]\npath=\"physics.nu\"\nkind=\"linspace\"\nstart=0.1\nstop=0.7\ncount=4\n[study.parameters.b]\npath=\"physics.rho\"\nkind=\"logspace\"\nstart=0.05\nstop=0.2\ncount=3\n",
        );
        let p = plan(&c, true).unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(p.params[0].values.last().and_then(Value::as_f64), Some(0.7));
        assert_eq!(p.params[1].values[0].as_f64(), Some(0.05));
        assert_eq!(p.params[1].values[2].as_f64(), Some(0.2));
        assert_eq!(p.points().len(), 12);
        // first parameter slowest
        let pts = p.points();
        assert_eq!(pts[0].values[0].1, pts[1].values[0].1);
        assert_ne!(pts[0].values[1].1, pts[1].values[1].1);
    }

    #[test]
    fn located_static_errors() {
        // the static check runs when the case is parsed
        let r = Case::parse_toml(
            "[mesh]
generator=\"box_2d\"
[study.parameters.a]
path=\"physics.nope\"
values=[1.0]
",
        );
        let e = r.err().unwrap_or_default();
        assert!(e.iter().any(|d| d.path == "study.parameters.a.path"), "{e:?}");
    }
}
