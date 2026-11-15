//! F-624 (ADR 0566 D1): the analysis of a pattern for the supernodal factor, used again for another matrix of that pattern.
//! Checks A3 (the factor bit for bit that of `SupernodalLdlt::factor`), A4 (the refusals equal), A5 (the order and the
//! refusal of a matrix that is not square) and the negative control N2 (another pattern panics). Every limit is the one
//! registered in ADR 0566.

use rcfd_linalg::skyline::SkylineError;
use rcfd_linalg::sparse_chol::{nested_dissection_order, SupernodalAnalysis, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;

/// The 7-point Dirichlet Laplacian on `m^3` (diagonal 6).
fn lap7(m: usize) -> CsrMatrix {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let p = id(i, j, k);
                t.push((p, p, 6.0));
                let mut nb = |q: usize| t.push((p, q, -1.0));
                if i > 0 {
                    nb(id(i - 1, j, k));
                }
                if i + 1 < m {
                    nb(id(i + 1, j, k));
                }
                if j > 0 {
                    nb(id(i, j - 1, k));
                }
                if j + 1 < m {
                    nb(id(i, j + 1, k));
                }
                if k > 0 {
                    nb(id(i, j, k - 1));
                }
                if k + 1 < m {
                    nb(id(i, j, k + 1));
                }
            }
        }
    }
    CsrMatrix::from_triplets(m * m * m, t)
}

/// ADR 0566 A3's second matrix: every stored entry `k` scaled by `1 + 0.1 sin(k)`, each pair then replaced by its mean.
fn rescaled(a: &CsrMatrix) -> CsrMatrix {
    let mut b = a.clone();
    for (k, v) in b.val.iter_mut().enumerate() {
        *v *= 1.0 + 0.1 * (k as f64).sin();
    }
    let mut out = b.clone();
    for r in 0..b.n {
        for k in b.row_ptr[r]..b.row_ptr[r + 1] {
            let c = b.col[k];
            let row = &b.col[b.row_ptr[c]..b.row_ptr[c + 1]];
            let m = b.row_ptr[c] + row.binary_search(&r).expect("symmetric pattern");
            out.val[k] = 0.5 * (b.val[k] + b.val[m]);
        }
    }
    out
}

fn rhs(n: usize) -> Vec<f64> {
    (0..n).map(|i| ((i + 1) as f64).sin()).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// A3: `factor_analysed(b, analysis of a)` against `factor(b)`, every reported quantity.
fn a3(name: &str, a: &CsrMatrix, b: &CsrMatrix) -> Vec<String> {
    let analysis = SupernodalAnalysis::nested_dissection(a).expect("square");
    let x = SupernodalLdlt::factor_analysed(b, &analysis).expect("positive definite");
    let y = SupernodalLdlt::factor(b).expect("positive definite");
    let mut missed = Vec::new();
    let mut check = |what: &str, ok: bool| {
        println!("A3 {name}: {what} {}", if ok { "equal" } else { "MISSED" });
        if !ok {
            missed.push(format!("A3 {name}: {what}"));
        }
    };
    check("order", x.order() == y.order());
    check("pivots (bits)", bits(x.pivots()) == bits(y.pivots()));
    let columns = (0..b.n).all(|j| {
        let (rx, vx) = x.column(j);
        let (ry, vy) = y.column(j);
        rx == ry && bits(vx) == bits(vy)
    });
    check("every column of L (rows, bits)", columns);
    check("solve (bits)", bits(&x.solve(&rhs(b.n))) == bits(&y.solve(&rhs(b.n))));
    check(
        "factor_entries, supernodes, max_front, split_fronts, peak_front_bytes",
        (
            x.factor_entries(),
            x.supernodes(),
            x.max_front(),
            x.split_fronts(),
            x.peak_front_bytes(),
        ) == (
            y.factor_entries(),
            y.supernodes(),
            y.max_front(),
            y.split_fronts(),
            y.peak_front_bytes(),
        ),
    );
    println!(
        "A3 {name}: n {}, nnz(L) {}, supernodes {}, split fronts {}",
        b.n,
        y.factor_entries(),
        y.supernodes(),
        y.split_fronts()
    );
    missed
}

/// A3 of ADR 0566 on `lap7(10)` and on `lap7(24)` (whose largest fronts are split over the pool), each with the same matrix
/// and with the rescaled one of the same pattern.
#[test]
fn f624_a3_analysed_factor_bit_for_bit() {
    let mut missed = Vec::new();
    for m in [10usize, 24] {
        let k = lap7(m);
        let k2 = rescaled(&k);
        missed.extend(a3(&format!("lap7({m}), the same matrix"), &k, &k));
        missed.extend(a3(&format!("lap7({m}), the rescaled matrix"), &k, &k2));
    }
    assert!(missed.is_empty(), "missed: {missed:?}");
}

/// A4 of ADR 0566: the refusals of `factor_analysed` with the analysis of `lap7(10)` equal those of `factor`.
#[test]
fn f624_a4_refusals_equal() {
    let k = lap7(10);
    let analysis = SupernodalAnalysis::nested_dissection(&k).expect("square");
    let mut shifted = k.clone();
    for r in 0..shifted.n {
        for q in shifted.row_ptr[r]..shifted.row_ptr[r + 1] {
            if shifted.col[q] == r {
                shifted.val[q] -= 7.0;
            }
        }
    }
    let mut nan = k.clone();
    nan.val[17] = f64::NAN;
    let mut unsym = k.clone();
    let q = (unsym.row_ptr[40]..unsym.row_ptr[41])
        .find(|&q| unsym.col[q] != 40)
        .expect("an off-diagonal entry");
    unsym.val[q] += 1e-3;
    let mut missed = Vec::new();
    for (name, a) in [("indefinite", &shifted), ("NaN", &nan), ("unsymmetric", &unsym)] {
        let x = SupernodalLdlt::factor_analysed(a, &analysis).err();
        let y = SupernodalLdlt::factor(a).err();
        println!("A4 {name}: analysed {x:?}, factor {y:?}");
        let kind_ok = match name {
            "indefinite" => matches!(y, Some(SkylineError::Pivot { .. })),
            "NaN" => matches!(y, Some(SkylineError::NonFinite { .. })),
            _ => matches!(y, Some(SkylineError::Unsymmetric { .. })),
        };
        if x != y || !kind_ok {
            missed.push(name);
        }
    }
    assert!(missed.is_empty(), "A4 missed: {missed:?}");
}

/// A5 of ADR 0566: the analysis' order is `nested_dissection_order`; a matrix that is not square is refused.
#[test]
fn f624_a5_order_and_not_square() {
    let k = lap7(10);
    let analysis = SupernodalAnalysis::nested_dissection(&k).expect("square");
    assert_eq!(
        analysis.order(),
        nested_dissection_order(&k).as_slice(),
        "A5: the order"
    );
    assert_eq!(analysis.n(), k.n);
    assert!(
        analysis.matches(&k) && analysis.matches(&rescaled(&k)),
        "A5: the same pattern matches"
    );
    let r = CsrMatrix::rect(2, 3, vec![0, 1, 2], vec![0, 1], vec![1.0, 1.0]);
    assert_eq!(
        SupernodalAnalysis::nested_dissection(&r).err(),
        Some(SkylineError::NotSquare { rows: 2, cols: 3 }),
        "A5: not square"
    );
}

/// N2 of ADR 0566: a matrix with the same dimension and number of entries as the analysed one but one pair moved panics.
#[test]
#[should_panic(expected = "the analysis of another pattern")]
fn f624_n2_another_pattern_panics() {
    let k = lap7(4);
    let analysis = SupernodalAnalysis::nested_dissection(&k).expect("square");
    let mut t = Vec::new();
    for r in 0..k.n {
        for q in k.row_ptr[r]..k.row_ptr[r + 1] {
            let c = k.col[q];
            let (r2, c2) = match (r, c) {
                (0, 1) => (0, 5),
                (1, 0) => (5, 0),
                x => x,
            };
            t.push((r2, c2, k.val[q]));
        }
    }
    let moved = CsrMatrix::from_triplets(k.n, t);
    assert_eq!((moved.n, moved.col.len()), (k.n, k.col.len()), "the same n and entries");
    assert!(!analysis.matches(&moved), "another pattern");
    let _ = SupernodalLdlt::factor_analysed(&moved, &analysis);
}
