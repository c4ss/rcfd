//! Runs of the project through its run service (ADR 0123, ADR 0133): one handle, [`RunService`], that the MCP server
//! and the Studio both start, hold, resume and cancel runs with (ADR 0185 E7), and the run commands of the MCP project
//! tools (`start_run`, `pause_run`, `resume_run`, `cancel_run`, `wait_run`; moved from `rcfd-mcp`, text unchanged).
//!
//! Every [`RunService`] call is the runner client's call. A submission and an accepted pause, resume or cancel request
//! also leave one line in the project journal (`run_submitted`, `run_control`), with the service's actor when it has one
//! and the command and reason of [`RunService::by`] (ADR 0188 J3, J5). `start_run` can make its case first (`new_case`,
//! ADR 0186 G5) and apply the start plan of [`crate::plan`] (ADR 0173 D7).

use crate::create::invalid;
use crate::journal::{append, put_list, reason_arg, Actor, Origin};
use crate::plan::{Plan, PLAN_FOR_PROJECT_CASES};
use crate::queries::state_name;
use crate::{
    err, str_arg, u64_arg, RpcError, Workspace, BAD_STATE, INTERNAL, MAX_WAIT_S, NOT_FOUND, PARAMS, VALIDATION,
};
use rcfd_api::case::Case;
use rcfd_api::facts::{InText, RunTime};
use rcfd_runner::client::Client;
use rcfd_runner::records::{LaunchSettings, RunSettings, RunState, RunStatus};
use rcfd_runner::started_by::StartedBy;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The project's run service as both clients reach it: the client of `rcfd-runner` behind one handle. Cloning it is
/// cheap; the clones talk to the same run service.
#[derive(Debug, Clone)]
pub struct RunService {
    client: Client,
    actor: Option<Actor>,
    who: Option<String>,
    reason: Option<String>,
}

impl RunService {
    /// The run service of `project`, whose runner is started from `runner_exe`. Its journal lines name no actor.
    pub fn new(project: impl Into<PathBuf>, runner_exe: impl Into<PathBuf>) -> Self {
        RunService {
            client: Client::new(project, runner_exe),
            actor: None,
            who: None,
            reason: None,
        }
    }

    /// Like [`RunService::new`] with the runner executable from `RCFD_RUNNER_BIN`, else next to the current executable.
    pub fn with_default_runner(project: impl Into<PathBuf>) -> Self {
        RunService {
            client: Client::with_default_runner(project),
            actor: None,
            who: None,
            reason: None,
        }
    }

    /// Name `actor` in the journal lines of this service's submissions and requests (ADR 0188 J3).
    pub fn with_actor(mut self, actor: Option<Actor>) -> Self {
        // Q-RL-2 (ADR 0801): a runner this service starts records the actor's client (cli, mcp) as started_by
        let by = actor.as_ref().and_then(|a| StartedBy::from_client_name(&a.client));
        self.client = self.client.with_started_by(by);
        self.actor = actor;
        self
    }

    /// This service for one call of command `who` with `reason` (its journal line names them; without this call the
    /// line names the method: `submit`, `pause`, `resume`, `cancel`).
    pub fn by(&self, who: &str, reason: Option<String>) -> RunService {
        RunService {
            who: Some(who.to_string()),
            reason,
            ..self.clone()
        }
    }

    fn journal(&self, kind: &str, method: &str, fields: Map<String, Value>) {
        let who = self.who.as_deref().unwrap_or(method);
        let origin = Origin::new(who, self.actor.clone(), self.reason.clone());
        append(self.client.project(), kind, &origin, fields);
    }

    fn control_line(&self, id: &str, requested: &str) {
        let mut m = Map::new();
        m.insert("run".into(), json!(id));
        m.insert("requested".into(), json!(requested));
        self.journal("run_control", requested, m);
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

    /// Queue a run of `case` without starting a runner; returns the run id. Journal: one `run_submitted` line.
    pub fn submit(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        self.submit_launch(case, settings, &LaunchSettings::default())
    }

    /// [`Self::submit`] with the run's launch settings (ADR 0734 P3: `start_run`'s `continue_from` and `max_steps`),
    /// checked as [`Client::submit_with`] checks them. The journal line also names them when set.
    pub fn submit_launch(
        &self,
        case: &Path,
        settings: &RunSettings,
        launch: &LaunchSettings,
    ) -> Result<String, String> {
        let id = self
            .client
            .submit_with(case, settings, launch, &rcfd_runner::store::RunTags::default())?;
        self.submitted(&id, case, settings, launch, None, None);
        Ok(id)
    }

    /// [`Self::submit_launch`] on the remote site `site` for an MCP client (ADR 0739 Q6 (a)): only a site with
    /// `assistant = true`, confirmed and admitted ([`Client::submit_on_site`]); nothing is written otherwise. The journal
    /// line also names the site.
    pub fn submit_launch_on_site(
        &self,
        case: &Path,
        settings: &RunSettings,
        launch: &LaunchSettings,
        site: &str,
    ) -> Result<String, String> {
        let id = self.client.submit_on_site(
            case,
            settings,
            launch,
            &rcfd_runner::store::RunTags::default(),
            site,
            true,
        )?;
        self.submitted(&id, case, settings, launch, None, Some(site));
        Ok(id)
    }

    /// A new run of run `src`'s case snapshot with its settings ([`Client::rerun`], ADR 0734 P3); the journal line is a
    /// `run_submitted` with `rerun_of`.
    pub fn rerun(&self, src: &str, over: &rcfd_runner::client::RerunOptions) -> Result<String, String> {
        self.rerun_at(src, over, None)
    }

    /// [`Self::rerun`] on the remote site `site` for an MCP client (ADR 0739 Q6 (a); `None`: the local rerun).
    pub fn rerun_at(
        &self,
        src: &str,
        over: &rcfd_runner::client::RerunOptions,
        site: Option<&str>,
    ) -> Result<String, String> {
        let id = match site {
            Some(s) => self.client.rerun_on_site(src, over, s, true)?,
            None => self.client.rerun(src, over)?,
        };
        let rec = rcfd_runner::store::FsStore::new(self.client.project());
        let rec = rcfd_runner::store::RunStore::read_record(&rec, &id).map_err(|e| format!("run {id}: {e}"))?;
        let settings = RunSettings::of(&rec)?;
        let launch = LaunchSettings::of(&rec)?;
        self.submitted(
            &id,
            &self.client.project().join(&rec.case),
            &settings,
            &launch,
            Some(src),
            site,
        );
        Ok(id)
    }

    /// The latest launch record of run `id` (ADR 0734 P1), `None` before its launch.
    pub fn launch(&self, id: &str) -> Result<Option<Value>, String> {
        self.client.launch(id)
    }

    fn submitted(
        &self,
        id: &str,
        case: &Path,
        settings: &RunSettings,
        launch: &LaunchSettings,
        rerun_of: Option<&str>,
        site: Option<&str>,
    ) {
        let root = self.client.project();
        let mut m = Map::new();
        m.insert("run".into(), json!(id));
        let case_rel = rcfd_project::paths::rel_string(root, case)
            .or_else(|| {
                let (r, c) = (root.canonicalize().ok()?, case.canonicalize().ok()?);
                rcfd_project::paths::rel_string(&r, &c)
            })
            .unwrap_or_else(|| case.display().to_string());
        m.insert("case".into(), json!(case_rel));
        put_list(&mut m, "set", settings.set.iter().map(|e| json!(e)).collect());
        m.insert("priority".into(), json!(settings.priority));
        m.insert("threads".into(), json!(settings.threads));
        if let Some(mb) = settings.mem_mb {
            m.insert("mem_mb".into(), json!(mb));
        }
        if let Some(c) = &launch.continue_from {
            m.insert("continue_from".into(), json!(c));
        }
        if let Some(n) = launch.max_steps {
            m.insert("max_steps".into(), json!(n));
        }
        if let Some(r) = rerun_of {
            m.insert("rerun_of".into(), json!(r));
        }
        if let Some(s) = site {
            m.insert("site".into(), json!(s));
        }
        self.journal("run_submitted", "submit", m);
    }

    /// Queue a run and make sure a runner is alive; returns the run id (the runner client's `start`: [`Self::submit`],
    /// then [`Self::ensure_runner`]).
    pub fn start(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        let id = self.submit(case, settings)?;
        self.ensure_runner()?;
        Ok(id)
    }

    /// Every run with its status.
    pub fn list(&self) -> Result<Vec<(String, RunStatus)>, String> {
        self.client.list()
    }

    /// Status of run `id`.
    pub fn status(&self, id: &str) -> Result<RunStatus, String> {
        self.client.status(id)
    }

    /// Hold run `id`: a queued run is not launched, a running one stops between two steps. Journal: one `run_control`
    /// line when the request was accepted.
    pub fn pause(&self, id: &str) -> Result<(), String> {
        self.client.pause(id)?;
        self.control_line(id, "pause");
        Ok(())
    }

    /// Release a held run (journal as [`Self::pause`]).
    pub fn resume(&self, id: &str) -> Result<(), String> {
        self.client.resume(id)?;
        self.control_line(id, "resume");
        Ok(())
    }

    /// Cancel run `id` (a running solver stops cleanly and writes its results; journal as [`Self::pause`]).
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.client.cancel(id)?;
        self.control_line(id, "cancel");
        Ok(())
    }

    /// Wait until run `id` is finished, failed or cancelled, at most `timeout`.
    pub fn wait(&self, id: &str, timeout: Duration) -> Result<RunStatus, String> {
        self.client.wait(id, timeout)
    }

    /// Queue a run of `case` tagged with its design study, variant and condition, without starting a runner; returns the
    /// run id (ADR 0189 S6).
    pub fn submit_tagged(
        &self,
        case: &Path,
        settings: &RunSettings,
        tags: &rcfd_runner::store::RunTags,
    ) -> Result<String, String> {
        self.client.submit_tagged(case, settings, tags)
    }

    /// The `solver_version` a run submitted now records (ADR 0201 S1).
    pub fn solver_version(&self) -> Option<String> {
        self.client.solver_version()
    }

    /// [`Self::submit_tagged`] with the run's launch settings, `continue_from` and `max_steps` (ADR 0200 S2); like it, no
    /// journal line.
    pub fn submit_with(
        &self,
        case: &Path,
        settings: &RunSettings,
        launch: &rcfd_runner::records::LaunchSettings,
        tags: &rcfd_runner::store::RunTags,
    ) -> Result<String, String> {
        self.client.submit_with(case, settings, launch, tags)
    }
}

/// `site` of `start_run` (ADR 0739): a site name or absent.
fn site_arg(args: &Value) -> Result<Option<String>, RpcError> {
    match args.get("site") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(err(PARAMS, "site: a site name from list_sites")),
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
        let reason = reason_arg(args)?;
        // ADR 0739: a run on a remote site, named by an MCP client (only a site with assistant = true)
        let site = site_arg(args)?;
        if let Some(s) = &site {
            // the file's checks first, before a new case is written or the site's agent is started
            let (file, entry) = rcfd_runner::site::sites::site(s).map_err(|e| err(BAD_STATE, format!("site: {e}")))?;
            if !entry.assistant {
                return Err(err(
                    BAD_STATE,
                    format!(
                        "site: {s} is not open to assistants; the user can set assistant = true in [site.{s}] of {}",
                        file.display()
                    ),
                ));
            }
        }
        // ADR 0734 P3: a rerun of a run's case snapshot instead of a case
        if args.get("from_run").is_some() {
            return self.rerun_run(args, reason, site.as_deref());
        }
        let launch = self.launch_arg(args)?;
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
        // ADR 0737 P2: the run-time plan (the engine's rule of ADR 0153 and 0158) and the dry run; without either, as before
        let run_time = match args.get("run_time") {
            None | Some(Value::Null) => None,
            Some(v) => Some(RunTime::from_json(v).map_err(|m| err(PARAMS, m))?),
        };
        let dry_run = match args.get("dry_run") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(err(PARAMS, "dry_run: true or false")),
        };
        // the run-time entries of `case` (`raw`: its text's document, when there is one) with the call's own entries
        // applied, and the plan's JSON
        let timed = |case: &Case, raw: Option<&Value>, set: &[String], name: &str| -> Result<Timing, RpcError> {
            let Some(rt) = &run_time else { return Ok(None) };
            let mut c = case.clone();
            for e in set {
                rcfd_api::chain::apply_set(&mut c, e).map_err(|x| err(VALIDATION, format!("set: {x}")))?;
            }
            let in_text = raw.map_or(InText::NotDefault, InText::Text);
            let f = rcfd_api::facts::case_facts(&c, in_text, None).map_err(|e| crate::facts::facts_error(name, e))?;
            let tp = rcfd_api::facts::plan(&f, rt).map_err(|m| err(VALIDATION, format!("{name}: run_time: {m}")))?;
            Ok(Some((tp.entries.clone(), tp.to_json(name, &f))))
        };
        let timed_set = |timing: &Timing, set: &[String]| -> Vec<String> {
            timing
                .iter()
                .flat_map(|t| t.0.clone())
                .chain(set.iter().cloned())
                .collect()
        };
        let (path, set, plan, created, timing, dry) = match new_case {
            Some(n) => {
                // made and checked, the entries applied and the plan made, all before anything is written
                let nc = self.prepare_case(n)?;
                let set = set_of()?;
                let timing = timed(&nc.case, None, &set, &nc.rel)?;
                let plan = planned
                    .then(|| Plan::with_set(nc.case.clone(), &timed_set(&timing, &set)))
                    .transpose()
                    .map_err(|m| err(VALIDATION, m))?;
                if dry_run {
                    let path = self.project.root().join(&nc.rel);
                    (path, set, plan, None, timing, Some((nc.case.clone(), nc.rel.clone())))
                } else {
                    let created = self.write_case(&nc, "start_run", reason.clone())?;
                    let path = self.project.resolve(&nc.rel).map_err(crate::project_error)?;
                    (path, set, plan, Some(created), timing, None)
                }
            }
            None => {
                let rel = str_arg(args, "case");
                let path = self.project.resolve(rel).map_err(crate::project_error)?;
                if !path.is_file() {
                    return Err(err(NOT_FOUND, format!("{rel}: no such case file")));
                }
                let set = set_of()?;
                let (timing, read) = if run_time.is_some() || dry_run {
                    let (case, raw) = self.read_case_text(&path)?;
                    (timed(&case, Some(&raw), &set, &self.rel(&path))?, Some(case))
                } else {
                    (None, None)
                };
                let plan = match planned {
                    true => Some(
                        Plan::with_set(self.read_case(&path)?, &timed_set(&timing, &set))
                            .map_err(|m| err(VALIDATION, m))?,
                    ),
                    false => None,
                };
                let dry = if dry_run {
                    read.map(|c| (c, self.rel(&path)))
                } else {
                    None
                };
                (path, set, plan, None, timing, dry)
            }
        };
        // the run-time entries first, then the store plan's, the run's own after them (the Studio's order: the last entry
        // of a key applies)
        let set: Vec<String> = timing
            .iter()
            .flat_map(|t| t.0.clone())
            .chain(plan.iter().flat_map(Plan::entries))
            .chain(set)
            .collect();
        if let Some((case, rel)) = dry {
            let timing = timing.map(|t| t.1);
            return Ok(self.dry_start(&case, &rel, &set, plan.as_ref(), timing, u64_arg(args, "mem_mb")));
        }
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
        let id = match &site {
            Some(s) => self
                .runs
                .by("start_run", reason)
                .submit_launch_on_site(&path, &settings, &launch, s)
                .map_err(|e| err(BAD_STATE, format!("site: {e}")))?,
            None => self
                .runs
                .by("start_run", reason)
                .submit_launch(&path, &settings, &launch)
                .map_err(|e| err(INTERNAL, e))?,
        };
        if let Err(e) = self.runs.ensure_runner() {
            return Err(RpcError {
                code: INTERNAL,
                message: format!("run {id} is queued, but the run service did not start: {e}"),
                data: Some(json!({"run": id})),
            });
        }
        let st = self.runs.status(&id).map_err(|e| err(INTERNAL, e))?;
        let mut out = json!({"run": id, "case": self.rel(&path), "state": state_name(st.state)});
        if let Some(s) = &site {
            out["site"] = json!(s);
        }
        if let Some(c) = created {
            out["created"] = c;
        }
        if let Some(p) = &plan {
            out["store"] = store_json(p);
        }
        if let Some((_, t)) = timing {
            out["run_time"] = t;
        }
        Ok(out)
    }

    /// What `start_run` would do (ADR 0737 P2 `dry_run`), written nowhere: the case, the `--set` entries, the store and
    /// run-time plans, the check of the case with those entries (`rcfd run --check-only`'s reply), the memory the runner
    /// would book and whether a runner would be started.
    fn dry_start(
        &self,
        case: &Case,
        rel: &str,
        set: &[String],
        plan: Option<&Plan>,
        timing: Option<Value>,
        mem_mb: Option<u64>,
    ) -> Value {
        let mut c = case.clone();
        let applied = set
            .iter()
            .try_for_each(|e| rcfd_api::chain::apply_set(&mut c, e).map_err(|x| format!("set: {x}")));
        let check = match applied {
            Ok(()) => rcfd_api::runctl::check_only(&c).1,
            Err(m) => json!({"ok": false, "diagnostics": [{"path": "", "line": null, "message": m}]}),
        };
        let memory = match (mem_mb, check["cells"].as_u64()) {
            (Some(m), _) => json!({"mb": m, "rule": "mem_mb given", "budgets": null}),
            (None, Some(cells)) => {
                let (b, from) = self.budgets();
                rcfd_api::facts::memory_json(cells, &b, from)
            }
            (None, None) => Value::Null,
        };
        let running = self.runs.runner_running();
        json!({
            "dry_run": true,
            "case": rel,
            "set": set,
            "store": plan.map(store_json),
            "run_time": timing,
            "check": check,
            "memory_mb": memory["mb"],
            "memory": memory,
            "runner": {"running": running, "would_start": !running},
        })
    }

    /// `continue_from` and `max_steps` of `start_run` (ADR 0734 P3), checked before anything is written: a run of this
    /// project (1004 not a run id, 1202 no such run) and at least 1.
    fn launch_arg(&self, args: &Value) -> Result<LaunchSettings, RpcError> {
        let continue_from = match args.get("continue_from").and_then(Value::as_str) {
            Some(c) => Some(
                self.run_arg(&json!({"run": c}), "run")
                    .map(|(id, _)| id)
                    .map_err(|e| RpcError {
                        message: format!("continue_from: {}", e.message),
                        ..e
                    })?,
            ),
            None => None,
        };
        let max_steps = u64_arg(args, "max_steps");
        if max_steps == Some(0) {
            return Err(err(PARAMS, "max_steps: must be at least 1"));
        }
        Ok(LaunchSettings {
            continue_from,
            max_steps,
        })
    }

    /// `start_run {from_run}` (ADR 0734 P3): a new run of that run's `case.snapshot` with its record's settings;
    /// `priority`, `threads` and `mem_mb` override them. -32602 with `case`, `new_case`, `set`, `store`,
    /// `continue_from` or `max_steps` (the record gives those); 1202 no such run; 1003 the record's settings do not read.
    /// ADR 0737: `run_time` and `dry_run` are -32602 too: the run-time plan adds settings, which a rerun takes from its
    /// record, and a dry run reads a case file, which a rerun has none of.
    fn rerun_run(&self, args: &Value, reason: Option<String>, site: Option<&str>) -> Result<Value, RpcError> {
        let with: Vec<&str> = [
            "case",
            "new_case",
            "set",
            "store",
            "continue_from",
            "max_steps",
            "run_time",
            "dry_run",
        ]
        .into_iter()
        .filter(|k| args.get(*k).is_some_and(|v| !v.is_null()))
        .collect();
        if !with.is_empty() {
            return Err(err(
                PARAMS,
                format!(
                    "start_run: from_run reruns that run's case snapshot with its record's settings; give no {}",
                    with.join(", ")
                ),
            ));
        }
        let (src, dir) = self.run_arg(args, "from_run")?;
        let rec: rcfd_project::RunRecord =
            rcfd_project::atomic::read_json(&dir.join("run.json")).map_err(crate::project_error)?;
        RunSettings::of(&rec)
            .and_then(|_| LaunchSettings::of(&rec))
            .map_err(|e| err(VALIDATION, format!("run {src}: {e}")))?;
        let over = rcfd_runner::client::RerunOptions {
            priority: args
                .get("priority")
                .and_then(Value::as_i64)
                .map(|p| p.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
            threads: u64_arg(args, "threads").map(|t| t as usize),
            mem_mb: u64_arg(args, "mem_mb"),
        };
        let id = self
            .runs
            .by("start_run", reason)
            .rerun_at(&src, &over, site)
            .map_err(|e| err(if site.is_some() { BAD_STATE } else { INTERNAL }, e))?;
        if let Err(e) = self.runs.ensure_runner() {
            return Err(RpcError {
                code: INTERNAL,
                message: format!("run {id} is queued, but the run service did not start: {e}"),
                data: Some(json!({"run": id})),
            });
        }
        let st = self.runs.status(&id).map_err(|e| err(INTERNAL, e))?;
        let mut out = json!({"run": id, "case": rec.case, "rerun_of": src, "state": state_name(st.state)});
        if let Some(s) = site {
            out["site"] = json!(s);
        }
        Ok(out)
    }

    /// `list_sites` (ADR 0739): the sites of the user's sites file; nothing is started.
    pub(crate) fn list_sites(&self) -> Result<Value, RpcError> {
        let (file, sites) = rcfd_runner::site::sites::load().map_err(|e| err(VALIDATION, e))?;
        let list: Vec<Value> = sites.iter().map(|(n, e)| e.listing(n)).collect();
        Ok(json!({"file": file.display().to_string(), "sites": list}))
    }

    pub(crate) fn control(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let (id, _) = self.run_arg(args, "run")?;
        let runs = self.runs.by(name, reason);
        let (r, what) = match name {
            "pause_run" => (runs.pause(&id), "pause"),
            "resume_run" => (runs.resume(&id), "resume"),
            _ => (runs.cancel(&id), "cancel"),
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

/// `store` of a `start_run` reply.
fn store_json(p: &Plan) -> Value {
    json!({"applied": p.every.is_some(), "set": p.entries(), "snapshots": p.snapshots(),
        "estimated_bytes": p.estimated_bytes(), "note": p.note})
}

/// The run-time plan of a `start_run`: its `--set` entries and its JSON (`None` without `run_time`).
type Timing = Option<(Vec<String>, Value)>;

#[cfg(test)]
mod started_by_tests {
    use super::*;

    /// Q-RL-2 (ADR 0801): a runner the service starts records its actor's client (cli, mcp) as started_by.
    #[test]
    fn the_actor_names_who_starts_the_runner() {
        if std::env::var_os(rcfd_runner::started_by::ENV).is_some() {
            eprintln!(
                "skipped: {} is set in the test's environment",
                rcfd_runner::started_by::ENV
            );
            return;
        }
        let s = RunService::new("p", "rcfd-runner");
        assert_eq!(s.client.started_by(), None);
        let s = s.with_actor(Some(Actor::this_process(crate::journal::MCP)));
        assert_eq!(s.client.started_by(), Some(StartedBy::Mcp));
        let s = s.with_actor(Some(Actor::this_process("cli")));
        assert_eq!(s.client.started_by(), Some(StartedBy::Cli));
        let s = s.with_actor(Some(Actor::this_process(crate::journal::STUDIO)));
        assert_eq!(s.client.started_by(), None);
        let s = s.with_actor(None);
        assert_eq!(s.client.started_by(), None);
    }
}
