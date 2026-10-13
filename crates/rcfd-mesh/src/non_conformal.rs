//! Non-conformal mesh interfaces (F-114): two meshes whose interface patches do not match face by face are joined into
//! one ordinary conformal polyhedral mesh by intersecting the two patches.
//!
//! Design: the finite-volume solver is not touched. Every non-empty overlap of a face of patch A with a face of patch B
//! becomes one internal face with the A cell as owner and the B cell as neighbour, whose area vector is the overlap area
//! times the common normal and whose centre is the overlap centroid. The original interface faces are deleted. Since
//! the overlaps tile every original face, every cell stays closed (its face area vectors sum to zero to round-off) and
//! whatever flux leaves the A side through the interface enters the B side, by construction, to round-off.
//!
//! Method: both patches must be planar, coplanar and face each other. Their faces are projected to plane coordinates;
//! each A face is clipped against each B face (Sutherland-Hodgman, so faces must be convex, which holds for the
//! quadrilaterals of block meshes and for most polygons of unstructured meshes). Points of the two patches and the
//! intersection points are merged into one registry (merge distance `tol` times the patch size), the B points that
//! coincide with an A point are replaced by it, and every face edge lying in the plane receives the registry points that
//! lie on it (the hanging nodes), so that cells remain topologically closed in the sense of [`Mesh::validate`].
//!
//! Limits: the patches must cover each other completely (error otherwise); face zones are dropped; periodic
//! information of the joined patches is not carried over.

use crate::mesh::{Mesh, PatchKind};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

/// What the join did.
#[derive(Debug, Clone, Default)]
pub struct JoinReport {
    /// Number of overlap faces created.
    pub n_overlap_faces: usize,
    /// Overlaps dropped because their area was below `1e-10` of the smaller parent face.
    pub n_dropped: usize,
    /// Number of face edges that received hanging-node points.
    pub n_split_edges: usize,
    /// Total area of patch A and of patch B.
    pub area: [f64; 2],
    /// Worst relative difference between a parent face area and the sum of its overlap areas.
    pub max_tiling_error: f64,
    /// Indices (in the joined mesh) of the overlap faces.
    pub interface_faces: Vec<usize>,
    /// Cells of mesh A come first; the first cell index of mesh B in the joined mesh.
    pub b_cell_offset: usize,
}

type PatchFaces = Vec<(usize, Vec<usize>)>;

struct Registry {
    d: f64,
    bin: f64,
    coords: Vec<[f64; 2]>,
    gid: Vec<usize>,
    bins: HashMap<(i64, i64), Vec<usize>>,
    by_gid: HashMap<usize, usize>,
}

impl Registry {
    fn new(d: f64, bin: f64) -> Self {
        Registry {
            d,
            bin: bin.max(4.0 * d),
            coords: vec![],
            gid: vec![],
            bins: HashMap::new(),
            by_gid: HashMap::new(),
        }
    }
    fn key(&self, p: [f64; 2]) -> (i64, i64) {
        ((p[0] / self.bin).floor() as i64, (p[1] / self.bin).floor() as i64)
    }
    fn find(&self, p: [f64; 2]) -> Option<usize> {
        let (kx, ky) = self.key(p);
        let mut best: Option<(f64, usize)> = None;
        for i in kx - 1..=kx + 1 {
            for j in ky - 1..=ky + 1 {
                for &k in self.bins.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                    let dist = (self.coords[k][0] - p[0]).hypot(self.coords[k][1] - p[1]);
                    if dist <= self.d && best.map_or(true, |(b, _)| dist < b) {
                        best = Some((dist, k));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }
    fn insert(&mut self, p: [f64; 2], gid: usize) -> usize {
        let k = self.coords.len();
        self.coords.push(p);
        self.gid.push(gid);
        self.bins.entry(self.key(p)).or_default().push(k);
        self.by_gid.insert(gid, k);
        k
    }
    /// Registry points strictly inside the segment `p0 -> p1` (further than `d` from both ends and within `d` of the
    /// line), ordered from `p0`.
    fn on_segment(&self, p0: [f64; 2], p1: [f64; 2]) -> Vec<usize> {
        let e = [p1[0] - p0[0], p1[1] - p0[1]];
        let len = e[0].hypot(e[1]);
        if len <= 2.0 * self.d {
            return vec![];
        }
        let (lo, hi) = (
            self.key([p0[0].min(p1[0]) - self.d, p0[1].min(p1[1]) - self.d]),
            self.key([p0[0].max(p1[0]) + self.d, p0[1].max(p1[1]) + self.d]),
        );
        let mut found: Vec<(f64, usize)> = vec![];
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                for &k in self.bins.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                    let r = [self.coords[k][0] - p0[0], self.coords[k][1] - p0[1]];
                    let s = (r[0] * e[0] + r[1] * e[1]) / len;
                    let off = (r[0] * e[1] - r[1] * e[0]).abs() / len;
                    if off <= self.d && s > self.d && s < len - self.d {
                        found.push((s, self.gid[k]));
                    }
                }
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|f| f.1).collect()
    }
}

fn shoelace(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    0.5 * (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
}

/// Sutherland-Hodgman: `subject` clipped against the convex counter-clockwise polygon `clip`.
fn clip_polygon(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (c0, c1) = (clip[i], clip[(i + 1) % clip.len()]);
        let e = [c1[0] - c0[0], c1[1] - c0[1]];
        let len = e[0].hypot(e[1]);
        let side = |p: [f64; 2]| (e[0] * (p[1] - c0[1]) - e[1] * (p[0] - c0[0])) / len;
        let input = std::mem::take(&mut out);
        let mut prev = input[input.len() - 1];
        let mut sp = side(prev);
        for &cur in &input {
            let sc = side(cur);
            let cross = |a: [f64; 2], b: [f64; 2], sa: f64, sb: f64| {
                let t = sa / (sa - sb);
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            };
            if sc >= 0.0 {
                if sp < 0.0 {
                    out.push(cross(prev, cur, sp, sc));
                }
                out.push(cur);
            } else if sp >= 0.0 {
                out.push(cross(prev, cur, sp, sc));
            }
            prev = cur;
            sp = sc;
        }
    }
    out
}

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// Join mesh `b` to mesh `a` across their interface patches `patch_a` and `patch_b` (see the module documentation).
///
/// `tol` is a relative geometric tolerance (a fraction of the interface size) for coplanarity and point merging,
/// typically `1e-8`. Cells of `a` keep their indices, cells of `b` follow (offset [`JoinReport::b_cell_offset`]).
/// Boundary patches of the same name in both meshes are merged (their kinds must agree); the two interface patches
/// disappear.
pub fn join_non_conformal(a: &Mesh, patch_a: &str, b: &Mesh, patch_b: &str, tol: f64) -> Result<(Mesh, JoinReport)> {
    let pa = a
        .patch(patch_a)
        .ok_or_else(|| Error::Mesh(format!("mesh A has no patch '{patch_a}'")))?
        .clone();
    let pb = b
        .patch(patch_b)
        .ok_or_else(|| Error::Mesh(format!("mesh B has no patch '{patch_b}'")))?
        .clone();
    if pa.size == 0 || pb.size == 0 {
        return bad("empty interface patch");
    }
    // plane from patch A
    let mut nsum = vec3::ZERO;
    for f in pa.faces() {
        nsum = vec3::add(nsum, a.face_area_vectors()[f]);
    }
    let n = vec3::unit(nsum);
    let o = a.face_centres()[pa.start];
    let mut e1 = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    e1 = vec3::unit(vec3::sub(e1, vec3::scale(n, vec3::dot(e1, n))));
    let e2 = vec3::cross(n, e1);
    let to_plane = |p: Vec3| -> ([f64; 2], f64) {
        let r = vec3::sub(p, o);
        ([vec3::dot(r, e1), vec3::dot(r, e2)], vec3::dot(r, n))
    };
    // extent and planarity
    let np_a = a.n_points();
    let mut ext = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    let mut max_off: f64 = 0.0;
    let gather = |m: &Mesh, patch: &crate::mesh::Patch, sign: f64| -> Result<()> {
        for f in patch.faces() {
            let un = vec3::unit(m.face_area_vectors()[f]);
            if sign * vec3::dot(un, n) < 1.0 - 1e-6 {
                return bad(format!(
                    "interface face {f} is not parallel to the interface plane (planar, opposed patches required)"
                ));
            }
        }
        Ok(())
    };
    gather(a, &pa, 1.0)?;
    gather(b, &pb, -1.0)?;
    for (m, patch) in [(a, &pa), (b, &pb)] {
        for f in patch.faces() {
            for &p in m.face_points(f) {
                let (c, off) = to_plane(m.points()[p]);
                max_off = max_off.max(off.abs());
                for k in 0..2 {
                    ext[0][k] = ext[0][k].min(c[k]);
                    ext[1][k] = ext[1][k].max(c[k]);
                }
            }
        }
    }
    let size = (ext[1][0] - ext[0][0]).hypot(ext[1][1] - ext[0][1]);
    let d = tol * size;
    if max_off > 1e3 * d {
        return bad(format!(
            "interface patches are not coplanar: max distance {max_off:.3e} from the plane (merge tolerance {d:.3e})"
        ));
    }
    let n_pts_est = (pa.size + pb.size) as f64;
    let mut reg = Registry::new(d, size / n_pts_est.sqrt().max(1.0));
    // combined points
    let mut points: Vec<Vec3> = a.points().to_vec();
    points.extend_from_slice(b.points());
    let mut remap: Vec<usize> = (0..points.len()).collect();
    for f in pa.faces() {
        for &p in a.face_points(f) {
            if !reg.by_gid.contains_key(&p) {
                reg.insert(to_plane(a.points()[p]).0, p);
            }
        }
    }
    for f in pb.faces() {
        for &p in b.face_points(f) {
            let g = np_a + p;
            if remap[g] != g || reg.by_gid.contains_key(&g) {
                continue;
            }
            let c = to_plane(b.points()[p]).0;
            match reg.find(c) {
                Some(k) => remap[g] = reg.gid[k],
                None => {
                    reg.insert(c, g);
                }
            }
        }
    }
    // overlaps
    struct Overlap {
        owner: usize,
        neighbour: usize,
        ids: Vec<usize>,
    }
    let n_cells_a = a.n_cells();
    let mut overlaps: Vec<Overlap> = vec![];
    let mut n_dropped = 0;
    let plane_loop = |m: &Mesh, f: usize, off: usize, rev: bool, reg: &Registry, remap: &[usize]| {
        let mut v: Vec<[f64; 2]> = m
            .face_points(f)
            .iter()
            .map(|&p| reg.coords[reg.by_gid[&remap[off + p]]])
            .collect();
        if rev {
            v.reverse();
        }
        v
    };
    let b_faces: Vec<(usize, Vec<[f64; 2]>)> = pb
        .faces()
        .map(|f| (f, plane_loop(b, f, np_a, true, &reg, &remap)))
        .collect();
    let b_box: Vec<([f64; 2], [f64; 2])> = b_faces
        .iter()
        .map(|(_, l)| {
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for p in l {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            (lo, hi)
        })
        .collect();
    let mut tiling_a = vec![0.0; pa.size];
    let mut tiling_b = vec![0.0; pb.size];
    let area_a_face: Vec<f64> = pa
        .faces()
        .map(|f| shoelace(&plane_loop(a, f, 0, false, &reg, &remap)))
        .collect();
    let area_b_face: Vec<f64> = b_faces.iter().map(|(_, l)| shoelace(l)).collect();
    if area_a_face.iter().chain(&area_b_face).any(|&x| x <= 0.0) {
        return bad("an interface face has non-positive area in the interface plane (non-convex or inverted face)");
    }
    for (ia, fa) in pa.faces().enumerate() {
        let sa = plane_loop(a, fa, 0, false, &reg, &remap);
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &sa {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        for (ib, (fb, cb)) in b_faces.iter().enumerate() {
            let (blo, bhi) = b_box[ib];
            if blo[0] > hi[0] || bhi[0] < lo[0] || blo[1] > hi[1] || bhi[1] < lo[1] {
                continue;
            }
            let poly = clip_polygon(&sa, cb);
            if poly.len() < 3 {
                continue;
            }
            let area = shoelace(&poly);
            if area <= 1e-10 * area_a_face[ia].min(area_b_face[ib]) {
                n_dropped += 1;
                continue;
            }
            let mut ids: Vec<usize> = vec![];
            for p in &poly {
                let g = match reg.find(*p) {
                    Some(k) => reg.gid[k],
                    None => {
                        let g = points.len();
                        points.push(vec3::add(o, vec3::add(vec3::scale(e1, p[0]), vec3::scale(e2, p[1]))));
                        remap.push(g);
                        reg.insert(*p, g);
                        g
                    }
                };
                if ids.last() != Some(&g) {
                    ids.push(g);
                }
            }
            if ids.len() > 1 && ids[0] == ids[ids.len() - 1] {
                ids.pop();
            }
            if ids.len() < 3 {
                n_dropped += 1;
                continue;
            }
            let area = shoelace(&ids.iter().map(|g| reg.coords[reg.by_gid[g]]).collect::<Vec<_>>());
            tiling_a[ia] += area;
            tiling_b[ib] += area;
            overlaps.push(Overlap {
                owner: a.owner()[fa],
                neighbour: n_cells_a + b.owner()[*fb],
                ids,
            });
        }
    }
    let mut max_tiling_error: f64 = 0.0;
    for (t, a0) in tiling_a
        .iter()
        .zip(&area_a_face)
        .chain(tiling_b.iter().zip(&area_b_face))
    {
        max_tiling_error = max_tiling_error.max((t - a0).abs() / a0);
    }
    if max_tiling_error > 1e-6 {
        return bad(format!(
            "the interface patches do not cover each other: worst face tiling error {max_tiling_error:.3e}"
        ));
    }
    // faces of the joined mesh: (owner, neighbour or usize::MAX, loop)
    let mut n_split = 0;
    let mut split = |ids: Vec<usize>, reg: &Registry| -> Vec<usize> {
        let n = ids.len();
        let mut out = Vec::with_capacity(n + 2);
        for i in 0..n {
            let (p, q) = (ids[i], ids[(i + 1) % n]);
            out.push(p);
            if let (Some(&kp), Some(&kq)) = (reg.by_gid.get(&p), reg.by_gid.get(&q)) {
                let mids = reg.on_segment(reg.coords[kp], reg.coords[kq]);
                if !mids.is_empty() {
                    n_split += 1;
                    out.extend(mids);
                }
            }
        }
        out
    };
    let mut internal: Vec<(usize, usize, Vec<usize>, bool)> = vec![];
    for f in 0..a.n_internal_faces() {
        internal.push((
            a.owner()[f],
            a.neighbour()[f],
            split(a.face_points(f).to_vec(), &reg),
            false,
        ));
    }
    for f in 0..b.n_internal_faces() {
        let ids: Vec<usize> = b.face_points(f).iter().map(|&p| remap[np_a + p]).collect();
        internal.push((
            n_cells_a + b.owner()[f],
            n_cells_a + b.neighbour()[f],
            split(ids, &reg),
            false,
        ));
    }
    for ov in &overlaps {
        internal.push((ov.owner, ov.neighbour, split(ov.ids.clone(), &reg), true));
    }
    internal.sort_by_key(|x| (x.0, x.1));
    if internal.windows(2).any(|w| (w[0].0, w[0].1) == (w[1].0, w[1].1)) {
        return bad("two cells share more than one face (an A cell overlaps a B cell in two pieces)");
    }
    let interface_faces: Vec<usize> = internal
        .iter()
        .enumerate()
        .filter(|(_, x)| x.3)
        .map(|(i, _)| i)
        .collect();
    // boundary patches, merged by name
    let mut pnames: Vec<(String, PatchKind, PatchFaces)> = vec![];
    for (m, skip, off_c, off_p, is_b) in [(a, &pa.name, 0, 0, false), (b, &pb.name, n_cells_a, np_a, true)] {
        for p in m.patches() {
            if &p.name == skip {
                continue;
            }
            let faces: PatchFaces = p
                .faces()
                .map(|f| {
                    let ids: Vec<usize> = m
                        .face_points(f)
                        .iter()
                        .map(|&q| if is_b { remap[off_p + q] } else { q })
                        .collect();
                    (off_c + m.owner()[f], split(ids, &reg))
                })
                .collect();
            match pnames.iter_mut().find(|x| x.0 == p.name) {
                Some(x) => {
                    if x.1 != p.kind {
                        return bad(format!("patch '{}' has different kinds in the two meshes", p.name));
                    }
                    x.2.extend(faces);
                }
                None => pnames.push((p.name.clone(), p.kind.clone(), faces)),
            }
        }
    }
    // assemble with compacted points
    let mut used = vec![usize::MAX; points.len()];
    let mut new_points: Vec<Vec3> = vec![];
    let mut renum = |g: usize| -> usize {
        if used[g] == usize::MAX {
            used[g] = new_points.len();
            new_points.push(points[g]);
        }
        used[g]
    };
    let mut faces: Vec<Vec<usize>> = vec![];
    let mut owner = vec![];
    let mut neighbour = vec![];
    for (o_, n_, ids, _) in &internal {
        faces.push(ids.iter().map(|&g| renum(g)).collect());
        owner.push(*o_);
        neighbour.push(*n_);
    }
    let mut patches = vec![];
    for (name, kind, fl) in &pnames {
        patches.push((name.clone(), kind.clone(), fl.len()));
        for (oc, ids) in fl {
            faces.push(ids.iter().map(|&g| renum(g)).collect());
            owner.push(*oc);
        }
    }
    let mut mesh = Mesh::from_face_lists(new_points, faces, owner, neighbour, patches)?;
    // cell zones: merged by name, B ids shifted
    let mut zones: Vec<(String, Vec<usize>)> = vec![];
    for (m, off) in [(a, 0), (b, n_cells_a)] {
        for z in m.cell_zones() {
            let ids: Vec<usize> = z.ids.iter().map(|&c| c + off).collect();
            match zones.iter_mut().find(|x| x.0 == z.name) {
                Some(x) => x.1.extend(ids),
                None => zones.push((z.name.clone(), ids)),
            }
        }
    }
    for (name, ids) in zones {
        mesh.add_cell_zone(&name, ids)?;
    }
    let area = [
        pa.faces().map(|f| a.face_area(f)).sum::<f64>(),
        pb.faces().map(|f| b.face_area(f)).sum::<f64>(),
    ];
    Ok((
        mesh,
        JoinReport {
            n_overlap_faces: overlaps.len(),
            n_dropped,
            n_split_edges: n_split,
            area,
            max_tiling_error,
            interface_faces,
            b_cell_offset: n_cells_a,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{channel, warp, Grading};

    fn block(nx: usize, ny: usize, x0: f64) -> Mesh {
        let m = channel([nx, ny, 1], [1.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
        warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
    }

    #[test]
    fn matching_blocks_reproduce_the_conformal_mesh() {
        let (m, rep) = join_non_conformal(&block(4, 5, 0.0), "outlet", &block(4, 5, 1.0), "inlet", 1e-8).unwrap();
        assert_eq!(rep.n_overlap_faces, 5);
        assert_eq!(m.n_cells(), 40);
        assert_eq!(m.n_internal_faces(), 2 * (3 * 5 + 4 * 4) + 5);
        assert!((m.total_volume() - 0.2).abs() < 1e-14);
    }

    #[test]
    fn mismatched_blocks_close_and_tile_to_round_off() {
        let a = block(6, 20, 0.0);
        let b = block(5, 27, 1.0);
        let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
        println!(
            "{} overlap faces, {} split edges, tiling {:.2e}",
            rep.n_overlap_faces, rep.n_split_edges, rep.max_tiling_error
        );
        // 20 and 27 equal intervals of [0,1] overlap in 20 + 27 - 1 pieces
        assert_eq!(rep.n_overlap_faces, 46);
        assert_eq!(m.n_cells(), a.n_cells() + b.n_cells());
        assert!(rep.max_tiling_error < 1e-14, "{}", rep.max_tiling_error);
        assert!((m.total_volume() - a.total_volume() - b.total_volume()).abs() < 1e-14);
        // every cell's area vectors sum to zero to round-off
        let mut worst: f64 = 0.0;
        for c in 0..m.n_cells() {
            let (mut sum, mut mag) = (vec3::ZERO, 0.0);
            for &f in m.cell_faces(c) {
                let s = m.face_area_vectors()[f];
                let sgn = if m.owner()[f] == c { 1.0 } else { -1.0 };
                sum = vec3::add(sum, vec3::scale(s, sgn));
                mag += vec3::norm(s);
            }
            worst = worst.max(vec3::norm(sum) / mag);
        }
        println!("worst closure {worst:.2e}");
        assert!(worst < 1e-15, "{worst}");
        // the sum of the overlap area vectors equals the area vector of each original patch
        let total = rep
            .interface_faces
            .iter()
            .fold(vec3::ZERO, |s, &f| vec3::add(s, m.face_area_vectors()[f]));
        assert!((total[0] - 0.1).abs() < 1e-15 && total[1].abs() < 1e-15, "{total:?}");
        let area: f64 = rep.interface_faces.iter().map(|&f| m.face_area(f)).sum();
        assert!((area - rep.area[0]).abs() < 1e-15 && (area - rep.area[1]).abs() < 1e-15);
        // merged patches keep their total size
        assert_eq!(
            m.patch("walls").unwrap().size,
            a.patch("walls").unwrap().size + b.patch("walls").unwrap().size
        );
    }

    #[test]
    fn partial_cover_and_misaligned_planes_are_errors() {
        let a = block(4, 5, 0.0);
        let shifted = warp(&block(4, 5, 1.0), |p| [p[0], p[1] + 0.3, p[2]]).unwrap();
        let e = join_non_conformal(&a, "outlet", &shifted, "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("cover"), "{e:?}");
        let e = join_non_conformal(&a, "outlet", &block(4, 5, 1.2), "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("coplanar"), "{e:?}");
        let e = join_non_conformal(&a, "inlet", &block(4, 5, 1.0), "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("parallel"), "{e:?}");
        assert!(join_non_conformal(&a, "nope", &block(4, 5, 1.0), "inlet", 1e-8).is_err());
    }

    #[test]
    fn three_dimensional_interface_with_different_cell_counts_in_both_directions() {
        let mk = |n: [usize; 3], x0: f64| {
            let m = channel(
                n,
                [1.0, 1.0, 1.0],
                [Grading::Uniform, Grading::Symmetric(3.0), Grading::Expansion(2.0)],
                false,
            )
            .unwrap();
            warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
        };
        let (a, b) = (mk([3, 4, 5], 0.0), mk([2, 7, 3], 1.0));
        let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
        println!(
            "3-D: {} overlaps, {} split edges, tiling {:.2e}",
            rep.n_overlap_faces, rep.n_split_edges, rep.max_tiling_error
        );
        assert!(rep.max_tiling_error < 1e-13);
        assert!((m.total_volume() - 2.0).abs() < 1e-13);
        let mut worst: f64 = 0.0;
        for c in 0..m.n_cells() {
            let (mut sum, mut mag) = (vec3::ZERO, 0.0);
            for &f in m.cell_faces(c) {
                let sgn = if m.owner()[f] == c { 1.0 } else { -1.0 };
                sum = vec3::add(sum, vec3::scale(m.face_area_vectors()[f], sgn));
                mag += m.face_area(f);
            }
            worst = worst.max(vec3::norm(sum) / mag);
        }
        assert!(worst < 1e-14, "{worst}");
    }
}
