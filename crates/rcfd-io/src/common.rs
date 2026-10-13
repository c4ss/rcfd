//! Helpers shared by the cell-based mesh readers.

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::collections::{HashMap, HashSet};

/// Guess a patch kind from a conventional patch name.
pub fn guess_kind(name: &str) -> PatchKind {
    let n = name.to_ascii_lowercase();
    if n.contains("wall") {
        PatchKind::Wall
    } else if n.contains("inlet") || n.contains("inflow") {
        PatchKind::Inlet
    } else if n.contains("outlet") || n.contains("outflow") {
        PatchKind::Outlet
    } else if n.contains("symm") {
        PatchKind::Symmetry
    } else if n.contains("empty") || n.contains("frontandback") {
        PatchKind::Empty
    } else {
        PatchKind::Generic
    }
}

/// Faces of the supported linear cell shapes, as local node-index loops.
pub fn cell_face_template(n_nodes: usize, shape: Shape) -> Option<&'static [&'static [usize]]> {
    const TET: &[&[usize]] = &[&[0, 1, 2], &[0, 1, 3], &[0, 2, 3], &[1, 2, 3]];
    const HEX: &[&[usize]] = &[
        &[0, 1, 2, 3],
        &[4, 5, 6, 7],
        &[0, 1, 5, 4],
        &[1, 2, 6, 5],
        &[2, 3, 7, 6],
        &[3, 0, 4, 7],
    ];
    const PRISM: &[&[usize]] = &[&[0, 1, 2], &[3, 4, 5], &[0, 1, 4, 3], &[1, 2, 5, 4], &[2, 0, 3, 5]];
    const PYR: &[&[usize]] = &[&[0, 1, 2, 3], &[0, 1, 4], &[1, 2, 4], &[2, 3, 4], &[3, 0, 4]];
    match (shape, n_nodes) {
        (Shape::Tet, 4) => Some(TET),
        (Shape::Hex, 8) => Some(HEX),
        (Shape::Prism, 6) => Some(PRISM),
        (Shape::Pyramid, 5) => Some(PYR),
        _ => None,
    }
}

/// Linear 3D cell shapes understood by the readers (Gmsh node ordering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Tetrahedron.
    Tet,
    /// Hexahedron.
    Hex,
    /// Triangular prism / wedge.
    Prism,
    /// Square pyramid.
    Pyramid,
}

/// A named group of surface faces (point-index loops).
pub struct SurfaceGroup {
    /// Group name.
    pub name: String,
    /// Faces as point-index loops.
    pub faces: Vec<Vec<usize>>,
    /// Explicit patch kind; guessed from the name when `None`.
    pub kind: Option<PatchKind>,
}

/// Build a mesh from cells (each a list of face loops) plus named surface and
/// cell groups. Surface faces that turn out to be interior become face zones;
/// boundary ones become patches (in group order); the rest go to
/// `defaultFaces`. Cell groups become cell zones.
pub fn build_mesh(
    points: Vec<Vec3>,
    cells: Vec<Vec<Vec<usize>>>,
    surfaces: Vec<SurfaceGroup>,
    cell_groups: Vec<(String, Vec<usize>)>,
) -> Result<Mesh> {
    fn key(f: &[usize]) -> Vec<usize> {
        let mut k = f.to_vec();
        k.sort_unstable();
        k
    }
    let mut count: HashMap<Vec<usize>, u32> = HashMap::new();
    for c in &cells {
        for f in c {
            *count.entry(key(f)).or_default() += 1;
        }
    }
    let mut seen: HashSet<Vec<usize>> = HashSet::new();
    let mut specs = Vec::new();
    let mut interior_groups: Vec<(String, Vec<Vec<usize>>)> = Vec::new();
    for g in surfaces {
        let mut bnd = Vec::new();
        let mut int = Vec::new();
        for f in g.faces {
            let k = key(&f);
            match count.get(&k) {
                Some(1) => {
                    if seen.insert(k) {
                        bnd.push(f);
                    }
                }
                Some(_) => int.push(f),
                None => {
                    return Err(Error::Mesh(format!(
                        "surface group '{}' has a face that belongs to no cell",
                        g.name
                    )))
                }
            }
        }
        if !bnd.is_empty() {
            specs.push(BoundarySpec {
                kind: g.kind.clone().unwrap_or_else(|| guess_kind(&g.name)),
                name: g.name.clone(),
                faces: bnd,
            });
        }
        if !int.is_empty() {
            interior_groups.push((g.name, int));
        }
    }
    let mut mesh = Mesh::from_cell_faces(points, &cells, &specs)?;
    if !interior_groups.is_empty() {
        let mut lookup: HashMap<Vec<usize>, usize> = HashMap::new();
        for f in 0..mesh.n_internal_faces() {
            lookup.insert(key(mesh.face_points(f)), f);
        }
        for (name, faces) in interior_groups {
            let ids: Vec<usize> = faces.iter().filter_map(|f| lookup.get(&key(f)).copied()).collect();
            mesh.add_face_zone(&name, ids)?;
        }
    }
    for (name, ids) in cell_groups {
        mesh.add_cell_zone(&name, ids)?;
    }
    Ok(mesh)
}
