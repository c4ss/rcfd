//! Structured builders (ADR 0271 D4): a box of hexahedra, the same box in six tetrahedra per hexahedron (the Kuhn split
//! along the diagonal from the lowest to the highest corner, conforming), and the upgrade of linear to quadratic
//! tetrahedra by mid-edge nodes.
//!
//! The boxes have the element set `all`, the node set `all`, and for each face of the box a node set and a surface set
//! named `xmin, xmax, ymin, ymax, zmin, zmax` (the patch names of the `rcfd_mesh::gen` box).

use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, FaceRef, SolidMesh, TET_EDGES};
use rcfd_core::vec3::{cross, dot, sub, Vec3};
use rcfd_mesh::fan::{AxialFan, Side};
use rcfd_mesh::fan_design::ComputerFan;
use std::collections::{BTreeSet, HashMap};
use std::f64::consts::PI;

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

fn blade_error(key: &'static str, reason: impl Into<String>) -> FemError {
    FemError::InvalidValue {
        key,
        reason: reason.into(),
    }
}

/// The structured mesh of one fan blade in linear tetrahedra (ADR 0505, the construction of ADR 0500 D1).
///
/// `divisions = [n_c, n_t, n_s]` are the cells along the chord (cosine spaced, `n_c >= 2`), through the thickness and along
/// the span. The point `(i, k, j)` lies on the cylinder of radius `r_hub + L j / n_s`, between the suction (`k = 0`) and the
/// pressure (`k = n_t`) surface of [`AxialFan::surface_zs`]; the two surfaces meet at the closed leading and trailing
/// edges, where the thickness nodes merge. Each grid cell is split into the six Kuhn tetrahedra, those with a repeated node
/// are dropped, and each is oriented by its signed volume. Element count `6 (n_c - 1) n_t n_s`.
///
/// Sets: element set `all`; node sets `all`, `hub` (`r = r_hub`), `tip` (`r = r_tip`); surface sets (the boundary faces)
/// `hub`, `tip`, `pressure` (the pressure side with the leading and trailing edges, caps excluded) and `suction` (every
/// other boundary face). The four surface sets are disjoint and together are the whole boundary. Divisions with a zero,
/// `n_c < 2`, or a fan that fails [`AxialFan::validate`] are refused.
pub fn fan_blade_tet4(fan: &AxialFan, divisions: [usize; 3]) -> Result<SolidMesh> {
    let [nc, nt, ns] = divisions;
    if nc < 2 || nt == 0 || ns == 0 {
        return Err(blade_error(
            "divisions",
            format!("[n_c, n_t, n_s] = {divisions:?} needs n_c >= 2, n_t >= 1 and n_s >= 1"),
        ));
    }
    let problems = fan.validate();
    if let Some(p) = problems.first() {
        return Err(blade_error("fan", format!("the blade is not valid: {p:?}")));
    }
    let signed_volume = |p: [Vec3; 4]| dot(sub(p[1], p[0]), cross(sub(p[2], p[0]), sub(p[3], p[0]))) / 6.0;
    // idx[j][i][k]; at i = 0 and i = nc the thickness nodes merge into the k = 0 node.
    let mut nodes: Vec<Vec3> = Vec::new();
    let mut node_j: Vec<usize> = Vec::new();
    let mut node_press: Vec<bool> = Vec::new();
    let mut idx = vec![vec![vec![0usize; nt + 1]; nc + 1]; ns + 1];
    for (j, plane) in idx.iter_mut().enumerate() {
        let r = fan.r_hub + (fan.r_tip - fan.r_hub) * j as f64 / ns as f64;
        let st = fan.station_at(r);
        for (i, column) in plane.iter_mut().enumerate() {
            let x = 0.5 * (1.0 - (PI * i as f64 / nc as f64).cos());
            let (zu, su) = fan.surface_zs(&st, x, Side::Suction);
            let (zl, sl) = fan.surface_zs(&st, x, Side::Pressure);
            let edge = i == 0 || i == nc;
            for k in 0..=nt {
                if edge && k > 0 {
                    column[k] = column[0];
                    continue;
                }
                let w = k as f64 / nt as f64;
                let (z, s) = (zu + w * (zl - zu), su + w * (sl - su));
                nodes.push([r * (s / r).cos(), r * (s / r).sin(), z]);
                node_j.push(j);
                node_press.push(edge || k == nt);
                column[k] = nodes.len() - 1;
            }
        }
    }
    const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut conn = Vec::new();
    for j in 0..ns {
        for i in 0..nc {
            for k in 0..nt {
                for p in PERMS {
                    let mut c = [i, k, j];
                    let mut v = [idx[j][i][k]; 4];
                    for s in 0..3 {
                        c[p[s]] += 1;
                        v[s + 1] = idx[c[2]][c[0]][c[1]];
                    }
                    if (0..4).any(|a| (a + 1..4).any(|b| v[a] == v[b])) {
                        continue;
                    }
                    if signed_volume(v.map(|n| nodes[n])) <= 0.0 {
                        v.swap(1, 2);
                    }
                    conn.extend_from_slice(&v);
                }
            }
        }
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Tet4, conn)?;
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect())?;
    mesh.add_node_set("all", (0..nn).collect())?;
    mesh.add_node_set("hub", (0..nn).filter(|&n| node_j[n] == 0).collect())?;
    mesh.add_node_set("tip", (0..nn).filter(|&n| node_j[n] == ns).collect())?;
    let (mut hub, mut tip, mut press, mut suction) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (key, m) in mesh.face_incidence() {
        if m.len() != 1 {
            continue;
        }
        if key.iter().all(|&n| node_j[n] == 0) {
            hub.push(m[0]);
        } else if key.iter().all(|&n| node_j[n] == ns) {
            tip.push(m[0]);
        } else if key.iter().all(|&n| node_press[n]) {
            press.push(m[0]);
        } else {
            suction.push(m[0]);
        }
    }
    mesh.add_surface_set("hub", hub)?;
    mesh.add_surface_set("tip", tip)?;
    mesh.add_surface_set("pressure", press)?;
    mesh.add_surface_set("suction", suction)?;
    Ok(mesh)
}

/// [`fan_blade_tet4`] lifted to quadratic tetrahedra by [`upgrade_to_tet10`] (mid-edge nodes at the midpoints of the
/// straight edges, not projected onto the curved blade). Same sets.
pub fn fan_blade_tet10(fan: &AxialFan, divisions: [usize; 3]) -> Result<SolidMesh> {
    Ok(upgrade_to_tet10(&fan_blade_tet4(fan, divisions)?)?.0)
}

/// Blade `blade` of a [`ComputerFan`] (with its per-blade deviation, `ComputerFan::axial_fan(blade, 0, 0)`) as the Tet10
/// mesh of [`fan_blade_tet10`]. `blade >= cf.n_blades` is refused.
pub fn computer_fan_blade_tet10(cf: &ComputerFan, blade: usize, divisions: [usize; 3]) -> Result<SolidMesh> {
    if blade >= cf.n_blades {
        return Err(blade_error(
            "blade",
            format!("blade {blade} does not exist in a fan of {} blades", cf.n_blades),
        ));
    }
    fan_blade_tet10(&cf.axial_fan(blade, 0.0, 0.0), divisions)
}
