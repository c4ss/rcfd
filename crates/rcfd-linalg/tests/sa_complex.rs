//! F-408 (ADR 0440 D9): the complex type of the advanced tier. Checks Z1 to Z3 and the negative control N4.

use rcfd_linalg::complex::{dot_unconj, norm2, C64};
use std::f64::consts::PI;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
    /// `s m 10^k`, `m` in [1, 10), `k` in -8..=8, a random sign.
    fn component(&mut self) -> f64 {
        let m = 1.0 + 9.0 * self.next();
        let k = (self.next() * 17.0).floor() as i32 - 8;
        let s = if self.next() < 0.5 { -1.0 } else { 1.0 };
        s * m * 10f64.powi(k)
    }
}

/// The registered sample of Z1: 200 pairs from the generator and the special pairs.
fn sample() -> Vec<(C64, C64)> {
    let mut g = Lcg(0xF408_0441);
    let mut out: Vec<(C64, C64)> = (0..200)
        .map(|_| {
            (
                C64::new(g.component(), g.component()),
                C64::new(g.component(), g.component()),
            )
        })
        .collect();
    let specials = [
        C64::new(1.0, 0.0),
        C64::new(0.0, 1.0),
        C64::new(-3.0, 4.0),
        C64::new(0.7, 0.7),
        C64::new(0.7, -0.7),
        C64::new(1.0, 1e-20),
        C64::new(1e-20, 1.0),
    ];
    for &a in &specials {
        for &b in &specials {
            out.push((a, b));
        }
    }
    out
}

fn measures(conj: bool) -> (f64, f64) {
    let (mut worst_id, mut worst_div) = (0.0f64, 0.0f64);
    for (a, b) in sample() {
        let bb = if conj { b.conj() } else { b };
        let lhs = (a * b) * bb;
        let rhs = a.scale(b.norm_sqr());
        let rel = (lhs - rhs).abs() / (a.abs() * b.norm_sqr());
        worst_id = worst_id.max(rel);
        worst_div = worst_div.max((a / a - C64::ONE).abs());
        worst_div = worst_div.max((b / b - C64::ONE).abs());
    }
    (worst_id, worst_div)
}

#[test]
fn z1_n4_identities_on_the_sample() {
    let (id, div) = measures(true);
    println!("Z1: max |(a b) conj(b) - a |b|^2| / (|a| |b|^2) = {id:e} (limit 1e-15)");
    println!("Z1: max |a / a - 1| = {div:e} (limit 1e-15)");
    let (bad, _) = measures(false);
    println!("N4: the same with (a b) b: {bad:e} (must exceed 1e-3)");
    assert!(id <= 1e-15, "Z1 identity: {id:e}");
    assert!(div <= 1e-15, "Z1 division: {div:e}");
    assert!(bad > 1e-3, "N4 control: {bad:e}");
}

#[test]
fn z2_print_and_parse_losslessly() {
    let values = [
        0.0,
        -0.0,
        5e-324,
        2.2250738585072014e-308,
        1.7976931348623157e308,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.1,
        1.0 / 3.0,
        -2.5e-7,
    ];
    let mut all: Vec<C64> = sample().into_iter().flat_map(|(a, b)| [a, b]).collect();
    for &re in &values {
        for &im in &values {
            all.push(C64::new(re, im));
        }
    }
    let mut bad = Vec::new();
    for z in &all {
        let s = z.to_string();
        match s.parse::<C64>() {
            Ok(w) if w.re.to_bits() == z.re.to_bits() && w.im.to_bits() == z.im.to_bits() => {}
            other => bad.push(format!("{z:?} -> '{s}' -> {other:?}")),
        }
    }
    println!("Z2: {} of {} round-trip bit for bit", all.len() - bad.len(), all.len());
    let nan = C64::new(f64::NAN, -1.0).to_string().parse::<C64>();
    let nan_ok = matches!(nan, Ok(w) if w.re.is_nan() && w.im == -1.0);
    println!("Z2: NaN -> {nan:?}");
    assert!(bad.is_empty(), "not lossless: {bad:?}");
    assert!(nan_ok, "NaN");
    for s in ["", "1", "1+2", "1+2j", "1++2i", "i"] {
        assert!(s.parse::<C64>().is_err(), "'{s}' must be refused");
    }
}

#[test]
fn z3_unit_and_helpers() {
    let ii = C64::I * C64::I;
    assert_eq!(ii, C64::new(-1.0, 0.0), "I I = -1");
    let e = C64::exp_i(PI / 2.0) - C64::I;
    assert!(e.abs() <= 1e-16, "exp_i(pi/2) - I = {e:?}");
    let x = [C64::new(1.0, 2.0), C64::new(-0.5, 0.25), C64::new(3.0, -1.0)];
    let y = [C64::new(0.5, -1.0), C64::new(2.0, 2.0), C64::new(-1.0, 0.5)];
    let mut s = C64::ZERO;
    for k in 0..3 {
        s += x[k] * y[k];
    }
    assert_eq!(dot_unconj(&x, &y), s);
    let n = (x[0].norm_sqr() + x[1].norm_sqr() + x[2].norm_sqr()).sqrt();
    assert_eq!(norm2(&x).to_bits(), n.to_bits());
    println!("Z3: I I = {ii}, |exp_i(pi/2) - I| = {:e}, dot {s}, norm {n}", e.abs());
}
