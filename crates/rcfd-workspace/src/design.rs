//! Design study evaluations on the project's runner (ADR 0189, ADR 0173 W7; ADR 0200). In project mode the MCP tools
//! `design_run_study`, `design_evaluate` and `design_run_operating_map` evaluate each candidate of a study at each operating
//! condition, or each point of a map, as one run of the project's runner, a study's runs tagged with its study, variant and
//! condition (`run.json`), and read the result from the run's `metrics.json`; so the runs are counted by the runner's
//! budget, outlive the server and show in the Studio.
//!
//! [`Workspace::design_call`] runs `design.run_study`, `design.evaluate` or `design.run_map` through
//! `rcfd_api::design::tools::call_with`, the same code as in process (loading the study, the allowance, the store, the map's
//! table, the result), with [`RunnerSource`] choosing the evaluator: [`RunnerEvaluator`] or [`RunnerMapEvaluator`] when the
//! base case lies in the project, else the in-process evaluator, as before, with the reason in the result (ADR 0189 S7). A
//! run is the base case file with `rcfd run --set` entries ([`entries`]) whose settings tree is checked to be the candidate's
//! case before it is queued (ADR 0189 S4); its result follows the in-process status rule on the solver's `finish` event
//! ([`cond_result`], ADR 0189 S5). A chained condition, or a map point after a converged one, continues from the run before
//! it when its restart file fits ([`start_from`], ADR 0200 S6), its iterations counted from the restart (ADR 0200 S7); a
//! study's own step cap is the runs' `max_steps` (ADR 0200 S5). Before a study's run is submitted, a run of the project
//! that already is that evaluation ([`RunKey`], a run left by a server stopped during the study) is adopted instead:
//! waited for, or read from its `metrics.json` ([`choose`], ADR 0201).

use crate::runs::RunService;
use crate::{err, Workspace, INTERNAL};
use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseEvaluator, CaseRun};
use rcfd_api::design::opmap::{MapPoint, PointEvaluator, PointResult, PointStatus};
use rcfd_api::design::store::CondResult;
use rcfd_api::design::study::{Condition, Evaluator, StudyDef};
use rcfd_api::design::tools::{call_with, EvaluatorSource, MapRun, StudyRun};
use rcfd_api::runctl::exit;
use rcfd_api::service::{RpcError, Service};
use rcfd_runner::client::state_name;
use rcfd_runner::records::{LaunchSettings, RunRecord, RunSettings, RunState, RunStatus};
use rcfd_runner::store::{extra, RunTags};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The design methods that [`Workspace::design_call`] evaluates on the runner.
pub const ON_RUNNER: [&str; 3] = ["design.run_study", "design.evaluate", "design.run_map"];
/// Most runs a result lists in `runs` (`runs_total` counts them all).
pub const MAX_LISTED_RUNS: usize = 50;
/// Period of reading a run's status while waiting for it.
pub const POLL: Duration = Duration::from_millis(100);
/// Period of making sure a runner holds the project's lock while waiting.
pub const RUNNER_CHECK: Duration = Duration::from_secs(1);

/// A number written to a settings path, as `CaseEvaluator::build_case` writes a parameter value.
fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// The `rcfd run --set` entry for `value` at `path` that gives the tree `rcfd_api::study::apply_value` gives (S4): a whole
/// number below 9e15 in magnitude as an integer (what `apply_value` tries first), any other number in serde_json's shortest
/// form, any other value as its JSON text.
pub fn set_entry(path: &str, value: &Value) -> String {
    if let Some(x) = value.as_f64() {
        if x.fract() == 0.0 && x.abs() < 9.0e15 {
            return format!("{path}={}", x as i64);
        }
    }
    format!("{path}={value}")
}

/// The entries of candidate `x` at `cond`: each parameter target (`scale * x + offset`) in parameter and target order,
/// then the condition's settings in their order, as `CaseEvaluator::build_case` applies them.
pub fn entries(def: &StudyDef, x: &[f64], cond: &Condition) -> Vec<String> {
    let mut out = Vec::new();
    for (p, &v) in def.params.iter().zip(x) {
        for t in &p.targets {
            out.push(set_entry(&t.path, &num(t.scale * v + t.offset)));
        }
    }
    out.extend(cond.set.iter().map(|(path, v)| set_entry(path, v)));
    out
}

/// The `variant` tag of candidate `x` (S6): `<parameter>=<value>` per parameter in definition order, joined by `,`.
pub fn variant_name(def: &StudyDef, x: &[f64]) -> String {
    def.params
        .iter()
        .zip(x)
        .map(|(p, v)| format!("{}={}", p.name, num(*v)))
        .collect::<Vec<_>>()
        .join(",")
}

/// The solver's result of an ended run as the in-process evaluator's [`CaseRun`] (ADR 0189 S5): from the `finish` event
/// in `metrics.json` (`solver_finish`), the finite report values as metrics (a non-finite one named in the message), the
/// iterations counted from `start_iteration`, the iteration a continued run started at (ADR 0200 S7); `None` when the run
/// has no `finish` event.
pub fn case_run(metrics: &Value, reason: &str, start_iteration: u64) -> Option<CaseRun> {
    let fin = metrics.get("solver_finish").filter(|f| f.is_object())?;
    let exit_code = fin["exit_code"]
        .as_u64()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(exit::FAIL);
    let mut values = BTreeMap::new();
    let mut notes = Vec::new();
    for (name, v) in metrics["reports"].as_object().into_iter().flatten() {
        match v.as_f64() {
            Some(x) if x.is_finite() => {
                values.insert(name.clone(), x);
            }
            _ => notes.push(format!("report {name}: not finite")),
        }
    }
    if exit_code != exit::OK {
        notes.insert(0, fin["reason"].as_str().unwrap_or(reason).to_string());
    }
    Some(CaseRun {
        exit_code,
        state: fin["state"].as_str().unwrap_or("").to_string(),
        iterations: fin["iterations"].as_u64().unwrap_or(0).saturating_sub(start_iteration) as usize,
        metrics: values,
        message: notes.join("; "),
        fields: None,
        warm: false,
    })
}

/// The condition result of a finished run (S5): the status by the in-process rule (`CaseRun::status`) on the solver's
/// `finish` event in `metrics.json` (`solver_finish`), usable when converged with at least one finite report value, the
/// metrics kept only then; a run without a `finish` event (ended before launch, cancelled, killed) failed with `reason`, the
/// run's status reason.
pub fn cond_result(label: &str, metrics: &Value, reason: &str) -> CondResult {
    cond_result_from(label, metrics, reason, 0)
}

/// [`cond_result`] of a run that started at iteration `start_iteration` (a continued run, ADR 0200 S7): its `iterations`
/// are the steps of that run.
pub fn cond_result_from(label: &str, metrics: &Value, reason: &str, start_iteration: u64) -> CondResult {
    let Some(run) = case_run(metrics, reason, start_iteration) else {
        let why = if reason.is_empty() {
            "the run ended without a solver result"
        } else {
            reason
        };
        return CondResult::failure(label, "failed", why);
    };
    let status = run.status();
    let ok = status == PointStatus::Converged && !run.metrics.is_empty();
    CondResult {
        label: label.to_string(),
        ok,
        status: status.name().to_string(),
        iterations: run.iterations,
        metrics: if ok { run.metrics } else { BTreeMap::new() },
        message: run.message,
    }
}

/// The `meta` section of a restart file.
fn restart_meta(restart: &Path) -> Result<Value, String> {
    let c = rcfd_io::container::Container::read_file(restart).map_err(|e| e.to_string())?;
    let m = c.get("meta").ok_or("no meta section")?;
    serde_json::from_slice::<Value>(m).map_err(|e| e.to_string())
}

/// The iteration counter a restart file holds (`iteration` of its `meta`): where a run continued from it starts counting
/// (ADR 0200 S7, amendment 1); `None` when the file does not read.
pub fn restart_iteration(restart: &Path) -> Option<u64> {
    restart_meta(restart).ok()?["iteration"].as_u64()
}

/// How a run starts from a previous run's result (ADR 0200 S6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartFrom {
    /// From the previous run's restart file (`rcfd run --continue-from`), whose iteration counter is `iteration`.
    Continue {
        /// The `iteration` of the restart file's `meta`.
        iteration: u64,
    },
    /// Cold; with the note `cold start: <why>` when the in-process evaluator would also have tried a warm start.
    Cold(Option<String>),
}

/// Whether a run of `next` can continue from the restart file `restart` of the run before it (ADR 0200 S6 (b), (c)): a file
/// of the steady engine (SIMPLE/SIMPLEC) without a temperature field, written for the mesh of `next`. A file of the
/// transient engine is no warm start, as in process (a transient run carries no fields), so it gives no note.
pub fn start_from(restart: &Path, next: &Case) -> StartFrom {
    let cold = |why: String| StartFrom::Cold(Some(format!("cold start: {why}")));
    if !restart.is_file() {
        return cold(format!("the run before left no restart file {}", restart.display()));
    }
    let meta = match restart_meta(restart) {
        Ok(m) => m,
        Err(e) => return cold(format!("the restart file {} does not read: {e}", restart.display())),
    };
    if meta["engine"].as_str().unwrap_or("steady") != "steady" {
        return StartFrom::Cold(None);
    }
    if meta["energy"].as_bool().unwrap_or(false) {
        return cold(
            "a warm start carries velocity and pressure only; the restart file holds a temperature field".into(),
        );
    }
    match rcfd_api::sim::build_mesh(next) {
        Ok(m) if meta["mesh_hash"].as_str() == Some(format!("{:016x}", m.content_hash()).as_str()) => {
            StartFrom::Continue {
                iteration: meta["iteration"].as_u64().unwrap_or(0),
            }
        }
        Ok(_) => cold("the restart file was written for another mesh".into()),
        Err(d) => cold(format!(
            "the mesh of the case does not build: {}",
            d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        )),
    }
}

/// What a run must record to be the evaluation of a study's candidate at a condition (ADR 0201 S1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunKey {
    /// The tags (ADR 0189 S6).
    pub tags: RunTags,
    /// `case` of `run.json`: the base case's project-relative path.
    pub case: String,
    /// `hash_bytes` of the base case file's bytes now.
    pub case_snapshot_hash: String,
    /// `settings.set`: the entries, in order.
    pub set: Vec<String>,
    /// `settings.continue_from` and `settings.max_steps` (ADR 0200).
    pub launch: LaunchSettings,
    /// The `solver_version` a new run would record.
    pub solver_version: String,
}

impl RunKey {
    /// Whether the record `rec` is this evaluation: every item of the key equal (absent as absent).
    pub fn matches(&self, rec: &RunRecord) -> bool {
        let set: Option<Vec<String>> = serde_json::from_value(rec.settings["set"].clone()).ok();
        rec.study == self.tags.study
            && rec.variant == self.tags.variant
            && rec.condition == self.tags.condition
            && rec.case == self.case
            && rec.case_snapshot_hash == self.case_snapshot_hash
            && set.as_ref() == Some(&self.set)
            && LaunchSettings::of(rec).ok().as_ref() == Some(&self.launch)
            && rec.solver_version == self.solver_version
    }
}

/// The run to adopt for `key` among `runs` (ADR 0201 S2): of the matches that are not `failed` or `cancelled`, the newest
/// `finished` one, else the newest of the others (`queued`, `running` or `paused`; newest: the largest run id).
pub fn choose<'a>(key: &RunKey, runs: &'a [(RunRecord, RunStatus)]) -> Option<&'a RunRecord> {
    let live = || {
        runs.iter()
            .filter(|(rec, st)| !matches!(st.state, RunState::Failed | RunState::Cancelled) && key.matches(rec))
    };
    live()
        .filter(|(_, st)| st.state == RunState::Finished)
        .map(|(rec, _)| rec)
        .max_by(|a, b| a.id.cmp(&b.id))
        .or_else(|| live().map(|(rec, _)| rec).max_by(|a, b| a.id.cmp(&b.id)))
}

/// The runs of a project as a study's call finds them (ADR 0201 S3): every run folder's `run.json` and `status.json`; a
/// folder that does not read is skipped.
#[derive(Debug, Default)]
pub struct RunIndex {
    runs: Vec<(RunRecord, RunStatus)>,
}

impl RunIndex {
    /// Read the runs of `runs`' project.
    pub fn read(runs: &RunService) -> RunIndex {
        let root = runs.project().join("runs");
        let mut ids: Vec<String> = std::fs::read_dir(&root)
            .map(|rd| {
                rd.filter_map(|e| e.ok()?.file_name().into_string().ok())
                    .filter(|n| rcfd_project::runid::is_run_id(n))
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        let read = |id: &str| -> Option<(RunRecord, RunStatus)> {
            let dir = root.join(id);
            let rec: RunRecord = rcfd_project::atomic::read_json(&dir.join("run.json")).ok()?;
            let st: RunStatus = rcfd_project::atomic::read_json(&dir.join("status.json")).ok()?;
            Some((rec, st))
        };
        RunIndex {
            runs: ids.iter().filter_map(|id| read(id)).collect(),
        }
    }

    /// The number of runs read.
    pub fn len(&self) -> usize {
        self.runs.len()
    }

    /// No run read.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// The run to adopt for `key` ([`choose`]).
    pub fn adopt(&self, key: &RunKey) -> Option<&RunRecord> {
        choose(key, &self.runs)
    }
}

/// What the key of a study's runs takes from the project and the solver once per call (ADR 0201 S1, S3).
struct Adoption {
    index: RunIndex,
    case: String,
    case_snapshot_hash: String,
    solver_version: String,
}

/// The dotted path of the first place where `a` and `b` differ (`None` when equal).
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    let join = |k: &str| {
        if at.is_empty() {
            k.to_string()
        } else {
            format!("{at}.{k}")
        }
    };
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => {
                        if let Some(d) = first_difference(v, w, &join(k)) {
                            return Some(d);
                        }
                    }
                    None => return Some(join(k)),
                }
            }
            y.keys().find(|k| !x.contains_key(*k)).map(|k| join(k))
        }
        _ if a == b => None,
        _ => Some(if at.is_empty() {
            "(the whole tree)".into()
        } else {
            at.to_string()
        }),
    }
}

/// One run made or adopted by an evaluator of this module.
#[derive(Debug, Clone, Default)]
struct Made {
    run: String,
    variant: Option<String>,
    condition: Option<String>,
    point: Option<usize>,
    continue_from: Option<String>,
    state: String,
    adopted: bool,
}

/// What the two evaluators share: the run service, the base case file, the list of the runs they made, and for a study the
/// runs it may adopt (ADR 0201).
struct Queue<'s> {
    runs: &'s RunService,
    root: PathBuf,
    base_path: PathBuf,
    made: &'s Mutex<Vec<Made>>,
    adoption: Option<Adoption>,
}

/// One run waited for: its result, its id, the iteration it started at (the restart file's, ADR 0200 S7, amendment 1) and,
/// when `rcfd run` could not load the restart file it was to continue from, the run's reason (ADR 0200 S6).
struct Ran {
    metrics: Value,
    reason: String,
    run: Option<String>,
    start_iteration: u64,
    load_failure: Option<String>,
}

impl Queue<'_> {
    fn run_dir(&self, id: &str) -> PathBuf {
        self.root.join("runs").join(id)
    }

    /// `runs/<id>/output/restart.rcfd`, the restart file run `id` leaves.
    fn restart_file(&self, id: &str) -> PathBuf {
        self.run_dir(id).join(extra::OUTPUT).join("restart.rcfd")
    }

    /// The base case file read as `rcfd run` reads it, with `set` applied as `rcfd run --set` applies it.
    fn case_of(&self, set: &[String]) -> Result<Case, String> {
        let text = std::fs::read_to_string(&self.base_path)
            .map_err(|e| format!("cannot read the base case {}: {e}", self.base_path.display()))?;
        let dir = self.base_path.parent().unwrap_or(&self.root);
        let mut case = crate::plan::parse_case(&text, dir, Some(&self.root)).map_err(|d| {
            format!(
                "the base case does not read: {}",
                d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            )
        })?;
        for e in set {
            rcfd_api::chain::apply_set(&mut case, e).map_err(|e| e.to_string())?;
        }
        Ok(case)
    }

    /// Check that the base case file with `set` gives the settings tree of `want` (ADR 0189 S4).
    fn check_tree(&self, want: &Case, set: &[String], whose: &str) -> Result<(), String> {
        let got = self.case_of(set)?;
        match first_difference(&want.tree.to_json(), &got.tree.to_json(), "") {
            Some(at) => Err(format!(
                "the run's settings would differ from the {whose} case at {at}; no run was made"
            )),
            None => Ok(()),
        }
    }

    /// Queue a run of the base case file with `set`, `launch` and `tags`, recorded in the list of runs; for a study, a run
    /// of the project that already is this evaluation is adopted instead (ADR 0201 S1 to S3).
    fn queue(&self, set: &[String], launch: &LaunchSettings, tags: &RunTags, mut made: Made) -> Result<String, String> {
        if let Some(a) = self.adoption.as_ref().filter(|_| tags.study.is_some()) {
            let key = RunKey {
                tags: tags.clone(),
                case: a.case.clone(),
                case_snapshot_hash: a.case_snapshot_hash.clone(),
                set: set.to_vec(),
                launch: launch.clone(),
                solver_version: a.solver_version.clone(),
            };
            if let Some(rec) = a.index.adopt(&key) {
                made.run = rec.id.clone();
                made.continue_from = launch.continue_from.clone();
                made.state = self.runs.status(&rec.id).map(|st| state_name(st.state))?;
                made.adopted = true;
                self.made.lock().unwrap_or_else(|e| e.into_inner()).push(made);
                return Ok(rec.id.clone());
            }
        }
        let settings = RunSettings {
            set: set.to_vec(),
            ..RunSettings::default()
        };
        let id = self
            .runs
            .submit_with(&self.base_path, &settings, launch, tags)
            .map_err(|e| format!("the run could not be queued: {e}"))?;
        made.run = id.clone();
        made.continue_from = launch.continue_from.clone();
        made.state = "queued".into();
        self.made.lock().unwrap_or_else(|e| e.into_inner()).push(made);
        Ok(id)
    }

    fn note_state(&self, id: &str, state: &str) {
        let mut made = self.made.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = made.iter_mut().find(|m| m.run == id) {
            m.state = state.to_string();
        }
    }

    /// Wait for run `id` (no time limit, as in process; `cancel_run` ends it): its `metrics.json` and status reason.
    fn wait_end(&self, id: &str) -> Result<(Value, String), String> {
        let mut checked = Instant::now();
        loop {
            let st = self.runs.status(id)?;
            if st.state.is_terminal() {
                let file = self.run_dir(id).join("metrics.json");
                let metrics: Value = std::fs::read(&file)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or(Value::Null);
                self.note_state(id, &state_name(st.state));
                return Ok((metrics, st.reason.unwrap_or_default()));
            }
            if checked.elapsed() >= RUNNER_CHECK {
                checked = Instant::now();
                if !self.runs.runner_running() {
                    if let Err(e) = self.runs.ensure_runner() {
                        return Err(format!(
                            "the project's run service stopped and did not start again: {e}; run {id} stays queued"
                        ));
                    }
                }
            }
            std::thread::sleep(POLL);
        }
    }

    /// Wait for run `id`, queued with `launch` and continued at `start_iteration` (0 for a cold run), and read what [`Ran`]
    /// holds.
    fn ran(&self, id: String, launch: &LaunchSettings, start_iteration: u64) -> Result<Ran, String> {
        let (metrics, reason) = self.wait_end(&id)?;
        let continued = launch.continue_from.is_some();
        let no_result = metrics.get("solver_finish").filter(|f| f.is_object()).is_none();
        let load_failure =
            (continued && no_result && (reason.contains("--continue-from") || reason.contains("continue_from:")))
                .then(|| reason.clone());
        Ok(Ran {
            metrics,
            reason,
            run: Some(id),
            start_iteration,
            load_failure,
        })
    }
}

/// `note` before `message` (joined with `; `).
fn with_note(note: Option<String>, message: String) -> String {
    match note {
        Some(n) if message.is_empty() => n,
        Some(n) => format!("{n}; {message}"),
        None => message,
    }
}

/// The note of a continued run that ended at the iteration cap (ADR 0200 S7, Q6).
fn cap_note(start_iteration: u64) -> String {
    format!("the run continued at iteration {start_iteration} (--continue-from keeps the iteration cap)")
}

/// The evaluator of a study on the project's runner (ADR 0189 S4, ADR 0200 S5, S6): one tagged run per operating condition
/// of a candidate, all queued together, or with a chain one after another, each continued from the one before when its
/// restart file fits; each waited for and read from its `metrics.json`.
pub struct RunnerEvaluator<'s> {
    q: Queue<'s>,
    study: String,
    in_process: CaseEvaluator,
    chain: bool,
    max_steps: Option<u64>,
}

impl RunnerEvaluator<'_> {
    fn fail(label: &str, message: String) -> CondResult {
        CondResult::failure(label, "failed", message)
    }

    fn tags(&self, variant: &str, label: &str) -> RunTags {
        RunTags {
            study: Some(self.study.clone()),
            variant: Some(variant.to_string()),
            condition: Some(label.to_string()),
        }
    }

    fn launch(&self, continue_from: Option<String>) -> LaunchSettings {
        LaunchSettings {
            continue_from,
            max_steps: self.max_steps,
        }
    }

    /// The candidate's case at `cond` and its entries, checked (ADR 0189 S4), or the condition's failure.
    fn prepare(&self, def: &StudyDef, x: &[f64], cond: &Condition) -> Result<(Case, Vec<String>), CondResult> {
        let label = &cond.label;
        let want = self
            .in_process
            .build_case(def, x, Some(cond))
            .map_err(|m| Self::fail(label, m))?;
        let set = entries(def, x, cond);
        self.q
            .check_tree(&want, &set, "candidate's")
            .map_err(|m| Self::fail(label, m))?;
        Ok((want, set))
    }

    fn made(variant: &str, label: &str) -> Made {
        Made {
            variant: Some(variant.to_string()),
            condition: Some(label.to_string()),
            ..Made::default()
        }
    }

    /// Queue the run of a prepared condition, continued from `cont` (a run and the iteration of its restart file), and wait
    /// for it.
    fn run_one(&self, set: &[String], variant: &str, label: &str, cont: Option<(String, u64)>) -> Result<Ran, String> {
        let (cont, start) = cont.map_or((None, 0), |(r, i)| (Some(r), i));
        let launch = self.launch(cont);
        let id = self
            .q
            .queue(set, &launch, &self.tags(variant, label), Self::made(variant, label))?;
        // a runner that cannot start is reported by the wait
        let _ = self.q.runs.ensure_runner();
        self.q.ran(id, &launch, start)
    }

    /// Condition `cond` of a chained candidate (ADR 0200 S6): after the run of the condition before (`prev`, when it was
    /// usable) has ended, continued from it when its restart file fits, else cold; the condition's result and its run.
    fn chained(
        &self,
        def: &StudyDef,
        x: &[f64],
        cond: &Condition,
        variant: &str,
        prev: Option<&str>,
    ) -> (CondResult, Option<String>) {
        let label = &cond.label;
        let (want, set) = match self.prepare(def, x, cond) {
            Ok(v) => v,
            Err(r) => return (r, None),
        };
        let (cont, mut note) = match prev {
            None => (None, None),
            Some(p) => match start_from(&self.q.restart_file(p), &want) {
                StartFrom::Continue { iteration } => (Some((p.to_string(), iteration)), None),
                StartFrom::Cold(n) => (None, n),
            },
        };
        let mut ran = match self.run_one(&set, variant, label, cont) {
            Ok(r) => r,
            Err(m) => return (Self::fail(label, m), None),
        };
        if let Some(why) = ran.load_failure.take() {
            // rcfd run could not load the restart file (an engine or a turbulence model it does not fit): once more, cold
            note = Some(format!("cold start: {why}"));
            ran = match self.run_one(&set, variant, label, None) {
                Ok(r) => r,
                Err(m) => return (Self::fail(label, m), None),
            };
        }
        let mut r = cond_result_from(label, &ran.metrics, &ran.reason, ran.start_iteration);
        if ran.start_iteration > 0 && r.status == PointStatus::NotConverged.name() {
            r.message = with_note(Some(cap_note(ran.start_iteration)), r.message);
        }
        r.message = with_note(note, r.message);
        (r, ran.run)
    }
}

impl Evaluator for RunnerEvaluator<'_> {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        let variant = variant_name(def, x);
        if self.chain && def.conditions.len() > 1 {
            let mut out = Vec::with_capacity(def.conditions.len());
            let mut prev: Option<String> = None;
            for c in &def.conditions {
                let (r, run) = self.chained(def, x, c, &variant, prev.as_deref());
                prev = if r.ok { run } else { None };
                out.push(r);
            }
            return out;
        }
        // ADR 0189 S4: all conditions of the candidate queued together
        let launch = self.launch(None);
        let queued: Vec<Result<String, CondResult>> = def
            .conditions
            .iter()
            .map(|c| {
                let (_, set) = self.prepare(def, x, c)?;
                self.q
                    .queue(
                        &set,
                        &launch,
                        &self.tags(&variant, &c.label),
                        Self::made(&variant, &c.label),
                    )
                    .map_err(|m| Self::fail(&c.label, m))
            })
            .collect();
        if queued.iter().any(Result::is_ok) {
            // a runner that cannot start is reported by the wait, once per run
            let _ = self.q.runs.ensure_runner();
        }
        queued
            .into_iter()
            .zip(&def.conditions)
            .map(|(q, c)| match q {
                Ok(id) => match self.q.ran(id, &launch, 0) {
                    Ok(ran) => cond_result_from(&c.label, &ran.metrics, &ran.reason, ran.start_iteration),
                    Err(m) => Self::fail(&c.label, m),
                },
                Err(r) => r,
            })
            .collect()
    }
}

/// The point evaluator of an operating map on the project's runner (ADR 0200 S8): one untagged run per point, the base case
/// file with the map's prefix entries and the point's own, continued from the previous converged point's run when its
/// restart file fits; the state carried to the next point is the id of a converged point's run.
pub struct RunnerMapEvaluator<'s> {
    q: Queue<'s>,
    prefix: Vec<(String, Value)>,
    base: Case,
    max_steps: Option<u64>,
}

impl RunnerMapEvaluator<'_> {
    fn run_one(&self, point: &MapPoint, set: &[String], cont: Option<(String, u64)>) -> Result<Ran, String> {
        let (cont, start) = cont.map_or((None, 0), |(r, i)| (Some(r), i));
        let launch = LaunchSettings {
            continue_from: cont,
            max_steps: self.max_steps,
        };
        let made = Made {
            point: Some(point.index),
            ..Made::default()
        };
        let id = self.q.queue(set, &launch, &RunTags::default(), made)?;
        let _ = self.q.runs.ensure_runner();
        self.q.ran(id, &launch, start)
    }
}

impl PointEvaluator for RunnerMapEvaluator<'_> {
    type State = String;

    fn run(&self, point: &MapPoint, warm: Option<&String>) -> PointResult<String> {
        // the point's case as `CaseMapEvaluator::run` builds it
        let mut want = self.base.clone();
        for (path, v) in &point.set {
            if let Err(e) = rcfd_api::study::apply_value(&mut want, path, v) {
                return PointResult::failed(format!("{path}: {e}"));
            }
        }
        let set: Vec<String> = self
            .prefix
            .iter()
            .chain(&point.set)
            .map(|(p, v)| set_entry(p, v))
            .collect();
        if let Err(m) = self.q.check_tree(&want, &set, "point's") {
            return PointResult::failed(m);
        }
        let (cont, mut note) = match warm {
            None => (None, None),
            Some(p) => match start_from(&self.q.restart_file(p), &want) {
                StartFrom::Continue { iteration } => (Some((p.clone(), iteration)), None),
                StartFrom::Cold(n) => (None, n),
            },
        };
        let mut warm_applied = cont.is_some();
        let mut ran = match self.run_one(point, &set, cont) {
            Ok(r) => r,
            Err(m) => return PointResult::failed(m),
        };
        if let Some(why) = ran.load_failure.take() {
            note = Some(format!("cold start: {why}"));
            warm_applied = false;
            ran = match self.run_one(point, &set, None) {
                Ok(r) => r,
                Err(m) => return PointResult::failed(m),
            };
        }
        let Some(run) = case_run(&ran.metrics, &ran.reason, ran.start_iteration) else {
            let why = if ran.reason.is_empty() {
                "the run ended without a solver result".to_string()
            } else {
                ran.reason.clone()
            };
            let mut r = PointResult::failed(with_note(note, why));
            r.warm_applied = warm_applied;
            return r;
        };
        let status = run.status();
        let usable = status == PointStatus::Converged;
        let mut message = run.message.clone();
        if ran.start_iteration > 0 && status == PointStatus::NotConverged {
            message = with_note(Some(cap_note(ran.start_iteration)), message);
        }
        PointResult {
            status,
            exit_code: run.exit_code,
            state_name: run.state.clone(),
            iterations: run.iterations,
            metrics: if usable {
                run.metrics.into_iter().collect()
            } else {
                Vec::new()
            },
            message: with_note(note, message),
            warm_applied,
            state: if usable { ran.run } else { None },
        }
    }
}

/// The evaluator source of a project (ADR 0189 S3, ADR 0200 S5, S8): the runner's evaluators when the base case lies in
/// the project, else the in-process evaluator with the reason; it keeps what it chose and the runs it made for the result.
pub struct RunnerSource<'w> {
    ws: &'w Workspace,
    made: Mutex<Vec<Made>>,
    chosen: Mutex<Option<Result<(), String>>>,
}

impl<'w> RunnerSource<'w> {
    /// The source of `ws`.
    pub fn new(ws: &'w Workspace) -> Self {
        RunnerSource {
            ws,
            made: Mutex::new(Vec::new()),
            chosen: Mutex::new(None),
        }
    }

    /// `path` relative to the project, both canonical (`None` when outside it); a path that does not exist yet is taken
    /// from its nearest existing folder.
    fn in_project(&self, path: &Path) -> Option<String> {
        let root = rcfd_project::paths::canonicalize(self.ws.root()).ok()?;
        let mut rest = Vec::new();
        let mut at = path;
        let p = loop {
            match rcfd_project::paths::canonicalize(at) {
                Ok(c) => break rest.iter().rev().fold(c, |c: PathBuf, n| c.join(n)),
                Err(_) => {
                    rest.push(at.file_name()?.to_os_string());
                    at = at.parent()?;
                }
            }
        };
        rcfd_project::paths::rel_string(&root, &p).filter(|r| !r.is_empty())
    }

    /// Why the runner cannot make this study's evaluations (ADR 0200 S5), or its project-relative folder when it can.
    fn runner_folder(&self, study: &StudyRun<'_>) -> Result<String, String> {
        if self.in_project(study.base_path).is_none() {
            return Err(format!(
                "the base case {} is not in the project, so the runner cannot run it",
                study.base_path.display()
            ));
        }
        self.in_project(study.dir)
            .ok_or_else(|| format!("the study folder {} is not in the project", study.dir.display()))
    }

    /// Why the runner cannot run this map's points (ADR 0200 S8), or nothing.
    fn map_rule(&self, map: &MapRun<'_>) -> Result<(), String> {
        if self.in_project(map.base_path).is_none() {
            return Err(format!(
                "the base case {} is not in the project, so the runner cannot run it",
                map.base_path.display()
            ));
        }
        if self.in_project(map.out_dir).is_none() {
            return Err(format!(
                "the map folder {} is not in the project",
                map.out_dir.display()
            ));
        }
        Ok(())
    }

    fn choose(&self, c: Result<(), String>) {
        *self.chosen.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
    }

    fn queue(&self, base_path: &Path, adoption: Option<Adoption>) -> Queue<'_> {
        Queue {
            runs: &self.ws.runs,
            root: self.ws.root().to_path_buf(),
            base_path: base_path.to_path_buf(),
            made: &self.made,
            adoption,
        }
    }

    /// What adopting a study's runs needs (ADR 0201 S3): the project's runs, read before the runner is made sure of, the
    /// base case's project-relative path and the hash of its bytes, and the solver version.
    fn adoption(&self, base_path: &Path) -> Result<Adoption, RpcError> {
        let bytes = std::fs::read(base_path).map_err(|e| {
            err(
                INTERNAL,
                format!("cannot read the base case {}: {e}", base_path.display()),
            )
        })?;
        Ok(Adoption {
            index: RunIndex::read(&self.ws.runs),
            case: self
                .in_project(base_path)
                .unwrap_or_else(|| base_path.display().to_string()),
            case_snapshot_hash: rcfd_project::records::hash_bytes(&bytes),
            solver_version: self
                .ws
                .runs
                .solver_version()
                .unwrap_or_else(|| rcfd_project::records::UNKNOWN_SOLVER_VERSION.to_string()),
        })
    }

    fn ensure_runner(&self) -> Result<(), RpcError> {
        self.ws
            .runs
            .ensure_runner()
            .map_err(|e| err(INTERNAL, format!("the project's run service did not start: {e}")))
    }

    /// Add what the source chose to the method's result (ADR 0189 S7, ADR 0200 S10; with `adopting`, a study's call, the
    /// count of the runs it adopted, ADR 0201 S4).
    fn annotate(&self, out: &mut Value, adopting: bool) {
        let chosen = self.chosen.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match chosen {
            Some(Ok(())) => {
                let made = self.made.lock().unwrap_or_else(|e| e.into_inner());
                let adopted = made.iter().filter(|m| m.adopted).count();
                out["evaluator"] = json!("runner");
                out["runs"] = Value::Array(made.iter().take(MAX_LISTED_RUNS).map(run_object).collect());
                out["runs_total"] = json!(made.len() - adopted);
                if adopting {
                    out["runs_adopted"] = json!(adopted);
                }
            }
            Some(Err(note)) => {
                out["evaluator"] = json!("process");
                out["evaluator_note"] = json!(note);
            }
            None => {}
        }
    }
}

/// The object of one run in a result's `runs` (ADR 0189 S7, ADR 0200 S10).
fn run_object(m: &Made) -> Value {
    let mut o = json!({"run": m.run, "state": m.state});
    if let Some(v) = &m.variant {
        o["variant"] = json!(v);
    }
    if let Some(c) = &m.condition {
        o["condition"] = json!(c);
    }
    if let Some(p) = m.point {
        o["point"] = json!(p);
    }
    if let Some(c) = &m.continue_from {
        o["continue_from"] = json!(c);
    }
    if m.adopted {
        o["adopted"] = json!(true);
    }
    o
}

impl EvaluatorSource for RunnerSource<'_> {
    fn evaluator<'s>(&'s self, study: &StudyRun<'_>) -> Result<Option<Box<dyn Evaluator + 's>>, RpcError> {
        let folder = match self.runner_folder(study) {
            Ok(f) => f,
            Err(note) => {
                self.choose(Err(note));
                return Ok(None);
            }
        };
        let adoption = self.adoption(study.base_path)?;
        self.ensure_runner()?;
        self.choose(Ok(()));
        // ADR 0200 S5: a study's own step cap is the in-process cap of its runs
        let max_steps = study.max_steps.and(study.limits.max_steps).map(|m| m as u64);
        Ok(Some(Box::new(RunnerEvaluator {
            q: self.queue(study.base_path, Some(adoption)),
            study: folder,
            in_process: CaseEvaluator::new(study.base.clone(), study.limits, study.chain),
            chain: study.chain,
            max_steps,
        })))
    }

    fn map_evaluator<'s>(
        &'s self,
        map: &MapRun<'_>,
    ) -> Result<Option<Box<dyn PointEvaluator<State = String> + 's>>, RpcError> {
        if let Err(note) = self.map_rule(map) {
            self.choose(Err(note));
            return Ok(None);
        }
        self.ensure_runner()?;
        self.choose(Ok(()));
        Ok(Some(Box::new(RunnerMapEvaluator {
            q: self.queue(map.base_path, None),
            prefix: map.prefix.to_vec(),
            base: map.base.clone(),
            max_steps: map.max_steps.map(|m| m as u64),
        })))
    }
}

impl Workspace {
    /// `design.run_study`, `design.evaluate` or `design.run_map` of the MCP server `svc` in project mode (ADR 0189 S1, ADR
    /// 0200 S8): the candidates or points are evaluated on the project's runner when [`RunnerSource`] can, else in the server
    /// process as before; the result has the method's keys plus `evaluator` and, for the runner, `runs` and `runs_total`
    /// (and for a study `runs_adopted`, ADR 0201 S4), or `evaluator_note` (ADR 0189 S7, ADR 0200 S10). Any other design
    /// method is the service's.
    pub fn design_call(&self, svc: &Arc<Service>, method: &str, args: &Value) -> Result<Value, RpcError> {
        let source = RunnerSource::new(self);
        let mut out = call_with(svc, method, args, &source)?;
        if ON_RUNNER.contains(&method) {
            source.annotate(&mut out, method != "design.run_map");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_difference_names_the_setting() {
        let a = json!({"physics": {"nu": 0.01}, "mesh": {"nx": 2}});
        assert_eq!(first_difference(&a, &a.clone(), ""), None);
        let b = json!({"physics": {"nu": 0.02}, "mesh": {"nx": 2}});
        assert_eq!(first_difference(&a, &b, "").as_deref(), Some("physics.nu"));
        let c = json!({"physics": {"nu": 0.01}, "mesh": {"nx": 2.0}});
        assert_eq!(first_difference(&a, &c, "").as_deref(), Some("mesh.nx"));
        let d = json!({"physics": {"nu": 0.01}});
        assert_eq!(first_difference(&a, &d, "").as_deref(), Some("mesh"));
        assert_eq!(first_difference(&d, &a, "").as_deref(), Some("mesh"));
    }
}
