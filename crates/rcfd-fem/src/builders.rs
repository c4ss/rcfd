//! Structured builders (ADR 0271 D4): a box of hexahedra, the same box in six tetrahedra per hexahedron (the Kuhn split
//! along the diagonal from the lowest to the highest corner, conforming), and the upgrade of linear to quadratic
//! tetrahedra by mid-edge nodes.
//!
//! The boxes have the element set `all`, the node set `all`, and for each face of the box a node set and a surface set
//! named `xmin, xmax, ymin, ymax, zmin, zmax` (the patch names of the `rcfd_mesh::gen` box).

use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, FaceRef, SolidMesh, TET_EDGES};
use rcfd_core::vec3::Vec3;
use std::collections::{BTreeSet, HashMap};

/// Names of the six box faces, in the order x min, x max, y min, y max, z min, z max.
pub const BOX_FACES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

struct Grid {
    n: [usize; 3],
}

impl Grid {
    fn node(&self, i: usize, j: usize, k: usize) -> usize {
        i + (self.n[0] + 1) * (j + (self.n[1] + 1) * k)
    }
    fn ijk(&self, node: usize) -> [usize; 3] {
        let i = node % (self.n[0] + 1);
        let r = node / (self.n[0] + 1);
        [i, r % (self.n[1] + 1), r / (self.n[1] + 1)]
    }
    // Whether a node lies on box face `f` (index into BOX_FACES).
    fn on_face(&self, node: usize, f: usize) -> bool {
        let g = self.ijk(node);
        let axis = f / 2;
        if f % 2 == 0 {
            g[axis] == 0
        } else {
            g[axis] == self.n[axis]
        }
    }
}

fn box_nodes(origin: Vec3, lengths: Vec3, divisions: [usize; 3]) -> Result<(Grid, Vec<Vec3>)> {
    if origin.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "origin",
            reason: format!("{origin:?} is not finite"),
        });
    }
    if lengths.iter().any(|&l| !(l > 0.0 && l.is_finite())) {
        return Err(FemError::InvalidValue {
            key: "lengths",
            reason: format!("{lengths:?}: each must be positive"),
        });
    }
    if divisions.contains(&0) {
        return Err(FemError::InvalidValue {
            key: "divisions",
            reason: format!("{divisions:?}: each must be >= 1"),
        });
    }
    let g = Grid { n: divisions };
    let [nx, ny, nz] = divisions;
    let mut nodes = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                nodes.push([
                    origin[0] + lengths[0] * (i as f64 / nx as f64),
                    origin[1] + lengths[1] * (j as f64 / ny as f64),
                    origin[2] + lengths[2] * (k as f64 / nz as f64),
                ]);
            }
        }
    }
    Ok((g, nodes))
}

fn add_box_node_sets(mesh: &mut SolidMesh, g: &Grid) -> Result<()> {
    let n = mesh.nodes().len();
    mesh.add_node_set("all", (0..n).collect())?;
    for (f, name) in BOX_FACES.iter().enumerate() {
        mesh.add_node_set(name, (0..n).filter(|&v| g.on_face(v, f)).collect())?;
    }
    Ok(())
}

/// A box `[origin, origin + lengths]` of `divisions[0] x divisions[1] x divisions[2]` hexahedra (Hex8).
pub fn hex_box(origin: Vec3, lengths: Vec3, divisions: [usize; 3]) -> Result<SolidMesh> {
    let (g, nodes) = box_nodes(origin, lengths, divisions)?;
    let [nx, ny, nz] = divisions;
    let mut conn = Vec::with_capacity(8 * nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                conn.extend_from_slice(&[
                    g.node(i, j, k),
                    g.node(i + 1, j, k),
                    g.node(i + 1, j + 1, k),
                    g.node(i, j + 1, k),
                    g.node(i, j, k + 1),
                    g.node(i + 1, j, k + 1),
                    g.node(i + 1, j + 1, k + 1),
                    g.node(i, j + 1, k + 1),
                ]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Hex8, conn)?;
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect())?;
    add_box_node_sets(&mut mesh, &g)?;
    // Local face of the hexahedron on each box face (ADR 0271 D2): xi -1 -> 5, xi +1 -> 3, eta -1 -> 2, eta +1 -> 4,
    // zeta -1 -> 0, zeta +1 -> 1.
    let local = [5, 3, 2, 4, 0, 1];
    for (f, name) in BOX_FACES.iter().enumerate() {
        let mut faces = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let c = [i, j, k];
                    let axis = f / 2;
                    let on = if f % 2 == 0 {
                        c[axis] == 0
                    } else {
                        c[axis] == divisions[axis] - 1
                    };
                    if on {
                        faces.push(FaceRef {
                            element: i + nx * (j + ny * k),
                            face: local[f],
                        });
                    }
                }
            }
        }
        mesh.add_surface_set(name, faces)?;
    }
    Ok(mesh)
}

// The six axis permutations in lexicographic order with their parity (true for odd).
const KUHN: [([usize; 3], bool); 6] = [
    ([0, 1, 2], false),
    ([0, 2, 1], true),
    ([1, 0, 2], true),
    ([1, 2, 0], false),
    ([2, 0, 1], false),
    ([2, 1, 0], true),
];

/// The box of [`hex_box`] with each hexahedron split into six tetrahedra (Tet4) along its diagonal from the lowest to
/// the highest corner (the Kuhn split, conforming between cells); element `6 c + t` is tetrahedron `t` of cell `c`.
pub fn tet_box(origin: Vec3, lengths: Vec3, divisions: [usize; 3]) -> Result<SolidMesh> {
    let (g, nodes) = box_nodes(origin, lengths, divisions)?;
    let [nx, ny, nz] = divisions;
    let mut conn = Vec::with_capacity(24 * nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                for (p, odd) in KUHN {
                    let mut c = [i, j, k];
                    let mut v = [0usize; 4];
                    v[0] = g.node(c[0], c[1], c[2]);
                    for s in 0..3 {
                        c[p[s]] += 1;
                        v[s + 1] = g.node(c[0], c[1], c[2]);
                    }
                    if odd {
                        v.swap(1, 2);
                    }
                    conn.extend_from_slice(&v);
                }
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Tet4, conn)?;
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect())?;
    add_box_node_sets(&mut mesh, &g)?;
    for (f, name) in BOX_FACES.iter().enumerate() {
        let mut faces = Vec::new();
        for element in 0..ne {
            for face in 0..ElementKind::Tet4.n_faces() {
                let fr = FaceRef { element, face };
                if mesh.face_nodes(fr).iter().all(|&v| g.on_face(v, f)) {
                    faces.push(fr);
                }
            }
        }
        mesh.add_surface_set(name, faces)?;
    }
    Ok(mesh)
}

/// A mid-edge node created by [`upgrade_to_tet10`]: the new node and the two corner nodes of its edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidEdgeNode {
    /// Index of the new node.
    pub node: usize,
    /// The ends of the edge, smaller index first.
    pub ends: [usize; 2],
}

/// Upgrades a mesh of linear tetrahedra to quadratic ones: one new node at the middle of each edge, numbered after the
/// existing nodes in the order of first appearance (element order, then local edge order). Elements keep their numbers,
/// element and surface sets their members; a node set gains the mid-edge node of each edge whose two ends it contains.
/// A block of another kind is refused.
pub fn upgrade_to_tet10(mesh: &SolidMesh) -> Result<(SolidMesh, Vec<MidEdgeNode>)> {
    for (block, b) in mesh.blocks().iter().enumerate() {
        if b.kind() != ElementKind::Tet4 {
            return Err(FemError::UnsupportedKind {
                block,
                kind: b.kind().name(),
                operation: "upgrade_to_tet10",
            });
        }
    }
    let mut nodes = mesh.nodes().to_vec();
    let mut edge_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut mids = Vec::new();
    let mut blocks = Vec::new();
    for b in mesh.blocks() {
        let mut conn10 = Vec::with_capacity(b.len() * 10);
        for t in b.connectivity().chunks(4) {
            conn10.extend_from_slice(t);
            for [la, lb] in TET_EDGES {
                let (a, c) = (t[la], t[lb]);
                let key = (a.min(c), a.max(c));
                let node = *edge_index.entry(key).or_insert_with(|| {
                    let (pa, pc) = (nodes[key.0], nodes[key.1]);
                    nodes.push([0.5 * (pa[0] + pc[0]), 0.5 * (pa[1] + pc[1]), 0.5 * (pa[2] + pc[2])]);
                    mids.push(MidEdgeNode {
                        node: nodes.len() - 1,
                        ends: [key.0, key.1],
                    });
                    nodes.len() - 1
                });
                conn10.push(node);
            }
        }
        blocks.push(conn10);
    }
    let mut out = SolidMesh::new(nodes)?;
    for conn10 in blocks {
        out.add_block(ElementKind::Tet10, conn10)?;
    }
    for name in mesh.node_set_names() {
        let set = mesh.node_set(name).expect("listed");
        let members: BTreeSet<usize> = set.iter().copied().collect();
        let mut v = set.to_vec();
        v.extend(
            mids.iter()
                .filter(|m| members.contains(&m.ends[0]) && members.contains(&m.ends[1]))
                .map(|m| m.node),
        );
        out.add_node_set(name, v)?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    Ok((out, mids))
}
