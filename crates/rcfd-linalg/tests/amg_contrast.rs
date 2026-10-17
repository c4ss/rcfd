//! ADR 0052 A1: AMG-preconditioned CG on a two-density pressure-type matrix (5-point finite-volume Poisson on a 120 x 40 grid,
//! cell coefficient 1 below the interface and 1000 above, harmonic face mean, all-Neumann with the reference-cell diagonal
//! doubling used by the pressure equation). The test records iterations and the symmetry/positivity of the preconditioner.

use rcfd_linalg::{cg, Amg, AmgSettings, CsrMatrix, Identity, Preconditioner, SolveControl};

const NX: usize = 120;
const NY: usize = 40;

/// Cell coefficient field: `lo` below `y_if(x)`, `hi` above.
fn matrix(scale: f64, lo: f64, hi: f64, y_if: &dyn Fn(usize) -> f64, anchor: usize) -> CsrMatrix {
    let k = |i: usize, j: usize| if (j as f64 + 0.5) < y_if(i) { lo } else { hi };
    let id = |i: usize, j: usize| j * NX + i;
    let mut t: Vec<(usize, usize, f64)> = Vec::new();
    for j in 0..NY {
        for i in 0..NX {
            let c = id(i, j);
            let mut d = 0.0;
            for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (ii, jj) = (i as i64 + di, j as i64 + dj);
                if ii < 0 || jj < 0 || ii >= NX as i64 || jj >= NY as i64 {
                    continue;
                }
                let (ii, jj) = (ii as usize, jj as usize);
                let (a, b) = (k(i, j), k(ii, jj));
                let g = scale * 2.0 * a * b / (a + b);
                t.push((c, id(ii, jj), -g));
                d += g;
            }
            if c == anchor {
                d *= 2.0;
            }
            t.push((c, c, d));
        }
    }
    CsrMatrix::from_triplets(NX * NY, t)
}

fn rhs() -> Vec<f64> {
    // smooth, compatible right-hand side (zero mean) like a divergence field
    let mut b: Vec<f64> = (0..NX * NY)
        .map(|c| {
            let (i, j) = (c % NX, c / NX);
            ((i as f64) * 0.13).sin() * ((j as f64) * 0.37).cos() + 0.3 * ((i * 7 + j * 3) % 11) as f64 / 11.0
        })
        .collect();
    let m = b.iter().sum::<f64>() / b.len() as f64;
    b.iter_mut().for_each(|v| *v -= m);
    b
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn run(name: &str, a: &CsrMatrix) -> (usize, usize) {
    let b = rhs();
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-10,
        max_iter: 3000,
    };
    let amg = Amg::new(a, AmgSettings::default());
    let mut x = vec![0.0; b.len()];
    let s = cg(a, &b, &mut x, &amg, &ctl);
    let mut x2 = vec![0.0; b.len()];
    let s2 = cg(a, &b, &mut x2, &Identity, &ctl);
    // symmetry and positivity of the preconditioner on pseudo-random vectors
    let mut worst_sym: f64 = 0.0;
    let mut min_pos = f64::INFINITY;
    for seed in 0..5u64 {
        let v: Vec<f64> = (0..b.len())
            .map(|i| (((i as u64 + 1) * (2654435761 + seed * 40503)) % 10007) as f64 / 10007.0 - 0.5)
            .collect();
        let w: Vec<f64> = (0..b.len())
            .map(|i| (((i as u64 + 3) * (97531 + seed * 7919)) % 10009) as f64 / 10009.0 - 0.5)
            .collect();
        let (mut mv, mut mw) = (vec![0.0; b.len()], vec![0.0; b.len()]);
        amg.apply(&v, &mut mv);
        amg.apply(&w, &mut mw);
        let (s1, s2_) = (dot(&w, &mv), dot(&v, &mw));
        worst_sym = worst_sym.max((s1 - s2_).abs() / (s1.abs() + s2_.abs() + 1e-300));
        min_pos = min_pos.min(dot(&v, &mv) / dot(&v, &v));
    }
    println!(
        "{name}: levels {} complexity {:.2} | AMG-CG {} it (res {:.2e} -> {:.2e}, conv {}) | CG {} it (conv {}) | M asym {worst_sym:.1e} min v'Mv/v'v {min_pos:.2e}",
        amg.n_levels(),
        amg.operator_complexity(),
        s.iterations,
        s.initial_residual,
        s.final_residual,
        s.converged,
        s2.iterations,
        s2.converged
    );
    assert!(x.iter().all(|v| v.is_finite()), "{name}: non-finite AMG-CG solution");
    (s.iterations, s2.iterations)
}

#[test]
fn contrast_matrices() {
    let flat = |h: f64| move |_i: usize| h;
    for (name, scale, lo, hi, anchor) in [
        ("1:1000 flat, anchor 0", 1.5e-6, 1.0, 1000.0, 0),
        ("1:1000 flat, anchor top", 1.5e-6, 1.0, 1000.0, NX * NY - 1),
        ("1000:1 flat (water below)", 1.5e-6, 1000.0, 1.0, 0),
        ("1:1 control", 1.5e-6, 1.0, 1.0, 0),
    ] {
        let a = matrix(scale, lo, hi, &flat(20.0), anchor);
        run(name, &a);
    }
    // interface cutting through rows / slanted / thin film
    let a = matrix(1.5e-6, 1000.0, 1.0, &|i| 10.0 + 0.1 * i as f64, 0);
    run("1000:1 slanted", &a);
    let a = matrix(1.5e-6, 1000.0, 1.0, &|i| 10.0 + 6.0 * ((i as f64) * 0.2).sin(), 0);
    run("1000:1 wavy", &a);
    let a = matrix(1.5e-6, 1000.0, 1.0, &flat(1.0), 0);
    run("1000:1 one-row film", &a);
}

/// ADR 0052 A2/A3: a node with zero row sum whose every coupling is below the strength threshold has the filtered diagonal
/// `d_i + sum_weak a_ij = 0`; the Jacobi smoothing of the tentative prolongator divides by it (found on a coarse level of the
/// dam-break pressure matrix at step 120). Deterministic construction: a chain of 200 heavy nodes (coupling 1000) and one light
/// node coupled with 1 to twenty of them (strength ratio 1/sqrt(20 * 2000) = 0.005 < theta = 0.08).
fn weak_row_matrix() -> CsrMatrix {
    let heavy = 200;
    let c = heavy;
    let mut diag = vec![0.0; heavy + 1];
    let mut t: Vec<(usize, usize, f64)> = Vec::new();
    let mut link = |i: usize, j: usize, g: f64, t: &mut Vec<(usize, usize, f64)>| {
        t.push((i, j, -g));
        t.push((j, i, -g));
        diag[i] += g;
        diag[j] += g;
    };
    for i in 0..heavy - 1 {
        link(i, i + 1, 1000.0, &mut t);
    }
    for k in 0..20 {
        link(c, 3 + 9 * k, 1.0, &mut t);
    }
    diag[0] *= 2.0; // anchor: the matrix is nonsingular
    for (i, d) in diag.iter().enumerate() {
        t.push((i, i, *d));
    }
    CsrMatrix::from_triplets(heavy + 1, t)
}

#[test]
fn weak_row_has_finite_hierarchy() {
    let a = weak_row_matrix();
    let n = a.n;
    let b: Vec<f64> = (0..n).map(|i| ((i * 37 % 11) as f64) - 5.0).collect();
    let amg = Amg::new(&a, AmgSettings::default());
    let mut z = vec![0.0; n];
    amg.apply(&b, &mut z);
    assert!(z.iter().all(|v| v.is_finite()), "non-finite preconditioner output");
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-10,
        max_iter: 200,
    };
    let mut x = vec![0.0; n];
    let s = cg(&a, &b, &mut x, &amg, &ctl);
    println!(
        "weak-row matrix: {} levels, AMG-CG {} it, conv {}",
        amg.n_levels(),
        s.iterations,
        s.converged
    );
    assert!(
        s.converged && s.iterations <= 60,
        "AMG-CG {} it conv {}",
        s.iterations,
        s.converged
    );
}
