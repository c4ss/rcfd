//! The sparse whirl state solver above the loss of stiffness (ADR 0428, F-519): the eigenvalues of the state matrix of ADR
//! 0416 nearest a registered shift point `+/- i sqrt(sigma)` by an isotropic shift-invert Arnoldi process, without the dense
//! cap.
//!
//! With `x = [u; u_t]`, `x_t = C x`, `C = [[0, I], [-M^-1 K, -M^-1 G]]`, the real operator `P = (C - i w_0)^-1 (C + i w_0)^-1
//! = (C^2 + sigma I)^-1`, `sigma = w_0^2`, has the eigenvalues `nu = 1 / (s^2 + sigma)` (D1). In the scaled state `x~ = [c u;
//! u_t]`, `c = sqrt(sigma)`, one application is one solve with the real symmetric `R = [[K - sigma M, c G], [-c G, K - sigma
//! M]]` (the real form of `Q(i sqrt(sigma))`, its diagonal blocks `K_eff - sigma M`), factored once by the skyline `L D L^T`
//! without pivoting (D4). `P` is skew-Hamiltonian for the form `Omega = [[G, M], [-M, 0]]`, so its Krylov spaces are isotropic
//! and see one direction of each plane `span{x_s, x_-s}` of the double eigenvalue `nu(s) = nu(-s)`; the isotropy is enforced
//! against rounding (D2, after Mehrmann and Watkins's SHIRA). The Ritz values come from the Francis QR algorithm of ADR 0416
//! on the Hessenberg matrix, each giving `+/- s`, `s^2 = 1/nu - sigma`; the stability flag is ADR 0416's rule on that set.

use super::super::{dot, doubled, inertia, mass_refused, speed_refused, Whirl};
use super::{cdiv, cmul, hessenberg_eigenvalues, null_vector, Stability, StateWhirlMode, STABILITY_TOLERANCE};
use crate::constraint::reduce;
use crate::error::{FemError, Result};
use crate::mpc::Transformation;
use crate::rotating::{LossOfStiffness, Path, RotatingPart, RotatingSector};
use rcfd_linalg::skyline::{PivotRule, SkylineLdlt, SkylineOptions, SkylineOrdering};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

mod refine;

const TOLERANCE: f64 = 1e-12;
const BREAKDOWN: f64 = 1e-13;
const SEED: u64 = 0x5eed_0519;

/// Options of the sparse whirl state solver (ADR 0428 D5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SparseStateOptions {
    /// The shift `sigma > 0`, rad^2/s^2: the eigenvalues nearest `+/- i sqrt(sigma)` (largest `|1 / (s^2 + sigma)|`) are found.
    pub shift: f64,
    /// The number of Ritz values `nu` wanted (each gives the pair `+/- s`; a conjugate partner at the boundary is added).
    pub count: usize,
    /// A Ritz pair is converged when its residual estimate carried to `s` is at most `tolerance` relative (default 1e-12).
    pub tolerance: f64,
    /// Refine every returned mode by inverse iteration on `Q(s)` at its own eigenvalue (ADR 0442, F-520; default `false`).
    /// The eigenvalues, the flag and the run are not changed by it, only the mode vectors and their residuals.
    pub refine: bool,
}

impl SparseStateOptions {
    /// `count` Ritz values at the shift `shift` (rad^2/s^2) with the default tolerance 1e-12.
    pub fn new(shift: f64, count: usize) -> SparseStateOptions {
        SparseStateOptions {
            shift,
            count,
            tolerance: TOLERANCE,
            refine: false,
        }
    }
    /// The same options with another convergence tolerance.
    pub fn tolerance(mut self, tolerance: f64) -> SparseStateOptions {
        self.tolerance = tolerance;
        self
    }
    /// The same options with the refinement of the mode vectors of ADR 0442 on or off.
    pub fn refine(mut self, on: bool) -> SparseStateOptions {
        self.refine = on;
        self
    }
}

/// The eigenvalues of a sparse gyroscopic state problem nearest a shift, with its reported modes (ADR 0428).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SparseStateEigen {
    /// The returned eigenvalues `[Re s, Im s]` (`+/- s` of each Ritz value), ordered by `(Im s, Re s)`.
    pub eigenvalues: Vec<[f64; 2]>,
    /// The flag of ADR 0416 D3 on the returned eigenvalues.
    pub stability: Stability,
    /// `max|Re s| / max|s|` over the returned eigenvalues.
    pub real_defect: f64,
    /// `Re s` of each reported mode (`Im s > 0`, or real with `Re s >= 0`), ordered by `(Im s, Re s)`.
    pub growth: Vec<f64>,
    /// `Im s` of each reported mode, rad/s.
    pub omegas: Vec<f64>,
    /// The real parts of the modes, `phi^H M phi = 1`, the entry of largest modulus real and positive.
    pub re: Vec<Vec<f64>>,
    /// The imaginary parts.
    pub im: Vec<Vec<f64>>,
    /// `||Q(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)` of each.
    pub residuals: Vec<f64>,
    /// The Arnoldi steps (solves with the shifted factor) of the run.
    pub steps: usize,
    /// `max_ij |q_i^T Omega q_j| / max_j ||Omega q_j||` of the Arnoldi basis (0 in exact arithmetic, D2).
    pub isotropy: f64,
}

/// The sparse whirl state analysis of a part or of a harmonic index of a sector at one speed (ADR 0428 D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SparseStateWhirl {
    /// The speed, rad/s.
    pub speed: f64,
    /// `None` if the reduced `K_eff` is positive definite, otherwise the loss of stiffness of ADR 0403 D3.
    pub loss: Option<LossOfStiffness>,
    /// The shift `sigma`, rad^2/s^2.
    pub shift: f64,
    /// The returned eigenvalues `[Re s, Im s]`, ordered by `(Im s, Re s)`.
    pub eigenvalues: Vec<[f64; 2]>,
    /// The flag of ADR 0416 D3 on the returned eigenvalues.
    pub stability: Stability,
    /// `max|Re s| / max|s|` over the returned eigenvalues.
    pub real_defect: f64,
    /// The reported modes, ordered by `(omega, growth)`, on the model's (or the sector's) equations.
    pub modes: Vec<StateWhirlMode>,
    /// The number of reduced unknowns `n`.
    pub unknowns: usize,
    /// The Arnoldi steps of the run.
    pub steps: usize,
    /// The isotropy diagnostic of the Arnoldi basis.
    pub isotropy: f64,
}

fn shift_refused(reason: String) -> FemError {
    FemError::InvalidValue { key: "shift", reason }
}

fn not_converged(reason: String) -> FemError {
    FemError::Solver {
        solver: "sparse_state_whirl",
        reason,
    }
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn norm(x: &[f64]) -> f64 {
    dot(x, x).sqrt()
}

/// `scale (a - a^T)`, exactly skew.
fn skew_part(a: &CsrMatrix, scale: f64) -> CsrMatrix {
    let mut t = Vec::with_capacity(2 * a.val.len());
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let v = scale * a.val[p];
            t.push((r, a.col[p], v));
            t.push((a.col[p], r, -v));
        }
    }
    CsrMatrix::from_triplets(a.n, t)
}

/// The principal square root of `z`, exactly imaginary for a negative real `z`.
fn csqrt(z: [f64; 2]) -> [f64; 2] {
    let r = z[0].hypot(z[1]);
    let re = (0.5 * (r + z[0])).max(0.0).sqrt();
    let im = (0.5 * (r - z[0])).max(0.0).sqrt();
    [re, if z[1] < 0.0 { -im } else { im }]
}

/// The one of `+/- s` with `Im > 0`, or `Re >= 0` when `Im = 0`.
fn canonical(s: [f64; 2]) -> [f64; 2] {
    if s[1] < 0.0 || (s[1] == 0.0 && s[0] < 0.0) {
        [-s[0], -s[1]]
    } else {
        s
    }
}

/// The scaled operator `P~` and the form `Omega^` (ADR 0428 D1, D2).
struct Op<'a> {
    k: &'a CsrMatrix,
    m: &'a CsrMatrix,
    g: CsrMatrix,
    fr: SkylineLdlt,
    c: f64,
    n: usize,
}

impl Op<'_> {
    /// `P~ [f; g] = R^-1 [-M f; -M g - G f / c]`.
    fn apply(&self, x: &[f64]) -> Vec<f64> {
        let n = self.n;
        let (f, g) = x.split_at(n);
        let mf = matvec(self.m, f);
        let mg = matvec(self.m, g);
        let gf = matvec(&self.g, f);
        let mut rhs = Vec::with_capacity(2 * n);
        rhs.extend(mf.iter().map(|v| -v));
        rhs.extend(mg.iter().zip(&gf).map(|(a, b)| -a - b / self.c));
        self.fr.solve(&rhs)
    }
    /// `Omega^ [a; b] = [G a / c + M b; -M a]`.
    fn form(&self, x: &[f64]) -> Vec<f64> {
        let n = self.n;
        let (a, b) = x.split_at(n);
        let ga = matvec(&self.g, a);
        let mb = matvec(self.m, b);
        let ma = matvec(self.m, a);
        let mut out = Vec::with_capacity(2 * n);
        out.extend(ga.iter().zip(&mb).map(|(p, q)| p / self.c + q));
        out.extend(ma.iter().map(|v| -v));
        out
    }
}

fn start_vector(len: usize) -> Vec<f64> {
    let mut s = SEED;
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        })
        .collect()
}

/// A selected Ritz value with the real and imaginary parts of its eigenvector in `H_m`.
struct Ritz {
    nu: [f64; 2],
    yr: Vec<f64>,
    yi: Vec<f64>,
}

struct Run {
    q: Vec<Vec<f64>>,
    ritz: Vec<Ritz>,
    steps: usize,
}

/// Subtracts from `w` its components along the orthonormal `set`, returning the coefficients.
fn project(w: &mut [f64], set: &[Vec<f64>]) -> Vec<f64> {
    let c: Vec<f64> = set.iter().map(|q| dot(q, w)).collect();
    for (q, &ci) in set.iter().zip(&c) {
        if ci != 0.0 {
            for (x, y) in w.iter_mut().zip(q) {
                *x -= ci * y;
            }
        }
    }
    c
}

/// The selected Ritz values of `H_m` (row-major `m x m`) if every one is converged (`Ok(Some)`), `Ok(None)` otherwise.
fn ritz(hm: &[f64], m: usize, beta: f64, opts: &SparseStateOptions) -> Result<Option<Vec<Ritz>>> {
    let ev = hessenberg_eigenvalues(hm.to_vec(), m)?;
    let mut order: Vec<usize> = (0..ev.len()).collect();
    let mag = |i: usize| ev[i][0].hypot(ev[i][1]);
    order.sort_by(|&i, &j| {
        mag(j)
            .total_cmp(&mag(i))
            .then(ev[j][1].total_cmp(&ev[i][1]))
            .then(i.cmp(&j))
    });
    if order.len() < opts.count {
        return Ok(None);
    }
    let mut take = opts.count;
    let last = ev[order[take - 1]];
    if last[1] > 0.0 && take < order.len() {
        let next = ev[order[take]];
        if next[0] == last[0] && next[1] == -last[1] {
            take += 1;
        }
    }
    let zeros = vec![0.0; m * m];
    let mut neg_eye = vec![0.0; m * m];
    for i in 0..m {
        neg_eye[i * m + i] = -1.0;
    }
    let mut out = Vec::with_capacity(take);
    for &i in &order[..take] {
        let nu = ev[i];
        let a = nu[0].hypot(nu[1]);
        if !(a > 0.0 && a.is_finite()) {
            return Ok(None);
        }
        let (yr, yi) = null_vector(hm, &zeros, &neg_eye, m, nu);
        let r = beta * yr[m - 1].hypot(yi[m - 1]);
        let s2 = {
            let inv = cdiv([1.0, 0.0], nu);
            [inv[0] - opts.shift, inv[1]]
        };
        let s_abs2 = s2[0].hypot(s2[1]);
        // |d s / s| = r |s^2 + sigma| / (2 |nu| |s|^2), |s^2 + sigma| = 1 / |nu|.
        let converged =
            s_abs2 > 0.0 && r / (2.0 * a * a * s_abs2) <= opts.tolerance && yr.iter().chain(&yi).all(|v| v.is_finite());
        if !converged {
            return Ok(None);
        }
        out.push(Ritz { nu, yr, yi });
    }
    Ok(Some(out))
}

/// The isotropic shift-invert Arnoldi run (ADR 0428 D2).
fn arnoldi(op: &Op, opts: &SparseStateOptions) -> Result<Run> {
    let n = op.n;
    let len = 2 * n;
    let count = opts.count;
    let max_steps = n.min(20 * count + 100);
    let first = (2 * count + 2).min(max_steps);
    let mut q0 = start_vector(len);
    let nq = norm(&q0);
    q0.iter_mut().for_each(|x| *x /= nq);
    let mut q: Vec<Vec<f64>> = vec![q0];
    let mut y: Vec<Vec<f64>> = Vec::new();
    let mut h: Vec<Vec<f64>> = Vec::new();
    loop {
        let j = q.len() - 1;
        // Omega^ q_j into Y.
        let mut z = op.form(&q[j]);
        let zn = norm(&z);
        for _ in 0..2 {
            project(&mut z, &q);
            project(&mut z, &y);
        }
        let zn2 = norm(&z);
        if zn2 > 1e-12 * zn && zn2.is_finite() {
            z.iter_mut().for_each(|v| *v /= zn2);
            y.push(z);
        }
        // The Arnoldi vector, both passes' coefficients against Q kept.
        let mut w = op.apply(&q[j]);
        let wn = norm(&w);
        let mut col = vec![0.0; j + 2];
        for _ in 0..2 {
            let c = project(&mut w, &q);
            for (a, b) in col.iter_mut().zip(&c) {
                *a += b;
            }
            project(&mut w, &y);
        }
        let beta = norm(&w);
        col[j + 1] = beta;
        h.push(col);
        let m = j + 1;
        let breakdown = beta.partial_cmp(&(BREAKDOWN * wn)) != Some(std::cmp::Ordering::Greater);
        let last = m >= max_steps;
        let due = m >= first && (m - first) % 8 == 0;
        if breakdown || last || due {
            let mut hm = vec![0.0; m * m];
            for (jj, cj) in h.iter().enumerate() {
                for (i, &v) in cj.iter().enumerate().take(m) {
                    hm[i * m + jj] = v;
                }
            }
            let res_beta = if breakdown { 0.0 } else { beta };
            if let Some(r) = ritz(&hm, m, res_beta, opts)? {
                q.truncate(m);
                return Ok(Run { q, ritz: r, steps: m });
            }
            if breakdown || last {
                return Err(not_converged(format!(
                    "{count} Ritz values not converged in {m} Arnoldi steps ({})",
                    if breakdown {
                        "breakdown"
                    } else {
                        "the largest number of steps"
                    }
                )));
            }
        }
        w.iter_mut().for_each(|x| *x /= beta);
        q.push(w);
    }
}

fn isotropy(op: &Op, q: &[Vec<f64>]) -> f64 {
    let forms: Vec<Vec<f64>> = q.iter().map(|x| op.form(x)).collect();
    let fmax = forms.iter().map(|f| norm(f)).fold(0.0f64, f64::max);
    let mut worst: f64 = 0.0;
    for qi in q {
        for f in &forms {
            worst = worst.max(dot(qi, f).abs());
        }
    }
    if fmax > 0.0 {
        worst / fmax
    } else {
        0.0
    }
}

/// `phi^H M phi = 1`, the entry of largest modulus (the first on a tie) real and positive.
fn normalise(m: &CsrMatrix, re: &mut [f64], im: &mut [f64]) {
    let nm = dot(re, &matvec(m, re)) + dot(im, &matvec(m, im));
    let sc = 1.0 / nm.sqrt();
    let mut imax = 0;
    let mut best = -1.0;
    for i in 0..re.len() {
        let v = re[i] * re[i] + im[i] * im[i];
        if v > best {
            best = v;
            imax = i;
        }
    }
    let theta = im[imax].atan2(re[imax]);
    let (cs, sn) = (theta.cos(), theta.sin());
    for i in 0..re.len() {
        let (r, s) = (re[i], im[i]);
        re[i] = sc * (r * cs + s * sn);
        im[i] = sc * (s * cs - r * sn);
    }
    im[imax] = 0.0;
}

/// `||Q(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)`.
fn residual(op: &Op, s: [f64; 2], re: &[f64], im: &[f64]) -> f64 {
    residual_of(op.k, op.m, &op.g, s, re, im)
}

/// The residual of [`residual`] for the matrices `k`, `m` and the skew `g`.
fn residual_of(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, s: [f64; 2], re: &[f64], im: &[f64]) -> f64 {
    let (kr, ki) = (matvec(k, re), matvec(k, im));
    let (mr, mi) = (matvec(m, re), matvec(m, im));
    let (gr, gi) = (matvec(g, re), matvec(g, im));
    let s2 = cmul(s, s);
    let mut num = 0.0;
    for i in 0..re.len() {
        let sg = cmul(s, [gr[i], gi[i]]);
        let sm = cmul(s2, [mr[i], mi[i]]);
        let a = kr[i] + sg[0] + sm[0];
        let b = ki[i] + sg[1] + sm[1];
        num += a * a + b * b;
    }
    let n2 = |x: &[f64], y: &[f64]| (dot(x, x) + dot(y, y)).sqrt();
    let abs = s[0].hypot(s[1]);
    let den = n2(&kr, &ki) + abs * abs * n2(&mr, &mi) + abs * n2(&gr, &gi);
    if den == 0.0 {
        num.sqrt()
    } else {
        num.sqrt() / den
    }
}

/// `(s, re, im, residual)` of one reported mode on the reduced unknowns.
type SparseMode = ([f64; 2], Vec<f64>, Vec<f64>, f64);

struct Core {
    eigenvalues: Vec<[f64; 2]>,
    stability: Stability,
    real_defect: f64,
    modes: Vec<SparseMode>,
    steps: usize,
    isotropy: f64,
}

/// The solve on validated matrices; `G = scale (g - g^T)` (ADR 0428 D1 to D4).
fn solve(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, scale: f64, opts: &SparseStateOptions) -> Result<Core> {
    let n = k.n;
    let sigma = opts.shift;
    if !(sigma.is_finite() && sigma > 0.0) {
        return Err(shift_refused(format!(
            "{sigma} is not a finite shift above 0 (rad^2/s^2)"
        )));
    }
    if !(opts.tolerance > 0.0 && opts.tolerance < 1.0) {
        return Err(FemError::InvalidValue {
            key: "options",
            reason: format!("tolerance {} (between 0 and 1)", opts.tolerance),
        });
    }
    if opts.count == 0 || opts.count > n {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} Ritz values requested of a whirl problem of {n} unknowns (at least 1, at most {n})",
                opts.count
            ),
        });
    }
    let gs = skew_part(g, scale);
    // The factor of M: the test that the mass is positive definite (D4); it is not applied.
    SkylineLdlt::factor_with(
        m,
        &SkylineOptions {
            ordering: SkylineOrdering::Rcm,
            pivots: PivotRule::Positive,
            symmetry_tolerance: 1e-12,
        },
    )
    .map_err(|_| FemError::InvalidValue {
        key: "mass",
        reason: "the mass of the whirl problem is not positive definite".to_string(),
    })?;
    let c = sigma.sqrt();
    let mut t = Vec::with_capacity(2 * (k.val.len() + m.val.len() + gs.val.len()));
    for off in [0, n] {
        for r in 0..n {
            for p in k.row_ptr[r]..k.row_ptr[r + 1] {
                t.push((off + r, off + k.col[p], k.val[p]));
            }
        }
        for r in 0..n {
            for p in m.row_ptr[r]..m.row_ptr[r + 1] {
                t.push((off + r, off + m.col[p], -sigma * m.val[p]));
            }
        }
    }
    for r in 0..n {
        for p in gs.row_ptr[r]..gs.row_ptr[r + 1] {
            let v = c * gs.val[p];
            t.push((r, n + gs.col[p], v));
            t.push((n + r, gs.col[p], -v));
        }
    }
    let rmat = CsrMatrix::from_triplets(2 * n, t);
    let fr = SkylineLdlt::factor_with(
        &rmat,
        &SkylineOptions {
            ordering: SkylineOrdering::Rcm,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        },
    )
    .map_err(|e| {
        shift_refused(format!(
            "the shifted matrix of Q(i sqrt({sigma})) has no factor without pivoting ({e}); choose another shift"
        ))
    })?;
    let op = Op { k, m, g: gs, fr, c, n };
    let run = arnoldi(&op, opts)?;
    let iso = isotropy(&op, &run.q);
    let mut eigenvalues = Vec::with_capacity(2 * run.ritz.len());
    let mut modes = Vec::with_capacity(run.ritz.len());
    for r in &run.ritz {
        let inv = cdiv([1.0, 0.0], r.nu);
        let rho = canonical(csqrt([inv[0] - sigma, inv[1]]));
        eigenvalues.push(rho);
        eigenvalues.push([-rho[0], -rho[1]]);
        // The Ritz vector, smoothed by one application of P~ (D3).
        let mut xr = vec![0.0; 2 * n];
        let mut xi = vec![0.0; 2 * n];
        for (qj, (&a, &b)) in run.q.iter().zip(r.yr.iter().zip(&r.yi)) {
            for ((u, v), w) in xr.iter_mut().zip(xi.iter_mut()).zip(qj) {
                *u += a * w;
                *v += b * w;
            }
        }
        let xr = op.apply(&xr);
        let xi = op.apply(&xi);
        let mut re = vec![0.0; n];
        let mut im = vec![0.0; n];
        for i in 0..n {
            let u = [xr[i] / c, xi[i] / c];
            let v = cdiv([xr[n + i], xi[n + i]], rho);
            re[i] = u[0] + v[0];
            im[i] = u[1] + v[1];
        }
        normalise(m, &mut re, &mut im);
        let res = residual(&op, rho, &re, &im);
        modes.push((rho, re, im, res));
    }
    if eigenvalues.iter().any(|s| !(s[0].is_finite() && s[1].is_finite())) {
        return Err(not_converged("a non-finite eigenvalue".to_string()));
    }
    if opts.refine {
        refine::refine(k, m, &op.g, &mut modes)?;
    }
    eigenvalues.sort_by(|p, q| p[1].total_cmp(&q[1]).then(p[0].total_cmp(&q[0])));
    modes.sort_by(|p, q| p.0[1].total_cmp(&q.0[1]).then(p.0[0].total_cmp(&q.0[0])));
    let smax = eigenvalues.iter().map(|s| s[0].hypot(s[1])).fold(0.0f64, f64::max);
    let remax = eigenvalues.iter().map(|s| s[0].abs()).fold(0.0f64, f64::max);
    let real_defect = if smax > 0.0 { remax / smax } else { 0.0 };
    let stability = if real_defect <= STABILITY_TOLERANCE {
        Stability::Stable
    } else {
        Stability::Unstable {
            growth: eigenvalues.iter().map(|s| s[0]).fold(f64::NEG_INFINITY, f64::max),
        }
    };
    Ok(Core {
        eigenvalues,
        stability,
        real_defect,
        modes,
        steps: run.steps,
        isotropy: iso,
    })
}

fn matrix_refused(reason: String) -> FemError {
    FemError::InvalidValue { key: "matrix", reason }
}

fn check_sparse(a: &CsrMatrix, n: usize, what: &str) -> Result<()> {
    if a.n != n || a.ncols != n {
        return Err(matrix_refused(format!(
            "{what} is {} x {}, not {n} x {n}",
            a.n, a.ncols
        )));
    }
    if a.val.iter().any(|v| !v.is_finite()) {
        return Err(matrix_refused(format!("{what} has a non-finite entry")));
    }
    Ok(())
}

/// The eigenvalues of `(s^2 M + s G + K) phi = 0` nearest `+/- i sqrt(opts.shift)` for sparse `n x n` matrices by the
/// isotropic shift-invert Arnoldi solver of ADR 0428: `k` symmetric of any inertia, `m` symmetric positive definite, `g` skew
/// (its skew part is used). Refuses matrices of unequal or non-square size or with non-finite entries (`"matrix"`), a shift
/// not above 0 or one at which the shifted matrix has no factor without pivoting (`"shift"`), a count of 0 or above `n`
/// (`"modes"`), a tolerance outside `(0, 1)` (`"options"`), an `m` that is not positive definite (`"mass"`) and a run that
/// does not converge (`FemError::Solver`).
pub fn gyroscopic_state_eigen_sparse(
    k: &CsrMatrix,
    m: &CsrMatrix,
    g: &CsrMatrix,
    opts: &SparseStateOptions,
) -> Result<SparseStateEigen> {
    let n = k.n;
    check_sparse(k, n, "K")?;
    check_sparse(m, n, "M")?;
    check_sparse(g, n, "G")?;
    let core = solve(k, m, g, 0.5, opts)?;
    let mut r = SparseStateEigen {
        eigenvalues: core.eigenvalues,
        stability: core.stability,
        real_defect: core.real_defect,
        growth: Vec::new(),
        omegas: Vec::new(),
        re: Vec::new(),
        im: Vec::new(),
        residuals: Vec::new(),
        steps: core.steps,
        isotropy: core.isotropy,
    };
    for (s, re, im, res) in core.modes {
        r.growth.push(s[0]);
        r.omegas.push(s[1]);
        r.re.push(re);
        r.im.push(im);
        r.residuals.push(res);
    }
    Ok(r)
}

fn sparse_kappa(cr: &CsrMatrix, mr: &CsrMatrix, re: &[f64], im: &[f64]) -> f64 {
    let num = 2.0 * dot(re, &matvec(cr, im));
    let den = dot(re, &matvec(mr, re)) + dot(im, &matvec(mr, im));
    num / den
}

impl<'a> RotatingPart<'a> {
    /// The whirl state at the speed `omega`, rad/s, by the sparse solver of ADR 0428: the eigenvalues of the state matrix of
    /// ADR 0416 nearest `+/- i sqrt(opts.shift)`, the flag of ADR 0416 D3 on them and their modes, without the dense cap
    /// (also where `K_eff` is not positive definite; then `loss` is set). `G` is the Coriolis matrix if `options.coriolis` and
    /// 0 otherwise. Refuses a negative or non-finite speed (`"speed"`) and everything [`gyroscopic_state_eigen_sparse`]
    /// refuses.
    pub fn whirl_state_sparse(&self, omega: f64, opts: &SparseStateOptions) -> Result<SparseStateWhirl> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.coriolis()?;
        let dofs = self.model.dof_map();
        let keff = self.operator(omega);
        let n = self.m.n;
        let (kr, mr, cr, free) = match &self.path {
            Path::Elimination { fixed } => {
                let zero = vec![0.0; n];
                let kr = reduce(&keff, &zero, fixed)?;
                let mr = reduce(&self.m, &zero, fixed)?;
                let cr = reduce(&c, &zero, fixed)?;
                let free = kr.free().to_vec();
                (kr.matrix, mr.matrix, cr.matrix, Some(free))
            }
            Path::Transformation(tr) => {
                let zero = vec![0.0; tr.n_unknowns()];
                let (kr, _) = tr.reduce(&keff, &zero)?;
                let (mr, _) = tr.reduce(&self.m, &zero)?;
                let (cr, _) = tr.reduce(&c, &zero)?;
                (kr, mr, cr, None)
            }
        };
        for (i, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let what = match (&self.path, &free) {
                    (Path::Elimination { .. }, Some(free)) => {
                        let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
                        format!("node {node} component {component}")
                    }
                    (Path::Transformation(tr), _) => crate::mpc::unknown_name(tr, self.model, tr.independent()[i]),
                    _ => unreachable!("the elimination path has its free list"),
                };
                return Err(mass_refused(what, *d));
            }
        }
        let loss = inertia(&kr, &mr)?.map(|negative| LossOfStiffness { speed: omega, negative });
        let scale = if self.options.coriolis { omega } else { 0.0 };
        let core = solve(&kr, &mr, &cr, scale, opts)?;
        let expand = |q: &[f64]| -> Vec<f64> {
            match (&self.path, &free) {
                (Path::Elimination { .. }, Some(free)) => {
                    let mut full = vec![0.0; n];
                    for (&eq, &x) in free.iter().zip(q) {
                        full[eq] = x;
                    }
                    full
                }
                (Path::Transformation(tr), _) => tr.expand(q)[..n].to_vec(),
                _ => unreachable!("the elimination path has its free list"),
            }
        };
        let modes = core
            .modes
            .iter()
            .map(|(s, re, im, res)| {
                let kappa = sparse_kappa(&cr, &mr, re, im);
                StateWhirlMode {
                    growth: s[0],
                    omega: s[1],
                    frequency: s[1] / (2.0 * PI),
                    re: expand(re),
                    im: expand(im),
                    whirl: kappa,
                    direction: Whirl::of(kappa),
                    residual: *res,
                }
            })
            .collect();
        Ok(SparseStateWhirl {
            speed: omega,
            loss,
            shift: opts.shift,
            eigenvalues: core.eigenvalues,
            stability: core.stability,
            real_defect: core.real_defect,
            modes,
            unknowns: kr.n,
            steps: core.steps,
            isotropy: core.isotropy,
        })
    }
}

impl<'a> RotatingSector<'a> {
    /// The whirl state of harmonic index `harmonic` at the speed `omega`, rad/s, by the sparse solver of ADR 0428 on the
    /// index's real reduced problem (doubled for a pair, as [`RotatingSector::whirl_state`]); modes on the sector's equations
    /// (cosine then sine part for a pair), no wave sense. Refuses a harmonic above `N / 2` (`"harmonic"`), a negative or
    /// non-finite speed (`"speed"`) and everything [`gyroscopic_state_eigen_sparse`] refuses.
    pub fn whirl_state_sparse(
        &self,
        harmonic: usize,
        omega: f64,
        opts: &SparseStateOptions,
    ) -> Result<SparseStateWhirl> {
        let sectors = self.sectors;
        if 2 * harmonic > sectors {
            return Err(FemError::InvalidValue {
                key: "harmonic",
                reason: format!(
                    "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                    sectors / 2
                ),
            });
        }
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.part.coriolis()?;
        let model = self.part.model;
        let multiplicity = if harmonic == 0 || 2 * harmonic == sectors { 1 } else { 2 };
        let n = model.dof_map().n_equations();
        let tr: Transformation = if harmonic == 0 {
            crate::mpc::transformation(model, &[])?
        } else {
            crate::mpc::harmonic_transformation(model, &[], harmonic)?
        };
        let keff = self.part.operator(omega);
        let (k2, m2, c2) = if multiplicity == 2 {
            (doubled(&keff), doubled(&self.part.m), doubled(&c))
        } else {
            (keff, self.part.m.clone(), c)
        };
        let zero = vec![0.0; tr.n_unknowns()];
        let (kr, _) = tr.reduce(&k2, &zero)?;
        let (mr, _) = tr.reduce(&m2, &zero)?;
        let (cr, _) = tr.reduce(&c2, &zero)?;
        for (a, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let i = tr.independent()[a];
                return Err(mass_refused(crate::cyclic::unknown_label(&tr, model, i), *d));
            }
        }
        let loss = inertia(&kr, &mr)?.map(|negative| LossOfStiffness { speed: omega, negative });
        let scale = if self.part.options.coriolis { omega } else { 0.0 };
        let core = solve(&kr, &mr, &cr, scale, opts)?;
        let width = multiplicity * n;
        let modes = core
            .modes
            .iter()
            .map(|(s, re, im, res)| {
                let kappa = sparse_kappa(&cr, &mr, re, im);
                StateWhirlMode {
                    growth: s[0],
                    omega: s[1],
                    frequency: s[1] / (2.0 * PI),
                    re: tr.expand(re)[..width].to_vec(),
                    im: tr.expand(im)[..width].to_vec(),
                    whirl: kappa,
                    direction: Whirl::of(kappa),
                    residual: *res,
                }
            })
            .collect();
        Ok(SparseStateWhirl {
            speed: omega,
            loss,
            shift: opts.shift,
            eigenvalues: core.eigenvalues,
            stability: core.stability,
            real_defect: core.real_defect,
            modes,
            unknowns: kr.n,
            steps: core.steps,
            isotropy: core.isotropy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        (*s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    fn csr(a: &[f64], n: usize) -> CsrMatrix {
        let t = (0..n * n)
            .filter(|&p| a[p] != 0.0)
            .map(|p| (p / n, p % n, a[p]))
            .collect();
        CsrMatrix::from_triplets(n, t)
    }

    /// The solver of the development against the dense state solver of ADR 0416 on random banded indefinite problems (not
    /// a registered check of ADR 0428).
    #[test]
    fn development_against_dense() {
        for (n, count, seed, sigma) in [(4, 2, 5u64, 0.3), (9, 3, 11, 2.0), (20, 4, 23, 0.7), (40, 5, 31, 1.5)] {
            let mut s = seed;
            let mut k = vec![0.0; n * n];
            let mut m = vec![0.0; n * n];
            let mut g = vec![0.0; n * n];
            for i in 0..n {
                k[i * n + i] = 2.0 * lcg(&mut s) + 0.3;
                m[i * n + i] = 1.0 + 0.2 * lcg(&mut s);
                if i + 1 < n {
                    let b = lcg(&mut s);
                    k[i * n + i + 1] = b;
                    k[(i + 1) * n + i] = b;
                    let c = 0.1 * lcg(&mut s);
                    m[i * n + i + 1] = c;
                    m[(i + 1) * n + i] = c;
                    let v = 2.0 * lcg(&mut s);
                    g[i * n + i + 1] = v;
                    g[(i + 1) * n + i] = -v;
                }
            }
            let d = super::super::gyroscopic_state_eigen_dense(&k, &m, &g, n).unwrap();
            let r = gyroscopic_state_eigen_sparse(
                &csr(&k, n),
                &csr(&m, n),
                &csr(&g, n),
                &SparseStateOptions::new(sigma, count),
            )
            .unwrap();
            let nu = |e: &[f64; 2]| {
                let s2 = cmul(*e, *e);
                let v = cdiv([1.0, 0.0], [s2[0] + sigma, s2[1]]);
                v[0].hypot(v[1])
            };
            let mut dense = d.eigenvalues.clone();
            dense.sort_by(|a, b| nu(b).total_cmp(&nu(a)));
            dense.truncate(r.eigenvalues.len());
            let mut worst: f64 = 0.0;
            for e in &r.eigenvalues {
                let best = dense
                    .iter()
                    .map(|t| (e[0] - t[0]).hypot(e[1] - t[1]) / t[0].hypot(t[1]))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(best);
            }
            let res = r.residuals.iter().fold(0.0f64, |a, &b| a.max(b));
            println!(
                "n {n}: {} eigenvalues, worst {worst:.2e}, residual {res:.2e}, steps {}, isotropy {:.2e}, flags {:?} {:?}",
                r.eigenvalues.len(),
                r.steps,
                r.isotropy,
                r.stability,
                d.stability
            );
            assert!(worst < 1e-10, "n {n}: {worst:e}");
            assert!(res < 1e-9, "n {n}: residual {res:e}");
        }
    }
}
