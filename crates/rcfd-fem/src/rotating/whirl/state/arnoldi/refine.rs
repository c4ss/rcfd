//! The refinement of the mode vectors of the sparse whirl state solver (ADR 0442, F-520): each returned mode `(rho, phi)` is
//! refined by inverse iteration on `Q(rho) = K + rho G + rho^2 M` at its own eigenvalue (the iteration of ADR 0416 D4, three
//! solves, the previous iterate as the right-hand side), with a complex LU factor with partial pivoting on the band of the
//! reverse Cuthill-McKee order (D2; `Q(rho)` is neither symmetric nor Hermitian at a complex `rho`) and Gram-Schmidt in the `M`
//! product against the members of a cluster already refined (D3: eigenvalues within `1e-9` relative, closed transitively).

use super::super::INVERSE_ITERATIONS;
use super::{cdiv, cmul, dot, matvec, normalise, not_converged, residual_of, SparseMode};
use crate::error::Result;
use rcfd_linalg::{rcm_order, CsrMatrix};

/// Two returned modes whose eigenvalues are within this distance relative to the larger modulus are one cluster (D3).
const CLUSTER: f64 = 1e-9;

type C = [f64; 2];

fn cabs(z: C) -> f64 {
    z[0].hypot(z[1])
}

/// One row of the band: the columns `lo .. lo + v.len()`.
struct Row {
    lo: usize,
    v: Vec<C>,
}

impl Row {
    fn get(&self, c: usize) -> C {
        if c >= self.lo && c < self.lo + self.v.len() {
            self.v[c - self.lo]
        } else {
            [0.0, 0.0]
        }
    }
    fn hi(&self) -> usize {
        self.lo + self.v.len() - 1
    }
    /// Extends the window to cover the columns `a ..= b`.
    fn cover(&mut self, a: usize, b: usize) {
        let lo = self.lo.min(a);
        let hi = self.hi().max(b);
        if lo == self.lo && hi == self.hi() {
            return;
        }
        let mut v = vec![[0.0, 0.0]; hi - lo + 1];
        v[self.lo - lo..self.lo - lo + self.v.len()].copy_from_slice(&self.v);
        self.lo = lo;
        self.v = v;
    }
}

/// `P Q(rho) = L U` on the RCM order (D2).
struct BandLu {
    /// `order[i]`: the original unknown at position `i`.
    order: Vec<usize>,
    /// The row exchanged with row `j` at step `j`.
    piv: Vec<usize>,
    /// The multipliers `(row, l)` of step `j`.
    mult: Vec<Vec<(usize, C)>>,
    /// The rows of `U` (columns below the diagonal are zero).
    rows: Vec<Row>,
}

impl BandLu {
    fn factor(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, s: C) -> BandLu {
        let n = k.n;
        let mut faces: Vec<(usize, usize)> = Vec::new();
        for a in [k, m, g] {
            for r in 0..n {
                for p in a.row_ptr[r]..a.row_ptr[r + 1] {
                    let c = a.col[p];
                    if c != r {
                        faces.push((r.min(c), r.max(c)));
                    }
                }
            }
        }
        faces.sort_unstable();
        faces.dedup();
        let lower: Vec<usize> = faces.iter().map(|f| f.0).collect();
        let upper: Vec<usize> = faces.iter().map(|f| f.1).collect();
        let order = rcm_order(n, &lower, &upper);
        let mut pos = vec![0; n];
        for (i, &o) in order.iter().enumerate() {
            pos[o] = i;
        }
        // The windows of the permuted rows, each with its diagonal.
        let (mut lo, mut hi): (Vec<usize>, Vec<usize>) = ((0..n).collect(), (0..n).collect());
        for &(a, b) in &faces {
            let (i, j) = (pos[a], pos[b]);
            let (r, c) = (i.min(j), i.max(j));
            hi[r] = hi[r].max(c);
            lo[c] = lo[c].min(r);
        }
        let mut rows: Vec<Row> = (0..n)
            .map(|i| Row {
                lo: lo[i],
                v: vec![[0.0, 0.0]; hi[i] - lo[i] + 1],
            })
            .collect();
        let s2 = cmul(s, s);
        for (a, coef) in [(k, [1.0, 0.0]), (g, s), (m, s2)] {
            for r in 0..n {
                for p in a.row_ptr[r]..a.row_ptr[r + 1] {
                    let (i, j) = (pos[r], pos[a.col[p]]);
                    let row = &mut rows[i];
                    let e = &mut row.v[j - row.lo];
                    e[0] += coef[0] * a.val[p];
                    e[1] += coef[1] * a.val[p];
                }
            }
        }
        let qmax = rows
            .iter()
            .flat_map(|r| r.v.iter())
            .map(|&z| cabs(z))
            .fold(0.0f64, f64::max);
        let tiny = f64::EPSILON * if qmax > 0.0 { qmax } else { 1.0 };
        let kl = (0..n).map(|i| i - rows[i].lo).max().unwrap_or(0);
        let mut piv = vec![0; n];
        let mut mult = Vec::with_capacity(n);
        for j in 0..n {
            let end = n.min(j + kl + 1);
            let mut p = j;
            let mut best = -1.0;
            for (i, row) in rows.iter().enumerate().take(end).skip(j) {
                let v = cabs(row.get(j));
                if v > best {
                    best = v;
                    p = i;
                }
            }
            rows.swap(j, p);
            piv[j] = p;
            rows[j].cover(j, j);
            if cabs(rows[j].get(j)) < tiny {
                let lo = rows[j].lo;
                rows[j].v[j - lo] = [tiny, 0.0];
            }
            let pivot = rows[j].get(j);
            let hj = rows[j].hi();
            let mut mj = Vec::new();
            for i in j + 1..end {
                let a = rows[i].get(j);
                if a == [0.0, 0.0] {
                    continue;
                }
                let f = cdiv(a, pivot);
                let (top, bottom) = rows.split_at_mut(i);
                let (rj, ri) = (&top[j], &mut bottom[0]);
                ri.cover(j, hj);
                let lo = ri.lo;
                for c in j + 1..=hj {
                    let u = cmul(f, rj.get(c));
                    ri.v[c - lo][0] -= u[0];
                    ri.v[c - lo][1] -= u[1];
                }
                ri.v[j - lo] = [0.0, 0.0];
                mj.push((i, f));
            }
            mult.push(mj);
        }
        BandLu { order, piv, mult, rows }
    }

    fn solve(&self, b: &[C]) -> Vec<C> {
        let n = self.order.len();
        let mut x: Vec<C> = self.order.iter().map(|&o| b[o]).collect();
        for j in 0..n {
            x.swap(j, self.piv[j]);
            let xj = x[j];
            for &(i, f) in &self.mult[j] {
                let u = cmul(f, xj);
                x[i][0] -= u[0];
                x[i][1] -= u[1];
            }
        }
        for j in (0..n).rev() {
            let row = &self.rows[j];
            let mut acc = x[j];
            for (c, xc) in x.iter().enumerate().take(row.hi() + 1).skip(j + 1) {
                let u = cmul(row.get(c), *xc);
                acc[0] -= u[0];
                acc[1] -= u[1];
            }
            x[j] = cdiv(acc, row.get(j));
        }
        let mut out = vec![[0.0, 0.0]; n];
        for (i, &o) in self.order.iter().enumerate() {
            out[o] = x[i];
        }
        out
    }
}

/// The cluster label of each mode (the smallest index of its cluster), by `CLUSTER`, closed transitively.
fn clusters(modes: &[SparseMode]) -> Vec<usize> {
    let len = modes.len();
    let mut label: Vec<usize> = (0..len).collect();
    loop {
        let mut changed = false;
        for a in 0..len {
            for b in a + 1..len {
                let (sa, sb) = (modes[a].0, modes[b].0);
                let d = (sa[0] - sb[0]).hypot(sa[1] - sb[1]);
                if d <= CLUSTER * cabs(sa).max(cabs(sb)) && label[a] != label[b] {
                    let l = label[a].min(label[b]);
                    label[a] = l;
                    label[b] = l;
                    changed = true;
                }
            }
        }
        if !changed {
            return label;
        }
    }
}

/// A refined mode vector `(re, im)` with `(M re, M im)`.
type Refined = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);

/// Refines the vector of every mode in place (ADR 0442 D3): `k`, `m` and the skew `g` of the solve; the eigenvalues are not
/// changed, the vectors are normalised as ADR 0428 D3 and their residuals recomputed.
pub(super) fn refine(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, modes: &mut [SparseMode]) -> Result<()> {
    let n = k.n;
    let label = clusters(modes);
    // The refined members by mode index: `(re, im, M re, M im)`.
    let mut done: Vec<Option<Refined>> = vec![None; modes.len()];
    for i in 0..modes.len() {
        let rho = modes[i].0;
        let lu = BandLu::factor(k, m, g, rho);
        let mut x: Vec<C> = (0..n).map(|e| [modes[i].1[e], modes[i].2[e]]).collect();
        let prior: Vec<usize> = (0..i).filter(|&j| label[j] == label[i]).collect();
        for _ in 0..INVERSE_ITERATIONS {
            let mut y = lu.solve(&x);
            for _ in 0..2 {
                for &j in &prior {
                    let (pr, pi, mpr, mpi) = done[j].as_ref().expect("an earlier member is refined");
                    let yr: Vec<f64> = y.iter().map(|z| z[0]).collect();
                    let yi: Vec<f64> = y.iter().map(|z| z[1]).collect();
                    // phi^H M y.
                    let coef = [dot(mpr, &yr) + dot(mpi, &yi), dot(mpr, &yi) - dot(mpi, &yr)];
                    for (e, z) in y.iter_mut().enumerate() {
                        let u = cmul(coef, [pr[e], pi[e]]);
                        z[0] -= u[0];
                        z[1] -= u[1];
                    }
                }
            }
            let nrm = y.iter().map(|z| z[0] * z[0] + z[1] * z[1]).sum::<f64>().sqrt();
            if !(nrm.is_finite() && nrm > 0.0) {
                return Err(not_converged(format!(
                    "the refinement of the mode at s = {} + {} i",
                    rho[0], rho[1]
                )));
            }
            x = y.iter().map(|z| [z[0] / nrm, z[1] / nrm]).collect();
        }
        let mut re: Vec<f64> = x.iter().map(|z| z[0]).collect();
        let mut im: Vec<f64> = x.iter().map(|z| z[1]).collect();
        normalise(m, &mut re, &mut im);
        if !re.iter().chain(&im).all(|v| v.is_finite()) {
            return Err(not_converged(format!(
                "the refinement of the mode at s = {} + {} i",
                rho[0], rho[1]
            )));
        }
        let res = residual_of(k, m, g, rho, &re, &im);
        let (mr, mi) = (matvec(m, &re), matvec(m, &im));
        done[i] = Some((re.clone(), im.clone(), mr, mi));
        modes[i].1 = re;
        modes[i].2 = im;
        modes[i].3 = res;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        (*s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    /// The `4 x 4` problem `P1` of ADR 0442 K2, dense row-major `(k, m, g)`.
    fn p1() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = 4;
        let mut s = 0x5eed_0520u64;
        let (mut k, mut m, mut g) = (vec![0.0; n * n], vec![0.0; n * n], vec![0.0; n * n]);
        for i in 0..n {
            k[i * n + i] = 2.0 * lcg(&mut s) + 0.3;
            m[i * n + i] = 1.0 + 0.2 * lcg(&mut s);
            for j in i + 1..n {
                let b = lcg(&mut s);
                k[i * n + j] = b;
                k[j * n + i] = b;
                let c = 0.1 * lcg(&mut s);
                m[i * n + j] = c;
                m[j * n + i] = c;
                let v = 2.0 * lcg(&mut s);
                g[i * n + j] = v;
                g[j * n + i] = -v;
            }
        }
        (k, m, g)
    }

    /// `blockdiag(a, b)` of two dense `4 x 4` as CSR `8 x 8`.
    fn blocks(a: &[f64], b: &[f64]) -> CsrMatrix {
        let mut t = Vec::new();
        for (off, x) in [(0, a), (4, b)] {
            for p in 0..16 {
                if x[p] != 0.0 {
                    t.push((off + p / 4, off + p % 4, x[p]));
                }
            }
        }
        CsrMatrix::from_triplets(8, t)
    }

    fn reported(k: &[f64], m: &[f64], g: &[f64]) -> Vec<C> {
        let d = super::super::super::gyroscopic_state_eigen_dense(k, m, g, 4).unwrap();
        d.eigenvalues
            .iter()
            .copied()
            .filter(|s| s[1] > 0.0 || (s[1] == 0.0 && s[0] >= 0.0))
            .collect()
    }

    fn start(s: C) -> SparseMode {
        let re: Vec<f64> = (0..8).map(|i| 1.0 + ((7 * i + 3) % 11) as f64 / 11.0).collect();
        let im: Vec<f64> = (0..8).map(|i| ((5 * i + 1) % 13) as f64 / 13.0 - 0.5).collect();
        (s, re, im, f64::NAN)
    }

    /// `|phi_a^H M phi_b|`.
    fn mdot(m: &CsrMatrix, a: &SparseMode, b: &SparseMode) -> f64 {
        let (mr, mi) = (matvec(m, &b.1), matvec(m, &b.2));
        let re = dot(&a.1, &mr) + dot(&a.2, &mi);
        let im = dot(&a.1, &mi) - dot(&a.2, &mr);
        re.hypot(im)
    }

    /// ADR 0442 K2a and K2b: an exact double (the same start vector, the same eigenvalue: kept apart by the Gram-Schmidt of the
    /// cluster) and a near pair at a relative distance of order 5e-4 (kept apart by the shifts alone).
    #[test]
    fn k2a_k2b_clusters() {
        let (k, m, g) = p1();
        let ev = reported(&k, &m, &g);
        println!("K2: P1 reported eigenvalues {ev:?}");
        let (kc, mc, gc) = (blocks(&k, &k), blocks(&m, &m), blocks(&g, &g));
        let mut modes: Vec<SparseMode> = ev.iter().flat_map(|&s| [start(s), start(s)]).collect();
        refine(&kc, &mc, &gc, &mut modes).unwrap();
        let (mut res_a, mut dot_a): (f64, f64) = (0.0, 0.0);
        for p in modes.chunks(2) {
            res_a = res_a.max(p[0].3).max(p[1].3);
            dot_a = dot_a.max(mdot(&mc, &p[0], &p[1]));
        }
        println!("K2a: residual {res_a:.3e}, |phi_a^H M phi_b| {dot_a:.3e}");

        let k2: Vec<f64> = k.iter().map(|v| (1.0 + 1e-3) * v).collect();
        let ev2 = reported(&k2, &m, &g);
        println!("K2b: P1' reported eigenvalues {ev2:?}");
        let (kb, mb, gb) = (blocks(&k, &k2), blocks(&m, &m), blocks(&g, &g));
        let mut modes: Vec<SparseMode> = Vec::new();
        let mut gap: f64 = f64::INFINITY;
        for &s in &ev {
            let t = *ev2
                .iter()
                .min_by(|a, b| {
                    (a[0] - s[0])
                        .hypot(a[1] - s[1])
                        .total_cmp(&(b[0] - s[0]).hypot(b[1] - s[1]))
                })
                .unwrap();
            gap = gap.min((t[0] - s[0]).hypot(t[1] - s[1]) / cabs(s).max(cabs(t)));
            modes.push(start(s));
            modes.push(start(t));
        }
        refine(&kb, &mb, &gb, &mut modes).unwrap();
        let (mut res_b, mut dot_b): (f64, f64) = (0.0, 0.0);
        for p in modes.chunks(2) {
            res_b = res_b.max(p[0].3).max(p[1].3);
            dot_b = dot_b.max(mdot(&mb, &p[0], &p[1]));
        }
        println!("K2b: smallest relative distance {gap:.3e}, residual {res_b:.3e}, |phi^H M phi'| {dot_b:.3e}");
        assert!(res_a <= 1e-8, "K2a residual {res_a:e}");
        assert!(dot_a <= 1e-6, "K2a M product {dot_a:e}");
        assert!(gap > CLUSTER, "K2b pair inside the cluster radius: {gap:e}");
        assert!(res_b <= 1e-8, "K2b residual {res_b:e}");
        assert!(dot_b <= 1e-6, "K2b M product {dot_b:e}");
    }
}
