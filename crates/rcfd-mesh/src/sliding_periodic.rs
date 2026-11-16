//! Planar and sector sliding interfaces with periodic seams (F-304, ADR 0629).
//!
//! A rotor block slides against a stator block of the same period: a planar slider periodic in the slide direction
//! (translation by the period vector `t`), or a rotor sector of pitch `Theta` turning against a stator sector of the same
//! pitch about their common axis. The two interface patches each span one period. At an offset `delta` the rotor patch
//! covers `[delta, delta + P]` in the chart coordinate along the slide and the stator patch `[0, P]`, so the part of the
//! rotor beyond the stator's seam faces the stator only through the periodic image.
//!
//! [`join_non_conformal_periodic`] joins such a pair (ADR 0629 M1 to M3): the overlaps of a rotor face with a stator face
//! become internal faces as in [`crate::non_conformal`]; an overlap with the image of a stator face (a *wrap overlap*)
//! becomes a pair of boundary faces, the rotor copy in the patch [`WRAP_A`] and the stator copy (the same polygon moved back
//! by one period, reversed) in [`WRAP_B`], which form a periodic pair, so the finite-volume coupling of periodic faces carries
//! the flux across. The merged boundary patches keep the periodic partners and transforms of the blocks (the existing joins
//! drop them). The two wrap patches come last, so the original boundary faces keep their positions at every offset.
//!
//! [`PeriodicSlidingInterface`] builds the joined mesh at an offset (ADR 0629 M4), always from the rotor's reference points,
//! for `0 <= delta < P`; [`PeriodicSlidingMesh`] maps faces across a step and gives the rotor's swept volumes.
//!
//! [`PeriodicSlidingInterface::mesh_turned`] takes any turned offset `>= 0` (F-704, ADR 0634 R1): the mesh at the offset
//! reduced to one period, with the number of periods taken off in [`PeriodicSlidingMesh::image`]; the step across a change
//! of image re-images the rotor's state (`rcfd_fv::sliding_periodic`).
//!
//! Limits: patches of one period each (a stator of another pitch is refused); planar or cylindrical patches; one interface.

use crate::mesh::{Mesh, PatchKind, PeriodicRotation};
use crate::motion::{mat_vec, moved_mesh, rotation_matrix, swept_volumes_about};
use crate::non_conformal::{clip_polygon, shoelace, Chart, JoinReport, Registry};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

/// Name of the patch of the rotor copies of the wrap overlaps.
pub const WRAP_A: &str = "sliding_wrap_a";
/// Name of the patch of the stator copies of the wrap overlaps.
pub const WRAP_B: &str = "sliding_wrap_b";

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// The periodicity of a sliding pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Periodicity {
    /// Planar slide, periodic under translation by `period` (in the interface plane).
    Translation {
        /// Period vector (the stator's far seam is its near seam moved by it).
        period: Vec3,
    },
    /// Sector, periodic under rotation by `angle` (radians, `0 < angle < pi`) about the axis through `origin`.
    Rotation {
        /// Point on the axis.
        origin: Vec3,
        /// Axis direction (normalised on use).
        axis: Vec3,
        /// Sector pitch.
        angle: f64,
    },
}

impl Periodicity {
    /// The period of the motion: `|period|` (length) or the pitch (radians).
    pub fn motion_period(&self) -> f64 {
        match *self {
            Periodicity::Translation { period } => vec3::norm(period),
            Periodicity::Rotation { angle, .. } => angle,
        }
    }

    /// The point `p` moved by the offset `delta` (translation along the period direction, or rotation about the axis).
    pub fn apply(&self, delta: f64, p: Vec3) -> Vec3 {
        self.mover(delta)(p)
    }

    fn mover(&self, delta: f64) -> impl Fn(Vec3) -> Vec3 {
        let (shift, rot, o) = match *self {
            Periodicity::Translation { period } => (vec3::scale(vec3::unit(period), delta), None, vec3::ZERO),
            Periodicity::Rotation { origin, axis, .. } => {
                (vec3::ZERO, Some(rotation_matrix(vec3::unit(axis), delta)), origin)
            }
        };
        move |p: Vec3| match &rot {
            None => vec3::add(p, shift),
            Some(r) => vec3::add(o, mat_vec(r, vec3::sub(p, o))),
        }
    }
}

/// What [`join_non_conformal_periodic`] did.
#[derive(Debug, Clone, Default)]
pub struct PeriodicJoinReport {
    /// The direct overlaps as in the other joins (`join.interface_faces` the internal interface faces); its tiling error
    /// covers the direct and the wrap overlaps together.
    pub join: JoinReport,
    /// The wrap pairs `(rotor copy, stator copy)` as face indices of the joined mesh (boundary faces of [`WRAP_A`] and
    /// [`WRAP_B`], the i-th of each patch).
    pub wrap_pairs: Vec<(usize, usize)>,
    /// The image `k` of the stator the wrap overlaps were cut with (`+1`, or `-1`; `+1` when there is none): the rotor copy
    /// is the stator copy moved by `k` periods.
    pub shift: i32,
}

type PatchFaces = Vec<(usize, Vec<usize>)>;

/// Join mesh `b` (the stator) to mesh `a` (the rotor) across interface patches that each span one period of `per`
/// (ADR 0629 M1 to M3). Cells of `a` keep their indices, cells of `b` follow. The boundary patches of both meshes are kept
/// (same-name patches merged, `a`'s faces first; kinds, periodic partners and transforms must agree) with their periodic
/// partners and transforms; the patches [`WRAP_A`] and [`WRAP_B`] (a periodic pair) are appended. `tol` is the relative
/// geometric tolerance of the other joins (typically `1e-8`).
pub fn join_non_conformal_periodic(
    a: &Mesh,
    patch_a: &str,
    b: &Mesh,
    patch_b: &str,
    per: &Periodicity,
    tol: f64,
) -> Result<(Mesh, PeriodicJoinReport)> {
    let pa = a
        .patch(patch_a)
        .ok_or_else(|| Error::Mesh(format!("periodic sliding join: mesh A has no patch '{patch_a}'")))?
        .clone();
    let pb = b
        .patch(patch_b)
        .ok_or_else(|| Error::Mesh(format!("periodic sliding join: mesh B has no patch '{patch_b}'")))?
        .clone();
    if pa.size == 0 || pb.size == 0 {
        return bad("periodic sliding join: empty interface patch");
    }
    if !(tol > 0.0 && tol.is_finite()) {
        return bad("periodic sliding join: the tolerance must be positive and finite");
    }
    for m in [a, b] {
        if m.patches().iter().any(|p| p.name == WRAP_A || p.name == WRAP_B) {
            return bad(format!(
                "periodic sliding join: the patch names '{WRAP_A}' and '{WRAP_B}' are reserved"
            ));
        }
    }
    // M1: the chart and its period
    let (chart, period_s) = match *per {
        Periodicity::Translation { period } => {
            let mut nsum = vec3::ZERO;
            for f in pa.faces() {
                nsum = vec3::add(nsum, a.face_area_vectors()[f]);
            }
            let n = vec3::unit(nsum);
            let tl = vec3::norm(period);
            if !(tl > 0.0 && tl.is_finite()) {
                return bad("periodic sliding join: the period vector must be finite and non-zero");
            }
            if vec3::dot(period, n).abs() > 1e-9 * tl {
                return bad("periodic sliding join: the period vector must lie in the interface plane");
            }
            let e1 = vec3::unit(vec3::sub(period, vec3::scale(n, vec3::dot(period, n))));
            (
                Chart::Plane {
                    o: a.face_centres()[pa.start],
                    e1,
                    e2: vec3::cross(n, e1),
                    n,
                },
                tl,
            )
        }
        Periodicity::Rotation { origin, axis, angle } => {
            let an = vec3::norm(axis);
            if !(an > 1e-300 && an.is_finite()) {
                return bad("periodic sliding join: the axis must be a finite non-zero vector");
            }
            if !(angle > 0.0 && angle < std::f64::consts::PI) {
                return bad(format!(
                    "periodic sliding join: the sector period must lie in (0, pi) (got {angle})"
                ));
            }
            let axis = vec3::scale(axis, 1.0 / an);
            let radial = |x: Vec3| {
                let d = vec3::sub(x, origin);
                vec3::sub(d, vec3::scale(axis, vec3::dot(d, axis)))
            };
            // e1 between the two patches, so that neither crosses the branch cut of atan2
            let mut mid = vec3::ZERO;
            for (m, p) in [(a, &pa), (b, &pb)] {
                for f in p.faces() {
                    mid = vec3::add(mid, vec3::unit(radial(m.face_centres()[f])));
                }
            }
            if vec3::norm(mid) <= 1e-6 * (pa.size + pb.size) as f64 {
                return bad("periodic sliding join: the interface patches have no mean direction about the axis");
            }
            let e1 = vec3::unit(mid);
            let e2 = vec3::cross(axis, e1);
            let unit_cyl = Chart::Cylinder {
                o: origin,
                axis,
                e1,
                e2,
                r: 1.0,
                sign: 1.0,
            };
            let (mut rsum, mut cnt) = (0.0, 0.0);
            for f in pa.faces() {
                for &p in a.face_points(f) {
                    rsum += unit_cyl.coords(a.points()[p]).1 + 1.0;
                    cnt += 1.0;
                }
            }
            let r = rsum / cnt;
            if r <= 0.0 || !r.is_finite() {
                return bad("periodic sliding join: patch A has no positive radius about the axis");
            }
            let f0 = pa.start;
            let side = vec3::dot(a.face_area_vectors()[f0], unit_cyl.normal(a.face_centres()[f0]));
            (
                Chart::Cylinder {
                    o: origin,
                    axis,
                    e1,
                    e2,
                    r,
                    sign: if side < 0.0 { -1.0 } else { 1.0 },
                },
                r * angle,
            )
        }
    };
    let to_plane = |p: Vec3| chart.coords(p);
    // orientation, surface and extent
    for (m, patch, sign) in [(a, &pa, 1.0), (b, &pb, -1.0)] {
        for f in patch.faces() {
            let un = vec3::unit(m.face_area_vectors()[f]);
            if sign * vec3::dot(un, chart.normal(m.face_centres()[f])) < 1.0 - 1e-6 {
                return bad(match chart {
                    Chart::Plane { .. } => format!(
                        "periodic sliding join: interface face {f} is not parallel to the interface plane (planar, opposed patches required)"
                    ),
                    Chart::Cylinder { .. } => format!(
                        "periodic sliding join: interface face {f} is not radial (patches on one cylinder about the axis, facing each other, required)"
                    ),
                });
            }
        }
    }
    let mut ext = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    let mut ext_s = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    let mut max_off: f64 = 0.0;
    for (side, (m, patch)) in [(a, &pa), (b, &pb)].into_iter().enumerate() {
        for f in patch.faces() {
            for &p in m.face_points(f) {
                let (c, off) = to_plane(m.points()[p]);
                max_off = max_off.max(off.abs());
                for k in 0..2 {
                    ext[0][k] = ext[0][k].min(c[k]);
                    ext[1][k] = ext[1][k].max(c[k]);
                }
                ext_s[side][0] = ext_s[side][0].min(c[0]);
                ext_s[side][1] = ext_s[side][1].max(c[0]);
            }
        }
    }
    let size = (ext[1][0] - ext[0][0]).hypot(ext[1][1] - ext[0][1]);
    let d = tol * size;
    if max_off > 1e3 * d {
        return bad(format!(
            "periodic sliding join: the interface patches do not lie on one surface (max distance {max_off:.3e}, merge tolerance {d:.3e})"
        ));
    }
    for (side, name) in [(0, patch_a), (1, patch_b)] {
        let span = ext_s[side][1] - ext_s[side][0];
        if (span - period_s).abs() > 1e3 * d {
            return bad(format!(
                "periodic sliding join: patch '{name}' spans {span:.9e} along the slide, not one period {period_s:.9e} \
                 (sectors or slides of another period are not supported)"
            ));
        }
    }
    // M2: points
    let n_pts_est = (pa.size + pb.size) as f64;
    let mut reg = Registry::new(d, size / n_pts_est.sqrt().max(1.0), None);
    let np_a = a.n_points();
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
    let bbox = |l: &[[f64; 2]]| {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in l {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    };
    let b_box: Vec<([f64; 2], [f64; 2])> = b_faces.iter().map(|(_, l)| bbox(l)).collect();
    let area_a_face: Vec<f64> = pa
        .faces()
        .map(|f| shoelace(&plane_loop(a, f, 0, false, &reg, &remap)))
        .collect();
    let area_b_face: Vec<f64> = b_faces.iter().map(|(_, l)| shoelace(l)).collect();
    if area_a_face.iter().chain(&area_b_face).any(|&x| x <= 0.0) {
        return bad(
            "periodic sliding join: an interface face has non-positive area in the chart (non-convex or inverted face)",
        );
    }
    struct Overlap {
        owner: usize,
        neighbour: usize,
        ids: Vec<usize>,
        /// Stator-side copy (wrap overlaps only), in the same vertex order as `ids`.
        ids_b: Option<Vec<usize>>,
    }
    let n_cells_a = a.n_cells();
    let mut overlaps: Vec<Overlap> = vec![];
    let mut n_dropped = 0;
    let mut tiling_a = vec![0.0; pa.size];
    let mut tiling_b = vec![0.0; pb.size];
    let mut wrap_k: Option<i32> = None;
    let find_or_add = |c: [f64; 2], reg: &mut Registry, points: &mut Vec<Vec3>, remap: &mut Vec<usize>| -> usize {
        match reg.find(c) {
            Some(k) => reg.gid[k],
            None => {
                let g = points.len();
                points.push(chart.point(c));
                remap.push(g);
                reg.insert(c, g);
                g
            }
        }
    };
    for (ia, fa) in pa.faces().enumerate() {
        let sa = plane_loop(a, fa, 0, false, &reg, &remap);
        let (lo, hi) = bbox(&sa);
        for (ib, (fb, cb0)) in b_faces.iter().enumerate() {
            for k in [-1i32, 0, 1] {
                let sh = k as f64 * period_s;
                let (mut blo, mut bhi) = b_box[ib];
                blo[0] += sh;
                bhi[0] += sh;
                if blo[0] > hi[0] || bhi[0] < lo[0] || blo[1] > hi[1] || bhi[1] < lo[1] {
                    continue;
                }
                let cb: Vec<[f64; 2]> = cb0.iter().map(|q| [q[0] + sh, q[1]]).collect();
                let poly = clip_polygon(&sa, &cb);
                if poly.len() < 3 {
                    continue;
                }
                let area = shoelace(&poly);
                if area <= 1e-10 * area_a_face[ia].min(area_b_face[ib]) {
                    n_dropped += 1;
                    continue;
                }
                // rotor-side vertices, consecutive duplicates removed
                let mut verts: Vec<([f64; 2], usize)> = vec![];
                for p in &poly {
                    let g = find_or_add(*p, &mut reg, &mut points, &mut remap);
                    if verts.last().map(|v| v.1) != Some(g) {
                        verts.push((reg.coords[reg.by_gid[&g]], g));
                    }
                }
                if verts.len() > 1 && verts[0].1 == verts[verts.len() - 1].1 {
                    verts.pop();
                }
                if verts.len() < 3 {
                    n_dropped += 1;
                    continue;
                }
                let ids: Vec<usize> = verts.iter().map(|v| v.1).collect();
                let ring: Vec<[f64; 2]> = verts.iter().map(|v| v.0).collect();
                let area = shoelace(&ring);
                tiling_a[ia] += area;
                tiling_b[ib] += area;
                let ids_b = if k == 0 {
                    None
                } else {
                    match wrap_k {
                        None => wrap_k = Some(k),
                        Some(k0) if k0 != k => {
                            return bad(
                                "periodic sliding join: the rotor patch wraps across both seams of the stator (the offset must lie within one period)",
                            )
                        }
                        _ => {}
                    }
                    Some(
                        verts
                            .iter()
                            .map(|v| find_or_add([v.0[0] - sh, v.0[1]], &mut reg, &mut points, &mut remap))
                            .collect::<Vec<usize>>(),
                    )
                };
                overlaps.push(Overlap {
                    owner: a.owner()[fa],
                    neighbour: n_cells_a + b.owner()[*fb],
                    ids,
                    ids_b,
                });
            }
        }
    }
    let shift = wrap_k.unwrap_or(1);
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
            "periodic sliding join: the interface patches do not cover each other over one period: worst face tiling error {max_tiling_error:.3e}"
        ));
    }
    // hanging nodes on every face edge that lies on the interface
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
    let mut wrap_a: PatchFaces = vec![];
    let mut wrap_b: PatchFaces = vec![];
    let mut n_direct = 0;
    for ov in &overlaps {
        match &ov.ids_b {
            None => {
                n_direct += 1;
                internal.push((ov.owner, ov.neighbour, split(ov.ids.clone(), &reg), true));
            }
            Some(ib) => {
                wrap_a.push((ov.owner, split(ov.ids.clone(), &reg)));
                let mut rev = ib.clone();
                rev.reverse();
                wrap_b.push((ov.neighbour, split(rev, &reg)));
            }
        }
    }
    internal.sort_by_key(|x| (x.0, x.1));
    if internal.windows(2).any(|w| (w[0].0, w[0].1) == (w[1].0, w[1].1)) {
        return bad("periodic sliding join: two cells share more than one face (a rotor cell overlaps a stator cell in two pieces)");
    }
    let interface_faces: Vec<usize> = internal
        .iter()
        .enumerate()
        .filter(|(_, x)| x.3)
        .map(|(i, _)| i)
        .collect();
    // M3: boundary patches merged by name, with their periodic partners and transforms
    type Periodic = (Option<String>, Option<PeriodicRotation>);
    let mut pnames: Vec<(String, PatchKind, PatchFaces, Periodic)> = vec![];
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
            let periodic = (p.periodic_with.clone(), p.rotation);
            match pnames.iter_mut().find(|x| x.0 == p.name) {
                Some(x) => {
                    if x.1 != p.kind {
                        return bad(format!(
                            "periodic sliding join: patch '{}' has different kinds in the two meshes",
                            p.name
                        ));
                    }
                    if x.3 != periodic {
                        return bad(format!(
                            "periodic sliding join: patch '{}' has different periodic partners or transforms in the two meshes",
                            p.name
                        ));
                    }
                    x.2.extend(faces);
                }
                None => pnames.push((p.name.clone(), p.kind.clone(), faces, periodic)),
            }
        }
    }
    pnames.push((WRAP_A.to_string(), PatchKind::Cyclic, wrap_a, (None, None)));
    pnames.push((WRAP_B.to_string(), PatchKind::Cyclic, wrap_b, (None, None)));
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
    for (name, kind, fl, _) in &pnames {
        patches.push((name.clone(), kind.clone(), fl.len()));
        for (oc, ids) in fl {
            faces.push(ids.iter().map(|&g| renum(g)).collect());
            owner.push(*oc);
        }
    }
    let mut mesh = Mesh::from_face_lists(new_points, faces, owner, neighbour, patches)?;
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
    for (name, _, _, (partner, rot)) in &pnames {
        if let Some(q) = partner {
            match rot {
                Some(r) => mesh.set_periodic_rotation(name, q, r.origin, r.axis, r.angle)?,
                None => mesh.set_periodic(name, q)?,
            }
        }
    }
    match *per {
        Periodicity::Translation { .. } => {
            mesh.set_periodic(WRAP_A, WRAP_B)?;
            mesh.set_periodic(WRAP_B, WRAP_A)?;
        }
        Periodicity::Rotation { origin, axis, angle } => {
            mesh.set_periodic_rotation(WRAP_A, WRAP_B, origin, axis, -(shift as f64) * angle)?;
        }
    }
    let (sa, sb) = (
        mesh.patch(WRAP_A).expect("wrap patch").start,
        mesh.patch(WRAP_B).expect("wrap patch").start,
    );
    let n_wrap = overlaps.len() - n_direct;
    let wrap_pairs = (0..n_wrap).map(|i| (sa + i, sb + i)).collect();
    let area = [
        pa.faces().map(|f| a.face_area(f)).sum::<f64>(),
        pb.faces().map(|f| b.face_area(f)).sum::<f64>(),
    ];
    Ok((
        mesh,
        PeriodicJoinReport {
            join: JoinReport {
                n_overlap_faces: n_direct,
                n_dropped,
                n_split_edges: n_split,
                area,
                max_tiling_error,
                interface_faces,
                b_cell_offset: n_cells_a,
            },
            wrap_pairs,
            shift,
        },
    ))
}

/// A rotor block that slides against a stator block of the same period (ADR 0629 M4): a planar slider or a rotor sector.
#[derive(Debug, Clone)]
pub struct PeriodicSlidingInterface {
    rotor: Mesh,
    rotor_patch: String,
    stator: Mesh,
    stator_patch: String,
    per: Periodicity,
    tol: f64,
}

impl PeriodicSlidingInterface {
    /// A planar slider: the rotor `rotor` (reference position, offset 0) slides along the period vector `period` against
    /// the stator; both blocks periodic under translation by `period` (their seam pairs set by the caller); interface
    /// patches `rotor_patch` and `stator_patch` each one period long. The pair is joined once at offset 0, so a pair that
    /// cannot be joined fails here.
    pub fn planar(
        rotor: Mesh,
        rotor_patch: &str,
        stator: Mesh,
        stator_patch: &str,
        period: Vec3,
        tol: f64,
    ) -> Result<Self> {
        Self::new(
            rotor,
            rotor_patch,
            stator,
            stator_patch,
            Periodicity::Translation { period },
            tol,
        )
    }

    /// A rotor sector of pitch `angle` turning about the axis through `origin` against a stator sector of the same pitch
    /// (the blocks' rotational seam pairs set by the caller); interface patches on one cylinder about the axis.
    #[allow(clippy::too_many_arguments)]
    pub fn sector(
        rotor: Mesh,
        rotor_patch: &str,
        stator: Mesh,
        stator_patch: &str,
        origin: Vec3,
        axis: Vec3,
        angle: f64,
        tol: f64,
    ) -> Result<Self> {
        Self::new(
            rotor,
            rotor_patch,
            stator,
            stator_patch,
            Periodicity::Rotation { origin, axis, angle },
            tol,
        )
    }

    fn new(
        rotor: Mesh,
        rotor_patch: &str,
        stator: Mesh,
        stator_patch: &str,
        per: Periodicity,
        tol: f64,
    ) -> Result<Self> {
        if rotor.patch(rotor_patch).is_none() {
            return bad(format!(
                "periodic sliding interface: the rotor has no patch '{rotor_patch}'"
            ));
        }
        if stator.patch(stator_patch).is_none() {
            return bad(format!(
                "periodic sliding interface: the stator has no patch '{stator_patch}'"
            ));
        }
        let per = match per {
            Periodicity::Rotation { origin, axis, angle } => {
                let n = vec3::norm(axis);
                if !(n > 1e-300 && n.is_finite()) || origin.iter().any(|x| !x.is_finite()) {
                    return bad(
                        "periodic sliding interface: the axis must be a finite non-zero vector through a finite origin",
                    );
                }
                Periodicity::Rotation {
                    origin,
                    axis: vec3::scale(axis, 1.0 / n),
                    angle,
                }
            }
            t => t,
        };
        let s = PeriodicSlidingInterface {
            rotor,
            rotor_patch: rotor_patch.to_string(),
            stator,
            stator_patch: stator_patch.to_string(),
            per,
            tol,
        };
        s.mesh_at(0.0)?;
        Ok(s)
    }

    /// Number of rotor cells (the first cells of every joined mesh).
    pub fn n_rotor_cells(&self) -> usize {
        self.rotor.n_cells()
    }

    /// The periodicity of the pair.
    pub fn periodicity(&self) -> Periodicity {
        self.per
    }

    /// The period of the motion: the offsets allowed are `0 <= offset < period()`.
    pub fn period(&self) -> f64 {
        self.per.motion_period()
    }

    /// The rotor in its reference position.
    pub fn rotor(&self) -> &Mesh {
        &self.rotor
    }

    /// The stator.
    pub fn stator(&self) -> &Mesh {
        &self.stator
    }

    /// The joined mesh with the rotor moved by `offset` (a length along the period vector, or an angle about the axis,
    /// right-handed) from its reference position, `0 <= offset < period()`.
    pub fn mesh_at(&self, offset: f64) -> Result<PeriodicSlidingMesh> {
        let p = self.period();
        if !(offset.is_finite() && offset >= 0.0 && offset < p) {
            return bad(format!(
                "periodic sliding interface: the offset must lie in [0, {p}) (one period; got {offset})"
            ));
        }
        let rotor = if offset == 0.0 {
            self.rotor.clone()
        } else {
            let mv = self.per.mover(offset);
            moved_mesh(&self.rotor, self.rotor.points().iter().map(|&q| mv(q)).collect())?
        };
        let (mesh, report) = join_non_conformal_periodic(
            &rotor,
            &self.rotor_patch,
            &self.stator,
            &self.stator_patch,
            &self.per,
            self.tol,
        )?;
        let mut interface = vec![false; mesh.n_faces()];
        for &f in &report.join.interface_faces {
            interface[f] = true;
        }
        let mut wrap = vec![false; mesh.n_faces()];
        for &(fa, fb) in &report.wrap_pairs {
            wrap[fa] = true;
            wrap[fb] = true;
        }
        Ok(PeriodicSlidingMesh {
            mesh,
            report,
            offset,
            image: 0,
            n_rotor_cells: self.rotor.n_cells(),
            per: self.per,
            interface,
            wrap,
        })
    }
}

impl PeriodicSlidingInterface {
    /// The image `k` and the reduced offset `r` in `[0, period())` of a turned offset `turned >= 0` (F-704, ADR 0634 R1):
    /// `turned = r + k . period()`, the rotor at `turned` being the image of the rotor at `r` moved by `k` periods.
    pub fn reduce(&self, turned: f64) -> Result<(i64, f64)> {
        let p = self.period();
        if !(turned.is_finite() && turned >= 0.0) {
            return bad(format!(
                "periodic sliding interface: the turned offset must be finite and not negative (got turned = {turned})"
            ));
        }
        let mut k = (turned / p).floor() as i64;
        let mut r = turned - k as f64 * p;
        if r < 0.0 {
            k -= 1;
            r = turned - k as f64 * p;
        } else if r >= p {
            k += 1;
            r = turned - k as f64 * p;
        }
        if !(r >= 0.0 && r < p) {
            return bad(format!(
                "periodic sliding interface: the turned offset {turned} cannot be reduced to one period"
            ));
        }
        Ok((k, r))
    }

    /// The joined mesh with the rotor turned by `turned >= 0` from its reference position, any number of periods (F-704,
    /// ADR 0634 R1): the mesh of [`mesh_at`](Self::mesh_at) at the reduced offset, with [`PeriodicSlidingMesh::image`] the
    /// number of whole periods taken off. A step between two such meshes whose images differ by one is a re-imaging step
    /// (ADR 0634 R2).
    pub fn mesh_turned(&self, turned: f64) -> Result<PeriodicSlidingMesh> {
        let (k, r) = self.reduce(turned)?;
        let mut m = self.mesh_at(r)?;
        m.image = k;
        Ok(m)
    }
}

/// The joined mesh of a [`PeriodicSlidingInterface`] at one offset.
#[derive(Debug, Clone)]
pub struct PeriodicSlidingMesh {
    /// The joined mesh (rotor cells first; the wrap patches last).
    pub mesh: Mesh,
    /// What the join did.
    pub report: PeriodicJoinReport,
    /// Offset of the rotor.
    pub offset: f64,
    /// The image of the rotor (F-704, ADR 0634 R1): the rotor stands `image` periods further on than this mesh shows it
    /// (its turned offset is `offset + image . period`). `0` from [`PeriodicSlidingInterface::mesh_at`].
    pub image: i64,
    n_rotor_cells: usize,
    per: Periodicity,
    interface: Vec<bool>,
    wrap: Vec<bool>,
}

impl PeriodicSlidingMesh {
    /// Number of rotor cells.
    pub fn n_rotor_cells(&self) -> usize {
        self.n_rotor_cells
    }

    /// The periodicity of the pair.
    pub fn periodicity(&self) -> Periodicity {
        self.per
    }

    /// The turned offset `offset + image . period` (ADR 0634 R1).
    pub fn turned(&self) -> f64 {
        self.offset + self.image as f64 * self.per.motion_period()
    }

    /// Whether face `f` is a direct (internal) interface face.
    pub fn is_interface(&self, f: usize) -> bool {
        self.interface[f]
    }

    /// Whether face `f` is a face of a wrap pair (either copy).
    pub fn is_wrap(&self, f: usize) -> bool {
        self.wrap[f]
    }

    /// Number of boundary faces before the wrap patches (the original patches, the same at every offset).
    pub fn n_fixed_boundary_faces(&self) -> usize {
        self.mesh.patch(WRAP_A).expect("wrap patch").start - self.mesh.n_internal_faces()
    }

    /// Whether face `f` moves with the rotor: neither an interface nor a wrap face, its owner a rotor cell and (internal
    /// faces) its neighbour too.
    pub fn is_rotor_face(&self, f: usize) -> bool {
        if self.interface[f] || self.wrap[f] {
            return false;
        }
        let nr = self.n_rotor_cells;
        let own = self.mesh.owner()[f];
        if f < self.mesh.n_internal_faces() {
            own < nr && self.mesh.neighbour()[f] < nr
        } else {
            own < nr
        }
    }

    /// The rotation that maps vectors of the stator side of a wrap pair onto the rotor side (rotation by `shift` pitches);
    /// `None` for a translation.
    pub fn wrap_rotation(&self) -> Option<[[f64; 3]; 3]> {
        match self.per {
            Periodicity::Translation { .. } => None,
            Periodicity::Rotation { axis, angle, .. } => Some(rotation_matrix(axis, self.report.shift as f64 * angle)),
        }
    }

    /// For every face of this mesh, the face of `old` it continues (ADR 0629 M4): an internal face off the interface is the
    /// old face with the same owner and neighbour, a boundary face of an original patch the old face at the same position
    /// after the internal faces (same owner); interface and wrap faces have none. Fails when the meshes are not of one pair
    /// or a face has no partner.
    pub fn face_map_from(&self, old: &PeriodicSlidingMesh) -> Result<Vec<Option<usize>>> {
        let (m1, m0) = (&self.mesh, &old.mesh);
        if m0.n_cells() != m1.n_cells()
            || old.n_rotor_cells != self.n_rotor_cells
            || old.n_fixed_boundary_faces() != self.n_fixed_boundary_faces()
            || old.per != self.per
        {
            return bad(
                "periodic sliding interface: the two meshes are not of one pair (cell or boundary face counts differ)",
            );
        }
        let (ni0, ni1) = (m0.n_internal_faces(), m1.n_internal_faces());
        let (own0, nei0) = (m0.owner(), m0.neighbour());
        let (own1, nei1) = (m1.owner(), m1.neighbour());
        let mut key: HashMap<(usize, usize), usize> = HashMap::with_capacity(ni0);
        for f in 0..ni0 {
            if !old.interface[f] {
                key.insert((own0[f], nei0[f]), f);
            }
        }
        let mut used = vec![false; m0.n_faces()];
        let mut map = vec![None; m1.n_faces()];
        for f in 0..ni1 {
            if self.interface[f] {
                continue;
            }
            let g = match key.get(&(own1[f], nei1[f])) {
                Some(&g) => g,
                None => {
                    return bad(format!(
                        "periodic sliding interface: face {f} between cells {} and {} has no partner in the old mesh",
                        own1[f], nei1[f]
                    ))
                }
            };
            if used[g] {
                return bad(format!("periodic sliding interface: old face {g} continues two faces"));
            }
            used[g] = true;
            map[f] = Some(g);
        }
        for (f, slot) in map.iter_mut().enumerate().skip(ni1).take(self.n_fixed_boundary_faces()) {
            let g = f - ni1 + ni0;
            if own0[g] != own1[f] {
                return bad(format!(
                    "periodic sliding interface: boundary face {f} (owner {}) does not continue old face {g} (owner {})",
                    own1[f], own0[g]
                ));
            }
            *slot = Some(g);
        }
        Ok(map)
    }

    /// Volume swept by every rotor face from the offset `old_offset` to this mesh's offset (ADR 0629 M5): ADR 0033's swept
    /// volume between the face's point loop of this mesh moved back by `offset - old_offset` and the same loop here
    /// (positive out of the owner). Zero on stator, interface and wrap faces.
    pub fn rotor_swept_volumes(&self, old_offset: f64) -> Result<Vec<f64>> {
        if !old_offset.is_finite() {
            return bad(format!(
                "periodic sliding interface: the old offset must be finite (got {old_offset})"
            ));
        }
        let mv = self.per.mover(old_offset - self.offset);
        let back: Vec<Vec3> = self.mesh.points().iter().map(|&p| mv(p)).collect();
        let old = moved_mesh(&self.mesh, back)?;
        let w = swept_volumes_about(&old, self.mesh.points(), None);
        Ok(w.into_iter()
            .enumerate()
            .map(|(f, x)| if self.is_rotor_face(f) { x } else { 0.0 })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{annulus_3d_sector, annulus_sector, rectilinear_2d, Grading};
    use std::f64::consts::{FRAC_PI_3, PI};

    const L: f64 = 0.1;
    const Z: Vec3 = [0.0, 0.0, 1.0];

    fn uniform(n: usize, a: f64, b: f64) -> Vec<f64> {
        (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
    }

    fn periodic_x(mut m: Mesh) -> Mesh {
        m.set_periodic("xmin", "xmax").unwrap();
        m.set_periodic("xmax", "xmin").unwrap();
        m
    }

    /// ADR 0629 PP.
    fn pair_pp() -> PeriodicSlidingInterface {
        let stator = periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap());
        let slider = periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap());
        PeriodicSlidingInterface::planar(slider, "ymin", stator, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap()
    }

    /// ADR 0629 SS.
    fn pair_ss() -> PeriodicSlidingInterface {
        let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
        let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
        PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
    }

    /// ADR 0629 S3.
    fn pair_s3() -> PeriodicSlidingInterface {
        let a = annulus_3d_sector(12, 4, 2, 1.0, 1.5, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
        let b = annulus_3d_sector(10, 4, 3, 1.5, 2.0, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
        PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
    }

    fn worst_closure(m: &Mesh) -> f64 {
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
        worst
    }

    fn k1_on(s: &PeriodicSlidingInterface, name: &str) {
        let meshes: Vec<PeriodicSlidingMesh> = (0..5).map(|k| s.mesh_at(0.0123 * k as f64).unwrap()).collect();
        for w in meshes.windows(2) {
            let (old, new) = (&w[0], &w[1]);
            let map = new.face_map_from(old).unwrap();
            let (m0, m1) = (&old.mesh, &new.mesh);
            let mut used = vec![0usize; m0.n_faces()];
            for (f, g) in map.iter().enumerate() {
                match g {
                    Some(g) => {
                        assert!(
                            !new.is_interface(f) && !new.is_wrap(f),
                            "{name}: interface or wrap face {f} mapped"
                        );
                        assert!(
                            !old.is_interface(*g) && !old.is_wrap(*g),
                            "{name}: face {f} mapped to {g}"
                        );
                        assert_eq!(m1.owner()[f], m0.owner()[*g], "{name}: owner of face {f}");
                        if f < m1.n_internal_faces() {
                            assert!(*g < m0.n_internal_faces());
                            assert_eq!(m1.neighbour()[f], m0.neighbour()[*g], "{name}: neighbour of face {f}");
                        } else {
                            assert!(*g >= m0.n_internal_faces());
                            assert_eq!(
                                m1.patch_of_face(f).unwrap().name,
                                m0.patch_of_face(*g).unwrap().name,
                                "{name}: patch of face {f}"
                            );
                        }
                        used[*g] += 1;
                    }
                    None => assert!(new.is_interface(f) || new.is_wrap(f), "{name}: face {f} unmapped"),
                }
            }
            assert!(used.iter().all(|&u| u <= 1), "{name}: an old face used twice");
            println!(
                "K1 {name}: offset {:.4} -> {:.4}: {} direct, {} wrap pairs; exact",
                old.offset,
                new.offset,
                new.report.join.interface_faces.len(),
                new.report.wrap_pairs.len()
            );
        }
    }

    /// K1 (ADR 0629): the face map between consecutive offsets.
    #[test]
    fn k1_face_map_between_consecutive_offsets() {
        k1_on(&pair_pp(), "PP");
        k1_on(&pair_ss(), "SS");
    }

    /// K2 (ADR 0629): the rotor's swept volumes.
    #[test]
    fn k2_rotor_swept_volumes() {
        // (i) planar: W = S . d
        let pp = pair_pp();
        let m = pp.mesh_at(0.0173).unwrap();
        let w = m.rotor_swept_volumes(0.0123).unwrap();
        let (mut e1, mut n1) = (0.0f64, 0);
        let mut e3 = 0.0f64;
        for f in 0..m.mesh.n_faces() {
            let s = m.mesh.face_area_vectors()[f];
            if m.is_rotor_face(f) {
                let ex = s[0] * 0.005;
                e1 = e1.max((w[f] - ex).abs() / (vec3::norm(s) * 0.005));
                n1 += 1;
            } else {
                e3 = e3.max(w[f].abs());
            }
        }
        // (ii) sector: radial rotor faces against the straight-path sector
        let ss = pair_ss();
        let m = ss.mesh_at(0.0173).unwrap();
        let w = m.rotor_swept_volumes(0.0123).unwrap();
        let (mut e2, mut n2) = (0.0f64, 0);
        for f in 0..m.mesh.n_faces() {
            if !m.is_rotor_face(f) {
                e3 = e3.max(w[f].abs());
                continue;
            }
            let s = m.mesh.face_area_vectors()[f];
            if s[2].abs() > 1e-9 * vec3::norm(s) {
                continue;
            }
            let c = m.mesh.face_centres()[f];
            let radial = vec3::unit([c[0], c[1], 0.0]);
            if vec3::dot(vec3::unit(s), radial).abs() > 1e-9 {
                continue; // not a radial face
            }
            let rs: Vec<f64> = m
                .mesh
                .face_points(f)
                .iter()
                .map(|&p| m.mesh.points()[p][0].hypot(m.mesh.points()[p][1]))
                .collect();
            let (r1, r2) = (
                rs.iter().cloned().fold(f64::INFINITY, f64::min),
                rs.iter().cloned().fold(0.0, f64::max),
            );
            let ex = L * (r2 * r2 - r1 * r1) * 0.005f64.sin() / 2.0;
            e2 = e2.max((w[f].abs() - ex).abs() / ex);
            n2 += 1;
        }
        println!(
            "K2: (i) planar {e1:.2e} over {n1} slider faces (1e-12); (ii) sector radial {e2:.2e} over {n2} faces (1e-12); \
             (iii) largest |W| off the rotor {e3:.2e} (exact zero)"
        );
        assert!(n1 > 0 && n2 > 0);
        assert!(e1 <= 1e-12, "K2 (i) {e1:.3e}");
        assert!(e2 <= 1e-12, "K2 (ii) {e2:.3e}");
        assert!(e3 == 0.0, "K2 (iii) {e3:.3e}");
    }

    /// K3 and K5 (ADR 0629): the total volume against the closed form, tiling and closure, at each offset.
    #[test]
    fn k3_k5_volume_tiling_and_closure() {
        let cases: [(&str, PeriodicSlidingInterface, [f64; 4], f64); 3] = [
            ("PP", pair_pp(), [0.0, 0.0123, 0.3, 0.9], 0.1),
            (
                "SS",
                pair_ss(),
                [0.0, 0.0123, 0.3, 1.0],
                L * (10.0 * 4.0 * (PI / 60.0).sin() - 12.0 * (PI / 72.0).sin()),
            ),
            (
                "S3",
                pair_s3(),
                [0.0, 0.0123, 0.3, 1.0],
                0.4 * (5.0 * 4.0 * (PI / 30.0).sin() - 6.0 * (PI / 36.0).sin()),
            ),
        ];
        let (mut kv, mut kt, mut kc) = (0.0f64, 0.0f64, 0.0f64);
        for (name, s, offs, vex) in &cases {
            for &o in offs {
                let m = s.mesh_at(o).unwrap();
                let ve = (m.mesh.total_volume() - vex).abs() / vex;
                let cl = worst_closure(&m.mesh);
                println!(
                    "K3/K5 {name} offset {o}: volume {ve:.2e} (1e-12), tiling {:.2e} (1e-12), closure {cl:.2e} (1e-13); \
                     {} direct, {} wrap, {} dropped",
                    m.report.join.max_tiling_error,
                    m.report.join.interface_faces.len(),
                    m.report.wrap_pairs.len(),
                    m.report.join.n_dropped
                );
                kv = kv.max(ve);
                kt = kt.max(m.report.join.max_tiling_error);
                kc = kc.max(cl);
            }
        }
        assert!(kv <= 1e-12, "K3 {kv:.3e}");
        assert!(kt <= 1e-12, "K5 tiling {kt:.3e}");
        assert!(kc <= 1e-13, "K5 closure {kc:.3e}");
    }

    /// K6 (ADR 0629): refusals.
    #[test]
    fn k6_refusals() {
        let pp = pair_pp();
        let ss = pair_ss();
        for (s, o) in [
            (&pp, -0.1),
            (&pp, 1.0),
            (&pp, f64::NAN),
            (&ss, -0.1),
            (&ss, FRAC_PI_3),
            (&ss, f64::NAN),
        ] {
            let e = format!("{:?}", s.mesh_at(o).unwrap_err());
            println!("K6 offset {o}: {e}");
            assert!(e.contains("offset"), "{e}");
        }
        let stator = periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap());
        let slider = periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap());
        let e = format!(
            "{:?}",
            PeriodicSlidingInterface::planar(slider, "ymin", stator, "ymax", [1.0, 0.1, 0.0], 1e-8).unwrap_err()
        );
        println!("K6 period out of the plane: {e}");
        assert!(e.contains("plane"), "{e}");
        let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
        let b = annulus_sector(20, 8, 1.5, 2.0, PI / 4.0, Grading::Uniform, L).unwrap();
        let e = format!(
            "{:?}",
            PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap_err()
        );
        println!("K6 stator of another pitch: {e}");
        assert!(e.contains("period"), "{e}");
    }

    // ---------------------------------------------------------------------------------------------------------------
    // F-704 (ADR 0634): turned offsets and the re-imaging step
    // ---------------------------------------------------------------------------------------------------------------

    type MeshBits = (Vec<u64>, Vec<usize>, Vec<usize>, Vec<Vec<usize>>, String);

    fn mesh_bits(m: &Mesh) -> MeshBits {
        (
            m.points().iter().flatten().map(|x| x.to_bits()).collect(),
            m.owner().to_vec(),
            m.neighbour().to_vec(),
            (0..m.n_faces()).map(|f| m.face_points(f).to_vec()).collect(),
            format!("{:?}", m.patches()),
        )
    }

    /// Whether rotor cell `c` has a face on the interface (a direct or a wrap face).
    fn on_interface(m: &PeriodicSlidingMesh, c: usize) -> bool {
        m.mesh.cell_faces(c).iter().any(|&f| m.is_interface(f) || m.is_wrap(f))
    }

    /// K1 and K2 (with K2c of amendment 1) of ADR 0634.
    #[test]
    fn f704_k1_k2_turned_offsets() {
        let cases: [(&str, PeriodicSlidingInterface, [f64; 4]); 2] = [
            ("PP", pair_pp(), [0.0, 0.0123, 0.3, 0.9]),
            ("SS", pair_ss(), [0.0, 0.0123, 0.3, 1.0]),
        ];
        for (name, s, offs) in &cases {
            let p = s.period();
            let reference = s.rotor().cell_centres().to_vec();
            let nr = s.n_rotor_cells();
            for &t in offs {
                let a = s.mesh_at(t).unwrap();
                let b = s.mesh_turned(t).unwrap();
                let k1 = b.image == 0 && b.offset.to_bits() == t.to_bits() && mesh_bits(&a.mesh) == mesh_bits(&b.mesh);
                println!("K1 {name} t {t}: image {}, bit for bit {k1}", b.image);
                assert!(k1, "K1 {name} t {t}");
                for k in 1..=3i64 {
                    let turned = t + k as f64 * p;
                    let m = s.mesh_turned(turned).unwrap();
                    let de = (m.offset - t).abs();
                    let at = s.mesh_at(m.offset).unwrap();
                    let same = (0..nr).all(|c| {
                        m.mesh.cell_centres()[c]
                            .iter()
                            .zip(&at.mesh.cell_centres()[c])
                            .all(|(x, y)| x.to_bits() == y.to_bits())
                    });
                    let (mut ec, mut n_off) = (0.0f64, 0);
                    for c in 0..nr {
                        if on_interface(&m, c) {
                            continue;
                        }
                        let moved = s.periodicity().apply(turned, reference[c]);
                        let back = s.periodicity().apply(-(k as f64) * p, moved);
                        ec = ec.max(vec3::norm(vec3::sub(m.mesh.cell_centres()[c], back)));
                        n_off += 1;
                    }
                    println!(
                        "K2 {name} t {t} k {k}: image {} ({k}), |offset - t| {de:.2e} ({:.1e}); K2c centres off the \
                         interface {ec:.2e} over {n_off} cells (1e-12); every rotor centre as mesh_at(offset): {same}",
                        m.image,
                        1e-14 * k as f64
                    );
                    assert!(m.image == k, "K2 image");
                    assert!(de <= 1e-14 * k as f64, "K2 offset {de:.3e}");
                    assert!(n_off > 0 && ec <= 1e-12, "K2c {ec:.3e}");
                    assert!(same, "K2c bits");
                }
            }
        }
    }

    /// K3 of ADR 0634: the re-imaging step on the mesh.
    #[test]
    fn f704_k3_re_imaging_step_face_map_and_swept_volumes() {
        for (name, s, t0, t1) in [
            ("PP", pair_pp(), 0.9975, 1.0025),
            ("SS", pair_ss(), FRAC_PI_3 - 0.0025, FRAC_PI_3 + 0.0025),
        ] {
            let p = s.period();
            let (old, new) = (s.mesh_turned(t0).unwrap(), s.mesh_turned(t1).unwrap());
            assert_eq!((old.image, new.image), (0, 1), "K3 {name}: images");
            // (i) the face map
            let map = new.face_map_from(&old).unwrap();
            let (m0, m1) = (&old.mesh, &new.mesh);
            let mut used = vec![0usize; m0.n_faces()];
            let mut ok = true;
            for (f, g) in map.iter().enumerate() {
                match g {
                    Some(g) => {
                        ok &= !new.is_interface(f) && !new.is_wrap(f) && !old.is_interface(*g) && !old.is_wrap(*g);
                        ok &= m1.owner()[f] == m0.owner()[*g];
                        if f < m1.n_internal_faces() {
                            ok &= *g < m0.n_internal_faces() && m1.neighbour()[f] == m0.neighbour()[*g];
                        } else {
                            ok &= *g >= m0.n_internal_faces()
                                && m1.patch_of_face(f).unwrap().name == m0.patch_of_face(*g).unwrap().name;
                        }
                        used[*g] += 1;
                    }
                    None => ok &= new.is_interface(f) || new.is_wrap(f),
                }
            }
            ok &= used.iter().all(|&u| u <= 1);
            // (ii) the swept volumes of the motion 0.005 from old.offset - P
            let w = new.rotor_swept_volumes(old.offset - p).unwrap();
            let (mut e, mut n, mut e0) = (0.0f64, 0, 0.0f64);
            for f in 0..m1.n_faces() {
                if !new.is_rotor_face(f) {
                    e0 = e0.max(w[f].abs());
                    continue;
                }
                let sv = m1.face_area_vectors()[f];
                if name == "PP" {
                    e = e.max((w[f] - sv[0] * 0.005).abs() / (vec3::norm(sv) * 0.005));
                    n += 1;
                    continue;
                }
                if sv[2].abs() > 1e-9 * vec3::norm(sv) {
                    continue;
                }
                let c = m1.face_centres()[f];
                if vec3::dot(vec3::unit(sv), vec3::unit([c[0], c[1], 0.0])).abs() > 1e-9 {
                    continue;
                }
                let rs: Vec<f64> = m1
                    .face_points(f)
                    .iter()
                    .map(|&q| m1.points()[q][0].hypot(m1.points()[q][1]))
                    .collect();
                let (r1, r2) = (
                    rs.iter().cloned().fold(f64::INFINITY, f64::min),
                    rs.iter().cloned().fold(0.0, f64::max),
                );
                let ex = L * (r2 * r2 - r1 * r1) * 0.005f64.sin() / 2.0;
                e = e.max((w[f].abs() - ex).abs() / ex);
                n += 1;
            }
            println!(
                "K3 {name}: offsets {:.6} (image {}) -> {:.6} (image {}); (i) face map exact {ok}; (ii) swept volumes \
                 {e:.2e} over {n} faces (1e-12), largest off the rotor {e0:.2e} (exact zero)",
                old.offset, old.image, new.offset, new.image
            );
            assert!(ok, "K3 (i) {name}");
            assert!(n > 0 && e <= 1e-12, "K3 (ii) {name} {e:.3e}");
            assert!(e0 == 0.0, "K3 (ii) zero {name} {e0:.3e}");
        }
    }

    /// K4 of ADR 0634: refusals of a turned offset.
    #[test]
    fn f704_k4_refusals() {
        let (pp, ss) = (pair_pp(), pair_ss());
        for s in [&pp, &ss] {
            for t in [-0.1, f64::NAN, f64::INFINITY] {
                let e1 = format!("{:?}", s.reduce(t).unwrap_err());
                let e2 = format!("{:?}", s.mesh_turned(t).unwrap_err());
                println!("K4 turned {t}: {e2}");
                assert!(e1.contains("turned") && e2.contains("turned"), "{e1} / {e2}");
            }
        }
    }
}
