//! Cell-field transfer between meshes (F-115, wave 19C, ADR 0031 D2).
//!
//! Methods ([`RemapMethod`]):
//!
//! * `nearest`: value of the nearest source cell centre (first order).
//! * `inverse_distance`: convex combination of the [`K_NEAREST`] nearest source centres with
//!   weights `1/d^2` (bounded, first order).
//! * `least_squares`: weighted (`1/d^2`) linear least-squares fit over the [`K_NEAREST`] nearest
//!   source centres evaluated at the target centre (exact for linear fields, second order).
//!   Directions without any spread of the neighbours (the thin direction of a 2D mesh) are
//!   dropped from the fit. A target centre outside the bounding box of the source mesh takes the
//!   nearest value (the fit extrapolates linearly over the boundary half cells inside the box).
//! * `gradient`: the first-slice mode of [`crate::interpolate`] (nearest cell plus its gradient,
//!   clipped to the neighbour range).
//! * `conservative`: `gradient` plus one uniform shift matching the volume average
//!   ([`crate::interpolate::InterpMode::Conservative`]).
//! * `conservative_overlap`: for meshes of axis-aligned box cells, the overlap-volume weighted mean
//!   of the linear source reconstruction `s_i + grad_i . (c_ij - x_i)`. The integral is conserved
//!   to rounding when both meshes fill the same region, a linear field is reproduced exactly, and
//!   the method is second order. The part of a target cell outside the source takes the nearest
//!   source value. A direction in which both meshes are a single cell thick is ignored in the
//!   overlap (areas instead of volumes), so 2D meshes of different depth can be paired.
//!
//! Identical meshes (equal content hash) are copied bit for bit for every method.

use crate::interpolate::{InterpMode, Interpolator};
use rcfd_core::vec3::{dot, sub, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Number of source cells used by `inverse_distance` and `least_squares`.
pub const K_NEAREST: usize = 10;

/// Field transfer method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapMethod {
    /// Nearest source cell value.
    Nearest,
    /// Inverse-distance weighting over the nearest source cells.
    InverseDistance,
    /// Local weighted linear least-squares fit.
    LeastSquares,
    /// Nearest cell plus clipped gradient (first slice, non-conservative).
    Gradient,
    /// `Gradient` plus a uniform shift matching the volume average.
    Conservative,
    /// Overlap-volume remap with linear reconstruction (axis-aligned box cells).
    ConservativeOverlap,
}

impl RemapMethod {
    /// All methods, in the order of the documentation.
    pub const ALL: [RemapMethod; 6] = [
        RemapMethod::Gradient,
        RemapMethod::Nearest,
        RemapMethod::InverseDistance,
        RemapMethod::LeastSquares,
        RemapMethod::Conservative,
        RemapMethod::ConservativeOverlap,
    ];

    /// Parse a method name (`nonconservative` is accepted for `gradient`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "gradient" | "nonconservative" | "non_conservative" | "linear" => Some(Self::Gradient),
            "nearest" => Some(Self::Nearest),
            "inverse_distance" | "idw" => Some(Self::InverseDistance),
            "least_squares" => Some(Self::LeastSquares),
            "conservative" => Some(Self::Conservative),
            "conservative_overlap" | "overlap" => Some(Self::ConservativeOverlap),
            _ => None,
        }
    }

    /// The name used in the case tree and the CLI.
    pub fn name(self) -> &'static str {
        match self {
            Self::Gradient => "gradient",
            Self::Nearest => "nearest",
            Self::InverseDistance => "inverse_distance",
            Self::LeastSquares => "least_squares",
            Self::Conservative => "conservative",
            Self::ConservativeOverlap => "conservative_overlap",
        }
    }
}

/// Uniform bucket grid over points, for k-nearest queries.
struct Grid {
    lo: Vec3,
    h: f64,
    n: [usize; 3],
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(pts: &[Vec3]) -> Self {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let ext: Vec<f64> = (0..3).map(|k| hi[k] - lo[k]).collect();
        let maxe = ext.iter().cloned().fold(0.0, f64::max).max(1e-300);
        let live: Vec<usize> = (0..3).filter(|&k| ext[k] > 1e-9 * maxe).collect();
        let vol: f64 = live.iter().map(|&k| ext[k]).product::<f64>().max(1e-300);
        let h = (vol / (pts.len().max(1) as f64 / 2.0))
            .powf(1.0 / live.len().max(1) as f64)
            .max(1e-300);
        let mut n = [1usize; 3];
        for &k in &live {
            n[k] = ((ext[k] / h).ceil() as usize).clamp(1, 1024);
        }
        let mut g = Grid {
            lo,
            h,
            n,
            cells: vec![Vec::new(); n[0] * n[1] * n[2]],
        };
        for (i, p) in pts.iter().enumerate() {
            let id = g.id(g.coord(*p));
            g.cells[id].push(i as u32);
        }
        g
    }

    fn coord(&self, p: Vec3) -> [usize; 3] {
        let mut c = [0usize; 3];
        for k in 0..3 {
            let v = ((p[k] - self.lo[k]) / self.h).floor();
            c[k] = if v.is_nan() || v < 0.0 {
                0
            } else {
                (v as usize).min(self.n[k] - 1)
            };
        }
        c
    }

    fn id(&self, c: [usize; 3]) -> usize {
        (c[2] * self.n[1] + c[1]) * self.n[0] + c[0]
    }

    /// The `k` nearest points of `q`, nearest first (ties by index).
    fn knn(&self, pts: &[Vec3], q: Vec3, k: usize) -> Vec<(f64, usize)> {
        let k = k.min(pts.len()).max(1);
        let c = self.coord(q);
        let rmax = *self.n.iter().max().unwrap_or(&1);
        let mut best: Vec<(f64, usize)> = Vec::new();
        for r in 0..=rmax {
            let ri = r as i64;
            for dz in -ri..=ri {
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != ri {
                            continue;
                        }
                        let cc = [c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz];
                        if (0..3).any(|a| cc[a] < 0 || cc[a] >= self.n[a] as i64) {
                            continue;
                        }
                        let id = self.id([cc[0] as usize, cc[1] as usize, cc[2] as usize]);
                        for &i in &self.cells[id] {
                            let d = sub(pts[i as usize], q);
                            best.push((dot(d, d), i as usize));
                        }
                    }
                }
            }
            best.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            best.truncate(k);
            // all points not yet visited are at least r*h away
            if best.len() == k && best[k - 1].0.sqrt() <= r as f64 * self.h {
                break;
            }
        }
        best
    }
}

/// Axis-aligned box of a cell.
fn cell_boxes(mesh: &Mesh) -> Vec<([f64; 3], [f64; 3])> {
    let pts = mesh.points();
    (0..mesh.n_cells())
        .map(|c| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for &f in mesh.cell_faces(c) {
                for &p in mesh.face_points(f) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(pts[p][k]);
                        hi[k] = hi[k].max(pts[p][k]);
                    }
                }
            }
            (lo, hi)
        })
        .collect()
}

/// True when every cell of `mesh` is an axis-aligned box.
fn all_boxes(mesh: &Mesh, boxes: &[([f64; 3], [f64; 3])]) -> bool {
    let (vol, cen) = (mesh.cell_volumes(), mesh.cell_centres());
    boxes.iter().enumerate().all(|(c, (lo, hi))| {
        let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let bv = ext[0] * ext[1] * ext[2];
        let ok_v = (vol[c] - bv).abs() <= 1e-10 * bv.abs().max(1e-300);
        let ok_c = (0..3).all(|k| (cen[c][k] - 0.5 * (lo[k] + hi[k])).abs() <= 1e-10 * ext[k].max(1e-300));
        ok_v && ok_c
    })
}

/// A direction is thin when the mesh is one cell thick in it (all cells share one interval).
fn thin_dims(boxes: &[([f64; 3], [f64; 3])]) -> [bool; 3] {
    let mut t = [true; 3];
    let (l0, h0) = boxes[0];
    for (lo, hi) in boxes {
        for k in 0..3 {
            let tol = 1e-9 * (h0[k] - l0[k]).abs().max(1e-300);
            if (lo[k] - l0[k]).abs() > tol || (hi[k] - h0[k]).abs() > tol {
                t[k] = false;
            }
        }
    }
    t
}

fn inv3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let tr = (m[0][0] + m[1][1] + m[2][2]).max(1e-300);
    let mut a = m;
    for k in 0..3 {
        if m[k][k] <= 1e-9 * tr {
            a[k][k] += tr;
        }
    }
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-300 {
        return [[0.0; 3]; 3];
    }
    let c = |i: usize, j: usize| {
        let (i1, i2) = ((i + 1) % 3, (i + 2) % 3);
        let (j1, j2) = ((j + 1) % 3, (j + 2) % 3);
        a[i1][j1] * a[i2][j2] - a[i1][j2] * a[i2][j1]
    };
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det;
        }
    }
    r
}

/// Solve `m x = rhs` (n x n, row-major, n <= 4) with partial pivoting; `None` if singular.
fn solve_small(m: &mut [[f64; 4]; 4], rhs: &mut [f64; 4], n: usize) -> Option<[f64; 4]> {
    let scale = (0..n).map(|i| m[i][i].abs()).fold(0.0, f64::max).max(1e-300);
    for col in 0..n {
        let piv = (col..n).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[piv][col].abs() < 1e-13 * scale {
            return None;
        }
        m.swap(col, piv);
        rhs.swap(col, piv);
        for r in col + 1..n {
            let f = m[r][col] / m[col][col];
            for c2 in col..n {
                m[r][c2] -= f * m[col][c2];
            }
            rhs[r] -= f * rhs[col];
        }
    }
    let mut x = [0.0; 4];
    for r in (0..n).rev() {
        let mut s = rhs[r];
        for c2 in r + 1..n {
            s -= m[r][c2] * x[c2];
        }
        x[r] = s / m[r][r];
    }
    Some(x)
}

struct Overlap {
    /// Per target cell: (source cell, overlap volume, centroid of the overlap).
    parts: Vec<Vec<(usize, f64, Vec3)>>,
    /// Active-direction volume of every target cell.
    vol: Vec<f64>,
    /// Covered fraction of every target cell.
    covered: Vec<f64>,
    /// Nearest source cell of every target cell (value of the uncovered part).
    nearest: Vec<usize>,
}

enum Plan<'a> {
    Identity,
    /// Linear stencils: value = sum of coefficient * source value.
    Stencil(Vec<Vec<(usize, f64)>>),
    Interp(Interpolator<'a>, InterpMode),
    Overlap(Overlap),
}

/// Precomputed source-to-target map for one pair of meshes; apply it to all fields.
pub struct Remapper<'a> {
    src: &'a Mesh,
    tgt: &'a Mesh,
    method: RemapMethod,
    plan: Plan<'a>,
    nbrs: Vec<Vec<usize>>,
    ls_inv: Vec<[[f64; 3]; 3]>,
}

impl<'a> Remapper<'a> {
    /// Build the map. `conservative_overlap` fails for meshes that are not made of box cells.
    pub fn new(src: &'a Mesh, tgt: &'a Mesh, method: RemapMethod) -> Result<Self> {
        if src.n_cells() == 0 || tgt.n_cells() == 0 {
            return Err(Error::Mesh("field transfer needs non-empty meshes".into()));
        }
        let mut r = Remapper {
            src,
            tgt,
            method,
            plan: Plan::Identity,
            nbrs: Vec::new(),
            ls_inv: Vec::new(),
        };
        if src.n_cells() == tgt.n_cells() && src.content_hash() == tgt.content_hash() {
            return Ok(r);
        }
        let sc = src.cell_centres();
        let tc = tgt.cell_centres();
        match method {
            RemapMethod::Gradient => r.plan = Plan::Interp(Interpolator::new(src, tgt)?, InterpMode::NonConservative),
            RemapMethod::Conservative => r.plan = Plan::Interp(Interpolator::new(src, tgt)?, InterpMode::Conservative),
            RemapMethod::Nearest => {
                let g = Grid::new(sc);
                let st = tc
                    .iter()
                    .map(|&q| vec![(g.knn(sc, q, 1)[0].1, 1.0)])
                    .collect::<Vec<_>>();
                r.plan = Plan::Stencil(st);
            }
            RemapMethod::InverseDistance => {
                let g = Grid::new(sc);
                let st = tc
                    .iter()
                    .map(|&q| {
                        let nn = g.knn(sc, q, K_NEAREST);
                        let h2 = nn.last().map_or(1.0, |x| x.0).max(1e-300);
                        if nn[0].0 <= 1e-24 * h2 {
                            return vec![(nn[0].1, 1.0)];
                        }
                        let w: Vec<f64> = nn.iter().map(|x| 1.0 / x.0).collect();
                        let sum: f64 = w.iter().sum();
                        nn.iter().zip(&w).map(|(x, w)| (x.1, w / sum)).collect()
                    })
                    .collect::<Vec<_>>();
                r.plan = Plan::Stencil(st);
            }
            RemapMethod::LeastSquares => {
                let g = Grid::new(sc);
                // the fit is trusted inside the bounding box of the source MESH (the cells reach it);
                // dimensions in which the source centres have no spread are not tested
                let (mut clo, mut chi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for p in sc {
                    for k in 0..3 {
                        clo[k] = clo[k].min(p[k]);
                        chi[k] = chi[k].max(p[k]);
                    }
                }
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for p in src.points() {
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
                let maxe = (0..3).map(|k| chi[k] - clo[k]).fold(0.0, f64::max).max(1e-300);
                let live: Vec<usize> = (0..3).filter(|&k| chi[k] - clo[k] > 1e-9 * maxe).collect();
                let st = tc
                    .iter()
                    .map(|&q| {
                        let nn = g.knn(sc, q, 1);
                        let nearest = vec![(nn[0].1, 1.0)];
                        let inside = live
                            .iter()
                            .all(|&k| q[k] >= lo[k] - 1e-12 * maxe && q[k] <= hi[k] + 1e-12 * maxe);
                        if !inside {
                            return nearest;
                        }
                        ls_stencil(sc, &g, q, &live).unwrap_or(nearest)
                    })
                    .collect::<Vec<_>>();
                r.plan = Plan::Stencil(st);
            }
            RemapMethod::ConservativeOverlap => {
                r.build_neighbours();
                r.plan = Plan::Overlap(build_overlap(src, tgt)?);
            }
        }
        Ok(r)
    }

    fn build_neighbours(&mut self) {
        let sc = self.src.cell_centres();
        let mut nbrs = vec![Vec::new(); self.src.n_cells()];
        for f in 0..self.src.n_internal_faces() {
            let (o, n) = (self.src.owner()[f], self.src.neighbour()[f]);
            nbrs[o].push(n);
            nbrs[n].push(o);
        }
        self.ls_inv = (0..self.src.n_cells())
            .map(|c| {
                let mut m = [[0.0; 3]; 3];
                for &n in &nbrs[c] {
                    let d = sub(sc[n], sc[c]);
                    for i in 0..3 {
                        for j in 0..3 {
                            m[i][j] += d[i] * d[j];
                        }
                    }
                }
                inv3(m)
            })
            .collect();
        self.nbrs = nbrs;
    }

    /// The method this map was built with.
    pub fn method(&self) -> RemapMethod {
        self.method
    }

    /// True when the two meshes are identical and the map is a copy.
    pub fn is_identity(&self) -> bool {
        matches!(self.plan, Plan::Identity)
    }

    /// Smallest covered fraction of a target cell by the source (1 for all methods but
    /// `conservative_overlap`, whose value it reports).
    pub fn min_coverage(&self) -> f64 {
        match &self.plan {
            Plan::Overlap(o) => o.covered.iter().cloned().fold(1.0, f64::min),
            _ => 1.0,
        }
    }

    /// Transfer one cell field.
    pub fn scalar(&self, values: &[f64]) -> Result<Vec<f64>> {
        if values.len() != self.src.n_cells() {
            return Err(Error::Mesh(format!(
                "source field has {} values, source mesh has {} cells",
                values.len(),
                self.src.n_cells()
            )));
        }
        Ok(match &self.plan {
            Plan::Identity => values.to_vec(),
            Plan::Stencil(st) => st
                .iter()
                .map(|s| s.iter().map(|&(i, w)| w * values[i]).sum::<f64>())
                .collect(),
            Plan::Interp(ip, mode) => ip.scalar(values, *mode)?,
            Plan::Overlap(o) => self.apply_overlap(o, values),
        })
    }

    /// Transfer a vector field component by component.
    pub fn vector(&self, values: &[Vec3]) -> Result<Vec<Vec3>> {
        let mut comps = Vec::with_capacity(3);
        for k in 0..3 {
            let c: Vec<f64> = values.iter().map(|v| v[k]).collect();
            comps.push(self.scalar(&c)?);
        }
        Ok((0..self.tgt.n_cells())
            .map(|i| [comps[0][i], comps[1][i], comps[2][i]])
            .collect())
    }

    fn apply_overlap(&self, o: &Overlap, values: &[f64]) -> Vec<f64> {
        let sc = self.src.cell_centres();
        let grad: Vec<Vec3> = (0..sc.len())
            .map(|c| {
                let mut rhs = [0.0; 3];
                for &n in &self.nbrs[c] {
                    let d = sub(sc[n], sc[c]);
                    let dv = values[n] - values[c];
                    for k in 0..3 {
                        rhs[k] += d[k] * dv;
                    }
                }
                let m = &self.ls_inv[c];
                [dot(m[0], rhs), dot(m[1], rhs), dot(m[2], rhs)]
            })
            .collect();
        (0..self.tgt.n_cells())
            .map(|j| {
                let mut acc = 0.0;
                let mut vsum = 0.0;
                for &(i, v, c) in &o.parts[j] {
                    acc += v * (values[i] + dot(grad[i], sub(c, sc[i])));
                    vsum += v;
                }
                if o.covered[j] >= 1.0 - 1e-10 {
                    acc / vsum
                } else {
                    let unc = (o.vol[j] - vsum).max(0.0);
                    (acc + unc * values[o.nearest[j]]) / (vsum + unc)
                }
            })
            .collect()
    }
}

/// Stencil coefficients of the weighted linear fit evaluated at `q`.
fn ls_stencil(sc: &[Vec3], g: &Grid, q: Vec3, live: &[usize]) -> Option<Vec<(usize, f64)>> {
    let nn = g.knn(sc, q, K_NEAREST);
    let m = 1 + live.len();
    if nn.len() < m {
        return None;
    }
    let hs = nn.last()?.0.sqrt().max(1e-300);
    let phi = |i: usize| -> [f64; 4] {
        let mut p = [0.0; 4];
        p[0] = 1.0;
        for (a, &k) in live.iter().enumerate() {
            p[1 + a] = (sc[i][k] - q[k]) / hs;
        }
        p
    };
    let w: Vec<f64> = nn.iter().map(|x| 1.0 / (x.0 / (hs * hs) + 1e-12)).collect();
    let mut mat = [[0.0; 4]; 4];
    for (idx, &(_, i)) in nn.iter().enumerate() {
        let p = phi(i);
        for a in 0..m {
            for b in 0..m {
                mat[a][b] += w[idx] * p[a] * p[b];
            }
        }
    }
    let mut e0 = [0.0; 4];
    e0[0] = 1.0;
    let v = solve_small(&mut mat, &mut e0, m)?;
    Some(
        nn.iter()
            .enumerate()
            .map(|(idx, &(_, i))| {
                let p = phi(i);
                let c: f64 = (0..m).map(|a| v[a] * p[a]).sum();
                (i, w[idx] * c)
            })
            .collect(),
    )
}

fn build_overlap(src: &Mesh, tgt: &Mesh) -> Result<Overlap> {
    let (sb, tb) = (cell_boxes(src), cell_boxes(tgt));
    if !all_boxes(src, &sb) || !all_boxes(tgt, &tb) {
        return Err(Error::Mesh(
            "conservative_overlap needs meshes whose cells are all axis-aligned boxes; use method conservative".into(),
        ));
    }
    let (ts, tt) = (thin_dims(&sb), thin_dims(&tb));
    let ignore = [ts[0] && tt[0], ts[1] && tt[1], ts[2] && tt[2]];
    // bucket grid over the source boxes
    let centres: Vec<Vec3> = sb
        .iter()
        .map(|(lo, hi)| [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])])
        .collect();
    let grid = Grid::new(&centres);
    let max_half = sb
        .iter()
        .map(|(lo, hi)| (0..3).map(|k| 0.5 * (hi[k] - lo[k])).fold(0.0, f64::max))
        .fold(0.0, f64::max);
    let sc = src.cell_centres();
    let tcen = tgt.cell_centres();
    let mut parts = Vec::with_capacity(tgt.n_cells());
    let mut vol = Vec::with_capacity(tgt.n_cells());
    let mut covered = Vec::with_capacity(tgt.n_cells());
    let mut nearest = Vec::with_capacity(tgt.n_cells());
    for (j, (tlo, thi)) in tb.iter().enumerate() {
        let tv: f64 = (0..3).filter(|&k| !ignore[k]).map(|k| thi[k] - tlo[k]).product();
        // candidate source cells: centres within the target box grown by the largest source half-extent
        let mut cand: Vec<usize> = Vec::new();
        let (mut glo, mut ghi) = (grid.coord(*tlo), grid.coord(*thi));
        for k in 0..3 {
            let pad = ((max_half / grid.h).ceil() as usize) + 1;
            glo[k] = glo[k].saturating_sub(pad);
            ghi[k] = (ghi[k] + pad).min(grid.n[k] - 1);
        }
        for z in glo[2]..=ghi[2] {
            for y in glo[1]..=ghi[1] {
                for x in glo[0]..=ghi[0] {
                    cand.extend(grid.cells[grid.id([x, y, z])].iter().map(|&i| i as usize));
                }
            }
        }
        cand.sort_unstable();
        let mut list = Vec::new();
        let mut vsum = 0.0;
        for i in cand {
            let (slo, shi) = sb[i];
            let mut v = 1.0;
            let mut cen = [0.0; 3];
            let mut hit = true;
            for k in 0..3 {
                if ignore[k] {
                    cen[k] = tcen[j][k];
                    continue;
                }
                let (lo, hi) = (slo[k].max(tlo[k]), shi[k].min(thi[k]));
                if hi - lo <= 0.0 {
                    hit = false;
                    break;
                }
                v *= hi - lo;
                cen[k] = 0.5 * (lo + hi);
            }
            if hit && v > 0.0 {
                // the centroid of the ignored direction is the source centre line (no gradient offset there)
                for k in 0..3 {
                    if ignore[k] {
                        cen[k] = sc[i][k];
                    }
                }
                list.push((i, v, cen));
                vsum += v;
            }
        }
        let nn = if list.is_empty() || vsum < (1.0 - 1e-10) * tv {
            grid.knn(&centres, tcen[j], 1)[0].1
        } else {
            0
        };
        vol.push(tv);
        covered.push((vsum / tv).min(1.0));
        nearest.push(nn);
        parts.push(list);
    }
    Ok(Overlap {
        parts,
        vol,
        covered,
        nearest,
    })
}
