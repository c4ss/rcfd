//! The polyhedral mesh type, its construction paths and invariant checks.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Physical/semantic type of a boundary patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchKind {
    /// Generic patch without special meaning.
    Generic,
    /// No-slip or slip wall.
    Wall,
    /// Inflow boundary.
    Inlet,
    /// Outflow boundary.
    Outlet,
    /// Symmetry plane.
    Symmetry,
    /// Empty (2D reduction) patch.
    Empty,
    /// Periodic (cyclic) patch; the partner patch is named in [`Patch::periodic_with`].
    Cyclic,
}

/// A contiguous range of boundary faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    /// Patch name (unique).
    pub name: String,
    /// Patch kind.
    pub kind: PatchKind,
    /// First face index.
    pub start: usize,
    /// Number of faces.
    pub size: usize,
    /// Partner patch for [`PatchKind::Cyclic`].
    pub periodic_with: Option<String>,
    /// Rotational periodicity: the partner patch is this patch rotated by
    /// [`PeriodicRotation::angle`]; `None` means pure translation.
    #[serde(default)]
    pub rotation: Option<PeriodicRotation>,
}

/// Rotation (right-handed, about an axis through `origin`) that maps a cyclic
/// patch onto its partner patch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeriodicRotation {
    /// Point on the rotation axis.
    pub origin: Vec3,
    /// Unit axis direction.
    pub axis: Vec3,
    /// Rotation angle in radians (this patch to its partner).
    pub angle: f64,
}

impl Eq for PeriodicRotation {}

impl PeriodicRotation {
    /// Rotation matrix `R` (row-major) for the angle (Rodrigues' formula).
    pub fn matrix(&self) -> [[f64; 3]; 3] {
        let a = vec3::unit(self.axis);
        let (s, c) = self.angle.sin_cos();
        let t = 1.0 - c;
        [
            [
                t * a[0] * a[0] + c,
                t * a[0] * a[1] - s * a[2],
                t * a[0] * a[2] + s * a[1],
            ],
            [
                t * a[0] * a[1] + s * a[2],
                t * a[1] * a[1] + c,
                t * a[1] * a[2] - s * a[0],
            ],
            [
                t * a[0] * a[2] - s * a[1],
                t * a[1] * a[2] + s * a[0],
                t * a[2] * a[2] + c,
            ],
        ]
    }

    /// Rotate a point about the axis.
    pub fn apply_point(&self, p: Vec3) -> Vec3 {
        let r = self.matrix();
        let d = vec3::sub(p, self.origin);
        let rd = [0, 1, 2].map(|i| r[i][0] * d[0] + r[i][1] * d[1] + r[i][2] * d[2]);
        vec3::add(self.origin, rd)
    }

    /// The inverse rotation (the partner patch back onto this one).
    pub fn inverse(&self) -> PeriodicRotation {
        PeriodicRotation {
            angle: -self.angle,
            ..*self
        }
    }
}

impl Patch {
    /// Face index range.
    pub fn faces(&self) -> std::ops::Range<usize> {
        self.start..self.start + self.size
    }
}

/// A named group of cells or faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    /// Zone name.
    pub name: String,
    /// Member cell or face indices (sorted).
    pub ids: Vec<usize>,
}

/// Boundary patch description used when building from cells: faces given as
/// point loops (any orientation, any start vertex).
#[derive(Debug, Clone)]
pub struct BoundarySpec {
    /// Patch name.
    pub name: String,
    /// Patch kind.
    pub kind: PatchKind,
    /// Faces as point-id loops.
    pub faces: Vec<Vec<usize>>,
}

/// Topology-only description (what gets serialized); geometry is derived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshTopology {
    /// Point coordinates.
    pub points: Vec<Vec3>,
    /// CSR offsets into `face_points` (length n_faces + 1).
    pub face_offsets: Vec<usize>,
    /// Concatenated face point loops.
    pub face_points: Vec<usize>,
    /// Owner cell of each face.
    pub owner: Vec<usize>,
    /// Neighbour cell of each internal face.
    pub neighbour: Vec<usize>,
    /// Boundary patches.
    pub patches: Vec<Patch>,
    /// Cell zones.
    pub cell_zones: Vec<Zone>,
    /// Face zones.
    pub face_zones: Vec<Zone>,
}

/// Polyhedral finite-volume mesh with derived geometry.
#[derive(Debug, Clone)]
pub struct Mesh {
    topo: MeshTopology,
    n_cells: usize,
    face_centres: Vec<Vec3>,
    face_area_vectors: Vec<Vec3>,
    cell_centres: Vec<Vec3>,
    cell_volumes: Vec<f64>,
    cell_face_offsets: Vec<usize>,
    cell_faces: Vec<usize>,
}

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// Face centre and area vector of a polygon (triangle fan around the vertex
/// average, which is exact for planar polygons and robust for warped ones).
pub fn polygon_geometry(points: &[Vec3], loop_ids: &[usize]) -> (Vec3, Vec3) {
    let n = loop_ids.len();
    let p = |i: usize| points[loop_ids[i % n]];
    if n == 3 {
        let (a, b, c) = (p(0), p(1), p(2));
        let sf = vec3::scale(vec3::cross(vec3::sub(b, a), vec3::sub(c, a)), 0.5);
        let fc = vec3::scale(vec3::add(vec3::add(a, b), c), 1.0 / 3.0);
        return (fc, sf);
    }
    let mut est = vec3::ZERO;
    for i in 0..n {
        est = vec3::add(est, p(i));
    }
    est = vec3::scale(est, 1.0 / n as f64);
    let mut sum_n = vec3::ZERO;
    let mut sum_a = 0.0;
    let mut sum_ac = vec3::ZERO;
    for i in 0..n {
        let (a, b) = (p(i), p(i + 1));
        let c = vec3::add(vec3::add(a, b), est);
        let nvec = vec3::cross(vec3::sub(b, a), vec3::sub(est, a));
        let area = vec3::norm(nvec);
        sum_n = vec3::add(sum_n, nvec);
        sum_a += area;
        sum_ac = vec3::add(sum_ac, vec3::scale(c, area));
    }
    if sum_a < 1e-300 {
        return (est, vec3::ZERO);
    }
    (vec3::scale(sum_ac, 1.0 / (3.0 * sum_a)), vec3::scale(sum_n, 0.5))
}

impl Mesh {
    /// Build from canonical face lists: faces `0..neighbour.len()` internal.
    ///
    /// `patches` supplies `(name, kind, n_faces)` in order; their sizes must
    /// sum to the number of boundary faces. The mesh is validated.
    pub fn from_face_lists(
        points: Vec<Vec3>,
        faces: Vec<Vec<usize>>,
        owner: Vec<usize>,
        neighbour: Vec<usize>,
        patches: Vec<(String, PatchKind, usize)>,
    ) -> Result<Mesh> {
        let mut face_offsets = vec![0];
        let mut face_points = Vec::new();
        for f in &faces {
            face_points.extend_from_slice(f);
            face_offsets.push(face_points.len());
        }
        let mut start = neighbour.len();
        let mut ps = Vec::new();
        for (name, kind, size) in patches {
            ps.push(Patch {
                name,
                kind,
                start,
                size,
                periodic_with: None,
                rotation: None,
            });
            start += size;
        }
        let topo = MeshTopology {
            points,
            face_offsets,
            face_points,
            owner,
            neighbour,
            patches: ps,
            cell_zones: vec![],
            face_zones: vec![],
        };
        Mesh::from_topology(topo)
    }

    /// Build (and validate) a mesh from its topology; derives all geometry.
    pub fn from_topology(topo: MeshTopology) -> Result<Mesh> {
        let n_faces = topo.owner.len();
        if topo.face_offsets.len() != n_faces + 1 {
            return err("face_offsets length does not match number of faces");
        }
        if topo.neighbour.len() > n_faces {
            return err("more neighbours than faces");
        }
        let np = topo.points.len();
        if topo.face_points.iter().any(|&p| p >= np) {
            return err("face references a point index out of range");
        }
        for f in 0..n_faces {
            if topo.face_offsets[f + 1] < topo.face_offsets[f] + 3 {
                return err(format!("face {f} has fewer than 3 points"));
            }
        }
        let n_cells = topo
            .owner
            .iter()
            .chain(topo.neighbour.iter())
            .copied()
            .max()
            .map_or(0, |m| m + 1);
        let mut m = Mesh {
            topo,
            n_cells,
            face_centres: vec![],
            face_area_vectors: vec![],
            cell_centres: vec![],
            cell_volumes: vec![],
            cell_face_offsets: vec![],
            cell_faces: vec![],
        };
        m.build_cell_faces();
        m.compute_geometry();
        m.validate()?;
        Ok(m)
    }

    /// Build from cells described by their faces (point loops of any
    /// orientation). Shared faces are detected by their sorted point set;
    /// owner/neighbour, orientation and canonical ordering are derived.
    /// Cells must be convex enough for the vertex-average to lie inside them.
    ///
    /// Boundary faces are assigned to `boundary` patches by point set;
    /// unassigned ones go to a trailing `defaultFaces` patch.
    pub fn from_cell_faces(points: Vec<Vec3>, cells: &[Vec<Vec<usize>>], boundary: &[BoundarySpec]) -> Result<Mesh> {
        fn key(f: &[usize]) -> Vec<usize> {
            let mut k = f.to_vec();
            k.sort_unstable();
            k
        }
        // 1. orient each cell's faces outward.
        struct Entry {
            cell: usize,
            loop_ids: Vec<usize>,
        }
        let mut by_key: HashMap<Vec<usize>, Vec<Entry>> = HashMap::new();
        let mut order: Vec<Vec<usize>> = Vec::new();
        for (ci, cell) in cells.iter().enumerate() {
            let mut est = vec3::ZERO;
            let mut cnt = 0.0;
            for f in cell {
                for &p in f {
                    if p >= points.len() {
                        return err(format!("cell {ci} references point {p} out of range"));
                    }
                    est = vec3::add(est, points[p]);
                    cnt += 1.0;
                }
            }
            if cnt == 0.0 {
                return err(format!("cell {ci} has no faces"));
            }
            est = vec3::scale(est, 1.0 / cnt);
            for f in cell {
                if f.len() < 3 {
                    return err(format!("cell {ci} has a face with fewer than 3 points"));
                }
                let (fc, sf) = polygon_geometry(&points, f);
                let d = vec3::dot(sf, vec3::sub(fc, est));
                if d == 0.0 {
                    return err(format!("cell {ci} has a degenerate face"));
                }
                let mut loop_ids = f.clone();
                if d < 0.0 {
                    loop_ids.reverse();
                }
                let k = key(f);
                let e = by_key.entry(k.clone()).or_insert_with(|| {
                    order.push(k);
                    Vec::new()
                });
                e.push(Entry { cell: ci, loop_ids });
            }
        }
        // 2. split internal / boundary.
        struct Internal {
            owner: usize,
            neigh: usize,
            loop_ids: Vec<usize>,
        }
        let mut internal = Vec::new();
        let mut boundary_faces: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
        let mut boundary_order = Vec::new();
        for k in &order {
            let entries = by_key.remove(k).expect("key present");
            match entries.len() {
                1 => {
                    let e = entries.into_iter().next().expect("one entry");
                    boundary_order.push(k.clone());
                    boundary_faces.insert(k.clone(), (e.cell, e.loop_ids));
                }
                2 => {
                    let mut it = entries.into_iter();
                    let (a, b) = (it.next().expect("first"), it.next().expect("second"));
                    if a.cell == b.cell {
                        return err(format!("cell {} lists the same face twice", a.cell));
                    }
                    let (o, n) = if a.cell < b.cell { (a, b) } else { (b, a) };
                    internal.push(Internal {
                        owner: o.cell,
                        neigh: n.cell,
                        loop_ids: o.loop_ids,
                    });
                }
                n => return err(format!("face shared by {n} cells (non-manifold)")),
            }
        }
        internal.sort_by_key(|f| (f.owner, f.neigh));
        let mut faces: Vec<Vec<usize>> = Vec::new();
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        for f in internal {
            faces.push(f.loop_ids);
            owner.push(f.owner);
            neighbour.push(f.neigh);
        }
        let mut patches = Vec::new();
        for spec in boundary {
            let mut size = 0;
            for f in &spec.faces {
                let k = key(f);
                let (cell, lp) = boundary_faces.remove(&k).ok_or_else(|| {
                    Error::Mesh(format!(
                        "patch '{}' lists a face that is not a free boundary face",
                        spec.name
                    ))
                })?;
                faces.push(lp);
                owner.push(cell);
                size += 1;
            }
            patches.push((spec.name.clone(), spec.kind.clone(), size));
        }
        let mut leftover = 0;
        for k in &boundary_order {
            if let Some((cell, lp)) = boundary_faces.remove(k) {
                faces.push(lp);
                owner.push(cell);
                leftover += 1;
            }
        }
        if leftover > 0 {
            patches.push(("defaultFaces".to_string(), PatchKind::Generic, leftover));
        }
        Mesh::from_face_lists(points, faces, owner, neighbour, patches)
    }

    fn build_cell_faces(&mut self) {
        let mut counts = vec![0usize; self.n_cells + 1];
        for &o in &self.topo.owner {
            counts[o + 1] += 1;
        }
        for &n in &self.topo.neighbour {
            counts[n + 1] += 1;
        }
        for i in 0..self.n_cells {
            counts[i + 1] += counts[i];
        }
        let mut fill = counts.clone();
        let mut cf = vec![0usize; counts[self.n_cells]];
        for (f, &o) in self.topo.owner.iter().enumerate() {
            cf[fill[o]] = f;
            fill[o] += 1;
        }
        for (f, &n) in self.topo.neighbour.iter().enumerate() {
            cf[fill[n]] = f;
            fill[n] += 1;
        }
        self.cell_face_offsets = counts;
        self.cell_faces = cf;
    }

    fn compute_geometry(&mut self) {
        let nf = self.n_faces();
        let mut fcs = Vec::with_capacity(nf);
        let mut sfs = Vec::with_capacity(nf);
        for f in 0..nf {
            let (fc, sf) = polygon_geometry(&self.topo.points, self.face_points(f));
            fcs.push(fc);
            sfs.push(sf);
        }
        // Cell centre estimate: average of face centres.
        let mut est = vec![vec3::ZERO; self.n_cells];
        for (c, e) in est.iter_mut().enumerate() {
            let faces = &self.cell_faces[self.cell_face_offsets[c]..self.cell_face_offsets[c + 1]];
            let mut s = vec3::ZERO;
            for &f in faces {
                s = vec3::add(s, fcs[f]);
            }
            *e = vec3::scale(s, 1.0 / faces.len().max(1) as f64);
        }
        let mut vol = vec![0.0; self.n_cells];
        let mut cvol = vec![vec3::ZERO; self.n_cells];
        let mut accum = |c: usize, f: usize, sign: f64| {
            let sf = vec3::scale(sfs[f], sign);
            let pv = vec3::dot(sf, vec3::sub(fcs[f], est[c])) / 3.0;
            let pc = vec3::add(vec3::scale(fcs[f], 0.75), vec3::scale(est[c], 0.25));
            vol[c] += pv;
            cvol[c] = vec3::add(cvol[c], vec3::scale(pc, pv));
        };
        for f in 0..nf {
            accum(self.topo.owner[f], f, 1.0);
            if f < self.topo.neighbour.len() {
                accum(self.topo.neighbour[f], f, -1.0);
            }
        }
        let mut cc = vec![vec3::ZERO; self.n_cells];
        for c in 0..self.n_cells {
            cc[c] = if vol[c].abs() > 1e-300 {
                vec3::scale(cvol[c], 1.0 / vol[c])
            } else {
                est[c]
            };
        }
        self.face_centres = fcs;
        self.face_area_vectors = sfs;
        self.cell_centres = cc;
        self.cell_volumes = vol;
    }

    /// Check structural and geometric invariants:
    /// patch coverage, owner < neighbour, closed cells (every directed edge of
    /// a cell's outward-oriented faces is matched by its reverse exactly once,
    /// and area vectors sum to zero), positive volumes, and owner-to-neighbour
    /// face orientation.
    pub fn validate(&self) -> Result<()> {
        let nf = self.n_faces();
        let ni = self.n_internal_faces();
        for f in 0..ni {
            if self.topo.owner[f] >= self.topo.neighbour[f] {
                return err(format!("internal face {f}: owner must be < neighbour"));
            }
            if f > 0
                && (self.topo.owner[f - 1], self.topo.neighbour[f - 1]) >= (self.topo.owner[f], self.topo.neighbour[f])
            {
                return err(format!(
                    "internal face {f}: faces must be ordered by (owner, neighbour) with no duplicates"
                ));
            }
        }
        let mut next = ni;
        for p in &self.topo.patches {
            if p.start != next {
                return err(format!("patch '{}' does not start at face {next}", p.name));
            }
            next += p.size;
        }
        if next != nf {
            return err(format!("patches cover faces up to {next} but mesh has {nf} faces"));
        }
        let mut names: Vec<&str> = self.topo.patches.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        if names.windows(2).any(|w| w[0] == w[1]) {
            return err("duplicate patch names");
        }
        for c in 0..self.n_cells {
            let range = self.cell_face_offsets[c]..self.cell_face_offsets[c + 1];
            if range.len() < 4 {
                return err(format!("cell {c} has fewer than 4 faces"));
            }
            let mut edges: Vec<(usize, usize)> = Vec::new();
            let mut sum = vec3::ZERO;
            let mut mag = 0.0;
            for &f in &self.cell_faces[range] {
                let ids = self.face_points(f);
                let is_owner = self.topo.owner[f] == c;
                let n = ids.len();
                for i in 0..n {
                    let (a, b) = (ids[i], ids[(i + 1) % n]);
                    edges.push(if is_owner { (a, b) } else { (b, a) });
                }
                let sf = if is_owner {
                    self.face_area_vectors[f]
                } else {
                    vec3::scale(self.face_area_vectors[f], -1.0)
                };
                sum = vec3::add(sum, sf);
                mag += vec3::norm(sf);
            }
            edges.sort_unstable();
            if edges.windows(2).any(|w| w[0] == w[1]) {
                return err(format!(
                    "cell {c}: a directed edge appears twice (inconsistent face orientation)"
                ));
            }
            for &(a, b) in &edges {
                if edges.binary_search(&(b, a)).is_err() {
                    return err(format!(
                        "cell {c} is not closed: edge ({a},{b}) has no matching reverse edge"
                    ));
                }
            }
            if vec3::norm(sum) > 1e-9 * mag.max(1e-300) {
                return err(format!("cell {c}: face area vectors do not sum to zero"));
            }
            if self.cell_volumes[c] <= 0.0 {
                return err(format!("cell {c} has non-positive volume {}", self.cell_volumes[c]));
            }
        }
        for f in 0..ni {
            let d = vec3::sub(
                self.cell_centres[self.topo.neighbour[f]],
                self.cell_centres[self.topo.owner[f]],
            );
            if vec3::dot(d, self.face_area_vectors[f]) <= 0.0 {
                return err(format!(
                    "internal face {f}: normal does not point from owner to neighbour"
                ));
            }
        }
        for z in self.topo.cell_zones.iter() {
            if z.ids.iter().any(|&c| c >= self.n_cells) {
                return err(format!("cell zone '{}' has out-of-range cell", z.name));
            }
        }
        for z in self.topo.face_zones.iter() {
            if z.ids.iter().any(|&f| f >= nf) {
                return err(format!("face zone '{}' has out-of-range face", z.name));
            }
        }
        Ok(())
    }

    /// Topology (for serialization).
    pub fn topology(&self) -> &MeshTopology {
        &self.topo
    }
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.topo.points.len()
    }
    /// Number of cells.
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }
    /// Number of faces.
    pub fn n_faces(&self) -> usize {
        self.topo.owner.len()
    }
    /// Number of internal faces.
    pub fn n_internal_faces(&self) -> usize {
        self.topo.neighbour.len()
    }
    /// Number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.n_faces() - self.n_internal_faces()
    }
    /// Point coordinates.
    pub fn points(&self) -> &[Vec3] {
        &self.topo.points
    }
    /// Point loop of face `f`.
    pub fn face_points(&self, f: usize) -> &[usize] {
        &self.topo.face_points[self.topo.face_offsets[f]..self.topo.face_offsets[f + 1]]
    }
    /// Owner cells (all faces).
    pub fn owner(&self) -> &[usize] {
        &self.topo.owner
    }
    /// Neighbour cells (internal faces only).
    pub fn neighbour(&self) -> &[usize] {
        &self.topo.neighbour
    }
    /// Face centres.
    pub fn face_centres(&self) -> &[Vec3] {
        &self.face_centres
    }
    /// Face area vectors (owner to neighbour / outward at boundary).
    pub fn face_area_vectors(&self) -> &[Vec3] {
        &self.face_area_vectors
    }
    /// Face area magnitude.
    pub fn face_area(&self, f: usize) -> f64 {
        vec3::norm(self.face_area_vectors[f])
    }
    /// Cell centroids.
    pub fn cell_centres(&self) -> &[Vec3] {
        &self.cell_centres
    }
    /// Cell volumes.
    pub fn cell_volumes(&self) -> &[f64] {
        &self.cell_volumes
    }
    /// Total mesh volume.
    pub fn total_volume(&self) -> f64 {
        self.cell_volumes.iter().sum()
    }
    /// Faces of cell `c` (internal and boundary).
    pub fn cell_faces(&self, c: usize) -> &[usize] {
        &self.cell_faces[self.cell_face_offsets[c]..self.cell_face_offsets[c + 1]]
    }
    /// Cells sharing an internal face with `c`.
    pub fn cell_neighbours(&self, c: usize) -> Vec<usize> {
        self.cell_faces(c)
            .iter()
            .filter(|&&f| f < self.n_internal_faces())
            .map(|&f| {
                if self.topo.owner[f] == c {
                    self.topo.neighbour[f]
                } else {
                    self.topo.owner[f]
                }
            })
            .collect()
    }
    /// Patches.
    pub fn patches(&self) -> &[Patch] {
        &self.topo.patches
    }
    /// Patch by name.
    pub fn patch(&self, name: &str) -> Option<&Patch> {
        self.topo.patches.iter().find(|p| p.name == name)
    }
    /// Patch containing boundary face `f`.
    pub fn patch_of_face(&self, f: usize) -> Option<&Patch> {
        self.topo.patches.iter().find(|p| p.faces().contains(&f))
    }
    /// Set the periodic partner of a cyclic patch.
    pub fn set_periodic(&mut self, patch: &str, partner: &str) -> Result<()> {
        if self.patch(partner).is_none() {
            return err(format!("unknown patch '{partner}'"));
        }
        let p = self
            .topo
            .patches
            .iter_mut()
            .find(|p| p.name == patch)
            .ok_or_else(|| Error::Mesh(format!("unknown patch '{patch}'")))?;
        p.kind = PatchKind::Cyclic;
        p.periodic_with = Some(partner.to_string());
        p.rotation = None;
        Ok(())
    }

    /// Make `a` and `b` a rotationally periodic pair: `b` is `a` rotated by
    /// `angle` (radians, right-handed) about the axis through `origin`.
    pub fn set_periodic_rotation(&mut self, a: &str, b: &str, origin: Vec3, axis: Vec3, angle: f64) -> Result<()> {
        let rot = PeriodicRotation {
            origin,
            axis: vec3::unit(axis),
            angle,
        };
        self.set_periodic(a, b)?;
        self.set_periodic(b, a)?;
        for (name, r) in [(a, rot), (b, rot.inverse())] {
            if let Some(p) = self.topo.patches.iter_mut().find(|p| p.name == name) {
                p.rotation = Some(r);
            }
        }
        Ok(())
    }
    /// Cell zones.
    pub fn cell_zones(&self) -> &[Zone] {
        &self.topo.cell_zones
    }
    /// Face zones.
    pub fn face_zones(&self) -> &[Zone] {
        &self.topo.face_zones
    }
    /// Add (or replace) a cell zone.
    pub fn add_cell_zone(&mut self, name: &str, mut ids: Vec<usize>) -> Result<()> {
        ids.sort_unstable();
        ids.dedup();
        if ids.last().is_some_and(|&c| c >= self.n_cells) {
            return err(format!("cell zone '{name}' has out-of-range cell"));
        }
        self.topo.cell_zones.retain(|z| z.name != name);
        self.topo.cell_zones.push(Zone { name: name.into(), ids });
        Ok(())
    }
    /// Add (or replace) a face zone.
    pub fn add_face_zone(&mut self, name: &str, mut ids: Vec<usize>) -> Result<()> {
        ids.sort_unstable();
        ids.dedup();
        if ids.last().is_some_and(|&f| f >= self.n_faces()) {
            return err(format!("face zone '{name}' has out-of-range face"));
        }
        self.topo.face_zones.retain(|z| z.name != name);
        self.topo.face_zones.push(Zone { name: name.into(), ids });
        Ok(())
    }
    /// Cell zone by name.
    pub fn cell_zone(&self, name: &str) -> Option<&Zone> {
        self.topo.cell_zones.iter().find(|z| z.name == name)
    }

    /// Order-sensitive content hash of topology and coordinates.
    pub fn content_hash(&self) -> u64 {
        let mut h = rcfd_core::hash::Fnv64::new();
        for p in &self.topo.points {
            for x in p {
                h.write_f64(*x);
            }
        }
        for v in self
            .topo
            .face_offsets
            .iter()
            .chain(&self.topo.face_points)
            .chain(&self.topo.owner)
            .chain(&self.topo.neighbour)
        {
            h.write_u64(*v as u64);
        }
        for p in &self.topo.patches {
            h.write(p.name.as_bytes());
            h.write_u64(p.start as u64);
            h.write_u64(p.size as u64);
        }
        for z in self.topo.cell_zones.iter().chain(&self.topo.face_zones) {
            h.write(z.name.as_bytes());
            for i in &z.ids {
                h.write_u64(*i as u64);
            }
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit cube cell as 6 quad faces (corners numbered binary xyz).
    pub(crate) fn cube_points(dx: f64, dy: f64, dz: f64, ox: f64) -> Vec<Vec3> {
        (0..8)
            .map(|i| {
                [
                    ox + dx * (i & 1) as f64,
                    dy * ((i >> 1) & 1) as f64,
                    dz * ((i >> 2) & 1) as f64,
                ]
            })
            .collect()
    }
    pub(crate) fn hex_faces(p: [usize; 8]) -> Vec<Vec<usize>> {
        // p index = x + 2y + 4z
        vec![
            vec![p[0], p[2], p[6], p[4]],
            vec![p[1], p[3], p[7], p[5]],
            vec![p[0], p[1], p[5], p[4]],
            vec![p[2], p[3], p[7], p[6]],
            vec![p[0], p[1], p[3], p[2]],
            vec![p[4], p[5], p[7], p[6]],
        ]
    }

    #[test]
    fn single_cube_geometry() {
        let pts = cube_points(2.0, 3.0, 4.0, 0.0);
        let cells = vec![hex_faces([0, 1, 2, 3, 4, 5, 6, 7])];
        let m = Mesh::from_cell_faces(pts, &cells, &[]).unwrap();
        assert_eq!(m.n_cells(), 1);
        assert_eq!(m.n_faces(), 6);
        assert_eq!(m.n_internal_faces(), 0);
        assert!((m.cell_volumes()[0] - 24.0).abs() < 1e-12);
        let c = m.cell_centres()[0];
        assert!((c[0] - 1.0).abs() < 1e-12 && (c[1] - 1.5).abs() < 1e-12 && (c[2] - 2.0).abs() < 1e-12);
        // areas: 2 faces each of 12, 8, 6
        let mut areas: Vec<f64> = (0..6).map(|f| m.face_area(f)).collect();
        areas.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (a, e) in areas.iter().zip([6.0, 6.0, 8.0, 8.0, 12.0, 12.0]) {
            assert!((a - e).abs() < 1e-12);
        }
        // outward normals: sum of Sf = 0, each points away from centre
        for f in 0..6 {
            let d = vec3::sub(m.face_centres()[f], c);
            assert!(vec3::dot(d, m.face_area_vectors()[f]) > 0.0);
        }
        assert_eq!(m.patches().len(), 1);
        assert_eq!(m.patches()[0].name, "defaultFaces");
    }

    #[test]
    fn two_cells_share_a_face() {
        // two unit cubes along x, sharing 4 points
        let mut pts = Vec::new();
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    pts.push([x as f64, y as f64, z as f64]);
                }
            }
        }
        let id = |x: usize, y: usize, z: usize| x + 3 * y + 6 * z;
        let hex = |x0: usize| {
            let mut p = [0; 8];
            for (i, pi) in p.iter_mut().enumerate() {
                *pi = id(x0 + (i & 1), (i >> 1) & 1, i >> 2);
            }
            hex_faces(p)
        };
        // give cells in reverse order to verify owner<neighbour canonicalisation
        let cells = vec![hex(1), hex(0)];
        let left = BoundarySpec {
            name: "left".into(),
            kind: PatchKind::Inlet,
            faces: vec![vec![id(0, 0, 0), id(0, 1, 0), id(0, 1, 1), id(0, 0, 1)]],
        };
        let m = Mesh::from_cell_faces(pts, &cells, &[left]).unwrap();
        assert_eq!(m.n_cells(), 2);
        assert_eq!(m.n_internal_faces(), 1);
        assert_eq!(m.n_faces(), 11);
        assert_eq!(m.owner()[0], 0);
        assert_eq!(m.neighbour()[0], 1);
        assert!((m.total_volume() - 2.0).abs() < 1e-12);
        // internal normal points from cell 0 (x in [1,2]) to cell 1 (x in [0,1]) i.e. -x
        let sf = m.face_area_vectors()[0];
        assert!((sf[0] + 1.0).abs() < 1e-12 && sf[1].abs() < 1e-12);
        assert_eq!(m.patch("left").unwrap().size, 1);
        assert_eq!(m.patch("left").unwrap().kind, PatchKind::Inlet);
        assert_eq!(m.patch("defaultFaces").unwrap().size, 9);
        assert_eq!(m.cell_neighbours(0), vec![1]);
        assert_eq!(m.patch_of_face(m.patch("left").unwrap().start).unwrap().name, "left");
        // boundary normals outward
        for f in m.n_internal_faces()..m.n_faces() {
            let c = m.cell_centres()[m.owner()[f]];
            assert!(vec3::dot(vec3::sub(m.face_centres()[f], c), m.face_area_vectors()[f]) > 0.0);
        }
    }

    #[test]
    fn triangular_prism_volume_and_centroid() {
        // right prism: triangle (0,0),(2,0),(0,2) extruded by 3 in z -> V = 2*3 = 6
        let pts = vec![
            [0., 0., 0.],
            [2., 0., 0.],
            [0., 2., 0.],
            [0., 0., 3.],
            [2., 0., 3.],
            [0., 2., 3.],
        ];
        let cell = vec![
            vec![0, 1, 2],
            vec![3, 4, 5],
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![2, 0, 3, 5],
        ];
        let m = Mesh::from_cell_faces(pts, &[cell], &[]).unwrap();
        assert!((m.cell_volumes()[0] - 6.0).abs() < 1e-12);
        let c = m.cell_centres()[0];
        assert!((c[0] - 2.0 / 3.0).abs() < 1e-12 && (c[1] - 2.0 / 3.0).abs() < 1e-12 && (c[2] - 1.5).abs() < 1e-12);
    }

    #[test]
    fn tetrahedron_and_pyramid() {
        let pts = vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        let tet = vec![vec![0, 1, 2], vec![0, 1, 3], vec![0, 2, 3], vec![1, 2, 3]];
        let m = Mesh::from_cell_faces(pts, &[tet], &[]).unwrap();
        assert!((m.cell_volumes()[0] - 1.0 / 6.0).abs() < 1e-14);
        let c = m.cell_centres()[0];
        for x in c {
            assert!((x - 0.25).abs() < 1e-14);
        }
        // square pyramid base 1x1 height 1: V = 1/3, centroid z = 1/4
        let pts = vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.], [0.5, 0.5, 1.]];
        let pyr = vec![
            vec![0, 1, 2, 3],
            vec![0, 1, 4],
            vec![1, 2, 4],
            vec![2, 3, 4],
            vec![3, 0, 4],
        ];
        let m = Mesh::from_cell_faces(pts, &[pyr], &[]).unwrap();
        assert!((m.cell_volumes()[0] - 1.0 / 3.0).abs() < 1e-14);
        assert!((m.cell_centres()[0][2] - 0.25).abs() < 1e-14);
    }

    #[test]
    fn invariants_detect_bad_meshes() {
        let pts = cube_points(1.0, 1.0, 1.0, 0.0);
        let mut faces = hex_faces([0, 1, 2, 3, 4, 5, 6, 7]);
        // orient all faces outward by building through from_cell_faces, then corrupt
        let good = Mesh::from_cell_faces(pts.clone(), &[faces.clone()], &[]).unwrap();
        let topo = good.topology().clone();
        // 1. flip one face -> orientation inconsistency
        let mut bad = topo.clone();
        let (a, b) = (bad.face_offsets[2], bad.face_offsets[3]);
        bad.face_points[a..b].reverse();
        assert!(Mesh::from_topology(bad).is_err());
        // 2. open cell: only five faces
        let mut open = topo.clone();
        open.face_offsets.pop();
        open.owner.pop();
        open.face_points.truncate(*open.face_offsets.last().unwrap());
        open.patches[0].size = 4;
        assert!(Mesh::from_topology(open).is_err());
        // 3. patch coverage mismatch
        let mut badp = topo.clone();
        badp.patches[0].size = 5;
        assert!(Mesh::from_topology(badp).is_err());
        // 4. face with bad point index
        let mut badi = topo;
        badi.face_points[0] = 99;
        assert!(Mesh::from_topology(badi).is_err());
        // 5. face shared by three cells
        faces.truncate(6);
        let cells = vec![faces.clone(), faces.clone(), faces];
        assert!(Mesh::from_cell_faces(pts, &cells, &[]).is_err());
    }

    #[test]
    fn zones_and_hash() {
        let pts = cube_points(1.0, 1.0, 1.0, 0.0);
        let mut m = Mesh::from_cell_faces(pts, &[hex_faces([0, 1, 2, 3, 4, 5, 6, 7])], &[]).unwrap();
        let h0 = m.content_hash();
        m.add_cell_zone("fluid", vec![0, 0]).unwrap();
        assert_eq!(m.cell_zone("fluid").unwrap().ids, vec![0]);
        assert!(m.add_cell_zone("bad", vec![3]).is_err());
        m.add_face_zone("f", vec![5, 1]).unwrap();
        assert!(m.add_face_zone("f2", vec![6]).is_err());
        assert_ne!(m.content_hash(), h0);
        let m2 = Mesh::from_topology(m.topology().clone()).unwrap();
        assert_eq!(m.content_hash(), m2.content_hash());
    }
}
