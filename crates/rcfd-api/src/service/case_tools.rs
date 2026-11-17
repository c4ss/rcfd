//! The service methods of ADR 0737 (F-837), on only with `rcfd serve --ui` (Q1 (a)): `case.facts` (P1, P2: the facts
//! of a session's case and the run-time plan of a run of it), `mesh.refine` (P3: Refine x2 as setting edits) and
//! `case.save` (P4: the session's case written to one file outside a project).

use super::{code, lock, RpcError, RunState, Service, Session};
use crate::case_save::{self, Form, SaveError};
use crate::facts::{self, FactsError, InText, RunTime};
use crate::refine::{self, Refusal};
use crate::service::Rpc;
use serde_json::{json, Value};
use std::sync::Arc;

/// The parameters of `p` must be an object whose members are all in `allowed`.
fn members(method: &str, p: &Value, allowed: &[&str]) -> Rpc<()> {
    let Some(o) = p.as_object() else {
        return Err(RpcError::params(format!("{method} takes an object of parameters")));
    };
    if let Some(k) = o.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(RpcError::params(format!(
            "{method}: unknown parameter '{k}' (it takes {})",
            allowed.join(", ")
        )));
    }
    Ok(())
}

/// The name of a session's case in messages: the file it was made from, else `the session's case`.
fn case_name(s: &Session) -> String {
    s.source
        .get("path")
        .and_then(Value::as_str)
        .map_or_else(|| "the session's case".to_string(), String::from)
}

fn facts_error(e: FactsError) -> RpcError {
    match e {
        FactsError::Invalid(d) => RpcError::validation(d),
        FactsError::TooLarge { cells, limit } => {
            RpcError::new(code::LIMIT, format!("mesh has {cells} cells, limit is {limit}"))
        }
    }
}

impl Service {
    /// `case.facts {session, run_time?}`: the facts of the session's case as it is now (its unsaved changes included),
    /// the plan of a run of it with `run_time` (default: the automatic step at Courant 0.5, adaptive correctors), and
    /// the runner's memory estimate with its default budgets (Q7 (a): a session has no project).
    pub(super) fn case_facts_call(&self, p: &Value) -> Rpc {
        members("case.facts", p, &["session", "run_time"])?;
        let s = self.session(p)?;
        let rt = match p.get("run_time") {
            None | Some(Value::Null) => RunTime::default(),
            Some(v) => RunTime::from_json(v).map_err(RpcError::params)?,
        };
        let case = lock(&s.inner).case.clone();
        self.check_case_paths(&case)?;
        let f = facts::case_facts(&case, InText::NotDefault, Some(self.cfg.max_cells as u64)).map_err(facts_error)?;
        let mut out = f.to_json();
        out["run_time"] = facts::plan_json(&case_name(&s), &f, &rt);
        out["memory"] = facts::memory_json(f.cells, &rcfd_runner::runner::Config::default(), "runner_defaults");
        Ok(out)
    }

    /// `mesh.refine {session, factor, apply?}`: the refinement of the session's generator mesh; with `apply: true` its
    /// edits are set in the session (each a `settings.set` with its `base_value`, so they are unsaved changes in its
    /// journal, and a concurrent change of a count is 1007 with nothing set).
    pub(super) fn mesh_refine_call(self: &Arc<Self>, p: &Value) -> Rpc {
        members("mesh.refine", p, &["session", "factor", "apply"])?;
        let s = self.session(p)?;
        let factor = match p.get("factor") {
            None | Some(Value::Null) => 2,
            Some(v) => v
                .as_u64()
                .ok_or_else(|| RpcError::params("parameter 'factor' must be the integer 2"))?,
        };
        let apply = match p.get("apply") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(RpcError::params("parameter 'apply' must be a boolean")),
        };
        let case = lock(&s.inner).case.clone();
        self.check_case_paths(&case)?;
        let r = match refine::count_refinement(&case, factor, Some(self.cfg.max_cells as u64)) {
            Ok(r) => r,
            Err(Refusal::Factor(m)) => return Err(RpcError::params(m)),
            Err(Refusal::Refused(m)) => return Err(RpcError::new(code::LIMIT, m)),
            Err(Refusal::Invalid(d)) => return Err(RpcError::validation(d)),
        };
        if apply && matches!(s.state(), RunState::Running | RunState::Paused) {
            return Err(RpcError::new(code::BAD_STATE, "stop the run before changing settings"));
        }
        let mut out = r.to_json();
        out["memory_after"] = facts::memory_json(
            r.cells_after,
            &rcfd_runner::runner::Config::default(),
            "runner_defaults",
        );
        out["over_limit"] = json!((r.cells_after > self.cfg.max_cells as u64).then_some(self.cfg.max_cells));
        if apply {
            // ADR 0736 P2: each count is set with its value as read (`base_value`), so a writer that changed one in
            // between gets 1007 instead of being overwritten; the counts set before it are put back (with their new
            // value as the base), so the refine is applied whole or not at all
            for (k, e) in r.edits.iter().enumerate() {
                let set = json!({"session": s.id, "path": e.path, "value": e.after, "base_value": e.before});
                if let Err(err) = self.call("settings.set", &set) {
                    for d in r.edits[..k].iter().rev() {
                        let back = json!({"session": s.id, "path": d.path, "value": d.before, "base_value": d.after});
                        let _ = self.call("settings.set", &back);
                    }
                    return Err(err);
                }
            }
        }
        out["applied"] = json!(apply);
        Ok(out)
    }

    /// `case.save {session, path, base_hash, form?}`: the session's case written to `path` (sandboxed, outside any
    /// project) only if the file still has `base_hash` (null: no file there yet).
    pub(super) fn case_save_call(&self, p: &Value) -> Rpc {
        members("case.save", p, &["session", "path", "base_hash", "form"])?;
        let s = self.session(p)?;
        let path = self.sandbox_path(super::s_param(p, "path")?)?;
        let base = match p.get("base_hash") {
            Some(Value::String(h)) => Some(h.as_str()),
            Some(Value::Null) => None,
            Some(_) => return Err(RpcError::params("parameter 'base_hash' must be a hash string or null")),
            None => {
                return Err(RpcError::params(
                    "missing parameter 'base_hash' (the file's hash; null when there is no file yet)",
                ))
            }
        };
        if p.get("form").is_some_and(|f| !f.is_string() && !f.is_null()) {
            return Err(RpcError::params("parameter 'form' must be whole or lines"));
        }
        let form = match p.get("form").and_then(Value::as_str) {
            None | Some("whole") => Form::Whole,
            Some("lines") => Form::Lines,
            Some(other) => return Err(RpcError::params(format!("form: '{other}' is not whole or lines"))),
        };
        let case = lock(&s.inner).case.clone();
        if case.to_toml().starts_with("# serialisation failed") {
            return Err(RpcError::new(
                code::VALIDATION,
                "the session's case does not serialise as TOML",
            ));
        }
        match case_save::save(&path, base, &case, form) {
            Ok(saved) => Ok(json!({
                "path": crate::session_journal::absolute(&saved.path).display().to_string(),
                "old_hash": saved.old_hash,
                "new_hash": saved.new_hash,
                "form": saved.form.name(),
            })),
            Err(e) => {
                let message = e.to_string();
                Err(match e {
                    SaveError::Conflict { current } => RpcError {
                        code: code::FILE_CONFLICT,
                        message,
                        data: Some(json!({"current_hash": current})),
                    },
                    SaveError::InProject(root) => RpcError {
                        code: code::LIMIT,
                        message,
                        data: Some(json!({"project": root.display().to_string(), "use": "edit_case"})),
                    },
                    SaveError::Extension(_) => RpcError::params(message),
                    SaveError::Busy(_) => RpcError::new(code::BAD_STATE, message),
                    SaveError::Failed(_) => RpcError::new(code::INTERNAL, message),
                })
            }
        }
    }
}
