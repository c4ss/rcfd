//! Helpers shared by the cell-based mesh readers, and the result type of the surface readers ([`SurfaceImport`],
//! F-280, ADR 0184).

use rcfd_core::units::{Dimension, Unit};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::surface::TriSurface;
use rcfd_mesh::{BoundarySpec, Mesh, PatchKind};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A triangulated surface read from a surface file (OBJ, glTF), with one patch per named object (ADR 0184 D2).
///
/// Coordinates are in metres. Triangle tag `k` is patch `patches[k]`. Boundary types are not part of the result: the
/// readers never guess them (ADR 0184 D4).
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceImport {
    /// Welded surface: bitwise-equal vertices merged, unused vertices dropped.
    pub surface: TriSurface,
    /// Patch names in order of first appearance; only patches that have triangles.
    pub patches: Vec<String>,
    /// What was ignored, merged or triangulated, as counts (not one line per item).
    pub notes: Vec<String>,
}

impl SurfaceImport {
    /// Tag-to-name map in the form `TetMesh::to_mesh` takes.
    pub fn patch_names(&self) -> BTreeMap<u32, String> {
        self.patches
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u32, n.clone()))
            .collect()
    }

    /// Triangles of one patch as coordinate triples.
    pub fn patch_triangles(&self, tag: u32) -> Vec<[Vec3; 3]> {
        (0..self.surface.n_tris())
            .filter(|&i| self.surface.tags[i] == tag)
            .map(|i| self.surface.tri_points(i))
            .collect()
    }
}

/// Factor from a length unit to metres (`"m"`, `"mm"`, `"cm"`, `"in"`, `"ft"`, ...), parsed by
/// [`rcfd_core::units::Unit::parse`]. Anything that is not a length with zero offset is an error.
pub fn length_factor(unit: &str) -> Result<f64> {
    let u = Unit::parse(unit.trim()).map_err(|e| Error::Io(format!("length unit '{unit}': {e}")))?;
    if u.dim != Dimension::LENGTH || u.offset != 0.0 || !u.factor.is_finite() || u.factor <= 0.0 {
        return Err(Error::Io(format!(
            "'{unit}' is not a length unit (for example m, mm, cm, in, ft)"
        )));
    }
    Ok(u.factor)
}

/// Build a [`SurfaceImport`] from raw reader output (ADR 0184 D7): merge bitwise-equal vertices (`-0.0` equals `0.0`;
/// the first used occurrence in vertex order is kept), drop vertices no triangle uses, drop names with no triangle and
/// renumber the tags in name order. `what` names the format in the error for an empty result.
pub fn finish_import(
    points: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
    tags: Vec<u32>,
    names: Vec<String>,
    mut notes: Vec<String>,
    what: &str,
) -> Result<SurfaceImport> {
    if tris.is_empty() {
        return Err(Error::Io(format!("{what}: the file has no triangles")));
    }
    let mut used = vec![false; points.len()];
    for t in &tris {
        for &v in t {
            used[v] = true;
        }
    }
    let mut remap = vec![usize::MAX; points.len()];
    let mut key_to_new: HashMap<[u64; 3], usize> = HashMap::new();
    let mut new_points: Vec<Vec3> = Vec::new();
    let (mut welded, mut unused) = (0usize, 0usize);
    for (i, p) in points.iter().enumerate() {
        if !used[i] {
            unused += 1;
            continue;
        }
        let key = [(p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits(), (p[2] + 0.0).to_bits()];
        remap[i] = match key_to_new.get(&key) {
            Some(&j) => {
                welded += 1;
                j
            }
            None => {
                new_points.push(*p);
                key_to_new.insert(key, new_points.len() - 1);
                new_points.len() - 1
            }
        };
    }
    let mut count = vec![0usize; names.len()];
    for &t in &tags {
        count[t as usize] += 1;
    }
    let mut tag_map = vec![u32::MAX; names.len()];
    let mut patches = Vec::new();
    for (k, n) in names.iter().enumerate() {
        if count[k] > 0 {
            tag_map[k] = patches.len() as u32;
            patches.push(n.clone());
        }
    }
    let empty = names.len() - patches.len();
    let new_tris: Vec<[usize; 3]> = tris.iter().map(|t| [remap[t[0]], remap[t[1]], remap[t[2]]]).collect();
    let new_tags: Vec<u32> = tags.iter().map(|&t| tag_map[t as usize]).collect();
    if welded > 0 {
        notes.push(format!("{welded} vertices with bitwise-equal coordinates merged"));
    }
    if unused > 0 {
        notes.push(format!("{unused} vertices used by no triangle dropped"));
    }
    if empty > 0 {
        notes.push(format!("{empty} named objects with no triangles dropped"));
    }
    let mut surface = TriSurface::new(new_points, new_tris);
    surface.tags = new_tags;
    Ok(SurfaceImport {
        surface,
        patches,
        notes,
    })
}

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
