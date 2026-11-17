//! The project trash commands (ADR 0736 P1, F-829 E-829a): `trash_run`, `trash_results`, `trash_case`, `list_trash`,
//! `restore_trash` and `empty_trash`, on `rcfd_project::trash`.
//!
//! The rules are the maintainer's answers of 2026-11-17: a run, or its results, is trashed only once it is finished,
//! failed or cancelled (1002 otherwise), and not while a run that is not yet finished, failed or cancelled has
//! `continue_from` it (1003, naming those runs in `data.runs`); a case file under `cases/` only while it still has
//! `base_hash` (1200 with `data.current_hash`), under its edit lock (1201). A restore onto an occupied path is 1200 with
//! `data.path`, and nothing moves. `empty_trash` removes items for good; nothing empties the trash by itself.
//!
//! Every accepted call but `list_trash` leaves one journal line (`trashed`, `restored`, `trash_emptied`) with the
//! command, the workspace's actor, the reason, the path and the token, and so one change-feed entry.
//!
//! Limit: the dependency check reads every run's `run.json` at the moment of the call; a run submitted with
//! `continue_from` between that check and the move finds its restart file gone and fails as any run whose source is
//! missing does.

use crate::journal::{append, put_list, reason_arg};
use crate::{
    err, project_error as perr, str_arg, text_len, u64_arg, RpcError, Workspace, BAD_STATE, CONFLICT, NOT_FOUND,
    PARAMS, REFUSED, TEXT_BUDGET, VALIDATION,
};
use rcfd_project::trash::{self, Item, Spec};
use rcfd_project::{ProjectError, RunRecord};
use serde_json::{json, Map, Value};
use std::time::Duration;

/// The command names, also each journal line's `who`.
pub const TRASH_RUN: &str = "trash_run";
/// See [`TRASH_RUN`].
pub const TRASH_RESULTS: &str = "trash_results";
/// See [`TRASH_RUN`].
pub const TRASH_CASE: &str = "trash_case";
/// See [`TRASH_RUN`].
pub const LIST_TRASH: &str = "list_trash";
/// See [`TRASH_RUN`].
pub const RESTORE_TRASH: &str = "restore_trash";
/// See [`TRASH_RUN`].
pub const EMPTY_TRASH: &str = "empty_trash";
/// The parts of a run's results `trash_results` moves.
pub const RESULT_PARTS: [&str; 2] = ["fields", "output"];

/// A trash error as a command error: an occupied path 1200 with `data.path` (project-relative), the rest as
/// [`crate::project_error`].
fn trash_error(ws: &Workspace, e: ProjectError) -> RpcError {
    match e {
        ProjectError::Exists { path } => {
            let rel = ws.rel(&path);
            RpcError {
                code: CONFLICT,
                message: format!("{rel} exists now; move it away first, nothing was restored"),
                data: Some(json!({"path": rel})),
            }
        }
        other => perr(other),
    }
}

/// An item as a reply object.
pub fn item_json(i: &Item) -> Value {
    serde_json::to_value(i).unwrap_or(Value::Null)
}

impl Workspace {
    fn trash_line(&self, kind: &str, who: &str, reason: Option<String>, fields: Map<String, Value>) {
        let origin = self.origin(who, reason);
        append(self.project.root(), kind, &origin, fields);
    }

    /// The checks of a run to trash: it is terminal, and no run that is not terminal continues from it.
    fn trashable_run(&self, args: &Value) -> Result<String, RpcError> {
        let (id, _) = self.run_arg(args, "run")?;
        let st = self.project.read_status(&id).map_err(perr)?;
        if !st.state.is_terminal() {
            return Err(err(
                BAD_STATE,
                format!(
                    "run {id} is {}; only a finished, failed or cancelled run is trashed (cancel it, or wait for it)",
                    crate::queries::state_name(st.state)
                ),
            ));
        }
        let mut waiting = Vec::new();
        for other in self.project.run_ids().map_err(perr)? {
            if other == id {
                continue;
            }
            let Ok(rec) = self.project.read_run(&other) else {
                continue;
            };
            if continues_from(&rec) != Some(id.as_str()) {
                continue;
            }
            let terminal = self.project.read_status(&other).is_ok_and(|s| s.state.is_terminal());
            if !terminal {
                waiting.push(other);
            }
        }
        if !waiting.is_empty() {
            return Err(RpcError {
                code: VALIDATION,
                message: format!(
                    "run {id} is the start of {} that {} not finished: {}; wait for {} or cancel {}",
                    if waiting.len() == 1 { "a run" } else { "runs" },
                    if waiting.len() == 1 { "is" } else { "are" },
                    waiting.join(", "),
                    if waiting.len() == 1 { "it" } else { "them" },
                    if waiting.len() == 1 { "it" } else { "them" },
                ),
                data: Some(json!({"runs": waiting})),
            });
        }
        Ok(id)
    }

    fn do_trash(&self, spec: &Spec<'_>, reason: Option<String>, run: Option<&str>) -> Result<Value, RpcError> {
        let item = trash::trash(self.project.root(), spec).map_err(|e| trash_error(self, e))?;
        let mut m = Map::new();
        m.insert("path".into(), json!(item.path));
        m.insert("token".into(), json!(item.token));
        m.insert("item".into(), json!(item.kind));
        if let Some(r) = run {
            m.insert("run".into(), json!(r));
        }
        if !item.parts.is_empty() {
            m.insert("parts".into(), json!(item.parts));
        }
        if let Some(h) = &item.hash {
            m.insert("hash".into(), json!(h));
        }
        m.insert("bytes".into(), json!(item.bytes));
        self.trash_line("trashed", spec.who, reason, m);
        let mut v = json!({"token": item.token, "kind": item.kind, "path": item.path, "bytes": item.bytes});
        if !item.parts.is_empty() {
            v["parts"] = json!(item.parts);
        }
        if let Some(h) = &item.hash {
            v["hash"] = json!(h);
        }
        Ok(v)
    }

    pub(crate) fn trash_run(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let id = self.trashable_run(args)?;
        let path = format!("runs/{id}");
        let spec = Spec {
            kind: "run",
            path: &path,
            parts: &[],
            base_hash: None,
            who: TRASH_RUN,
            reason: reason.as_deref(),
            actor: self.actor().map(|a| a.to_json()),
        };
        self.do_trash(&spec, reason.clone(), Some(&id))
    }

    pub(crate) fn trash_results(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let mut parts: Vec<&str> = Vec::new();
        for p in args.get("parts").and_then(Value::as_array).into_iter().flatten() {
            let Some(s) = p.as_str().filter(|s| RESULT_PARTS.contains(s)) else {
                return Err(err(PARAMS, format!("parts: each is \"fields\" or \"output\", not {p}")));
            };
            if !parts.contains(&s) {
                parts.push(s);
            }
        }
        if parts.is_empty() {
            return Err(err(PARAMS, "parts: give \"fields\", \"output\" or both"));
        }
        let id = self.trashable_run(args)?;
        let path = format!("runs/{id}");
        let dir = self.project.run_dir(&id).map_err(perr)?;
        let missing: Vec<&str> = parts.iter().copied().filter(|p| !dir.join(p).exists()).collect();
        if !missing.is_empty() {
            return Err(err(
                NOT_FOUND,
                format!(
                    "run {id} has no {}; nothing was moved",
                    missing
                        .iter()
                        .map(|m| format!("{m}/"))
                        .collect::<Vec<_>>()
                        .join(" and no ")
                ),
            ));
        }
        let spec = Spec {
            kind: "results",
            path: &path,
            parts: &parts,
            base_hash: None,
            who: TRASH_RESULTS,
            reason: reason.as_deref(),
            actor: self.actor().map(|a| a.to_json()),
        };
        self.do_trash(&spec, reason.clone(), Some(&id))
    }

    pub(crate) fn trash_case(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        let norm = self.rel(&path);
        let key = if cfg!(windows) {
            norm.to_lowercase()
        } else {
            norm.clone()
        };
        if !key.starts_with("cases/") {
            return Err(err(REFUSED, format!("trash_case moves files under cases/, not {norm}")));
        }
        if path.is_dir() {
            return Err(err(
                REFUSED,
                format!("{norm} is a folder; trash_case moves one case file"),
            ));
        }
        if !path.exists() {
            return Err(err(NOT_FOUND, format!("{norm}: no such file")));
        }
        let base = str_arg(args, "base_hash");
        let spec = Spec {
            kind: "case",
            path: &norm,
            parts: &[],
            base_hash: Some(base),
            who: TRASH_CASE,
            reason: reason.as_deref(),
            actor: self.actor().map(|a| a.to_json()),
        };
        self.do_trash(&spec, reason.clone(), None)
    }

    pub(crate) fn list_trash(&self, args: &Value) -> Result<Value, RpcError> {
        let all = trash::list(self.project.root()).map_err(perr)?;
        let bytes: u64 = all.items.iter().map(|i| i.bytes).sum();
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        let limit = u64_arg(args, "limit").unwrap_or(50) as usize;
        let skipped: Vec<Value> = all
            .skipped
            .iter()
            .take(20)
            .map(|i| json!({"path": self.rel(&i.path), "message": i.message}))
            .collect();
        let mut v = json!({"items": [], "bytes": bytes, "total": all.items.len(), "offset": offset,
            "next_offset": null, "skipped": skipped});
        let mut size = text_len(&v);
        let mut out = Vec::new();
        for i in all.items.iter().skip(offset).take(limit) {
            let row = item_json(i);
            size += text_len(&row) + 1;
            if size > TEXT_BUDGET && !out.is_empty() {
                break;
            }
            out.push(row);
        }
        let next = offset + out.len();
        if next < all.items.len() {
            v["next_offset"] = json!(next);
        }
        v["items"] = Value::Array(out);
        Ok(v)
    }

    pub(crate) fn restore_trash(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let token = str_arg(args, "token");
        if !trash::is_token(token) {
            return Err(err(
                PARAMS,
                format!("token: {token:?} is not a trash token (t-<yyyymmddThhmmss>-<4 hex>, see list_trash)"),
            ));
        }
        let item = trash::restore(self.project.root(), token, RESTORE_TRASH).map_err(|e| match e {
            ProjectError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                err(NOT_FOUND, format!("{token}: {e}; see list_trash"))
            }
            other => trash_error(self, other),
        })?;
        let mut m = Map::new();
        m.insert("path".into(), json!(item.path));
        m.insert("token".into(), json!(item.token));
        m.insert("item".into(), json!(item.kind));
        if item.kind != "case" {
            if let Some(id) = item.path.strip_prefix("runs/") {
                m.insert("run".into(), json!(id));
            }
        }
        if !item.parts.is_empty() {
            m.insert("parts".into(), json!(item.parts));
        }
        if let Some(h) = &item.hash {
            m.insert("hash".into(), json!(h));
        }
        m.insert("bytes".into(), json!(item.bytes));
        self.trash_line("restored", RESTORE_TRASH, reason, m);
        let mut v =
            json!({"token": item.token, "kind": item.kind, "path": item.path, "bytes": item.bytes, "restored": true});
        if !item.parts.is_empty() {
            v["parts"] = json!(item.parts);
        }
        if let Some(h) = &item.hash {
            v["hash"] = json!(h);
        }
        Ok(v)
    }

    pub(crate) fn empty_trash(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let tokens: Option<Vec<String>> = match args.get("tokens") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) => {
                let mut v = Vec::new();
                for t in a {
                    match t.as_str() {
                        Some(s) if trash::is_token(s) => v.push(s.to_string()),
                        _ => {
                            return Err(err(
                                PARAMS,
                                format!("tokens: {t} is not a trash token (see list_trash)"),
                            ))
                        }
                    }
                }
                Some(v)
            }
            Some(_) => return Err(err(PARAMS, "tokens: an array of trash tokens")),
        };
        let older = match args.get("older_than_days") {
            None | Some(Value::Null) => None,
            Some(v) => match v.as_f64().filter(|d| d.is_finite() && *d >= 0.0) {
                Some(d) => Some(Duration::from_secs_f64(d * 86_400.0)),
                None => return Err(err(PARAMS, "older_than_days: a number of days, 0 or more")),
            },
        };
        let removed =
            trash::empty(self.project.root(), tokens.as_deref(), older, EMPTY_TRASH).map_err(|e| match e {
                ProjectError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                    err(NOT_FOUND, format!("{e}; nothing was removed (see list_trash)"))
                }
                other => perr(other),
            })?;
        let bytes: u64 = removed.iter().map(|i| i.bytes).sum();
        let toks: Vec<&str> = removed.iter().map(|i| i.token.as_str()).collect();
        let mut m = Map::new();
        put_list(&mut m, "tokens", toks.iter().map(|t| json!(t)).collect());
        put_list(&mut m, "paths", removed.iter().map(|i| json!(i.path)).collect());
        m.insert("count".into(), json!(removed.len()));
        m.insert("bytes".into(), json!(bytes));
        self.trash_line("trash_emptied", EMPTY_TRASH, reason, m);
        Ok(json!({"removed": toks, "count": removed.len(), "bytes": bytes}))
    }
}

/// The run a run record continues from (`settings.continue_from`, ADR 0200 S1).
fn continues_from(rec: &RunRecord) -> Option<&str> {
    rec.settings.get("continue_from").and_then(Value::as_str)
}
