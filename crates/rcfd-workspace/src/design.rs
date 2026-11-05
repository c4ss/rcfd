//! Design study evaluations on the project's runner (ADR 0189, ADR 0173 W7). In project mode the MCP tools
//! `design_run_study` and `design_evaluate` evaluate each candidate of a study at each operating condition as one run of the
//! project's runner, tagged with its study, variant and condition (`run.json`), and read the result from the run's
//! `metrics.json`; so the runs are counted by the runner's budget, outlive the server and show in the Studio.
//!
//! [`Workspace::design_call`] runs `design.run_study` or `design.evaluate` through `rcfd_api::design::tools::call_with`, the
//! same code as in process (loading the study, the allowance, the store, the result), with [`RunnerSource`] choosing the
//! evaluator: [`RunnerEvaluator`] when a run of `rcfd run` is the evaluation the server would make (S3), else the in-process
//! evaluator, as before, with the reason in the result (S7). A run is the base case file with `rcfd run --set` entries
//! ([`entries`]) whose settings tree is checked to be the candidate's case before it is queued (S4); its result follows the
//! in-process status rule on the solver's `finish` event ([`cond_result`], S5).

use crate::runs::RunService;
use crate::{err, Workspace, INTERNAL};
use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseEvaluator, CaseRun};
use rcfd_api::design::opmap::PointStatus;
use rcfd_api::design::store::CondResult;
use rcfd_api::design::study::{Condition, Evaluator, StudyDef};
use rcfd_api::design::tools::{call_with, EvaluatorSource, StudyRun};
use rcfd_api::runctl::exit;
use rcfd_api::service::{RpcError, Service};
use rcfd_runner::client::state_name;
use rcfd_runner::records::RunSettings;
use rcfd_runner::store::RunTags;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The design methods that [`Workspace::design_call`] evaluates on the runner.
pub const ON_RUNNER: [&str; 2] = ["design.run_study", "design.evaluate"];
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

/// The condition result of a finished run (S5): the status by the in-process rule (`CaseRun::status`) on the solver's
/// `finish` event in `metrics.json` (`solver_finish`), usable when converged with at least one finite report value, the
/// metrics kept only then; a run without a `finish` event (ended before launch, cancelled, killed) failed with `reason`, the
/// run's status reason.
pub fn cond_result(label: &str, metrics: &Value, reason: &str) -> CondResult {
    let Some(fin) = metrics.get("solver_finish").filter(|f| f.is_object()) else {
        let why = if reason.is_empty() {
            "the run ended without a solver result"
        } else {
            reason
        };
        return CondResult::failure(label, "failed", why);
    };
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
    let run = CaseRun {
        exit_code,
        state: fin["state"].as_str().unwrap_or("").to_string(),
        iterations: fin["iterations"].as_u64().unwrap_or(0) as usize,
        metrics: values,
        message: notes.join("; "),
        fields: None,
        warm: false,
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

/// One run made by a [`RunnerEvaluator`].
#[derive(Debug, Clone)]
struct Made {
    run: String,
    variant: String,
    condition: String,
    state: String,
}

/// The evaluator of a study on the project's runner (S4): one tagged run per operating condition of a candidate, all
/// queued together, each waited for and read from its `metrics.json`.
pub struct RunnerEvaluator<'s> {
    runs: &'s RunService,
    root: PathBuf,
    study: String,
    base_path: PathBuf,
    in_process: CaseEvaluator,
    made: &'s Mutex<Vec<Made>>,
}

impl RunnerEvaluator<'_> {
    fn fail(label: &str, message: String) -> CondResult {
        CondResult::failure(label, "failed", message)
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

    /// Queue the run of `x` at `cond`, or the condition's failure when no run can be made.
    fn submit(&self, def: &StudyDef, x: &[f64], cond: &Condition, variant: &str) -> Result<String, CondResult> {
        let label = &cond.label;
        let want = self
            .in_process
            .build_case(def, x, Some(cond))
            .map_err(|m| Self::fail(label, m))?;
        let set = entries(def, x, cond);
        let got = self.case_of(&set).map_err(|m| Self::fail(label, m))?;
        if let Some(at) = first_difference(&want.tree.to_json(), &got.tree.to_json(), "") {
            return Err(Self::fail(
                label,
                format!("the run's settings would differ from the candidate's case at {at}; no run was made"),
            ));
        }
        let tags = RunTags {
            study: Some(self.study.clone()),
            variant: Some(variant.to_string()),
            condition: Some(label.clone()),
        };
        let settings = RunSettings {
            set,
            ..RunSettings::default()
        };
        let id = self
            .runs
            .submit_tagged(&self.base_path, &settings, &tags)
            .map_err(|e| Self::fail(label, format!("the run could not be queued: {e}")))?;
        self.made.lock().unwrap_or_else(|e| e.into_inner()).push(Made {
            run: id.clone(),
            variant: variant.to_string(),
            condition: label.clone(),
            state: "queued".into(),
        });
        Ok(id)
    }

    fn note_state(&self, id: &str, state: &str) {
        let mut made = self.made.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = made.iter_mut().find(|m| m.run == id) {
            m.state = state.to_string();
        }
    }

    /// Wait for run `id` (no time limit, as in process; `cancel_run` ends it) and read its result.
    fn wait_result(&self, id: &str, label: &str) -> CondResult {
        let mut checked = Instant::now();
        loop {
            match self.runs.status(id) {
                Ok(st) if st.state.is_terminal() => {
                    let file = self.root.join("runs").join(id).join("metrics.json");
                    let metrics: Value = std::fs::read(&file)
                        .ok()
                        .and_then(|b| serde_json::from_slice(&b).ok())
                        .unwrap_or(Value::Null);
                    self.note_state(id, &state_name(st.state));
                    return cond_result(label, &metrics, st.reason.as_deref().unwrap_or(""));
                }
                Ok(_) => {}
                Err(e) => return Self::fail(label, e),
            }
            if checked.elapsed() >= RUNNER_CHECK {
                checked = Instant::now();
                if !self.runs.runner_running() {
                    if let Err(e) = self.runs.ensure_runner() {
                        return Self::fail(
                            label,
                            format!(
                                "the project's run service stopped and did not start again: {e}; run {id} stays queued"
                            ),
                        );
                    }
                }
            }
            std::thread::sleep(POLL);
        }
    }
}

impl Evaluator for RunnerEvaluator<'_> {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        let variant = variant_name(def, x);
        let queued: Vec<Result<String, CondResult>> = def
            .conditions
            .iter()
            .map(|c| self.submit(def, x, c, &variant))
            .collect();
        if queued.iter().any(Result::is_ok) {
            // a runner that cannot start is reported by the wait, once per run
            let _ = self.runs.ensure_runner();
        }
        queued
            .into_iter()
            .zip(&def.conditions)
            .map(|(q, c)| match q {
                Ok(id) => self.wait_result(&id, &c.label),
                Err(r) => r,
            })
            .collect()
    }
}

/// The evaluator source of a project (S3): [`RunnerEvaluator`] when the runner can make the evaluation the server would make,
/// else the in-process evaluator with the reason; it keeps what it chose and the runs it made for the result (S7).
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

    /// `path` relative to the project, both canonical (`None` when outside it).
    fn in_project(&self, path: &Path) -> Option<String> {
        let root = rcfd_project::paths::canonicalize(self.ws.root()).ok()?;
        let p = rcfd_project::paths::canonicalize(path).ok()?;
        rcfd_project::paths::rel_string(&root, &p).filter(|r| !r.is_empty())
    }

    /// Why the runner cannot make this study's evaluations (S3), or its project-relative folder when it can.
    fn runner_folder(&self, study: &StudyRun<'_>) -> Result<String, String> {
        if self.in_project(study.base_path).is_none() {
            return Err(format!(
                "the base case {} is not in the project, so the runner cannot run it",
                study.base_path.display()
            ));
        }
        let Some(folder) = self.in_project(study.dir) else {
            return Err(format!(
                "the study folder {} is not in the project",
                study.dir.display()
            ));
        };
        let n = study.def.conditions.len();
        if study.chain && n > 1 {
            return Err(format!(
                "chain: each of the study's {n} operating conditions starts from the previous one's result, which a run \
                 of the runner cannot do yet (ADR 0189 Q2); create the study with chain false to run it on the runner"
            ));
        }
        if let Some(m) = study.max_steps {
            return Err(format!(
                "max_steps: the study caps each case run at {m} steps, which a run of the runner cannot do yet (ADR 0189 Q3)"
            ));
        }
        Ok(folder)
    }

    /// Add what the source chose to the method's result (S7).
    fn annotate(&self, out: &mut Value) {
        let chosen = self.chosen.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match chosen {
            Some(Ok(())) => {
                let made = self.made.lock().unwrap_or_else(|e| e.into_inner());
                out["evaluator"] = json!("runner");
                out["runs"] = Value::Array(
                    made.iter()
                        .take(MAX_LISTED_RUNS)
                        .map(
                            |m| json!({"run": m.run, "variant": m.variant, "condition": m.condition, "state": m.state}),
                        )
                        .collect(),
                );
                out["runs_total"] = json!(made.len());
            }
            Some(Err(note)) => {
                out["evaluator"] = json!("process");
                out["evaluator_note"] = json!(note);
            }
            None => {}
        }
    }
}

impl EvaluatorSource for RunnerSource<'_> {
    fn evaluator<'s>(&'s self, study: &StudyRun<'_>) -> Result<Option<Box<dyn Evaluator + 's>>, RpcError> {
        let folder = match self.runner_folder(study) {
            Ok(f) => f,
            Err(note) => {
                *self.chosen.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(note));
                return Ok(None);
            }
        };
        self.ws
            .runs
            .ensure_runner()
            .map_err(|e| err(INTERNAL, format!("the project's run service did not start: {e}")))?;
        *self.chosen.lock().unwrap_or_else(|e| e.into_inner()) = Some(Ok(()));
        Ok(Some(Box::new(RunnerEvaluator {
            runs: &self.ws.runs,
            root: self.ws.root().to_path_buf(),
            study: folder,
            base_path: study.base_path.to_path_buf(),
            in_process: CaseEvaluator::new(study.base.clone(), study.limits, study.chain),
            made: &self.made,
        })))
    }
}

impl Workspace {
    /// `design.run_study` or `design.evaluate` of the MCP server `svc` in project mode (ADR 0189 S1): the candidates are
    /// evaluated on the project's runner when [`RunnerSource`] can, else in the server process as before; the result has
    /// the method's keys plus `evaluator` and, for the runner, `runs` and `runs_total`, or `evaluator_note` (S7). Any other
    /// design method is the service's.
    pub fn design_call(&self, svc: &Arc<Service>, method: &str, args: &Value) -> Result<Value, RpcError> {
        let source = RunnerSource::new(self);
        let mut out = call_with(svc, method, args, &source)?;
        if ON_RUNNER.contains(&method) {
            source.annotate(&mut out);
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
