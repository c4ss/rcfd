//! Headless launch and run-control contract (F-121): documented exit codes, check-only,
//! continue-from, stop file / wall-clock limit interrupts, and a machine-readable status stream.
//!
//! The same driver serves `rcfd run` (status stream on stdout with `--status-json`) and the
//! service method `run.headless` (used by the MCP server), so both produce identical events.
//! See `docs/run-control.md`.

use crate::case::{Case, Diagnostic};
use crate::sim::Simulation;
use rcfd_mesh::quality::check_mesh;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// `format` field of every status event.
pub const STATUS_FORMAT: &str = "rcfd-status";
/// Version of the status event format.
pub const STATUS_VERSION: u32 = 1;

/// Process exit codes of `rcfd run` (and `exit_code` of the final status event).
pub mod exit {
    /// Finished: converged, or a transient run reached its end time.
    pub const OK: u8 = 0;
    /// Runtime or I/O failure.
    pub const FAIL: u8 = 1;
    /// Usage error (bad command line).
    pub const USAGE: u8 = 2;
    /// Invalid case file or settings.
    pub const INVALID: u8 = 3;
    /// The mesh failed its quality check.
    pub const BAD_MESH: u8 = 4;
    /// Steady run stopped without meeting its convergence targets.
    pub const NOT_CONVERGED: u8 = 5;
    /// Residuals became non-finite.
    pub const DIVERGED: u8 = 6;
    /// Interrupted through the stop file (or the service); a checkpoint is written when supported.
    pub const INTERRUPTED: u8 = 7;
    /// The `--max-elapsed` wall-clock limit was reached.
    pub const TIME_LIMIT: u8 = 8;
}

/// Options of one headless launch.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Stop after this many steps of this launch (the run is then `step_limit`, not finished).
    pub max_steps: Option<usize>,
    /// Wall-clock limit in seconds.
    pub max_elapsed: Option<f64>,
    /// The run stops cleanly when this file exists.
    pub stop_file: Option<PathBuf>,
    /// Emit an `iteration` event every this many steps (0 = 10); the first five and the last always.
    pub status_every: usize,
    /// Write a restart file here at the end, when interrupted and every `checkpoint_every` steps.
    pub checkpoint: Option<PathBuf>,
    /// Steps between periodic checkpoints (0 = only at the end).
    pub checkpoint_every: usize,
    /// Output directory: with `output.write_interval` > 0 numbered snapshots go to `<out_dir>/snapshots`
    /// (newest `output.retain_snapshots` kept).
    pub out_dir: Option<PathBuf>,
    /// Informational fields copied into the `start` event (threads, continue-from, ...).
    pub info: Value,
}

/// Final result of a launch.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    /// Process exit code (see [`exit`]).
    pub exit_code: u8,
    /// `converged`, `finished`, `not_converged`, `diverged`, `interrupted`, `time_limit`,
    /// `step_limit` or `failed`.
    pub state: &'static str,
    /// Iterations / steps completed in total (including a continued run).
    pub iterations: usize,
    /// Simulation time (transient).
    pub time: f64,
    /// Human readable reason.
    pub reason: String,
    /// Wall-clock seconds of this launch.
    pub elapsed_s: f64,
}

impl RunOutcome {
    /// The outcome as the body of the `finish` event.
    pub fn to_json(&self) -> Value {
        json!({
            "state": self.state, "exit_code": self.exit_code, "iterations": self.iterations,
            "time": self.time, "reason": self.reason, "elapsed_s": self.elapsed_s,
        })
    }
}

fn event(kind: &str, seq: &mut u64, elapsed: f64, body: Value) -> Value {
    *seq += 1;
    let mut v = json!({
        "format": STATUS_FORMAT, "version": STATUS_VERSION,
        "event": kind, "seq": *seq, "wall_s": elapsed,
    });
    if let (Some(o), Some(b)) = (v.as_object_mut(), body.as_object()) {
        for (k, x) in b {
            o.insert(k.clone(), x.clone());
        }
    }
    v
}

/// A status event (shared with the chain driver).
pub fn make_event(kind: &str, seq: &mut u64, elapsed: f64, body: Value) -> Value {
    event(kind, seq, elapsed, body)
}

fn lock_sim(m: &Mutex<Simulation>) -> std::sync::MutexGuard<'_, Simulation> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn diag(message: String) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        line: None,
        message,
    }
}

/// Check a case without running it (`--check-only`): settings, simulation set-up and mesh quality.
/// Returns the exit code and a `check` status event.
pub fn check_only(case: &Case) -> (u8, Value) {
    let mut seq = 0;
    let mut diags: Vec<Diagnostic> = case.semantic_errors();
    if diags.is_empty() {
        if let Err(d) = Simulation::new(case) {
            diags = d;
        }
    }
    let mesh = if diags.is_empty() {
        match crate::sim::build_mesh(case) {
            Ok(m) => Some(m),
            Err(d) => {
                diags = d;
                None
            }
        }
    } else {
        None
    };
    let Some(mesh) = mesh else {
        let v = event(
            "check",
            &mut seq,
            0.0,
            json!({"ok": false, "exit_code": exit::INVALID, "diagnostics": diags, "rules": [], "mesh": null}),
        );
        return (exit::INVALID, v);
    };
    let q = check_mesh(&mesh);
    let rules = crate::rules::check_case(case, Some(&crate::rules::mesh_context(&q)));
    let code = if !q.is_ok() {
        exit::BAD_MESH
    } else if crate::rules::has_errors(&rules) {
        exit::INVALID
    } else {
        exit::OK
    };
    let v = event(
        "check",
        &mut seq,
        0.0,
        json!({
            "ok": code == exit::OK, "exit_code": code, "cells": q.n_cells,
            "diagnostics": [], "rules": crate::rules::to_json(&rules),
            "mesh": serde_json::to_value(&q).unwrap_or(Value::Null),
        }),
    );
    (code, v)
}

/// Drive `sim` to its end, an interrupt or a limit, emitting status events to `sink`.
///
/// `should_stop` is polled between steps (the service passes its stop flag); the stop file and the
/// wall-clock limit of `opts` are checked in the same place. The simulation lock is held only for
/// one step at a time.
pub fn drive(
    sim: &Mutex<Simulation>,
    opts: &RunOptions,
    should_stop: &dyn Fn() -> bool,
    sink: &mut dyn FnMut(&Value),
) -> RunOutcome {
    let t0 = Instant::now();
    let mut seq = 0u64;
    let every = if opts.status_every == 0 { 10 } else { opts.status_every };
    {
        let g = lock_sim(sim);
        sink(&event(
            "start",
            &mut seq,
            0.0,
            json!({
                "cells": g.geom.n_cells(), "algorithm": g.case.get("solver.algorithm").unwrap_or(Value::Null),
                "steady": g.is_steady(), "start_iteration": g.iteration(), "info": opts.info,
                "case_hash": format!("{:016x}", g.case.tree.content_hash()),
            }),
        ));
    }
    // setup warnings found before solving (rule engine), passed in `info.warnings` as objects with
    // `message`, `rule` and `path`
    if let Some(ws) = opts.info.get("warnings").and_then(Value::as_array) {
        for w in ws {
            sink(&event("warning", &mut seq, 0.0, w.clone()));
        }
    }
    let finish = |state: &'static str, code: u8, reason: String| {
        let g = lock_sim(sim);
        RunOutcome {
            exit_code: code,
            state,
            iterations: g.iteration(),
            time: g.time(),
            reason,
            elapsed_s: t0.elapsed().as_secs_f64(),
        }
    };
    let save = |seq: &mut u64, sink: &mut dyn FnMut(&Value), why: &str| {
        let Some(path) = &opts.checkpoint else { return };
        let g = lock_sim(sim);
        if !g.supports_restart() {
            if !matches!(why, "interrupted" | "time_limit" | "periodic") {
                return;
            }
            sink(&event(
                "warning",
                seq,
                t0.elapsed().as_secs_f64(),
                json!({"message": "this run cannot be restarted (supported: steady SIMPLE/SIMPLEC without turbulence, and transient laminar PISO/PIMPLE without energy, scalars or statistics)"}),
            ));
            return;
        }
        let keep = g
            .case
            .get("output.retain_checkpoints")
            .ok()
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let numbered = why == "periodic" && keep > 1;
        let target = if numbered {
            let mut n = path.clone().into_os_string();
            n.push(format!(".{}", g.iteration()));
            PathBuf::from(n)
        } else {
            path.clone()
        };
        match g.save_restart(&target) {
            Ok(()) => {
                if numbered {
                    prune_numbered(path, keep);
                }
                sink(&event(
                    "checkpoint",
                    seq,
                    t0.elapsed().as_secs_f64(),
                    json!({"path": target.display().to_string(), "iteration": g.iteration(), "why": why}),
                ))
            }
            Err(e) => sink(&event(
                "warning",
                seq,
                t0.elapsed().as_secs_f64(),
                json!({"message": format!("checkpoint failed: {e}")}),
            )),
        }
    };
    let (snapshot_every, retain_snapshots) = {
        let g = lock_sim(sim);
        let get = |p: &str| g.case.get(p).ok().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        (get("output.write_interval"), get("output.retain_snapshots"))
    };
    let mut steps = 0usize;
    let mut pending: Option<Value> = None;
    let outcome = loop {
        if should_stop() {
            break finish("interrupted", exit::INTERRUPTED, "stop requested".into());
        }
        if let Some(f) = opts.stop_file.as_ref().filter(|f| f.exists()) {
            break finish(
                "interrupted",
                exit::INTERRUPTED,
                format!("stop file {} found", f.display()),
            );
        }
        if let Some(m) = opts.max_elapsed {
            if t0.elapsed().as_secs_f64() >= m {
                break finish(
                    "time_limit",
                    exit::TIME_LIMIT,
                    format!("wall-clock limit of {m} s reached"),
                );
            }
        }
        if opts.max_steps.is_some_and(|m| steps >= m) {
            break finish("step_limit", exit::OK, format!("{steps} steps taken"));
        }
        let step = {
            let mut g = lock_sim(sim);
            if g.is_done() {
                let (steady, conv) = (g.is_steady(), g.is_converged());
                let reason = g.stop_reason().unwrap_or("finished").to_string();
                let failure = g.failure().map(String::from);
                drop(g);
                break if let Some(f) = failure {
                    finish("failed", exit::FAIL, f)
                } else if conv {
                    finish("converged", exit::OK, reason)
                } else if !steady {
                    finish("finished", exit::OK, reason)
                } else {
                    finish("not_converged", exit::NOT_CONVERGED, reason)
                };
            }
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| g.step())) {
                Ok(s) => s,
                Err(_) => {
                    drop(g);
                    break finish("failed", exit::FAIL, "solver panicked".into());
                }
            }
        };
        steps += 1;
        if let (Some(dir), true) = (&opts.out_dir, snapshot_every > 0) {
            if step.index % snapshot_every == 0 {
                let g = lock_sim(sim);
                let snaps = dir.join("snapshots");
                let name = format!("snapshot_{:08}", step.index);
                match g.export_vtu(&snaps, &name) {
                    Ok(path) => {
                        drop(g);
                        prune_snapshots(&snaps, retain_snapshots);
                        sink(&event(
                            "snapshot",
                            &mut seq,
                            t0.elapsed().as_secs_f64(),
                            json!({"path": path.display().to_string(), "index": step.index, "time": step.time}),
                        ));
                    }
                    Err(e) => sink(&event(
                        "warning",
                        &mut seq,
                        t0.elapsed().as_secs_f64(),
                        json!({"message": format!("snapshot failed: {e}")}),
                    )),
                }
            }
        }
        let body = json!({
            "index": step.index, "time": step.time, "res_u": step.res_u, "res_p": step.res_p,
            "continuity": step.continuity, "done": step.done,
        });
        if step.index <= 5 || step.index % every == 0 || step.done {
            pending = None;
            sink(&event("iteration", &mut seq, t0.elapsed().as_secs_f64(), body));
        } else {
            pending = Some(body);
        }
        if !step.res_u.is_finite() || !step.res_p.is_finite() {
            break finish("diverged", exit::DIVERGED, "residuals are not finite".into());
        }
        if opts.checkpoint_every > 0 && steps % opts.checkpoint_every == 0 {
            save(&mut seq, sink, "periodic");
        }
    };
    // the last step is always reported, whatever the decimation
    if let Some(body) = pending {
        sink(&event("iteration", &mut seq, outcome.elapsed_s, body));
    }
    if matches!(
        outcome.state,
        "interrupted" | "time_limit" | "converged" | "not_converged" | "finished" | "step_limit"
    ) {
        save(&mut seq, sink, outcome.state);
    }
    let mut body = outcome.to_json();
    body["ok"] = json!(outcome.exit_code == exit::OK);
    sink(&event("finish", &mut seq, outcome.elapsed_s, body));
    outcome
}

/// Remove the oldest `snapshot_*.vtu` files of `dir` until at most `keep` remain (`keep` 0 = all kept).
fn prune_snapshots(dir: &Path, keep: usize) {
    if keep == 0 {
        return;
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("snapshot_") && n.ends_with(".vtu"))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let excess = files.len().saturating_sub(keep);
    for f in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(f);
    }
}

/// Remove the oldest numbered checkpoints `<path>.<iteration>` until at most `keep` remain.
fn prune_numbered(path: &Path, keep: usize) {
    let (Some(dir), Some(stem)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let prefix = format!("{stem}.");
    let mut files: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter_map(|p| {
                    let n = p.file_name()?.to_str()?.strip_prefix(&prefix)?.parse::<u64>().ok()?;
                    Some((n, p))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let excess = files.len().saturating_sub(keep);
    for (_, f) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(f);
    }
}

/// Write the standard result files (`solution.vtu`, `monitors.json/csv`, `report.html`) to `dir`.
pub fn write_results(sim: &Simulation, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    sim.export_vtu(dir, "solution")?;
    std::fs::write(dir.join("monitors.json"), sim.monitors.to_json().to_string())?;
    std::fs::write(dir.join("report.html"), sim.html_report())?;
    std::fs::write(dir.join("monitors.csv"), sim.monitors.to_csv())?;
    crate::postproc::write_outputs(sim, dir)?;
    Ok(())
}

/// Rebuild a case from a service journal (`journal.get` document): the `create` entry's source
/// followed by the recorded `set` / `remove` changes (journal playback, `rcfd run --journal`).
pub fn case_from_journal(journal: &Value, base_dir: &Path, sandbox: Option<&Path>) -> Result<Case, Vec<Diagnostic>> {
    if journal["format"] != "rcfd-journal" || journal["version"] != 1 {
        return Err(vec![diag("not an rcfd-journal version 1 document".into())]);
    }
    let entries = journal["entries"]
        .as_array()
        .ok_or_else(|| vec![diag("journal has no entries".into())])?;
    let first = entries.first().filter(|e| e["op"] == "create");
    let src = first.map_or(json!({}), |e| e["source"].clone());
    let mut case = if let Some(p) = src["path"].as_str() {
        Case::load_with(base_dir.join(p), sandbox)?
    } else if let Some(t) = src["toml"].as_str() {
        Case::parse_toml_with(t, base_dir, sandbox)?
    } else if src["settings"].is_object() {
        Case::from_json_with(&src["settings"], base_dir, sandbox)?
    } else {
        Case::new()
    };
    for e in entries.iter().skip(usize::from(first.is_some())) {
        let path = e["path"].as_str().unwrap_or_default();
        match e["op"].as_str() {
            Some("set") => case
                .set(path, e["value"].clone())
                .map_err(|err| vec![diag(format!("journal entry set {path}: {err}"))])?,
            Some("remove") => {
                let (coll, name) = path.rsplit_once('.').unwrap_or(("", path));
                case.tree
                    .remove_named(coll, name)
                    .map_err(|err| vec![diag(format!("journal entry remove {path}: {err}"))])?;
            }
            _ => {}
        }
    }
    let errs = case.semantic_errors();
    if errs.is_empty() {
        Ok(case)
    } else {
        Err(errs)
    }
}

/// Summarise a status stream (JSON lines of [`STATUS_FORMAT`] events): final state, exit code,
/// iterations, the last residuals and the warnings. Used by the MCP server and the CLI.
pub fn summarize_stream(text: &str) -> Result<Value, String> {
    let mut events = Vec::new();
    for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1))?;
        if v["format"] != STATUS_FORMAT {
            return Err(format!("line {}: not an {STATUS_FORMAT} event", i + 1));
        }
        if v["version"] != STATUS_VERSION {
            return Err(format!("line {}: unsupported status version {}", i + 1, v["version"]));
        }
        events.push(v);
    }
    let last = |k: &str| events.iter().rev().find(|e| e["event"] == k).cloned();
    let finish = last("finish");
    let iter = last("iteration");
    Ok(json!({
        "events": events.len(),
        "complete": finish.is_some(),
        "state": finish.as_ref().map_or(json!("running"), |f| f["state"].clone()),
        "exit_code": finish.as_ref().map_or(Value::Null, |f| f["exit_code"].clone()),
        "reason": finish.as_ref().map_or(Value::Null, |f| f["reason"].clone()),
        "iterations": iter.as_ref().map_or(Value::Null, |f| f["index"].clone()),
        "last_iteration": iter,
        "checkpoints": events.iter().filter(|e| e["event"] == "checkpoint").count(),
        "warnings": events.iter().filter(|e| e["event"] == "warning").map(|e| e["message"].clone()).collect::<Vec<_>>(),
    }))
}
