//! Cell-centre-based polyhedral dual of a tetrahedral mesh (F-102, ADR 0054 D6).
//!
//! One polyhedron per tetrahedron vertex. The dual face of an edge `(v, w)` is the ring through the centroids of the tetrahedra
//! around the edge (open rings of boundary edges are closed through the two boundary-triangle centroids and the edge midpoint).
//! Boundary faces are the planar quads `(v, mid(v,a), centroid(t), mid(v,b))` of every boundary triangle `t`, so the boundary
//! patches are tiled exactly and the total volume equals the tetrahedral volume.

use crate::mesh::{Mesh, PatchKind};
use crate::tetmesh::TetMesh;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashMap};

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PKey {
    Vert(usize),
    Tet(usize),
    Tri([usize; 3]),
    Mid(usize, usize),
}

fn sorted3(a: usize, b: usize, c: usize) -> [usize; 3] {
    let mut k = [a, b, c];
    k.sort_unstable();
    k
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

struct Builder<'a> {
    t: &'a TetMesh,
    cent: Vec<Vec3>,
    ids: HashMap<PKey, usize>,
    points: Vec<Vec3>,
}

impl Builder<'_> {
    fn position(&self, k: PKey) -> Vec3 {
        let p = &self.t.points;
        match k {
            PKey::Vert(v) => p[v],
            PKey::Tet(i) => self.cent[i],
            PKey::Tri(t) => vec3::scale(vec3::add(vec3::add(p[t[0]], p[t[1]]), p[t[2]]), 1.0 / 3.0),
            PKey::Mid(a, b) => vec3::scale(vec3::add(p[a], p[b]), 0.5),
        }
    }

    fn id(&mut self, k: PKey) -> usize {
        if let Some(&i) = self.ids.get(&k) {
            return i;
        }
        let i = self.points.len();
        let pos = self.position(k);
        self.points.push(pos);
        self.ids.insert(k, i);
        i
    }
}

/// Convert a tetrahedral mesh to its polyhedral dual. Boundary patches follow the surface tags (tag 0 is `wall`, tag `k` is
/// `wall<k>`, `names` overrides by tag), as in [`TetMesh::to_mesh`].
pub fn tet_to_polydual(t: &TetMesh, names: &BTreeMap<u32, String>) -> Result<Mesh> {
    let nv = t.points.len();
    let mut cell_of = vec![usize::MAX; nv];
    for c in &t.cells {
        for &v in c {
            if v >= nv {
                return err("tet_to_polydual: a tetrahedron references a vertex out of range");
            }
            cell_of[v] = 0;
        }
    }
    for (v, c) in cell_of.iter_mut().enumerate() {
        if *c == usize::MAX {
            return err(format!("tet_to_polydual: vertex {v} is used by no tetrahedron"));
        }
        *c = v;
    }
    let cent: Vec<Vec3> = t
        .cells
        .iter()
        .map(|c| {
            let mut s = vec3::ZERO;
            for &v in c {
                s = vec3::add(s, t.points[v]);
            }
            vec3::scale(s, 0.25)
        })
        .collect();
    // face -> tets, edge -> tets
    let mut face_tets: HashMap<[usize; 3], Vec<usize>> = HashMap::new();
    let mut edge_tets: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (i, c) in t.cells.iter().enumerate() {
        for a in 0..4 {
            let f: Vec<usize> = (0..4).filter(|&k| k != a).map(|k| c[k]).collect();
            face_tets.entry(sorted3(f[0], f[1], f[2])).or_default().push(i);
        }
        for a in 0..4 {
            for b in a + 1..4 {
                edge_tets.entry(edge_key(c[a], c[b])).or_default().push(i);
            }
        }
    }
    for (k, v) in &face_tets {
        if v.len() > 2 {
            return err(format!(
                "tet_to_polydual: face {k:?} is shared by {} tetrahedra (non-manifold)",
                v.len()
            ));
        }
    }
    let mut surf_tag: HashMap<[usize; 3], u32> = HashMap::new();
    for (tri, tag) in t.surface.tris.iter().zip(t.surface.tags.iter()) {
        surf_tag.insert(sorted3(tri[0], tri[1], tri[2]), *tag);
    }
    let mut b = Builder {
        t,
        cent,
        ids: HashMap::new(),
        points: Vec::new(),
    };
    struct Face {
        owner: usize,
        neigh: usize,
        lp: Vec<usize>,
    }
    let mut internal: Vec<Face> = Vec::new();
    let mut edges: Vec<(&(usize, usize), &Vec<usize>)> = edge_tets.iter().collect();
    edges.sort();
    for (&(va, vb), tets) in edges {
        // the two faces of tet `ti` that contain the edge: (key, opposite vertex)
        let edge_faces = |ti: usize| -> [[usize; 3]; 2] {
            let c = t.cells[ti];
            let others: Vec<usize> = c.iter().copied().filter(|&v| v != va && v != vb).collect();
            [sorted3(va, vb, others[0]), sorted3(va, vb, others[1])]
        };
        let mut bnd = 0;
        let mut start = None;
        for &ti in tets {
            for (k, f) in edge_faces(ti).iter().enumerate() {
                if face_tets[f].len() == 1 {
                    bnd += 1;
                    if start.is_none() {
                        start = Some((ti, k));
                    }
                }
            }
        }
        if bnd != 0 && bnd != 2 {
            return err(format!(
                "tet_to_polydual: edge ({va},{vb}) lies on {bnd} boundary faces (non-manifold boundary)"
            ));
        }
        let (mut cur, enter_idx) = match start {
            Some((ti, k)) => (ti, k),
            None => (tets[0], 1),
        };
        // ring walk: enter through face `enter_idx`, leave through the other
        let mut ring = vec![cur];
        let first_face = edge_faces(cur)[enter_idx];
        let mut exit = edge_faces(cur)[1 - enter_idx];
        let mut last_face = exit;
        loop {
            let nb = &face_tets[&exit];
            if nb.len() == 1 {
                break;
            }
            let next = if nb[0] == cur { nb[1] } else { nb[0] };
            if next == ring[0] {
                break;
            }
            ring.push(next);
            let ef = edge_faces(next);
            exit = if ef[0] == exit { ef[1] } else { ef[0] };
            last_face = exit;
            cur = next;
            if ring.len() > tets.len() {
                break;
            }
        }
        if ring.len() != tets.len() {
            return err(format!(
                "tet_to_polydual: the tetrahedra around edge ({va},{vb}) do not form one ring (non-manifold edge)"
            ));
        }
        let mut lp: Vec<usize> = ring.iter().map(|&ti| b.id(PKey::Tet(ti))).collect();
        if bnd == 2 {
            lp.push(b.id(PKey::Tri(last_face)));
            lp.push(b.id(PKey::Mid(va, vb)));
            lp.push(b.id(PKey::Tri(first_face)));
        }
        let (ca, cb) = (cell_of[va], cell_of[vb]);
        let (own, neigh, vo, vn) = if ca < cb { (ca, cb, va, vb) } else { (cb, ca, vb, va) };
        let pts: Vec<Vec3> = lp.iter().map(|&i| b.points[i]).collect();
        let ids: Vec<usize> = (0..pts.len()).collect();
        let (_, sf) = crate::mesh::polygon_geometry(&pts, &ids);
        let d = vec3::dot(sf, vec3::sub(t.points[vn], t.points[vo]));
        if d == 0.0 {
            return err(format!("tet_to_polydual: dual face of edge ({va},{vb}) is degenerate"));
        }
        if d < 0.0 {
            lp.reverse();
        }
        internal.push(Face { owner: own, neigh, lp });
    }
    internal.sort_by_key(|f| (f.owner, f.neigh));
    // boundary quads per tag
    let mut by_tag: BTreeMap<u32, Vec<(usize, Vec<usize>)>> = BTreeMap::new();
    let mut tri_keys: Vec<(&[usize; 3], &u32)> = surf_tag.iter().collect();
    tri_keys.sort();
    let n_free = face_tets.values().filter(|v| v.len() == 1).count();
    if n_free != surf_tag.len() {
        return err(format!(
            "tet_to_polydual: {n_free} free tetrahedron faces but {} boundary triangles",
            surf_tag.len()
        ));
    }
    for (key, tag) in tri_keys {
        let Some(adj) = face_tets.get(key) else {
            return err("tet_to_polydual: a boundary triangle is not a face of any tetrahedron");
        };
        if adj.len() != 1 {
            return err("tet_to_polydual: a boundary triangle is not a free face of exactly one tetrahedron");
        }
        let tc = b.cent[adj[0]];
        let [x, y, z] = *key;
        for (v, p, q) in [(x, y, z), (y, z, x), (z, x, y)] {
            // (v, mid(v,p), centroid, mid(v,q))
            let mut lp = vec![
                b.id(PKey::Vert(v)),
                b.id(PKey::Mid(v.min(p), v.max(p))),
                b.id(PKey::Tri(*key)),
                b.id(PKey::Mid(v.min(q), v.max(q))),
            ];
            let pts: Vec<Vec3> = lp.iter().map(|&i| b.points[i]).collect();
            let ids: Vec<usize> = (0..4).collect();
            let (fc, sf) = crate::mesh::polygon_geometry(&pts, &ids);
            let d = vec3::dot(sf, vec3::sub(fc, tc));
            if d == 0.0 {
                return err("tet_to_polydual: degenerate boundary quad");
            }
            if d < 0.0 {
                lp.reverse();
            }
            by_tag.entry(*tag).or_default().push((cell_of[v], lp));
        }
    }
    let mut faces = Vec::new();
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for f in internal {
        faces.push(f.lp);
        owner.push(f.owner);
        neighbour.push(f.neigh);
    }
    let mut patches = Vec::new();
    for (tag, quads) in by_tag {
        let name = names.get(&tag).cloned().unwrap_or_else(|| {
            if tag == 0 {
                "wall".to_string()
            } else {
                format!("wall{tag}")
            }
        });
        patches.push((name, PatchKind::Wall, quads.len()));
        for (c, lp) in quads {
            faces.push(lp);
            owner.push(c);
        }
    }
    Mesh::from_face_lists(b.points, faces, owner, neighbour, patches)
}
