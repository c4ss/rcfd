//! F-664 (ADR 0587): the normwise backward error of `nearest_pencil_eigenpairs` on the pencils K1, K2 (of ADR 0580) and K5c.
//! K4: the library's number against the test's recomputation; K5: the stated bound `eta <= rho est + delta`; K6: the rank-one
//! perturbation that attains `eta` (its meaning); K7: a complex pair is returned and every pair is within `2 tolerance
//! max(1, rho)`. Each registered run is made once.

mod f664_pencils;

use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_pencil::{nearest_pencil_eigenpairs, PencilEigen, PencilOptions};
use rcfd_linalg::CsrMatrix;

const TOL: f64 = 1e-10;
const EPS: f64 = 2.22e-16;

fn frobenius(a: &CsrMatrix) -> f64 {
    a.val.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn dense_frobenius(a: &[f64]) -> f64 {
    a.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn dense_apply(a: &[f64], n: usize, x: &[C64]) -> Vec<C64> {
    (0..n)
        .map(|i| {
            let mut s = C64::ZERO;
            for j in 0..n {
                s += x[j].scale(a[i * n + j]);
            }
            s
        })
        .collect()
}

fn cnorm(x: &[C64]) -> f64 {
    x.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt()
}

fn solve(p: &f664_pencils::Pencil) -> PencilEigen {
    nearest_pencil_eigenpairs(&p.a, &p.b, p.count, &PencilOptions::new().shift(p.shift)).unwrap()
}

/// The test's own `eta` from the dense matrices.
fn eta(p: &f664_pencils::Pencil, l: C64, x: &[C64]) -> f64 {
    let ax = dense_apply(&p.a_dense, p.n, x);
    let bx = dense_apply(&p.b_dense, p.n, x);
    let r: Vec<C64> = ax.iter().zip(&bx).map(|(&u, &v)| u - l * v).collect();
    cnorm(&r) / ((dense_frobenius(&p.a_dense) + l.abs() * dense_frobenius(&p.b_dense)) * cnorm(x))
}

#[test]
fn k4_k5_k7_backward_error_of_the_pencils() {
    let mut k4: f64 = 0.0;
    let mut k5: f64 = 0.0;
    let mut k7 = true;
    let mut complex_in_k5c = false;
    for p in f664_pencils::all() {
        let r = solve(&p);
        assert_eq!(r.backward_errors.len(), r.values.len(), "{}: lengths", p.name);
        let (na, nb) = (frobenius(&p.a), frobenius(&p.b));
        let sig = C64::from_real(p.shift);
        let near = r.values.iter().map(|v| (*v - sig).abs()).fold(f64::INFINITY, f64::min);
        for j in 0..r.values.len() {
            let l = r.values[j];
            let e = r.backward_errors[j];
            let t = eta(&p, l, &r.vectors[j]);
            k4 = k4.max((e / t - 1.0).abs());
            let rho = (na + p.shift.abs() * nb) / (na + l.abs() * nb);
            let s = (l - sig).abs() / near;
            let delta = EPS * (100.0 * (r.steps as f64).sqrt() * s + 1e4);
            let q = e / (rho * r.estimates[j] + delta);
            k5 = k5.max(q);
            k7 &= e <= 2.0 * TOL * rho.max(1.0);
            if p.name == "K5c" && l.im.abs() > 1e-6 * l.abs() {
                complex_in_k5c = true;
            }
            println!(
                "{} pair {j}: lambda {l:?}, eta {e:.3e} (test {t:.3e}), est {:.3e}, rho {rho:.3e}, delta {delta:.3e}, \
                 ratio {q:.3e}, residual {:.3e}",
                p.name, r.estimates[j], r.residuals[j]
            );
        }
    }
    println!("K4 max |eta_lib/eta_test - 1|: {k4:.3e} (limit <= 1e-12)");
    println!("K5 max eta/(rho est + delta): {k5:.3e} (limit <= 1)");
    println!("K7 a complex pair in K5c: {complex_in_k5c}; every eta <= 2 tol max(1, rho): {k7}");
    assert!(k4 <= 1e-12, "K4 {k4:e}");
    assert!(k5 <= 1.0, "K5 {k5:e}");
    assert!(complex_in_k5c, "K7: no complex pair in K5c");
    assert!(k7, "K7: a pair above 2 tol max(1, rho)");
}

#[test]
fn k6_rank_one_perturbation_attains_eta() {
    let p = f664_pencils::k5c();
    let n = p.n;
    let r = solve(&p);
    let (alpha, beta) = (dense_frobenius(&p.a_dense), dense_frobenius(&p.b_dense));
    let mut res: f64 = 0.0;
    let mut na_dev: f64 = 0.0;
    let mut nb_dev: f64 = 0.0;
    for j in 0..r.values.len() {
        let l = r.values[j];
        let x = &r.vectors[j];
        let e = r.backward_errors[j];
        let ax = dense_apply(&p.a_dense, n, x);
        let bx = dense_apply(&p.b_dense, n, x);
        let rv: Vec<C64> = ax.iter().zip(&bx).map(|(&u, &v)| u - l * v).collect();
        let s = alpha + l.abs() * beta;
        let xx = cnorm(x).powi(2);
        // dA = -(alpha/s) r x^H / ||x||^2, dB = (beta conj(lambda)/(s |lambda|)) r x^H / ||x||^2 (ADR 0587 arithmetic 1).
        let ca = C64::from_real(-alpha / (s * xx));
        let cb = l.conj().scale(beta / (s * l.abs() * xx));
        let mut da = vec![C64::ZERO; n * n];
        let mut db = vec![C64::ZERO; n * n];
        for i in 0..n {
            for k in 0..n {
                let o = rv[i] * x[k].conj();
                da[i * n + k] = ca * o;
                db[i * n + k] = cb * o;
            }
        }
        let mut pr = vec![C64::ZERO; n];
        for i in 0..n {
            let mut s1 = C64::ZERO;
            for k in 0..n {
                let aik = C64::from_real(p.a_dense[i * n + k]) + da[i * n + k];
                let bik = C64::from_real(p.b_dense[i * n + k]) + db[i * n + k];
                s1 += (aik - l * bik) * x[k];
            }
            pr[i] = s1;
        }
        let rel = cnorm(&pr) / (s * cnorm(x));
        let nda = da.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
        let ndb = db.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
        println!(
            "K6 pair {j}: eta {e:.3e}, perturbed residual {rel:.3e}, ||dA||_F/(eta alpha) {:.15}, ||dB||_F/(eta beta) {:.15}",
            nda / (e * alpha),
            ndb / (e * beta)
        );
        res = res.max(rel);
        na_dev = na_dev.max((nda / (e * alpha) - 1.0).abs());
        nb_dev = nb_dev.max((ndb / (e * beta) - 1.0).abs());
    }
    println!(
        "K6: max perturbed residual {res:.3e} (limit <= 1e-13), max dA dev {na_dev:.3e}, dB dev {nb_dev:.3e} (limits <= 1e-12)"
    );
    assert!(res <= 1e-13, "K6 residual {res:e}");
    assert!(na_dev <= 1e-12, "K6 dA {na_dev:e}");
    assert!(nb_dev <= 1e-12, "K6 dB {nb_dev:e}");
}
