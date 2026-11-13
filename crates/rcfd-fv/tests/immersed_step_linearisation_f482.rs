//! F-482 (ADR 0392): the single-corrector mode of the bounded immersed forcing by a finite-difference linearisation of the
//! whole step.
//!
//! ADR 0374: DFG 2D-2 on M2 with `build_second_order_bounded` and one PISO corrector stops at step 34 at `dt = 1e-3` after a
//! growth that alternates from step to step at the body, and runs at `dt = 5e-4` and with two correctors. Here the whole step
//! (velocity, the velocity of the step before, pressure, face flux and the flux of the step before) is linearised about the state
//! of an early step by central finite differences, and the outer part of its spectrum is computed by Arnoldi (ADR 0392 D1 to
//! D4). Heavy records (`#[ignore]`, run once in release): K (the bounded form at `dt = 1e-3` after steps 5, 10, 15; E0, E1, R1)
//! and L (E2 at `dt = 5e-4`, E3 with two correctors, E4 the first-order forcing). Fast checks: U1 (the Hessenberg eigenvalues),
//! U2 (Arnoldi on a map with a known outlier), U3 (the map and its linearisation on M1). The DFG helpers are copies of those of
//! `immersed_single_corrector_f363.rs` (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
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
    let mut a: Vec<Vec<C>> = h
        .iter()
        .map(|r| r.iter().map(|&x| C::new(x, 0.0)).collect())
        .collect();
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
        let p = (k..n)
            .max_by(|&i, &j| m[i][k].abs().total_cmp(&m[j][k].abs()))
            .unwrap();
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
        let k = (0..y.len())
            .max_by(|&i, &j| y[i].abs().total_cmp(&y[j].abs()))
            .unwrap();
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
            let r: f64 = jr
                .iter()
                .zip(zr)
                .map(|(a, b)| (a - theta.re * b).powi(2))
                .sum();
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

/// ADR 0392 U1: the Hessenberg eigenvalues of two 6 x 6 matrices `Q T Q` with known spectra.
#[test]
fn u1_hessenberg_eigenvalues_of_known_matrices() {
    let (a, b) = (0.5, 0.3);
    let (r, ang) = (1.15, 0.9 * std::f64::consts::PI);
    let cases: Vec<(Vec<(usize, usize, f64)>, Vec<C>)> = vec![
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
    s.u.values
        .iter()
        .map(|v| v[0].hypot(v[1]))
        .fold(0.0, f64::max)
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
// One linearisation (D3) and its report
// ---------------------------------------------------------------------------------------------------------------------------

#[allow(dead_code)]
struct Outcome {
    ritz: Vec<Ritz>,
    lam_l: C,
    lam_l_res: f64,
    lam_l_real: bool,
    mode_cell: usize,
    mode_first_fluid: bool,
    /// Ritz values with `Re < 0` and `|theta| >= 1`, with their true residuals.
    big_left: Vec<(C, f64)>,
    /// `J z` (`e = 1e-4`) and the vector `z` of `lambda_L` when it is real.
    jz: Vec<f64>,
    z: Vec<f64>,
}

/// Linearise the step of `s` (its present state) at `dt`; the solver's state and settings are restored afterwards.
fn linearise(label: &str, geom: &FvGeometry, s: &mut PimpleSolver, dt: f64) -> Outcome {
    let t0 = Instant::now();
    let base = s.snapshot();
    let saved = s.settings.clone();
    let lay = Layout::new(geom);
    for k in 0..3 {
        for c in 0..lay.nc {
            assert_eq!(
                base.hist_u[k].old[c].to_bits(),
                base.u[c][k].to_bits(),
                "{label}: hist_u old is not u"
            );
        }
    }
    assert!(
        base.phi_hist
            .old
            .iter()
            .zip(&base.phi)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "{label}: phi_hist old is not phi"
    );
    let x0 = lay.pack(&base);
    // Base step with the settings of R and with the converged solves.
    let s_r = step_map(s, &lay, &base, dt);
    tighten(s);
    let s_0 = step_map(s, &lay, &base, dt);
    let dstep: Vec<f64> = s_0.iter().zip(&x0).map(|(a, b)| a - b).collect();
    let dtol: Vec<f64> = s_r.iter().zip(&s_0).map(|(a, b)| a - b).collect();
    println!(
        "{label}: state dimension {}, |S_R(0) - S(0)| / |S(0) - x_0| = {:.3e}",
        lay.len(),
        norm(&dtol) / norm(&dstep)
    );
    let mut apply = |v: &[f64]| jv(s, &lay, &base, dt, v, EPS);
    let (v, h) = arnoldi(&mut apply, lcg_vector(lay.len(), 482), M_ARNOLDI);
    let mut rz = ritz(&h, M_ARNOLDI);
    rz.sort_by(|a, b| b.theta.abs().total_cmp(&a.theta.abs()));
    println!("{label}: the twelve Ritz values of largest magnitude (theta, |theta|, estimated residual):");
    for r in rz.iter().take(12) {
        println!(
            "{label}:   {:+.6} {:+.6}i  |{:.6}|  est {:.3e}",
            r.theta.re,
            r.theta.im,
            r.theta.abs(),
            r.est
        );
    }
    println!(
        "{label}: overall dominant {:+.6} {:+.6}i (|{:.6}|)",
        rz[0].theta.re,
        rz[0].theta.im,
        rz[0].theta.abs()
    );
    let l = leftmost(&rz).clone();
    let (zr, zi) = ritz_vector(&v, &l);
    let (res, jz) = true_residual(&mut apply, l.theta, &zr, zi.as_deref());
    let mut big_left = Vec::new();
    for r in rz.iter().filter(|r| r.theta.re < 0.0 && r.theta.abs() >= 1.0) {
        if r.theta == l.theta {
            big_left.push((r.theta, res));
            continue;
        }
        if r.theta.im < 0.0 && rz.iter().any(|q| q.theta == r.theta.conj()) {
            // The conjugate partner has the same residual; computed once.
            continue;
        }
        let (a, b) = ritz_vector(&v, r);
        let (rr, _) = true_residual(&mut apply, r.theta, &a, b.as_deref());
        big_left.push((r.theta, rr));
    }
    // The mode cell and the parts of the mode.
    let ib = s.immersed.as_ref().unwrap();
    let (fluid, first) = fluid_cells(geom, ib);
    let nk = lay.comps.len();
    let blocks = lay.blocks();
    let sq = |i: usize| zr[i] * zr[i] + zi.as_ref().map_or(0.0, |z| z[i] * z[i]);
    let cell_u = |c: usize| (0..nk).map(|j| sq(blocks[0].start + c * nk + j)).sum::<f64>();
    let mode_cell = (0..lay.nc)
        .max_by(|&a, &b| cell_u(a).total_cmp(&cell_u(b)))
        .unwrap();
    let total: f64 = (0..lay.len()).map(sq).sum();
    let shares: Vec<f64> = blocks
        .iter()
        .map(|b| b.clone().map(sq).sum::<f64>() / total)
        .collect();
    let u_total: f64 = (0..lay.nc).map(cell_u).sum();
    let u_first: f64 = (0..lay.nc).filter(|&c| first[c]).map(cell_u).sum();
    let x = geom.mesh.cell_centres()[mode_cell];
    println!(
        "{label}: lambda_L {:+.6} {:+.6}i (|{:.6}|), real {}, true residual {:.3e}, estimated {:.3e}",
        l.theta.re,
        l.theta.im,
        l.theta.abs(),
        is_real(l.theta),
        res,
        l.est
    );
    println!(
        "{label}: mode cell {mode_cell} at ({:.5}, {:.5}), fluid {}, first fluid {}, distance to the surface {:.5}",
        x[0],
        x[1],
        fluid[mode_cell],
        first[mode_cell],
        surface_distance(x)
    );
    println!(
        "{label}: shares of |z|^2: u_n {:.4}, u_n-1 {:.4}, p {:.4}, phi_n {:.4}, phi_n-1 {:.4}; u_n share in first fluid cells {:.4} ({} cells)",
        shares[0],
        shares[1],
        shares[2],
        shares[3],
        shares[4],
        u_first / u_total,
        first.iter().filter(|&&b| b).count()
    );
    for (t, r) in &big_left {
        println!(
            "{label}: Re < 0 and |theta| >= 1: {:+.6} {:+.6}i (|{:.6}|), true residual {:.3e}",
            t.re,
            t.im,
            t.abs(),
            r
        );
    }
    println!("{label}: wall time {:.1} s", t0.elapsed().as_secs_f64());
    s.settings = saved;
    s.restore(&base).unwrap();
    Outcome {
        lam_l: l.theta,
        lam_l_res: res,
        lam_l_real: is_real(l.theta),
        ritz: rz,
        mode_cell,
        mode_first_fluid: first[mode_cell],
        big_left,
        jz,
        z: zr,
    }
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
// U3: the map and its linearisation on M1
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0392 U3 on DFG M1, B, one corrector, `dt = 1e-3`, the state after step 2: (a) the map at zero is the step; (b) the
/// linearisation with `e = 1e-4` against `1e-5`; (c) the shift of the history is exact.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn u3_the_step_map_and_its_linearisation_on_m1() {
    let dt = 1e-3;
    let (geom, mut s) = r_solver(1, Forcing::B, 1);
    run_to("U3", &mut s, dt, 2, 0);
    let base = s.snapshot();
    s.step(dt);
    assert!(s.diverged.is_none());
    let direct = s.snapshot();
    let lay = Layout::new(&geom);
    // (a): a fresh solver, restored from the perturbation zero, one step with the settings of R.
    let (_, mut t) = r_solver(1, Forcing::B, 1);
    let zero = vec![0.0; lay.len()];
    t.restore(&lay.perturbed(&base, &zero)).unwrap();
    t.step(dt);
    assert!(t.diverged.is_none());
    let bits = |st: &PimpleState| -> Vec<u64> {
        st.u.iter()
            .flat_map(|v| v.iter().map(|x| x.to_bits()))
            .chain(st.p.iter().map(|x| x.to_bits()))
            .chain(st.phi.iter().map(|x| x.to_bits()))
            .collect()
    };
    assert!(
        bits(&t.snapshot()) == bits(&direct),
        "U3 (a): the map at zero is not the step"
    );
    // (b), (c): the linearisation with the converged solves.
    tighten(&mut t);
    let v = lcg_vector(lay.len(), 482);
    let j4 = jv(&mut t, &lay, &base, dt, &v, 1e-4);
    let j5 = jv(&mut t, &lay, &base, dt, &v, 1e-5);
    let diff: Vec<f64> = j4.iter().zip(&j5).map(|(a, b)| a - b).collect();
    let rel = norm(&diff) / norm(&j4);
    let bl = lay.blocks();
    let shift_u = bl[1]
        .clone()
        .zip(bl[0].clone())
        .map(|(o, i)| (j4[o] - v[i]).abs())
        .fold(0.0, f64::max);
    let shift_f = bl[4]
        .clone()
        .zip(bl[3].clone())
        .map(|(o, i)| (j4[o] - v[i]).abs())
        .fold(0.0, f64::max);
    let vmax = v.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    println!(
        "U3: dimension {}, |J4 v - J5 v| / |J4 v| = {rel:.3e}, history shift error u {:.3e}, phi {:.3e} (max |v| {vmax:.3e}), |J v| {:.4}",
        lay.len(),
        shift_u,
        shift_f,
        norm(&j4)
    );
    assert!(rel <= 1e-3, "U3 (b): {rel:.3e}");
    assert!(
        shift_u <= 1e-9 && shift_f <= 1e-9,
        "U3 (c): shift errors {shift_u:.3e} {shift_f:.3e}"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record K: the bounded form, one corrector, dt 1e-3 (K5, K10, K15; E0, E1, R1)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0392 record K. Heavy (about two minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0392 (K5, K10, K15, E0, E1, R1): run once in release"]
fn record_k_bounded_one_corrector_dt_1e3() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let (geom, mut s) = r_solver(2, Forcing::B, 1);
    run_to("K", &mut s, dt, 5, 0);
    let _k5 = linearise("K5", &geom, &mut s, dt);
    run_to("K", &mut s, dt, 10, 5);
    let _k10 = linearise("K10", &geom, &mut s, dt);
    run_to("K", &mut s, dt, 15, 10);
    let k15 = linearise("K15", &geom, &mut s, dt);
    // E0: J z with e = 1e-5 against e = 1e-4 (z the real vector of lambda_L).
    let e0 = {
        let base = s.snapshot();
        let saved = s.settings.clone();
        let lay = Layout::new(&geom);
        tighten(&mut s);
        let j5 = jv(&mut s, &lay, &base, dt, &k15.z, 1e-5);
        s.settings = saved;
        s.restore(&base).unwrap();
        let d: Vec<f64> = k15.jz.iter().zip(&j5).map(|(a, b)| a - b).collect();
        norm(&d) / norm(&k15.jz)
    };
    println!("E0: |J_1e-4 z - J_1e-5 z| / |J_1e-4 z| = {e0:.3e} (z real: {})", k15.lam_l_real);
    // R1: on with the settings of R to the stop.
    let mut stop = None;
    for n in 16..=200 {
        s.step(dt);
        if let Some(d) = &s.diverged {
            stop = Some((n, d.step, d.quantity, d.value));
            break;
        }
        println!("K: step {n} max |u| {:.5}", max_abs_u(&s));
    }
    println!("R1: stop {stop:?}");
    println!("K: wall time {:.1} s", t0.elapsed().as_secs_f64());
    let others = k15
        .ritz
        .iter()
        .filter(|r| r.theta != k15.lam_l)
        .map(|r| r.theta.abs())
        .fold(0.0, f64::max);
    let mut misses = Vec::new();
    if !matches!(stop, Some((34, 34, "courant", _))) {
        misses.push(format!("R1: stop {stop:?}, registered step 34 by the Courant guard"));
    }
    let e0_ok = e0 <= 1e-3;
    if !e0_ok {
        misses.push(format!("E0: {e0:.3e} > 1e-3"));
    }
    let e1a_ok = k15.lam_l_res <= 0.03;
    if !e1a_ok {
        misses.push(format!("E1 a: true residual {:.3e} > 0.03", k15.lam_l_res));
    }
    if !(k15.lam_l_real && k15.lam_l.re < -1.0) {
        misses.push(format!("E1 b: lambda_L {:?} not real below -1", k15.lam_l));
    }
    let e1c_ok = k15.lam_l.abs() > others;
    if !e1c_ok {
        misses.push(format!(
            "E1 c: |lambda_L| {:.6} not above the other Ritz values ({others:.6})",
            k15.lam_l.abs()
        ));
    }
    if !k15.mode_first_fluid {
        misses.push(format!("E1 d: mode cell {} not a first fluid cell", k15.mode_cell));
    }
    assert!(misses.is_empty(), "record K: {misses:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record L: dt 5e-4 (E2), two correctors (E3), the first-order forcing (E4)
// ---------------------------------------------------------------------------------------------------------------------------

/// E2 to E4: no converged Ritz value of the left half plane of magnitude 1 or more, and `lambda_L` converged inside the circle.
fn left_half_plane_inside(label: &str, o: &Outcome) -> Option<String> {
    let bad: Vec<_> = o.big_left.iter().filter(|(_, r)| *r <= 0.03).collect();
    if !bad.is_empty() {
        return Some(format!("{label}: converged Ritz values with Re < 0 and |theta| >= 1: {bad:?}"));
    }
    let inside = o.lam_l_res <= 0.03 && o.lam_l.abs() < 1.0;
    if !inside {
        return Some(format!(
            "{label}: lambda_L {:?} (|{:.6}|), true residual {:.3e}",
            o.lam_l,
            o.lam_l.abs(),
            o.lam_l_res
        ));
    }
    None
}

/// ADR 0392 record L. Heavy (about two minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0392 (H30, C15, F15; E2, E3, E4): run once in release"]
fn record_l_smaller_step_two_correctors_first_order() {
    let t0 = Instant::now();
    let mut misses = Vec::new();
    {
        let dt = 5e-4;
        let (geom, mut s) = r_solver(2, Forcing::B, 1);
        run_to("H", &mut s, dt, 30, 0);
        let o = linearise("H30", &geom, &mut s, dt);
        misses.extend(left_half_plane_inside("E2", &o));
    }
    {
        let dt = 1e-3;
        let (geom, mut s) = r_solver(2, Forcing::B, 2);
        run_to("C", &mut s, dt, 15, 0);
        let o = linearise("C15", &geom, &mut s, dt);
        misses.extend(left_half_plane_inside("E3", &o));
    }
    {
        let dt = 1e-3;
        let (geom, mut s) = r_solver(2, Forcing::F, 1);
        run_to("F", &mut s, dt, 15, 0);
        let o = linearise("F15", &geom, &mut s, dt);
        misses.extend(left_half_plane_inside("E4", &o));
    }
    println!("L: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record L: {misses:?}");
}
