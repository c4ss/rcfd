//! Transfer of cell-centred fields from one mesh to another (F-115).
//!
//! Each target cell takes the nearest source cell centre (bucket search) and extrapolates with
//! the least-squares gradient of that source cell, `phi = phi_c + grad(phi)_c . (x - x_c)`; the
//! result is clipped to the range of the source cell and its face neighbours (no new extrema).
//! Two modes are offered:
//!
//! * [`InterpMode::NonConservative`]: the values as above.
//! * [`InterpMode::Conservative`]: the values as above, then one uniform shift so that the
//!   volume-averaged value over the target mesh equals the one over the source mesh (the volume
//!   integrals agree whenever both meshes fill the same volume). This is an integral-matched
//!   correction, not a geometric cell-overlap remap; it does not conserve sub-regions.
//!
//! The target cell is found by nearest centre, which is the containing cell for meshes of
//! similar resolution and exact for identical meshes; it is not a point-in-cell search.

use rcfd_core::vec3::{dot, norm, sub, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpMode {
    /// Gradient-corrected nearest-cell values.
    NonConservative,
    /// As above plus a uniform shift that matches the volume average (integral) of the source.
    Conservative,
}

impl InterpMode {
    /// Parse `conservative` / `nonconservative` (also `non-conservative`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "conservative" => Some(InterpMode::Conservative),
            "nonconservative" | "linear" => Some(InterpMode::NonConservative),
            _ => None,
        }
    }
}

struct Buckets {
    lo: Vec3,
    h: f64,
    n: [usize; 3],
    cells: Vec<Vec<u32>>,
}

impl Buckets {
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
        // dimension count: axes whose extent is non-negligible
        let live: Vec<usize> = (0..3).filter(|&k| ext[k] > 1e-9 * maxe).collect();
        let vol: f64 = live.iter().map(|&k| ext[k]).product::<f64>().max(1e-300);
        let h = (vol / (pts.len().max(1) as f64 / 2.0))
            .powf(1.0 / live.len().max(1) as f64)
            .max(1e-300);
        let mut n = [1usize; 3];
        for &k in &live {
            n[k] = ((ext[k] / h).ceil() as usize).clamp(1, 1024);
        }
        let mut b = Buckets {
            lo,
            h,
            n,
            cells: vec![Vec::new(); n[0] * n[1] * n[2]],
        };
        for (i, p) in pts.iter().enumerate() {
            let c = b.coord(*p);
            let id = b.id(c);
            b.cells[id].push(i as u32);
        }
        b
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

    fn nearest(&self, pts: &[Vec3], q: Vec3) -> usize {
        let c = self.coord(q);
        let mut best = (f64::INFINITY, 0usize);
        let rmax = *self.n.iter().max().unwrap_or(&1);
        for r in 0..=rmax {
            let ri = r as i64;
            for dz in -ri..=ri {
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != ri {
                            continue;
                        }
                        let cc = [c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz];
                        if (0..3).any(|k| cc[k] < 0 || cc[k] >= self.n[k] as i64) {
                            continue;
                        }
                        let id = self.id([cc[0] as usize, cc[1] as usize, cc[2] as usize]);
                        for &i in &self.cells[id] {
                            let d = norm(sub(pts[i as usize], q));
                            if d < best.0 {
                                best = (d, i as usize);
                            }
                        }
                    }
                }
            }
            // everything not yet visited is at least r*h away from the query point
            if best.0.is_finite() && best.0 <= r as f64 * self.h {
                break;
            }
        }
        best.1
    }
}

/// Precomputed source-to-target mapping; reuse it for all fields of a solution.
pub struct Interpolator<'a> {
    src: &'a Mesh,
    tgt: &'a Mesh,
    nearest: Vec<usize>,
    /// Least-squares normal-matrix inverse per source cell.
    ls_inv: Vec<[[f64; 3]; 3]>,
    nbrs: Vec<Vec<usize>>,
}

fn inv3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    // Directions without any neighbour spread (the empty direction of a 2D mesh) get a unit
    // diagonal so the system is invertible; their right-hand side is zero, so the gradient
    // component is zero. Other directions are left untouched (exact for linear fields).
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

impl<'a> Interpolator<'a> {
    /// Build the mapping from `src` cells to `tgt` cells.
    pub fn new(src: &'a Mesh, tgt: &'a Mesh) -> Result<Self> {
        if src.n_cells() == 0 || tgt.n_cells() == 0 {
            return Err(Error::Mesh("interpolation needs non-empty meshes".into()));
        }
        let sc = src.cell_centres();
        let buckets = Buckets::new(sc);
        let nearest = tgt.cell_centres().iter().map(|&q| buckets.nearest(sc, q)).collect();
        let mut nbrs = vec![Vec::new(); src.n_cells()];
        for f in 0..src.n_internal_faces() {
            let (o, n) = (src.owner()[f], src.neighbour()[f]);
            nbrs[o].push(n);
            nbrs[n].push(o);
        }
        let ls_inv = (0..src.n_cells())
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
        Ok(Interpolator {
            src,
            tgt,
            nearest,
            ls_inv,
            nbrs,
        })
    }

    /// Source cell used for each target cell.
    pub fn nearest_cells(&self) -> &[usize] {
        &self.nearest
    }

    /// Interpolate one cell field.
    pub fn scalar(&self, values: &[f64], mode: InterpMode) -> Result<Vec<f64>> {
        if values.len() != self.src.n_cells() {
            return Err(Error::Mesh(format!(
                "source field has {} values, source mesh has {} cells",
                values.len(),
                self.src.n_cells()
            )));
        }
        let sc = self.src.cell_centres();
        let tc = self.tgt.cell_centres();
        let mut grad = vec![[0.0; 3]; sc.len()];
        for (c, g) in grad.iter_mut().enumerate() {
            let mut rhs = [0.0; 3];
            for &n in &self.nbrs[c] {
                let d = sub(sc[n], sc[c]);
                let dv = values[n] - values[c];
                for k in 0..3 {
                    rhs[k] += d[k] * dv;
                }
            }
            let m = &self.ls_inv[c];
            *g = [dot(m[0], rhs), dot(m[1], rhs), dot(m[2], rhs)];
        }
        let mut out: Vec<f64> = (0..tc.len())
            .map(|i| {
                let s = self.nearest[i];
                let v = values[s] + dot(grad[s], sub(tc[i], sc[s]));
                let (mut lo, mut hi) = (values[s], values[s]);
                for &n in &self.nbrs[s] {
                    lo = lo.min(values[n]);
                    hi = hi.max(values[n]);
                }
                v.clamp(lo, hi)
            })
            .collect();
        if mode == InterpMode::Conservative {
            let (vs, vt) = (self.src.cell_volumes(), self.tgt.cell_volumes());
            let tv_s: f64 = vs.iter().sum();
            let tv_t: f64 = vt.iter().sum();
            let mean_s = vs.iter().zip(values).map(|(v, x)| v * x).sum::<f64>() / tv_s;
            let mean_t = vt.iter().zip(&out).map(|(v, x)| v * x).sum::<f64>() / tv_t;
            let shift = mean_s - mean_t;
            for x in &mut out {
                *x += shift;
            }
        }
        Ok(out)
    }

    /// Interpolate a vector field component by component.
    pub fn vector(&self, values: &[Vec3], mode: InterpMode) -> Result<Vec<Vec3>> {
        let mut comps = Vec::with_capacity(3);
        for k in 0..3 {
            let c: Vec<f64> = values.iter().map(|v| v[k]).collect();
            comps.push(self.scalar(&c, mode)?);
        }
        Ok((0..self.tgt.n_cells())
            .map(|i| [comps[0][i], comps[1][i], comps[2][i]])
            .collect())
    }
}
