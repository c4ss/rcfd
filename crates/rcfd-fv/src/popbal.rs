//! Population balance for a dispersed population of particles of volume `v` (F-228, ADR 0041).
//!
//! Library scope: a well-mixed (0-D) population with coalescence and breakup, solved by
//!
//! * discrete bins with the fixed-pivot discretisation of Kumar and Ramkrishna ([`BinModel`]); the scheme conserves the
//!   total volume to round-off and, away from the grid ends, the event number;
//! * the quadrature method of moments with Wheeler's algorithm ([`invert_moments`], [`MomentModel`]).
//!
//! [`PopBalTransport`] carries the bin number densities with the existing scalar transport on a single-phase carrier flux
//! and adds the 0-D kernels as cell sources by Lie splitting. It is one-way: there is no coupling to the carrier flow, no
//! interphase momentum or mass exchange and no slip. The formulas are stated in `docs/adr/0041-wave23c-population-balance.md`.

use crate::ddt::{History, TimeScheme};
use crate::scalar::ScalarTransport;
use std::f64::consts::PI;
use std::fmt;

/// Errors of the population-balance toolkit; the text always starts with `popbal:` and names the failing item.
#[derive(Debug, Clone, PartialEq)]
pub enum PopBalError {
    /// The moment set has no positive quadrature; `index` is the highest moment index of the failing condition.
    NotRealizable {
        /// Highest moment index involved in the failing condition.
        index: usize,
        /// Explanation (values, time and stage when raised inside an integration).
        detail: String,
    },
    /// Invalid input (grid, kernel parameter, sizes).
    Invalid(String),
}

impl fmt::Display for PopBalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PopBalError::NotRealizable { index, detail } => {
                write!(f, "popbal: moment set not realizable at moment index {index}: {detail}")
            }
            PopBalError::Invalid(s) => write!(f, "popbal: {s}"),
        }
    }
}

impl std::error::Error for PopBalError {}

type Result<T> = std::result::Result<T, PopBalError>;

/// Coalescence kernel `b(u, v)` (volume of carrier per particle pair and second).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoalKernel {
    /// `K`.
    Constant(f64),
    /// `B (u + v)`.
    Sum(f64),
    /// `C u v`.
    Product(f64),
    /// Continuum-regime Brownian kernel `(2 kT/(3 mu)) (u^(1/3)+v^(1/3)) (u^(-1/3)+v^(-1/3))`; the parameter is `kT/mu`.
    Brownian {
        /// `k_B T / mu` (m^3/s).
        kt_over_mu: f64,
    },
    /// Laminar shear (Smoluchowski) `(4/3) gdot (r_u + r_v)^3`.
    Shear {
        /// Shear rate (1/s).
        gamma_dot: f64,
    },
    /// Turbulent shear (Saffman and Turner) `sqrt(8 pi/15) sqrt(eps/nu) (r_u + r_v)^3`.
    Turbulent {
        /// Dissipation rate (m^2/s^3).
        epsilon: f64,
        /// Kinematic viscosity (m^2/s).
        nu: f64,
    },
}

fn radius(v: f64) -> f64 {
    (3.0 * v / (4.0 * PI)).cbrt()
}

impl CoalKernel {
    /// Kernel value for particle volumes `u`, `v`.
    pub fn eval(&self, u: f64, v: f64) -> f64 {
        match *self {
            CoalKernel::Constant(k) => k,
            CoalKernel::Sum(b) => b * (u + v),
            CoalKernel::Product(c) => c * u * v,
            CoalKernel::Brownian { kt_over_mu } => {
                let (a, b) = (u.cbrt(), v.cbrt());
                2.0 * kt_over_mu / 3.0 * (a + b) * (1.0 / a + 1.0 / b)
            }
            CoalKernel::Shear { gamma_dot } => 4.0 / 3.0 * gamma_dot * (radius(u) + radius(v)).powi(3),
            CoalKernel::Turbulent { epsilon, nu } => {
                (8.0 * PI / 15.0).sqrt() * (epsilon / nu).sqrt() * (radius(u) + radius(v)).powi(3)
            }
        }
    }
}

/// Breakup rate `g(x)` (1/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakRate {
    /// `B`.
    Constant(f64),
    /// `B x^alpha`.
    Power {
        /// Prefactor.
        b: f64,
        /// Exponent.
        alpha: f64,
    },
}

impl BreakRate {
    /// Rate of a particle of volume `x`.
    pub fn eval(&self, x: f64) -> f64 {
        match *self {
            BreakRate::Constant(b) => b,
            BreakRate::Power { b, alpha } => b * x.powf(alpha),
        }
    }
}

/// Daughter-size distribution of a binary breakup (`nu = 2` daughters, volume conserved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Daughters {
    /// Two daughters of volume `x/2`.
    BinaryEqual,
    /// Uniform density `2/x` on `(0, x)`.
    Uniform,
    /// Symmetric beta density of integer order `q <= 5`: `2 (2q+1)!/(q!)^2 v^q (x-v)^q / x^(2q+1)` (`q = 0` is uniform).
    SymBeta(u32),
}

fn factorial(n: u32) -> f64 {
    (1..=n).map(f64::from).product()
}

impl Daughters {
    /// `mb_k(x) / x^k` with `mb_k(x) = int v^k nu p(v|x) dv` (ratio independent of `x`).
    pub fn moment_factor(&self, k: u32) -> f64 {
        match *self {
            Daughters::BinaryEqual => 2.0 * 0.5f64.powi(k as i32),
            Daughters::Uniform => 2.0 / (f64::from(k) + 1.0),
            Daughters::SymBeta(q) => {
                let q = f64::from(q);
                2.0 * (0..k)
                    .map(|r| (q + 1.0 + f64::from(r)) / (2.0 * q + 2.0 + f64::from(r)))
                    .product::<f64>()
            }
        }
    }

    fn order(&self) -> Option<u32> {
        match *self {
            Daughters::BinaryEqual => None,
            Daughters::Uniform => Some(0),
            Daughters::SymBeta(q) => Some(q),
        }
    }

    /// Density `nu p(v|x)` of the continuous distributions (`None` for [`Daughters::BinaryEqual`]).
    pub fn density(&self, v: f64, x: f64) -> Option<f64> {
        let q = self.order()?;
        let c = 2.0 * factorial(2 * q + 1) / (factorial(q) * factorial(q));
        let (a, b) = (v / x, (x - v) / x);
        Some(c * a.powi(q as i32) * b.powi(q as i32) / x)
    }
}

/// Breakup mechanism: rate and daughter distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakup {
    /// Rate of a particle of volume `x`.
    pub rate: BreakRate,
    /// Daughter distribution.
    pub daughters: Daughters,
}

/// Pivots of the discrete bins.
#[derive(Debug, Clone, PartialEq)]
pub struct BinGrid {
    /// Pivot volumes, strictly increasing and positive.
    pub x: Vec<f64>,
}

/// Where a point mass of volume `v` goes: two pivots with number weights (or one, with `w1 = 0`).
#[derive(Debug, Clone, Copy)]
struct Place {
    i0: usize,
    w0: f64,
    i1: usize,
    w1: f64,
}

impl BinGrid {
    /// Linear grid `x_i = i dx`, `i = 1..=n`.
    pub fn linear(n: usize, dx: f64) -> Result<Self> {
        if n < 2 || dx.is_nan() || dx <= 0.0 {
            return Err(PopBalError::Invalid("linear grid needs n >= 2 and dx > 0".into()));
        }
        Ok(BinGrid {
            x: (1..=n).map(|i| i as f64 * dx).collect(),
        })
    }

    /// Geometric grid `x_i = x1 2^(i/s)`, `i = 0..=s*doublings` (`s` bins per doubling).
    pub fn geometric(x1: f64, per_doubling: usize, doublings: usize) -> Result<Self> {
        if per_doubling == 0 || doublings == 0 || x1.is_nan() || x1 <= 0.0 {
            return Err(PopBalError::Invalid(
                "geometric grid needs x1 > 0, bins per doubling >= 1 and doublings >= 1".into(),
            ));
        }
        Ok(BinGrid {
            x: (0..=per_doubling * doublings)
                .map(|i| x1 * 2f64.powf(i as f64 / per_doubling as f64))
                .collect(),
        })
    }

    /// Number of bins.
    pub fn len(&self) -> usize {
        self.x.len()
    }

    /// True when the grid has no bins.
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    fn place(&self, v: f64) -> Place {
        let m = self.x.len();
        let idx = self.x.partition_point(|&xi| xi <= v);
        if idx == 0 {
            Place {
                i0: 0,
                w0: v / self.x[0],
                i1: 0,
                w1: 0.0,
            }
        } else if idx == m {
            Place {
                i0: m - 1,
                w0: v / self.x[m - 1],
                i1: m - 1,
                w1: 0.0,
            }
        } else {
            let lo = idx - 1;
            let a = (self.x[lo + 1] - v) / (self.x[lo + 1] - self.x[lo]);
            Place {
                i0: lo,
                w0: a,
                i1: lo + 1,
                w1: 1.0 - a,
            }
        }
    }

    fn deposit(&self, v: f64, count: f64, out: &mut [f64]) {
        let p = self.place(v);
        out[p.i0] += count * p.w0;
        out[p.i1] += count * p.w1;
    }
}

const GL_X: [f64; 3] = [0.238_619_186_083_196_9, 0.661_209_386_466_264_5, 0.932_469_514_203_152];
const GL_W: [f64; 3] = [0.467_913_934_572_691, 0.360_761_573_048_138_6, 0.171_324_492_379_170_4];

/// Six-point Gauss-Legendre rule on `[a, b]` split into `panels` panels; calls `f(v, weight)` for every node.
fn gauss_legendre(a: f64, b: f64, panels: usize, mut f: impl FnMut(f64, f64)) {
    let h = (b - a) / panels as f64;
    for p in 0..panels {
        let (lo, hi) = (a + h * p as f64, a + h * (p + 1) as f64);
        let (c, r) = (0.5 * (lo + hi), 0.5 * (hi - lo));
        for i in 0..3 {
            f(c - r * GL_X[i], r * GL_W[i]);
            f(c + r * GL_X[i], r * GL_W[i]);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Pair {
    j: u32,
    k: u32,
    i0: u32,
    i1: u32,
    beta: f64,
    w0: f64,
    w1: f64,
}

/// Fixed-pivot discrete-bin population balance of a well-mixed cell (ADR 0041 D2).
#[derive(Debug, Clone)]
pub struct BinModel {
    grid: BinGrid,
    pairs: Vec<Pair>,
    rates: Vec<f64>,
    daughters: Vec<Vec<(usize, f64)>>,
}

impl BinModel {
    /// Build the pair table of the coalescence kernel and the daughter matrix of the breakup mechanism.
    pub fn new(grid: BinGrid, coal: Option<CoalKernel>, brk: Option<Breakup>) -> Result<Self> {
        let m = grid.len();
        if m < 2 || grid.x.windows(2).any(|w| w[1].is_nan() || w[1] <= w[0]) || grid.x[0].is_nan() || grid.x[0] <= 0.0 {
            return Err(PopBalError::Invalid(
                "bin grid must have at least two strictly increasing positive pivots".into(),
            ));
        }
        let mut pairs = Vec::new();
        if let Some(kern) = coal {
            for j in 0..m {
                for k in j..m {
                    let b = kern.eval(grid.x[j], grid.x[k]);
                    if !b.is_finite() || b < 0.0 {
                        return Err(PopBalError::Invalid(format!(
                            "coalescence kernel is not finite and non-negative at pivots {j}, {k}"
                        )));
                    }
                    if b == 0.0 {
                        continue;
                    }
                    let p = grid.place(grid.x[j] + grid.x[k]);
                    pairs.push(Pair {
                        j: j as u32,
                        k: k as u32,
                        i0: p.i0 as u32,
                        i1: p.i1 as u32,
                        beta: if j == k { 0.5 * b } else { b },
                        w0: p.w0,
                        w1: p.w1,
                    });
                }
            }
        }
        let mut rates = vec![0.0; m];
        let mut daughters = vec![Vec::new(); m];
        if let Some(b) = brk {
            if let Daughters::SymBeta(q) = b.daughters {
                if q > 5 {
                    return Err(PopBalError::Invalid(
                        "symmetric-beta daughter order must be at most 5".into(),
                    ));
                }
            }
            for k in 0..m {
                let g = b.rate.eval(grid.x[k]);
                if !g.is_finite() || g < 0.0 {
                    return Err(PopBalError::Invalid(format!(
                        "breakup rate is not finite and non-negative at pivot {k}"
                    )));
                }
                rates[k] = g;
                if g == 0.0 {
                    continue;
                }
                let mut row = vec![0.0; m];
                match b.daughters {
                    Daughters::BinaryEqual => grid.deposit(0.5 * grid.x[k], 2.0, &mut row),
                    d => {
                        let xk = grid.x[k];
                        let mut lo = 0.0;
                        for &hi in &grid.x[..=k] {
                            gauss_legendre(lo, hi, 1, |v, w| {
                                if let Some(dens) = d.density(v, xk) {
                                    grid.deposit(v, dens * w, &mut row);
                                }
                            });
                            lo = hi;
                        }
                    }
                }
                daughters[k] = row.into_iter().enumerate().filter(|(_, n)| *n != 0.0).collect();
            }
        }
        Ok(BinModel {
            grid,
            pairs,
            rates,
            daughters,
        })
    }

    /// The pivots.
    pub fn grid(&self) -> &BinGrid {
        &self.grid
    }

    /// Number of bins.
    pub fn len(&self) -> usize {
        self.grid.len()
    }

    /// True when there are no bins.
    pub fn is_empty(&self) -> bool {
        self.grid.is_empty()
    }

    /// Daughter matrix entries `(i, n_ik)` of the breakup of bin `k`.
    pub fn daughter_row(&self, k: usize) -> &[(usize, f64)] {
        &self.daughters[k]
    }

    /// Time derivative of the bin number densities.
    pub fn rhs(&self, n: &[f64], out: &mut [f64]) {
        out.iter_mut().for_each(|o| *o = 0.0);
        for p in &self.pairs {
            let (j, k) = (p.j as usize, p.k as usize);
            if n[j] == 0.0 || n[k] == 0.0 {
                continue;
            }
            let r = p.beta * n[j] * n[k];
            out[j] -= r;
            out[k] -= r;
            out[p.i0 as usize] += p.w0 * r;
            out[p.i1 as usize] += p.w1 * r;
        }
        for (k, row) in self.daughters.iter().enumerate() {
            let g = self.rates[k] * n[k];
            if g == 0.0 {
                continue;
            }
            out[k] -= g;
            for &(i, nik) in row {
                out[i] += nik * g;
            }
        }
    }

    /// One classical RK4 step of size `h`.
    pub fn rk4_step(&self, n: &mut [f64], h: f64) {
        let m = n.len();
        let (mut k1, mut k2, mut k3, mut k4) = (vec![0.0; m], vec![0.0; m], vec![0.0; m], vec![0.0; m]);
        let mut tmp = vec![0.0; m];
        self.rhs(n, &mut k1);
        for i in 0..m {
            tmp[i] = n[i] + 0.5 * h * k1[i];
        }
        self.rhs(&tmp, &mut k2);
        for i in 0..m {
            tmp[i] = n[i] + 0.5 * h * k2[i];
        }
        self.rhs(&tmp, &mut k3);
        for i in 0..m {
            tmp[i] = n[i] + h * k3[i];
        }
        self.rhs(&tmp, &mut k4);
        for i in 0..m {
            n[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
    }

    /// Advance by `t` with steps of about `dt` (`round(t/dt)` equal steps). Returns the smallest ratio `min_i N_i / max_i N_i`
    /// seen after any step (negative when positivity was lost).
    pub fn advance(&self, n: &mut [f64], t: f64, dt: f64) -> f64 {
        let steps = ((t / dt).round() as usize).max(1);
        let h = t / steps as f64;
        let mut worst = f64::INFINITY;
        for _ in 0..steps {
            self.rk4_step(n, h);
            let mx = n.iter().cloned().fold(0.0, f64::max);
            let mn = n.iter().cloned().fold(f64::INFINITY, f64::min);
            if mx > 0.0 {
                worst = worst.min(mn / mx);
            }
        }
        worst
    }

    /// Moment of order `k`: `sum x_i^k N_i`.
    pub fn moment(&self, n: &[f64], k: f64) -> f64 {
        self.grid.x.iter().zip(n).map(|(x, ni)| x.powf(k) * ni).sum()
    }

    /// Total volume `sum x_i N_i` per carrier volume.
    pub fn volume(&self, n: &[f64]) -> f64 {
        self.moment(n, 1.0)
    }

    /// Bin values for a point mass: `count` particles of volume `v` (two-pivot split).
    pub fn monodisperse(&self, v: f64, count: f64) -> Vec<f64> {
        let mut n = vec![0.0; self.len()];
        self.grid.deposit(v, count, &mut n);
        n
    }

    /// Project a number density `f(v)` onto the bins with the two-pivot weights, `N_i = int hat_i f dv` over `(0, x_M)`.
    pub fn project(&self, f: impl Fn(f64) -> f64) -> Vec<f64> {
        let mut n = vec![0.0; self.len()];
        let mut lo = 0.0;
        for &hi in &self.grid.x {
            gauss_legendre(lo, hi, 4, |v, w| self.grid.deposit(v, f(v) * w, &mut n));
            lo = hi;
        }
        n
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Quadrature method of moments
// ---------------------------------------------------------------------------------------------------------------------

/// Gauss quadrature of a size distribution: `m_k = sum w_i x_i^k` for `k < 2 N`.
#[derive(Debug, Clone, PartialEq)]
pub struct Quadrature {
    /// Nodes, ascending.
    pub nodes: Vec<f64>,
    /// Weights.
    pub weights: Vec<f64>,
}

/// Eigen-decomposition of the symmetric tridiagonal matrix (`d` diagonal, `e[i]` couples `i` and `i+1`, `e[n-1]` unused):
/// implicit QL. On return `d` holds the eigenvalues and `z` (initially the identity) the eigenvectors in its columns.
fn tridiagonal_ql(d: &mut [f64], e: &mut [f64], z: &mut [Vec<f64>]) -> Result<()> {
    let n = d.len();
    for l in 0..n {
        let mut iter = 0;
        loop {
            let mut m = l;
            while m + 1 < n {
                let dd = d[m].abs() + d[m + 1].abs();
                if e[m].abs() <= 1e-16 * dd {
                    break;
                }
                m += 1;
            }
            if m == l {
                break;
            }
            iter += 1;
            if iter > 60 {
                return Err(PopBalError::Invalid(
                    "eigenvalue iteration of the Jacobi matrix did not converge".into(),
                ));
            }
            let mut g = (d[l + 1] - d[l]) / (2.0 * e[l]);
            let mut r = g.hypot(1.0);
            g = d[m] - d[l] + e[l] / (g + if g >= 0.0 { r.abs() } else { -r.abs() });
            let (mut s, mut c, mut p) = (1.0, 1.0, 0.0);
            let mut i = m;
            let mut underflow = false;
            while i > l {
                i -= 1;
                let f = s * e[i];
                let b = c * e[i];
                r = f.hypot(g);
                e[i + 1] = r;
                if r == 0.0 {
                    d[i + 1] -= p;
                    e[m] = 0.0;
                    underflow = true;
                    break;
                }
                s = f / r;
                c = g / r;
                g = d[i + 1] - p;
                r = (d[i] - g) * s + 2.0 * c * b;
                p = s * r;
                d[i + 1] = g + p;
                g = c * r - b;
                for row in z.iter_mut() {
                    let f = row[i + 1];
                    row[i + 1] = s * row[i] + c * f;
                    row[i] = c * row[i] - s * f;
                }
            }
            if underflow {
                continue;
            }
            d[l] -= p;
            e[l] = g;
            e[m] = 0.0;
        }
    }
    Ok(())
}

/// Relative threshold below which a recurrence coefficient counts as zero (fewer support points than nodes).
const BETA_TOL: f64 = 1e-12;

/// Invert `2 N` moments `m_0..m_{2N-1}` to an `N`-node quadrature with Wheeler's algorithm. A distribution with fewer than
/// `N` support points gives fewer nodes. Non-realizable sets are a located [`PopBalError::NotRealizable`].
pub fn invert_moments(m: &[f64], n_nodes: usize) -> Result<Quadrature> {
    if n_nodes == 0 || m.len() < 2 * n_nodes {
        return Err(PopBalError::Invalid(format!(
            "{n_nodes} quadrature nodes need {} moments, {} given",
            2 * n_nodes,
            m.len()
        )));
    }
    for (i, v) in m.iter().take(2 * n_nodes).enumerate() {
        if !v.is_finite() {
            return Err(PopBalError::NotRealizable {
                index: i,
                detail: format!("moment is not finite ({v})"),
            });
        }
    }
    if m[0] <= 0.0 {
        return Err(PopBalError::NotRealizable {
            index: 0,
            detail: format!("zeroth moment must be positive, got {}", m[0]),
        });
    }
    if m[1] <= 0.0 {
        return Err(PopBalError::NotRealizable {
            index: 1,
            detail: format!("first moment must be positive, got {}", m[1]),
        });
    }
    let scale = m[1] / m[0];
    let n = n_nodes;
    let mu: Vec<f64> = (0..2 * n).map(|l| m[l] / (m[0] * scale.powi(l as i32))).collect();
    let mut sig = vec![vec![0.0; 2 * n]; n + 1];
    sig[0][..2 * n].copy_from_slice(&mu);
    let mut alpha = vec![mu[1] / mu[0]];
    let mut beta = vec![mu[0]];
    for k in 1..n {
        for l in k..(2 * n - k) {
            let s2 = if k >= 2 { sig[k - 2][l] } else { 0.0 };
            sig[k][l] = sig[k - 1][l + 1] - alpha[k - 1] * sig[k - 1][l] - beta[k - 1] * s2;
        }
        let bk = sig[k][k] / sig[k - 1][k - 1];
        if bk < -BETA_TOL {
            return Err(PopBalError::NotRealizable {
                index: 2 * k,
                detail: format!(
                    "Hankel determinant of order {} is negative (normalised recurrence coefficient {bk:.3e})",
                    k + 1
                ),
            });
        }
        if bk <= BETA_TOL {
            break;
        }
        alpha.push(sig[k][k + 1] / sig[k][k] - sig[k - 1][k] / sig[k - 1][k - 1]);
        beta.push(bk);
    }
    let ne = alpha.len();
    let mut d = alpha;
    let mut e: Vec<f64> = (0..ne)
        .map(|i| if i + 1 < ne { beta[i + 1].sqrt() } else { 0.0 })
        .collect();
    let mut z: Vec<Vec<f64>> = (0..ne)
        .map(|i| {
            let mut r = vec![0.0; ne];
            r[i] = 1.0;
            r
        })
        .collect();
    tridiagonal_ql(&mut d, &mut e, &mut z)?;
    let mut idx: Vec<usize> = (0..ne).collect();
    idx.sort_by(|&a, &b| d[a].partial_cmp(&d[b]).unwrap());
    let nodes: Vec<f64> = idx.iter().map(|&i| d[i] * scale).collect();
    if nodes.iter().any(|&x| x.is_nan() || x <= 0.0) {
        return Err(PopBalError::NotRealizable {
            index: 1,
            detail: format!("quadrature node {:.3e} is not positive", nodes[0]),
        });
    }
    let weights: Vec<f64> = idx.iter().map(|&i| m[0] * z[0][i] * z[0][i]).collect();
    Ok(Quadrature { nodes, weights })
}

/// Moment-method population balance of a well-mixed cell: `2 N` moments evolved with the QMOM closure (ADR 0041 D3).
#[derive(Debug, Clone, Copy)]
pub struct MomentModel {
    /// Number of quadrature nodes `N` (moments `0..2N`).
    pub n_nodes: usize,
    /// Coalescence kernel.
    pub coal: Option<CoalKernel>,
    /// Breakup mechanism.
    pub brk: Option<Breakup>,
}

impl MomentModel {
    /// Time derivative of the moments `m_0..m_{2N-1}`.
    pub fn rhs(&self, m: &[f64]) -> Result<Vec<f64>> {
        let q = invert_moments(m, self.n_nodes)?;
        let nm = 2 * self.n_nodes;
        let mut out = vec![0.0; nm];
        if let Some(kern) = self.coal {
            for (xi, wi) in q.nodes.iter().zip(&q.weights) {
                for (xj, wj) in q.nodes.iter().zip(&q.weights) {
                    let b = 0.5 * wi * wj * kern.eval(*xi, *xj);
                    for (k, o) in out.iter_mut().enumerate() {
                        let k = k as i32;
                        *o += b * ((xi + xj).powi(k) - xi.powi(k) - xj.powi(k));
                    }
                }
            }
        }
        if let Some(b) = self.brk {
            for (x, w) in q.nodes.iter().zip(&q.weights) {
                let g = b.rate.eval(*x);
                for (k, o) in out.iter_mut().enumerate() {
                    *o += w * g * (b.daughters.moment_factor(k as u32) - 1.0) * x.powi(k as i32);
                }
            }
        }
        Ok(out)
    }

    /// Advance the moments by `t` with RK4 steps of about `dt`; a failed inversion reports the time and the stage.
    pub fn advance(&self, m: &mut [f64], t: f64, dt: f64) -> Result<()> {
        let steps = ((t / dt).round() as usize).max(1);
        let h = t / steps as f64;
        let nm = m.len();
        for s in 0..steps {
            let t0 = h * s as f64;
            let at = |e: PopBalError, stage: usize| match e {
                PopBalError::NotRealizable { index, detail } => PopBalError::NotRealizable {
                    index,
                    detail: format!("{detail} (time {:.6e}, Runge-Kutta stage {stage})", t0),
                },
                o => o,
            };
            let k1 = self.rhs(m).map_err(|e| at(e, 1))?;
            let tmp: Vec<f64> = (0..nm).map(|i| m[i] + 0.5 * h * k1[i]).collect();
            let k2 = self.rhs(&tmp).map_err(|e| at(e, 2))?;
            let tmp: Vec<f64> = (0..nm).map(|i| m[i] + 0.5 * h * k2[i]).collect();
            let k3 = self.rhs(&tmp).map_err(|e| at(e, 3))?;
            let tmp: Vec<f64> = (0..nm).map(|i| m[i] + h * k3[i]).collect();
            let k4 = self.rhs(&tmp).map_err(|e| at(e, 4))?;
            for i in 0..nm {
                m[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// One-way transport of the bins
// ---------------------------------------------------------------------------------------------------------------------

/// Bin number densities carried as passive scalars on a single-phase carrier flux, with the 0-D kernels as cell sources by
/// Lie splitting. One-way: no feedback on the carrier, no interphase coupling.
pub struct PopBalTransport {
    /// One transported scalar per bin (same geometry, own boundary conditions).
    pub bins: Vec<ScalarTransport>,
    /// The 0-D model used for the cell sources.
    pub model: BinModel,
}

impl PopBalTransport {
    /// Check that there is one scalar per bin and that all share the cell count.
    pub fn new(bins: Vec<ScalarTransport>, model: BinModel) -> Result<Self> {
        if bins.len() != model.len() {
            return Err(PopBalError::Invalid(format!(
                "{} transported scalars for {} bins",
                bins.len(),
                model.len()
            )));
        }
        let nc = bins[0].field.values.len();
        if bins.iter().any(|b| b.field.values.len() != nc) {
            return Err(PopBalError::Invalid("bin scalars differ in cell count".into()));
        }
        Ok(PopBalTransport { bins, model })
    }

    /// The bin vector of one cell.
    pub fn cell_state(&self, c: usize) -> Vec<f64> {
        self.bins.iter().map(|b| b.field.values[c]).collect()
    }

    /// Moment of order `k` in every cell.
    pub fn moment_field(&self, k: f64) -> Vec<f64> {
        let nc = self.bins[0].field.values.len();
        (0..nc).map(|c| self.model.moment(&self.cell_state(c), k)).collect()
    }

    /// Source half of the split: the 0-D bin system over `dt` in every cell (RK4 sub-steps of about `h`); the history of
    /// every bin is reset to the new level.
    pub fn source_step(&mut self, dt: f64, h: f64) {
        let nc = self.bins[0].field.values.len();
        let nb = self.bins.len();
        let mut n = vec![0.0; nb];
        for c in 0..nc {
            for (b, ni) in self.bins.iter().zip(n.iter_mut()) {
                *ni = b.field.values[c];
            }
            self.model.advance(&mut n, dt, h);
            for (b, ni) in self.bins.iter_mut().zip(&n) {
                b.field.values[c] = *ni;
            }
        }
        for b in &mut self.bins {
            b.history = History::new(b.field.values.clone());
        }
    }

    /// One split step on the carrier flux `phi`: implicit-Euler advection of every bin, then the source step. Returns the
    /// largest advection residual.
    pub fn advance(&mut self, phi: &[f64], dt: f64, source_dt: f64) -> f64 {
        let mut res: f64 = 0.0;
        for b in &mut self.bins {
            res = res.max(b.advance(phi, dt, TimeScheme::ImplicitEuler));
        }
        self.source_step(dt, source_dt);
        res
    }
}
