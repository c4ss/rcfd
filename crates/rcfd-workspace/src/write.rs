//! The write rule of a project folder (ADR 0173 D6, ADR 0185 E2): every write of an existing project file is a
//! base-hash compare-and-replace under the file's edit lock (`rcfd_project::atomic::replace_if_hash`), with one
//! `.rcfd/edits.jsonl` line per accepted write naming the command (`who`). A case changed by setting values keeps its
//! comments and layout ([`crate::case_text`]).
//!
//! The commands `edit_case` and `write_note` of the MCP project tools (moved from `rcfd-mcp`, text unchanged). An
//! `edit_case` that sets a `mesh.*` setting also builds and checks the edited mesh before it writes, and returns the
//! check as `mesh` (ADR 0187 M7); without one its result and bytes are as before.

use crate::case_text::apply_sets;
use crate::meshes::{check_object, is_mesh_key};
use crate::{
    err, project_error as perr, str_arg, text_len, RpcError, Workspace, CONFLICT, PARAMS, REFUSED, TEXT_BUDGET,
    VALIDATION,
};
use rcfd_api::case::{Case, Diagnostic};
use rcfd_mesh::quality::check_mesh;
use rcfd_project::atomic;
use rcfd_project::records::{hash_bytes, hash_matches};
use serde_json::{json, Map, Value};

impl Workspace {
    pub(crate) fn edit_case(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        let norm = self.rel(&path);
        // names differing only in case are one file on Windows (as the edit lock's key)
        let key = if cfg!(windows) {
            norm.to_lowercase()
        } else {
            norm.clone()
        };
        if key.starts_with("runs/") || key.starts_with(".rcfd/") || key == "rcfd-project.json" {
            return Err(err(
                REFUSED,
                format!("edit_case does not write {norm}: run records, .rcfd/ and the manifest are not case files"),
            ));
        }
        let is_json = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => false,
            Some("json") => true,
            _ => {
                return Err(err(
                    VALIDATION,
                    format!("{norm}: edit_case edits .toml or .json case files"),
                ))
            }
        };
        let base = str_arg(args, "base_hash");
        let set = args["set"].as_object().cloned().unwrap_or_default();
        let dry_run = args.get("dry_run").and_then(Value::as_bool).unwrap_or(false);
        if set.is_empty() && !dry_run {
            return Err(err(PARAMS, "set: give at least one {setting path: value} (an empty set only with dry_run, which checks the file as it is)"));
        }
        let bytes = atomic::read(&path).map_err(perr)?;
        let old_hash = hash_bytes(&bytes);
        if !hash_matches(base, &bytes) {
            return Err(RpcError {
                code: CONFLICT,
                message: format!("{norm}: changed since it was read (now {old_hash}); read it again"),
                data: Some(json!({"current_hash": old_hash})),
            });
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{norm} is not UTF-8 text ({e})")))?;
        let root = self.project.root();
        let base_dir = path.parent().unwrap_or(root).to_path_buf();
        let parse = |t: &str| {
            if is_json {
                Case::parse_json_with(t, &base_dir, Some(root))
            } else {
                Case::parse_toml_with(t, &base_dir, Some(root))
            }
        };
        let invalid = |what: String, diags: Vec<Diagnostic>| RpcError {
            code: VALIDATION,
            message: format!(
                "{norm}: {what}: {}",
                diags
                    .iter()
                    .map(|d| format!("{}: {}", d.path, d.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            data: Some(json!({"diagnostics": diags})),
        };
        let orig = parse(text).map_err(|d| invalid("the file is not a valid case before the edit".into(), d))?;
        let mut expected = orig.clone();
        for (k, v) in &set {
            expected.set(k, v.clone()).map_err(|e| {
                invalid(
                    format!("{k} = {v} refused"),
                    vec![Diagnostic {
                        path: k.clone(),
                        line: None,
                        message: e.to_string(),
                    }],
                )
            })?;
        }
        let sem = expected.semantic_errors();
        if !sem.is_empty() {
            return Err(invalid("the edited case is not valid".into(), sem));
        }
        // ADR 0187 M7: a mesh setting changed: the edited mesh is confined, built and checked before anything is written
        let mesh_check = if set.keys().any(|k| is_mesh_key(k)) {
            let (mesh, _) = self.build_case_mesh(&norm, &expected)?;
            let q = check_mesh(&mesh);
            Some((mesh, q))
        } else {
            None
        };
        let with_mesh = |mut v: Value| {
            if let Some((mesh, q)) = &mesh_check {
                let budget = TEXT_BUDGET.saturating_sub(text_len(&v) + 16);
                v["mesh"] = check_object(mesh, q, budget);
            }
            v
        };
        let want = expected.tree.to_json();
        let minimal = apply_sets(text, is_json, set.iter().map(|(k, v)| (k.as_str(), v)));
        let (new_text, form) = match minimal.filter(|t| parse(t).is_ok_and(|c| c.tree.to_json() == want)) {
            Some(t) => (t, "minimal"),
            // ADR 0135 amendment 3 item 3: never rewrite the whole case with every default spelled out
            None => {
                let keys: Vec<&String> = set.keys().collect();
                return Err(RpcError {
                    code: VALIDATION,
                    message: format!(
                        "{norm}: edit_case cannot change {keys:?} in the file's own lines (for example a value written over several lines or set through an inline table); the values are valid, but nothing was written: edit the file by hand"
                    ),
                    data: Some(json!({"settings": keys, "values": set})),
                });
            }
        };
        let values: Map<String, Value> = set
            .keys()
            .map(|k| (k.clone(), expected.get(k).unwrap_or(Value::Null)))
            .collect();
        let new_hash = hash_bytes(new_text.as_bytes());
        if dry_run {
            return Ok(with_mesh(
                json!({"path": norm, "written": false, "valid": true, "form": form,
                "old_hash": old_hash, "new_hash": new_hash, "values": values}),
            ));
        }
        let r = atomic::replace_if_hash(root, &norm, Some(base), new_text.as_bytes(), "edit_case").map_err(perr)?;
        Ok(with_mesh(
            json!({"path": r.rel, "written": true, "valid": true, "form": form, "old_hash": r.old_hash,
            "new_hash": r.new_hash, "values": values}),
        ))
    }

    pub(crate) fn write_note(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        let norm = self.rel(&path);
        if !norm.starts_with("notes/") || norm.len() <= "notes/".len() {
            return Err(err(REFUSED, format!("write_note writes under notes/ only, not {norm}")));
        }
        if path.is_dir() {
            return Err(err(REFUSED, format!("{norm} is a directory")));
        }
        let base = args.get("base_hash").and_then(Value::as_str);
        let r = atomic::replace_if_hash(
            self.project.root(),
            &norm,
            base,
            str_arg(args, "text").as_bytes(),
            "write_note",
        )
        .map_err(perr)?;
        Ok(
            json!({"path": r.rel, "created": r.old_hash.is_none(), "old_hash": r.old_hash,
            "new_hash": r.new_hash}),
        )
    }
}
