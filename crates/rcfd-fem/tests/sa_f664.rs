//! F-664 (ADR 0587): the normwise backward error of the pencil eigensolver on Beck's column of ADR 0580. B1: every pair of
//! every evaluation of the bisection within `2 tolerance`; B2: the buckling problems of ADR 0580 F6 and F7 likewise; B3: the
//! stated bound `eta <= rho est + delta` on the pencils assembled by the test; B4: the library's number against the test's
//! recomputation from the matrices. Each registered run is made once; a miss stays as an ignored `*_known_miss` test.

mod f664_beck;

use f664_beck::{bisection, p_ref, setup, state};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::follower::{buckling_factors_full, ComplexBuckling, ComplexModes, FollowerPressure};
use rcfd_fem::geometric::geometric_stiffness;
use rcfd_fem::mass::consistent;
use rcfd_fem::nonlinear::{add_triplets, initial_states, tangent, Contribution};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_pencil::{nearest_pencil_eigenpairs, PencilOptions};
use rcfd_linalg::CsrMatrix;

const TOL: f64 = 1e-10;
const EPS: f64 = 2.22e-16;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

fn max_of(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, &x| m.max(x))
}

fn frobenius(a: &CsrMatrix) -> f64 {
    a.val.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn apply(a: &CsrMatrix, x: &[C64]) -> Vec<C64> {
    (0..a.n)
        .map(|r| {
            let mut s = C64::ZERO;
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                s += x[a.col[k]].scale(a.val[k]);
            }
            s
        })
        .collect()
}

fn cnorm(x: &[C64]) -> f64 {
    x.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt()
}

/// The test's own `eta` of a pair of `A x = lambda B x`.
fn eta(a: &CsrMatrix, b: &CsrMatrix, l: C64, x: &[C64]) -> f64 {
    let ax = apply(a, x);
    let bx = apply(b, x);
    let r: Vec<C64> = ax.iter().zip(&bx).map(|(&p, &q)| p - l * q).collect();
    cnorm(&r) / ((frobenius(a) + l.abs() * frobenius(b)) * cnorm(x))
}

fn on_free(free: &[usize], v: &[C64]) -> Vec<C64> {
    free.iter().map(|&e| v[e]).collect()
}

/// B1 and B1n.
fn b1(ck: &mut Checks, runs: &[(f64, ComplexModes)]) {
    let mut worst: f64 = 0.0;
    let mut lens = true;
    let mut worst_res: f64 = 0.0;
    for (p, r) in runs {
        lens &= r.backward_errors.len() == r.eigenvalues.len();
        let e = max_of(&r.backward_errors);
        println!(
            "p = {p:.9}: {} pairs, steps {}, max eta {e:.3e}, max residual {:.3e}",
            r.eigenvalues.len(),
            r.steps,
            max_of(&r.residuals)
        );
        worst = worst.max(e);
        worst_res = worst_res.max(max_of(&r.residuals));
    }
    ck.holds(
        "B1n backward_errors.len() == eigenvalues.len()",
        lens,
        &format!("{} evaluations", runs.len()),
    );
    ck.at_most("B1 max eta over every pair of the bisection", worst, 2.0 * TOL);
    ck.record("B1 max registered residual of the same pairs (ADR 0580 F3)", worst_res);
}

/// B2.
fn b2(ck: &mut Checks, dead: &ComplexBuckling, foll: &ComplexBuckling) {
    for (name, r) in [("dead", dead), ("follower", foll)] {
        println!(
            "B2 {name}: factors {:?}, eta {:?}, residuals {:?}",
            r.factors, r.backward_errors, r.residuals
        );
    }
    ck.holds(
        "B2 lengths",
        dead.backward_errors.len() == dead.factors.len() && foll.backward_errors.len() == foll.factors.len(),
        "",
    );
    let worst = max_of(&dead.backward_errors).max(max_of(&foll.backward_errors));
    ck.at_most("B2 max eta of the F6 and F7 pairs", worst, 2.0 * TOL);
}

/// B3 on one assembled pencil: the direct call reproduces `values` to the bit; `max eta / (rho est + delta)`.
fn b3_one(name: &str, a: &CsrMatrix, b: &CsrMatrix, count: usize, values: &[C64], bits: &mut bool) -> f64 {
    let r = nearest_pencil_eigenpairs(a, b, count, &PencilOptions::new().shift(0.0).tolerance(TOL)).unwrap();
    let same = r.values.len() == values.len()
        && r.values
            .iter()
            .zip(values)
            .all(|(x, y)| x.re.to_bits() == y.re.to_bits() && x.im.to_bits() == y.im.to_bits());
    *bits &= same;
    let (na, nb) = (frobenius(a), frobenius(b));
    let near = r.values.iter().map(|v| v.abs()).fold(f64::INFINITY, f64::min);
    let mut worst: f64 = 0.0;
    for j in 0..r.values.len() {
        let l = r.values[j];
        let rho = na / (na + l.abs() * nb);
        let s = l.abs() / near;
        let delta = EPS * (100.0 * (r.steps as f64).sqrt() * s + 1e4);
        let q = r.backward_errors[j] / (rho * r.estimates[j] + delta);
        println!(
            "B3 {name} pair {j}: eta {:.3e}, est {:.3e}, rho {rho:.3e}, delta {delta:.3e}, ratio {q:.3e}",
            r.backward_errors[j], r.estimates[j]
        );
        worst = worst.max(q);
    }
    worst
}

/// B4 on one pencil: `max |eta_lib/eta_test - 1|` of the library's pairs (modes on the model's equations).
fn b4_one(a: &CsrMatrix, b: &CsrMatrix, free: &[usize], values: &[C64], modes: &[Vec<C64>], lib: &[f64]) -> f64 {
    let mut worst: f64 = 0.0;
    for ((l, m), e) in values.iter().zip(modes).zip(lib) {
        let t = eta(a, b, *l, &on_free(free, m));
        worst = worst.max((e / t - 1.0).abs());
    }
    worst
}

#[test]
fn b1_to_b4_backward_error_on_beck() {
    let mut ck = Checks::new();
    let b = setup();
    let fp = FollowerPressure::new(&b.model, "xmax", p_ref()).unwrap();
    let runs = bisection(&b, &fp);
    b1(&mut ck, &runs);

    let fpd = FollowerPressure::new(&b.dead, "xmax", p_ref()).unwrap();
    let dead = buckling_factors_full(&b.dead, &b.pre, &[], &BucklingOptions::new(2)).unwrap();
    let foll = buckling_factors_full(&b.dead, &b.pre, &[&fpd], &BucklingOptions::new(4)).unwrap();
    b2(&mut ck, &dead, &foll);

    // The reduced pencils as the library assembles them (ADR 0587 note on B3).
    let n = b.model.dof_map().n_equations();
    let zero = vec![0.0; n];
    let fixed: Vec<(usize, f64)> = prescribed_equations(&b.model, &[])
        .unwrap()
        .into_iter()
        .map(|(eq, _)| (eq, 0.0))
        .collect();
    let mr = reduce(&consistent(&b.model).unwrap(), &zero, &fixed).unwrap();
    let free = mr.free().to_vec();
    let mut bits = true;
    let mut b3: f64 = 0.0;
    let mut b4: f64 = 0.0;
    for p in [16.0, 20.0, 24.0] {
        let (u, lf) = state(&b, p);
        let states = initial_states(&b.model).unwrap();
        let mut kt = tangent(&b.model, &u, &states, None).unwrap();
        let mut triplets = Vec::new();
        let mut scratch = vec![0.0; n];
        fp.add_full(&u, lf, &mut scratch, &mut triplets).unwrap();
        add_triplets(&mut kt, &triplets);
        let kr = reduce(&kt, &zero, &fixed).unwrap();
        let r = &runs.iter().find(|(q, _)| *q == p).expect("evaluated").1;
        b3 = b3.max(b3_one(
            &format!("p = {p}"),
            &kr.matrix,
            &mr.matrix,
            6,
            &r.eigenvalues,
            &mut bits,
        ));
        b4 = b4.max(b4_one(
            &kr.matrix,
            &mr.matrix,
            &free,
            &r.eigenvalues,
            &r.modes,
            &r.backward_errors,
        ));
    }
    let fixed_d = prescribed_equations(&b.dead, &[]).unwrap();
    let k = reduce(&stiffness(&b.dead).unwrap(), &zero, &fixed_d).unwrap();
    for (name, with, r) in [("F6 dead", false, &dead), ("F7 follower", true, &foll)] {
        let mut g = geometric_stiffness(&b.dead, &b.pre, None).unwrap();
        if with {
            let kp: Vec<(usize, usize, f64)> = fpd
                .load_stiffness(&zero, 1.0)
                .unwrap()
                .into_iter()
                .map(|(r, c, v)| (r, c, -v))
                .collect();
            add_triplets(&mut g, &kp);
        }
        let mut gr = reduce(&g, &zero, &fixed_d).unwrap().matrix;
        for v in gr.val.iter_mut() {
            *v = -*v;
        }
        let count = if with { 4 } else { 2 };
        b3 = b3.max(b3_one(name, &k.matrix, &gr, count, &r.factors, &mut bits));
        b4 = b4.max(b4_one(
            &k.matrix,
            &gr,
            k.free(),
            &r.factors,
            &r.modes,
            &r.backward_errors,
        ));
    }
    ck.holds(
        "B3 the direct calls reproduce the follower functions' values to the bit",
        bits,
        "",
    );
    ck.at_most("B3 max eta / (rho est + delta)", b3, 1.0);
    ck.at_most("B4 max |eta_lib/eta_test - 1|", b4, 1e-12);
    ck.finish();
}
