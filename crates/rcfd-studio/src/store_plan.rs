//! The field store of a Studio-started run, as the start dialog shows it (ADR 0140, wave 48G job 3): what the case text
//! says about the store, the store that will really be written once the proposal and the user's overrides are applied,
//! how many snapshots it holds and how large it is, the dialog's lines and warnings with their one-click fixes, the cell
//! count of a mesh the plan cannot count from its parameters, and the plan of a "Re-run with fields stored".
//!
//! Pure functions of a [`StartPlan`] (and of files for the cell count and the re-run); no UI and no threads. The proposal
//! itself stays in [`crate::runs::propose_store`] (D1).

use crate::runs::{propose_store, StartPlan, StoreEvery, StorePlan, STORE_SNAPSHOTS};
use rcfd_api::case::Case;
use rcfd_project::records::RunRecord;
use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use serde_json::Value;
use std::path::Path;

/// Estimates above this many bytes are "huge" (D6: 1 GB, decimal).
pub const HUGE_BYTES: u64 = 1_000_000_000;
/// D4: file header and trailer, the mesh frame's head, prefix and JSON, the footer frame and `index.json`.
pub const FIXED_BYTES: u64 = 900;
/// D4: per snapshot, its directory frame head and JSON frame, its footer entry and its `index.json` entry.
pub const SNAP_BYTES: u64 = 115;
/// D4: per field and snapshot, the field's JSON in the snapshot directory.
pub const FIELD_BYTES: u64 = 113;
/// D4: per block of a field, its DATA frame head and its offset in the JSON.
pub const BLOCK_BYTES: u64 = 40;
/// D4: mesh bytes per cell of the 2D (one cell thick) generators when the mesh was not built.
pub const MESH_2D_PER_CELL: u64 = 264;
/// D4: mesh bytes per cell of every other mesh when it was not built.
pub const MESH_3D_PER_CELL: u64 = 196;
/// Values per block of the store's default write options.
pub const BLOCK_VALUES: u64 = rcfd_tsstore::DEFAULT_BLOCK_VALUES as u64;

/// The store keys of `[output]`.
pub const STORE_KEYS: [&str; 4] = [
    "output.store_interval",
    "output.store_time_interval",
    "output.store_fields",
    "output.store_precision",
];

/// Step-control keys that make a piso/pimple time step adaptive (`rcfd-api` `timectl::requested`), besides
/// `solver.max_courant`.
const STEP_KEYS: [&str; 12] = [
    "solver.max_courant_rms",
    "solver.dt_growth",
    "solver.dt_shrink",
    "solver.dt_safety",
    "solver.min_dt",
    "solver.max_dt",
    "solver.error_tolerance",
    "solver.period",
    "solver.steps_per_period",
    "solver.extrapolate_initial",
    "output.write_times",
    "output.write_time_interval",
];

/// The 2D generators of D4 (one cell thick, `empty` front and back).
const GEN_2D: [&str; 6] = [
    "cavity_2d",
    "box_2d",
    "channel_2d",
    "wedge_2d",
    "nozzle_2d",
    "cylinder_ogrid",
];

/// Where the plan's cell count comes from.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CellsState {
    /// From the generator's counts (`case_facts`).
    Table,
    /// From the mesh, built on a control thread (D5).
    Built,
    /// Being counted.
    Counting,
    /// The mesh could not be built (located message).
    Failed(String),
    /// Not counted (no count asked for).
    #[default]
    Unknown,
}

/// What the case text says beyond [`crate::runs::CaseFacts`]'s first fields (read once by `case_facts`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CaseExtra {
    /// `compressible.mode = "steady"` with `solver.algorithm = "compressible"`.
    pub compressible_steady: bool,
    /// The store keys the case text sets, in [`STORE_KEYS`] order, with their values.
    pub case_store: Vec<(String, Value)>,
    /// Other `output.*` keys the text sets, sorted.
    pub output_keys: Vec<String>,
    /// `output.store_fields` as resolved (empty: the store's four defaults).
    pub store_fields: Vec<String>,
    /// `solver.dt`.
    pub dt: f64,
    /// A fixed time step: `solver.max_courant = 0` and no step-control key set.
    pub fixed_dt: bool,
    /// `chain.sequence` is not empty.
    pub chain: bool,
    /// One of the 2D generators of D4.
    pub two_d: bool,
    /// Mesh bytes of the built mesh (D4), once counted.
    pub mesh_bytes: Option<u64>,
    /// Where `cells` comes from.
    pub cells_state: CellsState,
}

fn set_value(v: &Value) -> bool {
    v.as_f64().is_some_and(|x| x != 0.0) || v.as_bool() == Some(true) || v.as_array().is_some_and(|a| !a.is_empty())
}

/// Read [`CaseExtra`] from the raw TOML of a case and the parsed case (`counted`: the generator's counts gave the cells).
pub fn case_extra(raw: &toml::Value, case: &Case, counted: bool) -> CaseExtra {
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let num = |p: &str| get(p).as_f64().unwrap_or(0.0);
    let output = raw.get("output").and_then(toml::Value::as_table);
    let in_text = |key: &str| output.is_some_and(|t| t.contains_key(key));
    let case_store = STORE_KEYS
        .iter()
        .filter(|k| in_text(k.trim_start_matches("output.")))
        .map(|k| (k.to_string(), get(k)))
        .collect();
    let mut output_keys: Vec<String> = output
        .map(|t| {
            t.keys()
                .map(|k| format!("output.{k}"))
                .filter(|k| !STORE_KEYS.contains(&k.as_str()))
                .collect()
        })
        .unwrap_or_default();
    output_keys.sort();
    let store_fields = get("output.store_fields")
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let generator = get("mesh.generator").as_str().unwrap_or_default().to_string();
    CaseExtra {
        compressible_steady: get("solver.algorithm").as_str() == Some("compressible")
            && get("compressible.mode").as_str() == Some("steady"),
        case_store,
        output_keys,
        store_fields,
        dt: num("solver.dt"),
        fixed_dt: num("solver.max_courant") == 0.0 && STEP_KEYS.iter().all(|k| !set_value(&get(k))),
        chain: get("chain.sequence").as_array().is_some_and(|a| !a.is_empty()),
        two_d: get("mesh.source").as_str() == Some("generator") && GEN_2D.contains(&generator.as_str()),
        mesh_bytes: None,
        cells_state: if counted {
            CellsState::Table
        } else {
            CellsState::Unknown
        },
    }
}

/// True for a transient run (D2): `piso`, `pimple`, or `compressible` unless `compressible.mode = "steady"`.
pub fn is_transient(algorithm: &str, compressible_steady: bool) -> bool {
    matches!(algorithm, "piso" | "pimple") || (algorithm == "compressible" && !compressible_steady)
}

// ---------------------------------------------------------------- effective store (D3)

/// Why the effective store is off.
#[derive(Debug, Clone, PartialEq)]
pub enum OffBy {
    /// The dialog's switch.
    Switch,
    /// The case text sets this key to 0.
    Case(String),
    /// This override (`path=value`) sets it to 0.
    Override(String),
    /// The case uses a moving mesh.
    MovingMesh,
}

/// Where the effective interval comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The case text.
    Case,
    /// The Studio's proposal.
    Plan,
    /// The user's overrides.
    Override,
}

/// The store that will be written: case text, then the proposal when on, then the user's overrides (last wins).
#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    /// `output.store_interval` (0: off).
    pub every_steps: u64,
    /// `output.store_time_interval` (0: off).
    pub every_time: f64,
    /// Where the last interval key that is above 0 came from (`None` when off).
    pub source: Option<Source>,
    /// Stored fields.
    pub fields: Vec<String>,
    /// Single precision.
    pub f32: bool,
    /// Why it is off (`None` when on).
    pub off_by: Option<OffBy>,
    /// Steady run.
    pub steady: bool,
    /// Compressible (the CFL limit sets its step).
    pub compressible: bool,
    /// `solver.max_iterations`.
    pub max_iterations: u64,
    /// `solver.end_time`.
    pub end_time: f64,
    /// `solver.dt`.
    pub dt: f64,
    /// Fixed time step.
    pub fixed_dt: bool,
}

impl Effective {
    /// The store is written.
    pub fn on(&self) -> bool {
        self.every_steps > 0 || self.every_time > 0.0
    }
}

/// `path=value` as `rcfd run --set` reads it (`rcfd-api` `chain::apply_set`): a JSON value, or a bare word.
pub fn parse_entry(entry: &str) -> Option<(String, Value)> {
    let (p, v) = entry.split_once('=')?;
    let (p, v) = (p.trim(), v.trim());
    if p.is_empty() {
        return None;
    }
    let value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
    Some((p.to_string(), value))
}

/// The effective store of a plan (D3).
pub fn effective(plan: &StartPlan) -> Effective {
    let f = &plan.facts;
    let x = &f.extra;
    let mut algorithm = f.algorithm.clone();
    let mut mode_steady = x.compressible_steady;
    let mut e = Effective {
        every_steps: 0,
        every_time: 0.0,
        source: None,
        fields: x.store_fields.clone(),
        f32: false,
        off_by: None,
        steady: false,
        compressible: false,
        max_iterations: f.max_iterations,
        end_time: f.end_time,
        dt: x.dt,
        fixed_dt: x.fixed_dt,
    };
    let mut zero: Option<OffBy> = None;
    let mut apply = |e: &mut Effective, path: &str, v: &Value, src: Source, entry: &str| {
        let off = |k: &str| match src {
            Source::Case => OffBy::Case(k.to_string()),
            _ => OffBy::Override(entry.to_string()),
        };
        match path {
            "output.store_interval" => {
                e.every_steps = v.as_u64().unwrap_or(0);
                if e.every_steps > 0 {
                    e.source = Some(src);
                } else {
                    zero = Some(off(path));
                }
            }
            "output.store_time_interval" => {
                e.every_time = v.as_f64().unwrap_or(0.0).max(0.0);
                if e.every_time > 0.0 {
                    e.source = Some(src);
                } else {
                    zero = Some(off(path));
                }
            }
            "output.store_fields" => {
                e.fields = v
                    .as_array()
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
            }
            "output.store_precision" => e.f32 = v.as_str() == Some("f32"),
            "solver.max_iterations" => e.max_iterations = v.as_u64().unwrap_or(e.max_iterations),
            "solver.end_time" => e.end_time = v.as_f64().unwrap_or(e.end_time),
            "solver.dt" => e.dt = v.as_f64().unwrap_or(e.dt),
            "solver.algorithm" => algorithm = v.as_str().unwrap_or(&algorithm).to_string(),
            "compressible.mode" => mode_steady = v.as_str() == Some("steady"),
            p if (p == "solver.max_courant" || STEP_KEYS.contains(&p)) && set_value(v) => e.fixed_dt = false,
            _ => {}
        }
    };
    for (k, v) in &x.case_store {
        apply(&mut e, k, v, Source::Case, "");
    }
    for entry in plan.set() {
        // the proposal's keys come first in `set()`, then the user's overrides
        let src = if plan.extra_set.contains(&entry) {
            Source::Override
        } else {
            Source::Plan
        };
        if let Some((p, v)) = parse_entry(&entry) {
            apply(&mut e, &p, &v, src, &entry);
        }
    }
    if e.fields.is_empty() {
        e.fields = rcfd_api::fieldstore::DEFAULT_FIELDS
            .iter()
            .map(|s| s.to_string())
            .collect();
    }
    e.compressible = algorithm == "compressible";
    e.steady = !is_transient(&algorithm, mode_steady);
    if !e.on() {
        e.source = None;
        e.off_by = Some(if f.dynamic_mesh {
            OffBy::MovingMesh
        } else if plan.store.as_ref().is_some_and(|s| !s.on) && !matches!(zero, Some(OffBy::Override(_))) {
            OffBy::Switch
        } else {
            zero.unwrap_or(OffBy::Switch)
        });
    }
    e
}

/// How sure a snapshot count is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountKind {
    /// Exactly this many.
    Exact,
    /// At most: a steady run may converge earlier.
    Steady,
    /// At most: the compressible step follows the CFL limit.
    Cfl,
    /// At most: an adaptive piso/pimple step.
    Adaptive,
    /// At most: a fixed step whose snapshots do not line up with the interval.
    AtMost,
}

/// The expected number of snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    /// Snapshots (at least 1: the final state is always stored).
    pub n: u64,
    /// Exact or an upper bound.
    pub kind: CountKind,
}

/// The fraction of a step below which a fixed-step transient run takes no separate step for the remainder to its end
/// time but stretches its last whole step to it (ADR 0145 D1, D2: `REMAINDER_ABSORB` in `rcfd-api`'s `sim.rs`). The step
/// count of [`count`] follows it (ADR 0147 amendment 3, the maintainer's answer to ADR 0145 question 3).
pub const STEP_REMAINDER: f64 = 1e-6;

/// The slack of the step count before ADR 0147 amendment 3 (the route [`count_with`] keeps for its control).
pub const OLD_STEP_SLACK: f64 = 1e-9;

fn ceil_slack(x: f64, slack: f64) -> u64 {
    (x - slack).ceil().max(0.0) as u64
}

/// Whole intervals in `x` with the store's slack (its time slots are `floor(t / interval + 1e-9)`, `fieldstore.rs`).
fn ceil_eps(x: f64) -> u64 {
    ceil_slack(x, 1e-9)
}

fn integral(x: f64) -> bool {
    (x - x.round()).abs() <= 1e-9 * x.abs().max(1.0)
}

/// The snapshot count of D3 (`None` when the store is off or the count cannot be known). A fixed step takes
/// `ceil(end_time / dt - STEP_REMAINDER)` steps: a remainder below [`STEP_REMAINDER`] of a step is no step (ADR 0145).
pub fn count(e: &Effective) -> Option<Count> {
    count_with(e, STEP_REMAINDER)
}

/// [`count`] with `step_slack` in place of [`STEP_REMAINDER`] in the step count ([`OLD_STEP_SLACK`]: the route before
/// ADR 0147 amendment 3).
pub fn count_with(e: &Effective, step_slack: f64) -> Option<Count> {
    if !e.on() {
        return None;
    }
    if e.steady {
        if e.every_steps == 0 {
            return None; // a time interval on a steady run: the hook refuses it
        }
        return Some(Count {
            n: e.max_iterations.div_ceil(e.every_steps).max(1),
            kind: CountKind::Steady,
        });
    }
    let n_steps = (e.fixed_dt && !e.compressible && e.dt > 0.0 && e.end_time > 0.0)
        .then(|| ceil_slack(e.end_time / e.dt, step_slack));
    let loose = if e.compressible {
        CountKind::Cfl
    } else {
        CountKind::Adaptive
    };
    let mut n = 0u64;
    let mut kind = CountKind::Exact;
    if e.every_time > 0.0 {
        let b = ceil_eps(e.end_time / e.every_time).max(1);
        match n_steps {
            Some(steps) => {
                n += b.min(steps);
                let aligned = e.dt >= e.every_time || (integral(e.end_time / e.every_time) && e.dt <= e.every_time);
                if !aligned {
                    kind = CountKind::AtMost;
                }
            }
            None => {
                n += b;
                kind = loose;
            }
        }
    }
    if e.every_steps > 0 {
        let steps = n_steps?;
        n += steps.div_ceil(e.every_steps);
        if e.every_time > 0.0 {
            kind = CountKind::AtMost;
        }
    }
    Some(Count { n: n.max(1), kind })
}

/// Mesh bytes of a built mesh (D4): points, face offsets, face points, owners and neighbours as the store writes them.
pub fn mesh_bytes(m: &rcfd_mesh::Mesh) -> u64 {
    let t = m.topology();
    let (p, f, fp, n) = (
        t.points.len() as u64,
        t.owner.len() as u64,
        t.face_points.len() as u64,
        t.neighbour.len() as u64,
    );
    24 * p + 8 * (f + 1) + 8 * fp + 8 * f + 8 * n
}

/// D4 size of a store of `cells` cells, `fields` fields, `n` snapshots, `width` bytes per value and `mesh` bytes.
pub fn store_bytes(cells: u64, fields: u64, n: u64, width: u64, mesh: u64) -> u64 {
    let blocks = cells.div_ceil(BLOCK_VALUES).max(1);
    FIXED_BYTES + mesh + n * (SNAP_BYTES + fields * (FIELD_BYTES + BLOCK_BYTES * blocks + cells * width))
}

/// The plan's mesh bytes: built, else per cell (D4).
fn plan_mesh_bytes(plan: &StartPlan, cells: u64) -> u64 {
    let x = &plan.facts.extra;
    x.mesh_bytes
        .unwrap_or_else(|| cells * if x.two_d { MESH_2D_PER_CELL } else { MESH_3D_PER_CELL })
}

/// The D4 estimate of the plan's store (`None` when off, the count is unknown or the cells are not known).
pub fn estimate(plan: &StartPlan) -> Option<u64> {
    let e = effective(plan);
    let c = count(&e)?;
    let cells = plan.facts.cells?;
    Some(store_bytes(
        cells,
        e.fields.len() as u64,
        c.n,
        if e.f32 { 4 } else { 8 },
        plan_mesh_bytes(plan, cells),
    ))
}

/// A size as the dialog prints it: `342 kB`, `984.1 MB`, `1.86 GB`.
pub fn size_text(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1e6 {
        format!("{:.0} kB", b / 1e3)
    } else if b < 1e9 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.2} GB", b / 1e9)
    }
}

// ---------------------------------------------------------------- dialog text (D6)

fn every_text(e: &Effective) -> String {
    match (e.every_steps, e.every_time > 0.0) {
        (0, true) => format!("every {} s of simulated time", e.every_time),
        (1, false) => "every iteration".into(),
        (k, false) => format!("every {k} iterations"),
        (k, true) => format!("every {k} iterations and every {} s of simulated time", e.every_time),
    }
}

fn count_text(c: Option<Count>) -> String {
    match c {
        None => "an unknown number of snapshots".into(),
        Some(Count { n, kind }) => match kind {
            CountKind::Exact => format!("{n} snapshots"),
            CountKind::Steady => format!("at most {n} snapshots (fewer if the run converges earlier)"),
            CountKind::Cfl => format!("at most {n} snapshots (the time step follows the CFL limit)"),
            CountKind::Adaptive => format!("at most {n} snapshots (the time step is adaptive)"),
            CountKind::AtMost => format!("at most {n} snapshots"),
        },
    }
}

/// The store line: `Stores ux, uy, uz, p every 4 iterations: at most 100 snapshots (...), f32.` (`None` when off).
pub fn store_line(plan: &StartPlan) -> Option<String> {
    let e = effective(plan);
    if !e.on() {
        return None;
    }
    Some(format!(
        "Stores {} {}: {}, {}.",
        e.fields.join(", "),
        every_text(&e),
        count_text(count(&e)),
        if e.f32 { "f32" } else { "f64" }
    ))
}

/// The size line (`None` when the store is off).
pub fn size_line(plan: &StartPlan) -> Option<String> {
    let e = effective(plan);
    if !e.on() {
        return None;
    }
    let x = &plan.facts.extra;
    Some(match (estimate(plan), plan.facts.cells, &x.cells_state) {
        (Some(b), Some(cells), _) => format!(
            "Estimated size {} ({cells} cells), in runs/<run id>/fields/ where the Viewport reads it.",
            size_text(b)
        ),
        (_, None, CellsState::Counting) => "Estimated size: counting the cells of the mesh...".into(),
        (_, None, CellsState::Failed(err)) => format!("Estimated size unknown: the mesh could not be built ({err})."),
        (_, None, _) => "Estimated size unknown: the cell count is not known before the run.".into(),
        (None, Some(_), _) => "Estimated size unknown: the number of snapshots is not known before the run.".into(),
    })
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    }
}

/// The interval key the case text sets to 0 (the store turned off by the case), if any and no interval is above 0.
pub fn case_off_key(plan: &StartPlan) -> Option<String> {
    let cs = &plan.facts.extra.case_store;
    let interval = |k: &str| cs.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_f64().unwrap_or(0.0));
    let (a, b) = (
        interval("output.store_interval"),
        interval("output.store_time_interval"),
    );
    if a.unwrap_or(0.0) > 0.0 || b.unwrap_or(0.0) > 0.0 {
        return None;
    }
    if a.is_some() {
        Some("output.store_interval".into())
    } else if b.is_some() {
        Some("output.store_time_interval".into())
    } else {
        None
    }
}

/// Lines about the case's own output settings, in order: its own store (kept, or overridden by the Studio's fix), its
/// `output.directory`, its other `output.*` keys.
pub fn case_lines(plan: &StartPlan) -> Vec<String> {
    let x = &plan.facts.extra;
    let mut v = Vec::new();
    if let Some(k) = case_off_key(plan) {
        if plan.store.as_ref().is_some_and(|s| s.on) {
            v.push(format!("Overrides the case's {k} = 0 for this run."));
        }
    } else if let Some((k, val)) = x
        .case_store
        .iter()
        .find(|(k, v)| k.ends_with("interval") && v.as_f64().unwrap_or(0.0) > 0.0)
    {
        v.push(format!(
            "The case sets its own field store ({k} = {}), which is kept.",
            value_text(val)
        ));
    }
    if x.output_keys.iter().any(|k| k == "output.directory") {
        v.push(
            "The case's output.directory is not used: the runner writes the results to runs/<run id>/output/.".into(),
        );
    }
    let rest: Vec<&str> = x
        .output_keys
        .iter()
        .filter(|k| *k != "output.directory")
        .map(String::as_str)
        .collect();
    if !rest.is_empty() {
        v.push(format!("The case's own output settings are kept: {}.", rest.join(", ")));
    }
    v
}

/// A one-click fix of a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    /// Tick "Store fields" again.
    StoreOn,
    /// Store fields although the case turns the store off (the proposal as `--set` entries).
    StoreForCase,
    /// Bring the count back to the Studio's 100.
    AtMost100,
    /// f32 and the largest snapshot count whose estimate fits [`HUGE_BYTES`].
    FitHuge,
}

/// A warning of the dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    /// Text.
    pub text: String,
    /// Button label and fix.
    pub fix: Option<(String, Fix)>,
}

/// The snapshot count that fits [`HUGE_BYTES`] in f32 (at least 1).
fn fit_count(plan: &StartPlan, e: &Effective) -> Option<u64> {
    let cells = plan.facts.cells?;
    let mesh = plan_mesh_bytes(plan, cells);
    let per = store_bytes(cells, e.fields.len() as u64, 1, 4, mesh) - FIXED_BYTES - mesh;
    Some((HUGE_BYTES.saturating_sub(FIXED_BYTES + mesh) / per).max(1))
}

/// The dialog's warnings (D6), in order.
pub fn warnings(plan: &StartPlan) -> Vec<Warning> {
    let e = effective(plan);
    let nothing = "so the Viewport will have nothing to show for this run.";
    let mut v = Vec::new();
    if plan.facts.extra.chain {
        v.push(Warning {
            text:
                "The case is a chain: each stage stores its fields in runs/<run id>/output/<stage>/fields/, which the \
                   Viewport does not read yet."
                    .into(),
            fix: None,
        });
    }
    match &e.off_by {
        None => {}
        Some(OffBy::MovingMesh) => v.push(Warning {
            text: format!(
                "The case uses a moving mesh (dynamic_mesh.enabled), which the field store does not support, {nothing}"
            ),
            fix: None,
        }),
        Some(OffBy::Override(entry)) => v.push(Warning {
            text: format!("The override {entry} turns the field store off, {nothing}"),
            fix: None,
        }),
        Some(OffBy::Case(k)) if plan.store.is_none() => v.push(Warning {
            text: format!("The case turns the field store off ({k} = 0), {nothing}"),
            fix: Some(("Store fields for this run".into(), Fix::StoreForCase)),
        }),
        Some(_) => v.push(Warning {
            text: format!("Fields will not be stored, {nothing}"),
            fix: plan.store.is_some().then(|| ("Store fields".into(), Fix::StoreOn)),
        }),
    }
    if !e.on() {
        return v;
    }
    let fixable = e.source != Some(Source::Override);
    if let Some(c) = count(&e).filter(|c| c.n > STORE_SNAPSHOTS) {
        v.push(Warning {
            text: format!(
                "{} snapshots will be stored, more than the {STORE_SNAPSHOTS} the Studio proposes.",
                c.n
            ),
            fix: fixable.then(|| (format!("Store at most {STORE_SNAPSHOTS} snapshots"), Fix::AtMost100)),
        });
    }
    if let Some(b) = estimate(plan).filter(|b| *b > HUGE_BYTES) {
        let n = fit_count(plan, &e).unwrap_or(1);
        v.push(Warning {
            text: format!(
                "The store is estimated at {}, more than {}: check that the disk has room, or store fewer snapshots.",
                size_text(b),
                size_text(HUGE_BYTES)
            ),
            fix: fixable.then(|| (format!("Store fewer snapshots (about {n})"), Fix::FitHuge)),
        });
    }
    v
}

/// The proposal for the plan's case as if it set no store key (used by the fixes and the re-run).
fn proposal_ignoring_case(plan: &StartPlan) -> Option<StorePlan> {
    let mut f = plan.facts.clone();
    f.sets_store = false;
    propose_store(&f).0
}

/// Set the plan's interval so that the effective budget gives `n` snapshots (steps: `ceil(M / n)`; time: `T / n`).
fn retarget(plan: &mut StartPlan, n: u64) {
    if plan.store.is_none() {
        plan.store = proposal_ignoring_case(plan);
    }
    let e = effective(&StartPlan {
        store: None,
        ..plan.clone()
    });
    let n = n.max(1);
    if let Some(s) = plan.store.as_mut() {
        s.on = true;
        s.every = if !e.steady && e.end_time > 0.0 {
            StoreEvery::Time(e.end_time / n as f64)
        } else {
            StoreEvery::Steps(e.max_iterations.div_ceil(n).max(1))
        };
    }
}

/// Apply a fix to the plan (only the plan changes; the case file is not touched).
pub fn apply_fix(plan: &mut StartPlan, fix: Fix) {
    match fix {
        Fix::StoreOn => {
            if let Some(s) = plan.store.as_mut() {
                s.on = true;
            }
        }
        Fix::StoreForCase => plan.store = proposal_ignoring_case(plan),
        Fix::AtMost100 => retarget(plan, STORE_SNAPSHOTS),
        Fix::FitHuge => {
            if let Some(s) = plan.store.as_mut() {
                s.f32 = true;
            }
            let e = effective(plan);
            if let Some(n) = fit_count(plan, &e) {
                retarget(plan, n);
            }
        }
    }
}

// ---------------------------------------------------------------- cell count (D5)

/// Build the mesh of a case text (`base_dir`: where its relative files are) and return its cells and D4 mesh bytes.
pub fn count_cells(text: &str, base_dir: &Path) -> Result<(u64, u64), String> {
    let case = Case::parse_toml_in(text, base_dir).map_err(|errs| {
        errs.iter()
            .map(|d| format!("{}: {}", d.path, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let mesh = rcfd_api::sim::build_mesh(&case).map_err(|errs| {
        errs.iter()
            .map(|d| format!("{}: {}", d.path, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    Ok((mesh.n_cells() as u64, mesh_bytes(&mesh)))
}

/// Put a finished count into the plan.
pub fn set_cells(plan: &mut StartPlan, r: Result<(u64, u64), String>) {
    match r {
        Ok((cells, mesh)) => {
            plan.facts.cells = Some(cells);
            plan.facts.extra.mesh_bytes = Some(mesh);
            plan.facts.extra.cells_state = CellsState::Built;
        }
        Err(e) => plan.facts.extra.cells_state = CellsState::Failed(e),
    }
}

// ---------------------------------------------------------------- re-run (D9)

/// True for a `set` entry naming an interval or the precision of the store.
pub fn is_store_entry(entry: &str) -> bool {
    parse_entry(entry).is_some_and(|(p, _)| {
        matches!(
            p.as_str(),
            "output.store_interval" | "output.store_time_interval" | "output.store_precision"
        )
    })
}

/// The 16 hex digits of a snapshot hash (`fnv64:` or the legacy `fnv1a64:` spelling).
fn hash_digits(h: &str) -> &str {
    h.rsplit(':').next().unwrap_or(h)
}

/// The re-run's refusal texts (registered, ADR 0140 SF7).
pub fn changed_text(case: &str) -> String {
    format!(
        "{case} changed since this run, so it is not re-run; start a new run with Runs > Start run... (\"Store fields\" \
         is on by default)."
    )
}

/// See [`changed_text`].
pub fn not_found_text(case: &str) -> String {
    format!(
        "{case} is not found, so it is not re-run; start a new run with Runs > Start run... (\"Store fields\" is on by \
         default)."
    )
}

/// The case file of a run when it is unchanged since the run (its bytes hash to `case_snapshot_hash`), else the
/// refusal text ([`changed_text`], [`not_found_text`]).
pub fn case_unchanged(root: &Path, rec: &RunRecord) -> Result<Vec<u8>, String> {
    let path = root.join(&rec.case);
    let bytes = std::fs::read(&path).map_err(|_| not_found_text(&rec.case))?;
    let now = rcfd_project::records::hash_bytes(&bytes);
    if hash_digits(&now) != hash_digits(&rec.case_snapshot_hash) {
        return Err(changed_text(&rec.case));
    }
    Ok(bytes)
}

/// The plan of a re-run of run `id` with the field store on (D9): the same case (unchanged since the run), the old
/// priority, threads and memory, the store proposal for the old overrides' budget, then the old overrides without store
/// interval or precision entries.
pub fn rerun_plan(root: &Path, id: &str) -> Result<StartPlan, String> {
    let dir = root.join("runs").join(id);
    let rec: RunRecord = std::fs::read(dir.join("run.json"))
        .map_err(|e| format!("run {id}: run.json: {e}"))
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| format!("run {id}: run.json: {e}")))?;
    let bytes = case_unchanged(root, &rec)?;
    let text = String::from_utf8(bytes).map_err(|e| format!("{}: {e}", rec.case))?;
    let path = root.join(&rec.case);
    let base = path.parent().unwrap_or(root);
    let mut plan = StartPlan::new(&rec.case, &text, base)?;
    let s = RunSettings::of(&rec).unwrap_or_default();
    plan.priority = s.priority;
    plan.threads = s.threads.max(1);
    plan.mem_mb = s.mem_mb;
    plan.extra_set = s.set.into_iter().filter(|e| !is_store_entry(e)).collect();
    if plan.facts.dynamic_mesh {
        return Err(format!(
            "{}: the field store does not support dynamic_mesh.enabled",
            rec.case
        ));
    }
    retarget(&mut plan, STORE_SNAPSHOTS);
    Ok(plan)
}

/// Queue the re-run of run `id` with the field store on and make sure a runner is alive; the new run's id.
pub fn rerun(client: &Client, root: &Path, id: &str) -> Result<String, String> {
    let plan = rerun_plan(root, id)?;
    client.start(&root.join(&plan.case), &plan.settings())
}
