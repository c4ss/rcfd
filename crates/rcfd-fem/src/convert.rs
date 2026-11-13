//! Conversion of the repository's meshes to a [`SolidMesh`] (ADR 0272, registered and filled by ADR 0413): a finite-volume
//! [`rcfd_mesh::Mesh`] whose cells are all tetrahedra or all hexahedra ([`from_mesh`], the node order of each element
//! recovered from the face lists), the tetrahedral mesher's [`TetMesh`] ([`from_tet_mesh`]), the split of hexahedra into
//! tetrahedra ([`split_hexahedra`]) and the projection of mid-edge nodes onto a curved surface ([`project_mid_edge_nodes`],
//! [`onto_cylinder`]).
//!
//! The conversion lives in `rcfd-fem` because `rcfd-fem` depends on `rcfd-mesh` and not the converse (the crate graph stays
//! acyclic). Boundary patches become surface sets (normals out of the solid) and node sets of the patch's name; cell zones
//! become element sets; every converted mesh has the element set `all` and the node set `all`. Prisms, pyramids and general
//! polyhedra are refused naming the cell ([`FemError::UnsupportedCell`]), never split or approximated.

use crate::builders::MidEdgeNode;
use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, FaceRef, SolidMesh};
use rcfd_core::vec3::{add, dot, norm, scale, sub, Vec3};
use rcfd_mesh::tetmesh::TetMesh;
use rcfd_mesh::Mesh;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Tet,
    Hex,
}

// The loop of face `f` oriented out of `cell` (the loop of a face is outward for its owner).
fn outward_loop(mesh: &Mesh, cell: usize, f: usize) -> Vec<usize> {
    let mut l = mesh.face_points(f).to_vec();
    if mesh.owner()[f] != cell {
        l.reverse();
    }
    l
}

fn sorted(v: &[usize]) -> Vec<usize> {
    let mut k = v.to_vec();
    k.sort_unstable();
    k
}

// Hex8 node order from six outward quadrilateral loops (ADR 0413 D2), or None if they do not form a hexahedron.
fn hex_nodes(loops: &[Vec<usize>]) -> Option<Vec<usize>> {
    let b = &loops[0];
    // Local face 0 of Hex8 is [0, 3, 2, 1], outward.
    let mut n = vec![b[0], b[3], b[2], b[1]];
    let mut edges = BTreeSet::new();
    for l in loops {
        for i in 0..l.len() {
            let (p, q) = (l[i], l[(i + 1) % l.len()]);
            edges.insert((p.min(q), p.max(q)));
        }
    }
    if edges.len() != 12 {
        return None;
    }
    for i in 0..4 {
        let v = n[i];
        let ups: Vec<usize> = edges
            .iter()
            .filter_map(|&(p, q)| {
                if p == v {
                    Some(q)
                } else if q == v {
                    Some(p)
                } else {
                    None
                }
            })
            .filter(|w| !b.contains(w))
            .collect();
        if ups.len() != 1 {
            return None;
        }
        n.push(ups[0]);
    }
    if n.iter().collect::<BTreeSet<_>>().len() != 8 {
        return None;
    }
    let mut want: Vec<Vec<usize>> = ElementKind::Hex8
        .faces()
        .iter()
        .map(|f| sorted(&f.iter().map(|&i| n[i]).collect::<Vec<_>>()))
        .collect();
    let mut have: Vec<Vec<usize>> = loops.iter().map(|l| sorted(l)).collect();
    want.sort();
    have.sort();
    (want == have).then_some(n)
}

// Shape and element node order of cell `c` (ADR 0413 D2).
fn cell_nodes(mesh: &Mesh, c: usize) -> Result<(Shape, Vec<usize>)> {
    let faces = mesh.cell_faces(c);
    let nf = faces.len();
    let loops: Vec<Vec<usize>> = faces.iter().map(|&f| outward_loop(mesh, c, f)).collect();
    let points: BTreeSet<usize> = loops.iter().flatten().copied().collect();
    let n3 = loops.iter().filter(|l| l.len() == 3).count();
    let n4 = loops.iter().filter(|l| l.len() == 4).count();
    let refuse = |shape: &'static str| FemError::UnsupportedCell {
        cell: c,
        shape,
        faces: nf,
    };
    match (nf, n3, n4, points.len()) {
        (4, 4, 0, 4) => {
            let l = &loops[0];
            let t = *points.iter().find(|p| !l.contains(p)).expect("four points");
            // Local face 0 of Tet4 is [0, 2, 1], outward.
            Ok((Shape::Tet, vec![l[0], l[2], l[1], t]))
        }
        (6, 0, 6, 8) => hex_nodes(&loops)
            .map(|n| (Shape::Hex, n))
            .ok_or_else(|| refuse("polyhedron")),
        (5, 2, 3, 6) => Err(refuse("prism")),
        (5, 4, 1, 5) => Err(refuse("pyramid")),
        _ => Err(refuse("polyhedron")),
    }
}

fn add_all_sets(out: &mut SolidMesh) -> Result<()> {
    let (nn, ne) = (out.nodes().len(), out.n_elements());
    out.add_node_set("all", (0..nn).collect())?;
    out.add_element_set("all", (0..ne).collect())
}

// The one element face whose sorted corner nodes are `key`.
fn boundary_face(inc: &BTreeMap<Vec<usize>, Vec<FaceRef>>, key: &[usize], what: &str) -> Result<FaceRef> {
    match inc.get(key) {
        Some(m) if m.len() == 1 => Ok(m[0]),
        other => Err(FemError::InvalidValue {
            key: "patch",
            reason: format!(
                "{what} (nodes {key:?}) is not the face of exactly one element ({} elements)",
                other.map_or(0, |m| m.len())
            ),
        }),
    }
}

/// A [`SolidMesh`] from a finite-volume mesh whose cells are all tetrahedra or all hexahedra (ADR 0413 D2). Node `i` is
/// point `i` (same coordinates), element `i` is cell `i`, in one Tet4 or Hex8 block; the node order of each element is
/// recovered from the cell's face loops (topologically: warped cells are recovered as well; an element the order makes
/// inverted is refused by [`SolidMesh::add_block`]). Each patch with at least one face gives a surface set and a node set of
/// its name; each cell zone with at least one cell an element set of its name; plus the sets `all`.
///
/// Refused: a prism, a pyramid or any other polyhedron, and a cell of the other shape than cell 0
/// ([`FemError::UnsupportedCell`] naming the cell); a mesh without cells; a patch or zone named `all` (a duplicate set).
pub fn from_mesh(mesh: &Mesh) -> Result<SolidMesh> {
    let nc = mesh.n_cells();
    if nc == 0 {
        return Err(FemError::InvalidValue {
            key: "mesh",
            reason: "the mesh has no cell".into(),
        });
    }
    let mut conn = Vec::new();
    let mut first = None;
    for c in 0..nc {
        let (s, nodes) = cell_nodes(mesh, c)?;
        match first {
            None => first = Some(s),
            Some(s0) if s0 != s => {
                return Err(FemError::UnsupportedCell {
                    cell: c,
                    shape: match s {
                        Shape::Tet => "tetrahedron in a mesh of hexahedra",
                        Shape::Hex => "hexahedron in a mesh of tetrahedra",
                    },
                    faces: mesh.cell_faces(c).len(),
                })
            }
            _ => {}
        }
        conn.extend(nodes);
    }
    let kind = match first.expect("one cell") {
        Shape::Tet => ElementKind::Tet4,
        Shape::Hex => ElementKind::Hex8,
    };
    let mut out = SolidMesh::new(mesh.points().to_vec())?;
    out.add_block(kind, conn)?;
    add_all_sets(&mut out)?;
    let inc = out.face_incidence();
    for p in mesh.patches() {
        if p.size == 0 {
            continue;
        }
        let mut faces = Vec::with_capacity(p.size);
        let mut nodes = BTreeSet::new();
        for f in p.faces() {
            let fp = mesh.face_points(f);
            nodes.extend(fp.iter().copied());
            faces.push(boundary_face(
                &inc,
                &sorted(fp),
                &format!("face {f} of patch '{}'", p.name),
            )?);
        }
        out.add_surface_set(&p.name, faces)?;
        out.add_node_set(&p.name, nodes.into_iter().collect())?;
    }
    for z in mesh.cell_zones() {
        if !z.ids.is_empty() {
            out.add_element_set(&z.name, z.ids.clone())?;
        }
    }
    Ok(out)
}

/// A [`SolidMesh`] of Tet4 from the tetrahedral mesher's output (ADR 0413 D3): nodes `tm.points`, element `i` the cell `i`
/// (positively oriented as `TetMesh` stores it, the Tet4 convention). The boundary triangles give a surface set and a node set
/// per tag, named `names[tag]` or, as [`TetMesh::to_mesh`] names its patches, `wall` for tag 0 and `wall<k>` for tag `k`; plus
/// the sets `all`.
pub fn from_tet_mesh(tm: &TetMesh, names: &BTreeMap<u32, String>) -> Result<SolidMesh> {
    let mut out = SolidMesh::new(tm.points.clone())?;
    out.add_block(ElementKind::Tet4, tm.cells.iter().flatten().copied().collect())?;
    add_all_sets(&mut out)?;
    let inc = out.face_incidence();
    let mut by_tag: BTreeMap<u32, (Vec<FaceRef>, BTreeSet<usize>)> = BTreeMap::new();
    for (i, (t, &tag)) in tm.surface.tris.iter().zip(&tm.surface.tags).enumerate() {
        let f = boundary_face(&inc, &sorted(t), &format!("surface triangle {i} (tag {tag})"))?;
        let e = by_tag.entry(tag).or_default();
        e.0.push(f);
        e.1.extend(t.iter().copied());
    }
    for (tag, (faces, nodes)) in by_tag {
        let name = names.get(&tag).cloned().unwrap_or_else(|| {
            if tag == 0 {
                "wall".to_string()
            } else {
                format!("wall{tag}")
            }
        });
        out.add_surface_set(&name, faces)?;
        out.add_node_set(&name, nodes.into_iter().collect())?;
    }
    Ok(out)
}

const PERMUTATIONS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

fn det3(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
}

// The six Kuhn tetrahedra of a Hex8 (local node indices) along the diagonal from local corner `m` (ADR 0413 D4).
fn kuhn_local(m: usize) -> [[usize; 4]; 6] {
    let r = ElementKind::Hex8.reference_nodes();
    let index = |x: Vec3| r.iter().position(|y| *y == x).expect("a corner of the reference cube");
    let s = r[m];
    let mut out = [[0usize; 4]; 6];
    for (t, p) in PERMUTATIONS.iter().enumerate() {
        let mut v = [s; 4];
        v[1][p[0]] = -v[1][p[0]];
        v[2] = v[1];
        v[2][p[1]] = -v[2][p[1]];
        v[3] = [-s[0], -s[1], -s[2]];
        if det3(sub(v[1], v[0]), sub(v[2], v[0]), sub(v[3], v[0])) < 0.0 {
            v.swap(1, 2);
        }
        out[t] = [index(v[0]), index(v[1]), index(v[2]), index(v[3])];
    }
    out
}

/// Splits every Hex8 into six Tet4 (ADR 0413 D4): element `e` becomes elements `6 e .. 6 e + 5`, the Kuhn split along the
/// diagonal from the corner of smallest global node index to its opposite corner. Nodes and node sets are kept; element sets
/// map `e` to its six tetrahedra; a surface face of `e` becomes the two faces of its tetrahedra lying on it. Block properties
/// are not carried (a Hex8 formulation has no meaning for Tet4).
///
/// Two neighbours cut their common face along the same diagonal when its smallest node lies on the diagonal the split
/// chooses (true for the lexicographic numbering of the structured generators); the result is checked and a non-conforming
/// split is refused naming the hexahedron, as is a block of another kind than Hex8.
pub fn split_hexahedra(mesh: &SolidMesh) -> Result<SolidMesh> {
    for (block, b) in mesh.blocks().iter().enumerate() {
        if b.kind() != ElementKind::Hex8 {
            return Err(FemError::UnsupportedKind {
                block,
                kind: b.kind().name(),
                operation: "split_hexahedra",
            });
        }
    }
    let tables: Vec<[[usize; 4]; 6]> = (0..8).map(kuhn_local).collect();
    let mut out = SolidMesh::new(mesh.nodes().to_vec())?;
    for b in mesh.blocks() {
        let mut conn = Vec::with_capacity(b.len() * 24);
        for h in b.connectivity().chunks(8) {
            let m = (0..8).min_by_key(|&i| h[i]).expect("eight corners");
            for t in &tables[m] {
                conn.extend(t.iter().map(|&l| h[l]));
            }
        }
        out.add_block(ElementKind::Tet4, conn)?;
    }
    // Conformity (ADR 0413 D4): no face of three elements, and every unmatched tetrahedron face on a boundary face of its
    // own hexahedron.
    let mut boundary: BTreeMap<usize, Vec<Vec<usize>>> = BTreeMap::new();
    let mut n_boundary = 0;
    for (key, m) in mesh.face_incidence() {
        if m.len() == 1 {
            boundary.entry(m[0].element).or_default().push(key);
            n_boundary += 1;
        }
    }
    let mut n_free = 0;
    for (key, m) in out.face_incidence() {
        let hex = m[0].element / 6;
        let on_boundary = || {
            boundary
                .get(&hex)
                .is_some_and(|qs| qs.iter().any(|q| key.iter().all(|v| q.contains(v))))
        };
        if m.len() > 2 || (m.len() == 1 && !on_boundary()) {
            return Err(FemError::InvalidValue {
                key: "split_hexahedra",
                reason: format!(
                    "element {hex}: its split does not conform with its neighbour at the face through nodes {key:?} \
                     (the face's smallest node is off the diagonal the split chooses)"
                ),
            });
        }
        n_free += usize::from(m.len() == 1);
    }
    if n_free != 2 * n_boundary {
        return Err(FemError::InvalidValue {
            key: "split_hexahedra",
            reason: format!("{n_free} boundary triangles for {n_boundary} boundary quadrilaterals"),
        });
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.element_set_names() {
        let set = mesh.element_set(name).expect("listed");
        out.add_element_set(name, set.iter().flat_map(|&e| 6 * e..6 * e + 6).collect())?;
    }
    for name in mesh.surface_set_names() {
        let mut faces = Vec::new();
        for f in mesh.surface_set(name).expect("listed") {
            let quad = mesh.face_nodes(*f);
            let before = faces.len();
            for element in 6 * f.element..6 * f.element + 6 {
                for face in 0..ElementKind::Tet4.n_faces() {
                    let fr = FaceRef { element, face };
                    if out.face_nodes(fr).iter().all(|v| quad.contains(v)) {
                        faces.push(fr);
                    }
                }
            }
            if faces.len() - before != 2 {
                return Err(FemError::InvalidValue {
                    key: "split_hexahedra",
                    reason: format!(
                        "surface set '{name}': face {} of element {} is covered by {} triangles",
                        f.face,
                        f.element,
                        faces.len() - before
                    ),
                });
            }
        }
        out.add_surface_set(name, faces)?;
    }
    Ok(out)
}

/// Moves mid-edge nodes (ADR 0413 D5): for each `m` of `mids` (as [`crate::builders::upgrade_to_tet10`] or
/// [`crate::hex20::upgrade_to_hex20`] return them), `project(&m, x)` with `x` the node's position gives its new position, or
/// `None` to leave it. The mesh is rebuilt through [`SolidMesh::new`] and [`SolidMesh::add_block`] (ADR 0271), so `det J` is
/// checked again at every node: an element the projection inverts is refused ([`FemError::InvertedElement`]), not repaired,
/// and a non-finite position is refused ([`FemError::NonFiniteNode`]). Block properties and every set are kept. A node index
/// out of range is refused.
pub fn project_mid_edge_nodes<F>(mesh: &SolidMesh, mids: &[MidEdgeNode], mut project: F) -> Result<SolidMesh>
where
    F: FnMut(&MidEdgeNode, Vec3) -> Option<Vec3>,
{
    let mut nodes = mesh.nodes().to_vec();
    for m in mids {
        if m.node >= nodes.len() {
            return Err(FemError::InvalidValue {
                key: "mids",
                reason: format!("node {} does not exist (the mesh has {} nodes)", m.node, nodes.len()),
            });
        }
        if let Some(q) = project(m, nodes[m.node]) {
            nodes[m.node] = q;
        }
    }
    let mut out = SolidMesh::new(nodes)?;
    for (block, b) in mesh.blocks().iter().enumerate() {
        out.add_block(b.kind(), b.connectivity().to_vec())?;
        if let Some(p) = b.property() {
            out.set_block_property(block, p.clone())?;
        }
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    Ok(out)
}

/// The radial projection of `p` onto the cylinder of `radius` about the axis through `origin` along `axis` (need not be a
/// unit vector): the axial part of `p - origin` kept, the radial part scaled to length `radius`. A point on the axis gives a
/// non-finite result (refused by [`project_mid_edge_nodes`]).
pub fn onto_cylinder(p: Vec3, origin: Vec3, axis: Vec3, radius: f64) -> Vec3 {
    let a = scale(axis, 1.0 / norm(axis));
    let d = sub(p, origin);
    let axial = scale(a, dot(d, a));
    let radial = sub(d, axial);
    add(origin, add(axial, scale(radial, radius / norm(radial))))
}
