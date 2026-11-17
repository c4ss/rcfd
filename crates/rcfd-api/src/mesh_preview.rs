//! `mesh.preview` (F-836, ADR 0732): the mesh a session's case runs on, built without solving and written as a
//! mesh-only field store (no snapshot, no field) the Studio reads with its store reader.
//!
//! * The mesh is the one `Simulation::new` sets the run up on: `sim::build_mesh` for a plain case (what
//!   `Simulation::new` calls), and `Simulation::new`'s own mesh for a case with `[mesh.adapt]` (the base lattice) or
//!   `[mesh.sliding]` (the joined blocks). With `adapt: "final"` a `[mesh.adapt]` case runs the run's adaptive loop
//!   (`sim::adapted_mesh`, `cycles + 1` solves) and the store holds its final mesh (ADR 0732 Q3 (a)).
//! * Before a generator mesh is built its cells are estimated from the `mesh.n*` counts; over [`ESTIMATE_LIMIT`] the
//!   call is refused (1004) unless `force: true` (Q4 (b)). After any build, more cells than the service's `max_cells`
//!   are refused (1004) before anything is written.
//! * The store goes to `<dir>/<mesh hash>/` (Q2 (a)): written in a temporary folder beside it and renamed. A folder of
//!   that name that already holds a store of the same mesh hash is used again without writing (`reused: true`).
//!
//! Opt-in: only a service started with `rcfd serve --ui` has the method (Q1 (a), `crate::ui::UI_METHODS`).

use crate::case::{Case, Diagnostic};
use crate::service::{code, Rpc, RpcError};
use crate::sim::{self, Simulation};
use rcfd_mesh::{Mesh, PatchKind};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The cell estimate above which a generator mesh is not built without `force: true` (the egui preview's limit, ADR
/// 0138).
pub const ESTIMATE_LIMIT: u64 = 2_000_000;

/// Parameters `mesh.preview` takes.
pub const PARAMS: [&str; 4] = ["session", "dir", "adapt", "force"];

/// Which mesh of a `[mesh.adapt]` case to preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adapt {
    /// The mesh the run starts on (its first step changes it).
    Base,
    /// The mesh the run solves on after its first step (runs the adaptive loop).
    Final,
}

/// The checked parameters (everything but the session).
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The folder of previews; the store goes to `<dir>/<mesh hash>/`.
    pub dir: String,
    /// Base or final mesh.
    pub adapt: Adapt,
    /// Build even when the estimate is over [`ESTIMATE_LIMIT`].
    pub force: bool,
}

impl Request {
    /// Check the parameters: an object of [`PARAMS`] only, `session` and `dir` strings, `adapt` `"base"` or `"final"`,
    /// `force` a boolean. -32602 otherwise.
    pub fn from_json(p: &Value) -> Rpc<Request> {
        let o = p
            .as_object()
            .ok_or_else(|| RpcError::params("mesh.preview takes an object {session, dir, adapt?, force?}"))?;
        if let Some(k) = o.keys().find(|k| !PARAMS.contains(&k.as_str())) {
            return Err(RpcError::params(format!(
                "unknown parameter '{k}' (mesh.preview takes {})",
                PARAMS.join(", ")
            )));
        }
        if !o.get("session").is_some_and(Value::is_string) {
            return Err(RpcError::params("missing string parameter 'session'"));
        }
        let dir = match o.get("dir") {
            Some(Value::String(d)) if !d.is_empty() => d.clone(),
            _ => return Err(RpcError::params("missing string parameter 'dir'")),
        };
        let adapt = match o.get("adapt") {
            None | Some(Value::Null) => Adapt::Base,
            Some(Value::String(s)) if s == "base" => Adapt::Base,
            Some(Value::String(s)) if s == "final" => Adapt::Final,
            Some(v) => {
                return Err(RpcError::params(format!(
                    "'adapt' must be \"base\" or \"final\", not {v}"
                )))
            }
        };
        let force = match o.get("force") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(v) => return Err(RpcError::params(format!("'force' must be a boolean, not {v}"))),
        };
        Ok(Request { dir, adapt, force })
    }
}

fn int(case: &Case, path: &str) -> u64 {
    case.get(path).ok().and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64
}

fn text(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// The cells of the case's generator mesh before it is built: the product of its `mesh.n*` counts for the generators
/// whose count is that product (the rule of the egui preview, ADR 0138; checked against the built mesh by a test). None
/// for a mesh file and for a generator counted otherwise.
pub fn estimate(case: &Case) -> Option<u64> {
    if text(case, "mesh.source") == "file" {
        return None;
    }
    let (nx, ny, nz) = (int(case, "mesh.nx"), int(case, "mesh.ny"), int(case, "mesh.nz").max(1));
    let right = |key: &str, same: u64| match int(case, key) {
        0 => same,
        n => n,
    };
    let (nxr, nyr, nzr) = (
        right("mesh.nx_right", nx),
        right("mesh.ny_right", ny),
        right("mesh.nz_right", nz),
    );
    let m = |a: u64, b: u64, c: u64| a.saturating_mul(b).saturating_mul(c);
    Some(match text(case, "mesh.generator").as_str() {
        "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" => m(nx, ny, 1),
        "channel_2d_interface" => m(nx, ny, 1).saturating_add(m(nxr, nyr, 1)),
        "annulus_3d" | "annulus_3d_sector" => m(nx, ny, nz),
        "annulus_3d_interface" => m(nx, ny, nz).saturating_add(m(nx, nyr, nzr)),
        "annulus_3d_stage" | "annulus_3d_cylindrical_interface" => m(nx, ny, nz).saturating_add(m(nxr, nyr, nzr)),
        _ => return None,
    })
}

/// The mesh `Simulation::new(case)` sets the run up on: `build_mesh` for a plain case (`Simulation::new` calls it),
/// `Simulation::new`'s own mesh for `[mesh.adapt]` and `[mesh.sliding]`, whose run meshes differ from `build_mesh`'s.
pub fn run_mesh(case: &Case) -> Result<Mesh, Vec<Diagnostic>> {
    if sim::has_mesh_adapt(case) || sim::has_mesh_sliding(case) {
        return Simulation::new(case).map(|s| (*s.geom.mesh).clone());
    }
    sim::build_mesh(case)
}

/// What `adapt` reports for a `[mesh.adapt]` case: `pending` when the run's first step changes the mesh shown.
fn adapt_json(case: &Case, pending: bool, solves: usize) -> Value {
    json!({"pending": pending, "cycles": int(case, "mesh.adapt.cycles"), "solves": solves})
}

fn bounds(mesh: &Mesh) -> Value {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if mesh.n_points() == 0 {
        lo = [0.0; 3];
        hi = [0.0; 3];
    }
    json!({"min": lo, "max": hi})
}

/// The hexadecimal mesh hash the reply and the folder name use (16 digits, as the Studio's store reader prints it).
pub fn hash_hex(mesh: &Mesh) -> String {
    format!("{:016x}", mesh.content_hash())
}

static TMP: AtomicU64 = AtomicU64::new(0);

fn internal(message: String) -> RpcError {
    RpcError::new(code::INTERNAL, message)
}

/// True when `dir` holds a store of `mesh` (same hash, cells and points).
fn holds(dir: &Path, mesh: &Mesh) -> bool {
    rcfd_tsstore::Reader::open(dir).is_ok_and(|r| {
        r.mesh_hash() == mesh.content_hash() && r.n_cells() == mesh.n_cells() && r.n_points() == mesh.n_points()
    })
}

/// Write `mesh` as a mesh-only store at `<dir>/<hash>/` (temporary folder, then rename); `Ok(true)` when a store of
/// that mesh was there already. On failure nothing is left: neither the temporary folder nor `<dir>/<hash>/`.
fn write_store(dir: &Path, mesh: &Mesh) -> Rpc<(PathBuf, bool)> {
    let hash = hash_hex(mesh);
    let path = dir.join(&hash);
    if holds(&path, mesh) {
        return Ok((path, true));
    }
    std::fs::create_dir_all(dir).map_err(|e| internal(format!("cannot create {}: {e}", dir.display())))?;
    let tmp = dir.join(format!(
        ".tmp-{hash}-{}-{}",
        std::process::id(),
        TMP.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| -> Result<(), String> {
        let w = rcfd_tsstore::Writer::create(&tmp, mesh, rcfd_tsstore::WriteOptions::default())
            .map_err(|e| e.to_string())?;
        w.finish().map_err(|e| e.to_string())?;
        if path.exists() {
            // a folder of this name that does not hold this mesh (damaged, or not a store): replaced
            std::fs::remove_dir_all(&path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
        }
        std::fs::rename(&tmp, &path).map_err(|e| format!("cannot rename to {}: {e}", path.display()))
    })();
    match written {
        Ok(()) => Ok((path, false)),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            // another call may have written the same mesh in the meantime
            if holds(&path, mesh) {
                return Ok((path, true));
            }
            Err(internal(format!("the preview store cannot be written: {e}")))
        }
    }
}

/// The work of `mesh.preview` once the session's case, its paths and `dir` are checked (`dir` resolved in the sandbox).
pub fn preview(case: &Case, dir: &Path, req: &Request, max_cells: usize) -> Rpc {
    if !req.force {
        if let Some(n) = estimate(case) {
            if n > ESTIMATE_LIMIT {
                return Err(RpcError::new(
                    code::LIMIT,
                    format!(
                        "about {n} cells (from the mesh.n* counts of generator \"{}\"), more than the preview limit of \
                         {ESTIMATE_LIMIT}; pass force: true to build it anyway",
                        text(case, "mesh.generator")
                    ),
                ));
            }
        }
    }
    let adaptive = sim::has_mesh_adapt(case);
    let (mesh, adapt) = if adaptive && req.adapt == Adapt::Final {
        let a = sim::adapted_mesh(case, &|| false)
            .map_err(|e| {
                RpcError::validation(vec![Diagnostic {
                    path: "mesh.adapt".into(),
                    line: None,
                    message: format!("the adaptive loop failed: {e}"),
                }])
            })?
            .ok_or_else(|| internal("the adaptive loop stopped".into()))?;
        let solves = a.cycles.len();
        ((*a.mesh).clone(), adapt_json(case, false, solves))
    } else {
        let mesh = run_mesh(case).map_err(RpcError::validation)?;
        let adapt = if adaptive {
            adapt_json(case, true, 0)
        } else {
            Value::Null
        };
        (mesh, adapt)
    };
    if mesh.n_cells() > max_cells {
        return Err(RpcError::new(
            code::LIMIT,
            format!("mesh has {} cells, limit is {max_cells}", mesh.n_cells()),
        ));
    }
    let (path, reused) = write_store(dir, &mesh)?;
    Ok(json!({
        "path": path.display().to_string(),
        "mesh_hash": hash_hex(&mesh),
        "cells": mesh.n_cells(),
        "faces": mesh.n_faces(),
        "points": mesh.n_points(),
        "patches": mesh.patches().iter().map(|p| json!({"name": p.name, "kind": format!("{:?}", p.kind), "faces": p.size})).collect::<Vec<_>>(),
        "bounds": bounds(&mesh),
        "two_d": mesh.patches().iter().any(|p| p.kind == PatchKind::Empty),
        "adapt": adapt,
        "reused": reused,
    }))
}
