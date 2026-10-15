//! Mesh editing and repair operations (F-231, ADR 0018 part B).
//!
//! * affine transforms (translate, rotate, scale, mirror) with automatic handedness repair of
//!   mirrors ([`transform_mesh`]);
//! * merge of two meshes with stitching of coincident boundary faces, and copy by offset
//!   ([`merge_meshes`], [`copy_by_offset`]);
//! * extrusion of a boundary patch ([`extrude_patch`]);
//! * repair of inverted face loops on a raw topology ([`repair_face_orientation`],
//!   [`repair_mesh`]);
//! * merging boundary patches and separating a patch into connected components
//!   ([`merge_patches`], [`separate_patch`]).
//!
//! Not provided (ADR 0018 B1): slit/baffle creation, cell-zone merge or split, face zones through
//! merge and extrude (dropped), repair of owner/neighbour or face ordering.

use crate::mesh::{polygon_geometry, BoundarySpec, Mesh, MeshTopology, Patch, PatchKind, Zone};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashMap};

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// Affine map `x' = m x + t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    /// Linear part (row-major).
    pub m: [[f64; 3]; 3],
    /// Translation.
    pub t: Vec3,
}

impl Affine {
    /// The identity map.
    pub fn identity() -> Affine {
        Affine {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            t: vec3::ZERO,
        }
    }
    /// Translation by `v`.
    pub fn translation(v: Vec3) -> Affine {
        Affine {
            t: v,
            ..Affine::identity()
        }
    }
    /// Scaling about the origin by `s` along x, y, z.
    pub fn scaling(s: [f64; 3]) -> Affine {
        Affine {
            m: [[s[0], 0.0, 0.0], [0.0, s[1], 0.0], [0.0, 0.0, s[2]]],
            t: vec3::ZERO,
        }
    }
    /// Right-handed rotation by `angle` [rad] about the axis through `origin` with direction `axis`.
    pub fn rotation(axis: Vec3, angle: f64, origin: Vec3) -> Result<Affine> {
        let n = vec3::norm(axis);
        if n == 0.0 || !n.is_finite() {
            return err("rotation axis must be non-zero");
        }
        let k = vec3::scale(axis, 1.0 / n);
        let (s, c) = angle.sin_cos();
        let kk = |i: usize, j: usize| k[i] * k[j];
        let m = [
            [
                c + kk(0, 0) * (1.0 - c),
                kk(0, 1) * (1.0 - c) - k[2] * s,
                kk(0, 2) * (1.0 - c) + k[1] * s,
            ],
            [
                kk(1, 0) * (1.0 - c) + k[2] * s,
                c + kk(1, 1) * (1.0 - c),
                kk(1, 2) * (1.0 - c) - k[0] * s,
            ],
            [
                kk(2, 0) * (1.0 - c) - k[1] * s,
                kk(2, 1) * (1.0 - c) + k[0] * s,
                c + kk(2, 2) * (1.0 - c),
            ],
        ];
        let mut a = Affine { m, t: vec3::ZERO };
        a.t = vec3::sub(origin, a.linear(origin));
        Ok(a)
    }
    /// Reflection in the plane through `point` with normal `normal`.
    pub fn mirror(normal: Vec3, point: Vec3) -> Result<Affine> {
        let nn = vec3::norm(normal);
        if nn == 0.0 || !nn.is_finite() {
            return err("mirror normal must be non-zero");
        }
        let n = vec3::scale(normal, 1.0 / nn);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = if i == j { 1.0 } else { 0.0 } - 2.0 * n[i] * n[j];
            }
        }
        Ok(Affine {
            m,
            t: vec3::scale(n, 2.0 * vec3::dot(n, point)),
        })
    }
    /// The map that applies `self` first and `next` second.
    pub fn then(&self, next: &Affine) -> Affine {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| next.m[i][k] * self.m[k][j]).sum();
            }
        }
        Affine {
            m,
            t: vec3::add(next.linear(self.t), next.t),
        }
    }
    /// Linear part applied to a vector.
    pub fn linear(&self, v: Vec3) -> Vec3 {
        let r = |i: usize| self.m[i][0] * v[0] + self.m[i][1] * v[1] + self.m[i][2] * v[2];
        [r(0), r(1), r(2)]
    }
    /// Map a point.
    pub fn apply(&self, p: Vec3) -> Vec3 {
        vec3::add(self.linear(p), self.t)
    }
    /// Determinant of the linear part.
    pub fn det(&self) -> f64 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }
    /// Cofactor matrix `det(M) M^{-T}`: area vectors map as `S' = cof(M) S`.
    pub fn cofactor(&self) -> [[f64; 3]; 3] {
        let m = &self.m;
        let c = |i: usize, j: usize| {
            let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
            let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
            m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
        };
        let mut out = [[0.0; 3]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = c(i, j);
            }
        }
        out
    }
}

/// Apply an affine map to a mesh. A map with negative determinant reverses every face loop so
/// that the owner-to-neighbour orientation and the positive volumes are kept; topology, patches
/// and zones are unchanged.
pub fn transform_mesh(mesh: &Mesh, a: &Affine) -> Result<Mesh> {
    let det = a.det();
    if det == 0.0 || !det.is_finite() {
        return err("transform is singular");
    }
    let mut topo = mesh.topology().clone();
    for p in topo.points.iter_mut() {
        *p = a.apply(*p);
    }
    if det < 0.0 {
        reverse_all_loops(&mut topo);
    }
    Mesh::from_topology(topo)
}

fn reverse_all_loops(topo: &mut MeshTopology) {
    for f in 0..topo.owner.len() {
        topo.face_points[topo.face_offsets[f]..topo.face_offsets[f + 1]].reverse();
    }
}

fn cell_face_lists(mesh: &Mesh, map: &[usize], out: &mut Vec<Vec<Vec<usize>>>) {
    for c in 0..mesh.n_cells() {
        out.push(
            mesh.cell_faces(c)
                .iter()
                .map(|&f| mesh.face_points(f).iter().map(|&p| map[p]).collect())
                .collect(),
        );
    }
}

fn sorted_key(f: &[usize]) -> Vec<usize> {
    let mut k = f.to_vec();
    k.sort_unstable();
    k
}

fn refuse_periodic(mesh: &Mesh, what: &str) -> Result<()> {
    if let Some(p) = mesh
        .patches()
        .iter()
        .find(|p| p.periodic_with.is_some() || p.rotation.is_some())
    {
        return err(format!("{what}: periodic patch '{}' is not supported", p.name));
    }
    Ok(())
}

/// Merge `b` into `a`. Points of `b` closer than `tol` to a point of `a` are identified with it;
/// boundary faces that become shared by a cell of `a` and a cell of `b` turn into internal faces
/// (stitching). Patches of equal name are combined (kind from the first), cell zones of equal name
/// are combined (cell ids of `b` offset by the cell count of `a`); face zones are dropped.
pub fn merge_meshes(a: &Mesh, b: &Mesh, tol: f64) -> Result<Mesh> {
    if tol.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return err("merge tolerance must be positive");
    }
    refuse_periodic(a, "merge")?;
    refuse_periodic(b, "merge")?;
    let mut points: Vec<Vec3> = a.points().to_vec();
    let key = |p: Vec3| -> [i64; 3] {
        [
            (p[0] / tol).floor() as i64,
            (p[1] / tol).floor() as i64,
            (p[2] / tol).floor() as i64,
        ]
    };
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, p) in a.points().iter().enumerate() {
        grid.entry(key(*p)).or_default().push(i);
    }
    let mut map_b = Vec::with_capacity(b.n_points());
    for p in b.points() {
        let k = key(*p);
        let mut found = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        for &i in v {
                            if i < a.n_points() && vec3::norm(vec3::sub(points[i], *p)) <= tol {
                                found = Some(i);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        map_b.push(found.unwrap_or_else(|| {
            points.push(*p);
            points.len() - 1
        }));
    }
    let map_a: Vec<usize> = (0..a.n_points()).collect();
    let mut cells = Vec::new();
    cell_face_lists(a, &map_a, &mut cells);
    cell_face_lists(b, &map_b, &mut cells);
    let mut count: HashMap<Vec<usize>, usize> = HashMap::new();
    for c in &cells {
        for f in c {
            *count.entry(sorted_key(f)).or_insert(0) += 1;
        }
    }
    let mut order: Vec<String> = Vec::new();
    let mut specs: HashMap<String, BoundarySpec> = HashMap::new();
    for (mesh, map) in [(a, &map_a), (b, &map_b)] {
        for p in mesh.patches() {
            let spec = specs.entry(p.name.clone()).or_insert_with(|| {
                order.push(p.name.clone());
                BoundarySpec {
                    name: p.name.clone(),
                    kind: p.kind.clone(),
                    faces: vec![],
                }
            });
            for f in p.faces() {
                let lp: Vec<usize> = mesh.face_points(f).iter().map(|&q| map[q]).collect();
                if count.get(&sorted_key(&lp)).copied().unwrap_or(0) < 2 {
                    spec.faces.push(lp);
                }
            }
        }
    }
    let boundary: Vec<BoundarySpec> = order
        .into_iter()
        .map(|n| specs.remove(&n).expect("spec present"))
        .filter(|s| !s.faces.is_empty())
        .collect();
    let mut m = Mesh::from_cell_faces(points, &cells, &boundary)?;
    let mut zones: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for z in a.cell_zones() {
        zones.entry(z.name.clone()).or_default().extend(&z.ids);
    }
    for z in b.cell_zones() {
        zones
            .entry(z.name.clone())
            .or_default()
            .extend(z.ids.iter().map(|&c| c + a.n_cells()));
    }
    for (name, ids) in zones {
        m.add_cell_zone(&name, ids)?;
    }
    Ok(m)
}

/// `n_copies` additional copies of `mesh`, copy `k` translated by `k * offset`, each merged into the
/// result with [`merge_meshes`] (coincident boundary faces are stitched).
pub fn copy_by_offset(mesh: &Mesh, n_copies: usize, offset: Vec3, tol: f64) -> Result<Mesh> {
    let mut acc = mesh.clone();
    for k in 1..=n_copies {
        let shifted = transform_mesh(mesh, &Affine::translation(vec3::scale(offset, k as f64)))?;
        acc = merge_meshes(&acc, &shifted, tol)?;
    }
    Ok(acc)
}

/// Direction of an extrusion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtrudeDir {
    /// Total displacement vector of the far cap.
    Vector(Vec3),
    /// Total distance along the per-point averaged outward patch normals (exact for planar patches).
    Normal(f64),
}

/// Extrude boundary patch `patch` outward in `layers` equal layers. Each patch face yields a prism
/// or hexahedron per layer. The patch is replaced by a patch of the same name and kind (the far
/// cap) and a `Generic` patch `<patch>_side` with the new lateral faces. Cell zones are kept,
/// face zones are dropped.
pub fn extrude_patch(mesh: &Mesh, patch: &str, dir: ExtrudeDir, layers: usize) -> Result<Mesh> {
    extrude_core(mesh, patch, layers, Displacement::Dir(dir)).map(|(m, _)| m)
}

/// How the points of the extruded layers are placed (see [`extrude_core`]).
pub(crate) enum Displacement<'a> {
    /// A plain [`ExtrudeDir`] with equal layers.
    Dir(ExtrudeDir),
    /// Cumulative distance along the point normal after layer `k` (1-based) for a point whose local
    /// size (mean square root of the incident patch face areas) is the first argument.
    Layered(&'a dyn Fn(f64, usize) -> f64),
}

/// Shared extrusion core. Returns the new mesh and the local size of every patch point (in order of
/// first appearance in the patch faces).
pub(crate) fn extrude_core(
    mesh: &Mesh,
    patch: &str,
    layers: usize,
    disp_rule: Displacement<'_>,
) -> Result<(Mesh, Vec<f64>)> {
    if layers == 0 {
        return err("extrude: at least one layer is required");
    }
    refuse_periodic(mesh, "extrude")?;
    let Some(p) = mesh.patch(patch) else {
        return err(format!("extrude: no patch named '{patch}'"));
    };
    let side_name = format!("{patch}_side");
    if mesh.patch(&side_name).is_some() {
        return err(format!("extrude: patch '{side_name}' already exists"));
    }
    let mut local: HashMap<usize, usize> = HashMap::new();
    let mut base: Vec<usize> = Vec::new();
    let mut normal_sum: Vec<Vec3> = Vec::new();
    let mut size_sum: Vec<f64> = Vec::new();
    let mut size_cnt: Vec<usize> = Vec::new();
    for f in p.faces() {
        let s = mesh.face_area_vectors()[f];
        let root_area = vec3::norm(s).sqrt();
        for &q in mesh.face_points(f) {
            let j = *local.entry(q).or_insert_with(|| {
                base.push(q);
                normal_sum.push(vec3::ZERO);
                size_sum.push(0.0);
                size_cnt.push(0);
                base.len() - 1
            });
            normal_sum[j] = vec3::add(normal_sum[j], vec3::unit(s));
            size_sum[j] += root_area;
            size_cnt[j] += 1;
        }
    }
    let local_size: Vec<f64> = size_sum.iter().zip(&size_cnt).map(|(s, &n)| s / n as f64).collect();
    if let Displacement::Dir(ExtrudeDir::Vector(v)) = disp_rule {
        for f in p.faces() {
            if vec3::dot(v, mesh.face_area_vectors()[f]) <= 0.0 {
                return err(format!(
                    "extrude: direction does not point out of the domain at face {f} of patch '{patch}'"
                ));
            }
        }
    }
    let m = base.len();
    let mut points: Vec<Vec3> = mesh.points().to_vec();
    let np = points.len();
    for k in 1..=layers {
        let frac = k as f64 / layers as f64;
        for (j, &q) in base.iter().enumerate() {
            let disp = match &disp_rule {
                Displacement::Dir(ExtrudeDir::Vector(v)) => vec3::scale(*v, frac),
                Displacement::Dir(ExtrudeDir::Normal(d)) => vec3::scale(vec3::unit(normal_sum[j]), d * frac),
                Displacement::Layered(cum) => vec3::scale(vec3::unit(normal_sum[j]), cum(local_size[j], k)),
            };
            points.push(vec3::add(mesh.points()[q], disp));
        }
    }
    let id = |k: usize, q: usize| -> usize {
        if k == 0 {
            q
        } else {
            np + (k - 1) * m + local[&q]
        }
    };
    let identity: Vec<usize> = (0..np).collect();
    let mut cells = Vec::new();
    cell_face_lists(mesh, &identity, &mut cells);
    let mut cap = Vec::new();
    let mut side = Vec::new();
    for f in p.faces() {
        let lp = mesh.face_points(f);
        let s = lp.len();
        for k in 1..=layers {
            let bot: Vec<usize> = lp.iter().map(|&q| id(k - 1, q)).collect();
            let top: Vec<usize> = lp.iter().map(|&q| id(k, q)).collect();
            let mut faces = vec![bot.clone(), top.clone()];
            for i in 0..s {
                let j = (i + 1) % s;
                let quad = vec![bot[i], bot[j], top[j], top[i]];
                faces.push(quad.clone());
                side.push(quad);
            }
            if k == layers {
                cap.push(top);
            }
            cells.push(faces);
        }
    }
    // Lateral faces shared by two extruded columns are internal; from_cell_faces drops them from the
    // free-boundary set, so only free ones may be listed in a patch.
    let mut count: HashMap<Vec<usize>, usize> = HashMap::new();
    for c in &cells {
        for f in c {
            *count.entry(sorted_key(f)).or_insert(0) += 1;
        }
    }
    side.retain(|f| count.get(&sorted_key(f)).copied().unwrap_or(0) < 2);
    let mut boundary = Vec::new();
    for q in mesh.patches() {
        if q.name == patch {
            boundary.push(BoundarySpec {
                name: patch.to_string(),
                kind: q.kind.clone(),
                faces: std::mem::take(&mut cap),
            });
        } else {
            boundary.push(BoundarySpec {
                name: q.name.clone(),
                kind: q.kind.clone(),
                faces: q.faces().map(|f| mesh.face_points(f).to_vec()).collect(),
            });
        }
    }
    boundary.push(BoundarySpec {
        name: side_name,
        kind: PatchKind::Generic,
        faces: side,
    });
    let mut out = Mesh::from_cell_faces(points, &cells, &boundary)?;
    for z in mesh.cell_zones() {
        out.add_cell_zone(&z.name, z.ids.clone())?;
    }
    Ok((out, local_size))
}

/// What [`repair_face_orientation`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    /// Faces whose point loop was reversed.
    pub flipped_faces: usize,
}

/// Reverse every face loop whose area vector points into its owner cell (relative to the
/// vertex-average estimate of the owner centre). Works on a raw topology so that inverted input,
/// which [`Mesh::from_topology`] rejects, can be repaired first. The order of faces, owners and
/// neighbours is not changed (a violated `owner < neighbour` ordering stays an error of
/// `from_topology`).
pub fn repair_face_orientation(topo: &mut MeshTopology) -> RepairReport {
    let n_cells = topo
        .owner
        .iter()
        .chain(topo.neighbour.iter())
        .copied()
        .max()
        .map_or(0, |m| m + 1);
    let mut sum = vec![vec3::ZERO; n_cells];
    let mut cnt = vec![0.0f64; n_cells];
    for f in 0..topo.owner.len() {
        let ids = &topo.face_points[topo.face_offsets[f]..topo.face_offsets[f + 1]];
        let cells = [Some(topo.owner[f]), topo.neighbour.get(f).copied()];
        for c in cells.into_iter().flatten() {
            for &q in ids {
                sum[c] = vec3::add(sum[c], topo.points[q]);
                cnt[c] += 1.0;
            }
        }
    }
    let mut flipped = 0;
    for f in 0..topo.owner.len() {
        let range = topo.face_offsets[f]..topo.face_offsets[f + 1];
        let (fc, sf) = polygon_geometry(&topo.points, &topo.face_points[range.clone()]);
        let c = topo.owner[f];
        let est = vec3::scale(sum[c], 1.0 / cnt[c].max(1.0));
        if vec3::dot(sf, vec3::sub(fc, est)) < 0.0 {
            topo.face_points[range].reverse();
            flipped += 1;
        }
    }
    RepairReport { flipped_faces: flipped }
}

/// Repair a raw topology with [`repair_face_orientation`] and build the (validated) mesh.
pub fn repair_mesh(mut topo: MeshTopology) -> Result<(Mesh, RepairReport)> {
    let rep = repair_face_orientation(&mut topo);
    Ok((Mesh::from_topology(topo)?, rep))
}

/// Rebuild `mesh` with its boundary faces regrouped: each group is a patch template (name, kind,
/// periodic data) and the old face ids it takes, in order. Internal faces keep their ids; face
/// zones are remapped.
fn regroup_boundary(mesh: &Mesh, groups: Vec<(Patch, Vec<usize>)>) -> Result<Mesh> {
    let ni = mesh.n_internal_faces();
    let nf = mesh.n_faces();
    let mut topo = mesh.topology().clone();
    let mut new_of_old: Vec<usize> = (0..nf).collect();
    let mut order: Vec<usize> = (0..ni).collect();
    let mut seen = vec![false; nf];
    let mut patches = Vec::new();
    for (mut patch, faces) in groups {
        patch.start = order.len();
        patch.size = faces.len();
        for f in faces {
            if f < ni || f >= nf || seen[f] {
                return err("regroup: face listed twice or not a boundary face");
            }
            seen[f] = true;
            new_of_old[f] = order.len();
            order.push(f);
        }
        patches.push(patch);
    }
    if order.len() != nf {
        return err("regroup: groups do not cover all boundary faces");
    }
    topo.face_offsets = vec![0];
    topo.face_points = Vec::with_capacity(mesh.topology().face_points.len());
    topo.owner = Vec::with_capacity(nf);
    for &old in &order {
        topo.face_points.extend_from_slice(mesh.face_points(old));
        topo.face_offsets.push(topo.face_points.len());
        topo.owner.push(mesh.owner()[old]);
    }
    topo.patches = patches;
    topo.face_zones = mesh
        .face_zones()
        .iter()
        .map(|z| {
            let mut ids: Vec<usize> = z.ids.iter().map(|&f| new_of_old[f]).collect();
            ids.sort_unstable();
            Zone {
                name: z.name.clone(),
                ids,
            }
        })
        .collect();
    Mesh::from_topology(topo)
}

/// Combine boundary patches `names` into one patch `new_name` (at the position of the first of them
/// in the mesh, kind of that first one). Geometry, cells and internal faces are unchanged.
pub fn merge_patches(mesh: &Mesh, names: &[&str], new_name: &str) -> Result<Mesh> {
    if names.len() < 2 {
        return err("merge_patches: give at least two patches");
    }
    for n in names {
        let Some(p) = mesh.patch(n) else {
            return err(format!("merge_patches: no patch named '{n}'"));
        };
        if p.periodic_with.is_some() || p.rotation.is_some() {
            return err(format!("merge_patches: periodic patch '{n}' is not supported"));
        }
    }
    let mut uniq = names.to_vec();
    uniq.sort_unstable();
    uniq.dedup();
    if uniq.len() != names.len() {
        return err("merge_patches: duplicate patch name");
    }
    if !names.contains(&new_name) && mesh.patch(new_name).is_some() {
        return err(format!("merge_patches: patch '{new_name}' already exists"));
    }
    let mut groups: Vec<(Patch, Vec<usize>)> = Vec::new();
    let mut merged_at: Option<usize> = None;
    for p in mesh.patches() {
        if names.contains(&p.name.as_str()) {
            match merged_at {
                None => {
                    let mut t = p.clone();
                    t.name = new_name.to_string();
                    merged_at = Some(groups.len());
                    groups.push((t, p.faces().collect()));
                }
                Some(i) => groups[i].1.extend(p.faces()),
            }
        } else {
            groups.push((p.clone(), p.faces().collect()));
        }
    }
    regroup_boundary(mesh, groups)
}

/// Split patch `name` into its face-edge-connected components `<name>_0, <name>_1, ...` (ordered by
/// the lowest face id of each). A connected patch is returned unchanged.
pub fn separate_patch(mesh: &Mesh, name: &str) -> Result<Mesh> {
    let Some(p) = mesh.patch(name) else {
        return err(format!("separate_patch: no patch named '{name}'"));
    };
    if p.periodic_with.is_some() || p.rotation.is_some() {
        return err(format!("separate_patch: periodic patch '{name}' is not supported"));
    }
    let faces: Vec<usize> = p.faces().collect();
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut edge_owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (i, &f) in faces.iter().enumerate() {
        let ids = mesh.face_points(f);
        for k in 0..ids.len() {
            let (a, b) = (ids[k], ids[(k + 1) % ids.len()]);
            let e = (a.min(b), a.max(b));
            match edge_owner.get(&e) {
                Some(&j) => {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    if ri != rj {
                        parent[ri.max(rj)] = ri.min(rj);
                    }
                }
                None => {
                    edge_owner.insert(e, i);
                }
            }
        }
    }
    let mut comp_of_root: HashMap<usize, usize> = HashMap::new();
    let mut comps: Vec<Vec<usize>> = Vec::new();
    for (i, &f) in faces.iter().enumerate() {
        let r = find(&mut parent, i);
        let c = *comp_of_root.entry(r).or_insert_with(|| {
            comps.push(vec![]);
            comps.len() - 1
        });
        comps[c].push(f);
    }
    if comps.len() <= 1 {
        return Ok(mesh.clone());
    }
    for k in 0..comps.len() {
        let n = format!("{name}_{k}");
        if mesh.patch(&n).is_some() {
            return err(format!("separate_patch: patch '{n}' already exists"));
        }
    }
    let mut groups: Vec<(Patch, Vec<usize>)> = Vec::new();
    for q in mesh.patches() {
        if q.name == name {
            for (k, c) in comps.iter().enumerate() {
                let mut t = q.clone();
                t.name = format!("{name}_{k}");
                groups.push((t, c.clone()));
            }
        } else {
            groups.push((q.clone(), q.faces().collect()));
        }
    }
    regroup_boundary(mesh, groups)
}
