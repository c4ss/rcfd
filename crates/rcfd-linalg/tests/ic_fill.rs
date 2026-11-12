//! ADR 0357 (F-460): incomplete Cholesky with fill levels and the permuted preconditioner on small matrices with closed
//! forms. Checks P2, P3, P4 and L2 of the registration.

use rcfd_linalg::precond::{ic_fill_pattern, IcOptions, IncompleteCholesky, Permuted};
use rcfd_linalg::{CsrMatrix, Preconditioner};

fn dense_csr(n: usize, a: &[f64]) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if a[i * n + j] != 0.0 {
                t.push((i, j, a[i * n + j]));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn apply(p: &dyn Preconditioner, r: &[f64]) -> Vec<f64> {
    let mut z = vec![0.0; r.len()];
    p.apply(r, &mut z);
    z
}

fn mv(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    let m = b.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    a.iter().zip(b).fold(0.0f64, |s, (p, q)| s.max((p - q).abs())) / m
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn k4() -> CsrMatrix {
    #[rustfmt::skip]
    let a = [
        3.0, -2.0, 0.0, 2.0,
        -2.0, 3.0, -2.0, 0.0,
        0.0, -2.0, 3.0, -2.0,
        2.0, 0.0, -2.0, 3.0,
    ];
    dense_csr(4, &a)
}

fn a6() -> CsrMatrix {
    let n = 6;
    let g: Vec<f64> = (0..n * n)
        .map(|k| ((k / n) as f64 + 2.0 * (k % n) as f64 + 1.0).sin())
        .collect();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| g[i * n + k] * g[j * n + k]).sum::<f64>() + if i == j { 6.0 } else { 0.0 };
        }
    }
    dense_csr(n, &a)
}

fn five_point(nx: usize, ny: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = i + nx * j;
            t.push((c, c, 4.0));
            if i + 1 < nx {
                t.push((c, c + 1, -1.0));
                t.push((c + 1, c, -1.0));
            }
            if j + 1 < ny {
                t.push((c, c + nx, -1.0));
                t.push((c + nx, c, -1.0));
            }
        }
    }
    CsrMatrix::from_triplets(nx * ny, t)
}

#[test]
fn p2_level_one_is_exact_on_k4() {
    let a = k4();
    let p = IncompleteCholesky::with_fill(&a, 1, &IcOptions::default()).unwrap();
    let want = [3.0, 5.0 / 3.0, 3.0 / 5.0, 1.0 / 3.0];
    let e_d = max_rel(p.pivots(), &want);
    let x = [1.0, 2.0, 3.0, 4.0];
    let e_x = max_rel(&apply(&p, &mv(&a, &x)), &x);
    println!(
        "P2 IC(1) of K4: shift {}, {} breakdowns, {} fill entries, pivots {:?} ({e_d:.3e} from the closed form), \
         |M^-1 A x - x| {e_x:.3e} (limits 1e-14)",
        p.shift(),
        p.breakdowns().len(),
        p.fill_entries(),
        p.pivots()
    );
    assert!(p.breakdowns().is_empty() && p.shift() == 0.0, "P2 no breakdown");
    assert!(e_d <= 1e-14 && e_x <= 1e-14, "P2");
}

#[test]
fn p3_permuted_full_pattern_is_exact() {
    let a = a6();
    let order = vec![3, 0, 5, 1, 4, 2];
    let b = a.permute_symmetric(&order);
    let p = Permuted::new(order, IncompleteCholesky::new(&b, &IcOptions::default()).unwrap());
    let x: Vec<f64> = (0..6).map(|i| 1.0 + i as f64).collect();
    let z = apply(&p, &mv(&a, &x));
    let e = max_rel(&z, &x);
    println!("P3 permuted full pattern: max |z - x| / max |x| = {e:.3e} (limit 1e-12)");
    assert!(e <= 1e-12, "P3");
}

#[test]
fn p4_high_levels_and_refusals() {
    let (r1, c1) = ic_fill_pattern(&k4(), 1);
    let (r3, c3) = ic_fill_pattern(&k4(), 3);
    println!("P4 K4 level 1 {:?} {:?}, level 3 {:?} {:?}", r1, c1, r3, c3);
    assert_eq!((r1, c1), (r3, c3), "P4 K4 level 3 equals level 1");
    let (r5, c5) = ic_fill_pattern(&a6(), 5);
    let full_r: Vec<usize> = (0..=6usize).map(|i| i * i.saturating_sub(1) / 2).collect();
    let full_c: Vec<usize> = (0..6).flat_map(|i| 0..i).collect();
    println!("P4 A6 level 5 {:?} ({} entries)", r5, c5.len());
    assert_eq!((r5, c5), (full_r, full_c), "P4 A6 full pattern");
    let o = IcOptions::default();
    let cases = [
        dense_csr(2, &[1.0, 0.5, 0.5, 0.0]),
        CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, 0.0)]),
        CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 0, f64::NAN), (0, 1, f64::NAN), (1, 1, 1.0)]),
        CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 0, 0.1), (0, 1, 0.1)]),
    ];
    for m in &cases {
        let e0 = IncompleteCholesky::new(m, &o).unwrap_err();
        let e1 = IncompleteCholesky::with_fill(m, 1, &o).unwrap_err();
        println!("P4 refusal: new '{e0}', with_fill '{e1}'");
        assert_eq!(e0, e1, "P4 same refusal");
    }
}

#[test]
fn l2_level_zero_is_new_bit_for_bit() {
    for (tag, a) in [("K4", k4()), ("five-point 12 x 10", five_point(12, 10))] {
        let o = IcOptions::default();
        let p = IncompleteCholesky::new(&a, &o).unwrap();
        let q = IncompleteCholesky::with_fill(&a, 0, &o).unwrap();
        let r: Vec<f64> = (0..a.n).map(|i| (i as f64).sin()).collect();
        let same = bits(p.pivots()) == bits(q.pivots())
            && p.shift().to_bits() == q.shift().to_bits()
            && p.breakdowns() == q.breakdowns()
            && p.fill_entries() == q.fill_entries()
            && bits(&apply(&p, &r)) == bits(&apply(&q, &r));
        println!(
            "L2 {tag}: level 0 against new bit-equal {same} (shift {}, {} breakdowns, {} entries)",
            q.shift(),
            q.breakdowns().len(),
            q.fill_entries()
        );
        assert!(same, "L2 {tag}");
    }
}
