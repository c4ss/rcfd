//! F-657 (ADR 0573): whirl of a spinning annular disc of shells, the rotary block of the shell Coriolis matrix. A flat annular
//! plate (Shell4 or Shell3) clamped at its inner radius on a cyclic sector, its transverse forward and backward whirl from
//! `RotatingSector::whirl_modes` against the semi-analytical reference of D3 (the Mindlin plate with rotary inertia and the
//! closed-form centrifugal membrane stress, a p-version Galerkin solve in `r` per nodal diameter). Checks RF1 to RF4, WF, OD,
//! SP, KA, DI, TR and the negative control NC1 of the registration (B1 and B2 are the existing tests, `baseline_hashes` and
//! `docs_site`).

use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingSector, Whirl, WhirlMode};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const A: f64 = 0.04;
const B: f64 = 0.1;
const T: f64 = 5e-3;
const KS: f64 = 5.0 / 6.0;
const N: usize = 48;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// The two meshes of D2: (columns per sector, radial rows).
const MESHES: [(usize, usize); 2] = [(1, 8), (2, 16)];
const HARMONICS: [usize; 3] = [1, 2, 3];
/// The registered reference discretisation `(p, n_e)` and its check RF1.
const REF: (usize, usize) = (8, 8);
const REF_FINE: (usize, usize) = (10, 12);

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

struct Record {
    fails: Vec<String>,
    worst: Vec<(String, f64, f64)>,
}

impl Record {
    fn new() -> Record {
        Record {
            fails: Vec::new(),
            worst: Vec::new(),
        }
    }
    fn hold(&mut self, name: &str, ok: bool, detail: String) {
        println!("{name}: {detail} {}", met(ok));
        if !ok {
            self.fails.push(format!("{name}: {detail}"));
        }
    }
    /// `value <= limit`, and the worst `value / limit` of the group `group` kept for the summary.
    fn at_most(&mut self, group: &str, name: &str, value: f64, limit: f64) {
        self.hold(name, value <= limit, format!("{value:.4e} (limit <= {limit:.4e})"));
        match self.worst.iter_mut().find(|w| w.0 == group) {
            Some(w) => {
                if value / limit > w.1 / w.2 || (value / limit).is_nan() {
                    w.1 = value;
                    w.2 = limit;
                }
            }
            None => self.worst.push((group.to_string(), value, limit)),
        }
    }
    fn done(self) {
        for (g, v, l) in &self.worst {
            println!("worst {g}: {v:.4e} (its limit {l:.4e})");
        }
        assert!(self.fails.is_empty(), "missed: {:#?}", self.fails);
    }
}

// ------------------------------------------------------------------------------------------------ the reference (D3)

/// The closed-form plane-stress pre-stress at 1 rad/s, clamped at `a`, free at `b` (D3): `(u, sigma_r, sigma_t)`.
fn prestress(r: f64) -> (f64, f64, f64) {
    let al = (1.0 - NU * NU) * RHO / (8.0 * E);
    let c1 = al * ((3.0 + NU) * B.powi(4) + (1.0 - NU) * A.powi(4)) / ((1.0 + NU) * B * B + (1.0 - NU) * A * A);
    let c2 = al * A.powi(4) - c1 * A * A;
    let f = E / (1.0 - NU * NU);
    let u = c1 * r + c2 / r - al * r * r * r;
    let sr = f * ((1.0 + NU) * c1 - (1.0 - NU) * c2 / (r * r) - (3.0 + NU) * al * r * r);
    let st = f * ((1.0 + NU) * c1 + (1.0 - NU) * c2 / (r * r) - (1.0 + 3.0 * NU) * al * r * r);
    (u, sr, st)
}

/// The Legendre polynomial `P_p(x)` and its derivative.
fn legendre(p: usize, x: f64) -> (f64, f64) {
    let (mut p0, mut p1) = (1.0, x);
    if p == 0 {
        return (1.0, 0.0);
    }
    for k in 1..p {
        let k = k as f64;
        let p2 = ((2.0 * k + 1.0) * x * p1 - k * p0) / (k + 1.0);
        p0 = p1;
        p1 = p2;
    }
    let pf = p as f64;
    let d = if (x * x - 1.0).abs() < 1e-300 {
        0.5 * pf * (pf + 1.0) * x.powi(p as i32 + 1)
    } else {
        pf * (x * p1 - p0) / (x * x - 1.0)
    };
    (p1, d)
}

/// The Gauss-Lobatto points of degree `p` (the ends and the roots of `P_p'`).
fn lobatto(p: usize) -> Vec<f64> {
    let pf = p as f64;
    let mut x: Vec<f64> = (0..=p).map(|i| -(PI * i as f64 / pf).cos()).collect();
    for xi in x.iter_mut().take(p).skip(1) {
        for _ in 0..100 {
            let (pp, dp) = legendre(p, *xi);
            let d2 = (2.0 * *xi * dp - pf * (pf + 1.0) * pp) / (1.0 - *xi * *xi);
            let step = dp / d2;
            *xi -= step;
            if step.abs() < 1e-16 {
                break;
            }
        }
    }
    x
}

/// The Gauss-Legendre points and weights of order `q`.
fn gauss(q: usize) -> Vec<(f64, f64)> {
    (0..q)
        .map(|i| {
            let mut x = -(PI * (i as f64 + 0.75) / (q as f64 + 0.5)).cos();
            for _ in 0..100 {
                let (p, d) = legendre(q, x);
                let step = p / d;
                x -= step;
                if step.abs() < 1e-16 {
                    break;
                }
            }
            let (_, d) = legendre(q, x);
            (x, 2.0 / ((1.0 - x * x) * d * d))
        })
        .collect()
}

/// The Lagrange polynomials on `nodes` and their derivatives at `x`.
fn lagrange(nodes: &[f64], x: f64) -> (Vec<f64>, Vec<f64>) {
    let n = nodes.len();
    let mut l = vec![1.0; n];
    let mut dl = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            if j != i {
                l[i] *= (x - nodes[j]) / (nodes[i] - nodes[j]);
            }
        }
        for k in 0..n {
            if k == i {
                continue;
            }
            let mut term = 1.0 / (nodes[i] - nodes[k]);
            for j in 0..n {
                if j != i && j != k {
                    term *= (x - nodes[j]) / (nodes[i] - nodes[j]);
                }
            }
            dl[i] += term;
        }
    }
    (l, dl)
}

/// The dense reduced matrices of D3 for nodal diameter `n`: `K`, `K_sigma` (1 rad/s), `M`, `M_I`, `G` (`x^T G x = 2 int I R
/// T`), row-major, on the unknowns left free by the clamp at `a` (and at `b` with `outer_clamped`).
struct Mats {
    n: usize,
    k: Vec<f64>,
    ks: Vec<f64>,
    m: Vec<f64>,
    ms: Vec<f64>,
    g: Vec<f64>,
}

fn matrices(nd: usize, p: usize, ne: usize, t: f64, outer_clamped: bool) -> Mats {
    let nf = nd as f64;
    let d = E * t * t * t / (12.0 * (1.0 - NU * NU));
    let gs = KS * E / (2.0 * (1.0 + NU)) * t;
    let inertia = RHO * t * t * t / 12.0;
    let mt = RHO * t;
    let nodes = lobatto(p);
    let quad = gauss(p + 4);
    let nn = ne * p + 1;
    let full = 3 * nn;
    let mut fixed = vec![false; full];
    for f in 0..3 {
        fixed[f * nn] = true;
        if outer_clamped {
            fixed[f * nn + nn - 1] = true;
        }
    }
    let mut map = vec![usize::MAX; full];
    let mut nq = 0;
    for i in 0..full {
        if !fixed[i] {
            map[i] = nq;
            nq += 1;
        }
    }
    let mut out = Mats {
        n: nq,
        k: vec![0.0; nq * nq],
        ks: vec![0.0; nq * nq],
        m: vec![0.0; nq * nq],
        ms: vec![0.0; nq * nq],
        g: vec![0.0; nq * nq],
    };
    let h = (B - A) / ne as f64;
    let nb = p + 1;
    let nl = 3 * nb;
    for e in 0..ne {
        let r0 = A + e as f64 * h;
        let gidx: Vec<usize> = (0..3).flat_map(|f| (0..nb).map(move |a| f * nn + e * p + a)).collect();
        let mut loc = [
            vec![0.0; nl * nl],
            vec![0.0; nl * nl],
            vec![0.0; nl * nl],
            vec![0.0; nl * nl],
            vec![0.0; nl * nl],
        ];
        for &(xq, wq) in &quad {
            let r = r0 + 0.5 * (xq + 1.0) * h;
            let w = wq * 0.5 * h * r;
            let (l, dl) = lagrange(&nodes, xq);
            let dn: Vec<f64> = dl.iter().map(|v| v * 2.0 / h).collect();
            let field = |cw: &dyn Fn(usize) -> f64, cr: &dyn Fn(usize) -> f64, ct: &dyn Fn(usize) -> f64| {
                let mut v = vec![0.0; nl];
                for a in 0..nb {
                    v[a] = cw(a);
                    v[nb + a] = cr(a);
                    v[2 * nb + a] = ct(a);
                }
                v
            };
            let z = |_: usize| 0.0;
            let krr = field(&z, &|a| dn[a], &z);
            let ktt = field(&z, &|a| l[a] / r, &|a| nf * l[a] / r);
            let chi = field(&z, &|a| -nf * l[a] / r, &|a| dn[a] - l[a] / r);
            let gr = field(&|a| dn[a], &|a| l[a], &z);
            let gt = field(&|a| -nf * l[a] / r, &z, &|a| l[a]);
            let wr = field(&|a| dn[a], &z, &z);
            let wt = field(&|a| nf * l[a] / r, &z, &z);
            let fw = field(&|a| l[a], &z, &z);
            let fr = field(&z, &|a| l[a], &z);
            let ft = field(&z, &z, &|a| l[a]);
            let (_, sr, st) = prestress(r);
            let (nr, nt) = (t * sr, t * st);
            for i in 0..nl {
                for j in 0..nl {
                    let o = i * nl + j;
                    let kb = d
                        * (krr[i] * krr[j]
                            + ktt[i] * ktt[j]
                            + NU * (krr[i] * ktt[j] + ktt[i] * krr[j])
                            + 0.5 * (1.0 - NU) * chi[i] * chi[j]);
                    let kshear = gs * (gr[i] * gr[j] + gt[i] * gt[j]);
                    loc[0][o] += w * (kb + kshear);
                    loc[1][o] += w * (nr * wr[i] * wr[j] + nt * wt[i] * wt[j]);
                    let rot = fr[i] * fr[j] + ft[i] * ft[j];
                    loc[2][o] += w * (mt * fw[i] * fw[j] + inertia * rot);
                    loc[3][o] += w * inertia * rot;
                    loc[4][o] += w * inertia * (fr[i] * ft[j] + ft[i] * fr[j]);
                }
            }
        }
        let targets = [&mut out.k, &mut out.ks, &mut out.m, &mut out.ms, &mut out.g];
        for (dst, src) in targets.into_iter().zip(loc.iter()) {
            for i in 0..nl {
                let gi = map[gidx[i]];
                if gi == usize::MAX {
                    continue;
                }
                for j in 0..nl {
                    let gj = map[gidx[j]];
                    if gj == usize::MAX {
                        continue;
                    }
                    dst[gi * nq + gj] += src[i * nl + j];
                }
            }
        }
    }
    out
}

/// The lower Cholesky factor of a dense positive definite matrix.
fn cholesky(a: &[f64], n: usize) -> Vec<f64> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = a[j * n + j];
        for k in 0..j {
            s -= l[j * n + k] * l[j * n + k];
        }
        assert!(
            s > 0.0,
            "reference: the matrix of the fixed point is not positive definite"
        );
        let d = s.sqrt();
        l[j * n + j] = d;
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = s / d;
        }
    }
    l
}

fn chol_solve(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut y = b.to_vec();
    for i in 0..n {
        for k in 0..i {
            y[i] -= l[i * n + k] * y[k];
        }
        y[i] /= l[i * n + i];
    }
    for i in (0..n).rev() {
        for k in i + 1..n {
            y[i] -= l[k * n + i] * y[k];
        }
        y[i] /= l[i * n + i];
    }
    y
}

fn matvec(a: &[f64], n: usize, x: &[f64]) -> Vec<f64> {
    (0..n).map(|i| (0..n).map(|j| a[i * n + j] * x[j]).sum()).collect()
}

fn quad(a: &[f64], n: usize, x: &[f64]) -> f64 {
    matvec(a, n, x).iter().zip(x).map(|(u, v)| u * v).sum()
}

/// The first root of sign `sign` (+1 forward, -1 backward) at speed `om`, stress stiffening `s` (D3's fixed point):
/// `(|omega|, kappa)` with D3's signs, `kappa_f = -x^T G x / x^T M x`, `kappa_b = +x^T G x / x^T M x`.
fn root(mats: &Mats, om: f64, s: f64, sign: f64) -> (f64, f64) {
    let n = mats.n;
    let ke: Vec<f64> = (0..n * n)
        .map(|i| mats.k[i] + om * om * (s * mats.ks[i] - mats.ms[i]))
        .collect();
    let mut w = 0.0f64;
    let mut x = vec![1.0; n];
    for _ in 0..100 {
        let a: Vec<f64> = (0..n * n).map(|i| ke[i] + 2.0 * om * w * mats.g[i]).collect();
        let l = cholesky(&a, n);
        for _ in 0..4 {
            x = chol_solve(&l, n, &matvec(&mats.m, n, &x));
            let nm = quad(&mats.m, n, &x).sqrt();
            x.iter_mut().for_each(|v| *v /= nm);
        }
        let k = quad(&ke, n, &x);
        let m = quad(&mats.m, n, &x);
        let g = 2.0 * om * quad(&mats.g, n, &x);
        let new = (g + sign * (g * g + 4.0 * m * k).sqrt()) / (2.0 * m);
        let done = (new - w).abs() <= 1e-13 * new.abs();
        w = new;
        if done {
            break;
        }
    }
    let kappa = -sign * quad(&mats.g, n, &x) / quad(&mats.m, n, &x);
    (w.abs(), kappa)
}

/// The effect set: `S` no stress stiffening, `F` all three.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Set {
    S,
    F,
}

impl Set {
    fn s(self) -> f64 {
        match self {
            Set::S => 0.0,
            Set::F => 1.0,
        }
    }
}

/// The reference of one case: `(omega_f, omega_b, kappa_f, kappa_b)`.
fn reference_with(mats: &Mats, om: f64, set: Set) -> (f64, f64, f64, f64) {
    let (f, kf) = root(mats, om, set.s(), 1.0);
    let (b, kb) = root(mats, om, set.s(), -1.0);
    (f, b, kf, kb)
}

/// `omega_1`: the lowest reference frequency at rest over `n = 0 .. 3` (D4).
fn omega_1_with(p: usize, ne: usize) -> f64 {
    (0..=3)
        .map(|n| root(&matrices(n, p, ne, T, false), 0.0, 0.0, 1.0).0)
        .fold(f64::INFINITY, f64::min)
}

fn speeds() -> [f64; 3] {
    let w1 = omega_1_with(REF.0, REF.1);
    [1.0, 2.0, 3.0].map(|j| j / 6.0 * w1)
}

// ------------------------------------------------------------------------------------------------ the Kirchhoff closed form (D7)

const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

fn fact(k: usize) -> f64 {
    (1..=k).map(|i| i as f64).product()
}

/// `psi(m)` for an integer `m >= 1`.
fn digamma(m: usize) -> f64 {
    -EULER_GAMMA + (1..m).map(|j| 1.0 / j as f64).sum::<f64>()
}

fn bessel_j(n: usize, x: f64) -> f64 {
    (0..40)
        .map(|k| (-1.0f64).powi(k as i32) * (x / 2.0).powi((2 * k + n) as i32) / (fact(k) * fact(k + n)))
        .sum()
}

fn bessel_i(n: usize, x: f64) -> f64 {
    (0..40)
        .map(|k| (x / 2.0).powi((2 * k + n) as i32) / (fact(k) * fact(k + n)))
        .sum()
}

fn bessel_y(n: usize, x: f64) -> f64 {
    let h = x / 2.0;
    let mut s = 2.0 / PI * h.ln() * bessel_j(n, x);
    for k in 0..n {
        s -= 1.0 / PI * fact(n - k - 1) / fact(k) * h.powi(2 * k as i32 - n as i32);
    }
    for k in 0..40 {
        s -= 1.0 / PI * (digamma(k + 1) + digamma(n + k + 1)) * (-1.0f64).powi(k as i32) * h.powi((2 * k + n) as i32)
            / (fact(k) * fact(n + k));
    }
    s
}

fn bessel_k(n: usize, x: f64) -> f64 {
    let h = x / 2.0;
    let sg = if n % 2 == 0 { 1.0 } else { -1.0 };
    let mut s = -sg * h.ln() * bessel_i(n, x);
    for k in 0..n {
        s += 0.5 * (-1.0f64).powi(k as i32) * fact(n - k - 1) / fact(k) * h.powi(2 * k as i32 - n as i32);
    }
    for k in 0..40 {
        s += sg * 0.5 * (digamma(k + 1) + digamma(n + k + 1)) * h.powi((2 * k + n) as i32) / (fact(k) * fact(n + k));
    }
    s
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    J,
    Y,
    I,
    K,
}

fn bessel(kind: Kind, n: usize, x: f64) -> f64 {
    match kind {
        Kind::J => bessel_j(n, x),
        Kind::Y => bessel_y(n, x),
        Kind::I => bessel_i(n, x),
        Kind::K => bessel_k(n, x),
    }
}

/// The derivative of `Z_n` by the recurrences of D7.
fn bessel_d(kind: Kind, n: usize, x: f64) -> f64 {
    if n == 0 {
        return match kind {
            Kind::J | Kind::Y | Kind::K => -bessel(kind, 1, x),
            Kind::I => bessel(kind, 1, x),
        };
    }
    let lower = bessel(kind, n - 1, x);
    let own = n as f64 / x * bessel(kind, n, x);
    match kind {
        Kind::K => -lower - own,
        _ => lower - own,
    }
}

/// `(W, W', W'', W''')` of the four solutions at radius `r` (D7).
fn columns(n: usize, k: f64, r: f64) -> [[f64; 4]; 4] {
    let nf = n as f64;
    [(Kind::J, -1.0), (Kind::Y, -1.0), (Kind::I, 1.0), (Kind::K, 1.0)].map(|(kind, sg)| {
        let w = bessel(kind, n, k * r);
        let w1 = k * bessel_d(kind, n, k * r);
        let w2 = -w1 / r + nf * nf * w / (r * r) + sg * k * k * w;
        let w3 = -w2 / r + w1 / (r * r) + nf * nf * w1 / (r * r) - 2.0 * nf * nf * w / (r * r * r) + sg * k * k * w1;
        [w, w1, w2, w3]
    })
}

fn det4(mut a: [[f64; 4]; 4]) -> f64 {
    let mut det = 1.0;
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        if p != c {
            a.swap(p, c);
            det = -det;
        }
        det *= a[c][c];
        if a[c][c] == 0.0 {
            return 0.0;
        }
        for i in c + 1..4 {
            let f = a[i][c] / a[c][c];
            for j in c..4 {
                a[i][j] -= f * a[c][j];
            }
        }
    }
    det
}

/// The Kirchhoff frequency determinant at `lambda = k b`, clamped at `a`, clamped (`clamped_outer`) or free at `b`.
fn kirchhoff_det(n: usize, lam: f64, clamped_outer: bool) -> f64 {
    let k = lam / B;
    let nf = n as f64;
    let ca = columns(n, k, A);
    let cb = columns(n, k, B);
    let mut rows = [[0.0; 4]; 4];
    for s in 0..4 {
        rows[0][s] = ca[s][0];
        rows[1][s] = ca[s][1];
        let [w, w1, w2, w3] = cb[s];
        if clamped_outer {
            rows[2][s] = w;
            rows[3][s] = w1;
        } else {
            rows[2][s] = w2 + NU * (w1 / B - nf * nf * w / (B * B));
            rows[3][s] =
                w3 + w2 / B - (1.0 + (2.0 - NU) * nf * nf) * w1 / (B * B) + (3.0 - NU) * nf * nf * w / B.powi(3);
        }
    }
    det4(rows)
}

/// The first root `lambda` in `[1, hi]` (500 grid steps, 100 halvings).
fn kirchhoff_lambda(n: usize, clamped_outer: bool, hi: f64) -> f64 {
    let f = |l: f64| kirchhoff_det(n, l, clamped_outer);
    let mut prev = 1.0;
    for i in 1..=500 {
        let x = 1.0 + (hi - 1.0) * i as f64 / 500.0;
        if f(prev) * f(x) < 0.0 {
            let (mut lo, mut up) = (prev, x);
            for _ in 0..100 {
                let mid = 0.5 * (lo + up);
                if f(lo) * f(mid) <= 0.0 {
                    up = mid;
                } else {
                    lo = mid;
                }
            }
            return 0.5 * (lo + up);
        }
        prev = x;
    }
    panic!("no Kirchhoff root below {hi}");
}

#[test]
fn rf_reference_code() {
    let mut rec = Record::new();
    let w1 = omega_1_with(REF.0, REF.1);
    let w1f = omega_1_with(REF_FINE.0, REF_FINE.1);
    println!("omega_1 = {w1:.9} rad/s, speeds {:?}", speeds());
    rec.at_most("RF1", "RF1 omega_1", (w1f / w1 - 1.0).abs(), 1e-9);
    // RF1: (8, 8) against (10, 12).
    for set in [Set::S, Set::F] {
        for n in HARMONICS {
            let mc = matrices(n, REF.0, REF.1, T, false);
            let mf = matrices(n, REF_FINE.0, REF_FINE.1, T, false);
            for (j, om) in speeds().iter().enumerate() {
                let (f, b, kf, kb) = reference_with(&mc, *om, set);
                let (f2, b2, _, _) = reference_with(&mf, *om, set);
                let name = format!("{set:?} n={n} Omega_{}", j + 1);
                println!(
                    "reference {name}: omega_f {f:.6}, omega_b {b:.6}, split {:.6}, kappa {kf:.6e} / {kb:.6e}",
                    b - f
                );
                let e = (f2 / f - 1.0).abs().max((b2 / b - 1.0).abs());
                rec.at_most("RF1", &format!("RF1 {name}"), e, 1e-9);
                rec.at_most(
                    "RF1 split",
                    &format!("RF1 split {name}"),
                    ((b2 - f2) / (b - f) - 1.0).abs(),
                    1e-6,
                );
            }
        }
    }
    // RF2: the Wronskians.
    for n in 0..=3 {
        for x in [0.5, 1.5, 3.0, 4.0] {
            let wj = bessel(Kind::J, n, x) * bessel_d(Kind::Y, n, x)
                - bessel_d(Kind::J, n, x) * bessel(Kind::Y, n, x)
                - 2.0 / (PI * x);
            let wi = bessel(Kind::I, n, x) * bessel_d(Kind::K, n, x) - bessel_d(Kind::I, n, x) * bessel(Kind::K, n, x)
                + 1.0 / x;
            rec.at_most("RF2", &format!("RF2 n={n} x={x} J,Y"), (wj * x).abs(), 1e-12);
            rec.at_most("RF2", &format!("RF2 n={n} x={x} I,K"), (wi * x).abs(), 1e-12);
        }
    }
    // RF3: the Kirchhoff limit by Richardson in t, (p, n_e) = (16, 8).
    let ratio = |n: usize, t: f64, cc: bool, lam: f64| {
        let w = root(&matrices(n, 16, 8, t, cc), 0.0, 0.0, 1.0).0;
        let d = E * t * t * t / (12.0 * (1.0 - NU * NU));
        w / ((lam / B).powi(2) * (d / (RHO * t)).sqrt())
    };
    for n in 0..=3 {
        let lam = kirchhoff_lambda(n, true, 12.0);
        let (r1, r2) = (ratio(n, 1e-3, true, lam), ratio(n, 5e-4, true, lam));
        println!(
            "RF3a n={n}: lambda^2 {:.9}, ratios {:.4e}, {:.4e}",
            lam * lam,
            r1 - 1.0,
            r2 - 1.0
        );
        rec.at_most(
            "RF3a",
            &format!("RF3a n={n}"),
            ((4.0 * r2 - r1) / 3.0 - 1.0).abs(),
            2e-5,
        );
    }
    let lam = kirchhoff_lambda(0, false, 6.0);
    let (r1, r2) = (ratio(0, 1e-3, false, lam), ratio(0, 5e-4, false, lam));
    println!(
        "RF3b n=0: lambda^2 {:.9}, ratios {:.4e}, {:.4e}",
        lam * lam,
        r1 - 1.0,
        r2 - 1.0
    );
    rec.at_most("RF3b", "RF3b n=0", ((4.0 * r2 - r1) / 3.0 - 1.0).abs(), 1e-6);
    // RF4: the pre-stress closed form.
    let scale = RHO * B * B;
    let uscale = RHO * B * B * B / E;
    rec.at_most("RF4", "RF4 u(a)", (prestress(A).0 / uscale).abs(), 1e-6);
    rec.at_most("RF4", "RF4 sigma_r(b)", (prestress(B).1 / scale).abs(), 1e-6);
    let h = 1e-5 * (B - A);
    for i in 0..=10 {
        let r = A + 2.0 * h + (B - A - 4.0 * h) * i as f64 / 10.0;
        let rs = |x: f64| x * prestress(x).1;
        let res = (rs(r + h) - rs(r - h)) / (2.0 * h) - prestress(r).2 + RHO * r * r;
        rec.at_most("RF4", &format!("RF4 equilibrium r={r:.5}"), (res / scale).abs(), 1e-6);
    }
    rec.done();
}

// ------------------------------------------------------------------------------------------------ the disc sector (D2)

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// The D2 sector: `nc` columns, `nr` radial rows, Shell4 or each quad cut into two Shell3, clamped bore.
fn sector(kind: ElementKind, nc: usize, nr: usize) -> SolidModel {
    let n = axis();
    let l5 = 5.0f64.sqrt();
    let ea = [2.0 / l5, -1.0 / l5, 0.0];
    let eb = cross(n, ea);
    let id = |i: usize, j: usize| j * (nc + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=nr {
        for i in 0..=nc {
            let phi = 2.0 * PI * i as f64 / (N * nc) as f64;
            let (sp, cp) = phi.sin_cos();
            let r = A + (B - A) * j as f64 / nr as f64;
            nodes.push([0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k])));
        }
    }
    let mut conn = Vec::new();
    for j in 0..nr {
        for i in 0..nc {
            let q = [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            match kind {
                ElementKind::Shell4 => conn.extend(q),
                _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("low", (0..=nr).map(|j| id(0, j)).collect()).unwrap();
    mesh.add_node_set("high", (0..=nr).map(|j| id(nc, j)).collect())
        .unwrap();
    mesh.add_node_set("clamp", (0..=nc).map(|i| id(i, 0)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    m
}

fn rotating<'a>(m: &'a SolidModel, set: Set, coriolis: bool) -> RotatingSector<'a> {
    let o = RotatingOptions::new(2).coriolis(coriolis);
    let o = match set {
        Set::S => o.stress_stiffening(false),
        Set::F => o,
    };
    RotatingSector::new(m, &o).unwrap()
}

fn whirl(s: &RotatingSector, k: usize, om: f64) -> (Vec<WhirlMode>, usize, f64) {
    let r = s.whirl_modes(k, om).unwrap();
    let m = r
        .modes()
        .unwrap_or_else(|| panic!("set-up failure: loss of stiffness at harmonic {k}, {om} rad/s"));
    let wmax = m.frequencies_all.iter().fold(0.0f64, |a, w| a.max(w.abs()));
    (m.modes.clone(), m.unknowns, wmax)
}

/// D5: `sqrt(sum |u_perp|^2 / sum (u . n)^2)` over the translations of every node, both halves, real and imaginary parts.
fn in_plane_ratio(m: &SolidModel, mode: &WhirlMode) -> f64 {
    let dofs = m.dof_map();
    let neq = dofs.n_equations();
    let n = axis();
    let (mut perp, mut along) = (0.0, 0.0);
    for v in [&mode.re, &mode.im] {
        for half in 0..v.len() / neq {
            for p in 0..m.mesh().nodes().len() {
                let mut u = [0.0; 3];
                for (c, uc) in u.iter_mut().enumerate() {
                    if let Some(q) = dofs.equation(p, c) {
                        *uc = v[half * neq + q];
                    }
                }
                let a = dot3(u, n);
                along += a * a;
                perp += dot3(u, u) - a * a;
            }
        }
    }
    (perp.max(0.0) / along).sqrt()
}

/// D6: `e_est(k, mesh)`.
fn e_est(k: usize, nc: usize, nr: usize) -> f64 {
    let delta = 2.0 * PI / (N * nc) as f64;
    ((1.875 / nr as f64).powi(2) + (k as f64 * delta).powi(2)) / 8.0 + delta * delta / 6.0
}

fn disc_against_reference(kind: ElementKind, tag: char, c_wf: f64) {
    let mut rec = Record::new();
    let om = speeds();
    let refs: Vec<Mats> = HARMONICS.iter().map(|&k| matrices(k, REF.0, REF.1, T, false)).collect();
    // errors[mesh][set][speed][k][member]
    let mut errors = [[[[[0.0f64; 2]; 3]; 3]; 2]; 2];
    for (mi, &(nc, nr)) in MESHES.iter().enumerate() {
        let m = sector(kind, nc, nr);
        for (si, set) in [Set::S, Set::F].into_iter().enumerate() {
            let s = rotating(&m, set, true);
            for (j, &w) in om.iter().enumerate() {
                for (ki, &k) in HARMONICS.iter().enumerate() {
                    let name = format!("{tag} mesh {} {set:?} Omega_{} k={k}", ["c", "f"][mi], j + 1);
                    let (f, b, kf, kb) = reference_with(&refs[ki], w, set);
                    let (modes, _, _) = whirl(&s, k, w);
                    let (lo, hi, third) = (&modes[0], &modes[1], &modes[2]);
                    let e = [lo.omega / f - 1.0, hi.omega / b - 1.0];
                    let d = (hi.omega - lo.omega) / (b - f) - 1.0;
                    let ka = [lo.whirl / kf - 1.0, hi.whirl / kb - 1.0];
                    let tlo = lo.wave.unwrap_or(f64::NAN);
                    let thi = hi.wave.unwrap_or(f64::NAN);
                    println!(
                        "{name}: omega_lo {:.6} (ref {f:.6}), omega_hi {:.6} (ref {b:.6}), split {:.6} (ref {:.6}), kappa \
                         {:.6e} / {:.6e} (ref {kf:.6e} / {kb:.6e})",
                        lo.omega,
                        hi.omega,
                        hi.omega - lo.omega,
                        b - f,
                        lo.whirl,
                        hi.whirl
                    );
                    let est = e_est(k, nc, nr);
                    let wf = c_wf * est;
                    let sp = 4.0 * est;
                    rec.at_most(&format!("WF{tag}"), &format!("WF{tag} {name} lo"), e[0].abs(), wf);
                    rec.at_most(&format!("WF{tag}"), &format!("WF{tag} {name} hi"), e[1].abs(), wf);
                    rec.at_most(&format!("SP{tag}"), &format!("SP{tag} {name}"), d.abs(), sp);
                    rec.at_most(&format!("KA{tag}"), &format!("KA{tag} {name} lo"), ka[0].abs(), sp);
                    rec.at_most(&format!("KA{tag}"), &format!("KA{tag} {name} hi"), ka[1].abs(), sp);
                    let di = lo.direction == Whirl::Forward
                        && tlo >= 0.99
                        && hi.direction == Whirl::Backward
                        && thi <= -0.99
                        && third.omega >= 1.5 * b;
                    rec.hold(
                        &format!("DI{tag} {name}"),
                        di,
                        format!(
                            "lo {:?} tau {tlo:.6}, hi {:?} tau {thi:.6}, omega_3 / omega_b {:.4}",
                            lo.direction,
                            hi.direction,
                            third.omega / b
                        ),
                    );
                    let tr = in_plane_ratio(&m, lo).max(in_plane_ratio(&m, hi));
                    rec.at_most(&format!("TR{tag}"), &format!("TR{tag} {name}"), tr, 1e-6);
                    println!(
                        "  signed e_lo {:.4e}, e_hi {:.4e}, split d {:.4e}, kappa {:.4e} / {:.4e}, e_est {est:.4e}",
                        e[0], e[1], d, ka[0], ka[1]
                    );
                    errors[mi][si][j][ki] = e;
                }
            }
        }
    }
    for (si, set) in [Set::S, Set::F].into_iter().enumerate() {
        for j in 0..3 {
            for (ki, &k) in HARMONICS.iter().enumerate() {
                for (member, label) in ["lo", "hi"].iter().enumerate() {
                    let ec = errors[0][si][j][ki][member].abs();
                    let ef = errors[1][si][j][ki][member].abs();
                    let p = (ec / ef).log2();
                    rec.at_most(
                        &format!("OD{tag}"),
                        &format!("OD{tag} {set:?} Omega_{} k={k} {label}: p = {p:.4}", j + 1),
                        (p - 2.0).abs(),
                        0.5,
                    );
                }
            }
        }
    }
    rec.done();
}

#[test]
fn disc_shell4_whirl_against_the_reference() {
    disc_against_reference(ElementKind::Shell4, '4', 2.0);
}

#[test]
fn disc_shell3_whirl_against_the_reference() {
    disc_against_reference(ElementKind::Shell3, '3', 4.0);
}

#[test]
fn nc1_no_split_without_coriolis() {
    let mut rec = Record::new();
    let w = speeds()[2];
    for (kind, tag) in [(ElementKind::Shell4, '4'), (ElementKind::Shell3, '3')] {
        let (nc, nr) = MESHES[0];
        let m = sector(kind, nc, nr);
        let s = rotating(&m, Set::S, false);
        for k in HARMONICS {
            let (modes, nq, wmax) = whirl(&s, k, w);
            let split = modes[1].omega / modes[0].omega - 1.0;
            let limit = 10.0 * nq as f64 * f64::EPSILON * wmax / modes[0].omega;
            println!("NC1 {tag} k={k}: n_q {nq}, omega_max {wmax:.4e}");
            rec.at_most("NC1", &format!("NC1 {tag} k={k}"), split.abs(), limit);
        }
    }
    rec.done();
}
