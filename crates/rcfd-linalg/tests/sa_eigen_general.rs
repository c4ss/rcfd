//! F-414, ADR 0450: the general real eigensolver and the quadratic eigenproblem (checks G1 to G7, Q1 to Q3).

use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::symmetric_eigen;
use rcfd_linalg::eigen_general::{
    general_eigen, general_eigen_with, quadratic_eigen, GeneralEigen, GeneralEigenError, GeneralEigenOptions,
};
use std::f64::consts::PI;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

/// splitmix64 (Steele, Lea, Flood 2014, from memory), as ADR 0448.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [-1, 1).
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (2.0 / 9007199254740992.0) - 1.0
    }
}

fn random(n: usize, seed: u64) -> Vec<f64> {
    let mut r = Rng(seed);
    (0..n * n).map(|_| r.uniform()).collect()
}

/// Each exact root takes the nearest computed value not yet taken; returns the distances.
fn matched(values: &[C64], exact: &[C64]) -> Vec<(usize, f64)> {
    let mut taken = vec![false; values.len()];
    exact
        .iter()
        .map(|&r| {
            let mut best = (usize::MAX, f64::INFINITY);
            for (i, &v) in values.iter().enumerate() {
                let d = (v - r).abs();
                if !taken[i] && d < best.1 {
                    best = (i, d);
                }
            }
            taken[best.0] = true;
            best
        })
        .collect()
}

fn max_of(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0, f64::max)
}

fn print_eig(e: &GeneralEigen) {
    for (i, v) in e.values.iter().enumerate() {
        println!("  value {i}: {v}  residual {:e}", e.residuals[i]);
    }
    println!("  defects {:?}, iterations {}", e.defects, e.iterations);
}

/// `Q D Q^T` with `Q = I - 2 w w^T / w^T w`, `w` from splitmix64.
fn reflected(d: &[f64], n: usize, seed: u64) -> Vec<f64> {
    let mut r = Rng(seed);
    let w: Vec<f64> = (0..n).map(|_| r.uniform()).collect();
    let ww: f64 = w.iter().map(|x| x * x).sum();
    let mut q = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            q[i * n + j] = if i == j { 1.0 } else { 0.0 } - 2.0 * w[i] * w[j] / ww;
        }
    }
    let mut qd = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            qd[i * n + j] = (0..n).map(|k| q[i * n + k] * d[k * n + j]).sum();
        }
    }
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| qd[i * n + k] * q[j * n + k]).sum();
        }
    }
    a
}

#[test]
fn g1_companion() {
    let mut c = Checks::new();
    // p = (x - 1)(x - 2)(x + 3)(x^2 - x + 4.25)(x^2 + 2x + 2), coefficients low to high
    let factors: [&[f64]; 5] = [
        &[-1.0, 1.0],
        &[-2.0, 1.0],
        &[3.0, 1.0],
        &[4.25, -1.0, 1.0],
        &[2.0, 2.0, 1.0],
    ];
    let mut p = vec![1.0];
    for f in factors {
        let mut q = vec![0.0; p.len() + f.len() - 1];
        for (i, a) in p.iter().enumerate() {
            for (j, b) in f.iter().enumerate() {
                q[i + j] += a * b;
            }
        }
        p = q;
    }
    let n = p.len() - 1;
    println!("p = {p:?}");
    let mut a = vec![0.0; n * n];
    for j in 0..n {
        a[j] = -p[n - 1 - j] / p[n];
    }
    for i in 1..n {
        a[i * n + i - 1] = 1.0;
    }
    let e = general_eigen(&a, n).unwrap();
    print_eig(&e);
    let roots = [
        C64::from_real(1.0),
        C64::from_real(2.0),
        C64::from_real(-3.0),
        C64::new(0.5, 2.0),
        C64::new(0.5, -2.0),
        C64::new(-1.0, 1.0),
        C64::new(-1.0, -1.0),
    ];
    let m = matched(&e.values, &roots);
    c.at_most(
        "G1 companion roots max |lambda - r|",
        max_of(m.iter().map(|x| x.1)),
        1e-10,
    );
    c.at_most("G1b companion residuals", max_of(e.residuals.iter().copied()), 1e-10);
    c.finish();
}

#[test]
fn g2_random_order_50_and_g5_cap() {
    let mut c = Checks::new();
    let n = 50;
    let a = random(n, 0x5EED_0414);
    let e = general_eigen(&a, n).unwrap();
    println!("iterations {}", e.iterations);
    c.at_most(
        "G2 max ||A v - lambda v|| / ||A||_F",
        max_of(e.residuals.iter().copied()),
        1e-10,
    );
    let mut pairs = true;
    let mut i = 0;
    let mut complex = 0;
    while i < n {
        if e.values[i].im != 0.0 {
            complex += 2;
            pairs &= i + 1 < n
                && e.values[i + 1] == e.values[i].conj()
                && e.vectors[i + 1].iter().zip(&e.vectors[i]).all(|(a, b)| *a == b.conj());
            i += 2;
        } else {
            i += 1;
        }
    }
    c.holds(
        "G2b 50 values, exact conjugate pairs, no defect",
        e.values.len() == 50 && pairs && e.defects.is_empty(),
        &format!(
            "{} values, {complex} complex, pairs {pairs}, defects {}",
            e.values.len(),
            e.defects.len()
        ),
    );
    let r = general_eigen_with(&a, n, &GeneralEigenOptions::new().max_iterations(3));
    println!("G5 result: {:?}", r.as_ref().map(|_| "Ok").map_err(|e| e.to_string()));
    c.holds(
        "G5 cap 3 is NotConverged",
        matches!(r, Err(GeneralEigenError::NotConverged { iterations: 3, window, .. }) if window.1 > window.0),
        "",
    );
    c.finish();
}

#[test]
fn g3_symmetric_against_jacobi() {
    let mut c = Checks::new();
    let n = 40;
    let b = random(n, 0x5EED_0415);
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = 0.5 * (b[i * n + j] + b[j * n + i]);
        }
    }
    let e = general_eigen(&a, n).unwrap();
    let j = symmetric_eigen(&a, n).unwrap();
    let mut re: Vec<f64> = e.values.iter().map(|v| v.re).collect();
    re.sort_by(f64::total_cmp);
    let scale = max_of(j.values.iter().map(|v| v.abs()));
    let d = max_of(re.iter().zip(&j.values).map(|(a, b)| (a - b).abs())) / scale;
    let im = max_of(e.values.iter().map(|v| v.im.abs())) / scale;
    c.at_most("G3 symmetric: values against Jacobi / max|lambda|", d, 1e-10);
    c.at_most("G3 symmetric: max |Im| / max|lambda|", im, 1e-10);
    c.finish();
}

#[test]
fn g4_jordan_blocks() {
    let mut c = Checks::new();
    // G4a J_4(2)
    let n = 4;
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        a[i * n + i] = 2.0;
        if i + 1 < n {
            a[i * n + i + 1] = 1.0;
        }
    }
    let e = general_eigen(&a, n).unwrap();
    print_eig(&e);
    c.at_most(
        "G4a J_4(2) values",
        max_of(e.values.iter().map(|v| (*v - C64::from_real(2.0)).abs())),
        1e-6,
    );
    c.holds(
        "G4a one defect of 4",
        e.defects.len() == 1 && e.defects[0].indices.len() == 4,
        &format!("{:?}", e.defects),
    );
    // G4b Q blockdiag(J_2(1.5), -1, 3) Q^T
    let mut d = vec![0.0; 16];
    d[0] = 1.5;
    d[1] = 1.0;
    d[5] = 1.5;
    d[10] = -1.0;
    d[15] = 3.0;
    let a = reflected(&d, 4, 0x5EED_0416);
    let e = general_eigen(&a, 4).unwrap();
    print_eig(&e);
    let lam = C64::from_real(1.5);
    let m = matched(&e.values, &[lam, lam]);
    c.at_most("G4b J_2(1.5) values", max_of(m.iter().map(|x| x.1)), 1e-6);
    let mut idx = vec![m[0].0, m[1].0];
    idx.sort();
    c.holds(
        "G4b one defect, the two values",
        e.defects.len() == 1 && e.defects[0].indices == idx,
        &format!("{:?}", e.defects),
    );
    c.at_most(
        "G4b defect mean",
        e.defects.first().map_or(f64::INFINITY, |d| (d.mean - lam).abs()),
        1e-6,
    );
    // G4c Q J_3(-0.5) Q^T
    let mut d = vec![0.0; 9];
    for i in 0..3 {
        d[i * 3 + i] = -0.5;
        if i < 2 {
            d[i * 3 + i + 1] = 1.0;
        }
    }
    let a = reflected(&d, 3, 0x5EED_0417);
    let e = general_eigen(&a, 3).unwrap();
    print_eig(&e);
    let lam = C64::from_real(-0.5);
    println!(
        "G4c individual errors (recorded, not limited): {:?}",
        e.values.iter().map(|v| (*v - lam).abs()).collect::<Vec<_>>()
    );
    c.holds(
        "G4c one defect of 3",
        e.defects.len() == 1 && e.defects[0].indices.len() == 3,
        &format!("{:?}", e.defects),
    );
    c.at_most(
        "G4c defect mean",
        e.defects.first().map_or(f64::INFINITY, |d| (d.mean - lam).abs()),
        1e-6,
    );
    // G4d negative controls
    let dg = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0];
    let e1 = general_eigen(&dg, 3).unwrap();
    let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let e2 = general_eigen(&id, 3).unwrap();
    let mut v1: Vec<f64> = e1.values.iter().map(|v| v.re).collect();
    v1.sort_by(f64::total_cmp);
    let exact =
        v1 == [1.0, 1.0, 2.0] && e1.values.iter().all(|v| v.im == 0.0) && e2.values.iter().all(|v| *v == C64::ONE);
    c.holds(
        "G4d diag(1,1,2), I: no defect, exact values",
        e1.defects.is_empty() && e2.defects.is_empty() && exact,
        &format!("{:?} {:?} {:?} {:?}", e1.values, e1.defects, e2.values, e2.defects),
    );
    c.finish();
}

#[test]
fn g6_cyclic_permutation() {
    let mut c = Checks::new();
    let n = 6;
    let mut a = vec![0.0; n * n];
    for i in 1..n {
        a[i * n + i - 1] = 1.0;
    }
    a[n - 1] = 1.0;
    let e = general_eigen(&a, n).unwrap();
    print_eig(&e);
    let roots: Vec<C64> = (0..n).map(|k| C64::exp_i(2.0 * PI * k as f64 / n as f64)).collect();
    let m = matched(&e.values, &roots);
    c.at_most("G6 sixth roots of unity", max_of(m.iter().map(|x| x.1)), 1e-10);
    c.finish();
}

#[test]
fn g7_refusals_and_q3() {
    let mut c = Checks::new();
    let mut a = vec![1.0; 9];
    a[2 * 3 + 1] = f64::NAN;
    let r = general_eigen(&a, 3);
    c.holds(
        "G7 NaN refused with its position",
        matches!(
            r,
            Err(GeneralEigenError::NonFinite {
                matrix: "A",
                row: 2,
                col: 1
            })
        ),
        &format!("{r:?}"),
    );
    let r = general_eigen(&[1.0; 8], 3);
    c.holds(
        "G7 size refused",
        matches!(r, Err(GeneralEigenError::DimensionMismatch { .. })),
        &format!("{:?}", r.map(|_| ())),
    );
    let m = [1.0, 2.0, 2.0, 1.0];
    let r = quadratic_eigen(&m, &[0.0; 4], &[1.0, 0.0, 0.0, 1.0], 2);
    c.holds(
        "Q3 indefinite M refused",
        matches!(r, Err(GeneralEigenError::MassNotPositiveDefinite { .. })),
        &format!("{:?}", r.map(|_| ())),
    );
    c.finish();
}

#[test]
fn q1_one_oscillator() {
    let mut c = Checks::new();
    let w = 3.0;
    for zeta in [0.05, 0.5, 0.95] {
        let q = quadratic_eigen(&[1.0], &[2.0 * zeta * w], &[w * w], 1).unwrap();
        let wd = w * (1.0 - zeta * zeta).sqrt();
        let exact = [C64::new(-zeta * w, wd), C64::new(-zeta * w, -wd)];
        let m = matched(&q.values, &exact);
        println!("zeta {zeta}: {:?} residuals {:?}", q.values, q.residuals);
        c.at_most(
            &format!("Q1 zeta {zeta}: |lambda - exact| / w"),
            max_of(m.iter().map(|x| x.1)) / w,
            1e-12,
        );
    }
    let q = quadratic_eigen(&[1.0], &[2.0 * w], &[w * w], 1).unwrap();
    println!("zeta 1: {:?} defects {:?}", q.values, q.defects);
    c.at_most(
        "Q1b zeta 1: |lambda + w| / w",
        max_of(q.values.iter().map(|v| (*v - C64::from_real(-w)).abs())) / w,
        1e-6,
    );
    c.holds(
        "Q1b one defect of 2",
        q.defects.len() == 1 && q.defects[0].indices.len() == 2,
        &format!("{:?}", q.defects),
    );
    c.finish();
}

#[test]
fn q2_skew_coupling() {
    let mut c = Checks::new();
    for (k1, k2, g) in [(4.0, 4.0, 1.0), (1.0, 4.0, 0.5)] {
        let m = [1.0, 0.0, 0.0, 1.0];
        let gm = [0.0, g, -g, 0.0];
        let k = [k1, 0.0, 0.0, k2];
        let q = quadratic_eigen(&m, &gm, &k, 2).unwrap();
        let s: f64 = k1 + k2 + g * g;
        let disc = (s * s - 4.0 * k1 * k2).sqrt();
        let w = [((s - disc) / 2.0).sqrt(), ((s + disc) / 2.0).sqrt()];
        let mut up: Vec<C64> = q.values.iter().copied().filter(|v| v.im > 0.0).collect();
        up.sort_by(|a, b| a.im.total_cmp(&b.im));
        println!("k1 {k1} k2 {k2} g {g}: {:?}, closed form {w:?}", q.values);
        let err = if up.len() == 2 {
            max_of(up.iter().zip(&w).map(|(l, w)| (*l - C64::new(0.0, *w)).abs() / w))
        } else {
            f64::INFINITY
        };
        if k1 == k2 {
            let w0 = k1.sqrt();
            let split = [
                (w0 * w0 + g * g / 4.0).sqrt() - g / 2.0,
                (w0 * w0 + g * g / 4.0).sqrt() + g / 2.0,
            ];
            println!("  split form {split:?} (same as {w:?})");
        }
        c.at_most(&format!("Q2 k1 {k1} k2 {k2} g {g}: |lambda - i w| / w"), err, 1e-12);
    }
    c.finish();
}
