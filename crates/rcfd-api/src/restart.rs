//! The `restart` section of the case tree (F-115, ADR 0031 D3): start a run from the solution in a
//! restart file, onto the case's own mesh (exactly when it is the same mesh, by field transfer
//! otherwise).

use crate::case::{Case, Diagnostic};
use crate::sim::Simulation;
use rcfd_core::settings::Schema;
use rcfd_post::remap::RemapMethod;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Names accepted by `restart.method`.
pub const METHODS: &[&str] = &[
    "gradient",
    "nearest",
    "inverse_distance",
    "least_squares",
    "conservative",
    "conservative_overlap",
];

/// Schema of the `restart` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Start the run from the solution in a restart file (ADR 0031). Unset by default (from = \"\"): a fresh start")
        .field(
            "from",
            Schema::string("").doc("Restart file written by a previous run (rcfd run writes <out>/restart.rcfd); \"\" = fresh start. Relative to the case file. A file of another mesh is transferred onto this mesh"),
        )
        .field(
            "source_case",
            Schema::string("").doc("Case file that built the mesh of the restart file; only needed for files that do not embed their mesh (written before ADR 0031). \"\" = unset"),
        )
        .field(
            "method",
            Schema::enumeration("gradient", METHODS).doc("Transfer method used when the restart file is on another mesh: gradient (nearest cell plus limited gradient), nearest, inverse_distance, least_squares (linear fit over neighbours), conservative (gradient plus a uniform shift matching the volume average) or conservative_overlap (cell-overlap remap with a linear reconstruction; axis-aligned box cells only)"),
        )
}

fn diag(path: &str, message: String) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message,
    }
}

/// Static checks of the `restart` section.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let from = case.get("restart.from").ok().and_then(|v| v.as_str().map(String::from));
    let src = case
        .get("restart.source_case")
        .ok()
        .and_then(|v| v.as_str().map(String::from));
    if from.as_deref().is_some_and(str::is_empty) && src.as_deref().is_some_and(|s| !s.is_empty()) {
        out.push(diag(
            "restart.source_case",
            "restart.source_case is set but restart.from is not".into(),
        ));
    }
    out
}

/// The transfer method of the case.
pub fn method_of(case: &Case) -> RemapMethod {
    case.get("restart.method")
        .ok()
        .and_then(|v| v.as_str().and_then(RemapMethod::parse))
        .unwrap_or(RemapMethod::Gradient)
}

/// True when the case asks for a restart.
pub fn requested(case: &Case) -> bool {
    case.get("restart.from")
        .ok()
        .and_then(|v| v.as_str().map(|s| !s.is_empty()))
        .unwrap_or(false)
}

fn resolve(base: &Path, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

/// Restart `sim` as the `restart` section of `case` says. Returns the restart report
/// (`restart`: exact, initialised or transferred; coverage and flux imbalance for a transfer),
/// or `None` when the case asks for no restart.
pub fn apply_case(sim: &mut Simulation, case: &Case) -> Result<Option<Value>, String> {
    if !requested(case) {
        return Ok(None);
    }
    let from = case.get("restart.from").map_err(|e| e.to_string())?;
    let from = resolve(&case.base_dir, from.as_str().unwrap_or(""));
    let src = case
        .get("restart.source_case")
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.is_empty());
    restart_with(
        sim,
        &from,
        method_of(case),
        src.as_deref().map(|s| resolve(&case.base_dir, s)).as_deref(),
    )
}

/// Restart `sim` from `file` with `method`; `source_case` is a case file for restart files that do
/// not embed their mesh.
pub fn restart_with(
    sim: &mut Simulation,
    file: &Path,
    method: RemapMethod,
    source_case: Option<&Path>,
) -> Result<Option<Value>, String> {
    let src = match source_case {
        Some(p) => Some(Case::load(p).map_err(|d| {
            format!(
                "restart.source_case {}: {}",
                p.display(),
                d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            )
        })?),
        None => None,
    };
    sim.restart_from(file, method, src.as_ref())
        .map(Some)
        .map_err(|e| format!("restart from {}: {e}", file.display()))
}
