//! Surface meshes for the render core: polygon faces grouped into named parts, plus scalar fields on them.

use rcfd_core::vec3::{add, cross, norm, scale, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Polygon surface with named parts (boundary patches or face zones of a volume mesh, or any hand-built surface).
#[derive(Debug, Clone, Default)]
pub struct SurfaceMesh {
    /// Surface points.
    pub points: Vec<Vec3>,
    /// CSR offsets into `face_points` (length = faces + 1).
    pub face_offsets: Vec<usize>,
    /// Concatenated face point loops (indices into `points`).
    pub face_points: Vec<usize>,
    /// Part index of each face.
    pub face_part: Vec<usize>,
    /// Part names (patch or zone names).
    pub parts: Vec<String>,
    /// Volume-mesh face id of each face, when built from a mesh.
    pub mesh_face: Vec<Option<usize>>,
    /// Owner cell of each face, when built from a mesh.
    pub cell: Vec<Option<usize>>,
    /// Volume-mesh point id of each surface point (identity for a hand-built surface).
    pub point_id: Vec<usize>,
}

/// Scalar field on a surface.
#[derive(Debug, Clone, Copy)]
pub enum Field<'a> {
    /// No field: parts are drawn grey (or in their solid colour).
    None,
    /// One value per surface face (flat colouring).
    Face(&'a [f64]),
    /// One value per surface point (interpolated over each triangle, then colour mapped).
    Vertex(&'a [f64]),
}

impl SurfaceMesh {
    /// Build from point loops. Every face needs at least three valid point indices and a valid part index.
    pub fn new(
        points: Vec<Vec3>,
        faces: &[Vec<usize>],
        face_part: Vec<usize>,
        parts: Vec<String>,
    ) -> Result<SurfaceMesh> {
        if face_part.len() != faces.len() {
            return Err(Error::Mesh(format!(
                "{} part indices for {} faces",
                face_part.len(),
                faces.len()
            )));
        }
        let mut s = SurfaceMesh {
            point_id: (0..points.len()).collect(),
            points,
            face_offsets: vec![0],
            parts,
            ..Default::default()
        };
        for (f, (loop_, &part)) in faces.iter().zip(&face_part).enumerate() {
            if loop_.len() < 3 || loop_.iter().any(|&p| p >= s.points.len()) || part >= s.parts.len() {
                return Err(Error::Mesh(format!(
                    "face {f}: needs >= 3 valid points and a valid part"
                )));
            }
            s.push_face(loop_, part, None, None);
        }
        Ok(s)
    }

    /// Boundary patches and face zones of a volume mesh, by name; an empty list takes every patch.
    /// A name is looked up as a patch first, then as a face zone. Face zone faces keep their owner cell.
    pub fn from_mesh(mesh: &Mesh, names: &[&str]) -> Result<SurfaceMesh> {
        let all: Vec<&str> = mesh.patches().iter().map(|p| p.name.as_str()).collect();
        let names = if names.is_empty() { &all[..] } else { names };
        let mut s = SurfaceMesh {
            face_offsets: vec![0],
            ..Default::default()
        };
        let mut map = vec![usize::MAX; mesh.n_points()];
        for (part, name) in names.iter().enumerate() {
            let faces: Vec<usize> = if let Some(p) = mesh.patch(name) {
                p.faces().collect()
            } else if let Some(z) = mesh.face_zones().iter().find(|z| z.name == *name) {
                z.ids.clone()
            } else {
                return Err(Error::Mesh(format!("no patch or face zone named '{name}'")));
            };
            s.parts.push(name.to_string());
            for f in faces {
                let ids = mesh.face_points(f);
                if ids.len() < 3 {
                    continue;
                }
                let loop_: Vec<usize> = ids
                    .iter()
                    .map(|&p| {
                        if map[p] == usize::MAX {
                            map[p] = s.points.len();
                            s.points.push(mesh.points()[p]);
                            s.point_id.push(p);
                        }
                        map[p]
                    })
                    .collect();
                s.push_face(&loop_, part, Some(f), Some(mesh.owner()[f]));
            }
        }
        Ok(s)
    }

    fn push_face(&mut self, loop_: &[usize], part: usize, mesh_face: Option<usize>, cell: Option<usize>) {
        self.face_points.extend_from_slice(loop_);
        self.face_offsets.push(self.face_points.len());
        self.face_part.push(part);
        self.mesh_face.push(mesh_face);
        self.cell.push(cell);
    }

    /// Number of faces.
    pub fn n_faces(&self) -> usize {
        self.face_part.len()
    }

    /// Point loop of face `f`.
    pub fn face(&self, f: usize) -> &[usize] {
        &self.face_points[self.face_offsets[f]..self.face_offsets[f + 1]]
    }

    /// Fan triangles `(face, [a, b, c])` from the first point of each face.
    // shortcut: a fan is only right for convex faces; upgrade to ear clipping when non-convex faces appear.
    pub fn triangles(&self) -> impl Iterator<Item = (usize, [usize; 3])> + '_ {
        (0..self.n_faces()).flat_map(move |f| {
            let l = self.face(f);
            (1..l.len() - 1).map(move |k| (f, [l[0], l[k], l[k + 1]]))
        })
    }

    /// Area vector of face `f` (Newell's formula; length = area for a planar face).
    pub fn face_area_vector(&self, f: usize) -> Vec3 {
        let l = self.face(f);
        let mut a = [0.0; 3];
        for k in 0..l.len() {
            a = add(a, cross(self.points[l[k]], self.points[l[(k + 1) % l.len()]]));
        }
        scale(a, 0.5)
    }

    /// Unit normal of every point: the normalised sum of the area vectors of the faces around it.
    pub fn point_normals(&self) -> Vec<Vec3> {
        let mut n = vec![[0.0; 3]; self.points.len()];
        for f in 0..self.n_faces() {
            let a = self.face_area_vector(f);
            for &p in self.face(f) {
                n[p] = add(n[p], a);
            }
        }
        n.iter().map(|v| scale(*v, 1.0 / norm(*v).max(1e-300))).collect()
    }

    /// Bounding box `(lo, hi)` of the points.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &self.points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    /// Face values from a cell field (each face takes its owner cell's value; the rule of the glTF export).
    pub fn face_values_from_cells(&self, cells: &[f64]) -> Result<Vec<f64>> {
        self.cell
            .iter()
            .map(|c| {
                c.and_then(|c| cells.get(c).copied())
                    .ok_or_else(|| Error::Other("face without an owner cell in the cell field".into()))
            })
            .collect()
    }

    /// Check a field against the surface and return its min and max (`None` for [`Field::None`]).
    pub fn field_range(&self, field: Field) -> Result<Option<(f64, f64)>> {
        let (v, n, what) = match field {
            Field::None => return Ok(None),
            Field::Face(v) => (v, self.n_faces(), "face"),
            Field::Vertex(v) => (v, self.points.len(), "vertex"),
        };
        if v.len() != n {
            return Err(Error::InvalidValue {
                path: "field".into(),
                reason: format!("{} {what} values for {n} {what}s", v.len()),
            });
        }
        if let Some(i) = v.iter().position(|x| !x.is_finite()) {
            return Err(Error::Numerical(format!("non-finite {what} value at {i}")));
        }
        Ok(Some(v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| {
            (a.0.min(x), a.1.max(x))
        })))
    }
}
