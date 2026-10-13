//! Derived finite-volume geometry: interpolation weights, cell-to-cell
//! vectors, over-relaxed diffusion coefficients and periodic face maps.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::LduAddressing;
use rcfd_mesh::{Mesh, PatchKind};
use std::sync::Arc;

/// Mesh plus the face-based quantities used by every discretisation.
#[derive(Debug, Clone)]
pub struct FvGeometry {
    /// Underlying mesh.
    pub mesh: Arc<Mesh>,
    /// Number of internal faces.
    pub n_internal: usize,
    /// Linear interpolation weight of the owner cell, per internal face.
    pub weights: Vec<f64>,
    /// Internal faces: neighbour centre minus owner centre. Boundary faces:
    /// face centre minus owner centre.
    pub delta: Vec<Vec3>,
    /// Over-relaxed diffusion geometry coefficient `|S|^2 / (S . d)` per face
    /// (so that flux = gamma * coeff * (phi_N - phi_P) on internal faces).
    /// Boundary faces: `|S| / dn` with `dn` the normal distance.
    pub lap_coeff: Vec<f64>,
    /// Normal distance from owner centre to the face, per boundary face
    /// (index `f - n_internal`).
    pub dn: Vec<f64>,
    /// Per face: true for faces on `Empty` patches (excluded from all operators).
    pub empty: Vec<bool>,
    /// Directions that are solved (false for the empty direction of 2D meshes).
    pub active: [bool; 3],
    /// Per patch: for periodic patches the global partner face of each local face.
    pub periodic_partner: Vec<Option<Vec<usize>>>,
    /// Per patch: for rotationally periodic patches the matrix that expresses a vector
    /// living at the partner patch in the local frame of this patch (`R^T`, row-major).
    pub periodic_rot: Vec<Option<[[f64; 3]; 3]>>,
    /// Per boundary face (`f - n_internal`): the face belongs to a periodic patch.
    pub coupled_face: Vec<bool>,
    /// Shared matrix addressing.
    pub addressing: Arc<LduAddressing>,
}

impl FvGeometry {
    /// Compute the geometry of `mesh`.
    pub fn new(mesh: Arc<Mesh>) -> Result<Self> {
        let n_int = mesh.n_internal_faces();
        let nf = mesh.n_faces();
        let sf = mesh.face_area_vectors();
        let cf = mesh.face_centres();
        let cc = mesh.cell_centres();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut weights = vec![0.5; n_int];
        let mut delta = vec![[0.0; 3]; nf];
        let mut lap = vec![0.0; nf];
        let mut dn = vec![0.0; nf - n_int];
        for f in 0..n_int {
            let (o, n) = (own[f], nei[f]);
            delta[f] = vec3::sub(cc[n], cc[o]);
            let d_o = vec3::dot(sf[f], vec3::sub(cf[f], cc[o])).abs();
            let d_n = vec3::dot(sf[f], vec3::sub(cc[n], cf[f])).abs();
            weights[f] = d_n / (d_o + d_n);
            lap[f] = vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], delta[f]);
        }
        for f in n_int..nf {
            delta[f] = vec3::sub(cf[f], cc[own[f]]);
            let a = vec3::norm(sf[f]);
            dn[f - n_int] = vec3::dot(sf[f], delta[f]).abs() / a;
            lap[f] = a / dn[f - n_int];
        }
        let mut empty = vec![false; nf];
        let mut active = [true; 3];
        for p in mesh.patches() {
            if p.kind == PatchKind::Empty {
                for f in p.faces() {
                    empty[f] = true;
                    let n = vec3::unit(sf[f]);
                    for (d, a) in active.iter_mut().enumerate() {
                        if n[d].abs() > 0.99 {
                            *a = false;
                        }
                    }
                }
            }
        }
        let periodic_partner = mesh
            .patches()
            .iter()
            .map(|p| {
                p.periodic_with
                    .as_ref()
                    .map(|q| periodic_map(&mesh, &p.name, q, p.rotation.as_ref()))
                    .transpose()
            })
            .collect::<Result<Vec<_>>>()?;
        let periodic_rot = mesh
            .patches()
            .iter()
            .map(|p| {
                p.rotation.as_ref().map(|r| {
                    let m = r.matrix();
                    [0, 1, 2].map(|i| [0, 1, 2].map(|j| m[j][i]))
                })
            })
            .collect();
        let mut coupled_face = vec![false; nf - n_int];
        for (pi, p) in mesh.patches().iter().enumerate() {
            if periodic_partner[pi].is_some() {
                for f in p.faces() {
                    coupled_face[f - n_int] = true;
                }
            }
        }
        let addressing = Arc::new(LduAddressing {
            n: mesh.n_cells(),
            lower: own[..n_int].to_vec(),
            upper: nei.to_vec(),
        });
        Ok(FvGeometry {
            mesh,
            n_internal: n_int,
            weights,
            delta,
            lap_coeff: lap,
            dn,
            empty,
            active,
            periodic_partner,
            periodic_rot,
            coupled_face,
            addressing,
        })
    }

    /// Number of cells.
    pub fn n_cells(&self) -> usize {
        self.mesh.n_cells()
    }

    /// Unit normal of a face.
    pub fn normal(&self, f: usize) -> Vec3 {
        vec3::unit(self.mesh.face_area_vectors()[f])
    }

    /// Faces of all non-empty boundary patches with their patch index.
    pub fn boundary_faces(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.mesh
            .patches()
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.faces().map(move |f| (pi, f)))
            .filter(|&(_, f)| !self.empty[f])
    }
}

/// Match each face of `patch` to the face of `partner` obtained by a pure
/// translation, or by the patch rotation when one is given. Matching is
/// geometric, so it does not depend on face ordering.
fn periodic_map(
    mesh: &Mesh,
    patch: &str,
    partner: &str,
    rotation: Option<&rcfd_mesh::PeriodicRotation>,
) -> Result<Vec<usize>> {
    let p = mesh.patch(patch).ok_or_else(|| Error::PathNotFound(patch.into()))?;
    let q = mesh.patch(partner).ok_or_else(|| Error::PathNotFound(partner.into()))?;
    if p.size != q.size {
        return Err(Error::Mesh(format!(
            "periodic patches {patch}/{partner} differ in face count"
        )));
    }
    let cf = mesh.face_centres();
    let mean = |r: std::ops::Range<usize>| {
        let mut m = [0.0; 3];
        let n = r.len() as f64;
        for f in r {
            vec3::axpy(&mut m, 1.0 / n, cf[f]);
        }
        m
    };
    let shift = if rotation.is_some() {
        [0.0; 3]
    } else {
        vec3::sub(mean(q.faces()), mean(p.faces()))
    };
    let scale = (vec3::norm(shift)).max(1e-30);
    let mut used = vec![false; q.size];
    let mut out = Vec::with_capacity(p.size);
    for f in p.faces() {
        let target = match rotation {
            Some(r) => r.apply_point(cf[f]),
            None => vec3::add(cf[f], shift),
        };
        let (mut best, mut best_d) = (usize::MAX, f64::INFINITY);
        for g in q.faces() {
            let d = vec3::norm(vec3::sub(cf[g], target));
            if d < best_d {
                best_d = d;
                best = g;
            }
        }
        let tol = 1e-6 * scale.max(mesh.total_volume().cbrt());
        if best_d > tol || used[best - q.start] {
            return Err(Error::Mesh(format!(
                "periodic patches {patch}/{partner} are not matched by the periodic transform"
            )));
        }
        used[best - q.start] = true;
        out.push(best);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};

    #[test]
    fn weights_and_coefficients_on_stretched_mesh() {
        let m = Arc::new(
            box_mesh(
                [4, 3, 2],
                [1.0, 1.0, 1.0],
                [Grading::Expansion(3.0), Grading::Uniform, Grading::Uniform],
            )
            .unwrap(),
        );
        let g = FvGeometry::new(m.clone()).unwrap();
        assert!(g.weights.iter().all(|&w| w > 0.0 && w < 1.0));
        // orthogonal mesh: lap_coeff = |S| / |d|
        for f in 0..g.n_internal {
            let expect = m.face_area(f) / vec3::norm(g.delta[f]);
            assert!((g.lap_coeff[f] - expect).abs() < 1e-12 * expect);
        }
        // boundary normal distance on a unit-width uniform direction: half a cell
        assert!(g.dn.iter().all(|&d| d > 0.0));
        assert!(g.active == [true; 3]);
    }

    #[test]
    fn empty_direction_detected_in_2d() {
        let m = Arc::new(cavity_2d(4, 1.0).unwrap());
        let g = FvGeometry::new(m).unwrap();
        assert_eq!(g.active, [true, true, false]);
        assert_eq!(g.empty.iter().filter(|&&e| e).count(), 2 * 16);
        assert_eq!(g.boundary_faces().count(), 4 * 4);
    }

    #[test]
    fn periodic_map_is_geometric_and_bijective() {
        let mut m = box_mesh([3, 2, 1], [3.0, 2.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let names: Vec<String> = m.patches().iter().map(|p| p.name.clone()).collect();
        assert!(names.len() >= 6, "{names:?}");
        let lo = names
            .iter()
            .find(|n| n.contains("x") && (n.contains("min") || n.contains("0") || n.contains("left")))
            .cloned();
        let _ = lo;
        // find the two patches whose face normals point -x and +x
        let mut minus = None;
        let mut plus = None;
        for p in m.patches() {
            let n = vec3::unit(m.face_area_vectors()[p.start]);
            if n[0] < -0.9 {
                minus = Some(p.name.clone());
            }
            if n[0] > 0.9 {
                plus = Some(p.name.clone());
            }
        }
        let (a, b) = (minus.unwrap(), plus.unwrap());
        m.set_periodic(&a, &b).unwrap();
        m.set_periodic(&b, &a).unwrap();
        let g = FvGeometry::new(Arc::new(m)).unwrap();
        let pi = g.mesh.patches().iter().position(|p| p.name == a).unwrap();
        let map = g.periodic_partner[pi].as_ref().unwrap();
        for (i, &partner) in map.iter().enumerate() {
            let f = g.mesh.patches()[pi].start + i;
            let (cf, cp) = (g.mesh.face_centres()[f], g.mesh.face_centres()[partner]);
            assert!(
                (cp[0] - cf[0] - 3.0).abs() < 1e-12 && (cp[1] - cf[1]).abs() < 1e-12 && (cp[2] - cf[2]).abs() < 1e-12
            );
        }
    }
}
