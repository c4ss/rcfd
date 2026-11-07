//! Running rcfd cases for the design loop (ADR 0067): one case run with an optional warm start, and the evaluators built on it
//! (the candidate evaluator of a study and the point evaluator of an operating map). The warm start carries velocity and
//! pressure ([`WarmStart::Flow`], the default), or also the temperature and the k-epsilon / k-omega SST fields
//! ([`WarmStart::Extended`], ADR 0209 D4).

use super::opmap::{MapPoint, PointEvaluator, PointResult, PointStatus};
use super::store::CondResult;
use super::study::{Condition, Evaluator, StudyDef};
use crate::case::Case;
use crate::runctl::{self, exit, RunOptions};
use crate::sim::{Simulation, WarmState};
use rcfd_core::vec3::Vec3;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Velocity and pressure of a converged run, used to start the next one.
#[derive(Debug, Clone)]
pub struct Fields {
    /// Cell velocity.
    pub u: Vec<Vec3>,
    /// Cell (kinematic) pressure.
    pub p: Vec<f64>,
    /// The full warm state ([`WarmStart::Extended`] only; `None` under [`WarmStart::Flow`]).
    pub state: Option<WarmState>,
}

/// What a warm start carries (ADR 0209 D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WarmStart {
    /// Velocity and pressure ([`Simulation::seed_fields`]): steady SIMPLE/SIMPLEC without energy or turbulence.
    #[default]
    Flow,
    /// Also the temperature and the k-epsilon / k-omega SST fields ([`Simulation::seed_state`]).
    Extended,
}

/// Limits of one case run.
#[derive(Debug, Clone, Copy)]
pub struct RunLimits {
    /// Refuse meshes with more cells.
    pub max_cells: usize,
    /// Step cap of the run (`None` = the case's own `max_iterations` or end time).
    pub max_steps: Option<usize>,
}

impl Default for RunLimits {
    fn default() -> Self {
        RunLimits {
            max_cells: 2_000_000,
            max_steps: None,
        }
    }
}

/// Result of [`run_case`].
#[derive(Debug, Clone)]
pub struct CaseRun {
    /// Run-control exit code.
    pub exit_code: u8,
    /// Run-control state (`converged`, `finished`, `not_converged`, `diverged`, `failed`, ...).
    pub state: String,
    /// Iterations or steps.
    pub iterations: usize,
    /// All reports of the case that evaluated to a finite number.
    pub metrics: BTreeMap<String, f64>,
    /// Failure reason and notes.
    pub message: String,
    /// Final fields of a converged run (steady SIMPLE/SIMPLEC without energy or turbulence), for chaining.
    pub fields: Option<Fields>,
    /// Whether the supplied warm start was applied.
    pub warm: bool,
}

impl CaseRun {
    fn failed(code: u8, state: &str, message: String) -> CaseRun {
        CaseRun {
            exit_code: code,
            state: state.into(),
            iterations: 0,
            metrics: BTreeMap::new(),
            message,
            fields: None,
            warm: false,
        }
    }

    /// Classified status.
    pub fn status(&self) -> PointStatus {
        match self.state.as_str() {
            "converged" | "finished" if self.exit_code == exit::OK => PointStatus::Converged,
            "diverged" => PointStatus::Diverged,
            "not_converged" | "step_limit" | "time_limit" => PointStatus::NotConverged,
            _ => PointStatus::Failed,
        }
    }
}

/// Run `case` (built fresh), optionally starting from `seed`. Never panics; a seed that cannot be applied is ignored and
/// the run is cold (`warm == false`, the reason is in `message`).
pub fn run_case(case: &Case, seed: Option<&Fields>, limits: &RunLimits) -> CaseRun {
    run_case_with(case, seed, limits, WarmStart::Flow)
}

/// [`run_case`] with the warm start `warm` (ADR 0209 D4): under [`WarmStart::Extended`] the seed is applied by
/// [`Simulation::seed_state`] and a converged steady run returns its [`Simulation::warm_state`] in `fields.state`.
pub fn run_case_with(case: &Case, seed: Option<&Fields>, limits: &RunLimits, warm: WarmStart) -> CaseRun {
    match catch_unwind(AssertUnwindSafe(|| run_case_inner(case, seed, limits, warm))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "panic".into());
            CaseRun::failed(exit::FAIL, "failed", format!("internal error: {msg}"))
        }
    }
}

fn run_case_inner(case: &Case, seed: Option<&Fields>, limits: &RunLimits, mode: WarmStart) -> CaseRun {
    let sem = case.semantic_errors();
    if !sem.is_empty() {
        return CaseRun::failed(
            exit::INVALID,
            "failed",
            sem.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        );
    }
    let mut sim = match Simulation::new(case) {
        Ok(s) => s,
        Err(d) => {
            return CaseRun::failed(
                exit::INVALID,
                "failed",
                d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
            )
        }
    };
    if sim.geom.n_cells() > limits.max_cells {
        return CaseRun::failed(
            exit::INVALID,
            "failed",
            format!("mesh has {} cells, limit is {}", sim.geom.n_cells(), limits.max_cells),
        );
    }
    let q = rcfd_mesh::quality::check_mesh(&sim.geom.mesh);
    if !q.is_ok() {
        return CaseRun::failed(exit::BAD_MESH, "failed", format!("mesh error: {}", q.errors.join("; ")));
    }
    let rd = crate::rules::check_case(case, Some(&crate::rules::mesh_context(&q)));
    if crate::rules::has_errors(&rd) {
        return CaseRun::failed(
            exit::INVALID,
            "failed",
            rd.iter()
                .filter(|d| d.severity == crate::rules::Severity::Error)
                .map(|d| format!("{} {}: {}", d.id, d.path, d.message))
                .collect::<Vec<_>>()
                .join("; "),
        );
    }
    let mut notes = Vec::new();
    let mut warm = false;
    if let Some(f) = seed {
        let applied = match mode {
            WarmStart::Flow => sim.seed_fields(f.u.clone(), f.p.clone()),
            WarmStart::Extended => match &f.state {
                Some(st) => sim.seed_state(st),
                None => sim.seed_state(&WarmState {
                    u: f.u.clone(),
                    p: f.p.clone(),
                    t: None,
                    turbulence: None,
                }),
            },
        };
        match applied {
            Ok(()) => warm = true,
            Err(e) => notes.push(format!("cold start: {e}")),
        }
    }
    let opts = RunOptions {
        max_steps: limits.max_steps,
        info: json!({"driver": "design"}),
        ..RunOptions::default()
    };
    let sim = Mutex::new(sim);
    let outcome = runctl::drive(&sim, &opts, &|| false, &mut |_| {});
    let sim = sim.into_inner().unwrap_or_else(|e| e.into_inner());
    let mut metrics = BTreeMap::new();
    for (name, v) in sim.report_values() {
        match v {
            Ok(v) if v.value.is_finite() => {
                metrics.insert(name, v.value);
            }
            Ok(_) => notes.push(format!("report {name}: not finite")),
            Err(e) => notes.push(format!("report {name}: {e}")),
        }
    }
    let ok = outcome.exit_code == exit::OK;
    if !ok {
        notes.insert(0, outcome.reason.clone());
    }
    let fields = if ok && sim.is_steady() {
        Some(Fields {
            u: sim.velocity().values.clone(),
            p: sim.pressure().values.clone(),
            state: match mode {
                WarmStart::Flow => None,
                WarmStart::Extended => sim.warm_state(),
            },
        })
    } else {
        None
    };
    CaseRun {
        exit_code: outcome.exit_code,
        state: outcome.state.to_string(),
        iterations: outcome.iterations,
        metrics,
        message: notes.join("; "),
        fields,
        warm,
    }
}

/// Number from a parameter value, written to a settings path.
fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Hook applied to the case after the parameter targets, for geometry that is not one settings path.
pub type Generator = Box<dyn Fn(&mut Case, &[f64]) -> Result<(), String> + Send + Sync>;

/// Evaluator of study candidates by running a base case.
pub struct CaseEvaluator {
    /// The base case.
    pub base: Case,
    /// Run limits.
    pub limits: RunLimits,
    /// Start each condition from the previous converged one (same candidate).
    pub chain: bool,
    /// Optional generator hook (called after the parameter targets, before the condition overrides).
    pub hook: Option<Generator>,
    /// Case runs made so far (a cache hit does not count).
    pub runs: AtomicUsize,
    /// What a chained condition carries from the one before (ADR 0209 D4; `Flow` from [`CaseEvaluator::new`]).
    pub warm: WarmStart,
}

impl CaseEvaluator {
    /// Evaluator for `base`.
    pub fn new(base: Case, limits: RunLimits, chain: bool) -> CaseEvaluator {
        CaseEvaluator {
            base,
            limits,
            chain,
            hook: None,
            runs: AtomicUsize::new(0),
            warm: WarmStart::Flow,
        }
    }

    /// The case of candidate `x` at condition `cond`.
    pub fn build_case(&self, def: &StudyDef, x: &[f64], cond: Option<&Condition>) -> Result<Case, String> {
        let mut c = self.base.clone();
        for (p, &v) in def.params.iter().zip(x) {
            for t in &p.targets {
                crate::study::apply_value(&mut c, &t.path, &num(t.scale * v + t.offset))
                    .map_err(|e| format!("parameter '{}': {}: {e}", p.name, t.path))?;
            }
        }
        if let Some(h) = &self.hook {
            h(&mut c, x).map_err(|e| format!("generator hook: {e}"))?;
        }
        if let Some(cond) = cond {
            for (path, v) in &cond.set {
                crate::study::apply_value(&mut c, path, v)
                    .map_err(|e| format!("condition '{}': {path}: {e}", cond.label))?;
            }
        }
        Ok(c)
    }
}

impl Evaluator for CaseEvaluator {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        let mut out = Vec::new();
        let mut prev: Option<Fields> = None;
        for cond in &def.conditions {
            let case = match self.build_case(def, x, Some(cond)) {
                Ok(c) => c,
                Err(e) => {
                    out.push(CondResult::failure(&cond.label, "failed", e));
                    prev = None;
                    continue;
                }
            };
            self.runs.fetch_add(1, Ordering::SeqCst);
            let seed = if self.chain { prev.as_ref() } else { None };
            let run = run_case_with(&case, seed, &self.limits, self.warm);
            let status = run.status();
            let ok = status == PointStatus::Converged && !run.metrics.is_empty();
            out.push(CondResult {
                label: cond.label.clone(),
                ok,
                status: status.name().to_string(),
                iterations: run.iterations,
                metrics: if ok { run.metrics.clone() } else { BTreeMap::new() },
                message: run.message.clone(),
            });
            prev = if ok { run.fields } else { None };
        }
        out
    }
}

/// Operating-map point evaluator over a base case; the state is the converged fields.
pub struct CaseMapEvaluator {
    /// The base case.
    pub base: Case,
    /// Run limits.
    pub limits: RunLimits,
}

impl PointEvaluator for CaseMapEvaluator {
    type State = Fields;

    fn run(&self, point: &MapPoint, warm: Option<&Fields>) -> PointResult<Fields> {
        map_point(&self.base, &self.limits, point, warm, WarmStart::Flow)
    }
}

/// Operating-map point evaluator with the extended warm start (ADR 0209 D4): each point starts from the previous converged
/// point's velocity, pressure, temperature and k-epsilon / k-omega SST fields where the case allows it.
pub struct ExtendedMapEvaluator {
    /// The base case.
    pub base: Case,
    /// Run limits.
    pub limits: RunLimits,
}

impl PointEvaluator for ExtendedMapEvaluator {
    type State = Fields;

    fn run(&self, point: &MapPoint, warm: Option<&Fields>) -> PointResult<Fields> {
        map_point(&self.base, &self.limits, point, warm, WarmStart::Extended)
    }
}

fn map_point(
    base: &Case,
    limits: &RunLimits,
    point: &MapPoint,
    warm: Option<&Fields>,
    mode: WarmStart,
) -> PointResult<Fields> {
    let mut c = base.clone();
    for (path, v) in &point.set {
        if let Err(e) = crate::study::apply_value(&mut c, path, v) {
            return PointResult::failed(format!("{path}: {e}"));
        }
    }
    let run = run_case_with(&c, warm, limits, mode);
    let status = run.status();
    let usable = status == PointStatus::Converged;
    PointResult {
        status,
        exit_code: run.exit_code,
        state_name: run.state.clone(),
        iterations: run.iterations,
        metrics: if usable {
            run.metrics.iter().map(|(k, v)| (k.clone(), *v)).collect()
        } else {
            Vec::new()
        },
        message: run.message.clone(),
        warm_applied: run.warm,
        state: if usable { run.fields } else { None },
    }
}
