//! Case definition files (TOML or JSON) generated from, and validated
//! against, the settings schema (F-080).
//!
//! `Case::parse` reports *all* problems with their dotted setting path and,
//! for TOML, the source line, so `rcfd validate case.toml` can print precise
//! diagnostics.

use crate::schema::case_schema;
use rcfd_core::settings::{coerce, Kind, Schema, SettingsTree};
use rcfd_core::{Error, Result};
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::path::Path;

/// One validation problem.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    /// Dotted setting path (empty for whole-file problems).
    pub path: String,
    /// 1-based source line when known.
    pub line: Option<usize>,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.path.is_empty()) {
            (Some(l), false) => write!(f, "line {l}: {}: {}", self.path, self.message),
            (Some(l), true) => write!(f, "line {l}: {}", self.message),
            (None, false) => write!(f, "{}: {}", self.path, self.message),
            (None, true) => write!(f, "{}", self.message),
        }
    }
}

/// Collect every schema violation of `value` (instead of stopping at the first).
pub fn collect_errors(schema: &Schema, value: &Value, path: &str, out: &mut Vec<Diagnostic>) {
    let join = |seg: &str| {
        if path.is_empty() {
            seg.to_string()
        } else {
            format!("{path}.{seg}")
        }
    };
    let mut push = |p: &str, m: String| {
        out.push(Diagnostic {
            path: p.into(),
            line: None,
            message: m,
        })
    };
    match &schema.kind {
        Kind::Object(fields) => {
            let Some(obj) = value.as_object() else {
                push(path, "expected a table".into());
                return;
            };
            for (k, v) in obj {
                match fields
                    .iter()
                    .find(|f| &f.name == k && !matches!(f.schema.kind, Kind::Command(_)))
                {
                    Some(f) => collect_errors(&f.schema, v, &join(k), out),
                    None => {
                        let known: Vec<&str> = fields
                            .iter()
                            .filter(|f| !matches!(f.schema.kind, Kind::Command(_)))
                            .map(|f| f.name.as_str())
                            .collect();
                        out.push(Diagnostic {
                            path: join(k),
                            line: None,
                            message: format!("unknown setting '{k}' (known: {})", known.join(", ")),
                        });
                    }
                }
            }
        }
        Kind::Named(item) => match value.as_object() {
            Some(obj) => {
                for (k, v) in obj {
                    collect_errors(item, v, &join(k), out);
                }
            }
            None => push(path, "expected a table of named entries".into()),
        },
        Kind::List(item) => match value.as_array() {
            Some(a) => {
                for (i, v) in a.iter().enumerate() {
                    collect_errors(item, v, &join(&i.to_string()), out);
                }
            }
            None => push(path, "expected a list".into()),
        },
        Kind::Command(_) => push(path, "commands cannot appear in a case file".into()),
        _ => {
            if let Err(e) = coerce(schema, value, path) {
                let msg = match e {
                    Error::InvalidValue { reason, .. } => reason,
                    other => other.to_string(),
                };
                push(path, msg);
            }
        }
    }
}

/// Best-effort 1-based line of `path` in TOML `text` (the value's own line,
/// else the nearest enclosing table header).
pub fn locate_line(text: &str, path: &str) -> Option<usize> {
    let mut header = String::new();
    let mut best: Option<(usize, usize)> = None; // (matched segments, line)
    let target: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') && !line.starts_with("[[") {
            header = line.trim_matches(|c| c == '[' || c == ']').trim().to_string();
            let hs: Vec<&str> = header.split('.').collect();
            let n = hs.iter().zip(&target).take_while(|(a, b)| a.trim() == **b).count();
            if n == hs.len() && n > 0 && best.map_or(true, |(m, _)| n > m) {
                best = Some((n, i + 1));
            }
        } else if let Some((k, _)) = line.split_once('=') {
            if line.starts_with('#') {
                continue;
            }
            let full = if header.is_empty() {
                k.trim().to_string()
            } else {
                format!("{header}.{}", k.trim())
            };
            if full == path {
                return Some(i + 1);
            }
        }
    }
    best.map(|(_, l)| l)
}

/// A case: a validated settings tree plus where it came from.
#[derive(Debug, Clone)]
pub struct Case {
    /// The settings.
    pub tree: SettingsTree,
    /// Directory relative paths (mesh files, output) resolve against.
    pub base_dir: std::path::PathBuf,
}

impl Default for Case {
    fn default() -> Self {
        Case::new()
    }
}

impl Case {
    /// Case with all defaults.
    pub fn new() -> Self {
        Case {
            tree: SettingsTree::new(case_schema()),
            base_dir: std::path::PathBuf::from("."),
        }
    }

    /// Set a setting by dotted path (named entries such as `boundaries.lid` are created on demand).
    pub fn set(&mut self, path: &str, value: Value) -> Result<()> {
        let segs: Vec<&str> = path.split('.').collect();
        // create the named entry for `<collection>.<name>.<field>` when it does not exist yet
        if segs.len() >= 3 && self.tree.get(&segs[..2].join(".")).is_err() {
            self.tree.add_named(segs[0], segs[1])?;
        }
        self.tree.set(path, value)
    }

    /// Get a setting by dotted path.
    pub fn get(&self, path: &str) -> Result<Value> {
        self.tree.get(path)
    }

    /// Validate a JSON value (a full or partial case) against the schema.
    pub fn check_json(value: &Value) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        collect_errors(&case_schema(), value, "", &mut out);
        out
    }

    /// Build a case from a JSON value; all schema errors are returned together.
    pub fn from_json(value: &Value) -> std::result::Result<Case, Vec<Diagnostic>> {
        let errs = Self::check_json(value);
        if !errs.is_empty() {
            return Err(errs);
        }
        let tree = SettingsTree::from_json(case_schema(), value).map_err(|e| {
            vec![Diagnostic {
                path: String::new(),
                line: None,
                message: e.to_string(),
            }]
        })?;
        let case = Case {
            tree,
            base_dir: std::path::PathBuf::from("."),
        };
        let sem = case.semantic_errors();
        if sem.is_empty() {
            Ok(case)
        } else {
            Err(sem)
        }
    }

    /// Parse TOML text; schema errors carry source lines.
    pub fn parse_toml(text: &str) -> std::result::Result<Case, Vec<Diagnostic>> {
        let v: Value = toml::from_str(text).map_err(|e| {
            let line = e
                .span()
                .map(|s| text[..s.start.min(text.len())].matches('\n').count() + 1);
            vec![Diagnostic {
                path: String::new(),
                line,
                message: e.message().to_string(),
            }]
        })?;
        Self::from_json(&v).map_err(|mut errs| {
            for e in &mut errs {
                e.line = locate_line(text, &e.path);
            }
            errs
        })
    }

    /// Parse JSON text.
    pub fn parse_json(text: &str) -> std::result::Result<Case, Vec<Diagnostic>> {
        let v: Value = serde_json::from_str(text).map_err(|e| {
            vec![Diagnostic {
                path: String::new(),
                line: Some(e.line()),
                message: e.to_string(),
            }]
        })?;
        Self::from_json(&v)
    }

    /// Load a `.toml` or `.json` case file.
    pub fn load(path: impl AsRef<Path>) -> std::result::Result<Case, Vec<Diagnostic>> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| {
            vec![Diagnostic {
                path: String::new(),
                line: None,
                message: format!("cannot read {}: {e}", path.display()),
            }]
        })?;
        let mut case = match path.extension().and_then(|e| e.to_str()) {
            Some("json") => Self::parse_json(&text)?,
            _ => Self::parse_toml(&text)?,
        };
        case.base_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Ok(case)
    }

    /// Serialise as TOML.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(&self.tree.to_json()).unwrap_or_else(|e| format!("# serialisation failed: {e}\n"))
    }

    /// Serialise as pretty JSON.
    pub fn to_json_string(&self) -> String {
        self.tree.to_json_string()
    }

    /// Save as TOML or JSON by extension.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let text = match path.extension().and_then(|e| e.to_str()) {
            Some("json") => self.to_json_string(),
            _ => self.to_toml(),
        };
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Cross-field checks that the per-field schema cannot express.
    pub fn semantic_errors(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let mut err = |path: &str, message: String| {
            out.push(Diagnostic {
                path: path.into(),
                line: None,
                message,
            })
        };
        let vec_ok = |v: &Value| v.as_array().is_some_and(|a| a.is_empty() || a.len() == 3);
        let p = "initial.velocity";
        if !self.tree.get(p).map(|v| vec_ok(&v)).unwrap_or(true) {
            err(p, "expected 3 components [x, y, z] (or an empty list for zero)".into());
        }
        if let Ok(names) = self.tree.names("boundaries") {
            for n in names {
                let p = format!("boundaries.{n}.velocity_value");
                if !self.tree.get(&p).map(|v| vec_ok(&v)).unwrap_or(true) {
                    err(&p, "expected 3 components [x, y, z] (or an empty list for zero)".into());
                }
            }
        }
        if let Ok(names) = self.tree.names("reports") {
            for n in names {
                for f in ["direction", "origin", "point"] {
                    let p = format!("reports.{n}.{f}");
                    if !self.tree.get(&p).map(|v| vec_ok(&v)).unwrap_or(true) {
                        err(
                            &p,
                            "expected 3 components [x, y, z] (or an empty list for the default)".into(),
                        );
                    }
                }
            }
        }
        let alg = self
            .tree
            .get("solver.algorithm")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        if matches!(alg.as_str(), "piso" | "pimple") {
            let dt = self.tree.get("solver.dt").ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
            let end = self
                .tree
                .get("solver.end_time")
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if end > 0.0 && dt > end {
                err("solver.dt", format!("time step {dt} exceeds end_time {end}"));
            }
        }
        let p = "physics.gravity";
        if !self.tree.get(p).map(|v| vec_ok(&v)).unwrap_or(true) {
            err(
                p,
                "expected 3 components [gx, gy, gz] (or an empty list for zero)".into(),
            );
        }
        for p in ["solver.bulk_direction", "physics.body_force"] {
            if !self.tree.get(p).map(|v| vec_ok(&v)).unwrap_or(true) {
                err(
                    p,
                    "expected 3 components [x, y, z] (or an empty list for the default)".into(),
                );
            }
        }
        if matches!(alg.as_str(), "piso" | "pimple" | "compressible") {
            let bv = self
                .tree
                .get("solver.bulk_velocity")
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if bv != 0.0 {
                err(
                    "solver.bulk_velocity",
                    format!("the bulk-velocity driver is available with the steady algorithms (simple, simplec) only, not with \"{alg}\""),
                );
            }
            let bf = self
                .tree
                .get("physics.body_force")
                .ok()
                .and_then(|v| v.as_array().map(|a| a.iter().any(|x| x.as_f64().unwrap_or(0.0) != 0.0)))
                .unwrap_or(false);
            if bf {
                err(
                    "physics.body_force",
                    format!("the uniform body force is available with the steady algorithms (simple, simplec) only, not with \"{alg}\""),
                );
            }
        }
        let energy_on = self
            .tree
            .get("energy.enabled")
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if energy_on && matches!(alg.as_str(), "piso" | "pimple") {
            err(
                "energy.enabled",
                "the energy equation is available with the steady algorithms (simple, simplec) only".into(),
            );
        }
        if self
            .tree
            .get("mesh.source")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            == Some("file".into())
            && self
                .tree
                .get("mesh.file")
                .ok()
                .and_then(|v| v.as_str().map(str::is_empty))
                .unwrap_or(true)
        {
            err("mesh.file", "mesh.source = \"file\" requires mesh.file".into());
        }
        if let Ok(names) = self.tree.names("convergence") {
            let mut monitors: Vec<String> = [
                "res_u",
                "res_p",
                "continuity",
                "courant",
                "dt",
                "res_T",
                "res_nut",
                "res_rho",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            monitors.extend(self.tree.names("reports").unwrap_or_default());
            for n in names {
                let p = format!("convergence.{n}.monitor");
                let m = self
                    .tree
                    .get(&p)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                if !monitors.contains(&m) {
                    out.push(Diagnostic {
                        path: p,
                        line: None,
                        message: format!("unknown monitor '{m}' (known: {})", monitors.join(", ")),
                    });
                }
            }
        }
        out.extend(crate::turb::check(self));
        out.extend(crate::comp::check(self));
        out.extend(crate::exprs::check_case_expressions(self));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
title = "cavity"

[mesh]
generator = "cavity_2d"
nx = 16
ny = 16

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
"#;

    #[test]
    fn good_case_parses_and_round_trips() {
        let c = Case::parse_toml(GOOD).unwrap();
        assert_eq!(c.tree.get("mesh.nx").unwrap(), serde_json::json!(16));
        assert_eq!(
            c.tree.get("boundaries.lid.velocity_value").unwrap(),
            serde_json::json!([1.0, 0.0, 0.0])
        );
        let again = Case::parse_toml(&c.to_toml()).unwrap();
        assert_eq!(again.tree.to_json(), c.tree.to_json());
        let from_json = Case::parse_json(&c.to_json_string()).unwrap();
        assert_eq!(from_json.tree.to_json(), c.tree.to_json());
    }

    #[test]
    fn errors_are_precise_complete_and_carry_lines() {
        let bad = r#"
[mesh]
nx = "sixteen"
nz = 3

[physics]
nu = "5 m"

[boundaries.lid]
velocity = "slippy"
velocity_value = [1.0, 0.0]

[solver]
algorithm = "simple"
alpha_u = 7.0
"#;
        let errs = Case::parse_toml(bad).unwrap_err();
        let by = |p: &str| {
            errs.iter()
                .find(|e| e.path == p)
                .unwrap_or_else(|| panic!("{p} not in {errs:?}"))
        };
        assert_eq!(by("mesh.nx").line, Some(3));
        assert!(by("mesh.nx").message.contains("expected integer"));
        assert!(by("mesh.nz").message.contains("unknown setting"));
        assert!(
            by("physics.nu").message.contains("dimension"),
            "{}",
            by("physics.nu").message
        );
        assert_eq!(by("boundaries.lid.velocity").line, Some(10));
        assert!(by("boundaries.lid.velocity").message.contains("not one of"));
        assert_eq!(by("solver.alpha_u").line, Some(15));
        // all problems at once; the 2-component vector is a semantic error reported once schema errors are fixed
        assert!(errs.len() >= 5, "{errs:?}");
        let shown = errs[0].to_string();
        assert!(shown.starts_with("line "), "{shown}");
    }

    #[test]
    fn semantic_checks_run_after_schema_checks() {
        let text =
            "[boundaries.lid]\nvelocity_value = [1.0, 0.0]\n[solver]\nalgorithm = \"piso\"\ndt = 5.0\nend_time = 1.0\n";
        let errs = Case::parse_toml(text).unwrap_err();
        assert!(errs.iter().any(|e| e.path == "boundaries.lid.velocity_value"));
        assert!(errs
            .iter()
            .any(|e| e.path == "solver.dt" && e.message.contains("exceeds")));
        let e = Case::parse_toml("[mesh]\nsource = \"file\"\n").unwrap_err();
        assert_eq!(e[0].path, "mesh.file");
    }

    #[test]
    fn toml_syntax_errors_report_the_line() {
        let e = Case::parse_toml("title = \"x\"\n[mesh\nnx = 3\n").unwrap_err();
        assert_eq!(e.len(), 1);
        assert!(e[0].line.is_some());
        let j = Case::parse_json("{\"title\": }").unwrap_err();
        assert_eq!(j[0].line, Some(1));
    }

    #[test]
    fn locate_line_prefers_exact_keys_then_table_headers() {
        let t = "a = 1\n[mesh]\nnx = 2\n[boundaries.lid]\nvelocity = \"x\"\n";
        assert_eq!(locate_line(t, "mesh.nx"), Some(3));
        assert_eq!(locate_line(t, "boundaries.lid"), Some(4));
        assert_eq!(locate_line(t, "boundaries.lid.missing"), Some(4));
        assert_eq!(locate_line(t, "nothing.here"), None);
    }

    #[test]
    fn save_and_load_files() {
        let dir = std::env::temp_dir().join(format!("rcfd_case_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = Case::parse_toml(GOOD).unwrap();
        for name in ["case.toml", "case.json"] {
            c.save(dir.join(name)).unwrap();
            let l = Case::load(dir.join(name)).unwrap();
            assert_eq!(l.tree.to_json(), c.tree.to_json());
            assert_eq!(l.base_dir, dir);
        }
        assert!(Case::load(dir.join("missing.toml")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
