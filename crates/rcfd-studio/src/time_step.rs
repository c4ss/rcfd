//! The time step of a Studio run, set when the run starts and not in the case (ADR 0153, job 48S; the maintainer's
//! request: "set the dt automatically from the field resolution plus the seconds we want to simulate; it should stop and
//! save automatically once we've simulated that many seconds, and it should be set when we run the simulation, not on
//! the case settings").
//!
//! * [`facts`] reads from a case text what the time step depends on: the kind of run ([`Kind`]), the case's own
//!   `solver.dt` and `solver.end_time` (and whether the text sets them), its own step control, the thinnest cell of the
//!   mesh the run builds and the fastest speed of the case at its start. The last two come from the solver's own
//!   initial state (`rcfd_api::sim::Simulation::new`): the largest speed of the initial velocity field and of the
//!   boundary faces at t = 0 (fixed values, free stream, moving walls, boundary expressions and user functions), or, for a
//!   flow at rest under gravity, free fall across the domain.
//! * [`plan`] turns them and the run-time settings ([`RunTime`]: "Simulate for", the Courant target, the largest step,
//!   the case's own step) into the `rcfd run --set` entries of the run ([`TimePlan`]). `solver.end_time` is the time to
//!   simulate. The automatic step ([`StepMode::Adaptive`]) starts at the step that gives the Courant target at the
//!   fastest speed through the thinnest cell (rounded down) and then follows the solver's adaptive Courant controller of
//!   ADR 0050 (`solver.max_courant`, `solver.max_dt`), which lands on the end time; [`StepMode::Fixed`] is the
//!   alternative ADR 0153 measured (a whole number of equal steps).
//! * The texts of the Run toolbar, its hover, the Mesh side's time-step line, and the warning of an estimated Courant
//!   number above [`COURANT_WARN`] from explicit settings.
//!
//! Everything here is a pure function of a [`Facts`] apart from [`facts`] itself, which parses the text and builds the
//! case's simulation: call it off the UI thread.

use crate::viewport::fmt_num;
use rcfd_api::sim::Simulation;
use serde_json::Value;
use std::path::Path;

/// The default Courant target of the automatic step (ADR 0153 D3): inside the stable set that ADR 0069's stability test
/// requires of PISO (`Co_t` 0.25, 0.5 and 1 on an impulsive start), and below the onset this job measured on the
/// 256 x 256 cylinder (ADR 0153, "What I already knew").
pub const COURANT_TARGET: f64 = 0.5;

/// Explicit settings whose estimated Courant number is above this are warned about before the run (ADR 0153 D6).
pub const COURANT_WARN: f64 = 2.0;

/// The largest automatic step is the simulated time over this (so at least about this many steps).
pub const MIN_STEPS: f64 = 100.0;

/// Wall time per cell and step of a single-threaded laminar PISO run on the host of ADR 0153 (4.7 microseconds,
/// measured on the cylinder at 3,072 and 65,536 cells): the rough wall-time estimate of the Run hover.
pub const CELL_STEP_SECONDS: f64 = 4.7e-6;

/// The automatic step of a run unless the user picks another one ([`StepMode::Adaptive`], ADR 0153 D2).
pub const DEFAULT_MODE: StepMode = StepMode::Adaptive;

/// Keys that give a case its own step control (`rcfd-api` `timectl::requested`, and the earlier `solver.max_courant`
/// controller): the automatic step leaves such a case's control alone.
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
    /// Incompressible transient (`piso`, `pimple`, constant density, no VOF, the flow solved): "Simulate for" and the
    /// automatic step apply.
    Transient,
    /// Steady (`simple`, `simplec`, `coupled`): iterations to `solver.max_iterations`, no time step.
    Steady {
        /// `solver.max_iterations`.
        max_iterations: u64,
    },
    /// A transient run outside this job's scope, and why (compressible, variable density, VOF, prescribed flow, a
    /// chain): it runs with the case's own time settings.
    Other(String),
}

/// What the time step of a case depends on ([`facts`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    /// `hash_bytes` of the case text.
    pub hash: String,
    /// The kind of run.
    pub kind: Kind,
    /// `solver.algorithm`.
    pub algorithm: String,
    /// `solver.dt` as resolved (the schema default when the text does not set it), seconds.
    pub dt: f64,
    /// The text sets `solver.dt`.
    pub dt_in_text: bool,
    /// `solver.end_time` as resolved, seconds.
    pub end_time: f64,
    /// The text sets `solver.end_time`.
    pub end_in_text: bool,
    /// The step-control keys ([`OWN_CONTROL`]) the case sets to something other than their unset value.
    pub own_control: Vec<String>,
    /// The latest `output.write_times` entry (a run must reach it).
    pub write_times_last: Option<f64>,
    /// Cells of the mesh (transient runs; 0 otherwise).
    pub cells: u64,
    /// The thinnest cell of the mesh, metres: the smallest cell volume over its largest face area, the faces of `empty`
    /// patches (the front and back of a 2D mesh) left out as the solver's Courant number leaves them out (transient
    /// runs; 0 otherwise).
    pub h_min: f64,
    /// The fastest speed of the case at its start, m/s: the largest of the initial velocity field and of the boundary
    /// faces at t = 0; for a flow at rest under gravity, free fall across the domain, `sqrt(|g| L)` with `L` the largest
    /// extent of the mesh; 0 when there is none.
    pub u0: f64,
    /// Where [`Facts::u0`] is: `the initial velocity field`, `boundary <patch>` or [`GRAVITY_SCALE`].
    pub u0_from: Option<String>,
}

impl Facts {
    /// The estimated Courant number of a step `dt`: the fastest speed through the thinnest cell (`u0 dt / h_min`; ADR
    /// 0153 D4), `None` without a speed or a cell size.
    pub fn courant(&self, dt: f64) -> Option<f64> {
        (self.u0 > 0.0 && self.h_min > 0.0 && self.h_min.is_finite()).then(|| self.u0 * dt / self.h_min)
    }

    /// The step whose estimated Courant number is `c`, unrounded (`None` without a speed).
    pub fn step_for(&self, c: f64) -> Option<f64> {
        (self.u0 > 0.0 && self.h_min > 0.0 && self.h_min.is_finite()).then(|| c * self.h_min / self.u0)
    }
}

/// How the time step of a run is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Automatic, adaptive: the first step from the Courant target, then the solver's Courant controller (ADR 0050).
    Adaptive,
    /// Automatic, fixed: the simulated time in equal steps of at most the Courant target's step (the alternative ADR
    /// 0153 measured).
    Fixed,
    /// The case's own `solver.dt` (or its own step control), unchanged: an explicit setting, warned about above
    /// [`COURANT_WARN`].
    Case,
}

/// The run-time settings of a case's runs (the toolbar's "Simulate for" and its Advanced popover). Not saved in the case.
#[derive(Debug, Clone, PartialEq)]
pub struct RunTime {
    /// Simulated time, seconds (`None`: the case's `solver.end_time`, as `rcfd run` would simulate it).
    pub simulate: Option<f64>,
    /// Courant target of the automatic step.
    pub courant: f64,
    /// Largest automatic step, seconds (`None`: the simulated time over [`MIN_STEPS`]).
    pub max_dt: Option<f64>,
    /// How the step is chosen.
    pub mode: StepMode,
}

impl Default for RunTime {
    fn default() -> Self {
        RunTime {
            simulate: None,
            courant: COURANT_TARGET,
            max_dt: None,
            mode: DEFAULT_MODE,
        }
    }
}

/// The step of a planned run.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// The solver's Courant controller from `dt0` (ADR 0050).
    Adaptive {
        /// First step, seconds.
        dt0: f64,
        /// Courant target (`solver.max_courant`).
        courant: f64,
        /// Largest step (`solver.max_dt`).
        max_dt: f64,
    },
    /// `steps` equal steps of `dt`.
    Fixed {
        /// The step, seconds.
        dt: f64,
        /// Their number.
        steps: u64,
        /// The Courant target it was made for.
        courant: f64,
    },
    /// The case's own `solver.dt`.
    CaseDt {
        /// The step, seconds.
        dt: f64,
    },
    /// The case's own step control (its keys).
    CaseControl {
        /// The keys it sets.
        keys: Vec<String>,
    },
}

/// The time settings of one run: its `rcfd run --set` entries and what the toolbar says of them.
#[derive(Debug, Clone, PartialEq)]
pub struct TimePlan {
    /// Simulated time (`solver.end_time` of the run), seconds.
    pub end_time: f64,
    /// The step.
    pub step: Step,
    /// The `--set` entries, in order (`solver.end_time` first).
    pub entries: Vec<String>,
    /// Estimated Courant number of the first step ([`Facts::courant`]).
    pub courant0: Option<f64>,
    /// Steps: exact for equal steps, an estimate (at most about) for the adaptive step.
    pub steps: Option<u64>,
    /// Rough wall time, seconds ([`CELL_STEP_SECONDS`]).
    pub wall_s: Option<f64>,
    /// The case text's own `solver.dt` that this run replaces.
    pub over_dt: Option<f64>,
    /// The case text's own `solver.end_time` that this run replaces (only when it differs).
    pub over_end: Option<f64>,
    /// No speed scale was found: the adaptive step starts at the case's `solver.dt` (ADR 0153 D5).
    pub no_speed: bool,
}

// ---------------------------------------------------------------- facts

fn diags(d: &[rcfd_api::case::Diagnostic]) -> String {
    d.iter()
        .map(|x| format!("{}: {}", x.path, x.message))
        .collect::<Vec<_>>()
        .join("; ")
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

/// The kind of run of a parsed case.
fn kind_of(case: &rcfd_api::case::Case) -> Kind {
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let algorithm = get("solver.algorithm").as_str().unwrap_or_default().to_string();
    if matches!(algorithm.as_str(), "simple" | "simplec" | "coupled") {
        return Kind::Steady {
            max_iterations: get("solver.max_iterations").as_u64().unwrap_or(0),
        };
    }
    if rcfd_api::comp::enabled(case) {
        return Kind::Other("the compressible solver is explicit and sets its own step from compressible.cfl".into());
    }
    if !matches!(algorithm.as_str(), "piso" | "pimple") {
        return Kind::Other(format!("solver.algorithm \"{algorithm}\" has no automatic time step"));
    }
    if rcfd_api::vd::enabled(case) {
        return Kind::Other(
            "a variable-density run (fluid.density) takes fixed steps only, without the Courant controller".into(),
        );
    }
    if rcfd_api::vof::enabled(case) {
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

/// The thinnest cell of `sim`'s mesh ([`Facts::h_min`]).
fn thinnest(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let m = &g.mesh;
    let (areas, vols) = (m.face_area_vectors(), m.cell_volumes());
    let mut h = f64::INFINITY;
    for (c, v) in vols.iter().enumerate() {
        let largest = m
            .cell_faces(c)
            .iter()
            .filter(|&&f| !g.empty[f])
            .map(|&f| rcfd_core::vec3::norm(areas[f]))
            .fold(0.0, f64::max);
        if largest > 0.0 {
            h = h.min(v / largest);
        }
    }
    h
}

/// The fastest speed of `sim` at its start and where it is ([`Facts::u0`]).
fn fastest(sim: &Simulation) -> Result<(f64, Option<String>), String> {
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
    // ADR 0153 D5: a flow at rest driven by gravity (buoyancy): free fall across the domain, sqrt(|g| L), an upper bound
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
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in sim.geom.mesh.points() {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let l = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
        if l > 0.0 && l.is_finite() {
            return Ok(((g * l).sqrt(), Some(GRAVITY_SCALE.into())));
        }
    }
    Ok((0.0, None))
}

/// Where the speed scale of a flow at rest under gravity comes from (ADR 0153 D5).
pub const GRAVITY_SCALE: &str = "gravity (free fall across the domain, sqrt(|g| L))";

/// What a case without a speed scale lacks (ADR 0153 D5).
pub const NO_SPEED: &str = "no inflow, moving wall, initial velocity or gravity";

/// What the time step of the case `text` depends on (`dir`: its folder, for relative files). The text is read as
/// `rcfd run` reads it (JSON when it starts with `{`, else TOML: [`crate::preview::parse_case`]); a transient case's
/// simulation is built (`Simulation::new`, as the run builds it) for its thinnest cell and fastest speed. Errors are the
/// located messages of the parse or of the build.
pub fn facts(text: &str, dir: &Path) -> Result<Facts, String> {
    let hash = rcfd_project::records::hash_bytes(text.as_bytes());
    let case = crate::preview::parse_case(text, dir).map_err(|d| diags(&d))?;
    let raw = crate::runs::text_tree(text, false)?;
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let num = |p: &str| get(p).as_f64().unwrap_or(0.0);
    let own_control = OWN_CONTROL
        .iter()
        .filter(|k| set_value(&get(k)))
        .map(|k| k.to_string())
        .collect();
    let write_times_last = get("output.write_times")
        .as_array()
        .and_then(|a| a.iter().filter_map(Value::as_f64).reduce(f64::max));
    let mut f = Facts {
        hash,
        kind: kind_of(&case),
        algorithm: get("solver.algorithm").as_str().unwrap_or_default().to_string(),
        dt: num("solver.dt"),
        dt_in_text: text_sets(&raw, &["solver", "dt"]),
        end_time: num("solver.end_time"),
        end_in_text: text_sets(&raw, &["solver", "end_time"]),
        own_control,
        write_times_last,
        cells: 0,
        h_min: 0.0,
        u0: 0.0,
        u0_from: None,
    };
    if f.kind == Kind::Transient {
        let sim = Simulation::new(&case).map_err(|d| diags(&d))?;
        f.cells = sim.geom.mesh.n_cells() as u64;
        f.h_min = thinnest(&sim);
        (f.u0, f.u0_from) = fastest(&sim)?;
    }
    Ok(f)
}

// ---------------------------------------------------------------- the plan

/// `x` rounded down to 3 significant digits (a step that reads well and keeps its Courant number at most the target).
pub fn round_down(x: f64) -> f64 {
    if !(x.is_finite() && x > 0.0) {
        return x;
    }
    // digits kept after the decimal point; powers of ten up to 1e22 are exact, so 0.0625 stays 0.0625
    let k = 2 - x.log10().floor() as i32;
    let p = 10f64.powi(k.abs());
    if k >= 0 {
        (x * p).floor() / p
    } else {
        (x / p).floor() * p
    }
}

/// A `--set` entry of a number (Rust's shortest text that reads back to the same number).
fn entry(path: &str, v: f64) -> String {
    format!("{path}={v}")
}

/// The time settings of a run of a case with facts `f` and run-time settings `rt` ([`TimePlan`]); refused, with the
/// reason, for a case that is not [`Kind::Transient`], a simulated time that is not positive, an end before one of the
/// case's own `output.write_times`, a Courant target or largest step that is not positive, and a fixed automatic step
/// without a speed scale.
pub fn plan(f: &Facts, rt: &RunTime) -> Result<TimePlan, String> {
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
    })
}

/// The warning before a run whose explicit settings give an estimated Courant number above [`COURANT_WARN`] (ADR 0153
/// D6): the case's own `solver.dt` taken as it is, or a Courant target above the limit. It names the cause and the fix.
pub fn warning(case: &str, f: &Facts, tp: &TimePlan) -> Option<String> {
    match &tp.step {
        Step::CaseDt { dt } => {
            let co = tp.courant0.filter(|c| *c > COURANT_WARN)?;
            let ok = f.step_for(COURANT_WARN).map(round_down).unwrap_or(0.0);
            Some(format!(
                "{case}: this run takes the case's own solver.dt = {} s, an estimated Courant number of {} ({} m/s \
                 through the thinnest cell, {} m): above {}, the run may become unstable or noisy. Fix: use the \
                 automatic time step (Advanced), or a solver.dt of at most {} s.",
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
             most {} (Advanced; the default is {}).",
            fmt_num(*courant),
            fmt_num(COURANT_WARN),
            fmt_num(COURANT_WARN),
            fmt_num(COURANT_TARGET)
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------- texts

/// A wall time in words: `40 s`, `3.5 min`, `1.2 h`.
pub fn wall_text(s: f64) -> String {
    if s < 90.0 {
        format!("{} s", fmt_num(s.max(0.1).round()))
    } else if s < 5400.0 {
        format!("{} min", fmt_num((s / 6.0).round() / 10.0))
    } else {
        format!("{} h", fmt_num((s / 360.0).round() / 10.0))
    }
}

/// The toolbar's line of the step of a planned run (next to "Simulate for").
pub fn label(tp: &TimePlan) -> String {
    match &tp.step {
        Step::Adaptive { dt0, courant, .. } => format!(
            "dt auto: {} s at the start, then adaptive (Courant {})",
            fmt_num(*dt0),
            fmt_num(*courant)
        ),
        Step::Fixed { dt, steps, courant } => format!(
            "dt auto: {} s, {} steps (Courant {})",
            fmt_num(*dt),
            crate::preview::with_commas(*steps),
            fmt_num(*courant)
        ),
        Step::CaseDt { dt } => format!("dt {} s, the case's own", fmt_num(*dt)),
        Step::CaseControl { keys } => format!("the case's own step control ({})", keys.join(", ")),
    }
}

/// The indicator that this run overrides the case's own `solver.dt` or `solver.end_time` (`None`: it does not).
pub fn overridden(tp: &TimePlan) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(dt) = tp.over_dt {
        parts.push(format!("solver.dt {} s", fmt_num(dt)));
    }
    if let Some(t) = tp.over_end {
        parts.push(format!("solver.end_time {} s", fmt_num(t)));
    }
    (!parts.is_empty()).then(|| format!("overrides the case's {} for this run", parts.join(" and ")))
}

/// The hover text of a planned run: `this run: dt=..., Courant ..., N steps, estimated wall time ...` in sentences.
pub fn hover(f: &Facts, tp: &TimePlan) -> String {
    let n = fmt_num(tp.end_time);
    let mut s = match &tp.step {
        Step::Adaptive { dt0, courant, max_dt } => format!(
            "This run: {n} s of simulated time; dt = {} s at the start{}, then the solver's Courant controller keeps the \
             largest cell Courant number at about {} and at most {} (steps of at most {} s) and lands on {n} s.",
            fmt_num(*dt0),
            if tp.no_speed {
                format!(" (the case's solver.dt: {NO_SPEED} gives a speed at the start)")
            } else {
                String::new()
            },
            fmt_num(0.9 * courant),
            fmt_num(*courant),
            fmt_num(*max_dt)
        ),
        Step::Fixed { dt, steps, courant } => format!(
            "This run: {n} s of simulated time in {} equal steps of dt = {} s (Courant target {}).",
            crate::preview::with_commas(*steps),
            fmt_num(*dt),
            fmt_num(*courant)
        ),
        Step::CaseDt { dt } => format!("This run: {n} s of simulated time with the case's own solver.dt = {} s.", fmt_num(*dt)),
        Step::CaseControl { keys } => format!(
            "This run: {n} s of simulated time with the case's own step control ({}).",
            keys.join(", ")
        ),
    };
    if let Some(co) = tp.courant0 {
        s.push_str(&format!(
            " Estimated Courant number of the first step: {} ({} m/s at the start, from {}, through the thinnest cell, \
             {} m; an estimate on the high side).",
            fmt_num(co),
            fmt_num(f.u0),
            f.u0_from.as_deref().unwrap_or("the case"),
            fmt_num(f.h_min)
        ));
    }
    match (tp.steps, tp.wall_s, &tp.step) {
        (Some(k), Some(w), Step::Adaptive { .. }) => s.push_str(&format!(
            " About {} steps or fewer (the step grows to the Courant target), estimated wall time about {} or less \
             ({} cells, at {} microseconds per cell and step).",
            crate::preview::with_commas(k),
            wall_text(w),
            crate::preview::with_commas(f.cells),
            fmt_num(CELL_STEP_SECONDS * 1e6)
        )),
        (Some(k), Some(w), _) => s.push_str(&format!(
            " {} steps, estimated wall time about {} ({} cells, at {} microseconds per cell and step).",
            crate::preview::with_commas(k),
            wall_text(w),
            crate::preview::with_commas(f.cells),
            fmt_num(CELL_STEP_SECONDS * 1e6)
        )),
        _ => {}
    }
    if let Some(o) = overridden(tp) {
        s.push_str(&format!(
            " This run {}: the case file and the case editor keep them; the run's settings record the values it used.",
            o.trim_end_matches(" for this run")
        ));
    }
    s.push_str(" The run stops at the end time and stores its final state there.");
    s
}

/// The toolbar's line of a case that is not [`Kind::Transient`] (`None` for a transient case).
pub fn other_label(f: &Facts) -> Option<String> {
    match &f.kind {
        Kind::Transient => None,
        Kind::Steady { max_iterations } => Some(format!(
            "steady ({}): until converged, at most {} iterations",
            f.algorithm,
            crate::preview::with_commas(*max_iterations)
        )),
        Kind::Other(_) => Some(format!(
            "{}: the case's own time step, to its end time {} s",
            f.algorithm,
            fmt_num(f.end_time)
        )),
    }
}

/// The hover text of [`other_label`].
pub fn other_hover(f: &Facts) -> Option<String> {
    match &f.kind {
        Kind::Transient => None,
        Kind::Steady { max_iterations } => Some(format!(
            "A steady run iterates towards its converged state: it has no simulated time and no time step. Run uses the \
             case's solver.max_iterations ({}). Simulate for applies to transient piso and pimple runs.",
            crate::preview::with_commas(*max_iterations)
        )),
        Kind::Other(why) => Some(format!(
            "Not set by the Studio: {why}. Run uses the case's own solver.dt and solver.end_time as they are; Simulate \
             for and the automatic time step apply to incompressible piso and pimple runs."
        )),
    }
}

/// The Mesh side's time-step line of a transient case (ADR 0153 D7; `None` for other cases): the estimated Courant number
/// of the case's own step on this mesh, and what Run sets.
pub fn mesh_line(f: &Facts, rt: &RunTime) -> Option<String> {
    if f.kind != Kind::Transient {
        return None;
    }
    let tp = plan(f, rt).ok();
    let own = if f.dt_in_text {
        "the case's solver.dt"
    } else {
        "solver.dt (the schema default)"
    };
    let est = match f.courant(f.dt) {
        Some(co) => format!(
            "Courant estimate {} at {own} {} s ({} m/s through the thinnest cell, {} m)",
            fmt_num(co),
            fmt_num(f.dt),
            fmt_num(f.u0),
            fmt_num(f.h_min)
        ),
        None => format!(
            "no speed scale at the start ({NO_SPEED}), so no Courant estimate; thinnest cell {} m",
            fmt_num(f.h_min)
        ),
    };
    let run = match tp.as_ref().map(|t| &t.step) {
        Some(Step::Adaptive { dt0, courant, .. }) => format!(
            "Run sets the time step: {} s at the start for Courant {}, then adaptive{}",
            fmt_num(*dt0),
            fmt_num(*courant),
            if f.dt_in_text {
                ", so the case's solver.dt is overridden at run time"
            } else {
                ""
            }
        ),
        Some(Step::Fixed { dt, courant, .. }) => format!(
            "Run sets the time step: {} s for Courant {}{}",
            fmt_num(*dt),
            fmt_num(*courant),
            if f.dt_in_text {
                ", so the case's solver.dt is overridden at run time"
            } else {
                ""
            }
        ),
        Some(Step::CaseDt { .. }) => "Run takes the case's own solver.dt (Advanced)".to_string(),
        Some(Step::CaseControl { keys }) => format!("Run keeps the case's own step control ({})", keys.join(", ")),
        None => "Run sets the time step when it starts".to_string(),
    };
    Some(format!("{est}; {run}."))
}

/// The hover text of [`mesh_line`].
pub const MESH_HOVER: &str = "The estimated Courant number is the fastest speed of the case at its start (the initial \
     velocity field and the boundary values at t = 0) times the step, over the thinnest cell (the cell volume over its \
     largest face; the front and back of a 2D mesh left out): an estimate on the high side, because near walls the flow \
     runs along the thin cells. Run sets the time step when the run starts, from the mesh the run builds; the case file \
     keeps its own solver.dt.";

/// The sentence "Refine x2" adds to its hover for a transient case (ADR 0153 D8): the time step follows the finer cells
/// at run time.
pub fn refine_note(f: &Facts, rt: &RunTime) -> Option<String> {
    let tp = plan(f, rt).ok()?;
    let dt0 = match tp.step {
        Step::Adaptive { dt0, .. } => dt0,
        Step::Fixed { dt, .. } => dt,
        _ => return None,
    };
    Some(format!(
        "The time step is not edited: Run sets it from the finer cells when it starts (about half of the {} s it starts \
         with now){}.",
        fmt_num(dt0),
        if f.dt_in_text {
            "; the case's solver.dt is overridden at run time"
        } else {
            ""
        }
    ))
}

/// The Start run dialog's lines for a transient case (ADR 0153 D9): the dialog's run takes the case's own time settings
/// (with the dialog's overrides `set`, last wins), and, when that `solver.dt` gives an estimated Courant number above
/// [`COURANT_WARN`], the warning. `(warning, text)` pairs; none for other cases.
pub fn dialog_lines(case: &str, f: &Facts, set: &[String]) -> Vec<(bool, String)> {
    if f.kind != Kind::Transient {
        return Vec::new();
    }
    let (mut dt, mut end, mut controlled) = (f.dt, f.end_time, !f.own_control.is_empty());
    for e in set {
        if let Some((p, v)) = crate::store_plan::parse_entry(e) {
            match p.as_str() {
                "solver.dt" => dt = v.as_f64().unwrap_or(dt),
                "solver.end_time" => end = v.as_f64().unwrap_or(end),
                p if OWN_CONTROL.contains(&p) && set_value(&v) => controlled = true,
                _ => {}
            }
        }
    }
    let mut v = vec![(
        false,
        format!(
            "Time step: this dialog's run takes solver.dt = {} s to solver.end_time = {} s (the case's, unless an \
             override sets them); Run on the toolbar sets the time step for you (Simulate for, Advanced).",
            fmt_num(dt),
            fmt_num(end)
        ),
    )];
    if let Some(co) = f.courant(dt).filter(|c| *c > COURANT_WARN && !controlled) {
        v.push((
            true,
            format!(
                "{case}: solver.dt = {} s gives an estimated Courant number of {} ({} m/s through the thinnest cell, {} \
                 m): above {}, the run may become unstable or noisy. Fix: Run on the toolbar (automatic time step), or \
                 an override solver.dt of at most {} s.",
                fmt_num(dt),
                fmt_num(co),
                fmt_num(f.u0),
                fmt_num(f.h_min),
                fmt_num(COURANT_WARN),
                fmt_num(f.step_for(COURANT_WARN).map(round_down).unwrap_or(0.0))
            ),
        ));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts_of(u0: f64, h: f64) -> Facts {
        Facts {
            hash: String::new(),
            kind: Kind::Transient,
            algorithm: "piso".into(),
            dt: 0.0625,
            dt_in_text: true,
            end_time: 80.0,
            end_in_text: true,
            own_control: Vec::new(),
            write_times_last: None,
            cells: 3072,
            h_min: h,
            u0,
            u0_from: Some("the initial velocity field".into()),
        }
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
                "solver.max_dt=0.8".to_string()
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
}
