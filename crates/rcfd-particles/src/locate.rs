//! Point location and segment walking on a polyhedral mesh (ADR 0028 D5).
//!
//! Faces are triangulated in a fan around the face vertex average; both cells sharing a face see the same triangles, so the
//! partition of space into cells has no gaps or overlaps. Cell membership (the reference used by the tests) is the union of
//! the tetrahedra (cell centre, face vertex average, two consecutive face vertices).

use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Result of walking a segment through the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Walk {
    /// The end point is inside this cell.
    Inside(usize),
    /// The segment leaves the domain through boundary face `face` of cell `cell` at `point`.
    Boundary {
        /// Cell the segment was in.
        cell: usize,
        /// Global face index.
        face: usize,
        /// Crossing point.
        point: Vec3,
    },
    /// The iteration cap was reached (a failure of the walk, not expected).
    Lost,
}

/// Mesh locator and tracker geometry.
#[derive(Debug, Clone)]
pub struct Locator {
    mesh: Arc<Mesh>,
    face_avg: Vec<Vec3>,
    lc: Vec<f64>,
    lo: Vec<Vec3>,
    hi: Vec<Vec3>,
    bin_lo: Vec3,
    bin_h: Vec3,
    bin_n: [usize; 3],
    bins: Vec<Vec<usize>>,
}

const WALK_CAP: usize = 100_000;
const BARY_TOL: f64 = 1e-9;

fn bin_index(n: [usize; 3], i: [usize; 3]) -> usize {
    (i[2] * n[1] + i[1]) * n[0] + i[0]
}

impl Locator {
    /// Build the locator.
    pub fn new(mesh: Arc<Mesh>) -> Locator {
        let nf = mesh.n_faces();
        let pts = mesh.points();
        let mut face_avg = Vec::with_capacity(nf);
        for f in 0..nf {
            let ids = mesh.face_points(f);
            let mut s = [0.0; 3];
            for &i in ids {
                s = add(s, pts[i]);
            }
            face_avg.push(scale(s, 1.0 / ids.len() as f64));
        }
        let nc = mesh.n_cells();
        let mut lo = vec![[f64::MAX; 3]; nc];
        let mut hi = vec![[f64::MIN; 3]; nc];
        for c in 0..nc {
            for &f in mesh.cell_faces(c) {
                for &i in mesh.face_points(f) {
                    for k in 0..3 {
                        lo[c][k] = lo[c][k].min(pts[i][k]);
                        hi[c][k] = hi[c][k].max(pts[i][k]);
                    }
                }
            }
        }
        let lc: Vec<f64> = mesh.cell_volumes().iter().map(|v| v.abs().cbrt()).collect();
        // Bins of cell centres.
        let cc = mesh.cell_centres();
        let mut blo = [f64::MAX; 3];
        let mut bhi = [f64::MIN; 3];
        for p in cc {
            for k in 0..3 {
                blo[k] = blo[k].min(p[k]);
                bhi[k] = bhi[k].max(p[k]);
            }
        }
        let nb = ((nc as f64 / 2.0).cbrt().ceil() as usize).max(1);
        let bin_n = [nb; 3];
        let mut bin_h = [0.0; 3];
        for k in 0..3 {
            bin_h[k] = ((bhi[k] - blo[k]) / nb as f64).max(1e-300);
        }
        let mut bins = vec![Vec::new(); nb * nb * nb];
        for (c, p) in cc.iter().enumerate() {
            let mut i = [0usize; 3];
            for k in 0..3 {
                i[k] = (((p[k] - blo[k]) / bin_h[k]) as usize).min(nb - 1);
            }
            bins[bin_index(bin_n, i)].push(c);
        }
        Locator {
            mesh,
            face_avg,
            lc,
            lo,
            hi,
            bin_lo: blo,
            bin_h,
            bin_n,
            bins,
        }
    }

    /// The mesh.
    pub fn mesh(&self) -> &Arc<Mesh> {
        &self.mesh
    }

    /// Cell length scale `V^(1/3)`.
    pub fn length_scale(&self, c: usize) -> f64 {
        self.lc[c]
    }

    /// Face vertex average (the apex of the face fan).
    pub fn face_apex(&self, f: usize) -> Vec3 {
        self.face_avg[f]
    }

    /// Bounding box of a cell.
    pub fn cell_bbox(&self, c: usize) -> (Vec3, Vec3) {
        (self.lo[c], self.hi[c])
    }

    /// Is `p` inside cell `c` (tetrahedral decomposition, relative tolerance `tol`)?
    pub fn contains(&self, c: usize, p: Vec3, tol: f64) -> bool {
        let m = &self.mesh;
        let pts = m.points();
        let cc = m.cell_centres()[c];
        for &f in m.cell_faces(c) {
            let ids = m.face_points(f);
            let fa = self.face_avg[f];
            let n = ids.len();
            for i in 0..n {
                let t1 = pts[ids[i]];
                let t2 = pts[ids[(i + 1) % n]];
                if tet_contains(cc, fa, t1, t2, p, tol) {
                    return true;
                }
            }
        }
        false
    }

    /// Brute-force search: the first cell containing `p` among those whose bounding box does.
    pub fn locate_brute(&self, p: Vec3) -> Option<usize> {
        self.all_containing(p).into_iter().next()
    }

    /// All cells that contain `p` (reference for tests).
    pub fn all_containing(&self, p: Vec3) -> Vec<usize> {
        let mut out = Vec::new();
        for c in 0..self.mesh.n_cells() {
            let l = self.lc[c] * 1e-9;
            if (0..3).all(|k| p[k] >= self.lo[c][k] - l && p[k] <= self.hi[c][k] + l) && self.contains(c, p, 1e-10) {
                out.push(c);
            }
        }
        out
    }

    /// Nearest cell centre (bin grid search).
    pub fn nearest_centre(&self, p: Vec3) -> usize {
        let cc = self.mesh.cell_centres();
        let nb = self.bin_n;
        let mut i = [0i64; 3];
        for k in 0..3 {
            i[k] = (((p[k] - self.bin_lo[k]) / self.bin_h[k]).floor() as i64).clamp(0, nb[k] as i64 - 1);
        }
        let hmin = self.bin_h.iter().cloned().fold(f64::MAX, f64::min);
        let mut best = (f64::MAX, 0usize);
        let rmax = nb.iter().cloned().max().unwrap_or(1) as i64;
        for r in 0..=rmax {
            for dz in -r..=r {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != r {
                            continue;
                        }
                        let j = [i[0] + dx, i[1] + dy, i[2] + dz];
                        if (0..3).any(|k| j[k] < 0 || j[k] >= nb[k] as i64) {
                            continue;
                        }
                        let b = bin_index(nb, [j[0] as usize, j[1] as usize, j[2] as usize]);
                        for &c in &self.bins[b] {
                            let d = sub(cc[c], p);
                            let d2 = dot(d, d);
                            if d2 < best.0 {
                                best = (d2, c);
                            }
                        }
                    }
                }
            }
            if best.0 < f64::MAX && best.0.sqrt() <= r as f64 * hmin {
                break;
            }
        }
        best.1
    }

    fn start_cell(&self, p: Vec3, hint: Option<usize>) -> usize {
        match hint {
            Some(h) if h < self.mesh.n_cells() => h,
            _ => self.nearest_centre(p),
        }
    }

    /// Locate by walking only (no brute-force fallback): the segment from the start cell centre to `p` is followed across
    /// faces. `None` when the walk leaves the domain or ends in a cell that does not contain `p`.
    pub fn locate_walk(&self, p: Vec3, hint: Option<usize>) -> Option<usize> {
        let start = self.start_cell(p, hint);
        if self.contains(start, p, 1e-10) {
            return Some(start);
        }
        match self.walk(start, self.mesh.cell_centres()[start], p) {
            Walk::Inside(c) if self.contains(c, p, 1e-9) => Some(c),
            _ => None,
        }
    }

    /// Locate the cell containing `p`, starting from `hint` if given (walk, then brute force). `None` when `p` is outside
    /// the (faceted) domain.
    pub fn locate(&self, p: Vec3, hint: Option<usize>) -> Option<usize> {
        self.locate_walk(p, hint).or_else(|| self.locate_brute(p))
    }

    /// Walk the segment `a -> b` starting in cell `c0` (which must contain `a`).
    pub fn walk(&self, c0: usize, a: Vec3, b: Vec3) -> Walk {
        let m = &self.mesh;
        let nint = m.n_internal_faces();
        let mut c = c0;
        let mut cur = a;
        for _ in 0..WALK_CAP {
            match self.exit(c, cur, b) {
                None => return Walk::Inside(c),
                Some((t, f)) => {
                    let q = add(cur, scale(sub(b, cur), t));
                    if f < nint {
                        c = if m.owner()[f] == c {
                            m.neighbour()[f]
                        } else {
                            m.owner()[f]
                        };
                        cur = q;
                    } else {
                        return Walk::Boundary {
                            cell: c,
                            face: f,
                            point: q,
                        };
                    }
                }
            }
        }
        Walk::Lost
    }

    /// The face through which the segment `a -> b` leaves cell `c` (outward crossings only, smallest parameter).
    fn exit(&self, c: usize, a: Vec3, b: Vec3) -> Option<(f64, usize)> {
        let m = &self.mesh;
        let pts = m.points();
        let dir = sub(b, a);
        let mut best: Option<(f64, usize)> = None;
        for &f in m.cell_faces(c) {
            let sign = if m.owner()[f] == c { 1.0 } else { -1.0 };
            let ids = m.face_points(f);
            let fa = self.face_avg[f];
            let n = ids.len();
            for i in 0..n {
                let t1 = pts[ids[i]];
                let t2 = pts[ids[(i + 1) % n]];
                let e1 = sub(t1, fa);
                let e2 = sub(t2, fa);
                let n0 = cross(e1, e2);
                let nrm = scale(n0, sign);
                let denom = dot(dir, nrm);
                if denom <= 0.0 {
                    continue;
                }
                let t = dot(sub(fa, a), nrm) / denom;
                if !(-1e-9..=1.0).contains(&t) {
                    continue;
                }
                let t = t.max(0.0);
                let q = add(a, scale(dir, t));
                let w = sub(q, fa);
                // Barycentric coordinates in the triangle plane: w = u e1 + v e2.
                let nn = dot(n0, n0);
                if nn <= 0.0 {
                    continue;
                }
                let u = dot(cross(w, e2), n0) / nn;
                let v = dot(cross(e1, w), n0) / nn;
                if u >= -BARY_TOL && v >= -BARY_TOL && u + v <= 1.0 + BARY_TOL && best.map_or(true, |(bt, _)| t < bt) {
                    best = Some((t, f));
                }
            }
        }
        best
    }

    /// Outward unit normal of a face from its owner (area vector direction).
    pub fn face_normal(&self, f: usize) -> Vec3 {
        let s = self.mesh.face_area_vectors()[f];
        let n = norm(s);
        scale(s, 1.0 / n)
    }
}

fn vol(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    dot(sub(b, a), cross(sub(c, a), sub(d, a)))
}

fn tet_contains(a: Vec3, b: Vec3, c: Vec3, d: Vec3, p: Vec3, tol: f64) -> bool {
    let v0 = vol(a, b, c, d);
    if v0 == 0.0 {
        return false;
    }
    let s = v0.signum();
    let lim = -tol * v0.abs();
    s * vol(p, b, c, d) >= lim && s * vol(a, p, c, d) >= lim && s * vol(a, b, p, d) >= lim && s * vol(a, b, c, p) >= lim
}
