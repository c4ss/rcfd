//! The command `new_case` (ADR 0173 R1 and W3, ADR 0186 G3): a new case file `cases/<name>.<ext>` made from exactly one
//! source (a shipped example with the data files it reads, a built-in template, TOML text, a settings object or a copy
//! of a project case), validated through `rcfd-api` before anything is written, and created without ever replacing a
//! file ([`crate::cases::create_with`]). `start_run` with `new_case` makes the case the same way and then runs it.
//!
//! A creation leaves one `case_created` line in the project journal, with the caller's `reason` (ADR 0188 J4, J5).

use crate::cases::{check_name, create_by};
use crate::examples::EXAMPLES;
use crate::journal::reason_arg;
use crate::templates::TEMPLATES;
use crate::{err, project_error, str_arg, RpcError, Workspace, INTERNAL, NOT_FOUND, PARAMS, REFUSED, VALIDATION};
use rcfd_api::case::{Case, Diagnostic};
use rcfd_project::records::hash_bytes;
use serde_json::{json, Value};

/// The sources of a new case, one of which `new_case` takes.
pub const SOURCES: [&str; 5] = ["example", "template", "toml", "settings", "copy"];

/// A new case checked and validated, not yet written.
#[derive(Debug, Clone)]
pub(crate) struct NewCase {
    /// `cases/<name>.<ext>`.
    pub(crate) rel: String,
    /// The bytes of the case file.
    pub(crate) bytes: Vec<u8>,
    /// Data files written next to it (`cases/<file name>`, bytes).
    pub(crate) companions: Vec<(String, Vec<u8>)>,
    /// Where it comes from (`example <source>`, `template <name>`, `toml`, `settings`, `copy <path>`).
    pub(crate) from: String,
    /// The case as validated (for the start plan).
    pub(crate) case: Case,
}

/// A validation refusal (1003) with its located diagnostics.
pub(crate) fn invalid(what: &str, diags: Vec<Diagnostic>) -> RpcError {
    RpcError {
        code: VALIDATION,
        message: format!(
            "{what}: {}",
            diags
                .iter()
                .map(|d| match d.line {
                    Some(l) if d.path.is_empty() => format!("line {l}: {}", d.message),
                    Some(l) => format!("{} (line {l}): {}", d.path, d.message),
                    None => format!("{}: {}", d.path, d.message),
                })
                .collect::<Vec<_>>()
                .join("; ")
        ),
        data: Some(json!({"diagnostics": diags})),
    }
}

/// True when `text` is read as JSON by `rcfd run` (it starts with `{`, ADR 0149); else TOML.
pub(crate) fn is_json_text(text: &str) -> bool {
    text.trim_start().starts_with('{')
}

impl Workspace {
    /// The files directly under `cases/` as project-relative paths (`cases/<file name>`), for the name check.
    fn case_files(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.project.root().join("cases")) else {
            return Vec::new();
        };
        rd.filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .map(|n| format!("cases/{n}"))
            .collect()
    }

    /// Parse case `text` as it will be read from `cases/` (JSON or TOML by `json`), confined to the project.
    fn parse_in_cases(&self, text: &str, json: bool) -> Result<Case, Vec<Diagnostic>> {
        let root = self.project.root();
        let dir = root.join("cases");
        if json {
            Case::parse_json_with(text, &dir, Some(root))
        } else {
            Case::parse_toml_with(text, &dir, Some(root))
        }
    }

    /// Check and validate the new case of `new_case`'s arguments; nothing is written.
    pub(crate) fn prepare_case(&self, args: &Value) -> Result<NewCase, RpcError> {
        let given: Vec<&str> = SOURCES
            .iter()
            .copied()
            .filter(|k| args.get(*k).is_some_and(|v| !v.is_null()))
            .collect();
        if given.len() != 1 {
            return Err(err(
                PARAMS,
                format!(
                    "new_case: give exactly one of example, template, toml, settings or copy (given: {})",
                    if given.is_empty() {
                        "none".to_string()
                    } else {
                        given.join(", ")
                    }
                ),
            ));
        }
        let source = given[0];
        let text_arg = |k: &str| {
            args[k]
                .as_str()
                .ok_or_else(|| err(PARAMS, format!("new_case: {k} must be a string")))
        };
        // the source's file extension, before the name is checked
        let ext = match source {
            "example" => {
                let src = text_arg("example")?;
                let ex = EXAMPLES.iter().find(|e| e.source == src).ok_or_else(|| RpcError {
                    code: PARAMS,
                    message: format!("new_case: unknown example '{src}'"),
                    data: Some(json!({"examples": EXAMPLES.iter().map(|e| e.source).collect::<Vec<_>>()})),
                })?;
                ex.ext().to_string()
            }
            "template" => {
                let t = text_arg("template")?;
                if !TEMPLATES.iter().any(|x| x.0 == t) {
                    return Err(RpcError {
                        code: PARAMS,
                        message: format!("new_case: unknown template '{t}'"),
                        data: Some(json!({"templates": TEMPLATES.iter().map(|x| x.0).collect::<Vec<_>>()})),
                    });
                }
                "toml".to_string()
            }
            "toml" => "toml".to_string(),
            "settings" => "json".to_string(),
            _ => {
                let rel = text_arg("copy")?;
                let path = self.project.resolve(rel).map_err(project_error)?;
                match path.extension().and_then(|e| e.to_str()) {
                    Some(e @ ("toml" | "json")) => e.to_string(),
                    _ => {
                        return Err(err(
                            VALIDATION,
                            format!("new_case: copy: {rel} is not a .toml or .json case file"),
                        ))
                    }
                }
            }
        };
        let name = str_arg(args, "name");
        let rel = check_name(name, &ext, &self.case_files())
            .map_err(|m| err(REFUSED, format!("new_case: name '{name}': {m}")))?;
        let refused = |what: String| move |d: Vec<Diagnostic>| invalid(&what, d);
        let (bytes, companions, from, case) = match source {
            "example" => {
                let src = str_arg(args, "example");
                let ex = EXAMPLES.iter().find(|e| e.source == src).expect("checked above");
                let case = ex
                    .parse()
                    .ok_or_else(|| err(INTERNAL, format!("the shipped example {src} does not parse")))?;
                let companions = ex
                    .companions
                    .iter()
                    .map(|c| (format!("cases/{}", c.name()), c.bytes().to_vec()))
                    .collect();
                (ex.bytes().to_vec(), companions, format!("example {src}"), case)
            }
            "template" => {
                let t = str_arg(args, "template");
                let text = TEMPLATES.iter().find(|x| x.0 == t).expect("checked above").1;
                let case = self
                    .parse_in_cases(text, false)
                    .map_err(refused(format!("{rel}: the template {t} is not a valid case")))?;
                (text.as_bytes().to_vec(), Vec::new(), format!("template {t}"), case)
            }
            "toml" => {
                let text = text_arg("toml")?;
                let case = self
                    .parse_in_cases(text, false)
                    .map_err(refused(format!("{rel}: the toml text is not a valid case")))?;
                (text.as_bytes().to_vec(), Vec::new(), "toml".to_string(), case)
            }
            "settings" => {
                if !args["settings"].is_object() {
                    return Err(err(PARAMS, "new_case: settings must be an object"));
                }
                let mut text =
                    serde_json::to_string_pretty(&args["settings"]).map_err(|e| err(INTERNAL, e.to_string()))?;
                text.push('\n');
                let case = self
                    .parse_in_cases(&text, true)
                    .map_err(refused(format!("{rel}: the settings are not a valid case")))?;
                (text.into_bytes(), Vec::new(), "settings".to_string(), case)
            }
            _ => {
                let src = str_arg(args, "copy");
                let path = self.project.resolve(src).map_err(project_error)?;
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(err(NOT_FOUND, format!("new_case: copy: {src}: no such file")))
                    }
                    Err(e) => return Err(err(INTERNAL, format!("new_case: copy: {src}: {e}"))),
                };
                let text = std::str::from_utf8(&bytes)
                    .map_err(|e| err(VALIDATION, format!("new_case: copy: {src} is not UTF-8 text ({e})")))?;
                let case = self.parse_in_cases(text, is_json_text(text)).map_err(refused(format!(
                    "{rel}: the copy of {src} is not a valid case in cases/"
                )))?;
                let from = format!("copy {}", self.rel(&path));
                (bytes, Vec::new(), from, case)
            }
        };
        Ok(NewCase {
            rel,
            bytes,
            companions,
            from,
            case,
        })
    }

    /// Write a prepared new case (never replacing a file) for command `who` with `reason`; the `new_case` result object.
    pub(crate) fn write_case(&self, nc: &NewCase, who: &str, reason: Option<String>) -> Result<Value, RpcError> {
        let origin = self.origin(who, reason);
        let data = create_by(
            &self.project,
            &nc.rel,
            &nc.bytes,
            &nc.companions,
            &origin,
            Some(&nc.from),
        )
        .map_err(|m| {
            let code = if m.contains("exists") { REFUSED } else { INTERNAL };
            err(code, format!("new_case: {m}"))
        })?;
        Ok(json!({
            "path": nc.rel,
            "hash": hash_bytes(&nc.bytes),
            "bytes": nc.bytes.len(),
            "from": nc.from,
            "data_files": data.iter().map(|(p, w)| json!({"path": p, "written": w})).collect::<Vec<_>>(),
        }))
    }

    pub(crate) fn new_case(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let nc = self.prepare_case(args)?;
        self.write_case(&nc, "new_case", reason)
    }
}
