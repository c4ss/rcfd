//! F-572 (ADR 0560 D6): the unsymmetric envelope `L D U` factor on its own: K1 the residual on a structurally unsymmetric
//! random matrix, K2 the pivots and the solution against `SkylineLdlt` on a symmetric matrix, K3 the refusals. Each
//! registered run is made once; a miss stays as an ignored `*_known_miss` test with its limit.

use rcfd_linalg::skyline::{PivotRule, SkylineError, SkylineLdlt, SkylineOrdering};
use rcfd_linalg::skyline_lu::EnvelopeLu;
use rcfd_linalg::CsrMatrix;

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

const N: usize = 300;
const HALF_BAND: usize = 12;

/// The dense matrix of D6 (K1): half-bandwidth 12, each off-diagonal entry present with probability 1/2 independently in
/// each triangle, values in (-1, 1), the diagonal `1 + sum |a_ij|` of its row.
fn random_dense(seed: u64) -> Vec<f64> {
    let mut s = seed;
    let mut a = vec![0.0; N * N];
    for i in 0..N {
        for j in i.saturating_sub(HALF_BAND)..(i + HALF_BAND + 1).min(N) {
            if i != j && uniform(&mut s) < 0.5 {
                a[i * N + j] = 2.0 * uniform(&mut s) - 1.0;
            }
        }
    }
    diagonal(&mut a);
    a
}

fn diagonal(a: &mut [f64]) {
    for i in 0..N {
        let s: f64 = (0..N).filter(|&j| j != i).map(|j| a[i * N + j].abs()).sum();
        a[i * N + i] = 1.0 + s;
    }
}

fn csr(a: &[f64]) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..N {
        for j in 0..N {
            if a[i * N + j] != 0.0 {
                t.push((i, j, a[i * N + j]));
            }
        }
    }
    CsrMatrix::from_triplets(N, t)
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn report(name: &str, value: f64, limit: f64, failed: &mut Vec<String>) {
    let ok = value <= limit;
    println!(
        "{name}: {value:.6e} (limit <= {limit:e}) {}",
        if ok { "met" } else { "MISSED" }
    );
    if !ok {
        failed.push(format!("{name}: {value:e} > {limit:e}"));
    }
}

#[test]
fn k1_k2_k3_envelope_lu() {
    let mut failed = Vec::new();
    let seed = 0xF572_0560u64;
    // K1.
    let a = random_dense(seed);
    let unsymmetric = (0..N).any(|i| (0..N).any(|j| (a[i * N + j] == 0.0) != (a[j * N + i] == 0.0)));
    println!("K1: structurally unsymmetric {unsymmetric}");
    let m = csr(&a);
    let mut s = seed ^ 0x5555;
    let b: Vec<f64> = (0..N).map(|_| 2.0 * uniform(&mut s) - 1.0).collect();
    let lu = EnvelopeLu::factor(&m).unwrap();
    let x = lu.solve(&b);
    let mut r = vec![0.0; N];
    m.matvec(&x, &mut r);
    let res = r.iter().zip(&b).fold(0.0f64, |w, (p, q)| w.max((p - q).abs()));
    let norm_a = (0..N)
        .map(|i| (0..N).map(|j| a[i * N + j].abs()).sum::<f64>())
        .fold(0.0f64, f64::max);
    report("K1 residual", res / (norm_a * max_abs(&x)), 1e-13, &mut failed);
    if !unsymmetric {
        failed.push("K1: the pattern is not structurally unsymmetric".to_string());
    }
    // K2.
    let mut sym = vec![0.0; N * N];
    for i in 0..N {
        for j in 0..N {
            sym[i * N + j] = 0.5 * (a[i * N + j] + a[j * N + i]);
        }
    }
    diagonal(&mut sym);
    let ms = csr(&sym);
    let lu = EnvelopeLu::factor(&ms).unwrap();
    let ldlt = SkylineLdlt::factor(&ms).unwrap();
    println!("K2: same order {}", lu.order() == ldlt.order());
    let dp = lu
        .pivots()
        .iter()
        .zip(ldlt.pivots())
        .fold(0.0f64, |w, (p, q)| w.max((p - q).abs() / q.abs()));
    report("K2 pivots", dp, 1e-13, &mut failed);
    let x1 = lu.solve(&b);
    let x2 = ldlt.solve(&b);
    let dx = x1.iter().zip(&x2).fold(0.0f64, |w, (p, q)| w.max((p - q).abs())) / max_abs(&x2);
    report("K2 solutions", dx, 1e-12, &mut failed);
    // K3.
    let singular = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0)]);
    let e = EnvelopeLu::factor_with(&singular, &SkylineOrdering::Natural, PivotRule::NonZero);
    let ok = matches!(e, Err(SkylineError::Pivot { .. }));
    println!(
        "K3a singular refused: {:?} {}",
        e.as_ref().err(),
        if ok { "met" } else { "MISSED" }
    );
    if !ok {
        failed.push("K3a".to_string());
    }
    let neg = CsrMatrix::from_triplets(1, vec![(0, 0, -1.0)]);
    let e1 = EnvelopeLu::factor_with(&neg, &SkylineOrdering::Natural, PivotRule::Positive);
    let e2 = EnvelopeLu::factor_with(&neg, &SkylineOrdering::Natural, PivotRule::NonZero);
    let ok = matches!(e1, Err(SkylineError::Pivot { .. })) && e2.as_ref().is_ok_and(|f| f.negative_pivots() == 1);
    println!(
        "K3b negative pivot: Positive {:?}, NonZero ok {} {}",
        e1.as_ref().err(),
        e2.is_ok(),
        if ok { "met" } else { "MISSED" }
    );
    if !ok {
        failed.push("K3b".to_string());
    }
    let rect = CsrMatrix::rect(1, 2, vec![0, 1], vec![1], vec![1.0]);
    let nonfinite = CsrMatrix::from_triplets(1, vec![(0, 0, f64::NAN)]);
    let ok = matches!(EnvelopeLu::factor(&rect), Err(SkylineError::NotSquare { .. }))
        && matches!(EnvelopeLu::factor(&nonfinite), Err(SkylineError::NonFinite { .. }));
    println!(
        "K3c non-square and non-finite refused {}",
        if ok { "met" } else { "MISSED" }
    );
    if !ok {
        failed.push("K3c".to_string());
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
