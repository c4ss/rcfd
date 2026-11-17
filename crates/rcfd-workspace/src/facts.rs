//! The project commands of ADR 0737 (F-837): `case_facts {case}` (P1: a project case's facts, the plan of a run with
//! the default run-time settings, the runner's memory estimate with the project's budgets) and `refine_case_mesh {case,
//! factor, base_hash, reason?}` (P3: Refine x2 written through `edit_case`'s rule). `start_run {run_time, dry_run}`
//! (P2) is in [`crate::runs`].

use crate::journal::reason_arg;
use crate::{err, str_arg, RpcError, Workspace, NOT_FOUND, PARAMS, REFUSED, VALIDATION};
use rcfd_api::case::Case;
use rcfd_api::facts::{self, CaseFacts, FactsError, InText, RunTime};
use rcfd_api::refine::{self, Refusal};
use serde_json::{json, Value};
use std::path::Path;

/// A facts error as a command error: 1003 with the located diagnostics; 1004 over a cell limit.
pub(crate) fn facts_error(rel: &str, e: FactsError) -> RpcError {
    match e {
        FactsError::Invalid(d) => crate::create::invalid(&format!("{rel}: the case does not build"), d),
        FactsError::TooLarge { cells, limit } => {
            err(REFUSED, format!("{rel}: mesh has {cells} cells, limit is {limit}"))
        }
    }
}

impl Workspace {
    /// The case file at `path` (project-resolved) as `rcfd run` reads it, with the parsed document of its text.
    pub(crate) fn read_case_text(&self, path: &Path) -> Result<(Case, Value), RpcError> {
        let rel = self.rel(path);
        let bytes = rcfd_project::atomic::read(path).map_err(crate::project_error)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{rel} is not UTF-8 text ({e})")))?;
        let root = self.project.root();
        let case = crate::plan::parse_case(text, path.parent().unwrap_or(root), Some(root))
            .map_err(|d| crate::create::invalid(&format!("{rel}: not a valid case"), d))?;
        Ok((case, facts::text_tree(text).unwrap_or(Value::Null)))
    }

    fn project_case(&self, args: &Value) -> Result<(String, std::path::PathBuf), RpcError> {
        let rel = str_arg(args, "case");
        if rel.is_empty() {
            return Err(err(PARAMS, "case: a project-relative case file"));
        }
        let path = self.project.resolve(rel).map_err(crate::project_error)?;
        if !path.is_file() {
            return Err(err(NOT_FOUND, format!("{rel}: no such case file")));
        }
        Ok((self.rel(&path), path))
    }

    /// The runner's budgets of this project (`.rcfd/runner-config.json` over the defaults), or the defaults when the file
    /// does not read.
    pub(crate) fn budgets(&self) -> (rcfd_runner::runner::Config, &'static str) {
        match rcfd_runner::runner::Config::load(self.project.root()) {
            Ok(c) => (c, "project"),
            Err(_) => (rcfd_runner::runner::Config::default(), "runner_defaults"),
        }
    }

    /// The facts of a case and the JSON of `case_facts` (the plan with `rt`, the memory with the project's budgets).
    pub(crate) fn facts_reply(&self, rel: &str, f: &CaseFacts, rt: &RunTime) -> Value {
        let mut out = f.to_json();
        out["case"] = json!(rel);
        out["run_time"] = facts::plan_json(rel, f, rt);
        let (b, from) = self.budgets();
        out["memory"] = facts::memory_json(f.cells, &b, from);
        out
    }

    pub(crate) fn case_facts(&self, args: &Value) -> Result<Value, RpcError> {
        let (rel, path) = self.project_case(args)?;
        let rt = match args.get("run_time") {
            None | Some(Value::Null) => RunTime::default(),
            Some(v) => RunTime::from_json(v).map_err(|m| err(PARAMS, m))?,
        };
        let (case, raw) = self.read_case_text(&path)?;
        let f = facts::case_facts(&case, InText::Text(&raw), None).map_err(|e| facts_error(&rel, e))?;
        Ok(self.facts_reply(&rel, &f, &rt))
    }

    pub(crate) fn refine_case_mesh(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let (rel, path) = self.project_case(args)?;
        let factor = match args.get("factor") {
            None | Some(Value::Null) => 2,
            Some(v) => v.as_u64().ok_or_else(|| err(PARAMS, "factor: the integer 2"))?,
        };
        let base = str_arg(args, "base_hash");
        if base.is_empty() {
            return Err(err(PARAMS, "base_hash: the hash from read_project_file"));
        }
        let (case, _) = self.read_case_text(&path)?;
        let r = match refine::count_refinement(&case, factor, None) {
            Ok(r) => r,
            Err(Refusal::Factor(m)) => return Err(err(PARAMS, m)),
            Err(Refusal::Refused(m)) => return Err(err(REFUSED, format!("{rel}: {m}"))),
            Err(Refusal::Invalid(d)) => {
                return Err(crate::create::invalid(&format!("{rel}: the case does not build"), d))
            }
        };
        let mut edit = json!({"path": rel, "base_hash": base, "set": r.set()});
        if let Some(why) = reason {
            edit["reason"] = json!(why);
        }
        let written = self.edit_case(&edit)?;
        let mut out = r.to_json();
        let (b, from) = self.budgets();
        out["memory_after"] = facts::memory_json(r.cells_after, &b, from);
        out["case"] = json!(rel);
        out["written"] = written;
        Ok(out)
    }
}
