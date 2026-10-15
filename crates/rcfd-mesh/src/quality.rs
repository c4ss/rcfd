//! Mesh quality metrics and the check report (F-011).
//!
//! Definitions (standard FV mesh-quality measures):
//! * **non-orthogonality** of a face: angle in degrees between the face area
//!   vector `Sf` and the vector `d` joining the two adjacent cell centres
//!   (owner centre to face centre on boundaries);
//! * **skewness** of a face: distance between the face centre and the point
//!   where the line of centres crosses the face plane, divided by `|d|`;
//! * **aspect ratio** of a cell: longest edge over shortest edge among the
//!   edges of its faces;
//! * **volume ratio** of an internal face: larger over smaller adjacent cell volume.
//!
//! The extended catalogue (warp, face angles, ortho-skew, inverse orthogonal quality, size change,
//! face edge-length ratio) lives in [`crate::measures`]; its summary and the poor-mesh list are
//! part of [`QualityReport`] (ADR 0018). They never affect `is_ok`, `warnings` or `errors`.

use crate::measures::{compute_measures, REPORT_INF};
use crate::mesh::Mesh;
use rcfd_core::vec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-element metric arrays.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Non-orthogonality angle per face, degrees.
    pub non_orthogonality: Vec<f64>,
    /// Skewness per face.
    pub skewness: Vec<f64>,
    /// Aspect ratio per cell.
    pub aspect_ratio: Vec<f64>,
    /// Volume ratio per internal face.
    pub volume_ratio: Vec<f64>,
}

/// Limits above which a mesh is flagged.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// Maximum acceptable non-orthogonality, degrees (warning).
    pub non_orthogonality_warn: f64,
    /// Non-orthogonality above which the mesh is rejected, degrees.
    pub non_orthogonality_error: f64,
    /// Maximum acceptable skewness.
    pub skewness_warn: f64,
    /// Maximum acceptable aspect ratio.
    pub aspect_ratio_warn: f64,
    /// Maximum acceptable volume ratio.
    pub volume_ratio_warn: f64,
    /// Face warp above which a face is listed as poor, degrees.
    pub warp_warn: f64,
    /// Smallest face angle below which a cell is listed as poor, degrees.
    pub face_angle_min_warn: f64,
    /// Largest face angle above which a cell is listed as poor, degrees.
    pub face_angle_max_warn: f64,
    /// Inverse orthogonal quality above which a cell is listed as poor.
    pub inverse_orthogonal_warn: f64,
    /// Ortho-skew above which a cell is listed as poor.
    pub ortho_skew_warn: f64,
    /// Face edge-length ratio above which a face is listed as poor.
    pub edge_length_ratio_warn: f64,
    /// Largest number of offenders listed per measure.
    pub poor_list_cap: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            non_orthogonality_warn: 70.0,
            non_orthogonality_error: 85.0,
            skewness_warn: 4.0,
            aspect_ratio_warn: 1000.0,
            volume_ratio_warn: 100.0,
            warp_warn: 15.0,
            face_angle_min_warn: 20.0,
            face_angle_max_warn: 160.0,
            inverse_orthogonal_warn: 0.9,
            ortho_skew_warn: 0.85,
            edge_length_ratio_warn: 100.0,
            poor_list_cap: 10,
        }
    }
}

/// Extremes of the extended measures (ADR 0018). Infinity is stored as `1e30`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MeasureSummary {
    /// Largest face warp, degrees.
    pub max_warp: f64,
    /// Smallest interior face angle, degrees.
    pub min_face_angle: f64,
    /// Largest interior face angle, degrees.
    pub max_face_angle: f64,
    /// Largest ortho-skew.
    pub max_ortho_skew: f64,
    /// Largest inverse orthogonal quality.
    pub max_inverse_orthogonal: f64,
    /// Largest size change.
    pub max_size_change: f64,
    /// Largest face edge-length ratio.
    pub max_edge_length_ratio: f64,
}

/// One entry of the poor-mesh list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoorEntry {
    /// Measure name (`warp`, `face_angle_min`, `face_angle_max`, `ortho_skew`, `inverse_orthogonal`,
    /// `size_change`, `edge_length_ratio`).
    pub measure: String,
    /// `"face"` or `"cell"`.
    pub element: String,
    /// Face or cell index.
    pub index: usize,
    /// Measured value.
    pub value: f64,
    /// The limit that was passed.
    pub limit: f64,
}

/// Serializable summary of a mesh check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityReport {
    /// Number of cells.
    pub n_cells: usize,
    /// Number of faces.
    pub n_faces: usize,
    /// Number of internal faces.
    pub n_internal_faces: usize,
    /// Number of boundary patches.
    pub n_patches: usize,
    /// Smallest cell volume.
    pub min_volume: f64,
    /// Largest cell volume.
    pub max_volume: f64,
    /// Total volume.
    pub total_volume: f64,
    /// Maximum non-orthogonality (deg).
    pub max_non_orthogonality: f64,
    /// Mean non-orthogonality (deg).
    pub mean_non_orthogonality: f64,
    /// Maximum skewness.
    pub max_skewness: f64,
    /// Maximum aspect ratio.
    pub max_aspect_ratio: f64,
    /// Maximum volume ratio across internal faces (1 if none).
    pub max_volume_ratio: f64,
    /// Human-readable warnings.
    pub warnings: Vec<String>,
    /// Human-readable errors (mesh should not be used).
    pub errors: Vec<String>,
    /// Extremes of the extended measures.
    #[serde(default)]
    pub measures: MeasureSummary,
    /// Number of offenders per measure (only measures with at least one).
    #[serde(default)]
    pub poor_counts: BTreeMap<String, usize>,
    /// Worst offenders per measure, worst first (capped per measure).
    #[serde(default)]
    pub poor_list: Vec<PoorEntry>,
    /// One line per measure that has offenders (separate from `warnings`).
    #[serde(default)]
    pub poor_warnings: Vec<String>,
}

impl QualityReport {
    /// Whether no errors were found.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Compute all per-element metrics.
pub fn compute_metrics(mesh: &Mesh) -> Metrics {
    let ni = mesh.n_internal_faces();
    let nf = mesh.n_faces();
    let cc = mesh.cell_centres();
    let fcs = mesh.face_centres();
    let sfs = mesh.face_area_vectors();
    let vols = mesh.cell_volumes();
    let mut non_orth = Vec::with_capacity(nf);
    let mut skew = Vec::with_capacity(nf);
    let mut vr = Vec::with_capacity(ni);
    for f in 0..nf {
        let o = mesh.owner()[f];
        let n_hat = vec3::unit(sfs[f]);
        let (d, base) = if f < ni {
            let nb = mesh.neighbour()[f];
            (vec3::sub(cc[nb], cc[o]), cc[o])
        } else {
            (vec3::sub(fcs[f], cc[o]), cc[o])
        };
        let dm = vec3::norm(d);
        let cos = if dm > 0.0 {
            (vec3::dot(d, n_hat) / dm).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        non_orth.push(cos.acos().to_degrees());
        let dn = vec3::dot(d, n_hat);
        let fi = if dn.abs() > 0.0 {
            vec3::add(base, vec3::scale(d, vec3::dot(vec3::sub(fcs[f], base), n_hat) / dn))
        } else {
            base
        };
        skew.push(if dm > 0.0 {
            vec3::norm(vec3::sub(fcs[f], fi)) / dm
        } else {
            0.0
        });
        if f < ni {
            let nb = mesh.neighbour()[f];
            let (a, b) = (vols[o], vols[nb]);
            vr.push(a.max(b) / a.min(b));
        }
    }
    let mut ar = Vec::with_capacity(mesh.n_cells());
    let pts = mesh.points();
    for c in 0..mesh.n_cells() {
        let mut lo = f64::INFINITY;
        let mut hi = 0.0f64;
        for &f in mesh.cell_faces(c) {
            let ids = mesh.face_points(f);
            for i in 0..ids.len() {
                let l = vec3::norm(vec3::sub(pts[ids[(i + 1) % ids.len()]], pts[ids[i]]));
                lo = lo.min(l);
                hi = hi.max(l);
            }
        }
        ar.push(if lo > 0.0 { hi / lo } else { f64::INFINITY });
    }
    Metrics {
        non_orthogonality: non_orth,
        skewness: skew,
        aspect_ratio: ar,
        volume_ratio: vr,
    }
}

/// Run the mesh check with default thresholds.
pub fn check_mesh(mesh: &Mesh) -> QualityReport {
    check_mesh_with(mesh, &Thresholds::default())
}

/// Run the mesh check with explicit thresholds.
pub fn check_mesh_with(mesh: &Mesh, th: &Thresholds) -> QualityReport {
    let m = compute_metrics(mesh);
    let max = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
    let vols = mesh.cell_volumes();
    let mut r = QualityReport {
        n_cells: mesh.n_cells(),
        n_faces: mesh.n_faces(),
        n_internal_faces: mesh.n_internal_faces(),
        n_patches: mesh.patches().len(),
        min_volume: vols.iter().copied().fold(f64::INFINITY, f64::min),
        max_volume: max(vols),
        total_volume: mesh.total_volume(),
        max_non_orthogonality: max(&m.non_orthogonality),
        mean_non_orthogonality: m.non_orthogonality.iter().sum::<f64>() / m.non_orthogonality.len().max(1) as f64,
        max_skewness: max(&m.skewness),
        max_aspect_ratio: max(&m.aspect_ratio),
        max_volume_ratio: m.volume_ratio.iter().copied().fold(1.0, f64::max),
        warnings: vec![],
        errors: vec![],
        measures: MeasureSummary::default(),
        poor_counts: BTreeMap::new(),
        poor_list: vec![],
        poor_warnings: vec![],
    };
    poor_mesh_report(mesh, th, &mut r);
    if let Err(e) = mesh.validate() {
        r.errors.push(e.to_string());
    }
    if r.max_non_orthogonality > th.non_orthogonality_error {
        r.errors.push(format!(
            "max non-orthogonality {:.1} deg exceeds {:.1}",
            r.max_non_orthogonality, th.non_orthogonality_error
        ));
    } else if r.max_non_orthogonality > th.non_orthogonality_warn {
        let n = m
            .non_orthogonality
            .iter()
            .filter(|&&a| a > th.non_orthogonality_warn)
            .count();
        r.warnings.push(format!(
            "{n} faces with non-orthogonality above {:.0} deg (max {:.1})",
            th.non_orthogonality_warn, r.max_non_orthogonality
        ));
    }
    if r.max_skewness > th.skewness_warn {
        r.warnings.push(format!(
            "max skewness {:.2} exceeds {:.1}",
            r.max_skewness, th.skewness_warn
        ));
    }
    if r.max_aspect_ratio > th.aspect_ratio_warn {
        r.warnings.push(format!(
            "max aspect ratio {:.0} exceeds {:.0}",
            r.max_aspect_ratio, th.aspect_ratio_warn
        ));
    }
    if r.max_volume_ratio > th.volume_ratio_warn {
        r.warnings.push(format!(
            "max volume ratio {:.1} exceeds {:.0}",
            r.max_volume_ratio, th.volume_ratio_warn
        ));
    }
    r
}

fn cap(v: f64) -> f64 {
    if v.is_finite() {
        v.min(REPORT_INF)
    } else {
        REPORT_INF
    }
}

/// Fill the extended-measure summary and the poor-mesh list of `r` (read-only on the mesh).
fn poor_mesh_report(mesh: &Mesh, th: &Thresholds, r: &mut QualityReport) {
    let m = compute_measures(mesh);
    let mx = |v: &[f64]| cap(v.iter().copied().fold(0.0, f64::max));
    r.measures = MeasureSummary {
        max_warp: mx(&m.face_warp),
        min_face_angle: m
            .cell_face_angle_min
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
            .min(REPORT_INF),
        max_face_angle: mx(&m.cell_face_angle_max),
        max_ortho_skew: mx(&m.cell_ortho_skew),
        max_inverse_orthogonal: mx(&m.cell_inverse_orthogonal),
        max_size_change: mx(&m.cell_size_change),
        max_edge_length_ratio: mx(&m.face_edge_ratio),
    };
    // (name, element, values, limit, limit is an upper bound?)
    let specs: [(&str, &str, &[f64], f64, bool); 7] = [
        ("warp", "face", &m.face_warp, th.warp_warn, true),
        (
            "face_angle_min",
            "cell",
            &m.cell_face_angle_min,
            th.face_angle_min_warn,
            false,
        ),
        (
            "face_angle_max",
            "cell",
            &m.cell_face_angle_max,
            th.face_angle_max_warn,
            true,
        ),
        ("ortho_skew", "cell", &m.cell_ortho_skew, th.ortho_skew_warn, true),
        (
            "inverse_orthogonal",
            "cell",
            &m.cell_inverse_orthogonal,
            th.inverse_orthogonal_warn,
            true,
        ),
        ("size_change", "cell", &m.cell_size_change, th.volume_ratio_warn, true),
        (
            "edge_length_ratio",
            "face",
            &m.face_edge_ratio,
            th.edge_length_ratio_warn,
            true,
        ),
    ];
    for (name, element, values, limit, upper) in specs {
        let mut bad: Vec<(f64, usize)> = values
            .iter()
            .enumerate()
            .filter(|&(_, &v)| if upper { v > limit } else { v < limit })
            .map(|(i, &v)| {
                let sev = if upper {
                    v / limit
                } else if v > 0.0 {
                    limit / v
                } else {
                    f64::INFINITY
                };
                (sev, i)
            })
            .collect();
        if bad.is_empty() {
            continue;
        }
        bad.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        r.poor_counts.insert(name.to_string(), bad.len());
        r.poor_warnings.push(format!(
            "{} {element}s with {name} {} {limit} (worst {:.4} at {element} {})",
            bad.len(),
            if upper { "above" } else { "below" },
            cap(values[bad[0].1]),
            bad[0].1
        ));
        for &(_, i) in bad.iter().take(th.poor_list_cap) {
            r.poor_list.push(PoorEntry {
                measure: name.to_string(),
                element: element.to_string(),
                index: i,
                value: cap(values[i]),
                limit,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{box_mesh, Grading};

    #[test]
    fn uniform_cube_grid_is_perfect() {
        let m = box_mesh([4, 3, 2], [4.0, 3.0, 2.0], [Grading::Uniform; 3]).unwrap();
        let r = check_mesh(&m);
        assert!(r.is_ok() && r.warnings.is_empty(), "{r:?}");
        assert!(r.max_non_orthogonality < 1e-6);
        assert!(r.max_skewness < 1e-12);
        assert!((r.max_aspect_ratio - 1.0).abs() < 1e-12);
        assert!((r.max_volume_ratio - 1.0).abs() < 1e-12);
        assert!((r.total_volume - 24.0).abs() < 1e-12);
    }

    #[test]
    fn hand_computed_anisotropic_and_graded() {
        // cells 1 x 0.1 x 0.1 -> aspect ratio 10
        let m = box_mesh([2, 1, 1], [2.0, 0.1, 0.1], [Grading::Uniform; 3]).unwrap();
        let r = check_mesh(&m);
        assert!((r.max_aspect_ratio - 10.0).abs() < 1e-9);
        // grading with expansion ratio 4 in x with 2 cells -> sizes 0.2 and 0.8 -> volume ratio 4
        let m = box_mesh(
            [2, 1, 1],
            [1.0, 1.0, 1.0],
            [Grading::Expansion(4.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap();
        let r = check_mesh(&m);
        assert!((r.max_volume_ratio - 4.0).abs() < 1e-9, "{}", r.max_volume_ratio);
    }

    #[test]
    fn sheared_mesh_non_orthogonality_and_skewness() {
        // Shear the second layer of cells in x by s=1 over height 1 -> face between rows is
        // horizontal, centres offset by (1, 1): d at 45 deg to Sf (normal +y).
        let mut m = box_mesh([1, 2, 1], [1.0, 2.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let mut topo = m.topology().clone();
        for p in topo.points.iter_mut() {
            if p[1] > 1.5 {
                p[0] += 1.0;
            } else if p[1] > 0.5 {
                p[0] += 0.5; // middle layer shifted half so that centres are offset by 0.75? keep deterministic below
            }
        }
        m = Mesh::from_topology(topo).unwrap();
        let met = compute_metrics(&m);
        // Face between the two cells is the plane y=1 with points shifted x+0.5: centre (1.0,1,0.5).
        // Cell 0 spans y in [0,1]: bottom at x in [0,1], top at x in [0.5,1.5]: centroid x = 0.5+0.25=0.75, y=0.5
        // Cell 1 spans y in [1,2]: bottom x in [0.5,1.5], top x in [1,2]: centroid x = 1.25, y=1.5
        // d = (0.5, 1, 0), Sf is +-y -> angle = atan(0.5)
        let expected = (0.5f64).atan().to_degrees();
        let f = 0; // single internal face
        assert!(
            (met.non_orthogonality[f] - expected).abs() < 1e-9,
            "{} vs {expected}",
            met.non_orthogonality[f]
        );
        // line of centres crosses y=1 at x = 0.75 + 0.5*0.5 = 1.0 = face centre x -> zero skewness
        assert!(met.skewness[f].abs() < 1e-9);
    }

    #[test]
    fn report_serializes_and_flags_bad_meshes() {
        let m = box_mesh([2, 2, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let r = check_mesh(&m);
        let json = serde_json::to_string(&r).unwrap();
        let back: QualityReport = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
        // strict thresholds flag the aspect ratio and the volume ratio
        let m = box_mesh(
            [2, 1, 1],
            [1.0, 0.01, 1.0],
            [Grading::Expansion(500.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap();
        let r = check_mesh_with(
            &m,
            &Thresholds {
                aspect_ratio_warn: 10.0,
                volume_ratio_warn: 10.0,
                ..Thresholds::default()
            },
        );
        assert_eq!(r.warnings.len(), 2, "{:?}", r.warnings);
        assert!(r.is_ok());
    }
}
