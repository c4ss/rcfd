//! Run control model (ADR 0133): rows of the run list, the runner's queue order, the start plan with the field-store
//! proposal (W48-5 (a)) and the runner budgets. No UI code and no threads; file work happens on the run worker
//! ([`crate::poll`]).

use rcfd_api::case::Case;
use rcfd_project::records::RunState;
use rcfd_runner::records::RunSettings;
use rcfd_runner::runner::Config;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// One run as the run worker last read it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRow {
    /// Run id.
    pub id: String,
    /// Case path from `run.json`.
    pub case: String,
    /// Creation time from `run.json`.
    pub created: String,
    /// State from `status.json`.
    pub state: RunState,
    /// Last step.
    pub step: u64,
    /// Last simulated time (NaN when `status.json` holds `null`).
    pub time: f64,
    /// Solver process id while running.
    pub pid: Option<u32>,
    /// Why the run is in its state (failures and cancellations carry one).
    pub reason: Option<String>,
    /// Scheduling priority from `run.json` settings.
    pub priority: i32,
    /// Solver threads from `run.json` settings.
    pub threads: usize,
    /// A pause request is present (`runs/<id>/pause`).
    pub held: bool,
    /// Position in the runner's queue (1 = launched next) for queued runs that are not held.
    pub queue_pos: Option<usize>,
    /// Residuals of the last line of `residuals.jsonl` (`res_u`, `res_p`, `continuity`), NaN for `null`.
    pub last_residuals: BTreeMap<String, f64>,
}

impl RunRow {
    /// The state as `status.json` spells it, or "held" for a queued run with a pause request.
    pub fn state_label(&self) -> String {
        if self.state == RunState::Queued && self.held {
            return "held".into();
        }
        state_name(self.state)
    }
}

/// The `status.json` spelling of a state.
pub fn state_name(s: RunState) -> String {
    rcfd_runner::client::state_name(s)
}

/// Set [`RunRow::queue_pos`]: queued runs without a pause request, ordered as the runner's `schedule` does (priority
/// descending, then `created`, then id), numbered from 1; every other row gets `None`.
pub fn set_queue_positions(rows: &mut [RunRow]) {
    let mut q: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].state == RunState::Queued && !rows[i].held)
        .collect();
    q.sort_by(|&a, &b| {
        let (x, y) = (&rows[a], &rows[b]);
        (y.priority, &x.created, &x.id).cmp(&(x.priority, &y.created, &y.id))
    });
    for r in rows.iter_mut() {
        r.queue_pos = None;
    }
    for (pos, i) in q.into_iter().enumerate() {
        rows[i].queue_pos = Some(pos + 1);
    }
}

/// The residual series name of a residual key of `residuals.jsonl` (`u` -> `res_u`; `continuity` kept), so that
/// [`crate::monitors::MonitorStore::residual_names`] finds it.
pub fn residual_series(key: &str) -> String {
    if key == "continuity" || key.starts_with("res_") {
        key.to_string()
    } else {
        format!("res_{key}")
    }
}

/// What the UI knows about the open project's runs: the run worker's latest rows and the shown run's details. Filled
/// by [`crate::studio::Studio::drain`] from the worker's updates; the UI only reads it.
#[derive(Debug, Clone, Default)]
pub struct RunsView {
    /// Rows by run id.
    pub rows: BTreeMap<String, RunRow>,
    /// Ids the worker knows that the browser does not list yet, sorted by (`created`, id).
    pub extra: Vec<String>,
    /// Latest tick: number and start time.
    pub last_tick: Option<(u64, std::time::Instant)>,
    /// The runner's lock is held (`None` before the first tick).
    pub runner_running: Option<bool>,
    /// A start-runner call is in flight.
    pub starting_runner: bool,
    /// The last located error of `ensure_runner` (shown, not retried).
    pub runner_error: Option<String>,
    /// `status.json` reads that failed (the last known state was kept).
    pub status_errors: u64,
    /// Runner budgets as read.
    pub config: Option<Result<Config, String>>,
    /// The start dialog's plan, once made.
    pub plan: Option<Result<StartPlan, String>>,
    /// Events of the shown run.
    pub events: Vec<rcfd_project::records::RunEvent>,
    /// Last lines of the shown run's `log.txt`.
    pub log: Vec<String>,
    /// `metrics.json` of the shown run, once it ended.
    pub metrics: Option<Value>,
    /// Warnings of the shown run (non-finite values, unparsable lines).
    pub warnings: Vec<String>,
    /// The run whose details these are.
    pub shown: Option<String>,
}

impl RunsView {
    /// The row of `id` as the worker last read it.
    pub fn row(&self, id: &str) -> Option<&RunRow> {
        self.rows.get(id)
    }

    /// Rows shown running or paused.
    pub fn active(&self) -> usize {
        self.rows
            .values()
            .filter(|r| matches!(r.state, RunState::Running | RunState::Paused))
            .count()
    }
}

// ---------------------------------------------------------------- start plan

/// Snapshots the Studio aims at per run (W48-5 (a): at most 100).
pub const STORE_SNAPSHOTS: u64 = 100;

/// Fields the store keeps when the case names none (`rcfd_api::fieldstore::DEFAULT_FIELDS`).
pub const STORE_FIELDS: u64 = 4;

/// What the start dialog read from a case file.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseFacts {
    /// `solver.algorithm`.
    pub algorithm: String,
    /// `solver.max_iterations`.
    pub max_iterations: u64,
    /// `solver.end_time` (0 when not set).
    pub end_time: f64,
    /// Cells, when the mesh generator's counts give them (`None`: a mesh file or a generator not counted here).
    pub cells: Option<u64>,
    /// The case text sets `output.store_interval` or `output.store_time_interval` (any value, 0 included).
    pub sets_store: bool,
    /// The case text sets `output.store_precision`.
    pub sets_precision: bool,
    /// `dynamic_mesh.enabled` (the store does not support a moving mesh).
    pub dynamic_mesh: bool,
}

/// How often the store writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StoreEvery {
    /// `output.store_interval` (steps or iterations).
    Steps(u64),
    /// `output.store_time_interval` (simulated seconds).
    Time(f64),
}

/// The field store of one Studio-started run.
#[derive(Debug, Clone, PartialEq)]
pub struct StorePlan {
    /// Store fields for this run (the dialog's switch).
    pub on: bool,
    /// Snapshot spacing.
    pub every: StoreEvery,
    /// Lossy single precision (the Studio's default) instead of f64.
    pub f32: bool,
}

/// Everything the start dialog passes to [`rcfd_runner::client::Client::start`].
#[derive(Debug, Clone, PartialEq)]
pub struct StartPlan {
    /// Project-relative case path (`cases/...`).
    pub case: String,
    /// What the case says.
    pub facts: CaseFacts,
    /// The store proposal; `None` when the case sets the store itself or the store cannot be used.
    pub store: Option<StorePlan>,
    /// Why there is no proposal (shown in the dialog).
    pub note: Option<String>,
    /// Runner priority (higher first).
    pub priority: i32,
    /// Solver threads.
    pub threads: usize,
    /// Declared memory need, MB (`None`: the runner estimates it).
    pub mem_mb: Option<u64>,
    /// The user's own `path=value` overrides, applied after the store keys.
    pub extra_set: Vec<String>,
}

fn steady(alg: &str) -> bool {
    matches!(alg, "simple" | "simplec" | "coupled")
}

fn text_sets(raw: &toml::Value, path: &[&str]) -> bool {
    let mut v = raw;
    for p in path {
        match v.get(p) {
            Some(x) => v = x,
            None => return false,
        }
    }
    true
}

/// Read the facts of a case text (`base_dir`: where its relative data files are). The store keys are looked for in the
/// text, not in the resolved settings: an explicit `0` is the case's own choice, and the resolved tree cannot tell it
/// from the default.
pub fn case_facts(text: &str, base_dir: &Path) -> Result<CaseFacts, String> {
    let raw: toml::Value = toml::from_str(text).map_err(|e| format!("not TOML: {}", e.message()))?;
    let case = Case::parse_toml_in(text, base_dir).map_err(|errs| {
        errs.iter()
            .map(|d| format!("{}: {}", d.path, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let int = |p: &str| get(p).as_u64().unwrap_or(0);
    let generator = get("mesh.generator").as_str().unwrap_or_default().to_string();
    let source = get("mesh.source").as_str().unwrap_or_default().to_string();
    let cells = (source == "generator")
        .then_some(())
        .and_then(|()| match generator.as_str() {
            "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" | "cylinder_ogrid" => {
                Some(int("mesh.nx") * int("mesh.ny"))
            }
            "annulus_3d" | "annulus_3d_sector" => Some(int("mesh.nx") * int("mesh.ny") * int("mesh.nz")),
            _ => None,
        });
    Ok(CaseFacts {
        algorithm: get("solver.algorithm").as_str().unwrap_or_default().to_string(),
        max_iterations: int("solver.max_iterations"),
        end_time: get("solver.end_time").as_f64().unwrap_or(0.0),
        cells,
        sets_store: text_sets(&raw, &["output", "store_interval"])
            || text_sets(&raw, &["output", "store_time_interval"]),
        sets_precision: text_sets(&raw, &["output", "store_precision"]),
        dynamic_mesh: get("dynamic_mesh.enabled").as_bool() == Some(true),
    })
}

/// The store proposal of W48-5 (a) for `facts`: `None` with a note when the case sets the store itself or uses a moving
/// mesh. Steady algorithms get `store_interval = ceil(max_iterations / 100)` (so `ceil(M / k) <= 100` snapshots,
/// the final state included); `piso` and `pimple` with an `end_time` get `store_time_interval = end_time / 100`; other
/// algorithms get the step rule. Precision f32 (lossy, half the size).
pub fn propose_store(facts: &CaseFacts) -> (Option<StorePlan>, Option<String>) {
    if facts.sets_store {
        return (None, Some("the case sets its own field store (output.store_*)".into()));
    }
    if facts.dynamic_mesh {
        return (
            None,
            Some("no field store: the store does not support dynamic_mesh.enabled".into()),
        );
    }
    let every =
        if !steady(&facts.algorithm) && matches!(facts.algorithm.as_str(), "piso" | "pimple") && facts.end_time > 0.0 {
            StoreEvery::Time(facts.end_time / STORE_SNAPSHOTS as f64)
        } else {
            StoreEvery::Steps(facts.max_iterations.div_ceil(STORE_SNAPSHOTS).max(1))
        };
    (
        Some(StorePlan {
            on: true,
            every,
            f32: true,
        }),
        None,
    )
}

impl StartPlan {
    /// The dialog's first state for the case at `rel` with `text` (defaults: priority 0, 1 thread, memory estimated by
    /// the runner, the store proposal on).
    pub fn new(rel: &str, text: &str, base_dir: &Path) -> Result<StartPlan, String> {
        let facts = case_facts(text, base_dir).map_err(|e| format!("{rel}: {e}"))?;
        let (store, note) = propose_store(&facts);
        Ok(StartPlan {
            case: rel.to_string(),
            facts,
            store,
            note,
            priority: 0,
            threads: 1,
            mem_mb: None,
            extra_set: Vec::new(),
        })
    }

    /// The `rcfd run --set` entries: the store keys (when the proposal is on), then the user's overrides. f64 is the
    /// store's default, so it adds no key.
    pub fn set(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(s) = self.store.as_ref().filter(|s| s.on) {
            v.push(match s.every {
                StoreEvery::Steps(k) => format!("output.store_interval={k}"),
                StoreEvery::Time(dt) => format!("output.store_time_interval={dt}"),
            });
            if s.f32 && !self.facts.sets_precision {
                v.push("output.store_precision=f32".into());
            }
        }
        v.extend(self.extra_set.iter().filter(|s| !s.trim().is_empty()).cloned());
        v
    }

    /// The runner's settings of this plan.
    pub fn settings(&self) -> RunSettings {
        RunSettings {
            priority: self.priority,
            threads: self.threads.max(1),
            mem_mb: self.mem_mb,
            set: self.set(),
        }
    }

    /// Snapshots the store is expected to hold at most (steps rule: `ceil(M / k)`; time rule: the target count).
    pub fn snapshots(&self) -> Option<u64> {
        let s = self.store.as_ref().filter(|s| s.on)?;
        Some(match s.every {
            StoreEvery::Steps(k) => self.facts.max_iterations.div_ceil(k.max(1)).max(1),
            StoreEvery::Time(_) => STORE_SNAPSHOTS,
        })
    }

    /// Estimated store size in bytes (cells x 4 fields x snapshots x 4 or 8 bytes), when the cell count is known. A
    /// proposal for the dialog, not a measured value: compression and the mesh stored once are not counted.
    pub fn estimated_bytes(&self) -> Option<u64> {
        let s = self.store.as_ref().filter(|s| s.on)?;
        let width = if s.f32 { 4 } else { 8 };
        Some(self.facts.cells? * STORE_FIELDS * self.snapshots()? * width)
    }
}

// ---------------------------------------------------------------- budgets

/// Check runner budgets before they are written (the runner refuses zero `max_runs`, `cpu_threads`, `poll_ms`).
pub fn check_config(c: &Config) -> Result<(), String> {
    if c.max_runs == 0 || c.cpu_threads == 0 || c.poll_ms == 0 {
        return Err("max_runs, cpu_threads and poll_ms must be positive".into());
    }
    if !(c.mem_base_mb.is_finite() && c.mem_kb_per_cell.is_finite()) || c.mem_base_mb < 0.0 || c.mem_kb_per_cell < 0.0 {
        return Err("mem_base_mb and mem_kb_per_cell must be finite and not negative".into());
    }
    Ok(())
}

/// Write `.rcfd/runner-config.json` of `project` atomically (all keys, as the runner's reader expects; it reads the file
/// when it starts).
pub fn save_config(project: &Path, c: &Config) -> Result<(), String> {
    check_config(c)?;
    let path = rcfd_runner::runner::config_path(project);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut bytes = serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    rcfd_project::atomic::atomic_write(&path, &bytes).map_err(|e| e.to_string())
}
