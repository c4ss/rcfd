//! Mesh motion kernels (F-063, ADR 0033): rebuild a mesh with moved points, rigid poses, and the swept volume of
//! every face that satisfies the discrete geometric conservation law (GCL).
//!
//! # Swept volume
//!
//! The mesh volume of a cell is `V = (1/3) sum_f sigma_f S_f . fc_f` with `(fc_f, S_f)` from
//! [`polygon_geometry`] (the apex term of the pyramid sum vanishes because the face area vectors close). For one
//! step from the point positions `x0` to `x1` define for every face
//!
//! ```text
//! W_f = (1/3) [ S1 . (fc1 - O) - S0 . (fc0 - O) + sum_edges sum_{t in quad} A_t . (c_t - O) ]
//! ```
//!
//! where the side quad of the face edge `a -> b` (counter-clockwise about the outward normal) is
//! `a0, b0, b1, a1`, split along the diagonal `lo0 - hi1` (`lo < hi` the two point indices of the edge, so that both
//! faces sharing the edge use the same two triangles; the area vector changes sign with the traversal direction),
//! `A_t` and `c_t` are the area vector and centroid of a triangle. `W_f` does not depend on the origin `O` (the
//! side quads close the surface), the side-quad terms of the two faces that meet at a cell edge cancel exactly
//! inside a cell, so `sum_f sigma_f W_f = V1 - V0` for every cell to round-off, also for warped faces. For a rigid
//! translation `d`, `W_f = d . S_f`. The owner receives `+W_f`, the neighbour `-W_f`.

use crate::mesh::{polygon_geometry, Mesh};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::Result;

/// Rigid pose: rotation by `angle` about the axis `axis` through `origin`, followed by `translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Point on the rotation axis.
    pub origin: Vec3,
    /// Rotation axis (any non-zero length; ignored when `angle` is zero).
    pub axis: Vec3,
    /// Rotation angle in radians (right-handed about `axis`).
    pub angle: f64,
    /// Translation applied after the rotation.
    pub translation: Vec3,
}

impl Pose {
    /// The identity.
    pub fn identity() -> Self {
        Pose {
            origin: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            angle: 0.0,
            translation: vec3::ZERO,
        }
    }

    /// Pure translation.
    pub fn translation(d: Vec3) -> Self {
        Pose {
            translation: d,
            ..Self::identity()
        }
    }

    /// Image of a point.
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let r = rotation_matrix(self.axis, self.angle);
        let q = mat_vec(&r, vec3::sub(p, self.origin));
        vec3::add(vec3::add(self.origin, q), self.translation)
    }
}

/// Rotation matrix (row-major) about `axis` by `angle` (Rodrigues); the identity for a zero angle or axis.
pub fn rotation_matrix(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = vec3::norm(axis);
    if angle == 0.0 || n < 1e-300 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let k = vec3::scale(axis, 1.0 / n);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [
            c + k[0] * k[0] * t,
            k[0] * k[1] * t - k[2] * s,
            k[0] * k[2] * t + k[1] * s,
        ],
        [
            k[1] * k[0] * t + k[2] * s,
            c + k[1] * k[1] * t,
            k[1] * k[2] * t - k[0] * s,
        ],
        [
            k[2] * k[0] * t - k[1] * s,
            k[2] * k[1] * t + k[0] * s,
            c + k[2] * k[2] * t,
        ],
    ]
}

/// Row-major 3x3 matrix times a vector.
pub fn mat_vec(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// The mesh with the same topology and zones but the given point positions. The result is validated like any
/// mesh, so inverted cells (non-positive volume) and broken closure are reported as errors.
pub fn moved_mesh(mesh: &Mesh, points: Vec<Vec3>) -> Result<Mesh> {
    let mut topo = mesh.topology().clone();
    if points.len() != topo.points.len() {
        return Err(rcfd_core::Error::Mesh(format!(
            "moved_mesh: {} points given for a mesh of {}",
            points.len(),
            topo.points.len()
        )));
    }
    topo.points = points;
    Mesh::from_topology(topo)
}

/// Sorted unique point indices of the faces of a patch.
pub fn patch_points(mesh: &Mesh, patch: &str) -> Option<Vec<usize>> {
    let p = mesh.patch(patch)?;
    let mut ids: Vec<usize> = p.faces().flat_map(|f| mesh.face_points(f).iter().copied()).collect();
    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

/// Volume swept by every face of `mesh` when its points move to `new_points` (see the module docs). The origin of
/// the cone volumes is the old face centre unless `origin` is given.
pub fn swept_volumes_about(mesh: &Mesh, new_points: &[Vec3], origin: Option<Vec3>) -> Vec<f64> {
    let old = mesh.points();
    assert_eq!(old.len(), new_points.len(), "swept_volumes: point count mismatch");
    (0..mesh.n_faces())
        .map(|f| {
            let ids = mesh.face_points(f);
            let (fc0, s0) = polygon_geometry(old, ids);
            let (fc1, s1) = polygon_geometry(new_points, ids);
            let o = origin.unwrap_or(fc0);
            let mut sum = vec3::dot(s1, vec3::sub(fc1, o)) - vec3::dot(s0, vec3::sub(fc0, o));
            let n = ids.len();
            for i in 0..n {
                let (a, b) = (ids[i], ids[(i + 1) % n]);
                let (lo, hi, sign) = if a < b { (a, b, 1.0) } else { (b, a, -1.0) };
                let (lo0, hi0) = (vec3::sub(old[lo], o), vec3::sub(old[hi], o));
                let (lo1, hi1) = (vec3::sub(new_points[lo], o), vec3::sub(new_points[hi], o));
                // triangles (lo0, hi0, hi1) and (lo0, hi1, lo1)
                let a1 = vec3::scale(vec3::cross(vec3::sub(hi0, lo0), vec3::sub(hi1, lo0)), 0.5);
                let c1 = vec3::scale(vec3::add(vec3::add(lo0, hi0), hi1), 1.0 / 3.0);
                let a2 = vec3::scale(vec3::cross(vec3::sub(hi1, lo0), vec3::sub(lo1, lo0)), 0.5);
                let c2 = vec3::scale(vec3::add(vec3::add(lo0, hi1), lo1), 1.0 / 3.0);
                sum += sign * (vec3::dot(a1, c1) + vec3::dot(a2, c2));
            }
            sum / 3.0
        })
        .collect()
}

/// [`swept_volumes_about`] with the old face centre as the origin.
pub fn swept_volumes(mesh: &Mesh, new_points: &[Vec3]) -> Vec<f64> {
    swept_volumes_about(mesh, new_points, None)
}

/// Per-cell sum of the swept volumes (owner `+`, neighbour `-`); equals `V1 - V0` (the GCL).
pub fn cell_swept_sum(mesh: &Mesh, w: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; mesh.n_cells()];
    let ni = mesh.n_internal_faces();
    for f in 0..mesh.n_faces() {
        out[mesh.owner()[f]] += w[f];
        if f < ni {
            out[mesh.neighbour()[f]] -= w[f];
        }
    }
    out
}

/// Smallest ratio `V_new / V_old` over the cells.
pub fn min_volume_ratio(old: &Mesh, new: &Mesh) -> f64 {
    old.cell_volumes()
        .iter()
        .zip(new.cell_volumes())
        .map(|(a, b)| b / a)
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{box_mesh, Grading};

    fn mesh() -> Mesh {
        box_mesh(
            [3, 2, 2],
            [1.0, 0.8, 0.6],
            [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
        )
        .unwrap()
    }

    #[test]
    fn rotation_matrix_is_orthonormal_and_right_handed() {
        let r = rotation_matrix([0.0, 0.0, 2.0], std::f64::consts::FRAC_PI_2);
        let v = mat_vec(&r, [1.0, 0.0, 0.0]);
        assert!(vec3::norm(vec3::sub(v, [0.0, 1.0, 0.0])) < 1e-15);
        let p = Pose {
            origin: [1.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            angle: std::f64::consts::PI,
            translation: [0.0, 0.0, 1.0],
        };
        assert!(vec3::norm(vec3::sub(p.apply([2.0, 0.0, 0.0]), [0.0, 0.0, 1.0])) < 1e-15);
    }

    #[test]
    fn translation_sweeps_d_dot_s_and_the_gcl_holds() {
        let m = mesh();
        let d = [0.3, -0.2, 0.1];
        let np: Vec<Vec3> = m.points().iter().map(|&p| vec3::add(p, d)).collect();
        let w = swept_volumes(&m, &np);
        for f in 0..m.n_faces() {
            let e = (w[f] - vec3::dot(d, m.face_area_vectors()[f])).abs();
            assert!(e < 1e-14, "face {f}: {e}");
        }
        let m1 = moved_mesh(&m, np).unwrap();
        for (c, s) in cell_swept_sum(&m, &w).iter().enumerate() {
            assert!((s - (m1.cell_volumes()[c] - m.cell_volumes()[c])).abs() < 1e-14);
        }
    }

    #[test]
    fn rotation_and_warped_faces_satisfy_the_gcl_and_origin_independence() {
        let m = mesh();
        let pose = Pose {
            origin: [0.2, 0.1, 0.0],
            axis: [1.0, 2.0, 3.0],
            angle: 0.4,
            translation: [0.0; 3],
        };
        // warp the interior points to make the faces non-planar
        let mut np: Vec<Vec3> = m.points().to_vec();
        for (i, p) in np.iter_mut().enumerate() {
            let inside = (0..3).all(|k| p[k] > 1e-9 && p[k] < [1.0, 0.8, 0.6][k] - 1e-9);
            if inside {
                let w = 0.03;
                *p = vec3::add(
                    *p,
                    [
                        w * ((7 * i + 1) as f64).sin(),
                        w * ((5 * i + 2) as f64).cos(),
                        w * ((3 * i + 4) as f64).sin(),
                    ],
                );
            }
        }
        let m_w = moved_mesh(&m, np).unwrap();
        let np2: Vec<Vec3> = m_w.points().iter().map(|&p| pose.apply(p)).collect();
        let w = swept_volumes(&m_w, &np2);
        let w_alt = swept_volumes_about(&m_w, &np2, Some([5.0, -3.0, 2.0]));
        let m1 = moved_mesh(&m_w, np2).unwrap();
        let vmean = m_w.total_volume() / m_w.n_cells() as f64;
        for (c, s) in cell_swept_sum(&m_w, &w).iter().enumerate() {
            let dv = m1.cell_volumes()[c] - m_w.cell_volumes()[c];
            assert!((s - dv).abs() < 1e-13 * vmean, "cell {c}");
        }
        for f in 0..m_w.n_faces() {
            assert!((w[f] - w_alt[f]).abs() < 1e-12 * vmean, "face {f}");
        }
    }

    #[test]
    fn inverted_motion_is_reported() {
        let m = mesh();
        let np: Vec<Vec3> = m.points().iter().map(|&p| [p[0], -p[1], p[2]]).collect();
        let e = moved_mesh(&m, np).unwrap_err().to_string();
        assert!(e.contains("non-positive volume") || e.contains("normal"), "{e}");
    }
}
