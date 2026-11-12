//! Overset (chimera) meshes, F-134 (ADR 0219): a component mesh that moves rigidly over a static background mesh.
//!
//! The two meshes are joined into one [`Composite`] mesh without any face between them (ADR 0219 M1). At every pose of the
//! component, [`Overset::connectivity`] gives each cell a kind (M2): *calculated* (its equations are solved), *fringe* (its
//! values are interpolated from the other part) or *hole* (excluded). The component cells next to the named outer patches
//! are fringe; the background cells where the user's hole function is true are holes, and the background cells next to a
//! hole are fringe. Every fringe cell gets a stencil (M3, M4): the cell of the other part that contains its centre (found
//! by [`CellLocator`]) and that cell's calculated face neighbours, with weights that reproduce linear fields exactly
//! ([`linear_weights`]). The flow solver is `rcfd_fv::overset`.
//!
//! A donor interpolation that reproduces quadratic fields (F-336, ADR 0251) is an opt-in,
//! [`Overset::set_donor_interpolation`] with [`DonorInterpolation::Quadratic`]: the stencil of a fringe cell is then the
//! donor cell and the calculated cells that share a point with it, with the weights of the weighted least-squares
//! quadratic ([`quadratic_weights`]). A fringe cell whose neighbourhood cannot carry a quadratic keeps the linear stencil
//! and is listed in [`Connectivity::fallbacks`]. Without the opt-in nothing changes.
//!
//! Cells with non-planar faces (F-352, ADR 0253): the plane of M3 for each face leaves slivers between the cells along
//! their edges. A cell with a face whose points leave the face's plane by more than the locator's tolerance is tested as
//! the polyhedron of the triangles about its face centres ([`triangulated_margin`]); the cells of a face share those
//! triangles, so they tile the mesh. A cell whose faces are planar is tested as before, bit for bit.
//!
//! Non-convex cells with planar faces (F-356, ADR 0260): the planes of M3 enclose only the kernel of a non-convex cell. A
//! flat cell with a point outside one of its own face planes by more than the locator's tolerance is tested by the
//! triangles of its faces too; a face whose fan about its centre folds (leaves the face) is triangulated by ear clipping
//! ([`face_ear_triangles`]), the same from both sides. A convex cell with planar faces has no such point and is tested
//! as before, bit for bit.

use crate::mesh::{Mesh, PatchKind};
use crate::motion::{mat_vec, rotation_matrix, Pose};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::ops::Range;
use std::sync::Arc;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// The point `p` with `pose.apply(p) == x` (the inverse of a rigid pose).
pub fn pose_inverse_apply(pose: &Pose, x: Vec3) -> Vec3 {
    let r = rotation_matrix(pose.axis, pose.angle);
    let rt = [0, 1, 2].map(|i| [0, 1, 2].map(|j| r[j][i]));
    let q = vec3::sub(vec3::sub(x, pose.translation), pose.origin);
    vec3::add(pose.origin, mat_vec(&rt, q))
}

/// A background mesh and a component mesh joined into one mesh (ADR 0219 M1): the points, cells and internal faces of the
/// background, then those of the component, then the background patches, then the component patches with a prefix on
/// their names. No face joins the two parts. `mesh` has the component at its reference position.
#[derive(Debug, Clone)]
pub struct Composite {
    /// The composite mesh (component at its reference points).
    pub mesh: Mesh,
    /// Number of background cells (the background cells are `0..n_background_cells`).
    pub n_background_cells: usize,
    /// Number of background points (the background points are `0..n_background_points`).
    pub n_background_points: usize,
    /// Names of the background patches in the composite.
    pub background_patches: Vec<String>,
    /// Names of the component patches in the composite (with the prefix).
    pub component_patches: Vec<String>,
}

/// Join `background` and `component` into a [`Composite`]. The component patches are renamed `prefix + name`. Fails for a
/// duplicated patch name (named), a periodic patch, and an invalid result.
pub fn compose(background: &Mesh, component: &Mesh, prefix: &str) -> Result<Composite> {
    for m in [background, component] {
        if let Some(p) = m
            .patches()
            .iter()
            .find(|p| p.kind == PatchKind::Cyclic || p.periodic_with.is_some())
        {
            return err(format!("overset compose: periodic patch '{}' is not supported", p.name));
        }
    }
    let bg_names: Vec<String> = background.patches().iter().map(|p| p.name.clone()).collect();
    let c_names: Vec<String> = component
        .patches()
        .iter()
        .map(|p| format!("{prefix}{}", p.name))
        .collect();
    for n in &c_names {
        if bg_names.contains(n) {
            return err(format!(
                "overset compose: duplicate patch name '{n}' (give the component patches a prefix)"
            ));
        }
    }
    let (nb_c, nb_p) = (background.n_cells(), background.n_points());
    let mut points: Vec<Vec3> = background.points().to_vec();
    points.extend_from_slice(component.points());
    let mut faces: Vec<Vec<usize>> = Vec::with_capacity(background.n_faces() + component.n_faces());
    let mut owner = Vec::with_capacity(background.n_faces() + component.n_faces());
    let mut neighbour = Vec::with_capacity(background.n_internal_faces() + component.n_internal_faces());
    for f in 0..background.n_internal_faces() {
        faces.push(background.face_points(f).to_vec());
        owner.push(background.owner()[f]);
        neighbour.push(background.neighbour()[f]);
    }
    for f in 0..component.n_internal_faces() {
        faces.push(component.face_points(f).iter().map(|&q| q + nb_p).collect());
        owner.push(component.owner()[f] + nb_c);
        neighbour.push(component.neighbour()[f] + nb_c);
    }
    let mut patches = Vec::new();
    for p in background.patches() {
        for f in p.faces() {
            faces.push(background.face_points(f).to_vec());
            owner.push(background.owner()[f]);
        }
        patches.push((p.name.clone(), p.kind.clone(), p.size));
    }
    for (p, name) in component.patches().iter().zip(&c_names) {
        for f in p.faces() {
            faces.push(component.face_points(f).iter().map(|&q| q + nb_p).collect());
            owner.push(component.owner()[f] + nb_c);
        }
        patches.push((name.clone(), p.kind.clone(), p.size));
    }
    let mesh = Mesh::from_face_lists(points, faces, owner, neighbour, patches)?;
    Ok(Composite {
        mesh,
        n_background_cells: nb_c,
        n_background_points: nb_p,
        background_patches: bg_names,
        component_patches: c_names,
    })
}

impl Composite {
    /// The composite points with the component at `pose` (from its reference points; the background does not move).
    pub fn points_at(&self, pose: &Pose) -> Vec<Vec3> {
        let pts = self.mesh.points();
        let nb = self.n_background_points;
        let mut out = pts[..nb].to_vec();
        out.extend(pts[nb..].iter().map(|&p| pose.apply(p)));
        out
    }

    /// The background cell range.
    pub fn background_cells(&self) -> Range<usize> {
        0..self.n_background_cells
    }

    /// The component cell range.
    pub fn component_cells(&self) -> Range<usize> {
        self.n_background_cells..self.mesh.n_cells()
    }
}

/// Signed distance of `x` inside cell `c` of `mesh`: the smallest distance to the planes of its faces, positive inside
/// (ADR 0219 M3; exact for convex cells with planar faces).
pub fn containment_margin(mesh: &Mesh, c: usize, x: Vec3) -> f64 {
    let (own, sf, fc) = (mesh.owner(), mesh.face_area_vectors(), mesh.face_centres());
    let mut m = f64::INFINITY;
    for &f in mesh.cell_faces(c) {
        let s = if own[f] == c { sf[f] } else { vec3::scale(sf[f], -1.0) };
        let a = vec3::norm(s);
        if a < 1e-300 {
            continue;
        }
        m = m.min(-vec3::dot(s, vec3::sub(x, fc[f])) / a);
    }
    m
}

/// The warp of face `f` (ADR 0253): the largest distance of its points from the plane through the face centre normal to
/// the area vector. Zero for a face without area.
pub fn face_warp(mesh: &Mesh, f: usize) -> f64 {
    let s = mesh.face_area_vectors()[f];
    let a = vec3::norm(s);
    if a < 1e-300 {
        return 0.0;
    }
    let fc = mesh.face_centres()[f];
    mesh.face_points(f)
        .iter()
        .map(|&q| (vec3::dot(s, vec3::sub(mesh.points()[q], fc)) / a).abs())
        .fold(0.0, f64::max)
}

/// Distance of `x` from the segment `(p, q)`; exactly zero when `x` is `p` or `q`.
fn segment_distance(x: Vec3, p: Vec3, q: Vec3) -> f64 {
    let e = vec3::sub(q, p);
    let ee = vec3::dot(e, e);
    let t = if ee > 0.0 {
        vec3::dot(vec3::sub(x, p), e) / ee
    } else {
        0.0
    };
    let nearest = if t <= 0.0 {
        p
    } else if t >= 1.0 {
        q
    } else {
        vec3::add(p, vec3::scale(e, t))
    };
    vec3::norm(vec3::sub(x, nearest))
}

/// Distance of `x` from the triangle `(a, b, c)` (ADR 0253 G2): the smallest of the distances from the three edges and,
/// when the foot of the perpendicular on the plane lies in the triangle, of the distance from the plane.
fn triangle_distance(x: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let mut d = segment_distance(x, a, b)
        .min(segment_distance(x, b, c))
        .min(segment_distance(x, c, a));
    let n = vec3::cross(vec3::sub(b, a), vec3::sub(c, a));
    let nn = vec3::norm(n);
    if nn > 0.0 {
        let side = |p: Vec3, q: Vec3| vec3::dot(vec3::cross(vec3::sub(q, p), vec3::sub(x, p)), n);
        if side(a, b) >= 0.0 && side(b, c) >= 0.0 && side(c, a) >= 0.0 {
            d = d.min((vec3::dot(vec3::sub(x, a), n) / nn).abs());
        }
    }
    d
}

/// Signed solid angle of the triangle `(a, b, c)` seen from `x` (ADR 0253 G2), in `(-2 pi, 2 pi]`; zero when `x` is one
/// of its points.
fn triangle_solid_angle(x: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (a, b, c) = (vec3::sub(a, x), vec3::sub(b, x), vec3::sub(c, x));
    let (la, lb, lc) = (vec3::norm(a), vec3::norm(b), vec3::norm(c));
    let num = vec3::dot(a, vec3::cross(b, c));
    let den = la * lb * lc + vec3::dot(a, b) * lc + vec3::dot(b, c) * la + vec3::dot(c, a) * lb;
    2.0 * num.atan2(den)
}

/// Signed distance of `x` from the boundary of the triangulated cell `c` of `mesh`, positive inside (ADR 0253 G1, G2).
/// The cell is the polyhedron bounded by the triangles `(fc_f, p_i, p_{i+1})` of each of its faces (`fc_f` the face
/// centre, `p_i` the points of the face); the two cells of a face share those triangles, so the cells of a mesh tile it
/// whether its faces are planar or not. The distance is the smallest distance from the triangles; `x` is inside when the
/// solid angles of the triangles sum to more than `2 pi` (the winding number is above one half).
pub fn triangulated_margin(mesh: &Mesh, c: usize, x: Vec3) -> f64 {
    let (own, fc, pts) = (mesh.owner(), mesh.face_centres(), mesh.points());
    let (mut d, mut omega) = (f64::INFINITY, 0.0);
    for &f in mesh.cell_faces(c) {
        let ids = mesh.face_points(f);
        let sign = if own[f] == c { 1.0 } else { -1.0 };
        for i in 0..ids.len() {
            let (p, q) = (pts[ids[i]], pts[ids[(i + 1) % ids.len()]]);
            d = d.min(triangle_distance(x, fc[f], p, q));
            omega += sign * triangle_solid_angle(x, fc[f], p, q);
        }
    }
    if omega > 2.0 * std::f64::consts::PI {
        d
    } else {
        -d
    }
}

/// Whether the fan of triangles `(fc, p_i, p_{i+1})` of face `f` about its centre folds (ADR 0260 N2): some triangle has
/// a non-positive projection on the area vector. A fan that does not fold tiles a planar face exactly; one that folds
/// covers points outside the face. False for a face without area.
pub fn face_fan_folds(mesh: &Mesh, f: usize) -> bool {
    let s = mesh.face_area_vectors()[f];
    if vec3::norm(s) < 1e-300 {
        return false;
    }
    let (fc, pts, ids) = (mesh.face_centres()[f], mesh.points(), mesh.face_points(f));
    (0..ids.len()).any(|i| {
        let (p, q) = (pts[ids[i]], pts[ids[(i + 1) % ids.len()]]);
        vec3::dot(vec3::cross(vec3::sub(p, fc), vec3::sub(q, fc)), s) <= 0.0
    })
}

/// The triangles of face `f` by ear clipping of its point loop projected on the plane normal to its area vector (ADR 0260
/// N3), as point numbers in the stored orientation: the first point (from the start of the remaining loop) whose
/// triangle with its two neighbours turns positively (above `1e-12` of the face area) and contains no other point of the
/// loop is cut off, until three points remain; a degenerate remainder is dropped. For a simple polygon the triangles
/// tile it. `None` when no ear is found (a loop whose projection is not a simple polygon) or for a face without area.
pub fn face_ear_triangles(mesh: &Mesh, f: usize) -> Option<Vec<[usize; 3]>> {
    let s = mesh.face_area_vectors()[f];
    let ss = vec3::dot(s, s);
    if ss < 1e-300 {
        return None;
    }
    let pts = mesh.points();
    let o =
        |a: usize, b: usize, c: usize| vec3::dot(vec3::cross(vec3::sub(pts[b], pts[a]), vec3::sub(pts[c], pts[a])), s);
    let eps = 1e-12 * ss;
    let mut v: Vec<usize> = mesh.face_points(f).to_vec();
    let mut out = Vec::with_capacity(v.len().saturating_sub(2));
    while v.len() > 3 {
        let n = v.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (v[(i + n - 1) % n], v[i], v[(i + 1) % n]);
            o(a, b, c) > eps
                && !v.iter().any(|&q| {
                    q != a && q != b && q != c && o(a, b, q) >= -eps && o(b, c, q) >= -eps && o(c, a, q) >= -eps
                })
        })?;
        out.push([v[(ear + n - 1) % n], v[ear], v[(ear + 1) % n]]);
        v.remove(ear);
    }
    if o(v[0], v[1], v[2]) > eps {
        out.push([v[0], v[1], v[2]]);
    }
    Some(out)
}

/// Whether a cell has a point on the outer side of one of its own face planes by more than `tol` (ADR 0260 N1): exactly
/// the non-convex cells, for planar faces and `tol` above the round-off.
fn cell_is_nonconvex(mesh: &Mesh, c: usize, tol: f64) -> bool {
    let (own, sf, fc, pts) = (
        mesh.owner(),
        mesh.face_area_vectors(),
        mesh.face_centres(),
        mesh.points(),
    );
    let cell_points = Overset::cell_points(mesh, c);
    mesh.cell_faces(c).iter().any(|&f| {
        let s = if own[f] == c { sf[f] } else { vec3::scale(sf[f], -1.0) };
        let a = vec3::norm(s);
        a >= 1e-300
            && cell_points
                .iter()
                .any(|&q| vec3::dot(s, vec3::sub(pts[q], fc[f])) / a > tol)
    })
}

/// [`triangulated_margin`] with the ear triangles `ears[f]` in place of the fan for the faces that have them (ADR 0260 N3).
/// A face without them is evaluated as in [`triangulated_margin`], in the same order.
fn ear_margin(mesh: &Mesh, c: usize, x: Vec3, ears: &[Option<Vec<[usize; 3]>>]) -> f64 {
    let (own, fc, pts) = (mesh.owner(), mesh.face_centres(), mesh.points());
    let (mut d, mut omega) = (f64::INFINITY, 0.0);
    for &f in mesh.cell_faces(c) {
        let sign = if own[f] == c { 1.0 } else { -1.0 };
        if let Some(tris) = &ears[f] {
            for &[a, b, q] in tris {
                let (a, b, q) = (pts[a], pts[b], pts[q]);
                d = d.min(triangle_distance(x, a, b, q));
                omega += sign * triangle_solid_angle(x, a, b, q);
            }
            continue;
        }
        let ids = mesh.face_points(f);
        for i in 0..ids.len() {
            let (p, q) = (pts[ids[i]], pts[ids[(i + 1) % ids.len()]]);
            d = d.min(triangle_distance(x, fc[f], p, q));
            omega += sign * triangle_solid_angle(x, fc[f], p, q);
        }
    }
    if omega > 2.0 * std::f64::consts::PI {
        d
    } else {
        -d
    }
}

fn cell_box(mesh: &Mesh, c: usize) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for &f in mesh.cell_faces(c) {
        for &q in mesh.face_points(f) {
            let p = mesh.points()[q];
            for d in 0..3 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
    }
    (lo, hi)
}

/// Point location in a range of cells of a mesh (ADR 0219 M3): uniform bins over the bounding box, each cell in every bin
/// its (slightly widened) bounding box overlaps. A cell contains a point when [`containment_margin`] is at least `-1e-10`
/// of the cell's box diagonal; of several such cells the one with the largest margin, and of those the lowest index.
///
/// A cell with a face whose [`face_warp`] is above that tolerance is *warped* (ADR 0253 G3): its margin is
/// [`triangulated_margin`] instead, the signed distance from the triangles about its face centres. The flags are set on
/// the mesh the locator is built on, which is the mesh it must be asked on.
///
/// A cell that is not warped but has a point on the outer side of one of its face planes by more than the tolerance is
/// *non-convex* (ADR 0260 N1) and is tested by the triangles of its faces too. A face of a warped or non-convex cell
/// whose fan about its centre folds ([`face_fan_folds`]) is triangulated by [`face_ear_triangles`] (ADR 0260 N3).
#[derive(Debug, Clone)]
pub struct CellLocator {
    cells: Range<usize>,
    lo: Vec3,
    hi: Vec3,
    size: Vec3,
    n: [usize; 3],
    bins: Vec<Vec<usize>>,
    tol: Vec<f64>,
    /// Whether each cell of the range is warped (ADR 0253 G3).
    warped: Vec<bool>,
    /// Whether each cell of the range is flat and non-convex (ADR 0260 N1).
    nonconvex: Vec<bool>,
    /// The bounding boxes of the cells of the range; empty when no cell is warped or non-convex.
    boxes: Vec<(Vec3, Vec3)>,
    /// The ear triangles of each face of the mesh whose fan folds and that belongs to a warped or non-convex cell of the
    /// range (ADR 0260 N3); empty when there is no such face.
    ears: Vec<Option<Vec<[usize; 3]>>>,
    /// Number of faces with ear triangles.
    n_ear_clipped: usize,
    /// Number of faces of warped or non-convex cells whose fan folds and for which no ear was found (the fan is kept).
    n_unsupported: usize,
}

impl CellLocator {
    /// Build the bins of the cells `cells` of `mesh`.
    pub fn new(mesh: &Mesh, cells: Range<usize>) -> Self {
        let boxes: Vec<(Vec3, Vec3)> = cells.clone().map(|c| cell_box(mesh, c)).collect();
        let tol: Vec<f64> = boxes
            .iter()
            .map(|(a, b)| 1e-10 * vec3::norm(vec3::sub(*b, *a)))
            .collect();
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (k, (a, b)) in boxes.iter().enumerate() {
            for d in 0..3 {
                lo[d] = lo[d].min(a[d] - tol[k]);
                hi[d] = hi[d].max(b[d] + tol[k]);
            }
        }
        let count = cells.len().max(1) as f64;
        let ext = [0, 1, 2].map(|d| (hi[d] - lo[d]).max(1e-300));
        let s = (ext[0] * ext[1] * ext[2] / count).cbrt();
        let n = [0, 1, 2].map(|d| ((ext[d] / s).ceil() as usize).clamp(1, 1024));
        let size = [0, 1, 2].map(|d| ext[d] / n[d] as f64);
        let mut bins = vec![Vec::new(); n[0] * n[1] * n[2]];
        let bin_of = |x: f64, d: usize| (((x - lo[d]) / size[d]).floor().max(0.0) as usize).min(n[d] - 1);
        for (k, (a, b)) in boxes.iter().enumerate() {
            let i0 = [0, 1, 2].map(|d| bin_of(a[d] - tol[k], d));
            let i1 = [0, 1, 2].map(|d| bin_of(b[d] + tol[k], d));
            for i in i0[0]..=i1[0] {
                for j in i0[1]..=i1[1] {
                    for l in i0[2]..=i1[2] {
                        bins[i + n[0] * (j + n[1] * l)].push(cells.start + k);
                    }
                }
            }
        }
        // ADR 0253 G3: a cell is warped when one of its faces leaves its plane by more than the cell's tolerance
        let warped: Vec<bool> = cells
            .clone()
            .zip(&tol)
            .map(|(c, &t)| mesh.cell_faces(c).iter().any(|&f| face_warp(mesh, f) > t))
            .collect();
        // ADR 0260 N1: a flat cell is non-convex when one of its points lies outside one of its face planes
        let nonconvex: Vec<bool> = cells
            .clone()
            .zip(&tol)
            .zip(&warped)
            .map(|((c, &t), &w)| !w && cell_is_nonconvex(mesh, c, t))
            .collect();
        let triangulated = warped.contains(&true) || nonconvex.contains(&true);
        let boxes = if triangulated { boxes } else { Vec::new() };
        // ADR 0260 N3: the faces of the triangulated cells whose fan folds are ear-clipped
        let (mut ears, mut n_ear_clipped, mut n_unsupported) = (Vec::new(), 0, 0);
        if triangulated {
            let mut seen = vec![false; mesh.n_faces()];
            for (k, c) in cells.clone().enumerate() {
                if !(warped[k] || nonconvex[k]) {
                    continue;
                }
                for &f in mesh.cell_faces(c) {
                    if std::mem::replace(&mut seen[f], true) || !face_fan_folds(mesh, f) {
                        continue;
                    }
                    match face_ear_triangles(mesh, f) {
                        Some(tris) => {
                            if ears.is_empty() {
                                ears = vec![None; mesh.n_faces()];
                            }
                            ears[f] = Some(tris);
                            n_ear_clipped += 1;
                        }
                        None => n_unsupported += 1,
                    }
                }
            }
        }
        CellLocator {
            cells,
            lo,
            hi,
            size,
            n,
            bins,
            tol,
            warped,
            nonconvex,
            boxes,
            ears,
            n_ear_clipped,
            n_unsupported,
        }
    }

    /// Number of warped cells of the range (ADR 0253 G3): the cells tested by [`triangulated_margin`].
    pub fn n_warped(&self) -> usize {
        self.warped.iter().filter(|w| **w).count()
    }

    /// Number of flat non-convex cells of the range (ADR 0260 N1): tested by the triangles of their faces.
    pub fn n_nonconvex(&self) -> usize {
        self.nonconvex.iter().filter(|w| **w).count()
    }

    /// Number of faces of the warped and non-convex cells of the range that are triangulated by [`face_ear_triangles`]
    /// because their fan folds (ADR 0260 N3).
    pub fn n_ear_clipped(&self) -> usize {
        self.n_ear_clipped
    }

    /// Number of faces of the warped and non-convex cells of the range whose fan folds and for which ear clipping found no
    /// ear; their fan is kept (ADR 0260 N3, a limit).
    pub fn n_unsupported(&self) -> usize {
        self.n_unsupported
    }

    /// Every cell of the range that contains `x` under the locator's rule, in increasing order, by a search over all of
    /// them (for checks).
    pub fn containing_cells(&self, mesh: &Mesh, x: Vec3) -> Vec<usize> {
        self.cells
            .clone()
            .filter(|&c| self.accepts(mesh, c, x).is_some())
            .collect()
    }

    /// Whether cell `c` (of this locator's range) contains `x` under the locator's rule, and its margin.
    fn accepts(&self, mesh: &Mesh, c: usize, x: Vec3) -> Option<f64> {
        let k = c - self.cells.start;
        if self.warped[k] || self.nonconvex[k] {
            // ADR 0253 G2: the triangulated cell lies in its box, so a point further outside than the tolerance is refused
            let (lo, hi) = self.boxes[k];
            if (0..3).any(|d| !(x[d] >= lo[d] - self.tol[k] && x[d] <= hi[d] + self.tol[k])) {
                return None;
            }
            let m = if self.ears.is_empty() {
                triangulated_margin(mesh, c, x)
            } else {
                ear_margin(mesh, c, x, &self.ears)
            };
            return (m >= -self.tol[k]).then_some(m);
        }
        let m = containment_margin(mesh, c, x);
        (m >= -self.tol[k]).then_some(m)
    }

    /// The cell of the range that contains `x` (see the type docs), `None` when there is none.
    pub fn locate(&self, mesh: &Mesh, x: Vec3) -> Option<usize> {
        if (0..3).any(|d| !(x[d] >= self.lo[d] && x[d] <= self.hi[d])) {
            return None;
        }
        let i = [0, 1, 2].map(|d| (((x[d] - self.lo[d]) / self.size[d]).floor().max(0.0) as usize).min(self.n[d] - 1));
        let bin = &self.bins[i[0] + self.n[0] * (i[1] + self.n[1] * i[2])];
        let mut best: Option<(usize, f64)> = None;
        for &c in bin {
            if let Some(m) = self.accepts(mesh, c, x) {
                if best.map_or(true, |(_, b)| m > b) {
                    best = Some((c, m));
                }
            }
        }
        best.map(|(c, _)| c)
    }

    /// [`locate`](Self::locate) by a search over every cell of the range (the same rule; for checks).
    pub fn locate_brute_force(&self, mesh: &Mesh, x: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for c in self.cells.clone() {
            if let Some(m) = self.accepts(mesh, c, x) {
                if best.map_or(true, |(_, b)| m > b) {
                    best = Some((c, m));
                }
            }
        }
        best.map(|(c, _)| c)
    }
}

/// Interpolation weights of the stencil points `centres` at the point `x` (ADR 0219 M4): the minimum of `sum w_k^2 /
/// omega_k` under `sum w_k = 1` and `sum w_k (x_k - x) = 0` in the solved directions `dims`, with `omega_k = 1/(|r_k|^2 +
/// 1e-6 h^2)` (`r_k = (x_k - x)`, `h^2` the mean of `|r_k|^2`). Linear fields are reproduced exactly. Fails for fewer
/// points than solved directions plus one and for a degenerate stencil (a pivot below `1e-12` of the largest entry).
pub fn linear_weights(centres: &[Vec3], x: Vec3, dims: [bool; 3]) -> Result<Vec<f64>> {
    let act: Vec<usize> = (0..3).filter(|&d| dims[d]).collect();
    let m = act.len() + 1;
    if centres.len() < m {
        return err(format!(
            "overset weights: {} stencil points for {} solved directions",
            centres.len(),
            act.len()
        ));
    }
    let r: Vec<Vec<f64>> = centres
        .iter()
        .map(|c| act.iter().map(|&d| c[d] - x[d]).collect())
        .collect();
    let h2 = r.iter().map(|v| v.iter().map(|a| a * a).sum::<f64>()).sum::<f64>() / centres.len() as f64;
    if h2.is_nan() || h2 <= 0.0 {
        return err("overset weights: the stencil points coincide with the point");
    }
    let h = h2.sqrt();
    let rows: Vec<Vec<f64>> = r
        .iter()
        .map(|v| std::iter::once(1.0).chain(v.iter().map(|a| a / h)).collect())
        .collect();
    let om: Vec<f64> = rows
        .iter()
        .map(|a| 1.0 / (a[1..].iter().map(|b| b * b).sum::<f64>() + 1e-6))
        .collect();
    let mut mat = vec![vec![0.0; m + 1]; m];
    for (a, w) in rows.iter().zip(&om) {
        for i in 0..m {
            for j in 0..m {
                mat[i][j] += w * a[i] * a[j];
            }
        }
    }
    mat[0][m] = 1.0;
    let scale = mat
        .iter()
        .flat_map(|row| row[..m].iter())
        .fold(0.0f64, |s, v| s.max(v.abs()));
    for col in 0..m {
        let piv = (col..m)
            .max_by(|&a, &b| mat[a][col].abs().total_cmp(&mat[b][col].abs()))
            .unwrap_or(col);
        if mat[piv][col].abs() <= 1e-12 * scale {
            return err("overset weights: degenerate stencil");
        }
        mat.swap(col, piv);
        for row in 0..m {
            if row != col {
                let f = mat[row][col] / mat[col][col];
                for k in col..=m {
                    mat[row][k] -= f * mat[col][k];
                }
            }
        }
    }
    let lambda: Vec<f64> = (0..m).map(|i| mat[i][m] / mat[i][i]).collect();
    Ok(rows
        .iter()
        .zip(&om)
        .map(|(a, w)| w * a.iter().zip(&lambda).map(|(x, l)| x * l).sum::<f64>())
        .collect())
}

/// Which interpolation the stencils of the fringe cells carry (ADR 0251 D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DonorInterpolation {
    /// The donor cell and its calculated face neighbours, exact for linear fields (ADR 0219 M4). The default.
    #[default]
    Linear,
    /// The donor cell and the calculated cells that share a point with it, exact for quadratic fields (ADR 0251 D2, D3);
    /// the linear stencil where that neighbourhood cannot carry a quadratic (D4).
    Quadratic,
}

/// The largest `sum |w_k|` that [`quadratic_weights`] accepts (ADR 0251 D3, R3).
pub const QUADRATIC_MAX_ABS_SUM: f64 = 4.0;

/// Interpolation weights of the stencil points `centres` at the point `x` that reproduce every polynomial of total degree
/// two in the solved directions `dims` (ADR 0251 D3): the minimum of `sum w_k^2 / omega_k` under `sum w_k a_k = e_0`, where
/// `a_k` are the monomials `1, s_i, s_i s_j` of the scaled offsets `s_k = (x_k - x) / h` (`h^2` the mean of `|x_k - x|^2`)
/// and `omega_k = 1/(|s_k|^2 + 1e-6)`, the weight of [`linear_weights`]. These are the weights with which the weighted
/// least-squares quadratic through the stencil values gives its value at `x`. Solved by a Householder QR factorisation
/// with column pivoting of the rows `sqrt(omega_k) a_k`, taken in the order of decreasing `omega_k`. The weights sum to
/// one and can be negative. Fails for fewer points than monomials (6 for two solved directions, 10 for three), for a
/// rank-deficient stencil (a diagonal entry of `R` at most `1e-10` of the largest) and for `sum |w_k|` above
/// [`QUADRATIC_MAX_ABS_SUM`].
pub fn quadratic_weights(centres: &[Vec3], x: Vec3, dims: [bool; 3]) -> Result<Vec<f64>> {
    let act: Vec<usize> = (0..3).filter(|&d| dims[d]).collect();
    let nd = act.len();
    let m = 1 + nd + nd * (nd + 1) / 2;
    let n = centres.len();
    if n < m {
        return err(format!(
            "overset quadratic weights: {n} stencil points for {m} monomials"
        ));
    }
    let r: Vec<Vec<f64>> = centres
        .iter()
        .map(|c| act.iter().map(|&d| c[d] - x[d]).collect())
        .collect();
    let h2 = r.iter().map(|v| v.iter().map(|a| a * a).sum::<f64>()).sum::<f64>() / n as f64;
    if h2.is_nan() || h2 <= 0.0 {
        return err("overset quadratic weights: the stencil points coincide with the point");
    }
    let h = h2.sqrt();
    let s: Vec<Vec<f64>> = r.iter().map(|v| v.iter().map(|a| a / h).collect()).collect();
    let om: Vec<f64> = s
        .iter()
        .map(|v| 1.0 / (v.iter().map(|a| a * a).sum::<f64>() + 1e-6))
        .collect();
    // rows of very different size first (a stable sort, so the order is fixed by the input)
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| om[b].total_cmp(&om[a]));
    let mut b: Vec<Vec<f64>> = order
        .iter()
        .map(|&k| {
            let q = om[k].sqrt();
            let mut a = Vec::with_capacity(m);
            a.push(q);
            a.extend(s[k].iter().map(|v| q * v));
            for i in 0..nd {
                for j in i..nd {
                    a.push(q * s[k][i] * s[k][j]);
                }
            }
            a
        })
        .collect();
    // B P = Q R: Householder reflections `I - 2 v v^T / (v . v)`, the column of the largest remaining norm first
    let mut perm: Vec<usize> = (0..m).collect();
    let mut reflectors: Vec<(Vec<f64>, f64)> = Vec::with_capacity(m);
    for j in 0..m {
        let norms: Vec<f64> = (j..m).map(|c| (j..n).map(|i| b[i][c] * b[i][c]).sum::<f64>()).collect();
        let piv = j
            + (0..norms.len())
                .max_by(|&p, &q| norms[p].total_cmp(&norms[q]))
                .unwrap_or(0);
        if piv != j {
            for row in b.iter_mut() {
                row.swap(j, piv);
            }
            perm.swap(j, piv);
        }
        let alpha = norms[piv - j].sqrt();
        let beta = if b[j][j] >= 0.0 { -alpha } else { alpha };
        let mut v: Vec<f64> = (j..n).map(|i| b[i][j]).collect();
        v[0] -= beta;
        let vv: f64 = v.iter().map(|a| a * a).sum();
        if vv > 0.0 {
            for c in j..m {
                let f = 2.0 * (j..n).map(|i| v[i - j] * b[i][c]).sum::<f64>() / vv;
                for i in j..n {
                    b[i][c] -= f * v[i - j];
                }
            }
        }
        reflectors.push((v, vv));
    }
    let (mut dmin, mut dmax) = (f64::INFINITY, 0.0f64);
    for (j, row) in b.iter().enumerate().take(m) {
        dmin = dmin.min(row[j].abs());
        dmax = dmax.max(row[j].abs());
    }
    if dmin.is_nan() || dmin <= 1e-10 * dmax {
        return err("overset quadratic weights: rank-deficient stencil");
    }
    // B^T v = e_0 with the smallest |v|: R^T y = P^T e_0, v = Q (y, 0)
    let mut z = vec![0.0; n];
    for j in 0..m {
        let g = if perm[j] == 0 { 1.0 } else { 0.0 };
        let known: f64 = (0..j).map(|i| b[i][j] * z[i]).sum();
        z[j] = (g - known) / b[j][j];
    }
    for (j, (v, vv)) in reflectors.iter().enumerate().rev() {
        if *vv > 0.0 {
            let f = 2.0 * (j..n).map(|i| v[i - j] * z[i]).sum::<f64>() / vv;
            for i in j..n {
                z[i] -= f * v[i - j];
            }
        }
    }
    let mut w = vec![0.0; n];
    for (i, &k) in order.iter().enumerate() {
        w[k] = om[k].sqrt() * z[i];
    }
    let abs_sum: f64 = w.iter().map(|a| a.abs()).sum();
    if !abs_sum.is_finite() || abs_sum > QUADRATIC_MAX_ABS_SUM {
        return err(format!(
            "overset quadratic weights: sum |w| = {abs_sum:.3} above {QUADRATIC_MAX_ABS_SUM} (a one-sided stencil)"
        ));
    }
    Ok(w)
}

/// Kind of a cell at one pose (ADR 0219 M2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// Its finite-volume equations are solved.
    Calculated,
    /// Its values are interpolated from the other part.
    Fringe,
    /// Excluded (inside the body or in the cut-out part of the overlap).
    Hole,
}

/// Interpolation stencil of a fringe cell: the donor cell (first) and its calculated face neighbours, with the weights
/// (with [`DonorInterpolation::Quadratic`]: the donor cell and the calculated cells that share a point with it).
#[derive(Debug, Clone, PartialEq)]
pub struct Stencil {
    /// Cells of the other part (the donor first).
    pub cells: Vec<usize>,
    /// Weights (`sum = 1`, linear fields exact; with [`DonorInterpolation::Quadratic`] quadratic fields too).
    pub weights: Vec<f64>,
}

/// Kinds and stencils of every cell at one pose.
#[derive(Debug, Clone, PartialEq)]
pub struct Connectivity {
    /// Kind of every composite cell.
    pub kind: Vec<CellKind>,
    /// Every fringe cell with its stencil, in increasing cell order.
    pub stencils: Vec<(usize, Stencil)>,
    /// Number of background cells (the background cells are `0..n_background_cells`).
    pub n_background_cells: usize,
    /// The interpolation the stencils were built with.
    pub interpolation: DonorInterpolation,
    /// With [`DonorInterpolation::Quadratic`]: the fringe cells that carry the linear stencil because their donor's
    /// neighbourhood cannot carry a quadratic (ADR 0251 D4), in increasing cell order. Empty with `Linear`.
    pub fallbacks: Vec<usize>,
}

impl Connectivity {
    /// Every one of `n` cells calculated (a mesh without overset coupling).
    pub fn all_calculated(n: usize) -> Self {
        Connectivity {
            kind: vec![CellKind::Calculated; n],
            stencils: Vec::new(),
            n_background_cells: n,
            interpolation: DonorInterpolation::Linear,
            fallbacks: Vec::new(),
        }
    }

    /// Number of fringe cells that fell back to the linear stencil (zero with [`DonorInterpolation::Linear`]).
    pub fn n_fallback(&self) -> usize {
        self.fallbacks.len()
    }

    /// No fringe and no hole cell.
    pub fn is_trivial(&self) -> bool {
        self.kind.iter().all(|k| *k == CellKind::Calculated)
    }

    /// Number of hole cells.
    pub fn n_hole(&self) -> usize {
        self.kind.iter().filter(|k| **k == CellKind::Hole).count()
    }

    /// Number of background fringe cells.
    pub fn n_fringe_background(&self) -> usize {
        self.kind[..self.n_background_cells]
            .iter()
            .filter(|k| **k == CellKind::Fringe)
            .count()
    }

    /// Number of component fringe cells.
    pub fn n_fringe_component(&self) -> usize {
        self.kind[self.n_background_cells..]
            .iter()
            .filter(|k| **k == CellKind::Fringe)
            .count()
    }
}

/// Hole function of the background, evaluated at points in the component's reference frame.
pub type HoleFn = Arc<dyn Fn(Vec3) -> bool + Send + Sync>;

/// What the user gives: the component's outer patches (composite names) and the hole function.
#[derive(Clone)]
pub struct OversetSpec {
    /// Component patches whose cells are fringe cells (composite names, with the prefix).
    pub outer_patches: Vec<String>,
    /// True where a background cell centre, mapped into the component's reference frame, is a hole.
    pub hole: HoleFn,
}

impl std::fmt::Debug for OversetSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OversetSpec")
            .field("outer_patches", &self.outer_patches)
            .finish_non_exhaustive()
    }
}

/// A composite mesh with its overset specification, the locators and the fixed topology data.
#[derive(Debug, Clone)]
pub struct Overset {
    /// The composite mesh (component at its reference position).
    pub composite: Composite,
    /// The specification.
    pub spec: OversetSpec,
    background_locator: CellLocator,
    component_locator: CellLocator,
    outer_fringe: Vec<usize>,
    neighbours: Vec<Vec<usize>>,
    /// Solved directions (false for the direction normal to the empty patches of a 2-D mesh).
    pub dims: [bool; 3],
    interpolation: DonorInterpolation,
    /// The cells around every point; built when `Quadratic` is first set, empty before.
    point_cells: Vec<Vec<usize>>,
}

impl Overset {
    /// Check the specification and build the locators. Fails for an outer patch that is not a component patch (named) and
    /// for an empty part.
    pub fn new(composite: Composite, spec: OversetSpec) -> Result<Self> {
        let mesh = &composite.mesh;
        if composite.n_background_cells == 0 || composite.n_background_cells == mesh.n_cells() {
            return err("overset: the composite needs background and component cells");
        }
        let mut outer_fringe = Vec::new();
        for name in &spec.outer_patches {
            if !composite.component_patches.contains(name) {
                return err(format!("overset: outer patch '{name}' is not a component patch"));
            }
            let p = mesh
                .patch(name)
                .ok_or_else(|| Error::Mesh(format!("overset: no patch '{name}'")))?;
            outer_fringe.extend(p.faces().map(|f| mesh.owner()[f]));
        }
        outer_fringe.sort_unstable();
        outer_fringe.dedup();
        if outer_fringe.is_empty() {
            return err("overset: no component cell lies on the outer patches");
        }
        let mut neighbours = vec![Vec::new(); mesh.n_cells()];
        for f in 0..mesh.n_internal_faces() {
            let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
            neighbours[o].push(n);
            neighbours[n].push(o);
        }
        let mut dims = [true; 3];
        for p in mesh.patches() {
            if p.kind == PatchKind::Empty {
                for f in p.faces() {
                    let n = vec3::unit(mesh.face_area_vectors()[f]);
                    for (d, a) in dims.iter_mut().enumerate() {
                        if n[d].abs() > 0.99 {
                            *a = false;
                        }
                    }
                }
            }
        }
        let background_locator = CellLocator::new(mesh, composite.background_cells());
        let component_locator = CellLocator::new(mesh, composite.component_cells());
        Ok(Overset {
            composite,
            spec,
            background_locator,
            component_locator,
            outer_fringe,
            neighbours,
            dims,
            interpolation: DonorInterpolation::Linear,
            point_cells: Vec::new(),
        })
    }

    /// Choose the donor interpolation of every later [`connectivity`](Self::connectivity) (F-336, ADR 0251 D1). The
    /// default is [`DonorInterpolation::Linear`]; `Quadratic` is the opt-in and builds, once, the table of the cells
    /// around every point.
    pub fn set_donor_interpolation(&mut self, interpolation: DonorInterpolation) {
        if interpolation == DonorInterpolation::Quadratic && self.point_cells.is_empty() {
            let mesh = &self.composite.mesh;
            let mut point_cells = vec![Vec::new(); mesh.n_points()];
            for c in 0..mesh.n_cells() {
                for q in Self::cell_points(mesh, c) {
                    point_cells[q].push(c);
                }
            }
            self.point_cells = point_cells;
        }
        self.interpolation = interpolation;
    }

    /// The donor interpolation in use.
    pub fn donor_interpolation(&self) -> DonorInterpolation {
        self.interpolation
    }

    /// The points of cell `c`, sorted, each once.
    fn cell_points(mesh: &Mesh, c: usize) -> Vec<usize> {
        let mut pts: Vec<usize> = mesh
            .cell_faces(c)
            .iter()
            .flat_map(|&f| mesh.face_points(f).iter().copied())
            .collect();
        pts.sort_unstable();
        pts.dedup();
        pts
    }

    /// The cells that share at least one point with cell `c` (without `c`), in increasing order (ADR 0251 D2). Empty
    /// before [`DonorInterpolation::Quadratic`] was first set.
    pub fn point_neighbours(&self, c: usize) -> Vec<usize> {
        if self.point_cells.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<usize> = Self::cell_points(&self.composite.mesh, c)
            .into_iter()
            .flat_map(|q| self.point_cells[q].iter().copied())
            .filter(|&k| k != c)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Face neighbours of cell `c`.
    pub fn neighbours(&self, c: usize) -> &[usize] {
        &self.neighbours[c]
    }

    /// The locator of the background cells (on the composite mesh, which the background never leaves).
    pub fn background_locator(&self) -> &CellLocator {
        &self.background_locator
    }

    /// The locator of the component cells at their reference position.
    pub fn component_locator(&self) -> &CellLocator {
        &self.component_locator
    }

    /// Kinds and stencils with the component at `pose`; `moved` is the composite mesh at that pose (ADR 0219 M2 to M4).
    /// Fails, naming the fringe cell, its part and its centre, when a fringe centre lies in no cell of the other part or its
    /// donor cell is not calculated ("no donor"), and for a degenerate stencil.
    pub fn connectivity(&self, moved: &Mesh, pose: &Pose) -> Result<Connectivity> {
        let reference = &self.composite.mesh;
        let n = reference.n_cells();
        let nb = self.composite.n_background_cells;
        if moved.n_cells() != n {
            return err("overset connectivity: the moved mesh is not the composite mesh");
        }
        let mut kind = vec![CellKind::Calculated; n];
        for &c in &self.outer_fringe {
            kind[c] = CellKind::Fringe;
        }
        let centres = moved.cell_centres();
        for c in 0..nb {
            if (self.spec.hole)(pose_inverse_apply(pose, centres[c])) {
                kind[c] = CellKind::Hole;
            }
        }
        for c in 0..nb {
            if kind[c] == CellKind::Calculated && self.neighbours[c].iter().any(|&q| kind[q] == CellKind::Hole) {
                kind[c] = CellKind::Fringe;
            }
        }
        let ref_centres = reference.cell_centres();
        let mut stencils = Vec::new();
        let mut fallbacks = Vec::new();
        for c in 0..n {
            if kind[c] != CellKind::Fringe {
                continue;
            }
            let background = c < nb;
            let (x, locator, part) = if background {
                (
                    pose_inverse_apply(pose, centres[c]),
                    &self.component_locator,
                    "background",
                )
            } else {
                (centres[c], &self.background_locator, "component")
            };
            let fail = |why: &str| -> Result<Connectivity> {
                err(format!(
                    "overset: no donor for the {part} fringe cell {c} at ({:.6}, {:.6}, {:.6}): {why}",
                    centres[c][0], centres[c][1], centres[c][2]
                ))
            };
            let Some(d) = locator.locate(reference, x) else {
                return fail("the point lies in no cell of the other part");
            };
            if kind[d] != CellKind::Calculated {
                return fail(&format!("its donor cell {d} is not a calculated cell"));
            }
            if self.interpolation == DonorInterpolation::Quadratic {
                // ADR 0251 D2, D3: the donor and the calculated cells around its points; D4: else the stencil below
                let mut cells = vec![d];
                cells.extend(
                    self.point_neighbours(d)
                        .into_iter()
                        .filter(|&q| kind[q] == CellKind::Calculated),
                );
                let pts: Vec<Vec3> = cells.iter().map(|&q| ref_centres[q]).collect();
                if let Ok(weights) = quadratic_weights(&pts, x, self.dims) {
                    stencils.push((c, Stencil { cells, weights }));
                    continue;
                }
                fallbacks.push(c);
            }
            let mut cells = vec![d];
            cells.extend(
                self.neighbours[d]
                    .iter()
                    .copied()
                    .filter(|&q| kind[q] == CellKind::Calculated),
            );
            let pts: Vec<Vec3> = cells.iter().map(|&q| ref_centres[q]).collect();
            let weights = match linear_weights(&pts, x, self.dims) {
                Ok(w) => w,
                Err(e) => return fail(&e.to_string()),
            };
            stencils.push((c, Stencil { cells, weights }));
        }
        Ok(Connectivity {
            kind,
            stencils,
            n_background_cells: nb,
            interpolation: self.interpolation,
            fallbacks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
    use crate::motion::moved_mesh;

    /// Nodes `x0 + axis_coords(n, len)` with the cell at the far end (`toward_end`) or at `x0` of size `h`.
    fn graded(x0: f64, len: f64, n: usize, h: f64, toward_end: bool) -> Vec<f64> {
        let r = expansion_for_end_cell(n, len, h).unwrap();
        let r = if toward_end { r } else { 1.0 / r };
        axis_coords(n, len, Grading::Expansion(r))
            .unwrap()
            .into_iter()
            .map(|x| x0 + x)
            .collect()
    }

    fn join(parts: &[Vec<f64>]) -> Vec<f64> {
        let mut out: Vec<f64> = Vec::new();
        for p in parts {
            let skip = usize::from(!out.is_empty());
            out.extend_from_slice(&p[skip..]);
        }
        out
    }

    fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
    }

    /// ADR 0219 TR: the background and the component (prefix `c_`), and the composite with its specification.
    fn tr() -> (Mesh, Mesh, Overset) {
        let xs = join(&[
            graded(-25.0, 17.0, 30, 0.1, true),
            uniform(-8.0, 12.5, 125),
            graded(4.5, 17.5, 30, 0.1, false),
        ]);
        let ys = join(&[
            graded(-20.0, 17.5, 30, 0.1, true),
            uniform(-2.5, 5.0, 50),
            graded(2.5, 17.5, 30, 0.1, false),
        ]);
        let bg = rectilinear_2d(&xs, &ys, 0.1).unwrap();
        let q: f64 = 1.08;
        let r_out = 0.5 + 0.01 * (q.powi(30) - 1.0) / (q - 1.0);
        let comp = cylinder_ogrid(96, 30, 0.5, r_out, Grading::Expansion(q.powi(29)), 0.1).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: vec!["c_farfield".into()],
            hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
        };
        let ov = Overset::new(composite, spec).unwrap();
        (bg, comp, ov)
    }

    fn pose_x(x: f64) -> Pose {
        Pose::translation([x, 0.0, 0.0])
    }

    const POSES: [f64; 5] = [0.0, -0.37, -1.234, -3.9, -7.5];

    /// A fixed linear congruential sequence in [0, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[test]
    fn k1_compose_keeps_both_parts() {
        let (bg, comp, ov) = tr();
        let m = &ov.composite.mesh;
        assert_eq!(m.n_cells(), bg.n_cells() + comp.n_cells());
        assert_eq!(m.n_internal_faces(), bg.n_internal_faces() + comp.n_internal_faces());
        assert_eq!(m.n_boundary_faces(), bg.n_boundary_faces() + comp.n_boundary_faces());
        let nb = bg.n_cells();
        for c in 0..m.n_cells() {
            let (cc, v) = if c < nb {
                (bg.cell_centres()[c], bg.cell_volumes()[c])
            } else {
                (comp.cell_centres()[c - nb], comp.cell_volumes()[c - nb])
            };
            assert_eq!(m.cell_centres()[c], cc, "centre of cell {c}");
            assert_eq!(m.cell_volumes()[c], v, "volume of cell {c}");
        }
        let names: Vec<(String, usize)> = m.patches().iter().map(|p| (p.name.clone(), p.size)).collect();
        let mut want: Vec<(String, usize)> = bg.patches().iter().map(|p| (p.name.clone(), p.size)).collect();
        want.extend(comp.patches().iter().map(|p| (format!("c_{}", p.name), p.size)));
        assert_eq!(names, want);
        println!(
            "K1: {} + {} cells, patches {:?}",
            nb,
            comp.n_cells(),
            names.iter().map(|n| n.0.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn k2_locator_finds_every_centre_and_agrees_with_a_full_search() {
        let (_, _, ov) = tr();
        let m = &ov.composite.mesh;
        for (loc, range) in [
            (ov.background_locator(), ov.composite.background_cells()),
            (ov.component_locator(), ov.composite.component_cells()),
        ] {
            for c in range.clone() {
                assert_eq!(loc.locate(m, m.cell_centres()[c]), Some(c), "centre of cell {c}");
            }
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for c in range.clone() {
                for d in 0..3 {
                    lo[d] = lo[d].min(m.cell_centres()[c][d]);
                    hi[d] = hi[d].max(m.cell_centres()[c][d]);
                }
            }
            let mut g = Lcg(0x5eed_f134);
            let (mut inside, mut outside) = (0, 0);
            for _ in 0..2000 {
                let x = [
                    lo[0] - 0.2 + (hi[0] - lo[0] + 0.4) * g.next(),
                    lo[1] - 0.2 + (hi[1] - lo[1] + 0.4) * g.next(),
                    0.05,
                ];
                let a = loc.locate(m, x);
                let b = loc.locate_brute_force(m, x);
                assert_eq!(a, b, "point {x:?}");
                if a.is_some() {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
            println!("K2: cells {range:?}: every centre found; 2000 points, {inside} inside, {outside} outside, equal to the full search");
        }
    }

    #[test]
    fn k3_weights_are_exact_for_linear_fields() {
        let (_, _, ov) = tr();
        let f = |x: Vec3| 3.0 + 2.0 * x[0] - 5.0 * x[1];
        let (mut e_sum, mut e_lin, mut n_min, mut w_max, mut w_min) =
            (0.0f64, 0.0f64, usize::MAX, f64::NEG_INFINITY, f64::INFINITY);
        for s in POSES {
            let pose = pose_x(2.0 + s);
            let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
            let conn = ov.connectivity(&moved, &pose).unwrap();
            let cc = moved.cell_centres();
            let fmax = cc.iter().map(|&x| f(x).abs()).fold(0.0, f64::max);
            for (c, st) in &conn.stencils {
                let sw: f64 = st.weights.iter().sum();
                e_sum = e_sum.max((sw - 1.0).abs());
                let fi: f64 = st.cells.iter().zip(&st.weights).map(|(&k, w)| w * f(cc[k])).sum();
                e_lin = e_lin.max((fi - f(cc[*c])).abs() / fmax);
                n_min = n_min.min(st.cells.len());
                for &w in &st.weights {
                    w_max = w_max.max(w);
                    w_min = w_min.min(w);
                }
            }
        }
        println!(
            "K3: |sum w - 1| {e_sum:.3e} (limit 1e-13), linear field {e_lin:.3e} of max|f| (limit 1e-12), smallest stencil {n_min}, weights in [{w_min:.4}, {w_max:.4}]"
        );
        assert!(e_sum <= 1e-13, "sum of weights {e_sum}");
        assert!(e_lin <= 1e-12, "linear field {e_lin}");
        assert!(n_min >= 3, "stencil of {n_min}");
    }

    #[test]
    fn k4_kinds_have_a_fringe_layer_and_calculated_donors() {
        let (bg, _, ov) = tr();
        let nb = bg.n_cells();
        let outer: Vec<usize> = (0..96).map(|it| nb + 29 * 96 + it).collect();
        for s in POSES {
            let pose = pose_x(2.0 + s);
            let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
            let conn = ov.connectivity(&moved, &pose).unwrap();
            let cc = moved.cell_centres();
            for c in 0..cc.len() {
                if conn.kind[c] == CellKind::Calculated {
                    assert!(
                        ov.neighbours(c).iter().all(|&q| conn.kind[q] != CellKind::Hole),
                        "calculated cell {c} next to a hole"
                    );
                }
            }
            let loc = |c: usize| {
                if c < nb {
                    ov.component_locator()
                        .locate(&ov.composite.mesh, pose_inverse_apply(&pose, cc[c]))
                } else {
                    ov.background_locator().locate(&ov.composite.mesh, cc[c])
                }
            };
            for (c, st) in &conn.stencils {
                assert_eq!(conn.kind[*c], CellKind::Fringe);
                assert_eq!(Some(st.cells[0]), loc(*c), "donor of {c}");
                for &k in &st.cells {
                    assert_eq!(conn.kind[k], CellKind::Calculated, "stencil cell {k} of {c}");
                    assert_ne!(k < nb, *c < nb, "stencil cell {k} of {c} in the same part");
                }
            }
            let centre = [2.0 + s, 0.0, 0.0];
            for c in 0..nb {
                let r = ((cc[c][0] - centre[0]).powi(2) + (cc[c][1] - centre[1]).powi(2)).sqrt();
                if r < 0.5 {
                    assert_eq!(conn.kind[c], CellKind::Hole, "cell {c} inside the body");
                }
            }
            let comp_fringe: Vec<usize> = (nb..cc.len()).filter(|&c| conn.kind[c] == CellKind::Fringe).collect();
            assert_eq!(comp_fringe, outer);
            assert!(conn.n_hole() > 0 && conn.n_fringe_background() > 0);
            println!(
                "K4: pose {s}: {} holes, {} background fringe, {} component fringe, {} stencils",
                conn.n_hole(),
                conn.n_fringe_background(),
                conn.n_fringe_component(),
                conn.stencils.len()
            );
        }
    }

    #[test]
    fn k5_refusals_are_named() {
        let (bg, comp, ov) = tr();
        let e = compose(&bg, &comp, "").unwrap_err().to_string();
        println!("K5 duplicate: {e}");
        assert!(e.contains("frontAndBack"), "{e}");
        let spec = OversetSpec {
            outer_patches: vec!["nope".into()],
            hole: Arc::new(|_| false),
        };
        let e = Overset::new(ov.composite.clone(), spec).unwrap_err().to_string();
        println!("K5 patch: {e}");
        assert!(e.contains("nope"), "{e}");
        let pose = Pose::translation([0.0, 19.0, 0.0]);
        let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
        let e = ov.connectivity(&moved, &pose).unwrap_err().to_string();
        println!("K5 orphan: {e}");
        assert!(e.contains("donor"), "{e}");
    }

    #[test]
    fn pose_inverse_undoes_the_pose() {
        let pose = Pose {
            origin: [0.3, -0.2, 0.0],
            axis: [0.0, 0.0, 1.0],
            angle: 0.7,
            translation: [1.0, 2.0, 0.0],
        };
        let p = [0.4, 1.3, 0.05];
        let q = pose_inverse_apply(&pose, pose.apply(p));
        assert!(vec3::norm(vec3::sub(p, q)) < 1e-15);
    }
}
