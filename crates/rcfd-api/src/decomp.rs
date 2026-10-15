//! Partitioning through the case tree (F-236, ADR 0021 D2/D3): the `decomposition` section selects one of the
//! strategies of `rcfd_mesh::partition`, optional cell weights, smoothing and a bandwidth report of the reverse
//! Cuthill-McKee ordering.
//!
//! The solver runs in one process and ignores this section for its numerics; it is used by `rcfd partition`, the
//! service method `mesh.partition` and by `decomposition.write_field` (the part id as the cell array `partition` in
//! the VTK files). Nothing is renumbered: `reorder` only reports bandwidths.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::settings::Schema;
use rcfd_linalg::reorder::{bandwidth, rcm_order};
use rcfd_mesh::partition::{
    edge_cut, part_weights, smooth_partition, weighted_imbalance, Bisection, BisectionRule, Decomposition,
    GraphPartitioner, Partitioner, SectorKind, Sectors,
};
use rcfd_mesh::Mesh;
use serde::Serialize;

/// Method names accepted by `decomposition.method`.
pub const METHODS: [&str; 5] = ["none", "bisection", "inertial", "sectors", "graph"];

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn vec3(doc: &str) -> Schema {
    Schema::list(Schema::float(0.0)).doc(doc)
}

/// Schema of the `decomposition` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Mesh partitioning (ADR 0021). Used by `rcfd partition`, the service method mesh.partition and `write_field`; the solver itself runs in one process and ignores this section")
        .field(
            "method",
            Schema::enumeration("none", &METHODS).doc("none (inert), bisection (recursive coordinate bisection, see rule), inertial (bisection along the principal axis of the cell centres), sectors (radial or circumferential bands about an axis) or graph (bisection seed plus cut-reducing boundary refinement)"),
        )
        .field(
            "parts",
            Schema::int(1).range(1.0, 1e6).doc("Number of parts"),
        )
        .field(
            "rule",
            Schema::enumeration("longest", &["longest", "cycle", "x", "y", "z"]).doc("bisection: cutting direction: longest extent of the sub-set, axes in turn, or a fixed axis"),
        )
        .field(
            "sector_kind",
            Schema::enumeration("circumferential", &["circumferential", "radial"]).doc("sectors: bands of equal weight in the angle about the axis (circumferential) or in the distance from it (radial)"),
        )
        .field("origin", vec3("sectors: a point [m] on the axis (default [0, 0, 0])"))
        .field("axis", vec3("sectors: axis direction (default [0, 0, 1])"))
        .field(
            "reference",
            vec3("sectors, circumferential: direction from which the angle is measured (default [1, 0, 0]); must not be parallel to the axis"),
        )
        .field(
            "weights",
            Schema::object()
                .doc("Optional cell weights")
                .field(
                    "mode",
                    Schema::enumeration("none", &["none", "box"]).doc("none: unit weights; box: weight `inside` for cells whose centre lies in the closed box, `outside` elsewhere"),
                )
                .field("box_min", vec3("Lower corner [m] of the weight box"))
                .field("box_max", vec3("Upper corner [m] of the weight box"))
                .field(
                    "inside",
                    Schema::float(1.0).at_least(1e-300).doc("Cell weight inside the box"),
                )
                .field(
                    "outside",
                    Schema::float(1.0).at_least(1e-300).doc("Cell weight outside the box"),
                ),
        )
        .field(
            "tolerance",
            Schema::float(0.03).at_least(0.0).doc("graph and smooth: allowed excess of a part weight over the mean during refinement"),
        )
        .field(
            "max_passes",
            Schema::int(20).range(1.0, 1e6).doc("graph and smooth: maximum number of refinement passes"),
        )
        .field(
            "smooth",
            Schema::boolean(false).doc("Clean up the partition after the strategy (move cells to the part holding most of their neighbours when that lowers the edge cut)"),
        )
        .field(
            "reorder",
            Schema::boolean(false).doc("Report the matrix bandwidth of the natural and of the reverse Cuthill-McKee cell ordering. Report only: the mesh and the solvers are never renumbered"),
        )
        .field(
            "write_field",
            Schema::boolean(false).doc("rcfd run writes the part id as the cell array `partition` into the VTK files (needs method != none)"),
        )
}

/// The configured method (`none` when unset).
pub fn method(case: &Case) -> String {
    get_s(case, "decomposition.method")
}

/// Number of parts.
pub fn parts(case: &Case) -> usize {
    get_i(case, "decomposition.parts").max(1) as usize
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let m = method(case);
    let n = get_i(case, "decomposition.parts");
    if m == "none" {
        if n > 1 {
            out.push(d(
                "decomposition.method",
                format!("decomposition.parts = {n} needs a method (bisection, inertial, sectors or graph)"),
            ));
        }
        if get_b(case, "decomposition.write_field") {
            out.push(d(
                "decomposition.write_field",
                "writing the partition field needs decomposition.method other than none",
            ));
        }
    }
    if m == "sectors" {
        let axis = get_v(case, "decomposition.axis", [0.0, 0.0, 1.0]);
        let norm = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if norm(axis) < 1e-300 {
            out.push(d("decomposition.axis", "the axis direction must not be zero"));
        } else {
            let r = get_v(case, "decomposition.reference", [1.0, 0.0, 0.0]);
            let cr = [
                axis[1] * r[2] - axis[2] * r[1],
                axis[2] * r[0] - axis[0] * r[2],
                axis[0] * r[1] - axis[1] * r[0],
            ];
            if norm(cr) <= 1e-12 * norm(axis) * norm(r).max(1e-300) {
                out.push(d(
                    "decomposition.reference",
                    "the reference direction is zero or parallel to the axis",
                ));
            }
        }
    }
    if get_s(case, "decomposition.weights.mode") == "box" {
        let lo = get_v(case, "decomposition.weights.box_min", [0.0; 3]);
        let hi = get_v(case, "decomposition.weights.box_max", [0.0; 3]);
        if (0..3).any(|k| lo[k] > hi[k]) {
            out.push(d(
                "decomposition.weights.box_max",
                "box_max must not be below box_min in any component",
            ));
        }
    }
    out
}

/// Mesh-dependent checks of the section (`parts` against the cell count).
pub fn check_mesh(case: &Case, mesh: &Mesh) -> Vec<Diagnostic> {
    if method(case) != "none" && parts(case) > mesh.n_cells() {
        return vec![d(
            "decomposition.parts",
            format!("{} parts for a mesh of {} cells", parts(case), mesh.n_cells()),
        )];
    }
    Vec::new()
}

/// Result of a partitioning (ADR 0021 D2).
#[derive(Debug, Clone, Serialize)]
pub struct PartitionReport {
    /// Method name.
    pub method: String,
    /// Number of parts.
    pub n_parts: usize,
    /// Number of cells.
    pub n_cells: usize,
    /// Cells per part.
    pub part_cells: Vec<usize>,
    /// Weight per part (equals the cell counts for unit weights).
    pub part_weights: Vec<f64>,
    /// `max W_p / (W / n) - 1`.
    pub imbalance: f64,
    /// Internal faces between cells of different parts.
    pub edge_cut: usize,
    /// Ghost (halo) cells per part.
    pub ghost_cells: Vec<usize>,
    /// Cells moved by the smoothing (only when `smooth`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smoothed_moves: Option<usize>,
    /// Bandwidth of the natural and the reverse Cuthill-McKee ordering (only when `reorder`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<Bandwidths>,
    /// Part of every cell.
    pub part: Vec<usize>,
}

/// Matrix bandwidths of the cell ordering.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Bandwidths {
    /// Bandwidth of the mesh numbering.
    pub natural: usize,
    /// Bandwidth after reverse Cuthill-McKee.
    pub rcm: usize,
}

/// Cell weights from `decomposition.weights` (`None` for unit weights).
pub fn cell_weights(case: &Case, mesh: &Mesh) -> Option<Vec<f64>> {
    if get_s(case, "decomposition.weights.mode") != "box" {
        return None;
    }
    let lo = get_v(case, "decomposition.weights.box_min", [0.0; 3]);
    let hi = get_v(case, "decomposition.weights.box_max", [0.0; 3]);
    let (win, wout) = (
        get_f(case, "decomposition.weights.inside"),
        get_f(case, "decomposition.weights.outside"),
    );
    Some(
        mesh.cell_centres()
            .iter()
            .map(|c| {
                if (0..3).all(|k| c[k] >= lo[k] && c[k] <= hi[k]) {
                    win
                } else {
                    wout
                }
            })
            .collect(),
    )
}

/// The part of every cell as the configured strategy gives it, before smoothing.
pub fn assign(case: &Case, mesh: &Mesh) -> Result<Vec<usize>, Vec<Diagnostic>> {
    let m = method(case);
    let n = parts(case);
    if m == "none" {
        return Err(vec![d(
            "decomposition.method",
            "no partitioning method is set (bisection, inertial, sectors or graph)",
        )]);
    }
    if n > mesh.n_cells() {
        return Err(vec![d(
            "decomposition.parts",
            format!("{n} parts for a mesh of {} cells", mesh.n_cells()),
        )]);
    }
    let weights = cell_weights(case, mesh);
    let tolerance = get_f(case, "decomposition.tolerance");
    let max_passes = get_i(case, "decomposition.max_passes").max(1) as usize;
    let res = match m.as_str() {
        "bisection" => {
            let rule = match get_s(case, "decomposition.rule").as_str() {
                "cycle" => BisectionRule::Cycle,
                "x" => BisectionRule::Fixed(0),
                "y" => BisectionRule::Fixed(1),
                "z" => BisectionRule::Fixed(2),
                _ => BisectionRule::Longest,
            };
            Bisection { rule, weights }.partition(mesh, n)
        }
        "inertial" => Bisection {
            rule: BisectionRule::Inertial,
            weights,
        }
        .partition(mesh, n),
        "sectors" => Sectors {
            kind: if get_s(case, "decomposition.sector_kind") == "radial" {
                SectorKind::Radial
            } else {
                SectorKind::Circumferential
            },
            origin: get_v(case, "decomposition.origin", [0.0; 3]),
            axis: get_v(case, "decomposition.axis", [0.0, 0.0, 1.0]),
            reference: get_v(case, "decomposition.reference", [1.0, 0.0, 0.0]),
            weights,
        }
        .partition(mesh, n),
        "graph" => GraphPartitioner {
            weights,
            tolerance,
            max_passes,
        }
        .partition(mesh, n),
        other => return Err(vec![d("decomposition.method", format!("unknown method '{other}'"))]),
    };
    res.map_err(|e| vec![d("decomposition", e.to_string())])
}

/// Partition the mesh as configured (strategy, optional smoothing) and report.
pub fn run(case: &Case, mesh: &Mesh) -> Result<PartitionReport, Vec<Diagnostic>> {
    let mut part = assign(case, mesh)?;
    let n = parts(case);
    let weights = cell_weights(case, mesh);
    let mut smoothed_moves = None;
    if get_b(case, "decomposition.smooth") {
        let r = smooth_partition(
            mesh,
            &mut part,
            n,
            weights.as_deref(),
            get_f(case, "decomposition.tolerance"),
            get_i(case, "decomposition.max_passes").max(1) as usize,
        )
        .map_err(|e| vec![d("decomposition.smooth", e.to_string())])?;
        smoothed_moves = Some(r.moved);
    }
    let dec =
        Decomposition::from_assignment(mesh, n, part.clone()).map_err(|e| vec![d("decomposition", e.to_string())])?;
    let mut part_cells = vec![0usize; n];
    for &p in &part {
        part_cells[p] += 1;
    }
    let bw = get_b(case, "decomposition.reorder").then(|| {
        let nf = mesh.n_internal_faces();
        let (lower, upper) = (&mesh.owner()[..nf], mesh.neighbour());
        let order = rcm_order(mesh.n_cells(), lower, upper);
        Bandwidths {
            natural: bandwidth(lower, upper, None),
            rcm: bandwidth(lower, upper, Some(&order)),
        }
    });
    Ok(PartitionReport {
        method: method(case),
        n_parts: n,
        n_cells: mesh.n_cells(),
        part_cells,
        part_weights: part_weights(&part, n, weights.as_deref()),
        imbalance: weighted_imbalance(&part, n, weights.as_deref()),
        edge_cut: edge_cut(mesh, &part),
        ghost_cells: dec.subdomains.iter().map(|s| s.n_local() - s.n_owned()).collect(),
        smoothed_moves,
        bandwidth: bw,
        part,
    })
}

/// Keys of `decomposition` that service and CLI overrides may set.
pub const OVERRIDE_KEYS: [&str; 8] = [
    "method",
    "parts",
    "rule",
    "sector_kind",
    "tolerance",
    "max_passes",
    "smooth",
    "reorder",
];

/// A copy of `case` with `decomposition.<key>` overridden and the whole case re-validated.
pub fn with_overrides(case: &Case, overrides: &[(String, serde_json::Value)]) -> Result<Case, Vec<Diagnostic>> {
    let mut c = case.clone();
    for (k, v) in overrides {
        let path = format!("decomposition.{k}");
        c.tree
            .set(&path, v.clone())
            .map_err(|e| vec![d(&path, e.to_string())])?;
    }
    let diags = c.semantic_errors();
    if diags.is_empty() {
        Ok(c)
    } else {
        Err(diags)
    }
}

/// Part ids as floating-point cell values (the `partition` VTK array).
pub fn part_field(report: &PartitionReport) -> Vec<f64> {
    report.part.iter().map(|&p| p as f64).collect()
}

/// Report as JSON; the `part` vector is included only when `with_parts`.
pub fn report_json(report: &PartitionReport, with_parts: bool) -> serde_json::Value {
    let mut v = serde_json::to_value(report).unwrap_or(serde_json::Value::Null);
    if !with_parts {
        if let Some(o) = v.as_object_mut() {
            o.remove("part");
        }
    }
    v
}
