//! F-412 (ADR 0448): the complex symmetric envelope L D L^T of `rcfd_linalg::cskyline`. Checks L1 to L5 of the
//! registration.

use rcfd_linalg::complex::{norm2, C64};
use rcfd_linalg::cskyline::{ComplexCsr, ComplexSkylineError, ComplexSkylineLdlt, ComplexSkylineOptions};
use rcfd_linalg::skyline::{SkylineLdlt, SkylineOrdering};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// splitmix64 (Steele, Lea, Flood 2014, from memory): the registered generator of ADR 0448.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform in [-1, 1).
    fn sym(&mut self) -> f64 {
        2.0 * self.unit() - 1.0
    }
}

fn csr_from_rows(n: usize, rows: Vec<Vec<(usize, C64)>>) -> ComplexCsr {
    let mut row_ptr = vec![0usize];
    let (mut col, mut val) = (Vec::new(), Vec::new());
    for mut r in rows {
        r.sort_by_key(|e| e.0);
        for (c, v) in r {
            col.push(c);
            val.push(v);
        }
        row_ptr.push(col.len());
    }
    ComplexCsr::new(n, row_ptr, col, val).unwrap()
}

fn dense(n: usize, a: &[C64]) -> ComplexCsr {
    let rows = (0..n)
        .map(|r| {
            (0..n)
                .filter(|&c| a[r * n + c] != C64::ZERO)
                .map(|c| (c, a[r * n + c]))
                .collect()
        })
        .collect();
    csr_from_rows(n, rows)
}

fn random_dominant() -> (ComplexCsr, Vec<C64>) {
    let n = 200;
    let mut rng = Rng(0xF412_0448);
    let mut rows: Vec<Vec<(usize, C64)>> = vec![Vec::new(); n];
    let mut off = vec![0.0f64; n];
    for i in 0..n {
        for j in i + 1..n {
            if rng.unit() < 0.05 {
                let v = C64::new(rng.sym(), rng.sym());
                rows[i].push((j, v));
                rows[j].push((i, v));
                off[i] += v.abs();
                off[j] += v.abs();
            }
        }
    }
    for i in 0..n {
        let theta = 2.0 * PI * rng.unit();
        rows[i].push((i, C64::exp_i(theta).scale(off[i] + 1.0)));
    }
    let b: Vec<C64> = (0..n).map(|_| C64::new(rng.sym(), rng.sym())).collect();
    (csr_from_rows(n, rows), b)
}

fn residual(a: &ComplexCsr, x: &[C64], b: &[C64]) -> f64 {
    let ax = a.matvec(x);
    let r: Vec<C64> = b.iter().zip(&ax).map(|(p, q)| *p - *q).collect();
    norm2(&r) / norm2(b)
}

fn laplacian(m: usize) -> CsrMatrix {
    let n = m * m * m;
    let id = |i: usize, j: usize, k: usize| (k * m + j) * m + i;
    let mut row_ptr = vec![0usize];
    let (mut col, mut val) = (Vec::new(), Vec::new());
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let mut r: Vec<(usize, f64)> = vec![(id(i, j, k), 6.0)];
                if i > 0 {
                    r.push((id(i - 1, j, k), -1.0));
                }
                if i + 1 < m {
                    r.push((id(i + 1, j, k), -1.0));
                }
                if j > 0 {
                    r.push((id(i, j - 1, k), -1.0));
                }
                if j + 1 < m {
                    r.push((id(i, j + 1, k), -1.0));
                }
                if k > 0 {
                    r.push((id(i, j, k - 1), -1.0));
                }
                if k + 1 < m {
                    r.push((id(i, j, k + 1), -1.0));
                }
                r.sort_by_key(|e| e.0);
                for (c, v) in r {
                    col.push(c);
                    val.push(v);
                }
                row_ptr.push(col.len());
            }
        }
    }
    CsrMatrix::rect(n, n, row_ptr, col, val)
}

fn natural() -> ComplexSkylineOptions {
    ComplexSkylineOptions {
        ordering: SkylineOrdering::Natural,
        ..ComplexSkylineOptions::default()
    }
}

fn bits(x: &[C64]) -> Vec<(u64, u64)> {
    x.iter().map(|z| (z.re.to_bits(), z.im.to_bits())).collect()
}

#[test]
fn l1_l5_random_complex_dominant() {
    let mut c = Checks::new();
    let (a, b) = random_dominant();
    let stored = a.col.len();
    println!("order 200, {stored} stored entries");
    let f = ComplexSkylineLdlt::factor(&a).unwrap();
    let x = f.solve(&b);
    c.at_most("L1 relative residual", residual(&a, &x, &b), 1e-12);
    let f2 = ComplexSkylineLdlt::factor(&a).unwrap();
    let x2 = f2.solve(&b);
    c.holds(
        "L5 two factorisations bit-equal",
        bits(&x) == bits(&x2) && bits(f.pivots()) == bits(f2.pivots()),
        "solution and pivots",
    );
    c.finish();
}

#[test]
fn l2_real_matrix_equals_real_factor() {
    let mut c = Checks::new();
    let a = laplacian(6);
    let b: Vec<f64> = (0..a.n).map(|i| ((i * 37 % 101) as f64 - 50.0) / 17.0).collect();
    let real = SkylineLdlt::factor(&a).unwrap();
    let xr = real.solve(&b);
    let ac = ComplexCsr::from_real(&a);
    let fc = ComplexSkylineLdlt::factor(&ac).unwrap();
    let bc: Vec<C64> = b.iter().map(|&v| C64::from_real(v)).collect();
    let xc = fc.solve(&bc);
    let xmax = xr.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let diff = xr.iter().zip(&xc).fold(0.0f64, |m, (r, z)| m.max((r - z.re).abs()));
    let imax = xc.iter().fold(0.0f64, |m, z| m.max(z.im.abs()));
    c.at_most("L2 max |x_c - x_r| / max |x_r|", diff / xmax, 1e-13);
    c.holds("L2 max |Im x_c| == 0", imax == 0.0, &format!("{imax:e}"));
    let same = xr.iter().zip(&xc).all(|(r, z)| r.to_bits() == z.re.to_bits());
    println!(
        "L2 (report) real parts bit-equal to the real factor: {same}; orders equal: {}",
        real.order() == fc.order()
    );
    c.finish();
}

#[test]
fn l3_small_pivots_refused_with_the_equation() {
    let mut c = Checks::new();
    let i = C64::I;
    let one = C64::ONE;
    // [[i, 1], [1, -i]]: d_2 = -i - 1/i = 0
    let a = dense(2, &[i, one, one, -i]);
    match ComplexSkylineLdlt::factor_with(&a, &natural()) {
        Err(e @ ComplexSkylineError::Pivot { row: 1, .. }) => {
            let m = e.to_string();
            c.holds("L3 2x2 refused at row 1", m.contains("row 1"), &m);
        }
        other => c.holds("L3 2x2 refused at row 1", false, &format!("{other:?}")),
    }
    // [[1, 0, 1], [0, 2i, 0], [1, 0, 1]]: d_3 = 0
    let z = C64::ZERO;
    let a3 = dense(3, &[one, z, one, z, i.scale(2.0), z, one, z, one]);
    match ComplexSkylineLdlt::factor_with(&a3, &natural()) {
        Err(e @ ComplexSkylineError::Pivot { row: 2, .. }) => {
            let m = e.to_string();
            c.holds("L3 3x3 refused at row 2", m.contains("row 2"), &m);
        }
        other => c.holds("L3 3x3 refused at row 2", false, &format!("{other:?}")),
    }
    // control: [[i, 1], [1, -i + 1e-6]]
    let an = dense(2, &[i, one, one, C64::new(1e-6, -1.0)]);
    match ComplexSkylineLdlt::factor_with(&an, &natural()) {
        Ok(f) => {
            let b = vec![C64::new(1.0, 2.0), C64::new(-0.5, 0.25)];
            let x = f.solve(&b);
            c.at_most("L3n control residual", residual(&an, &x, &b), 1e-10);
        }
        Err(e) => c.holds("L3n control accepted", false, &e.to_string()),
    }
    // min_relative_pivot: [[1, 0.9], [0.9, 1]], |d_2|/s_2 = 0.19/1.81
    let at = dense(2, &[one, C64::from_real(0.9), C64::from_real(0.9), one]);
    let with = |tau: f64| ComplexSkylineOptions {
        min_relative_pivot: tau,
        ..natural()
    };
    let refused = matches!(
        ComplexSkylineLdlt::factor_with(&at, &with(0.2)),
        Err(ComplexSkylineError::Pivot { row: 1, .. })
    );
    let accepted = ComplexSkylineLdlt::factor_with(&at, &with(0.05)).is_ok();
    c.holds(
        "L3t tau 0.2 refused at row 1, tau 0.05 accepted",
        refused && accepted,
        &format!("{refused} {accepted}"),
    );
    c.finish();
}

#[test]
fn l4_invalid_matrices_refused() {
    let mut c = Checks::new();
    let nan = dense(
        2,
        &[C64::ONE, C64::new(f64::NAN, 0.0), C64::new(f64::NAN, 0.0), C64::ONE],
    );
    let r = ComplexSkylineLdlt::factor(&nan);
    c.holds(
        "L4 non-finite refused with row and column",
        matches!(r, Err(ComplexSkylineError::NonFinite { row: 0, col: 1, .. })),
        &format!("{r:?}"),
    );
    let herm = dense(
        2,
        &[
            C64::from_real(2.0),
            C64::new(1.0, 1.0),
            C64::new(1.0, -1.0),
            C64::from_real(2.0),
        ],
    );
    let r = ComplexSkylineLdlt::factor(&herm);
    c.holds(
        "L4 Hermitian refused as unsymmetric at (0, 1)",
        matches!(r, Err(ComplexSkylineError::Unsymmetric { row: 0, col: 1, .. })),
        &format!("{r:?}"),
    );
    let bad = ComplexCsr::new(2, vec![0, 2, 3], vec![1, 0, 1], vec![C64::ONE; 3]);
    c.holds(
        "L4 unsorted columns refused",
        matches!(bad, Err(ComplexSkylineError::Malformed { .. })),
        &format!("{bad:?}"),
    );
    c.finish();
}
