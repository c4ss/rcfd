//! The start plan of a run (ADR 0173 D7, ADR 0186 G4): the field store a run gets so that its fields can be scrubbed,
//! the proposal of ADR 0140 that the Studio's start dialog makes (`rcfd_studio::runs::propose_store` and
//! `StartPlan::set`), with the same rule, constants, entry text and order, snapshot count and size estimate. Only the
//! store: the time settings (ADR 0153) and pressure correctors (ADR 0158) that the Studio's Run adds are not part of it.
//!
//! The rule, for a case as `rcfd run` reads it: no store when the case sets `output.store_interval` or
//! `output.store_time_interval` to a value other than its schema default (ADR 0152) or sets `dynamic_mesh.enabled`;
//! a transient run (`piso`, `pimple`, `compressible` unless `compressible.mode = "steady"`) with an `end_time` gets
//! `output.store_time_interval = end_time / 100`; every other run `output.store_interval = max(1, ceil(max_iterations /
//! 100))`; and `output.store_precision = f32` unless the case sets the precision itself. The Studio keeps its own copy
//! of the rule until it uses this one; a parity test of the Studio (`new_simulations_f271.rs`) pins the two equal.

use rcfd_api::case::{Case, Diagnostic};
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

/// Snapshots a planned store aims at (ADR 0140, W48-5 (a): at most 100).
pub const STORE_SNAPSHOTS: u64 = 100;

/// Fields the store keeps when the case names none (`rcfd_api::fieldstore::DEFAULT_FIELDS`).
pub const STORE_FIELDS: u64 = 4;

/// Whether `start_run` of a project case that exists already applies the plan when the call does not say (`store`
/// omitted). `false` keeps such a call as it was before ADR 0186: ADR 0173 D7 (a) turns it on, which changes what
/// `start_run` writes for cases with no store setting and so waits for the maintainer's approval (ADR 0186 Q2). A case
/// made by the same call (`new_case`) had no behaviour before and gets the plan unless `store` is `"none"`.
pub const PLAN_FOR_PROJECT_CASES: bool = false;

/// The keys that decide whether and how often a store is written.
pub const INTERVAL_KEYS: [&str; 2] = ["output.store_interval", "output.store_time_interval"];

const PRECISION_KEY: &str = "output.store_precision";

/// Snapshot spacing of a planned store.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Every {
    /// `output.store_interval` (steps or iterations).
    Steps(u64),
    /// `output.store_time_interval` (simulated seconds).
    Time(f64),
}

/// The store a run of one case gets.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Snapshot spacing; `None` when no store is proposed.
    pub every: Option<Every>,
    /// Why no store is proposed.
    pub note: Option<String>,
    /// The case sets `output.store_precision` itself, so no precision entry is added.
    pub sets_precision: bool,
    /// `solver.max_iterations` of the case.
    pub max_iterations: u64,
    /// Cells, when the mesh generator's counts give them.
    pub cells: Option<u64>,
}

/// The schema defaults of the interval and precision keys (a case with every default).
fn defaults() -> &'static [(&'static str, Value)] {
    static D: OnceLock<Vec<(&'static str, Value)>> = OnceLock::new();
    D.get_or_init(|| {
        let c = Case::new();
        INTERVAL_KEYS
            .iter()
            .chain([&PRECISION_KEY])
            .map(|k| (*k, c.get(k).unwrap_or(Value::Null)))
            .collect()
    })
}

/// The case sets store key `key` itself: its value differs from the schema default (numbers by value, so `0`, `0.0`
/// and `-0.0` are one; ADR 0152).
fn sets(case: &Case, key: &str) -> bool {
    let Some((_, d)) = defaults().iter().find(|(k, _)| *k == key) else {
        return false;
    };
    case.get(key).is_ok_and(|v| match (v.as_f64(), d.as_f64()) {
        (Some(a), Some(b)) => a != b,
        _ => v != *d,
    })
}

impl Plan {
    /// The plan of `case` (read as `rcfd run` reads it, with any `--set` entries already applied).
    pub fn of(case: &Case) -> Plan {
        let get = |p: &str| case.get(p).unwrap_or(Value::Null);
        let int = |p: &str| get(p).as_u64().unwrap_or(0);
        let algorithm = get("solver.algorithm").as_str().unwrap_or_default().to_string();
        let max_iterations = int("solver.max_iterations");
        let end_time = get("solver.end_time").as_f64().unwrap_or(0.0);
        let generator = get("mesh.generator").as_str().unwrap_or_default().to_string();
        let cells = (get("mesh.source").as_str() == Some("generator"))
            .then_some(())
            .and_then(|()| match generator.as_str() {
                "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" | "cylinder_ogrid" => {
                    Some(int("mesh.nx") * int("mesh.ny"))
                }
                "annulus_3d" | "annulus_3d_sector" => Some(int("mesh.nx") * int("mesh.ny") * int("mesh.nz")),
                _ => None,
            });
        let mut plan = Plan {
            every: None,
            note: None,
            sets_precision: sets(case, PRECISION_KEY),
            max_iterations,
            cells,
        };
        if INTERVAL_KEYS.iter().any(|k| sets(case, k)) {
            plan.note = Some("the case sets its own field store (output.store_*)".into());
            return plan;
        }
        if get("dynamic_mesh.enabled").as_bool() == Some(true) {
            plan.note = Some("no field store: the store does not support dynamic_mesh.enabled".into());
            return plan;
        }
        let steady = matches!(algorithm.as_str(), "simple" | "simplec" | "coupled");
        let compressible_steady = algorithm == "compressible" && get("compressible.mode").as_str() == Some("steady");
        let transient =
            matches!(algorithm.as_str(), "piso" | "pimple") || (algorithm == "compressible" && !compressible_steady);
        plan.every = Some(if !steady && transient && end_time > 0.0 {
            Every::Time(end_time / STORE_SNAPSHOTS as f64)
        } else {
            Every::Steps(max_iterations.div_ceil(STORE_SNAPSHOTS).max(1))
        });
        plan
    }

    /// The plan of a run of `case` with its `set` entries (`path=value`) applied first, as `rcfd run --set` applies them
    /// (`rcfd_api::chain::apply_set`), so an override of `solver.max_iterations` or `solver.end_time` is planned for. No
    /// store when an entry gives the store's interval itself ([`INTERVAL_KEYS`], whatever its value: ADR 0173 D7, the
    /// caller sets the store explicitly). A malformed or refused entry is an error naming it. What `start_run` uses.
    pub fn with_set(mut case: Case, set: &[String]) -> Result<Plan, String> {
        for e in set {
            rcfd_api::chain::apply_set(&mut case, e).map_err(|x| format!("set: {x}"))?;
        }
        let own = set
            .iter()
            .filter_map(|e| e.split_once('=').map(|(p, _)| p.trim()))
            .any(|p| INTERVAL_KEYS.contains(&p));
        Ok(if own {
            Plan::none(
                "the run's set entries give the field store (output.store_interval or output.store_time_interval)",
            )
        } else {
            Plan::of(&case)
        })
    }

    /// No store, for `reason`.
    pub fn none(reason: impl Into<String>) -> Plan {
        Plan {
            every: None,
            note: Some(reason.into()),
            sets_precision: false,
            max_iterations: 0,
            cells: None,
        }
    }

    /// The `rcfd run --set` entries of the plan, in the Studio's order (the run's own entries follow them).
    pub fn entries(&self) -> Vec<String> {
        let Some(every) = self.every else {
            return Vec::new();
        };
        let mut v = vec![match every {
            Every::Steps(k) => format!("output.store_interval={k}"),
            Every::Time(dt) => format!("output.store_time_interval={dt}"),
        }];
        if !self.sets_precision {
            v.push("output.store_precision=f32".into());
        }
        v
    }

    /// Snapshots the store is expected to hold at most (steps rule: `ceil(M / k)`; time rule: the target count).
    pub fn snapshots(&self) -> Option<u64> {
        Some(match self.every? {
            Every::Steps(k) => self.max_iterations.div_ceil(k.max(1)).max(1),
            Every::Time(_) => STORE_SNAPSHOTS,
        })
    }

    /// Estimated store size in bytes (cells x 4 fields x snapshots x 4 bytes, the f32 width the plan asks for), when the
    /// cell count is known: the start dialog's estimate, not a measured value.
    pub fn estimated_bytes(&self) -> Option<u64> {
        Some(self.cells? * STORE_FIELDS * self.snapshots()? * 4)
    }
}

/// Parse case `text` as `rcfd run <file>` reads it: JSON when the text starts with `{`, else TOML (ADR 0149); relative
/// data files against `dir`, confined to `sandbox` when given.
pub fn parse_case(text: &str, dir: &Path, sandbox: Option<&Path>) -> Result<Case, Vec<Diagnostic>> {
    if crate::create::is_json_text(text) {
        Case::parse_json_with(text, dir, sandbox)
    } else {
        Case::parse_toml_with(text, dir, sandbox)
    }
}
