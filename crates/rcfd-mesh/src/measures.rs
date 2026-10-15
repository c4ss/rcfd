//! Extended mesh-quality measure catalogue (F-233, ADR 0018).
//!
//! Adds face warp, face angles, ortho-skew, inverse orthogonal quality, per-cell size change and
//! face edge-length ratio to the existing measures of [`crate::quality`]. The definitions are the
//! project definitions of ADR 0018 part A (closed forms for canonical shapes are derived there).
//!
//! The pure polygon and orthogonality functions take plain coordinates, so they can be evaluated on
//! degenerate inputs that are not valid mesh faces. Library functions return the true value,
//! including infinity; the report layer stores infinity as [`REPORT_INF`].

use crate::mesh::{polygon_geometry, Mesh};
use rcfd_core::vec3::{self, Vec3};

/// Value that stands for infinity in serialized reports (JSON has no infinity).
pub const REPORT_INF: f64 = 1e30;

/// Angle between two vectors in degrees, `atan2(|a x b|, a . b)`: well conditioned for nearly
/// parallel vectors (an `acos` form loses half the digits there). 0 if either vector vanishes.
fn angle_between(a: Vec3, b: Vec3) -> f64 {
    if vec3::norm(a) * vec3::norm(b) > 0.0 {
        vec3::norm(vec3::cross(a, b)).atan2(vec3::dot(a, b)).to_degrees()
    } else {
        0.0
    }
}

/// Remove consecutive coincident vertices, including a closing duplicate of the first vertex.
pub fn dedup_polygon(pts: &[Vec3]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(pts.len());
    for &p in pts {
        let keep = match out.last() {
            Some(&q) => vec3::norm(vec3::sub(p, q)) > 0.0,
            None => true,
        };
        if keep {
            out.push(p);
        }
    }
    while out.len() > 1 && vec3::norm(vec3::sub(out[0], out[out.len() - 1])) == 0.0 {
        out.pop();
    }
    out
}

/// Warp of a polygon in degrees (ADR 0018 A1.1): 0 for a triangle; for a quadrilateral the larger
/// of the two diagonal-split angles between the triangle normals; for `n >= 5` vertices the largest
/// angle between a fan-triangle normal (fan about the vertex average) and the area vector.
pub fn polygon_warp(pts: &[Vec3]) -> f64 {
    let p = dedup_polygon(pts);
    match p.len() {
        0..=3 => 0.0,
        4 => {
            let n = |a: Vec3, b: Vec3, c: Vec3| vec3::cross(vec3::sub(b, a), vec3::sub(c, a));
            let s1 = angle_between(n(p[0], p[1], p[2]), n(p[0], p[2], p[3]));
            let s2 = angle_between(n(p[0], p[1], p[3]), n(p[1], p[2], p[3]));
            s1.max(s2)
        }
        k => {
            let mut est = vec3::ZERO;
            for &q in &p {
                est = vec3::add(est, q);
            }
            est = vec3::scale(est, 1.0 / k as f64);
            let tri: Vec<Vec3> = (0..k)
                .map(|i| vec3::cross(vec3::sub(p[(i + 1) % k], p[i]), vec3::sub(est, p[i])))
                .collect();
            let mut sum = vec3::ZERO;
            for t in &tri {
                sum = vec3::add(sum, *t);
            }
            tri.iter().map(|&t| angle_between(t, sum)).fold(0.0, f64::max)
        }
    }
}

/// Smallest and largest interior angle of a polygon in degrees (ADR 0018 A1.2).
pub fn polygon_angles(pts: &[Vec3]) -> (f64, f64) {
    let p = dedup_polygon(pts);
    let k = p.len();
    if k < 3 {
        return (0.0, 180.0);
    }
    let ids: Vec<usize> = (0..k).collect();
    let (_, sf) = polygon_geometry(&p, &ids);
    let nn = vec3::norm(sf);
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in 0..k {
        let a = vec3::sub(p[(i + 1) % k], p[i]);
        let b = vec3::sub(p[(i + k - 1) % k], p[i]);
        let th = if nn > 0.0 {
            let n = vec3::scale(sf, 1.0 / nn);
            let mut t = vec3::dot(n, vec3::cross(a, b)).atan2(vec3::dot(a, b)).to_degrees();
            if t < 0.0 {
                t += 360.0;
            }
            t
        } else {
            angle_between(a, b)
        };
        lo = lo.min(th);
        hi = hi.max(th);
    }
    (lo, hi)
}

/// Longest over shortest edge of the polygon loop as given (a collapsed edge gives infinity).
pub fn polygon_edge_ratio(pts: &[Vec3]) -> f64 {
    let k = pts.len();
    let mut lo = f64::INFINITY;
    let mut hi = 0.0f64;
    for i in 0..k {
        let l = vec3::norm(vec3::sub(pts[(i + 1) % k], pts[i]));
        lo = lo.min(l);
        hi = hi.max(l);
    }
    if lo > 0.0 {
        hi / lo
    } else {
        f64::INFINITY
    }
}

fn cosine(s: Vec3, v: Vec3) -> f64 {
    let d = vec3::norm(s) * vec3::norm(v);
    if d > 0.0 {
        (vec3::dot(s, v) / d).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// One face as seen from a cell: face centre, area vector pointing out of the cell, and the
/// centre of the neighbour cell for an internal face.
#[derive(Debug, Clone, Copy)]
pub struct CellFace {
    /// Face centre.
    pub centre: Vec3,
    /// Area vector oriented out of the cell.
    pub area_out: Vec3,
    /// Neighbour cell centre (internal faces only).
    pub neighbour_centre: Option<Vec3>,
}

/// `(ortho_skew, inverse_orthogonal_quality)` of a cell (ADR 0018 A1.3 and A1.4).
pub fn cell_orthogonality(centre: Vec3, faces: &[CellFace]) -> (f64, f64) {
    let mut min_alpha = 1.0f64;
    let mut min_all = 1.0f64;
    for f in faces {
        let a = cosine(f.area_out, vec3::sub(f.centre, centre));
        min_alpha = min_alpha.min(a);
        min_all = min_all.min(a);
        if let Some(n) = f.neighbour_centre {
            min_all = min_all.min(cosine(f.area_out, vec3::sub(n, centre)));
        }
    }
    (1.0 - min_alpha, 1.0 - min_all)
}

/// Per-face and per-cell measure arrays.
#[derive(Debug, Clone)]
pub struct Measures {
    /// Warp per face, degrees.
    pub face_warp: Vec<f64>,
    /// Edge-length ratio per face.
    pub face_edge_ratio: Vec<f64>,
    /// Maximum face warp per cell, degrees.
    pub cell_warp: Vec<f64>,
    /// Smallest interior face angle per cell, degrees.
    pub cell_face_angle_min: Vec<f64>,
    /// Largest interior face angle per cell, degrees.
    pub cell_face_angle_max: Vec<f64>,
    /// Ortho-skew per cell.
    pub cell_ortho_skew: Vec<f64>,
    /// Inverse orthogonal quality per cell.
    pub cell_inverse_orthogonal: Vec<f64>,
    /// Size change per cell (largest neighbour-to-cell volume ratio, 1 without neighbours).
    pub cell_size_change: Vec<f64>,
    /// Largest face edge-length ratio per cell.
    pub cell_edge_length_ratio: Vec<f64>,
}

/// Evaluate the extended measures on a mesh.
pub fn compute_measures(mesh: &Mesh) -> Measures {
    let nf = mesh.n_faces();
    let ni = mesh.n_internal_faces();
    let nc = mesh.n_cells();
    let pts = mesh.points();
    let mut face_warp = Vec::with_capacity(nf);
    let mut face_edge_ratio = Vec::with_capacity(nf);
    let mut face_amin = Vec::with_capacity(nf);
    let mut face_amax = Vec::with_capacity(nf);
    for f in 0..nf {
        let poly: Vec<Vec3> = mesh.face_points(f).iter().map(|&i| pts[i]).collect();
        face_warp.push(polygon_warp(&poly));
        face_edge_ratio.push(polygon_edge_ratio(&poly));
        let (lo, hi) = polygon_angles(&poly);
        face_amin.push(lo);
        face_amax.push(hi);
    }
    let cc = mesh.cell_centres();
    let vols = mesh.cell_volumes();
    let mut m = Measures {
        face_warp,
        face_edge_ratio,
        cell_warp: vec![0.0; nc],
        cell_face_angle_min: vec![f64::INFINITY; nc],
        cell_face_angle_max: vec![0.0; nc],
        cell_ortho_skew: vec![0.0; nc],
        cell_inverse_orthogonal: vec![0.0; nc],
        cell_size_change: vec![1.0; nc],
        cell_edge_length_ratio: vec![1.0; nc],
    };
    let mut grow = vec![f64::NEG_INFINITY; nc];
    for c in 0..nc {
        let mut faces = Vec::new();
        for &f in mesh.cell_faces(c) {
            let is_owner = mesh.owner()[f] == c;
            let sgn = if is_owner { 1.0 } else { -1.0 };
            let nb = if f < ni {
                Some(cc[if is_owner { mesh.neighbour()[f] } else { mesh.owner()[f] }])
            } else {
                None
            };
            faces.push(CellFace {
                centre: mesh.face_centres()[f],
                area_out: vec3::scale(mesh.face_area_vectors()[f], sgn),
                neighbour_centre: nb,
            });
            m.cell_warp[c] = m.cell_warp[c].max(m.face_warp[f]);
            m.cell_face_angle_min[c] = m.cell_face_angle_min[c].min(face_amin[f]);
            m.cell_face_angle_max[c] = m.cell_face_angle_max[c].max(face_amax[f]);
            m.cell_edge_length_ratio[c] = m.cell_edge_length_ratio[c].max(m.face_edge_ratio[f]);
            if let Some(nbc) = nb.map(|_| if is_owner { mesh.neighbour()[f] } else { mesh.owner()[f] }) {
                grow[c] = grow[c].max(vols[nbc] / vols[c]);
            }
        }
        let (os, io) = cell_orthogonality(cc[c], &faces);
        m.cell_ortho_skew[c] = os;
        m.cell_inverse_orthogonal[c] = io;
        if grow[c].is_finite() {
            m.cell_size_change[c] = grow[c];
        }
        if !m.cell_face_angle_min[c].is_finite() {
            m.cell_face_angle_min[c] = 0.0;
        }
    }
    m
}
