//! F-494 (ADR 0405): the single-corrector growth of the bounded immersed forcing over several steps.
//!
//! ADR 0392 linearised one step of DFG 2D-2 on M2 with one PISO corrector at `dt = 1e-3`: both the bounded form B (which stops
//! at step 34) and the first-order forcing F (which runs) have one-step eigenvalues above one. Here the product of `k`
//! consecutive step Jacobians along the trajectory is linearised by central finite differences of the `k`-step map, and its
//! dominant eigenvalue per step `abs(mu)^(1/k)` is computed by Arnoldi (ADR 0405 D1 to D3); the one-step eigenvalues above one
//! of F are measured against the channel without the cylinder and a later state (D4). Heavy records (`#[ignore]`, run once in
//! release): P1 (B, one and two correctors; G0, G1, G4, R1), P2 (F over 10 and 20 steps; G0, G2, G3, R2), X (X1 to X4). Fast
//! checks: U1, U2 (the copied tools), U3 (the k-step map on M1), U4 (Arnoldi on exact products). The linear algebra, the DFG
//! helpers and the state layout are copies of those of `immersed_step_linearisation_f482.rs` (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::{courant_numbers, TimeScheme};
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, PimpleState};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// Complex numbers (ADR 0392 D4)
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    const ZERO: C = C { re: 0.0, im: 0.0 };
    fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    fn conj(self) -> C {
        C::new(self.re, -self.im)
    }
    fn scale(self, a: f64) -> C {
        C::new(self.re * a, self.im * a)
    }
    fn div(self, o: C) -> C {
        let d = o.re * o.re + o.im * o.im;
        C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    fn sqrt(self) -> C {
        let r = self.abs();
        if r == 0.0 {
            return C::ZERO;
        }
        let re = (0.5 * (r + self.re)).sqrt();
        let im = (0.5 * (r - self.re)).sqrt();
        C::new(re, if self.im < 0.0 { -im } else { im })
    }
}

impl std::ops::Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}

impl std::ops::Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}

impl std::ops::Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Eigenvalues of a small Hessenberg matrix: single-shift complex QR (ADR 0392 D4)
// ---------------------------------------------------------------------------------------------------------------------------

/// The eigenvalue of `[[a, b], [c, d]]` nearest `d` (the Wilkinson shift).
fn wilkinson(a: C, b: C, c: C, d: C) -> C {
    let half = (a + d).scale(0.5);
    let q = (a - d).scale(0.5);
    let disc = (q * q + b * c).sqrt();
    let (l1, l2) = (half + disc, half - disc);
    if (l1 - d).abs() <= (l2 - d).abs() {
        l1
    } else {
        l2
    }
}

/// Eigenvalues of the real upper Hessenberg matrix `h` (`n x n`, row-major `h[i][j]`).
fn hess_eigs(h: &[Vec<f64>]) -> Vec<C> {
    let n = h.len();
    let mut a: Vec<Vec<C>> = h.iter().map(|r| r.iter().map(|&x| C::new(x, 0.0)).collect()).collect();
    let mut eig = Vec::with_capacity(n);
    let mut hi = n;
    let mut since = 0usize;
    let mut iters = 0usize;
    while hi > 0 {
        if hi == 1 {
            eig.push(a[0][0]);
            break;
        }
        // The start of the unreduced block that ends at hi - 1.
        let mut l = hi - 1;
        while l > 0 {
            let s = a[l - 1][l - 1].abs() + a[l][l].abs();
            if a[l][l - 1].abs() <= 1e-14 * s.max(f64::MIN_POSITIVE) {
                a[l][l - 1] = C::ZERO;
                break;
            }
            l -= 1;
        }
        if l == hi - 1 {
            eig.push(a[hi - 1][hi - 1]);
            hi -= 1;
            since = 0;
            continue;
        }
        iters += 1;
        assert!(iters < 100 * n.max(1), "hess_eigs: no convergence");
        since += 1;
        let mu = if since % 10 == 0 {
            a[hi - 1][hi - 1] + C::new(0.75 * a[hi - 1][hi - 2].abs(), 0.0)
        } else {
            wilkinson(
                a[hi - 2][hi - 2],
                a[hi - 2][hi - 1],
                a[hi - 1][hi - 2],
                a[hi - 1][hi - 1],
            )
        };
        for (i, row) in a.iter_mut().enumerate().take(hi).skip(l) {
            row[i] = row[i] - mu;
        }
        // QR of the block by Givens rotations G_k = [[conj c, conj s], [-s, c]] from the left ...
        let mut rots: Vec<(C, C)> = Vec::with_capacity(hi - l);
        for k in l..hi - 1 {
            let (x, y) = (a[k][k], a[k + 1][k]);
            let r = x.abs().hypot(y.abs());
            let (c, s) = if r == 0.0 {
                (C::new(1.0, 0.0), C::ZERO)
            } else {
                (x.scale(1.0 / r), y.scale(1.0 / r))
            };
            for j in k..hi {
                let (ak, ak1) = (a[k][j], a[k + 1][j]);
                a[k][j] = c.conj() * ak + s.conj() * ak1;
                a[k + 1][j] = c * ak1 - s * ak;
            }
            rots.push((c, s));
        }
        // ... then R Q: the conjugate transposes from the right.
        for (idx, k) in (l..hi - 1).enumerate() {
            let (c, s) = rots[idx];
            for row in a.iter_mut().take((k + 2).min(hi)).skip(l) {
                let (ak, ak1) = (row[k], row[k + 1]);
                row[k] = ak * c + ak1 * s;
                row[k + 1] = ak1 * c.conj() - ak * s.conj();
            }
        }
        for (i, row) in a.iter_mut().enumerate().take(hi).skip(l) {
            row[i] = row[i] + mu;
        }
    }
    eig
}

/// Solve the complex system `m y = b` by Gaussian elimination with partial pivoting (`m` consumed).
fn csolve(mut m: Vec<Vec<C>>, mut b: Vec<C>) -> Vec<C> {
    let n = b.len();
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| m[i][k].abs().total_cmp(&m[j][k].abs())).unwrap();
        m.swap(k, p);
        b.swap(k, p);
        let piv = if m[k][k].abs() == 0.0 {
            C::new(1e-300, 0.0)
        } else {
            m[k][k]
        };
        for i in k + 1..n {
            let f = m[i][k].div(piv);
            if f.abs() == 0.0 {
                continue;
            }
            for j in k..n {
                let t = m[k][j];
                m[i][j] = m[i][j] - f * t;
            }
            let t = b[k];
            b[i] = b[i] - f * t;
        }
    }
    let mut y = vec![C::ZERO; n];
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s = s - m[k][j] * y[j];
        }
        let piv = if m[k][k].abs() == 0.0 {
            C::new(1e-300, 0.0)
        } else {
            m[k][k]
        };
        y[k] = s.div(piv);
    }
    y
}

fn cnormalise(y: &mut [C]) {
    let n = y.iter().map(|v| v.abs().powi(2)).sum::<f64>().sqrt();
    for v in y.iter_mut() {
        *v = v.scale(1.0 / n);
    }
}

/// The eigenvector of `h` (`m x m`) for the eigenvalue `theta` by two steps of inverse iteration (ADR 0392 D4).
fn inverse_iteration(h: &[Vec<f64>], theta: C) -> Vec<C> {
    let m = h.len();
    let shift = theta - C::new(1e-10 * theta.abs().max(1e-300), 0.0);
    let mat = || -> Vec<Vec<C>> {
        (0..m)
            .map(|i| {
                (0..m)
                    .map(|j| {
                        let v = C::new(h[i][j], 0.0);
                        if i == j {
                            v - shift
                        } else {
                            v
                        }
                    })
                    .collect()
            })
            .collect()
    };
    let mut y = csolve(mat(), vec![C::new(1.0, 0.0); m]);
    cnormalise(&mut y);
    let mut y = csolve(mat(), y);
    cnormalise(&mut y);
    y
}

// ---------------------------------------------------------------------------------------------------------------------------
// Arnoldi (ADR 0392 D3)
// ---------------------------------------------------------------------------------------------------------------------------

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// A fixed start vector: a linear congruential sequence with the given seed, entries uniform in `[-1, 1]`, normalised.
fn lcg_vector(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    let mut v: Vec<f64> = (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        })
        .collect();
    let nv = norm(&v);
    v.iter_mut().for_each(|x| *x /= nv);
    v
}

/// The Arnoldi basis `V` (`m + 1` vectors) and the Hessenberg matrix `H` (`(m + 1) x m`) of `apply` from `v1`.
fn arnoldi(apply: &mut dyn FnMut(&[f64]) -> Vec<f64>, v1: Vec<f64>, m: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut v = vec![v1];
    let mut h = vec![vec![0.0; m]; m + 1];
    for j in 0..m {
        let mut w = apply(&v[j]);
        for _pass in 0..2 {
            for (i, vi) in v.iter().enumerate() {
                let c = dot(vi, &w);
                h[i][j] += c;
                w.iter_mut().zip(vi).for_each(|(x, y)| *x -= c * y);
            }
        }
        let nw = norm(&w);
        h[j + 1][j] = nw;
        if nw > 1e-300 {
            w.iter_mut().for_each(|x| *x /= nw);
        }
        v.push(w);
    }
    (v, h)
}

/// A Ritz value, its coefficients in the basis and its estimated residual `|h_{m+1,m}| |y_m| / |y|`.
#[derive(Clone, Debug)]
struct Ritz {
    theta: C,
    y: Vec<C>,
    est: f64,
}

fn ritz(h: &[Vec<f64>], m: usize) -> Vec<Ritz> {
    let hm: Vec<Vec<f64>> = h[..m].iter().map(|r| r[..m].to_vec()).collect();
    hess_eigs(&hm)
        .into_iter()
        .map(|theta| {
            let y = inverse_iteration(&hm, theta);
            let est = h[m][m - 1].abs() * y[m - 1].abs() / theta.abs().max(1e-300);
            Ritz { theta, y, est }
        })
        .collect()
}

/// ADR 0392 D3: a Ritz value is real when `|Im theta| <= 1e-3 |theta|`.
fn is_real(theta: C) -> bool {
    theta.im.abs() <= 1e-3 * theta.abs()
}

/// The Ritz value of smallest real part.
fn leftmost(r: &[Ritz]) -> &Ritz {
    r.iter().min_by(|a, b| a.theta.re.total_cmp(&b.theta.re)).unwrap()
}

/// The Ritz vector `V_m y` as its real and imaginary parts; for a real Ritz value the phase is turned so that the largest
/// coefficient is real and the imaginary part is dropped. Normalised.
fn ritz_vector(v: &[Vec<f64>], r: &Ritz) -> (Vec<f64>, Option<Vec<f64>>) {
    let n = v[0].len();
    let mut y = r.y.clone();
    let real = is_real(r.theta);
    if real {
        let k = (0..y.len()).max_by(|&i, &j| y[i].abs().total_cmp(&y[j].abs())).unwrap();
        let ph = y[k].conj().scale(1.0 / y[k].abs());
        y.iter_mut().for_each(|c| *c = *c * ph);
    }
    let mut zr = vec![0.0; n];
    let mut zi = vec![0.0; n];
    for (yj, vj) in y.iter().zip(v) {
        for i in 0..n {
            zr[i] += yj.re * vj[i];
            zi[i] += yj.im * vj[i];
        }
    }
    if real {
        let nz = norm(&zr);
        zr.iter_mut().for_each(|x| *x /= nz);
        (zr, None)
    } else {
        let nz = (dot(&zr, &zr) + dot(&zi, &zi)).sqrt();
        zr.iter_mut().for_each(|x| *x /= nz);
        zi.iter_mut().for_each(|x| *x /= nz);
        (zr, Some(zi))
    }
}

/// The true residual `|J z - theta z| / (|theta| |z|)` of a Ritz pair; returns it with `J Re z`.
fn true_residual(
    apply: &mut dyn FnMut(&[f64]) -> Vec<f64>,
    theta: C,
    zr: &[f64],
    zi: Option<&[f64]>,
) -> (f64, Vec<f64>) {
    let jr = apply(zr);
    let (num, den) = match zi {
        None => {
            let r: f64 = jr.iter().zip(zr).map(|(a, b)| (a - theta.re * b).powi(2)).sum();
            (r, dot(zr, zr))
        }
        Some(zi) => {
            let ji = apply(zi);
            let mut r = 0.0;
            for i in 0..zr.len() {
                let re = jr[i] - (theta.re * zr[i] - theta.im * zi[i]);
                let im = ji[i] - (theta.re * zi[i] + theta.im * zr[i]);
                r += re * re + im * im;
            }
            (r, dot(zr, zr) + dot(zi, zi))
        }
    };
    ((num / den).sqrt() / theta.abs(), jr)
}

// ---------------------------------------------------------------------------------------------------------------------------
// U1, U2: the tools on matrices with known spectra
// ---------------------------------------------------------------------------------------------------------------------------

/// `x - 2 w (w . x) / (w . w)` with `w = (1, 2, ..., n)`.
fn householder(x: &[f64]) -> Vec<f64> {
    let w: Vec<f64> = (1..=x.len()).map(|i| i as f64).collect();
    let c = 2.0 * dot(&w, x) / dot(&w, &w);
    x.iter().zip(&w).map(|(a, b)| a - c * b).collect()
}

fn matvec(t: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    t.iter().map(|r| dot(r, x)).collect()
}

/// Every expected eigenvalue matched by a distinct computed one; returns the largest distance.
fn match_eigs(expected: &[C], got: &[C]) -> f64 {
    assert_eq!(expected.len(), got.len());
    let mut used = vec![false; got.len()];
    let mut worst: f64 = 0.0;
    for e in expected {
        let (k, d) = got
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(i, g)| (i, (*g - *e).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        used[k] = true;
        worst = worst.max(d);
    }
    worst
}

/// An entry `(row, column, value)` of a test matrix.
type Entry = (usize, usize, f64);

/// ADR 0392 U1: the Hessenberg eigenvalues of two 6 x 6 matrices `Q T Q` with known spectra.
#[test]
fn u1_hessenberg_eigenvalues_of_known_matrices() {
    let (a, b) = (0.5, 0.3);
    let (r, ang) = (1.15, 0.9 * std::f64::consts::PI);
    let cases: Vec<(Vec<Entry>, Vec<C>)> = vec![
        (
            vec![
                (0, 0, -1.15),
                (1, 1, 0.9),
                (2, 2, a),
                (2, 3, b),
                (3, 2, -b),
                (3, 3, a),
                (4, 4, 0.1),
                (5, 5, -0.4),
            ],
            vec![
                C::new(-1.15, 0.0),
                C::new(0.9, 0.0),
                C::new(a, b),
                C::new(a, -b),
                C::new(0.1, 0.0),
                C::new(-0.4, 0.0),
            ],
        ),
        (
            vec![
                (0, 0, r * ang.cos()),
                (0, 1, -r * ang.sin()),
                (1, 0, r * ang.sin()),
                (1, 1, r * ang.cos()),
                (2, 2, 0.95),
                (3, 3, 0.3),
                (4, 4, -0.2),
                (5, 5, 0.05),
            ],
            vec![
                C::new(r * ang.cos(), r * ang.sin()),
                C::new(r * ang.cos(), -r * ang.sin()),
                C::new(0.95, 0.0),
                C::new(0.3, 0.0),
                C::new(-0.2, 0.0),
                C::new(0.05, 0.0),
            ],
        ),
    ];
    for (ci, (entries, expected)) in cases.iter().enumerate() {
        let mut t = vec![vec![0.0; 6]; 6];
        // Block upper triangular: the blocks, and entries above them so that the matrix is not normal.
        for (i, row) in t.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate().skip(i + 1) {
                *x = 0.1 * (i + j) as f64;
            }
        }
        for &(i, j, v) in entries {
            t[i][j] = v;
        }
        let mut apply = |x: &[f64]| householder(&matvec(&t, &householder(x)));
        let (_, h) = arnoldi(&mut apply, lcg_vector(6, 7), 6);
        let hm: Vec<Vec<f64>> = h[..6].to_vec();
        let got = hess_eigs(&hm);
        let d = match_eigs(expected, &got);
        println!("U1 case {ci}: eigenvalues {got:?}, largest distance {d:.3e}");
        assert!(d <= 1e-10, "U1 case {ci}: distance {d:.3e}");
    }
}

/// ADR 0392 U2: Arnoldi with 40 vectors on a map of dimension 400 with a real outlier (-1.15) and with a complex pair
/// (`1.15 exp(+-0.9 pi i)`).
#[test]
fn u2_arnoldi_separates_a_real_outlier_from_a_complex_pair() {
    let n = 400;
    let (r, ang) = (1.15, 0.9 * std::f64::consts::PI);
    for case in 0..2 {
        let d: Vec<f64> = (0..n).map(|i| 0.05 + 0.9 * i as f64 / 399.0).collect();
        let mut apply = |x: &[f64]| {
            let mut y = householder(x);
            let (y0, y1) = (y[0], y[1]);
            for i in 2..n {
                y[i] *= d[i];
            }
            if case == 0 {
                y[0] = -1.15 * y0;
                y[1] = 0.7 * y1;
            } else {
                y[0] = r * ang.cos() * y0 - r * ang.sin() * y1;
                y[1] = r * ang.sin() * y0 + r * ang.cos() * y1;
            }
            householder(&y)
        };
        let (v, h) = arnoldi(&mut apply, lcg_vector(n, 482), 40);
        let rz = ritz(&h, 40);
        let l = leftmost(&rz).clone();
        let (zr, zi) = ritz_vector(&v, &l);
        let (res, _) = true_residual(&mut apply, l.theta, &zr, zi.as_deref());
        println!(
            "U2 case {case}: leftmost {:?}, real {}, estimated residual {:.3e}, true residual {:.3e}",
            l.theta,
            is_real(l.theta),
            l.est,
            res
        );
        if case == 0 {
            assert!(is_real(l.theta), "U2 (i): not real: {:?}", l.theta);
            assert!((l.theta - C::new(-1.15, 0.0)).abs() <= 1e-8, "U2 (i): {:?}", l.theta);
            assert!(l.est <= 1e-8, "U2 (i): estimated residual {:.3e}", l.est);
            assert!(res <= 1e-8, "U2 (i): true residual {res:.3e}");
        } else {
            let lam = C::new(r * ang.cos(), r * ang.sin());
            assert!(!is_real(l.theta), "U2 (ii): classified real: {:?}", l.theta);
            assert!(
                (l.theta - lam).abs() <= 1e-8 || (l.theta - lam.conj()).abs() <= 1e-8,
                "U2 (ii): {:?}",
                l.theta
            );
            let partner = l.theta.conj();
            assert!(
                rz.iter().any(|q| (q.theta - partner).abs() <= 1e-8),
                "U2 (ii): no conjugate partner"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_single_corrector_f363.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `build`, `theta_min = 0.01`.
    F,
    /// `build_second_order_bounded`, `theta_min = 0.01`.
    B,
}

fn forcing(geom: &FvGeometry, bodies: &[ImmersedBody], f: Forcing) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, NU);
    let mut s = ImmersedSettings::new(K_DFG);
    s.theta_min = 1e-2;
    let mut ib = match f {
        Forcing::F => ImmersedForcing::build(geom, bodies, s, &gamma),
        Forcing::B => ImmersedForcing::build_second_order_bounded(geom, bodies, s, &gamma),
    }
    .unwrap();
    ib.flux_weight_check = false;
    ib
}

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

fn dfg_body() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    )
}

fn dfg_fields(xs: &[f64], ys: &[f64], um: f64) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(xs, ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (geom, u, p)
}

fn two_d_two_start(geom: &FvGeometry, u: &mut VectorField, um: f64) {
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
}

/// The set-up R of ADR 0374 on `dfg_family(level)` with `n_correctors` correctors and the forcing `f` (check off).
fn r_solver(level: usize, f: Forcing, n_correctors: usize) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = dfg_family(level);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let mut st = PimpleSettings::piso(NU);
    st.n_correctors = n_correctors;
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(forcing(&geom, &[dfg_body()], f));
    (geom, s)
}

fn max_abs_u(s: &PimpleSolver) -> f64 {
    s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------------------------------------
// The state vector, the step map and its linearisation (ADR 0392 D1, D2)
// ---------------------------------------------------------------------------------------------------------------------------

/// The layout of D1: `u_n` (active components per cell), `u_{n-1}`, `p_n`, `phi_n / |S|`, `phi_{n-1} / |S|` (non-empty faces).
struct Layout {
    comps: Vec<usize>,
    nc: usize,
    faces: Vec<usize>,
    area: Vec<f64>,
}

impl Layout {
    fn new(geom: &FvGeometry) -> Layout {
        let comps: Vec<usize> = (0..3).filter(|&k| geom.active[k]).collect();
        let faces: Vec<usize> = (0..geom.mesh.n_faces()).filter(|&f| !geom.empty[f]).collect();
        let area = faces.iter().map(|&f| geom.mesh.face_area(f)).collect();
        Layout {
            comps,
            nc: geom.n_cells(),
            faces,
            area,
        }
    }
    fn nu(&self) -> usize {
        self.comps.len() * self.nc
    }
    /// Block ranges: `u_n`, `u_{n-1}`, `p`, `phi_n`, `phi_{n-1}`.
    fn blocks(&self) -> [std::ops::Range<usize>; 5] {
        let (nu, nc, nf) = (self.nu(), self.nc, self.faces.len());
        [
            0..nu,
            nu..2 * nu,
            2 * nu..2 * nu + nc,
            2 * nu + nc..2 * nu + nc + nf,
            2 * nu + nc + nf..2 * nu + nc + 2 * nf,
        ]
    }
    fn len(&self) -> usize {
        2 * self.nu() + self.nc + 2 * self.faces.len()
    }
    fn pack(&self, st: &PimpleState) -> Vec<f64> {
        let nk = self.comps.len();
        let mut x = Vec::with_capacity(self.len());
        for c in 0..self.nc {
            for &k in &self.comps {
                x.push(st.u[c][k]);
            }
        }
        for c in 0..self.nc {
            for &k in &self.comps {
                x.push(st.hist_u[k].older.as_ref().expect("pack: no older level")[c]);
            }
        }
        debug_assert_eq!(x.len(), 2 * nk * self.nc);
        x.extend_from_slice(&st.p);
        for (i, &f) in self.faces.iter().enumerate() {
            x.push(st.phi[f] / self.area[i]);
        }
        let older = st.phi_hist.older.as_ref().expect("pack: no older flux");
        for (i, &f) in self.faces.iter().enumerate() {
            x.push(older[f] / self.area[i]);
        }
        x
    }
    /// `base + scale * dx` (D1): `u` and `hist_u[k].old` together, `phi` and `phi_hist.old` together.
    fn perturbed(&self, base: &PimpleState, dx: &[f64]) -> PimpleState {
        let mut st = base.clone();
        let nk = self.comps.len();
        let [bu, bo, bp, bf, bfo] = self.blocks();
        for c in 0..self.nc {
            for (j, &k) in self.comps.iter().enumerate() {
                let d = dx[bu.start + c * nk + j];
                st.u[c][k] += d;
                st.hist_u[k].old[c] += d;
                st.hist_u[k].older.as_mut().unwrap()[c] += dx[bo.start + c * nk + j];
            }
        }
        for c in 0..self.nc {
            st.p[c] += dx[bp.start + c];
        }
        for (i, &f) in self.faces.iter().enumerate() {
            let d = dx[bf.start + i] * self.area[i];
            st.phi[f] += d;
            st.phi_hist.old[f] += d;
            st.phi_hist.older.as_mut().unwrap()[f] += dx[bfo.start + i] * self.area[i];
        }
        st
    }
}

/// The step of the solver from a state, packed (D2). A stop panics.
fn step_map(s: &mut PimpleSolver, lay: &Layout, st: &PimpleState, dt: f64) -> Vec<f64> {
    s.restore(st).unwrap();
    s.step(dt);
    assert!(s.diverged.is_none(), "step map: the step stopped: {:?}", s.diverged);
    lay.pack(&s.snapshot())
}

/// `J v = (S(e v) - S(-e v)) / (2 e)` about `base` (D2).
fn jv(s: &mut PimpleSolver, lay: &Layout, base: &PimpleState, dt: f64, v: &[f64], e: f64) -> Vec<f64> {
    let plus: Vec<f64> = v.iter().map(|x| e * x).collect();
    let minus: Vec<f64> = v.iter().map(|x| -e * x).collect();
    let sp = step_map(s, lay, &lay.perturbed(base, &plus), dt);
    let sm = step_map(s, lay, &lay.perturbed(base, &minus), dt);
    sp.iter().zip(&sm).map(|(a, b)| (a - b) / (2.0 * e)).collect()
}

/// The linear solves of the map (D2): momentum and pressure `tol_rel = 1e-12`, `tol_abs = 0`.
fn tighten(s: &mut PimpleSolver) {
    s.settings.momentum_solver.tol_rel = 1e-12;
    s.settings.momentum_solver.tol_abs = 0.0;
    s.settings.pressure_solver.tol_rel = 1e-12;
    s.settings.pressure_solver.tol_abs = 0.0;
}

const EPS: f64 = 1e-4;
const M_ARNOLDI: usize = 40;

// ---------------------------------------------------------------------------------------------------------------------------
// The geometry of the body
// ---------------------------------------------------------------------------------------------------------------------------

/// Per cell: fluid, and first fluid (a fluid cell with an interior face to a cell that is not fluid).
fn fluid_cells(geom: &FvGeometry, ib: &ImmersedForcing) -> (Vec<bool>, Vec<bool>) {
    let fluid: Vec<bool> = ib.body_of_cell.iter().map(|b| b.is_none()).collect();
    let mut first = vec![false; fluid.len()];
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    for f in 0..geom.n_internal {
        let (a, b) = (own[f], nei[f]);
        if fluid[a] && !fluid[b] {
            first[a] = true;
        }
        if fluid[b] && !fluid[a] {
            first[b] = true;
        }
    }
    (fluid, first)
}

fn surface_distance(x: Vec3) -> f64 {
    ((x[0] - CENTRE[0]).hypot(x[1] - CENTRE[1]) - 0.5 * DIAM).abs()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The k-step map along the trajectory and its linearisation (ADR 0405 D1, D2)
// ---------------------------------------------------------------------------------------------------------------------------

/// `S^k` from a state (D1): one restore, then `k` steps without a restore in between. A stop panics.
fn k_step_map(s: &mut PimpleSolver, lay: &Layout, st: &PimpleState, dt: f64, k: usize) -> Vec<f64> {
    s.restore(st).unwrap();
    for j in 0..k {
        s.step(dt);
        assert!(
            s.diverged.is_none(),
            "k-step map: stopped at step {j} of {k}: {:?}",
            s.diverged
        );
    }
    lay.pack(&s.snapshot())
}

/// `P_k v = (S^k(e v) - S^k(-e v)) / (2 e)` about `base` (D2).
#[allow(clippy::too_many_arguments)]
fn pkv(s: &mut PimpleSolver, lay: &Layout, base: &PimpleState, dt: f64, k: usize, v: &[f64], e: f64) -> Vec<f64> {
    let plus: Vec<f64> = v.iter().map(|x| e * x).collect();
    let minus: Vec<f64> = v.iter().map(|x| -e * x).collect();
    let sp = k_step_map(s, lay, &lay.perturbed(base, &plus), dt, k);
    let sm = k_step_map(s, lay, &lay.perturbed(base, &minus), dt, k);
    let two_e = 2.0 * e;
    sp.iter().zip(&sm).map(|(a, b)| (a - b) / two_e).collect()
}

/// The eigenvalue per step `abs(mu)^(1/k)` (ADR 0405).
fn per_step(mu: C, k: usize) -> f64 {
    mu.abs().powf(1.0 / k as f64)
}

/// The largest singular value of the `(m + 1) x m` Arnoldi matrix `Hbar_m` (D3): 4000 power iterations on `Hbar^T Hbar` from
/// the all-ones vector.
fn sigma_max(h: &[Vec<f64>], m: usize) -> f64 {
    let mut g = vec![vec![0.0; m]; m];
    for (i, gi) in g.iter_mut().enumerate() {
        for (j, gij) in gi.iter_mut().enumerate() {
            *gij = (0..=m).map(|r| h[r][i] * h[r][j]).sum();
        }
    }
    let mut x = vec![1.0; m];
    let mut lam = 0.0;
    for _ in 0..4000 {
        let y = matvec(&g, &x);
        let ny = norm(&y);
        if ny == 0.0 {
            return 0.0;
        }
        lam = dot(&x, &y) / dot(&x, &x);
        x = y.iter().map(|v| v / ny).collect();
    }
    lam.max(0.0).sqrt()
}

// ---------------------------------------------------------------------------------------------------------------------------
// U3, U4: the k-step map on M1 and Arnoldi on exact products
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0405 U3 on DFG M1, B, one corrector, `dt = 1e-3`, the state after step 2, `k = 3`: (a) the k-step map at zero is the
/// trajectory; (b) its linearisation with `e = 1e-4` against `1e-5`; (c) the product along the trajectory is the composition
/// of the one-step products about the states of the trajectory; (d) printed: the difference to the third power of `J_1`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn u3_the_k_step_map_and_its_linearisation_on_m1() {
    let dt = 1e-3;
    let k = 3;
    let (geom, mut s) = r_solver(1, Forcing::B, 1);
    run_to("U3", &mut s, dt, 2, 0);
    let base = s.snapshot();
    for _ in 0..k {
        s.step(dt);
        assert!(s.diverged.is_none());
    }
    let direct = s.snapshot();
    let lay = Layout::new(&geom);
    let (_, mut t) = r_solver(1, Forcing::B, 1);
    let zero = vec![0.0; lay.len()];
    let _ = k_step_map(&mut t, &lay, &lay.perturbed(&base, &zero), dt, k);
    let bits = |st: &PimpleState| -> Vec<u64> {
        st.u.iter()
            .flat_map(|v| v.iter().map(|x| x.to_bits()))
            .chain(st.p.iter().map(|x| x.to_bits()))
            .chain(st.phi.iter().map(|x| x.to_bits()))
            .collect()
    };
    assert!(
        bits(&t.snapshot()) == bits(&direct),
        "U3 (a): the k-step map at zero is not the trajectory"
    );
    tighten(&mut t);
    let v = lcg_vector(lay.len(), 482);
    let p4 = pkv(&mut t, &lay, &base, dt, k, &v, 1e-4);
    let p5 = pkv(&mut t, &lay, &base, dt, k, &v, 1e-5);
    let d45: Vec<f64> = p4.iter().zip(&p5).map(|(a, b)| a - b).collect();
    let rel_b = norm(&d45) / norm(&p4);
    // (c): the states of the converged trajectory from step 2.
    t.restore(&base).unwrap();
    let mut states = vec![base.clone()];
    for _ in 1..k {
        t.step(dt);
        assert!(t.diverged.is_none());
        states.push(t.snapshot());
    }
    let mut w = v.clone();
    for st in &states {
        w = jv(&mut t, &lay, st, dt, &w, 1e-4);
    }
    let dc: Vec<f64> = p4.iter().zip(&w).map(|(a, b)| a - b).collect();
    let rel_c = norm(&dc) / norm(&p4);
    // (d): the power of the Jacobian at the first state.
    let mut q = v.clone();
    for _ in 0..k {
        q = jv(&mut t, &lay, &base, dt, &q, 1e-4);
    }
    let dd: Vec<f64> = p4.iter().zip(&q).map(|(a, b)| a - b).collect();
    let rel_d = norm(&dd) / norm(&p4);
    println!(
        "U3: dimension {}, |P v| {:.5}, (b) |P4 v - P5 v| / |P4 v| = {rel_b:.3e}, (c) |P v - J3 J2 J1 v| / |P v| = {rel_c:.3e}, (d) |P v - J1^3 v| / |P v| = {rel_d:.3e}",
        lay.len(),
        norm(&p4)
    );
    assert!(rel_b <= 1e-3, "U3 (b): {rel_b:.3e}");
    assert!(rel_c <= 1e-3, "U3 (c): {rel_c:.3e}");
}

/// ADR 0405 U4: Arnoldi with 40 vectors on exact products of ten maps `Q D_j Q` of dimension 400: (i) a real outlier whose
/// per-step value is the geometric mean of the one-step values; (ii) two coordinates alternating 1.5 and 0.5, so that every
/// map has spectral radius 1.5 and the product `0.75^5` per coordinate (per step 0.8660254).
#[test]
fn u4_arnoldi_on_exact_products_gives_the_eigenvalue_per_step() {
    let n = 400;
    let k = 10;
    for case in 0..2 {
        let diag = |j: usize| -> Vec<f64> {
            let mut d: Vec<f64> = (0..n).map(|i| 0.05 + 0.55 * i as f64 / 399.0).collect();
            if case == 0 {
                d[0] = -(1.1 + 0.02 * j as f64);
                d[1] = 0.7;
            } else if j % 2 == 0 {
                d[0] = 1.5;
                d[1] = 0.5;
            } else {
                d[0] = 0.5;
                d[1] = 1.5;
            }
            d
        };
        let ds: Vec<Vec<f64>> = (0..k).map(diag).collect();
        let radius = ds
            .iter()
            .map(|d| d.iter().fold(0.0f64, |a, x| a.max(x.abs())))
            .fold(f64::INFINITY, f64::min);
        let mut apply = |x: &[f64]| {
            let mut y = x.to_vec();
            for d in &ds {
                let mut z = householder(&y);
                z.iter_mut().zip(d).for_each(|(a, b)| *a *= b);
                y = householder(&z);
            }
            y
        };
        let (_, h) = arnoldi(&mut apply, lcg_vector(n, 482), M_ARNOLDI);
        let mut rz = ritz(&h, M_ARNOLDI);
        rz.sort_by(|a, b| b.theta.abs().total_cmp(&a.theta.abs()));
        let mu = rz[0].theta;
        let g = per_step(mu, k);
        let sig = sigma_max(&h, M_ARNOLDI);
        let expected = if case == 0 {
            (0..k).map(|j| 1.1 + 0.02 * j as f64).product::<f64>().powf(0.1)
        } else {
            0.75f64.sqrt()
        };
        println!(
            "U4 case {case}: mu {:?}, per step {g:.10} (expected {expected:.10}), smallest one-step spectral radius {radius:.3}, sigma per step {:.10}",
            mu,
            sig.powf(0.1)
        );
        assert!(
            (g - expected).abs() <= 1e-8,
            "U4 case {case}: per step {g:.10} against {expected:.10}"
        );
        if case == 0 {
            assert!(is_real(mu) && mu.re > 0.0, "U4 (i): mu {mu:?} not real positive");
        } else {
            assert!(g < 1.0 && radius >= 1.5, "U4 (ii): {g} {radius}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The channel without the cylinder (ADR 0405 D4)
// ---------------------------------------------------------------------------------------------------------------------------

/// The set-up R without the cylinder (`immersed = None`) at the inflow `um`.
fn channel_solver(level: usize, um: f64) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = dfg_family(level);
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let mut st = PimpleSettings::piso(NU);
    st.n_correctors = 1;
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let s = PimpleSolver::new(geom.clone(), u, p, st);
    (geom, s)
}

/// The largest cell Courant number of the present state and its cell (D4).
fn c_max(geom: &FvGeometry, s: &PimpleSolver, dt: f64) -> (f64, usize) {
    let c = courant_numbers(geom, &s.snapshot().phi, dt);
    let i = (0..c.len()).max_by(|&a, &b| c[a].total_cmp(&c[b])).unwrap();
    (c[i], i)
}

/// Run `s` on to step `k` (no stop allowed), printing the largest `abs(u)` of every step.
fn run_to(label: &str, s: &mut PimpleSolver, dt: f64, k: usize, from: usize) {
    for n in from + 1..=k {
        s.step(dt);
        assert!(s.diverged.is_none(), "{label}: stopped at step {n}: {:?}", s.diverged);
        println!("{label}: step {n} max |u| {:.5}", max_abs_u(s));
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// One linearisation of k steps (D3) and its report
// ---------------------------------------------------------------------------------------------------------------------------

#[allow(dead_code)]
struct Analysis {
    ritz: Vec<Ritz>,
    mu: C,
    mu_res: f64,
    g: f64,
    /// Ritz values with `abs(theta)^(1/k) >= 1` (conjugate partners once) with their true residuals.
    big: Vec<(C, f64)>,
    /// Ritz values with `abs(theta)^(1/k) >= 1` whose residual was not computed (above the cap for `k > 1`).
    unchecked: usize,
    g0: Option<f64>,
    cmax: f64,
}

/// Linearise `k` steps of `s` from its present state at `dt` (D1 to D3); the state and settings are restored afterwards. With
/// `g0` the linearity check G0 on the dominant vector; for `k > 1` the nonlinear propagation is printed.
fn analyse(label: &str, geom: &FvGeometry, s: &mut PimpleSolver, dt: f64, k: usize, g0: bool) -> Analysis {
    let t0 = Instant::now();
    let base = s.snapshot();
    let saved = s.settings.clone();
    let lay = Layout::new(geom);
    for kk in 0..3 {
        for c in 0..lay.nc {
            assert_eq!(
                base.hist_u[kk].old[c].to_bits(),
                base.u[c][kk].to_bits(),
                "{label}: hist_u old is not u"
            );
        }
    }
    let (cmax, ccell) = c_max(geom, s, dt);
    let xc = geom.mesh.cell_centres()[ccell];
    println!(
        "{label}: C_max {cmax:.5} in cell {ccell} at ({:.5}, {:.5}), distance to the surface {:.5}",
        xc[0],
        xc[1],
        surface_distance(xc)
    );
    let x0 = lay.pack(&base);
    let s_r = k_step_map(s, &lay, &base, dt, k);
    tighten(s);
    let s_0 = k_step_map(s, &lay, &base, dt, k);
    let dstep: Vec<f64> = s_0.iter().zip(&x0).map(|(a, b)| a - b).collect();
    let dtol: Vec<f64> = s_r.iter().zip(&s_0).map(|(a, b)| a - b).collect();
    println!(
        "{label}: k = {k}, state dimension {}, |S_R(0) - S(0)| / |S(0) - x_0| = {:.3e}",
        lay.len(),
        norm(&dtol) / norm(&dstep)
    );
    let mut apply = |v: &[f64]| pkv(s, &lay, &base, dt, k, v, EPS);
    let (v, h) = arnoldi(&mut apply, lcg_vector(lay.len(), 482), M_ARNOLDI);
    let mut rz = ritz(&h, M_ARNOLDI);
    rz.sort_by(|a, b| b.theta.abs().total_cmp(&a.theta.abs()));
    println!("{label}: Ritz values (theta, |theta|, per step, estimated residual), the twelve largest and all with per step > 0.9:");
    for (i, r) in rz.iter().enumerate() {
        if i < 12 || per_step(r.theta, k) > 0.9 {
            println!(
                "{label}:   {:+.6e} {:+.6e}i  |{:.6e}|  per step {:.6}  est {:.3e}",
                r.theta.re,
                r.theta.im,
                r.theta.abs(),
                per_step(r.theta, k),
                r.est
            );
        }
    }
    let sig = sigma_max(&h, M_ARNOLDI);
    println!(
        "{label}: s_max(Hbar) {sig:.6e}, per step {:.6} (a lower bound of the largest singular value of P_k in the scaled norm)",
        sig.powf(1.0 / k as f64)
    );
    let dom = rz[0].clone();
    let (zr, zi) = ritz_vector(&v, &dom);
    let (res, pz) = true_residual(&mut apply, dom.theta, &zr, zi.as_deref());
    let g = per_step(dom.theta, k);
    println!(
        "{label}: dominant mu {:+.6e} {:+.6e}i (|{:.6e}|), per step g = {g:.6}, real {}, true residual {res:.3e}",
        dom.theta.re,
        dom.theta.im,
        dom.theta.abs(),
        is_real(dom.theta)
    );
    let cap = if k == 1 { usize::MAX } else { 6 };
    let mut big = Vec::new();
    let mut unchecked = 0;
    for r in rz.iter().filter(|r| per_step(r.theta, k) >= 1.0) {
        if r.theta == dom.theta {
            big.push((r.theta, res));
            continue;
        }
        if r.theta.im < 0.0 && rz.iter().any(|q| q.theta == r.theta.conj()) {
            continue;
        }
        if big.len() >= cap {
            unchecked += 1;
            continue;
        }
        let (a, b) = ritz_vector(&v, r);
        let (rr, _) = true_residual(&mut apply, r.theta, &a, b.as_deref());
        big.push((r.theta, rr));
    }
    for (t, r) in &big {
        println!(
            "{label}: per step >= 1: {:+.6e} {:+.6e}i (|{:.6e}|, per step {:.6}), true residual {:.3e}",
            t.re,
            t.im,
            t.abs(),
            per_step(*t, k),
            r
        );
    }
    if unchecked > 0 {
        println!("{label}: {unchecked} further Ritz values with per step >= 1 not checked (cap {cap})");
    }
    let g0v = if g0 {
        let p5 = pkv(s, &lay, &base, dt, k, &zr, 1e-5);
        let d: Vec<f64> = pz.iter().zip(&p5).map(|(a, b)| a - b).collect();
        let r = norm(&d) / norm(&pz);
        println!("{label}: G0 |P_1e-4 z - P_1e-5 z| / |P_1e-4 z| = {r:.3e}");
        Some(r)
    } else {
        None
    };
    // The parts of the dominant vector.
    let nk = lay.comps.len();
    let blocks = lay.blocks();
    let sq = |i: usize| zr[i] * zr[i] + zi.as_ref().map_or(0.0, |z| z[i] * z[i]);
    let cell_u = |c: usize| (0..nk).map(|j| sq(blocks[0].start + c * nk + j)).sum::<f64>();
    let total: f64 = (0..lay.len()).map(sq).sum();
    let shares: Vec<f64> = blocks.iter().map(|b| b.clone().map(sq).sum::<f64>() / total).collect();
    let mode_cell = (0..lay.nc).max_by(|&a, &b| cell_u(a).total_cmp(&cell_u(b))).unwrap();
    let pcell = (0..lay.nc)
        .max_by(|&a, &b| sq(blocks[2].start + a).total_cmp(&sq(blocks[2].start + b)))
        .unwrap();
    let cc = geom.mesh.cell_centres();
    let (xm, xp) = (cc[mode_cell], cc[pcell]);
    let (fluid, first) = match s.immersed.as_ref() {
        Some(ib) => fluid_cells(geom, ib),
        None => (vec![true; lay.nc], vec![false; lay.nc]),
    };
    println!(
        "{label}: dominant shares of |z|^2: u_n {:.4}, u_n-1 {:.4}, p {:.4}, phi_n {:.4}, phi_n-1 {:.4}",
        shares[0], shares[1], shares[2], shares[3], shares[4]
    );
    println!(
        "{label}: dominant mode cell (u) {mode_cell} at ({:.5}, {:.5}), fluid {}, first fluid {}, distance to the surface {:.5}; largest |p| cell {pcell} at ({:.5}, {:.5}), fluid {}, distance to the surface {:.5}, distance to the C_max cell {:.5}",
        xm[0],
        xm[1],
        fluid[mode_cell],
        first[mode_cell],
        surface_distance(xm),
        xp[0],
        xp[1],
        fluid[pcell],
        surface_distance(xp),
        (xp[0] - xc[0]).hypot(xp[1] - xc[1])
    );
    s.settings = saved;
    if k > 1 {
        // The nonlinear propagation of e v_1 with the settings of R (printed).
        let e = 1e-4;
        let v1 = lcg_vector(lay.len(), 482);
        let pert: Vec<f64> = v1.iter().map(|x| e * x).collect();
        let mut xs_base = Vec::with_capacity(k);
        s.restore(&base).unwrap();
        for _ in 0..k {
            s.step(dt);
            assert!(s.diverged.is_none());
            xs_base.push(lay.pack(&s.snapshot()));
        }
        s.restore(&lay.perturbed(&base, &pert)).unwrap();
        let mut line = String::new();
        for (j, xb) in xs_base.iter().enumerate() {
            s.step(dt);
            assert!(s.diverged.is_none());
            let xq = lay.pack(&s.snapshot());
            let d: Vec<f64> = xq.iter().zip(xb).map(|(a, b)| a - b).collect();
            line.push_str(&format!(" {}:{:.4e}", j + 1, norm(&d) / e));
        }
        println!("{label}: nonlinear propagation |x'_n - x_n| / e by step:{line}");
    }
    println!("{label}: wall time {:.1} s", t0.elapsed().as_secs_f64());
    s.restore(&base).unwrap();
    Analysis {
        ritz: rz,
        mu: dom.theta,
        mu_res: res,
        g,
        big,
        unchecked,
        g0: g0v,
        cmax,
    }
}

/// G2 to G4: the dominant converged and below one per step.
fn below_one(label: &str, a: &Analysis) -> Option<String> {
    if a.mu_res <= 0.03 && a.g < 1.0 {
        None
    } else {
        Some(format!(
            "{label}: dominant {:?}, per step {:.6}, true residual {:.3e}",
            a.mu, a.g, a.mu_res
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record P1: the bounded form, one corrector (PB: G0, G1, R1) and two correctors (PC: G4)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0405 record P1. Heavy (several minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0405 (PB, PC; G0, G1, G4, R1): run once in release"]
fn record_p1_bounded_one_and_two_correctors() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let mut misses = Vec::new();
    {
        let (geom, mut s) = r_solver(2, Forcing::B, 1);
        run_to("PB", &mut s, dt, 15, 0);
        let pb = analyse("PB", &geom, &mut s, dt, 10, true);
        let mut stop = None;
        for n in 16..=200 {
            s.step(dt);
            if let Some(d) = &s.diverged {
                stop = Some((n, d.step, d.quantity, d.value));
                break;
            }
            println!("PB: step {n} max |u| {:.5}", max_abs_u(&s));
        }
        println!("R1: stop {stop:?}");
        if !matches!(stop, Some((34, 34, "courant", _))) {
            misses.push(format!("R1: stop {stop:?}, registered step 34 by the Courant guard"));
        }
        let g0 = pb.g0.unwrap();
        if g0 > 1e-3 {
            misses.push(format!("G0 PB: {g0:.3e} > 1e-3"));
        }
        if pb.mu_res > 0.03 {
            misses.push(format!("G1 a: true residual {:.3e} > 0.03", pb.mu_res));
        }
        if pb.g <= 1.0 {
            misses.push(format!("G1 b: per step {:.6} <= 1", pb.g));
        }
    }
    {
        let (geom, mut s) = r_solver(2, Forcing::B, 2);
        run_to("PC", &mut s, dt, 15, 0);
        let pc = analyse("PC", &geom, &mut s, dt, 10, false);
        misses.extend(below_one("G4 PC", &pc));
    }
    println!("P1: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record P1: {misses:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record P2: the first-order forcing over 10 and 20 steps (PF: G0, G2; PF20: G3; R2)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0405 record P2. Heavy (several minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0405 (PF, PF20; G0, G2, G3, R2): run once in release"]
fn record_p2_first_order_ten_and_twenty_steps() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let mut misses = Vec::new();
    let (geom, mut s) = r_solver(2, Forcing::F, 1);
    run_to("PF", &mut s, dt, 15, 0);
    let pf = analyse("PF", &geom, &mut s, dt, 10, true);
    let g0 = pf.g0.unwrap();
    if g0 > 1e-3 {
        misses.push(format!("G0 PF: {g0:.3e} > 1e-3"));
    }
    misses.extend(below_one("G2 PF", &pf));
    let pf20 = analyse("PF20", &geom, &mut s, dt, 20, false);
    misses.extend(below_one("G3 PF20", &pf20));
    let mut stopped = None;
    for n in 16..=200 {
        s.step(dt);
        if let Some(d) = &s.diverged {
            stopped = Some((n, d.quantity));
            break;
        }
        if n % 10 == 0 {
            println!("PF: step {n} max |u| {:.5}", max_abs_u(&s));
        }
    }
    let u200 = max_abs_u(&s);
    println!("R2: stop {stopped:?}, max |u| at step 200 {u200:.5}");
    if stopped.is_some() || (u200 - 2.210).abs() > 0.01 {
        misses.push(format!(
            "R2: stop {stopped:?}, max |u| {u200:.5} (registered: runs, 2.210 within 0.01)"
        ));
    }
    println!("P2: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record P2: {misses:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record X: the one-step eigenvalues above one of F (X1 the channel, X2 the channel at the Courant number of F, X3 a later
// state, X4 printed along the window)
// ---------------------------------------------------------------------------------------------------------------------------

/// X2, X3: some converged Ritz value with `abs > 1` and positive real part.
fn has_converged_above_one(a: &Analysis) -> bool {
    a.big.iter().any(|(t, r)| t.abs() > 1.0 && t.re > 0.0 && *r <= 0.03)
}

/// ADR 0405 record X. Heavy (a few minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0405 (X1 to X4): run once in release"]
fn record_x_channel_courant_and_state() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let mut misses = Vec::new();
    // X1: the channel without the cylinder at U_m = 1.5.
    let c0 = {
        let (geom, mut s) = channel_solver(2, 1.5);
        run_to("X1", &mut s, dt, 15, 0);
        let x1 = analyse("X1", &geom, &mut s, dt, 1, false);
        let bad: Vec<_> = x1.big.iter().filter(|(_, r)| *r <= 0.03).collect();
        if !bad.is_empty() {
            misses.push(format!("X1: converged Ritz values with |theta| >= 1: {bad:?}"));
        }
        x1.cmax
    };
    // X4 and X3: F along the window and at step 100.
    let cf = {
        let (geom, mut s) = r_solver(2, Forcing::F, 1);
        run_to("XF", &mut s, dt, 15, 0);
        let (cf, cell) = c_max(&geom, &s, dt);
        println!("XF: C_F {cf:.5} (cell {cell}) after step 15");
        for (from, to) in [(15, 16), (16, 20), (20, 25)] {
            run_to("XF", &mut s, dt, to, from);
            let _ = analyse(&format!("X4 F{to}"), &geom, &mut s, dt, 1, false);
        }
        run_to("XF", &mut s, dt, 100, 25);
        let x3 = analyse("X3 F100", &geom, &mut s, dt, 1, false);
        if !has_converged_above_one(&x3) {
            misses.push(format!(
                "X3: no converged Ritz value above one of positive real part: {:?}",
                x3.big
            ));
        }
        cf
    };
    // X2: the channel at the matched inflow.
    let um = 1.5 * cf / c0;
    println!("X2: C_F {cf:.5}, C_0 {c0:.5}, U_m' = {um:.5}");
    {
        let (geom, mut s) = channel_solver(2, um);
        run_to("X2", &mut s, dt, 15, 0);
        let x2 = analyse("X2", &geom, &mut s, dt, 1, false);
        if !has_converged_above_one(&x2) {
            misses.push(format!(
                "X2: no converged Ritz value above one of positive real part: {:?}",
                x2.big
            ));
        }
    }
    println!("X: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record X: {misses:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Known misses (ADR 0405 addendum 1): the registered limits kept, the measured numbers recorded; not run again (each repeats a
// registered record)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0405 G4 (record P1, run once): for B with two correctors over steps 16 to 25 the dominant Ritz value of `P_10`,
/// 0.925388 +- 0.104328i (per step 0.992903), has the true residual 0.245 against `<= 0.03`: not converged. Limit unchanged.
#[test]
#[ignore = "known miss of ADR 0405 G4 (record P1): two correctors, dominant per step 0.9929 not converged (residual 0.245)"]
fn g4_two_correctors_below_one_per_step_known_miss() {
    record_p1_bounded_one_and_two_correctors();
}

/// ADR 0405 G2 and G3 (record P2, run once): for F with one corrector the dominant eigenvalue of the product is converged and
/// above one per step: -3.856 -+ 14.519i over steps 16 to 25 (per step 1.311215, true residual 2.1e-7) and -3.596 +- 9.255i
/// over steps 16 to 35 (per step 1.121620, 8.8e-7). Limits unchanged.
#[test]
#[ignore = "known miss of ADR 0405 G2, G3 (record P2): first-order forcing per step 1.3112 (k 10) and 1.1216 (k 20)"]
fn g2_g3_first_order_forcing_below_one_per_step_known_miss() {
    record_p2_first_order_ten_and_twenty_steps();
}

/// ADR 0405 X2 and X3 (record X, run once): the channel without the cylinder at `U_m' = 2.906` (`C_max` 1.168) has no
/// converged Ritz value above one (the largest, -1.108 +- 0.069i, true residual 0.059, of negative real part); F after step
/// 100 (`C_max` 1.155) has none of magnitude one or more (dominant -0.970777). Limits unchanged.
#[test]
#[ignore = "known miss of ADR 0405 X2, X3 (record X): no converged positive mode above one at the matched Courant number or at step 100"]
fn x2_x3_courant_hypothesis_known_miss() {
    record_x_channel_courant_and_state();
}
