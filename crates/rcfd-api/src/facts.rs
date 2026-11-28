//! Case facts and the run-time plan of a run (ADR 0737 P1, P2; F-837 E-837a).
//!
//! * [`case_facts`] reads from a case what its time step depends on: the kind of run ([`Kind`]), `solver.dt` and
//!   `solver.end_time` (and whether the case text sets them), the case's own step control ([`OWN_CONTROL`]), the
//!   thinnest and the thickest cell of the mesh the run builds, the fastest speed at the start and the pressure
//!   correctors. A transient case's simulation is built as `rcfd run` builds it ([`Simulation::new`]); no step is taken.
//! * [`plan`] turns the facts and the run-time settings ([`RunTime`]: the time to simulate, a Courant target, a largest
//!   step, a mode) into the `rcfd run --set` entries of the run ([`TimePlan`]); [`corrector_plan`] adds
//!   [`ADAPTIVE_ENTRY`] where the solver accepts adaptive correctors; [`warning`] warns above an estimated Courant number
//!   of [`COURANT_WARN`].
//!
//! The rule and its constants are the egui Studio's (`crates/rcfd-studio/src/time_step.rs` at 44e63df8; ADR 0153, ADR
//! 0156, ADR 0158), moved into the engine so that every client gets the same plan (G2). Nothing here changes a solver
//! default: a run takes the plan only as `--set` entries.

use crate::case::{Case, Diagnostic};
use crate::sim::Simulation;
use rcfd_mesh::mesh::{Mesh, PatchKind};
use serde_json::{json, Value};

/// The default Courant target of the automatic step (ADR 0153 D3).
pub const COURANT_TARGET: f64 = 0.5;
/// Explicit settings whose estimated Courant number is above this are warned about before the run (ADR 0153 D6).
pub const COURANT_WARN: f64 = 2.0;
/// The largest automatic step is the simulated time over this (so at least about this many steps).
pub const MIN_STEPS: f64 = 100.0;
/// Wall time per cell and step of a single-threaded laminar PISO run on the host of ADR 0153 (4.7 microseconds): the
/// rough wall-time estimate.
pub const CELL_STEP_SECONDS: f64 = 4.7e-6;
/// The automatic step unless the caller picks another one (ADR 0153 D2).
pub const DEFAULT_MODE: StepMode = StepMode::Adaptive;
/// Adaptive pressure correctors unless the caller switches them off (ADR 0158; ADR 0156 question 1).
pub const DEFAULT_ADAPTIVE_CORRECTORS: bool = true;
/// The run-time entry of adaptive pressure correctors (ADR 0156 D1, D8).
pub const ADAPTIVE_ENTRY: &str = "solver.corrector_control=adaptive";
/// Where the speed scale of a flow at rest under gravity comes from (ADR 0153 D5).
pub const GRAVITY_SCALE: &str = "gravity (free fall across the domain, sqrt(|g| L))";
/// What a case without a speed scale lacks (ADR 0153 D5).
pub const NO_SPEED: &str = "no inflow, moving wall, initial velocity or gravity";

/// Keys that give a case its own step control: the automatic step leaves such a case's control alone.
pub const OWN_CONTROL: [&str; 13] = [
    "solver.max_courant",
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

/// What kind of run a case is, for the time step.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Incompressible transient (`piso`, `pimple`, constant density, no VOF, the flow solved).
    Transient,
    /// Steady (`simple`, `simplec`, `coupled`).
    Steady {
        /// `solver.max_iterations`.
        max_iterations: u64,
    },
    /// A transient run that keeps its own time settings, and why.
    Other(String),
}

/// What the pressure correctors of a case's runs depend on (ADR 0158).
#[derive(Debug, Clone, PartialEq)]
pub struct Correctors {
    /// `solver.n_correctors`.
    pub n: u64,
    /// `solver.corrector_control`.
    pub control: String,
    /// Why the solver would refuse adaptive correctors for this case (ADR 0156 D5); `None` when it accepts them.
    pub not_adaptive: Option<String>,
}

/// What the time step of a case depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseFacts {
    /// `{:016x}` of the settings tree's content hash (as `journal.get` gives `case_hash`).
    pub case_hash: String,
    /// The kind of run.
    pub kind: Kind,
    /// `solver.algorithm`.
    pub algorithm: String,
    /// `solver.dt` as resolved, s.
    pub dt: f64,
    /// The case sets `solver.dt` ([`InText`]).
    pub dt_in_text: bool,
    /// `solver.end_time` as resolved, s.
    pub end_time: f64,
    /// The case sets `solver.end_time`.
    pub end_in_text: bool,
    /// The [`OWN_CONTROL`] keys the case sets to something other than their unset value.
    pub own_control: Vec<String>,
    /// The latest `output.write_times` entry.
    pub write_times_last: Option<f64>,
    /// Cells of the mesh the run builds.
    pub cells: u64,
    /// The mesh has `empty` patches (a 2D mesh).
    pub two_d: bool,
    /// Lower and upper corner of the mesh's bounding box, m.
    pub bounds: [[f64; 3]; 2],
    /// The thinnest cell, m: the smallest cell volume over its largest face area, the faces of `empty` patches left out.
    pub h_min: f64,
    /// The thickest cell by the same measure, m.
    pub h_max: f64,
    /// The fastest speed at the start, m/s (transient runs; 0 otherwise or when there is none).
    pub u0: f64,
    /// Where [`CaseFacts::u0`] is.
    pub u0_from: Option<String>,
    /// The pressure correctors.
    pub correctors: Correctors,
}

/// How [`case_facts`] tells whether the case sets `solver.dt` and `solver.end_time`.
#[derive(Debug, Clone, Copy)]
pub enum InText<'a> {
    /// The parsed document of the case text (TOML or JSON): set when the key is present.
    Text(&'a Value),
    /// No text (a service session): set when the resolved value differs from the schema default (ADR 0152's rule).
    NotDefault,
}

/// Why [`case_facts`] gave no facts.
#[derive(Debug, Clone, PartialEq)]
pub enum FactsError {
    /// The case does not build: located diagnostics.
    Invalid(Vec<Diagnostic>),
    /// The mesh has more cells than the caller's limit.
    TooLarge {
        /// Cells of the mesh.
        cells: u64,
        /// The limit.
        limit: u64,
    },
}

impl CaseFacts {
    /// The estimated Courant number of a step `dt` (`u0 dt / h_min`; ADR 0153 D4).
    pub fn courant(&self, dt: f64) -> Option<f64> {
        (self.u0 > 0.0 && self.h_min > 0.0 && self.h_min.is_finite()).then(|| self.u0 * dt / self.h_min)
    }

    /// The step whose estimated Courant number is `c`, unrounded.
    pub fn step_for(&self, c: f64) -> Option<f64> {
        (self.u0 > 0.0 && self.h_min > 0.0 && self.h_min.is_finite()).then(|| c * self.h_min / self.u0)
    }

    /// The facts as the JSON of `case.facts`, `case_facts` and `rcfd check --facts`.
    pub fn to_json(&self) -> Value {
        let (kind, extra) = match &self.kind {
            Kind::Transient => ("transient", json!(null)),
            Kind::Steady { max_iterations } => ("steady", json!(max_iterations)),
            Kind::Other(why) => ("other", json!(why)),
        };
        let mut v = json!({
            "case_hash": self.case_hash,
            "kind": kind,
            "algorithm": self.algorithm,
            "dt": self.dt, "dt_in_text": self.dt_in_text,
            "end_time": self.end_time, "end_in_text": self.end_in_text,
            "own_control": self.own_control,
            "write_times_last": self.write_times_last,
            "cells": self.cells,
            "two_d": self.two_d,
            "bounds": self.bounds,
            "h_min": finite(self.h_min), "h_max": finite(self.h_max), "h_unit": "m",
            "u0": self.u0, "u0_from": self.u0_from, "u0_unit": "m/s",
            "correctors": {"n": self.correctors.n, "control": self.correctors.control,
                "not_adaptive": self.correctors.not_adaptive},
        });
        match &self.kind {
            Kind::Steady { .. } => v["max_iterations"] = extra,
            Kind::Other(_) => v["why"] = extra,
            Kind::Transient => {}
        }
        v
    }
}

fn finite(x: f64) -> Value {
    if x.is_finite() {
        json!(x)
    } else {
        Value::Null
    }
}

/// The parsed document of a case text as `rcfd run` reads it: JSON when it starts with `{`, else TOML.
pub fn text_tree(text: &str) -> Option<Value> {
    if text.trim_start().starts_with('{') {
        serde_json::from_str(text).ok()
    } else {
        toml::from_str::<Value>(text).ok()
    }
}

fn text_sets(raw: &Value, path: &[&str]) -> bool {
    let mut v = raw;
    for p in path {
        match v.get(p) {
            Some(x) => v = x,
            None => return false,
        }
    }
    true
}

fn set_value(v: &Value) -> bool {
    v.as_f64().is_some_and(|x| x != 0.0) || v.as_bool() == Some(true) || v.as_array().is_some_and(|a| !a.is_empty())
}

fn kind_of(case: &Case) -> Kind {
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let algorithm = get("solver.algorithm").as_str().unwrap_or_default().to_string();
    if matches!(algorithm.as_str(), "simple" | "simplec" | "coupled") {
        return Kind::Steady {
            max_iterations: get("solver.max_iterations").as_u64().unwrap_or(0),
        };
    }
    if crate::comp::enabled(case) {
        return Kind::Other("the compressible solver is explicit and sets its own step from compressible.cfl".into());
    }
    if !matches!(algorithm.as_str(), "piso" | "pimple") {
        return Kind::Other(format!("solver.algorithm \"{algorithm}\" has no automatic time step"));
    }
    if crate::vd::enabled(case) {
        return Kind::Other(
            "a variable-density run (fluid.density) takes fixed steps only, without the Courant controller".into(),
        );
    }
    if crate::vof::enabled(case) {
        return Kind::Other("a VOF run (multiphase) takes fixed steps only, without the Courant controller".into());
    }
    if get("solver.frozen_flow").as_bool() == Some(true) {
        return Kind::Other("the flow is prescribed (solver.frozen_flow), so no Courant number follows it".into());
    }
    if get("chain.sequence").as_array().is_some_and(|a| !a.is_empty()) {
        return Kind::Other("a chain runs each stage with its own settings".into());
    }
    Kind::Transient
}

fn not_adaptive(case: &Case, kind: &Kind) -> Option<String> {
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    match kind {
        Kind::Steady { .. } => return Some("a steady run has no time steps".into()),
        Kind::Other(why) => return Some(why.clone()),
        Kind::Transient => {}
    }
    if get("solver.time_advance").as_str() == Some("fractional_step") {
        return Some("solver.time_advance = \"fractional_step\" (a non-iterative step has no correctors)".into());
    }
    if get("dynamic_mesh.enabled").as_bool() == Some(true) {
        return Some("dynamic_mesh (the pressure-lag estimate is not defined on a moving mesh)".into());
    }
    if get("solver.error_tolerance").as_f64().is_some_and(|e| e > 0.0) {
        return Some("solver.error_tolerance (step doubling restores the state of trial steps)".into());
    }
    None
}

/// The thinnest and thickest cell of `mesh` (volume over the largest face area, faces of `empty` patches left out).
fn thickness(mesh: &Mesh) -> (f64, f64) {
    let mut empty = vec![false; mesh.face_area_vectors().len()];
    for p in mesh.patches() {
        if p.kind == PatchKind::Empty {
            for f in p.faces() {
                empty[f] = true;
            }
        }
    }
    let (areas, vols) = (mesh.face_area_vectors(), mesh.cell_volumes());
    let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
    for (c, v) in vols.iter().enumerate() {
        let largest = mesh
            .cell_faces(c)
            .iter()
            .filter(|&&f| !empty[f])
            .map(|&f| rcfd_core::vec3::norm(areas[f]))
            .fold(0.0, f64::max);
        if largest > 0.0 {
            lo = lo.min(v / largest);
            hi = hi.max(v / largest);
        }
    }
    (lo, hi)
}

fn bounds(mesh: &Mesh) -> [[f64; 3]; 2] {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    [lo, hi]
}

/// The fastest speed of `sim` at its start and where it is.
fn fastest(sim: &Simulation, bounds: &[[f64; 3]; 2]) -> Result<(f64, Option<String>), String> {
    let comp = |n: &str| sim.field(n).map_err(|e| format!("{n}: {e}"));
    let (ux, uy, uz) = (comp("ux")?, comp("uy")?, comp("uz")?);
    let cells = (0..ux.len())
        .map(|i| (ux[i] * ux[i] + uy[i] * uy[i] + uz[i] * uz[i]).sqrt())
        .fold(0.0, f64::max);
    let faces = sim.boundary_field("speed").map_err(|e| format!("speed: {e}"))?;
    let (mut best, mut at) = (0.0_f64, None);
    for (i, s) in faces.iter().enumerate() {
        if s.is_finite() && *s > best {
            (best, at) = (*s, Some(i));
        }
    }
    if !(cells.is_finite() && best.is_finite()) {
        return Err("the initial speed is not finite".into());
    }
    if best > cells {
        let f = at.map(|i| sim.geom.n_internal + i);
        let patch = f.and_then(|f| {
            sim.geom
                .mesh
                .patches()
                .iter()
                .find(|p| (p.start..p.start + p.size).contains(&f))
                .map(|p| p.name.clone())
        });
        return Ok((best, Some(format!("boundary {}", patch.unwrap_or_else(|| "?".into())))));
    }
    if cells > 0.0 {
        return Ok((cells, Some("the initial velocity field".into())));
    }
    // ADR 0153 D5: a flow at rest driven by gravity: free fall across the domain, sqrt(|g| L), an upper bound
    let g = sim
        .case
        .get("physics.gravity")
        .ok()
        .and_then(|v| {
            v.as_array()
                .map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<f64>>())
        })
        .map_or(0.0, |a| a.iter().map(|x| x * x).sum::<f64>().sqrt());
    if g > 0.0 && g.is_finite() {
        let l = (0..3).map(|k| bounds[1][k] - bounds[0][k]).fold(0.0, f64::max);
        if l > 0.0 && l.is_finite() {
            return Ok(((g * l).sqrt(), Some(GRAVITY_SCALE.into())));
        }
    }
    Ok((0.0, None))
}

fn diag(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic {
        path: String::new(),
        line: None,
        message,
    }]
}

/// What the time step of `case` depends on. The mesh is built as the run builds it and refused over `max_cells` cells
/// (when given) before anything else is built; a transient case's simulation is then built for its fastest speed at the
/// start. Only reads: no step is taken and nothing is written.
///
/// With a limit, a mesh whose cells [`crate::mesh_preview::cell_estimate`] gives is refused by that estimate before it is
/// built (ADR 0812, F-823; ADR 0844, F-989: mesh files and every generator): the estimate is the built count, so the
/// refusal is the one the build would give. A case without an estimate is built, then checked.
pub fn case_facts(case: &Case, in_text: InText<'_>, max_cells: Option<u64>) -> Result<CaseFacts, FactsError> {
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let num = |p: &str| get(p).as_f64().unwrap_or(0.0);
    let sets = |p: &str| match in_text {
        InText::Text(raw) => text_sets(raw, &p.split('.').collect::<Vec<_>>()),
        InText::NotDefault => {
            let default = case.tree.schema_at(p).map(|s| s.default_value()).unwrap_or(Value::Null);
            get(p) != default
        }
    };
    let diags = case.semantic_errors();
    if !diags.is_empty() {
        return Err(FactsError::Invalid(diags));
    }
    if let (Some(limit), Some(n)) = (max_cells, crate::mesh_preview::cell_estimate(case)) {
        if n > limit {
            return Err(FactsError::TooLarge { cells: n, limit });
        }
    }
    let mesh = crate::sim::build_mesh(case).map_err(FactsError::Invalid)?;
    let cells = mesh.n_cells() as u64;
    if let Some(limit) = max_cells.filter(|l| cells > *l) {
        return Err(FactsError::TooLarge { cells, limit });
    }
    let kind = kind_of(case);
    let (h_min, h_max) = thickness(&mesh);
    let bounds = bounds(&mesh);
    let two_d = mesh.patches().iter().any(|p| p.kind == PatchKind::Empty && p.size > 0);
    let mut f = CaseFacts {
        case_hash: format!("{:016x}", case.tree.content_hash()),
        algorithm: get("solver.algorithm").as_str().unwrap_or_default().to_string(),
        dt: num("solver.dt"),
        dt_in_text: sets("solver.dt"),
        end_time: num("solver.end_time"),
        end_in_text: sets("solver.end_time"),
        own_control: OWN_CONTROL
            .iter()
            .filter(|k| set_value(&get(k)))
            .map(|k| k.to_string())
            .collect(),
        write_times_last: get("output.write_times")
            .as_array()
            .and_then(|a| a.iter().filter_map(Value::as_f64).reduce(f64::max)),
        cells,
        two_d,
        bounds,
        h_min,
        h_max,
        u0: 0.0,
        u0_from: None,
        correctors: Correctors {
            n: get("solver.n_correctors").as_u64().unwrap_or(0),
            control: get("solver.corrector_control").as_str().unwrap_or_default().to_string(),
            not_adaptive: None,
        },
        kind,
    };
    f.correctors.not_adaptive = not_adaptive(case, &f.kind);
    drop(mesh);
    if f.kind == Kind::Transient {
        let sim = Simulation::new(case).map_err(FactsError::Invalid)?;
        (f.u0, f.u0_from) = fastest(&sim, &f.bounds).map_err(|m| FactsError::Invalid(diag(m)))?;
    }
    Ok(f)
}

// ---------------------------------------------------------------- the plan

/// How the time step of a run is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// The first step from the Courant target, then the solver's Courant controller (ADR 0050).
    Adaptive,
    /// The simulated time in equal steps of at most the Courant target's step.
    Fixed,
    /// The case's own `solver.dt` (or its own step control), unchanged.
    Case,
}

/// The run-time settings of a run (`start_run {run_time}`, `case.facts {run_time}`). Not saved in the case.
#[derive(Debug, Clone, PartialEq)]
pub struct RunTime {
    /// Simulated time, s (`None`: the case's `solver.end_time`).
    pub simulate: Option<f64>,
    /// Courant target of the automatic step.
    pub courant: f64,
    /// Largest automatic step, s (`None`: the simulated time over [`MIN_STEPS`]).
    pub max_dt: Option<f64>,
    /// How the step is chosen.
    pub mode: StepMode,
    /// Adaptive pressure correctors where the solver accepts them and the case does not choose them itself.
    pub adaptive_correctors: bool,
}

impl Default for RunTime {
    fn default() -> Self {
        RunTime {
            simulate: None,
            courant: COURANT_TARGET,
            max_dt: None,
            mode: DEFAULT_MODE,
            adaptive_correctors: DEFAULT_ADAPTIVE_CORRECTORS,
        }
    }
}

impl RunTime {
    /// Read `{simulate_s, courant, max_dt, mode, adaptive_correctors}` (every member optional; `null` is the default);
    /// any other member, or a member of the wrong type, is refused with the reason.
    pub fn from_json(v: &Value) -> Result<RunTime, String> {
        let mut rt = RunTime::default();
        let Some(o) = v.as_object() else {
            return Err("run_time: an object {simulate_s, courant, max_dt, mode, adaptive_correctors}".into());
        };
        let num = |k: &str, x: &Value| x.as_f64().ok_or_else(|| format!("run_time.{k}: a number"));
        for (k, x) in o {
            if x.is_null() {
                continue;
            }
            match k.as_str() {
                "simulate_s" => rt.simulate = Some(num(k, x)?),
                "courant" => rt.courant = num(k, x)?,
                "max_dt" => rt.max_dt = Some(num(k, x)?),
                "mode" => {
                    rt.mode = match x.as_str() {
                        Some("adaptive") => StepMode::Adaptive,
                        Some("fixed") => StepMode::Fixed,
                        Some("case") => StepMode::Case,
                        _ => return Err("run_time.mode: adaptive, fixed or case".into()),
                    }
                }
                "adaptive_correctors" => {
                    rt.adaptive_correctors = x
                        .as_bool()
                        .ok_or_else(|| "run_time.adaptive_correctors: true or false".to_string())?
                }
                other => return Err(format!("run_time: unknown member '{other}'")),
            }
        }
        Ok(rt)
    }
}

/// The step of a planned run.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// The solver's Courant controller from `dt0`.
    Adaptive {
        /// First step, s.
        dt0: f64,
        /// Courant target (`solver.max_courant`).
        courant: f64,
        /// Largest step (`solver.max_dt`).
        max_dt: f64,
    },
    /// `steps` equal steps of `dt`.
    Fixed {
        /// The step, s.
        dt: f64,
        /// Their number.
        steps: u64,
        /// The Courant target it was made for.
        courant: f64,
    },
    /// The case's own `solver.dt`.
    CaseDt {
        /// The step, s.
        dt: f64,
    },
    /// The case's own step control.
    CaseControl {
        /// The keys it sets.
        keys: Vec<String>,
    },
}

/// The time settings of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct TimePlan {
    /// Simulated time (`solver.end_time` of the run), s.
    pub end_time: f64,
    /// The step.
    pub step: Step,
    /// The `--set` entries, in order (`solver.end_time` first; the corrector entry last when there is one).
    pub entries: Vec<String>,
    /// Estimated Courant number of the first step.
    pub courant0: Option<f64>,
    /// Steps: exact for equal steps, an estimate (at most about) for the adaptive step.
    pub steps: Option<u64>,
    /// Rough wall time, s ([`CELL_STEP_SECONDS`]).
    pub wall_s: Option<f64>,
    /// The case's own `solver.dt` that this run replaces.
    pub over_dt: Option<f64>,
    /// The case's own `solver.end_time` that this run replaces (only when it differs).
    pub over_end: Option<f64>,
    /// No speed scale was found: the adaptive step starts at the case's `solver.dt` (ADR 0153 D5).
    pub no_speed: bool,
    /// The pressure correctors of the run.
    pub correctors: CorrectorPlan,
}

/// The pressure correctors of a run (ADR 0158).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectorPlan {
    /// The run adds [`ADAPTIVE_ENTRY`].
    Adaptive {
        /// `solver.n_correctors` (the least count).
        n: u64,
    },
    /// The case sets adaptive correctors itself: no entry.
    CaseAdaptive {
        /// `solver.n_correctors`.
        n: u64,
    },
    /// The case's own fixed count, and why (`off`: switched off by the caller; else the solver's reason).
    Fixed {
        /// `solver.n_correctors`.
        n: u64,
        /// `None`: switched off; `Some`: the solver has no adaptive correctors for this case.
        scope: Option<String>,
    },
}

impl CorrectorPlan {
    /// The `--set` entry of this plan.
    pub fn entry(&self) -> Option<String> {
        matches!(self, CorrectorPlan::Adaptive { .. }).then(|| ADAPTIVE_ENTRY.to_string())
    }

    fn to_json(&self) -> Value {
        match self {
            CorrectorPlan::Adaptive { n } => json!({"plan": "adaptive", "n": n, "entry": ADAPTIVE_ENTRY}),
            CorrectorPlan::CaseAdaptive { n } => json!({"plan": "case_adaptive", "n": n, "entry": null}),
            CorrectorPlan::Fixed { n, scope: None } => {
                json!({"plan": "fixed", "n": n, "entry": null, "why": "adaptive correctors switched off"})
            }
            CorrectorPlan::Fixed { n, scope: Some(w) } => json!({"plan": "fixed", "n": n, "entry": null, "why": w}),
        }
    }
}

/// The pressure correctors of a run of a case with facts `f` and settings `rt` (ADR 0152's rule: `"fixed"`, the schema
/// default, is not the case's own; `"adaptive"` is).
pub fn corrector_plan(f: &CaseFacts, rt: &RunTime) -> CorrectorPlan {
    let c = &f.correctors;
    if let Some(why) = &c.not_adaptive {
        return CorrectorPlan::Fixed {
            n: c.n,
            scope: Some(why.clone()),
        };
    }
    if c.control == "adaptive" {
        return CorrectorPlan::CaseAdaptive { n: c.n };
    }
    if !rt.adaptive_correctors {
        return CorrectorPlan::Fixed { n: c.n, scope: None };
    }
    CorrectorPlan::Adaptive { n: c.n }
}

/// `x` rounded down to 3 significant digits.
pub fn round_down(x: f64) -> f64 {
    if !(x.is_finite() && x > 0.0) {
        return x;
    }
    let k = 2 - x.log10().floor() as i32;
    let p = 10f64.powi(k.abs());
    if k >= 0 {
        (x * p).floor() / p
    } else {
        (x / p).floor() * p
    }
}

/// A number in a message: at most 4 significant digits, trailing zeros trimmed.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    let trim = |s: String| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if (1e-3..1e5).contains(&a) {
        let decimals = (3 - a.log10().floor() as i32).clamp(0, 6) as usize;
        trim(format!("{v:.decimals$}"))
    } else {
        let s = format!("{v:.3e}");
        match s.split_once('e') {
            Some((m, e)) => format!("{}e{e}", trim(m.to_string())),
            None => s,
        }
    }
}

fn entry(path: &str, v: f64) -> String {
    format!("{path}={v}")
}

/// The time settings of a run of a case with facts `f` and settings `rt`; refused, with the reason, for a case that is
/// not [`Kind::Transient`], a simulated time that is not positive, an end before one of the case's own
/// `output.write_times`, a Courant target or largest step that is not positive, and a fixed automatic step without a
/// speed scale.
pub fn plan(f: &CaseFacts, rt: &RunTime) -> Result<TimePlan, String> {
    match &f.kind {
        Kind::Transient => {}
        Kind::Steady { .. } => return Err("a steady run has no time step".into()),
        Kind::Other(why) => return Err(format!("no automatic time step: {why}")),
    }
    let n = rt.simulate.unwrap_or(f.end_time);
    if !(n.is_finite() && n > 0.0) {
        return Err(format!("Simulate for: {n} s is not a positive time"));
    }
    if let Some(w) = f.write_times_last.filter(|w| *w > n) {
        return Err(format!(
            "Simulate for {} s ends before the case's output.write_times entry {} s: simulate at least that long",
            fmt_num(n),
            fmt_num(w)
        ));
    }
    let c = rt.courant;
    let mode = if f.own_control.is_empty() {
        rt.mode
    } else {
        StepMode::Case
    };
    if mode != StepMode::Case && !(c.is_finite() && c > 0.0) {
        return Err(format!("the Courant target {c} is not a positive number"));
    }
    let mut entries = vec![entry("solver.end_time", n)];
    let mut no_speed = false;
    let step = match mode {
        StepMode::Case if !f.own_control.is_empty() => Step::CaseControl {
            keys: f.own_control.clone(),
        },
        StepMode::Case => Step::CaseDt { dt: f.dt },
        StepMode::Adaptive => {
            let max_dt = rt.max_dt.unwrap_or(n / MIN_STEPS);
            if !(max_dt.is_finite() && max_dt > 0.0) {
                return Err(format!("the largest step {max_dt} s is not a positive time"));
            }
            let dt0 = match f.step_for(c) {
                Some(d) => round_down(d),
                None => {
                    no_speed = true;
                    f.dt
                }
            }
            .min(max_dt);
            entries.push(entry("solver.dt", dt0));
            entries.push(entry("solver.max_courant", c));
            entries.push(entry("solver.max_dt", max_dt));
            Step::Adaptive {
                dt0,
                courant: c,
                max_dt,
            }
        }
        StepMode::Fixed => {
            let raw = f.step_for(c).ok_or_else(|| {
                format!("no speed scale for a fixed automatic step: the case has {NO_SPEED} at its start")
            })?;
            let steps = ((n / raw) - 1e-9).ceil().max(1.0) as u64;
            let dt = n / steps as f64;
            entries.push(entry("solver.dt", dt));
            Step::Fixed { dt, steps, courant: c }
        }
    };
    let (first, steps) = match &step {
        Step::Adaptive { dt0, .. } => (Some(*dt0), Some((n / dt0 - 1e-6).ceil().max(1.0) as u64)),
        Step::Fixed { dt, steps, .. } => (Some(*dt), Some(*steps)),
        // ADR 0145: a remainder below 1e-6 of a step is no step
        Step::CaseDt { dt } => (Some(*dt), (*dt > 0.0).then(|| (n / dt - 1e-6).ceil().max(1.0) as u64)),
        Step::CaseControl { .. } => (None, None),
    };
    let overridden = matches!(step, Step::Adaptive { .. } | Step::Fixed { .. });
    let correctors = corrector_plan(f, rt);
    entries.extend(correctors.entry());
    Ok(TimePlan {
        end_time: n,
        courant0: first.and_then(|d| f.courant(d)),
        steps,
        wall_s: steps.map(|s| s as f64 * f.cells as f64 * CELL_STEP_SECONDS),
        over_dt: (overridden && f.dt_in_text).then_some(f.dt),
        over_end: (f.end_in_text && n != f.end_time).then_some(f.end_time),
        no_speed,
        step,
        entries,
        correctors,
    })
}

/// The warning before a run whose explicit settings give an estimated Courant number above [`COURANT_WARN`] (ADR 0153
/// D6): the case's own `solver.dt` taken as it is, or a Courant target above the limit. It names the cause and the fix.
pub fn warning(case: &str, f: &CaseFacts, tp: &TimePlan) -> Option<String> {
    match &tp.step {
        Step::CaseDt { dt } => {
            let co = tp.courant0.filter(|c| *c > COURANT_WARN)?;
            let ok = f.step_for(COURANT_WARN).map(round_down).unwrap_or(0.0);
            Some(format!(
                "{case}: this run takes the case's own solver.dt = {} s, an estimated Courant number of {} ({} m/s \
                 through the thinnest cell, {} m): above {}, the run may become unstable or noisy. Fix: use the \
                 automatic time step (run_time mode adaptive), or a solver.dt of at most {} s.",
                fmt_num(*dt),
                fmt_num(co),
                fmt_num(f.u0),
                fmt_num(f.h_min),
                fmt_num(COURANT_WARN),
                fmt_num(ok)
            ))
        }
        Step::Adaptive { courant, .. } | Step::Fixed { courant, .. } if *courant > COURANT_WARN => Some(format!(
            "{case}: the Courant target {} is above {}: the run may become unstable or noisy. Fix: a target of at \
             most {} (the default is {}).",
            fmt_num(*courant),
            fmt_num(COURANT_WARN),
            fmt_num(COURANT_WARN),
            fmt_num(COURANT_TARGET)
        )),
        _ => None,
    }
}

impl TimePlan {
    /// The plan as JSON (`run_time` of `start_run {dry_run}` and `case.facts`), with the warning for `case`.
    pub fn to_json(&self, case: &str, f: &CaseFacts) -> Value {
        let step = match &self.step {
            Step::Adaptive { dt0, courant, max_dt } => {
                json!({"mode": "adaptive", "dt0": dt0, "courant": courant, "max_dt": max_dt})
            }
            Step::Fixed { dt, steps, courant } => {
                json!({"mode": "fixed", "dt": dt, "steps": steps, "courant": courant})
            }
            Step::CaseDt { dt } => json!({"mode": "case", "dt": dt}),
            Step::CaseControl { keys } => json!({"mode": "case_control", "keys": keys}),
        };
        let dt0 = match &self.step {
            Step::Adaptive { dt0, .. } => Some(*dt0),
            Step::Fixed { dt, .. } | Step::CaseDt { dt } => Some(*dt),
            Step::CaseControl { .. } => None,
        };
        json!({
            "end_time": self.end_time,
            "step": step,
            "entries": self.entries,
            "dt0": dt0,
            "steps_estimate": self.steps,
            "courant_estimate": self.courant0,
            "wall_s_estimate": self.wall_s,
            "over_dt": self.over_dt,
            "over_end": self.over_end,
            "no_speed": self.no_speed,
            "correctors": self.correctors.to_json(),
            "warning": warning(case, f, self),
        })
    }
}

/// The plan of `f` with `rt` as JSON, or `{refused: reason}`, with the corrector plan either way.
pub fn plan_json(case: &str, f: &CaseFacts, rt: &RunTime) -> Value {
    match plan(f, rt) {
        Ok(tp) => tp.to_json(case, f),
        Err(why) => json!({"refused": why, "correctors": corrector_plan(f, rt).to_json()}),
    }
}

// ---------------------------------------------------------------- memory

/// The runner's budgets (`rcfd-runner`'s `Config`), for [`memory_mb`]; `Budgets::default()` is the runner's defaults.
pub use rcfd_runner::runner::Config as Budgets;

/// The runner's memory estimate of a run of `cells` cells, MB (`rcfd-runner`'s rule: base plus KB per cell, rounded up).
pub fn memory_mb(cells: u64, budgets: &rcfd_runner::runner::Config) -> u64 {
    (budgets.mem_base_mb + cells as f64 * budgets.mem_kb_per_cell / 1024.0).ceil() as u64
}

/// [`memory_mb`] as JSON with the rule and where the budgets came from (`project` or `runner_defaults`, ADR 0737 Q7).
pub fn memory_json(cells: u64, budgets: &rcfd_runner::runner::Config, from: &str) -> Value {
    json!({
        "mb": memory_mb(cells, budgets),
        "rule": format!("{} MB + {} KB per cell", budgets.mem_base_mb, budgets.mem_kb_per_cell),
        "budgets": from,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // the egui Studio's own tests of the rule (time_step.rs at 44e63df8), with the same numbers
    fn facts_of(u0: f64, h: f64) -> CaseFacts {
        CaseFacts {
            case_hash: String::new(),
            kind: Kind::Transient,
            algorithm: "piso".into(),
            dt: 0.0625,
            dt_in_text: true,
            end_time: 80.0,
            end_in_text: true,
            own_control: Vec::new(),
            write_times_last: None,
            cells: 3072,
            two_d: true,
            bounds: [[0.0; 3], [1.0; 3]],
            h_min: h,
            h_max: h,
            u0,
            u0_from: Some("the initial velocity field".into()),
            correctors: Correctors {
                n: 2,
                control: "fixed".into(),
                not_adaptive: None,
            },
        }
    }

    #[test]
    fn corrector_plan_follows_scope_case_and_switch() {
        let mut f = facts_of(1.0, 0.01);
        let rt = RunTime::default();
        assert_eq!(corrector_plan(&f, &rt), CorrectorPlan::Adaptive { n: 2 });
        assert_eq!(corrector_plan(&f, &rt).entry().as_deref(), Some(ADAPTIVE_ENTRY));
        let off = RunTime {
            adaptive_correctors: false,
            ..RunTime::default()
        };
        assert_eq!(corrector_plan(&f, &off), CorrectorPlan::Fixed { n: 2, scope: None });
        assert_eq!(corrector_plan(&f, &off).entry(), None);
        f.correctors.control = "adaptive".into();
        assert_eq!(corrector_plan(&f, &rt), CorrectorPlan::CaseAdaptive { n: 2 });
        f.correctors.not_adaptive = Some("why".into());
        assert!(matches!(
            corrector_plan(&f, &rt),
            CorrectorPlan::Fixed { scope: Some(_), .. }
        ));
    }

    #[test]
    fn round_down_keeps_three_digits_and_never_rounds_up() {
        assert_eq!(round_down(0.0011716), 0.00117);
        assert_eq!(round_down(0.0625), 0.0625);
        assert_eq!(round_down(12.3456), 12.3);
        assert_eq!(round_down(0.999999), 0.999);
        for x in [1.234e-7, 3.3333, 0.05, 98765.4321] {
            assert!(round_down(x) <= x, "{x}");
        }
    }

    #[test]
    fn adaptive_plan_entries_and_courant() {
        let f = facts_of(1.118, 0.01366);
        let tp = plan(&f, &RunTime::default()).unwrap();
        let dt0 = round_down(0.5 * 0.01366 / 1.118);
        assert_eq!(
            tp.entries,
            vec![
                "solver.end_time=80".to_string(),
                format!("solver.dt={dt0}"),
                "solver.max_courant=0.5".to_string(),
                "solver.max_dt=0.8".to_string(),
                ADAPTIVE_ENTRY.to_string(),
            ]
        );
        assert!(tp.courant0.unwrap() <= 0.5);
        assert_eq!(tp.over_dt, Some(0.0625));
        assert_eq!(tp.over_end, None);
    }

    #[test]
    fn fixed_plan_divides_the_time() {
        let f = facts_of(1.0, 0.01);
        let rt = RunTime {
            simulate: Some(7.3),
            mode: StepMode::Fixed,
            ..RunTime::default()
        };
        let tp = plan(&f, &rt).unwrap();
        let Step::Fixed { dt, steps, .. } = tp.step else {
            panic!()
        };
        assert_eq!(steps, 1460);
        assert!(dt <= 0.005 && (dt * steps as f64 - 7.3).abs() < 1e-12);
    }

    #[test]
    fn warnings_name_cause_and_fix() {
        let f = facts_of(1.118, 0.00262);
        let rt = RunTime {
            mode: StepMode::Case,
            ..RunTime::default()
        };
        let tp = plan(&f, &rt).unwrap();
        let w = warning("cases/c.toml", &f, &tp).unwrap();
        assert!(
            w.contains("solver.dt = 0.0625 s") && w.contains("Fix: use the automatic time step"),
            "{w}"
        );
        let rt = RunTime {
            courant: 3.0,
            ..RunTime::default()
        };
        let tp = plan(&f, &rt).unwrap();
        assert!(warning("c", &f, &tp)
            .unwrap()
            .contains("the Courant target 3 is above 2"));
        let tp = plan(&f, &RunTime::default()).unwrap();
        assert_eq!(warning("c", &f, &tp), None);
    }

    #[test]
    fn no_speed_starts_with_the_case_step() {
        let f = facts_of(0.0, 0.01);
        let tp = plan(&f, &RunTime::default()).unwrap();
        assert!(tp.no_speed);
        assert!(matches!(tp.step, Step::Adaptive { dt0, .. } if dt0 == 0.0625));
        let rt = RunTime {
            mode: StepMode::Fixed,
            ..RunTime::default()
        };
        assert!(plan(&f, &rt).unwrap_err().starts_with("no speed scale"));
    }

    #[test]
    fn run_time_json_members() {
        assert_eq!(RunTime::from_json(&json!({})).unwrap(), RunTime::default());
        let rt = RunTime::from_json(
            &json!({"simulate_s": 2, "courant": 0.4, "mode": "fixed", "adaptive_correctors": false, "max_dt": null}),
        )
        .unwrap();
        assert_eq!(
            (rt.simulate, rt.courant, rt.mode, rt.adaptive_correctors),
            (Some(2.0), 0.4, StepMode::Fixed, false)
        );
        assert!(RunTime::from_json(&json!({"x": 1})).is_err());
        assert!(RunTime::from_json(&json!({"mode": "auto"})).is_err());
        assert!(RunTime::from_json(&json!([])).is_err());
    }
}
