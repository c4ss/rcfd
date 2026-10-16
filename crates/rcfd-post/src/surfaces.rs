//! Surface and point post-objects (F-238, ADR 0048 D3): implicit and quadric cuts, iso-clip, offset, instance transforms,
//! rakes and point clouds.
//!
//! A [`Surface`] is a welded triangle mesh. Cuts are extracted by marching tetrahedra over a decomposition of every cell
//! into tets (cell centre, face centre, two consecutive face points); the implicit function is evaluated exactly at the tet
//! vertices and linearly interpolated along edges, so a plane is reproduced exactly and curved cuts are second order in the
//! mesh size. `g >= 0` counts as positive, a face lying on the level set is therefore emitted once (by the negative side).
//! The facet normal points to increasing `g`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_fv::FvGeometry;
use rcfd_mesh::Mesh;
use std::collections::HashMap;

/// Facets whose doubled area is below this fraction of the squared longest edge of the tet are dropped by [`cut_implicit`].
pub const CULL_RATIO: f64 = 1e-12;

fn bad(msg: String) -> Error {
    Error::Other(msg)
}

/// A triangle surface with welded vertices.
#[derive(Debug, Clone, Default)]
pub struct Surface {
    /// Vertex coordinates.
    pub points: Vec<Vec3>,
    /// Triangles (vertex indices, counter-clockwise seen from the oriented side).
    pub tris: Vec<[usize; 3]>,
    /// Mesh cell that produced each triangle (`None` after a merge of surfaces of different meshes is never produced;
    /// kept for sampling).
    pub tri_cell: Vec<Option<usize>>,
    /// Mesh cell used to reconstruct a field at each vertex.
    pub vertex_cell: Vec<Option<usize>>,
}

/// Quadric `p^T A p + b . p + c`; positive on the outer / normal side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quadric {
    /// Symmetric matrix `A`.
    pub a: [[f64; 3]; 3],
    /// Linear part.
    pub b: Vec3,
    /// Constant.
    pub c: f64,
}

impl Quadric {
    /// General quadric; `A` must be symmetric and everything finite.
    pub fn new(a: [[f64; 3]; 3], b: Vec3, c: f64) -> Result<Quadric> {
        let fin = a.iter().flatten().all(|x| x.is_finite()) && b.iter().all(|x| x.is_finite()) && c.is_finite();
        if !fin {
            return Err(bad("Quadric: coefficients must be finite".into()));
        }
        for i in 0..3 {
            for j in 0..i {
                if (a[i][j] - a[j][i]).abs() > 1e-12 * (a[i][j].abs() + a[j][i].abs()).max(1.0) {
                    return Err(bad("Quadric: the matrix a must be symmetric".into()));
                }
            }
        }
        if a.iter().flatten().all(|x| *x == 0.0) && b == [0.0; 3] {
            return Err(bad("Quadric: a and b are both zero (no surface)".into()));
        }
        Ok(Quadric { a, b, c })
    }

    /// Plane through `point` with normal `normal` (positive on the normal side).
    pub fn plane(point: Vec3, normal: Vec3) -> Result<Quadric> {
        let n = vec3::norm(normal);
        if !(n.is_finite() && n > 0.0) {
            return Err(bad("plane: normal must be a non-zero finite vector".into()));
        }
        let u = vec3::scale(normal, 1.0 / n);
        Quadric::new([[0.0; 3]; 3], u, -vec3::dot(u, point))
    }

    /// Sphere (positive outside).
    pub fn sphere(centre: Vec3, radius: f64) -> Result<Quadric> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(bad(format!("sphere: radius = {radius} must be positive and finite")));
        }
        let a = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        Quadric::new(
            a,
            vec3::scale(centre, -2.0),
            vec3::dot(centre, centre) - radius * radius,
        )
    }

    /// Infinite circular cylinder about the line `point + t axis` (positive outside).
    pub fn cylinder(point: Vec3, axis: Vec3, radius: f64) -> Result<Quadric> {
        if !(radius.is_finite() && radius > 0.0) {
            return Err(bad(format!("cylinder: radius = {radius} must be positive and finite")));
        }
        let n = vec3::norm(axis);
        if !(n.is_finite() && n > 0.0) {
            return Err(bad("cylinder: axis must be a non-zero finite vector".into()));
        }
        let u = vec3::scale(axis, 1.0 / n);
        let mut a = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] = if i == j { 1.0 } else { 0.0 } - u[i] * u[j];
            }
        }
        let ap = [vec3::dot(a[0], point), vec3::dot(a[1], point), vec3::dot(a[2], point)];
        Quadric::new(a, vec3::scale(ap, -2.0), vec3::dot(point, ap) - radius * radius)
    }

    /// Double circular cone with apex `apex`, axis `axis` and half angle `half_angle` [rad], in (0, pi/2)
    /// (positive outside both nappes; limit the mesh region to the wanted nappe).
    pub fn cone(apex: Vec3, axis: Vec3, half_angle: f64) -> Result<Quadric> {
        if !(half_angle.is_finite() && half_angle > 0.0 && half_angle < std::f64::consts::FRAC_PI_2) {
            return Err(bad(format!(
                "cone: half_angle = {half_angle} must lie in (0, pi/2) rad"
            )));
        }
        let n = vec3::norm(axis);
        if !(n.is_finite() && n > 0.0) {
            return Err(bad("cone: axis must be a non-zero finite vector".into()));
        }
        let u = vec3::scale(axis, 1.0 / n);
        let c2 = half_angle.cos().powi(2);
        let mut a = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] = c2 * if i == j { 1.0 } else { 0.0 } - u[i] * u[j];
            }
        }
        let ap = [vec3::dot(a[0], apex), vec3::dot(a[1], apex), vec3::dot(a[2], apex)];
        Quadric::new(a, vec3::scale(ap, -2.0), vec3::dot(apex, ap))
    }

    /// Value at a point.
    pub fn eval(&self, p: Vec3) -> f64 {
        let ap = [
            vec3::dot(self.a[0], p),
            vec3::dot(self.a[1], p),
            vec3::dot(self.a[2], p),
        ];
        vec3::dot(p, ap) + vec3::dot(self.b, p) + self.c
    }
}

/// Least-squares cell gradients of a cell field (neighbours through internal faces; a direction without any spread, the
/// thin direction of a one-cell-thick mesh, gets a zero gradient component). Exact for linear fields.
pub fn cell_gradients(mesh: &Mesh, values: &[f64]) -> Vec<Vec3> {
    let cc = mesh.cell_centres();
    let n = mesh.n_cells();
    let mut m = vec![[[0.0; 3]; 3]; n];
    let mut rhs = vec![[0.0; 3]; n];
    for f in 0..mesh.n_internal_faces() {
        let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
        for (a, b) in [(o, nb), (nb, o)] {
            let d = vec3::sub(cc[b], cc[a]);
            let dv = values[b] - values[a];
            for i in 0..3 {
                for j in 0..3 {
                    m[a][i][j] += d[i] * d[j];
                }
                rhs[a][i] += d[i] * dv;
            }
        }
    }
    (0..n)
        .map(|c| {
            let tr = (m[c][0][0] + m[c][1][1] + m[c][2][2]).max(1e-300);
            let mut a = m[c];
            for k in 0..3 {
                if a[k][k] <= 1e-9 * tr {
                    a[k][k] += tr;
                }
            }
            solve3(a, rhs[c])
        })
        .collect()
}

fn solve3(a: [[f64; 3]; 3], b: Vec3) -> Vec3 {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-300 {
        return [0.0; 3];
    }
    let cof = |i: usize, j: usize| {
        let (i1, i2) = ((i + 1) % 3, (i + 2) % 3);
        let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
        a[i1][j1] * a[i2][j2] - a[i1][j2] * a[i2][j1]
    };
    let mut x = [0.0; 3];
    for (i, xi) in x.iter_mut().enumerate() {
        for (j, bj) in b.iter().enumerate() {
            *xi += cof(j, i) / det * bj;
        }
    }
    x
}

/// Marching-tetrahedra cut of a mesh at `g = 0` (see the module comment).
pub fn cut_implicit(mesh: &Mesh, g: impl Fn(Vec3) -> f64) -> Result<Surface> {
    let np = mesh.n_points();
    let nf = mesh.n_faces();
    let nc = mesh.n_cells();
    let pos = |id: usize| -> Vec3 {
        if id < np {
            mesh.points()[id]
        } else if id < np + nf {
            mesh.face_centres()[id - np]
        } else {
            mesh.cell_centres()[id - np - nf]
        }
    };
    let gv: Vec<f64> = (0..np + nf + nc).map(|i| g(pos(i))).collect();
    if gv.iter().any(|x| !x.is_finite()) {
        return Err(bad(
            "cut_implicit: the implicit function is not finite at a mesh location".into(),
        ));
    }
    let mut surf = Surface::default();
    let mut vmap: HashMap<(usize, usize), usize> = HashMap::new();
    for c in 0..nc {
        let cid = np + nf + c;
        for &f in mesh.cell_faces(c) {
            let fp = mesh.face_points(f);
            for k in 0..fp.len() {
                let tet = [cid, np + f, fp[k], fp[(k + 1) % fp.len()]];
                let ids: Vec<usize> = (0..4).collect();
                let posi: Vec<usize> = ids.iter().copied().filter(|&i| gv[tet[i]] >= 0.0).collect();
                let negi: Vec<usize> = ids.iter().copied().filter(|&i| gv[tet[i]] < 0.0).collect();
                if posi.is_empty() || negi.is_empty() {
                    continue;
                }
                let mut vertex = |a: usize, b: usize, surf: &mut Surface| -> usize {
                    // a positive (g >= 0), b negative
                    let (ia, ib) = (tet[a], tet[b]);
                    let key = if gv[ia] == 0.0 {
                        (ia, ia)
                    } else {
                        (ia.min(ib), ia.max(ib))
                    };
                    *vmap.entry(key).or_insert_with(|| {
                        let p = if gv[ia] == 0.0 {
                            pos(ia)
                        } else {
                            let t = gv[ia] / (gv[ia] - gv[ib]);
                            vec3::add(pos(ia), vec3::scale(vec3::sub(pos(ib), pos(ia)), t))
                        };
                        surf.points.push(p);
                        surf.vertex_cell.push(Some(c));
                        surf.points.len() - 1
                    })
                };
                let mut polys: Vec<[usize; 3]> = Vec::new();
                match (posi.len(), negi.len()) {
                    (1, 3) => {
                        let a = posi[0];
                        let v: Vec<usize> = negi.iter().map(|&b| vertex(a, b, &mut surf)).collect();
                        polys.push([v[0], v[1], v[2]]);
                    }
                    (3, 1) => {
                        let b = negi[0];
                        let v: Vec<usize> = posi.iter().map(|&a| vertex(a, b, &mut surf)).collect();
                        polys.push([v[0], v[1], v[2]]);
                    }
                    _ => {
                        // two positive, two negative: quad around the four crossing edges
                        let (a0, a1, b0, b1) = (posi[0], posi[1], negi[0], negi[1]);
                        let q = [
                            vertex(a0, b0, &mut surf),
                            vertex(a0, b1, &mut surf),
                            vertex(a1, b1, &mut surf),
                            vertex(a1, b0, &mut surf),
                        ];
                        polys.push([q[0], q[1], q[2]]);
                        polys.push([q[0], q[2], q[3]]);
                    }
                }
                let mp = posi.iter().fold([0.0; 3], |s, &i| vec3::add(s, pos(tet[i])));
                let mn = negi.iter().fold([0.0; 3], |s, &i| vec3::add(s, pos(tet[i])));
                let up = vec3::sub(
                    vec3::scale(mp, 1.0 / posi.len() as f64),
                    vec3::scale(mn, 1.0 / negi.len() as f64),
                );
                for mut t in polys {
                    let n = vec3::cross(
                        vec3::sub(surf.points[t[1]], surf.points[t[0]]),
                        vec3::sub(surf.points[t[2]], surf.points[t[0]]),
                    );
                    // numerically degenerate facets (crossing within rounding of a tet vertex) have a noise normal and
                    // no area: drop them (area / (longest tet edge)^2 below CULL_RATIO)
                    let e2 = (0..4)
                        .flat_map(|a| (a + 1..4).map(move |b| (a, b)))
                        .map(|(a, b)| {
                            let d = vec3::sub(pos(tet[a]), pos(tet[b]));
                            vec3::dot(d, d)
                        })
                        .fold(0.0, f64::max);
                    if vec3::norm(n) <= CULL_RATIO * e2 {
                        continue;
                    }
                    if vec3::dot(n, up) < 0.0 {
                        t.swap(1, 2);
                    }
                    surf.tris.push(t);
                    surf.tri_cell.push(Some(c));
                }
            }
        }
    }
    Ok(surf)
}

/// Cut by a quadric.
pub fn cut_quadric(mesh: &Mesh, q: &Quadric) -> Result<Surface> {
    cut_implicit(mesh, |p| q.eval(p))
}

/// Plane cut (exact). The normal side is the positive side.
pub fn cut_plane(mesh: &Mesh, point: Vec3, normal: Vec3) -> Result<Surface> {
    cut_quadric(mesh, &Quadric::plane(point, normal)?)
}

/// Sphere cut (normals point outward).
pub fn cut_sphere(mesh: &Mesh, centre: Vec3, radius: f64) -> Result<Surface> {
    cut_quadric(mesh, &Quadric::sphere(centre, radius)?)
}

/// Cylinder cut.
pub fn cut_cylinder(mesh: &Mesh, point: Vec3, axis: Vec3, radius: f64) -> Result<Surface> {
    cut_quadric(mesh, &Quadric::cylinder(point, axis, radius)?)
}

/// Cone cut.
pub fn cut_cone(mesh: &Mesh, apex: Vec3, axis: Vec3, half_angle: f64) -> Result<Surface> {
    cut_quadric(mesh, &Quadric::cone(apex, axis, half_angle)?)
}

/// A rigid transform `p -> R (p - origin) + origin + translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Rotation matrix.
    pub rot: [[f64; 3]; 3],
    /// Rotation centre.
    pub origin: Vec3,
    /// Translation applied after the rotation.
    pub translation: Vec3,
}

impl Transform {
    /// Pure translation.
    pub fn translation(t: Vec3) -> Result<Transform> {
        if !t.iter().all(|x| x.is_finite()) {
            return Err(bad("translation: vector must be finite".into()));
        }
        Ok(Transform {
            rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            origin: [0.0; 3],
            translation: t,
        })
    }

    /// Rotation by `angle` [rad] about the axis through `origin` (right-hand rule).
    pub fn rotation(axis: Vec3, angle: f64, origin: Vec3) -> Result<Transform> {
        let n = vec3::norm(axis);
        if !(n.is_finite() && n > 0.0) || !angle.is_finite() {
            return Err(bad(
                "rotation: axis must be a non-zero finite vector and angle finite".into()
            ));
        }
        let u = vec3::scale(axis, 1.0 / n);
        let (s, c) = angle.sin_cos();
        let k = 1.0 - c;
        let rot = [
            [
                c + u[0] * u[0] * k,
                u[0] * u[1] * k - u[2] * s,
                u[0] * u[2] * k + u[1] * s,
            ],
            [
                u[1] * u[0] * k + u[2] * s,
                c + u[1] * u[1] * k,
                u[1] * u[2] * k - u[0] * s,
            ],
            [
                u[2] * u[0] * k - u[1] * s,
                u[2] * u[1] * k + u[0] * s,
                c + u[2] * u[2] * k,
            ],
        ];
        Ok(Transform {
            rot,
            origin,
            translation: [0.0; 3],
        })
    }

    /// Apply to a point.
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let d = vec3::sub(p, self.origin);
        let r = [
            vec3::dot(self.rot[0], d),
            vec3::dot(self.rot[1], d),
            vec3::dot(self.rot[2], d),
        ];
        vec3::add(vec3::add(r, self.origin), self.translation)
    }
}

impl Surface {
    /// Number of triangles.
    pub fn n_tris(&self) -> usize {
        self.tris.len()
    }

    /// Oriented area vector of every triangle.
    pub fn area_vectors(&self) -> Vec<Vec3> {
        self.tris
            .iter()
            .map(|t| {
                let (a, b, c) = (self.points[t[0]], self.points[t[1]], self.points[t[2]]);
                vec3::scale(vec3::cross(vec3::sub(b, a), vec3::sub(c, a)), 0.5)
            })
            .collect()
    }

    /// Total area.
    pub fn area(&self) -> f64 {
        self.area_vectors().iter().map(|a| vec3::norm(*a)).sum()
    }

    /// Triangle centroids.
    pub fn centroids(&self) -> Vec<Vec3> {
        self.tris
            .iter()
            .map(|t| {
                let s = vec3::add(vec3::add(self.points[t[0]], self.points[t[1]]), self.points[t[2]]);
                vec3::scale(s, 1.0 / 3.0)
            })
            .collect()
    }

    /// Facet values of a cell field: cell value plus gradient times the centroid offset (exact for linear fields).
    pub fn facet_scalar(&self, mesh: &Mesh, cell_values: &[f64]) -> Result<Vec<f64>> {
        if cell_values.len() != mesh.n_cells() {
            return Err(bad(format!(
                "facet_scalar: cell_values has {} entries, the mesh has {} cells",
                cell_values.len(),
                mesh.n_cells()
            )));
        }
        let g = cell_gradients(mesh, cell_values);
        let cc = mesh.cell_centres();
        let cen = self.centroids();
        Ok(cen
            .iter()
            .zip(&self.tri_cell)
            .map(|(x, c)| {
                let c = c.expect("facet has a cell");
                cell_values[c] + vec3::dot(g[c], vec3::sub(*x, cc[c]))
            })
            .collect())
    }

    /// Facet values of a vector cell field.
    pub fn facet_vector(&self, mesh: &Mesh, cell_values: &[Vec3]) -> Result<Vec<Vec3>> {
        let mut comps = Vec::new();
        for k in 0..3 {
            let c: Vec<f64> = cell_values.iter().map(|v| v[k]).collect();
            comps.push(self.facet_scalar(mesh, &c)?);
        }
        Ok((0..self.tris.len())
            .map(|i| [comps[0][i], comps[1][i], comps[2][i]])
            .collect())
    }

    /// Vertex values of a cell field (reconstruction in the generating cell).
    pub fn vertex_scalar(&self, mesh: &Mesh, cell_values: &[f64]) -> Result<Vec<f64>> {
        if cell_values.len() != mesh.n_cells() {
            return Err(bad(format!(
                "vertex_scalar: cell_values has {} entries, the mesh has {} cells",
                cell_values.len(),
                mesh.n_cells()
            )));
        }
        let g = cell_gradients(mesh, cell_values);
        let cc = mesh.cell_centres();
        self.points
            .iter()
            .zip(&self.vertex_cell)
            .map(|(x, c)| {
                let c = c.ok_or_else(|| bad("vertex_scalar: vertex has no source cell".into()))?;
                Ok(cell_values[c] + vec3::dot(g[c], vec3::sub(*x, cc[c])))
            })
            .collect()
    }

    /// `sum a_i v_i` over the facets (`v` per facet).
    pub fn integral(&self, facet_values: &[f64]) -> Result<f64> {
        if facet_values.len() != self.tris.len() {
            return Err(bad(format!(
                "integral: facet_values has {} entries, the surface has {} facets",
                facet_values.len(),
                self.tris.len()
            )));
        }
        Ok(self
            .area_vectors()
            .iter()
            .zip(facet_values)
            .map(|(a, v)| vec3::norm(*a) * v)
            .sum())
    }

    /// Flux `sum F_i . A_i` of a facet vector field through the oriented surface.
    pub fn flux(&self, facet_vectors: &[Vec3]) -> Result<f64> {
        if facet_vectors.len() != self.tris.len() {
            return Err(bad(format!(
                "flux: facet_vectors has {} entries, the surface has {} facets",
                facet_vectors.len(),
                self.tris.len()
            )));
        }
        Ok(self
            .area_vectors()
            .iter()
            .zip(facet_vectors)
            .map(|(a, v)| vec3::dot(*a, *v))
            .sum())
    }

    /// Area-weighted statistics of per-facet values.
    pub fn stats(&self, facet_values: &[f64]) -> Result<crate::stats::WeightedStats> {
        let w: Vec<f64> = self.area_vectors().iter().map(|a| vec3::norm(*a)).collect();
        crate::stats::weighted_stats(facet_values, &w)
    }

    /// Clip at an iso-value of vertex values: keeps `v >= level` (`keep_above`) or `v < level`; returns the clipped
    /// surface and its vertex values (the level at new vertices).
    pub fn clip_by_values(&self, vertex_values: &[f64], level: f64, keep_above: bool) -> Result<(Surface, Vec<f64>)> {
        if vertex_values.len() != self.points.len() {
            return Err(bad(format!(
                "clip_by_values: vertex_values has {} entries, the surface has {} vertices",
                vertex_values.len(),
                self.points.len()
            )));
        }
        if !level.is_finite() || vertex_values.iter().any(|v| !v.is_finite()) {
            return Err(bad("clip_by_values: level and vertex_values must be finite".into()));
        }
        let inside = |v: f64| (v >= level) == keep_above;
        let mut out = Surface::default();
        let mut vals = Vec::new();
        let mut old: HashMap<usize, usize> = HashMap::new();
        let mut edge: HashMap<(usize, usize), usize> = HashMap::new();
        for (ti, t) in self.tris.iter().enumerate() {
            // Sutherland-Hodgman against the half space; vertices are tagged old(i) or new(edge)
            #[derive(Clone, Copy)]
            enum V {
                Old(usize),
                New(usize, usize),
            }
            let mut poly: Vec<V> = Vec::new();
            for k in 0..3 {
                let (i, j) = (t[k], t[(k + 1) % 3]);
                let (ii, ij) = (inside(vertex_values[i]), inside(vertex_values[j]));
                if ii {
                    poly.push(V::Old(i));
                }
                if ii != ij {
                    poly.push(V::New(i, j));
                }
            }
            if poly.len() < 3 {
                continue;
            }
            let mut ids = Vec::new();
            for v in poly {
                let id = match v {
                    V::Old(i) => *old.entry(i).or_insert_with(|| {
                        out.points.push(self.points[i]);
                        out.vertex_cell.push(self.vertex_cell[i]);
                        vals.push(vertex_values[i]);
                        out.points.len() - 1
                    }),
                    V::New(i, j) => *edge.entry((i.min(j), i.max(j))).or_insert_with(|| {
                        let (lo, hi) = (i.min(j), i.max(j));
                        let (a, b) = (vertex_values[lo], vertex_values[hi]);
                        let s = (level - a) / (b - a);
                        out.points.push(vec3::add(
                            self.points[lo],
                            vec3::scale(vec3::sub(self.points[hi], self.points[lo]), s),
                        ));
                        out.vertex_cell.push(self.vertex_cell[lo]);
                        vals.push(level);
                        out.points.len() - 1
                    }),
                };
                ids.push(id);
            }
            for k in 1..ids.len() - 1 {
                out.tris.push([ids[0], ids[k], ids[k + 1]]);
                out.tri_cell.push(self.tri_cell[ti]);
            }
        }
        Ok((out, vals))
    }

    /// Offset surface: every vertex moves by `distance` along its area-weighted unit normal (positive along the
    /// orientation). Vertices without a normal stay.
    pub fn offset(&self, distance: f64) -> Result<Surface> {
        if !distance.is_finite() {
            return Err(bad("offset: distance must be finite".into()));
        }
        let mut n = vec![[0.0; 3]; self.points.len()];
        for (t, a) in self.tris.iter().zip(self.area_vectors()) {
            for &v in t {
                vec3::axpy(&mut n[v], 1.0, a);
            }
        }
        let mut s = self.clone();
        for (p, n) in s.points.iter_mut().zip(&n) {
            *p = vec3::add(*p, vec3::scale(vec3::unit(*n), distance));
        }
        Ok(s)
    }

    /// Transformed copy (a proper rotation keeps the orientation).
    pub fn transformed(&self, t: &Transform) -> Surface {
        let mut s = self.clone();
        for p in &mut s.points {
            *p = t.apply(*p);
        }
        s
    }

    /// `n` copies rotated by `2 pi i / n` about an axis (`i = 0` is the original).
    pub fn rotational_copies(&self, n: usize, axis: Vec3, origin: Vec3) -> Result<Vec<Surface>> {
        if n == 0 {
            return Err(bad("rotational_copies: n_copies must be at least 1".into()));
        }
        (0..n)
            .map(|i| {
                let t = Transform::rotation(axis, 2.0 * std::f64::consts::PI * i as f64 / n as f64, origin)?;
                Ok(self.transformed(&t))
            })
            .collect()
    }

    /// Concatenate surfaces (vertices are not welded across parts).
    pub fn merged(parts: &[Surface]) -> Surface {
        let mut s = Surface::default();
        for p in parts {
            let base = s.points.len();
            s.points.extend(&p.points);
            s.vertex_cell.extend(&p.vertex_cell);
            s.tri_cell.extend(&p.tri_cell);
            s.tris
                .extend(p.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        }
        s
    }
}

/// A set of sample points.
#[derive(Debug, Clone, PartialEq)]
pub struct PointCloud {
    /// Points.
    pub points: Vec<Vec3>,
}

impl PointCloud {
    /// `n >= 2` equally spaced points from `p0` to `p1` inclusive (a rake).
    pub fn rake(p0: Vec3, p1: Vec3, n: usize) -> Result<PointCloud> {
        if n < 2 {
            return Err(bad(format!("rake: n = {n}, at least two points are required")));
        }
        if vec3::norm(vec3::sub(p1, p0)) == 0.0 || ![p0, p1].iter().all(|p| p.iter().all(|x| x.is_finite())) {
            return Err(bad("rake: the end points p0 and p1 coincide or are not finite".into()));
        }
        let pts = (0..n)
            .map(|i| {
                if i == n - 1 {
                    p1
                } else {
                    vec3::add(p0, vec3::scale(vec3::sub(p1, p0), i as f64 / (n - 1) as f64))
                }
            })
            .collect();
        Ok(PointCloud { points: pts })
    }

    /// Cloud of the triangle centroids of a surface.
    pub fn from_surface(s: &Surface) -> PointCloud {
        PointCloud { points: s.centroids() }
    }

    /// Sample a cell field with gradient reconstruction in the containing cell; `(values, inside)`, `NaN` outside.
    pub fn sample(&self, geom: &FvGeometry, cell_values: &[f64]) -> Result<(Vec<f64>, Vec<bool>)> {
        let mesh = &geom.mesh;
        if cell_values.len() != mesh.n_cells() {
            return Err(bad(format!(
                "sample: cell_values has {} entries, the mesh has {} cells",
                cell_values.len(),
                mesh.n_cells()
            )));
        }
        let g = cell_gradients(mesh, cell_values);
        let cc = mesh.cell_centres();
        let mut vals = Vec::new();
        let mut ins = Vec::new();
        for p in &self.points {
            match crate::derived::locate_cell(geom, *p) {
                Some(c) => {
                    vals.push(cell_values[c] + vec3::dot(g[c], vec3::sub(*p, cc[c])));
                    ins.push(true);
                }
                None => {
                    vals.push(f64::NAN);
                    ins.push(false);
                }
            }
        }
        Ok((vals, ins))
    }

    /// Transformed copy.
    pub fn transformed(&self, t: &Transform) -> PointCloud {
        PointCloud {
            points: self.points.iter().map(|p| t.apply(*p)).collect(),
        }
    }
}
