//! Refine by 2 (ADR 0737 P3, F-837 E-837b; the egui Studio's Refine x2 of ADR 0148): every cell count of a generator
//! mesh doubled, as setting edits. A mesh file is not refined (Q3): splitting the cells of an arbitrary mesh would be a
//! mesher feature of its own. Only the factor 2 is offered (Q4). Only reads: nothing is set or written here.

use crate::case::{Case, Diagnostic};
use rcfd_core::settings::Kind;
use serde_json::{json, Value};

/// The factors [`count_refinement`] takes (ADR 0737 Q4 (a)).
pub const FACTORS: [u64; 1] = [2];

/// The count settings (under `mesh.`) of a generator whose cells are products, or sums of products, of its counts: 2D
/// generators count x and y (theta and radial for `cylinder_ogrid`), 3D ones also z; a second block's `_right` counts
/// follow (0 means "the same as the first block" and is left as it is). `None` for a generator counted otherwise
/// (`axial_fan_sector`: `mesh.fan.n_*`).
pub fn count_keys(generator: &str) -> Option<&'static [&'static str]> {
    Some(match generator {
        "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" | "cylinder_ogrid" => &["nx", "ny"],
        "channel_2d_interface" => &["nx", "ny", "nx_right", "ny_right"],
        "annulus_3d" | "annulus_3d_sector" | "pipe_sweep" | "box_3d" => &["nx", "ny", "nz"],
        "annulus_3d_interface" => &["nx", "ny", "nz", "ny_right", "nz_right"],
        "annulus_3d_stage" | "annulus_3d_cylindrical_interface" | "annulus_3d_sector_cylindrical_interface" => {
            &["nx", "ny", "nz", "nx_right", "ny_right", "nz_right"]
        }
        _ => return None,
    })
}

/// One edit of a refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Settings path (`mesh.nx`).
    pub path: String,
    /// The count now.
    pub before: u64,
    /// The count after.
    pub after: u64,
}

/// The edits of a refinement and the cells before and after.
#[derive(Debug, Clone, PartialEq)]
pub struct Refinement {
    /// The factor.
    pub factor: u64,
    /// The count edits, in the order of [`count_keys`].
    pub edits: Vec<Edit>,
    /// Cells of the mesh now.
    pub cells: u64,
    /// Cells after (`factor^2` or `factor^3` times as many).
    pub cells_after: u64,
    /// A 2D generator.
    pub two_d: bool,
}

/// Why a case is not refined.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A factor other than [`FACTORS`].
    Factor(String),
    /// The mesh is a file: the reason (error 1009 `MESH_FILE` of the service and the project tools; ADR 0737, change of
    /// 2026-11-17).
    MeshFile(String),
    /// A generator counted otherwise, a count would pass its maximum, or the mesh is over the cell limit: the reason.
    Refused(String),
    /// The case does not build.
    Invalid(Vec<Diagnostic>),
}

impl Refinement {
    /// As JSON (`mesh.refine`, `refine_case_mesh`, `rcfd check --refine`).
    pub fn to_json(&self) -> Value {
        json!({
            "factor": self.factor,
            "edits": self.edits.iter().map(|e| json!({"path": e.path, "before": e.before, "after": e.after})).collect::<Vec<_>>(),
            "cells": self.cells,
            "cells_after": self.cells_after,
            "two_d": self.two_d,
        })
    }

    /// The edits as `{path: after}`.
    pub fn set(&self) -> serde_json::Map<String, Value> {
        self.edits.iter().map(|e| (e.path.clone(), json!(e.after))).collect()
    }
}

/// The refinement of `case`'s generator mesh by `factor` (2 only), or the reason it is refused. The mesh is built once
/// for its cell count (`max_cells`, when given, refuses a larger one before anything else).
pub fn count_refinement(case: &Case, factor: u64, max_cells: Option<u64>) -> Result<Refinement, Refusal> {
    if !FACTORS.contains(&factor) {
        return Err(Refusal::Factor(format!(
            "factor {factor}: only 2 is offered (Refine x2)"
        )));
    }
    let s = |p: &str| {
        case.get(p)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    };
    if s("mesh.source") == "file" {
        return Err(Refusal::MeshFile(format!(
            "the resolution is set by the mesh file {}; a mesh file is not refined (its cells would have to be split, \
             which the engine does not do): refine it in the program that made it",
            s("mesh.file")
        )));
    }
    let generator = s("mesh.generator");
    let Some(keys) = count_keys(&generator) else {
        return Err(Refusal::Refused(format!(
            "generator \"{generator}\" counts its cells with other settings (mesh.fan.n_* for axial_fan_sector): edit \
             them in the case"
        )));
    };
    let mut edits = Vec::new();
    for k in keys {
        let path = format!("mesh.{k}");
        let before = case.get(&path).ok().and_then(|v| v.as_u64()).unwrap_or(0);
        if k.ends_with("_right") && before == 0 {
            continue;
        }
        let after = before.saturating_mul(factor);
        let max = case.tree.schema_at(&path).ok().and_then(|n| match n.kind {
            Kind::Int { max, .. } => max,
            _ => None,
        });
        if let Some(m) = max.filter(|m| after as i64 > *m || after > i64::MAX as u64) {
            return Err(Refusal::Refused(format!(
                "{path} would be {after}, more than its maximum {m}"
            )));
        }
        edits.push(Edit { path, before, after });
    }
    let diags = case.semantic_errors();
    if !diags.is_empty() {
        return Err(Refusal::Invalid(diags));
    }
    let mesh = crate::sim::build_mesh(case).map_err(Refusal::Invalid)?;
    let cells = mesh.n_cells() as u64;
    if let Some(l) = max_cells.filter(|l| cells > *l) {
        return Err(Refusal::Refused(format!(
            "the mesh has {cells} cells, more than the limit {l}"
        )));
    }
    let two_d = !keys.contains(&"nz");
    let cells_after = cells.saturating_mul(if two_d {
        factor * factor
    } else {
        factor * factor * factor
    });
    Ok(Refinement {
        factor,
        edits,
        cells,
        cells_after,
        two_d,
    })
}
