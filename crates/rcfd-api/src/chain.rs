//! Chained run configurations (F-241, ADR 0031 D4): a sequence of stages, each a run of the base
//! case with a few settings changed, each starting from the restart file of the previous one.
//!
//! ```toml
//! [chain]
//! sequence = ["steady", "transient"]
//! [chain.stages.transient]
//! set = ["solver.algorithm=\"piso\"", "solver.dt=0.05", "solver.end_time=1.0"]
//! ```
//!
//! `set` entries accumulate: stage n is the base case with the `set` entries of stages 1..n applied
//! in order. A stage writes `<out>/<stage>/restart.rcfd` and its results to `<out>/<stage>/`; the
//! next stage restarts from that file (a changed mesh makes it a field transfer).

use crate::case::{Case, Diagnostic};
use crate::runctl::{self, exit, RunOptions, RunOutcome};
use crate::sim::Simulation;
use rcfd_core::settings::Schema;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// Schema of the `chain` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Chained run configurations (ADR 0031): stages run one after the other, each restarting from the previous one. Empty sequence = no chain")
        .field(
            "sequence",
            Schema::list(Schema::string("")).doc("Names of the stages (entries of chain.stages) in the order they run; empty = no chain"),
        )
        .field(
            "stages",
            Schema::named(
                Schema::object()
                    .doc("One stage of the chain")
                    .field(
                        "set",
                        Schema::list(Schema::string("")).doc("Settings changed for this stage and the following ones, each \"dotted.path=<JSON value>\", for example \"solver.dt=0.05\" or \"solver.algorithm=\\\"piso\\\"\". Applied in order on top of the previous stage"),
                    )
                    .field(
                        "restart",
                        Schema::enumeration("auto", &["auto", "previous", "none"]).doc("auto and previous: start from the restart file of the previous stage (the first stage uses restart.from when set); none: fresh start"),
                    )
                    .field(
                        "method",
                        Schema::enumeration("gradient", crate::restart::METHODS).doc("Field-transfer method when the mesh changes from the previous stage"),
                    )
                    .field(
                        "allow_unconverged",
                        Schema::boolean(false).doc("Continue with the next stage when this steady stage stops at its iteration cap without converging (exit code 5); otherwise the chain stops with that code"),
                    ),
            )
            .doc("Stage definitions by name"),
        )
}

/// Why a `--set` / `set` entry was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum SetError {
    /// Not of the form `path=value` or the value is not JSON (a usage error on the command line).
    Malformed(String),
    /// Unknown path or a value the setting does not accept.
    Invalid(String),
}

impl std::fmt::Display for SetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetError::Malformed(m) | SetError::Invalid(m) => f.write_str(m),
        }
    }
}

/// Apply one `dotted.path=<JSON value>` entry to `case`. A bare word that is not valid JSON is
/// taken as a string, so `solver.algorithm=piso` works as well as `solver.algorithm="piso"`.
pub fn apply_set(case: &mut Case, entry: &str) -> Result<(), SetError> {
    let Some((path, value)) = entry.split_once('=') else {
        return Err(SetError::Malformed(format!(
            "'{entry}': expected dotted.path=<JSON value>"
        )));
    };
    let path = path.trim();
    if path.is_empty() || path.contains(char::is_whitespace) {
        return Err(SetError::Malformed(format!(
            "'{entry}': empty or malformed setting path"
        )));
    }
    let value = value.trim();
    let v: Value = match serde_json::from_str(value) {
        Ok(v) => v,
        Err(_) if !value.is_empty() && value.chars().all(|c| c.is_alphanumeric() || "_-.+/".contains(c)) => {
            Value::String(value.to_string())
        }
        Err(e) => return Err(SetError::Malformed(format!("'{entry}': the value is not JSON ({e})"))),
    };
    case.set(path, v).map_err(|e| SetError::Invalid(format!("{path}: {e}")))
}

/// Names of the stages in run order.
pub fn sequence(case: &Case) -> Vec<String> {
    case.get("chain.sequence")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// True when the case defines a chain.
pub fn is_chain(case: &Case) -> bool {
    !sequence(case).is_empty()
}

fn stage_sets(case: &Case, stage: &str) -> Vec<String> {
    case.get(&format!("chain.stages.{stage}.set"))
        .ok()
        .and_then(|v| v.as_array().cloned())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn diag(path: String, message: String) -> Diagnostic {
    Diagnostic {
        path,
        line: None,
        message,
    }
}

/// Static checks of the `chain` section: located diagnostics for unknown or repeated stage names
/// in the sequence, malformed `set` entries, unknown setting paths and values of the wrong type.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let seq = sequence(case);
    let stages = case.tree.names("chain.stages").unwrap_or_default();
    if seq.is_empty() {
        return out;
    }
    for (i, s) in seq.iter().enumerate() {
        if !stages.contains(s) {
            out.push(diag(
                format!("chain.sequence.{i}"),
                format!("stage '{s}' is not defined in chain.stages"),
            ));
        } else if seq[..i].contains(s) {
            out.push(diag(
                format!("chain.sequence.{i}"),
                format!("stage '{s}' appears more than once in the sequence"),
            ));
        }
    }
    // entries are tried on a scratch copy, cumulatively in sequence order
    let mut scratch = case.clone();
    for s in seq.iter().filter(|s| stages.contains(*s)) {
        for (i, e) in stage_sets(case, s).iter().enumerate() {
            let at = format!("chain.stages.{s}.set.{i}");
            if let Some((p, _)) = e.split_once('=') {
                let p = p.trim();
                if p == "chain" || p.starts_with("chain.") {
                    out.push(diag(at, "a stage cannot change the chain section".into()));
                    continue;
                }
            }
            if let Err(err) = apply_set(&mut scratch, e) {
                out.push(diag(at, err.to_string()));
            }
        }
    }
    out
}

/// The case of every stage, in run order (changes accumulate). Errors are located diagnostics.
pub fn stage_cases(case: &Case) -> Result<Vec<(String, Case)>, Vec<Diagnostic>> {
    let errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut cur = case.clone();
    let mut out = Vec::new();
    for s in sequence(case) {
        for (i, e) in stage_sets(case, &s).iter().enumerate() {
            apply_set(&mut cur, e).map_err(|err| vec![diag(format!("chain.stages.{s}.set.{i}"), err.to_string())])?;
        }
        out.push((s, cur.clone()));
    }
    Ok(out)
}

/// Result of a chain.
#[derive(Debug, Clone)]
pub struct ChainOutcome {
    /// Exit code of the first stage that did not finish with 0 (or an accepted 5), else 0.
    pub exit_code: u8,
    /// `finished`, `stopped` or `failed`.
    pub state: &'static str,
    /// Stages that were started: name and outcome (`None` when the stage failed before the driver ran).
    pub stages: Vec<(String, Option<RunOutcome>)>,
}

/// Run `case` as its chain. `base` supplies the run-control options shared by all stages (stop file,
/// wall-clock limit, status decimation, checkpoint interval); `out_dir` and the restart file are set
/// per stage. Events carry a `stage` field; `stage_start`, `stage_finish` and `chain_finish` are
/// added. Every stage's results are written to `<out_dir>/<stage>/`.
pub fn run_chain(
    case: &Case,
    base: &RunOptions,
    out_dir: &Path,
    should_stop: &dyn Fn() -> bool,
    sink: &mut dyn FnMut(&Value),
) -> ChainOutcome {
    let t0 = Instant::now();
    let seq = std::cell::Cell::new(0u64);
    let sink = std::cell::RefCell::new(sink);
    let emit = |kind: &str, body: Value| {
        let mut n = seq.get();
        let ev = runctl::make_event(kind, &mut n, t0.elapsed().as_secs_f64(), body);
        seq.set(n);
        (sink.borrow_mut())(&ev);
    };
    let stages = match stage_cases(case) {
        Ok(s) => s,
        Err(d) => {
            for x in &d {
                emit("error", json!({"path": x.path, "message": x.message}));
            }
            emit(
                "chain_finish",
                json!({"ok": false, "exit_code": exit::INVALID, "state": "failed", "reason": "invalid chain"}),
            );
            return ChainOutcome {
                exit_code: exit::INVALID,
                state: "failed",
                stages: Vec::new(),
            };
        }
    };
    let total = stages.len();
    let mut done: Vec<(String, Option<RunOutcome>)> = Vec::new();
    let mut prev_restart: Option<PathBuf> = None;
    let mut code = exit::OK;
    let mut state: &'static str = "finished";
    for (idx, (name, scase)) in stages.iter().enumerate() {
        emit("stage_start", json!({"stage": name, "index": idx, "of": total}));
        let dir = out_dir.join(name);
        let (outcome, stage_code) = run_stage(
            name,
            scase,
            case,
            &dir,
            prev_restart.as_deref(),
            base,
            should_stop,
            &mut |ev: &Value| (sink.borrow_mut())(ev),
            &mut |kind: &str, body: Value| emit(kind, body),
        );
        let mut accepted = stage_code == exit::OK;
        if stage_code == exit::NOT_CONVERGED {
            accepted = case
                .get(&format!("chain.stages.{name}.allow_unconverged"))
                .ok()
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }
        emit(
            "stage_finish",
            json!({
                "stage": name, "exit_code": stage_code,
                "state": outcome.as_ref().map_or("failed", |o| o.state),
                "accepted": accepted,
            }),
        );
        done.push((name.clone(), outcome));
        prev_restart = Some(dir.join("restart.rcfd"));
        if !accepted {
            code = stage_code;
            state = if stage_code == exit::INTERRUPTED || stage_code == exit::TIME_LIMIT {
                "stopped"
            } else {
                "failed"
            };
            break;
        }
    }
    emit(
        "chain_finish",
        json!({
            "ok": code == exit::OK, "exit_code": code, "state": state,
            "stages_run": done.len(), "stages": total,
        }),
    );
    ChainOutcome {
        exit_code: code,
        state,
        stages: done,
    }
}

/// Run one stage. Returns the driver outcome (when the driver ran) and the stage's exit code.
#[allow(clippy::too_many_arguments)]
fn run_stage(
    name: &str,
    scase: &Case,
    chain_case: &Case,
    dir: &Path,
    prev_restart: Option<&Path>,
    base: &RunOptions,
    should_stop: &dyn Fn() -> bool,
    raw_sink: &mut dyn FnMut(&Value),
    extra: &mut dyn FnMut(&str, Value),
) -> (Option<RunOutcome>, u8) {
    let fail = |extra: &mut dyn FnMut(&str, Value), code: u8, msg: String| {
        extra("error", json!({"stage": name, "message": msg}));
        (None, code)
    };
    let mut sim = match Simulation::new(scase) {
        Ok(s) => s,
        Err(d) => {
            return fail(
                extra,
                exit::INVALID,
                d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
            )
        }
    };
    let q = rcfd_mesh::quality::check_mesh(&sim.geom.mesh);
    if !q.is_ok() {
        return fail(extra, exit::BAD_MESH, format!("mesh error: {}", q.errors.join("; ")));
    }
    let rd = crate::rules::check_case(scase, Some(&crate::rules::mesh_context(&q)));
    if crate::rules::has_errors(&rd) {
        for d in rd.iter().filter(|d| d.severity == crate::rules::Severity::Error) {
            extra(
                "error",
                json!({"stage": name, "message": d.message, "rule": d.id, "path": d.path}),
            );
        }
        return (None, exit::INVALID);
    }
    let mut warnings = crate::rules::warnings_json(&rd);
    // where the stage starts from
    let mode = scase
        .get(&format!("chain.stages.{name}.restart"))
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "auto".into());
    let report = if mode == "none" {
        Ok(None)
    } else if let Some(prev) = prev_restart {
        let method = chain_case
            .get(&format!("chain.stages.{name}.method"))
            .ok()
            .and_then(|v| v.as_str().and_then(rcfd_post::remap::RemapMethod::parse))
            .unwrap_or(rcfd_post::remap::RemapMethod::Gradient);
        crate::restart::restart_with(&mut sim, prev, method, None)
    } else {
        crate::restart::apply_case(&mut sim, scase)
    };
    match report {
        Ok(Some(r)) => extra("restart", json!({"stage": name, "report": r})),
        Ok(None) => {}
        Err(e) => return fail(extra, exit::FAIL, e),
    }
    // ADR 0172 D2: the dynamic set-up rules (SV301 at the settled step, SV302) of the stage's restarted state, as `rcfd run`
    let dynamic = crate::rules::dynamic_setup_diagnostics(&sim);
    if let (Some(all), Value::Array(w)) = (warnings.as_array_mut(), crate::rules::warnings_json(&dynamic)) {
        all.extend(w);
    }
    let mut opts = base.clone();
    opts.checkpoint = Some(dir.join("restart.rcfd"));
    opts.out_dir = Some(dir.to_path_buf());
    opts.info = json!({"stage": name, "chain": true, "base": base.info, "warnings": warnings});
    if let Err(e) = std::fs::create_dir_all(dir) {
        return fail(extra, exit::FAIL, format!("cannot create {}: {e}", dir.display()));
    }
    let sim = Mutex::new(sim);
    let outcome = runctl::drive(&sim, &opts, should_stop, &mut |ev| {
        let mut ev = ev.clone();
        ev["stage"] = json!(name);
        raw_sink(&ev);
    });
    let sim = sim.into_inner().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = runctl::write_results(&sim, dir) {
        extra(
            "warning",
            json!({"stage": name, "message": format!("writing results to {}: {e}", dir.display())}),
        );
    }
    let code = outcome.exit_code;
    (Some(outcome), code)
}
