//! Runs of the project through its run service (ADR 0123, ADR 0133): one handle, [`RunService`], that the MCP server
//! and the Studio both start, hold, resume and cancel runs with (ADR 0185 E7), and the run commands of the MCP project
//! tools (`start_run`, `pause_run`, `resume_run`, `cancel_run`, `wait_run`; moved from `rcfd-mcp`, text unchanged).
//!
//! Every [`RunService`] call is the runner client's call. `start_run` can make its case first (`new_case`, ADR 0186 G5)
//! and apply the start plan of [`crate::plan`] (ADR 0173 D7); the journal line of a start (ADR 0173 W6) is a later item.

use crate::create::invalid;
use crate::plan::{Plan, PLAN_FOR_PROJECT_CASES};
use crate::queries::state_name;
use crate::{
    err, str_arg, u64_arg, RpcError, Workspace, BAD_STATE, INTERNAL, MAX_WAIT_S, NOT_FOUND, PARAMS, VALIDATION,
};
use rcfd_api::case::Case;
use rcfd_runner::client::Client;
use rcfd_runner::records::{RunSettings, RunState, RunStatus};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The project's run service as both clients reach it: the client of `rcfd-runner` behind one handle. Cloning it is
/// cheap; the clones talk to the same run service.
#[derive(Debug, Clone)]
pub struct RunService {
    client: Client,
}

impl RunService {
    /// The run service of `project`, whose runner is started from `runner_exe`.
    pub fn new(project: impl Into<PathBuf>, runner_exe: impl Into<PathBuf>) -> Self {
        RunService {
            client: Client::new(project, runner_exe),
        }
    }

    /// Like [`RunService::new`] with the runner executable from `RCFD_RUNNER_BIN`, else next to the current executable.
    pub fn with_default_runner(project: impl Into<PathBuf>) -> Self {
        RunService {
            client: Client::with_default_runner(project),
        }
    }

    /// The project folder.
    pub fn project(&self) -> &Path {
        self.client.project()
    }

    /// Does a live runner hold the project's lock?
    pub fn runner_running(&self) -> bool {
        self.client.runner_running()
    }

    /// Start the project's runner, detached, unless one is alive; waits up to 10 s for it to hold the lock.
    pub fn ensure_runner(&self) -> Result<(), String> {
        self.client.ensure_runner()
    }

    /// Queue a run of `case` without starting a runner; returns the run id.
    pub fn submit(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        self.client.submit(case, settings)
    }

    /// Queue a run and make sure a runner is alive; returns the run id.
    pub fn start(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        self.client.start(case, settings)
    }

    /// Every run with its status.
    pub fn list(&self) -> Result<Vec<(String, RunStatus)>, String> {
        self.client.list()
    }

    /// Status of run `id`.
    pub fn status(&self, id: &str) -> Result<RunStatus, String> {
        self.client.status(id)
    }

    /// Hold run `id`: a queued run is not launched, a running one stops between two steps.
    pub fn pause(&self, id: &str) -> Result<(), String> {
        self.client.pause(id)
    }

    /// Release a held run.
    pub fn resume(&self, id: &str) -> Result<(), String> {
        self.client.resume(id)
    }

    /// Cancel run `id` (a running solver stops cleanly and writes its results).
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.client.cancel(id)
    }

    /// Wait until run `id` is finished, failed or cancelled, at most `timeout`.
    pub fn wait(&self, id: &str, timeout: Duration) -> Result<RunStatus, String> {
        self.client.wait(id, timeout)
    }
}

impl Workspace {
    /// The case file at `path` read as `rcfd run` reads it, for its start plan.
    fn read_case(&self, path: &Path) -> Result<Case, RpcError> {
        let rel = self.rel(path);
        let bytes = rcfd_project::atomic::read(path).map_err(crate::project_error)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{rel} is not UTF-8 text ({e})")))?;
        let root = self.project.root();
        crate::plan::parse_case(text, path.parent().unwrap_or(root), Some(root))
            .map_err(|d| invalid(&format!("{rel}: not a valid case, so no start plan"), d))
    }

    pub(crate) fn start_run(&self, args: &Value) -> Result<Value, RpcError> {
        // ADR 0186 G5: a project case, or a new case made first; and whether the start plan applies
        let new_case = args.get("new_case").filter(|v| !v.is_null());
        if new_case.is_some() == args.get("case").is_some() {
            return Err(err(
                PARAMS,
                "start_run: give either case (a project case file) or new_case (a case to make first), not both",
            ));
        }
        let planned = match args.get("store").and_then(Value::as_str) {
            Some("plan") => true,
            Some("none") => false,
            None => new_case.is_some() || PLAN_FOR_PROJECT_CASES,
            Some(other) => return Err(err(PARAMS, format!("store: '{other}' is not plan or none"))),
        };
        let set_of = || -> Result<Vec<String>, RpcError> {
            let set: Vec<String> = args
                .get("set")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(String::from))
                .collect();
            if let Some(bad) = set.iter().find(|s| !s.contains('=')) {
                return Err(err(PARAMS, format!("set: '{bad}' is not path=value")));
            }
            Ok(set)
        };
        let (path, set, plan, created) = match new_case {
            Some(n) => {
                // made and checked, the entries applied and the plan made, all before anything is written
                let nc = self.prepare_case(n)?;
                let set = set_of()?;
                let plan = planned
                    .then(|| Plan::with_set(nc.case.clone(), &set))
                    .transpose()
                    .map_err(|m| err(VALIDATION, m))?;
                let created = self.write_case(&nc)?;
                let path = self.project.resolve(&nc.rel).map_err(crate::project_error)?;
                (path, set, plan, Some(created))
            }
            None => {
                let rel = str_arg(args, "case");
                let path = self.project.resolve(rel).map_err(crate::project_error)?;
                if !path.is_file() {
                    return Err(err(NOT_FOUND, format!("{rel}: no such case file")));
                }
                let set = set_of()?;
                let plan = match planned {
                    true => Some(Plan::with_set(self.read_case(&path)?, &set).map_err(|m| err(VALIDATION, m))?),
                    false => None,
                };
                (path, set, plan, None)
            }
        };
        // the plan's entries first, the run's own after them (the Studio's order: the last entry of a key applies)
        let set: Vec<String> = plan.iter().flat_map(Plan::entries).chain(set).collect();
        let settings = RunSettings {
            priority: args
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            threads: u64_arg(args, "threads").unwrap_or(1) as usize,
            mem_mb: u64_arg(args, "mem_mb"),
            set,
        };
        let id = self.runs.submit(&path, &settings).map_err(|e| err(INTERNAL, e))?;
        if let Err(e) = self.runs.ensure_runner() {
            return Err(RpcError {
                code: INTERNAL,
                message: format!("run {id} is queued, but the run service did not start: {e}"),
                data: Some(json!({"run": id})),
            });
        }
        let st = self.runs.status(&id).map_err(|e| err(INTERNAL, e))?;
        let mut out = json!({"run": id, "case": self.rel(&path), "state": state_name(st.state)});
        if let Some(c) = created {
            out["created"] = c;
        }
        if let Some(p) = &plan {
            out["store"] = json!({"applied": p.every.is_some(), "set": p.entries(), "snapshots": p.snapshots(),
                "estimated_bytes": p.estimated_bytes(), "note": p.note});
        }
        Ok(out)
    }

    pub(crate) fn control(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        let (id, _) = self.run_arg(args, "run")?;
        let (r, what) = match name {
            "pause_run" => (self.runs.pause(&id), "pause"),
            "resume_run" => (self.runs.resume(&id), "resume"),
            _ => (self.runs.cancel(&id), "cancel"),
        };
        r.map_err(|e| err(if e.contains("is already") { BAD_STATE } else { INTERNAL }, e))?;
        let st = self.runs.status(&id).map_err(|e| err(INTERNAL, e))?;
        Ok(json!({"run": id, "requested": what, "state": state_name(st.state)}))
    }

    pub(crate) fn wait_run(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, _) = self.run_arg(args, "run")?;
        let timeout = Duration::from_secs_f64(
            args.get("timeout_s")
                .and_then(Value::as_f64)
                .unwrap_or(30.0)
                .clamp(0.0, MAX_WAIT_S),
        );
        let t0 = Instant::now();
        let status = |id: &str| self.runs.status(id).map_err(|e| err(INTERNAL, e));
        let first = status(&id)?;
        let from = match args.get("from") {
            Some(f) => serde_json::from_value::<RunState>(f.clone()).map_err(|e| err(PARAMS, format!("from: {e}")))?,
            None => first.state,
        };
        let mut st = first;
        loop {
            if st.state != from || st.state.is_terminal() || t0.elapsed() >= timeout {
                break;
            }
            std::thread::sleep(Duration::from_millis(50).min(timeout.saturating_sub(t0.elapsed())));
            st = status(&id)?;
        }
        Ok(
            json!({"run": id, "from": state_name(from), "state": state_name(st.state),
            "changed": st.state != from, "terminal": st.state.is_terminal(), "waited_s": t0.elapsed().as_secs_f64(),
            "status": serde_json::to_value(&st).unwrap_or(Value::Null)}),
        )
    }
}
