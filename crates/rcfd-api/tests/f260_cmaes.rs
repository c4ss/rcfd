//! ADR 0222 (F-260): CMA-ES (`design::cmaes`, `Method::CmaEs`). K: the parts against their closed forms and the tutorial's
//! formulas written out again; C and N: the acceptance (the shifted Rastrigin function from a start where Nelder-Mead stays in a
//! local minimum) with its set-up checks and records; U: unimodal checks and records; S: the study path. Limits are those
//! registered in ADR 0222 before the code.

use rcfd_api::design::cmaes::{self, History, Options, Params, RunInfo, State, Termination};
use rcfd_api::design::optim::{self, Coefficients, Fitness, InitialSimplex, NmOptions, Stop};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::store::{CondResult, Record, Store};
use rcfd_api::design::study::{
    Condition, Evaluator, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study, StudyDef,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The target of the acceptance (Auger and Hansen 2005).
const TARGET: f64 = 1e-8;

// ------------------------------------------------------------------ problems

/// `o_i = 1.5 cos(i)`, `i = 1..d`.
fn shift(d: usize) -> Vec<f64> {
    (1..=d).map(|i| 1.5 * (i as f64).cos()).collect()
}

/// `x0_i = o_i + 3 (-1)^(i+1)`.
fn start(d: usize) -> Vec<f64> {
    shift(d)
        .iter()
        .enumerate()
        .map(|(k, o)| o + if k % 2 == 0 { 3.0 } else { -3.0 })
        .collect()
}

fn rastrigin(x: &[f64], o: &[f64]) -> f64 {
    10.0 * x.len() as f64
        + x.iter()
            .zip(o)
            .map(|(v, s)| {
                let t = v - s;
                t * t - 10.0 * (2.0 * std::f64::consts::PI * t).cos()
            })
            .sum::<f64>()
}

fn r4(x: &[f64]) -> f64 {
    rastrigin(x, &shift(4))
}

fn r10(x: &[f64]) -> f64 {
    rastrigin(x, &shift(10))
}

fn sphere(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum()
}

fn ellipsoid(x: &[f64]) -> f64 {
    let d = x.len();
    x.iter()
        .enumerate()
        .map(|(i, v)| 10f64.powf(6.0 * i as f64 / (d - 1) as f64) * v * v)
        .sum()
}

fn bounded_sphere(x: &[f64]) -> f64 {
    x.iter().map(|v| (v - 6.0) * (v - 6.0)).sum()
}

fn rosenbrock(x: &[f64]) -> f64 {
    100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)
}

/// A batch evaluator of `f` that asserts that every point lies in the box (S6) and counts the points; `first` is the count at
/// the first value at or below the target.
struct Probe<'a> {
    f: &'a dyn Fn(&[f64]) -> f64,
    b: &'a [(f64, f64)],
    calls: usize,
    first: Option<usize>,
}

impl<'a> Probe<'a> {
    fn new(f: &'a dyn Fn(&[f64]) -> f64, b: &'a [(f64, f64)]) -> Self {
        Probe {
            f,
            b,
            calls: 0,
            first: None,
        }
    }

    fn batch(&mut self, xs: &[Vec<f64>]) -> Result<Vec<Fitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                for (v, &(lo, hi)) in x.iter().zip(self.b) {
                    assert!(*v >= lo && *v <= hi, "S6: {v} outside [{lo}, {hi}]");
                }
                self.calls += 1;
                let f = (self.f)(x);
                if f <= TARGET && self.first.is_none() {
                    self.first = Some(self.calls);
                }
                Fitness::new(f, 0.0)
            })
            .collect())
    }
}

fn cma(
    f: &dyn Fn(&[f64]) -> f64,
    b: &[(f64, f64)],
    o: &Options,
) -> (optim::OptResult, Vec<RunInfo>, usize, Option<usize>) {
    let mut p = Probe::new(f, b);
    let (r, runs) = cmaes::run_report(b, o, &mut |xs| p.batch(xs)).unwrap();
    (r, runs, p.calls, p.first)
}

fn opts(max_evals: usize, restarts: usize, seed: u64, x0: Option<Vec<f64>>) -> Options {
    Options {
        max_evals,
        pop: None,
        sigma0: 0.3,
        restarts,
        seed,
        x0,
    }
}

// ------------------------------------------------------------------ K: the parts

#[test]
fn k0_problem_code() {
    let (o4, o10) = (shift(4), shift(10));
    let checks = [
        ("R4(o)", r4(&o4), 0.0),
        ("R4(x0)", r4(&start(4)), 36.0),
        ("R10(x0)", r10(&start(10)), 90.0),
        ("R4(o + e1)", r4(&[o4[0] + 1.0, o4[1], o4[2], o4[3]]), 1.0),
        ("R10(o)", r10(&o10), 0.0),
        ("E(e10)", ellipsoid(&[0., 0., 0., 0., 0., 0., 0., 0., 0., 1.]), 1e6),
        ("E(e1)", ellipsoid(&[1., 0., 0., 0., 0., 0., 0., 0., 0., 0.]), 1.0),
    ];
    for (name, got, want) in checks {
        println!("K0 {name} = {got:.17e} (want {want})");
        assert!(
            (got - want).abs() <= 1e-12 * want.abs().max(1.0),
            "K0 {name}: {got} for {want}"
        );
    }
    for b in start(10) {
        assert!((-5.12..=5.12).contains(&b), "K0: x0 outside the box");
    }
}

/// R1 Table 1 written out again: (mu, weights, mu_eff, mu_eff^-, c_sigma, d_sigma, c_c, c_1, c_mu, chi_n, the three alphas).
#[allow(clippy::type_complexity)]
fn table1(n: usize, lambda: usize) -> (usize, Vec<f64>, [f64; 9], [f64; 3]) {
    let nf = n as f64;
    let mu = lambda / 2;
    let wp: Vec<f64> = (1..=lambda)
        .map(|i| ((lambda as f64 + 1.0) / 2.0).ln() - (i as f64).ln())
        .collect();
    let pos: Vec<f64> = wp[..mu].to_vec();
    let neg: Vec<f64> = wp[mu..].to_vec();
    let mu_eff = pos.iter().sum::<f64>().powi(2) / pos.iter().map(|w| w * w).sum::<f64>();
    let mu_eff_m = neg.iter().sum::<f64>().powi(2) / neg.iter().map(|w| w * w).sum::<f64>();
    let c_sigma = (mu_eff + 2.0) / (nf + mu_eff + 5.0);
    let d_sigma = 1.0 + 2.0 * (((mu_eff - 1.0) / (nf + 1.0)).sqrt() - 1.0).max(0.0) + c_sigma;
    let c_c = (4.0 + mu_eff / nf) / (nf + 4.0 + 2.0 * mu_eff / nf);
    let c1 = 2.0 / ((nf + 1.3) * (nf + 1.3) + mu_eff);
    let c_mu = (1.0 - c1).min(2.0 * (0.25 + mu_eff + 1.0 / mu_eff - 2.0) / ((nf + 2.0) * (nf + 2.0) + mu_eff));
    let a_mu = 1.0 + c1 / c_mu;
    let a_eff = 1.0 + 2.0 * mu_eff_m / (mu_eff + 2.0);
    let a_pd = (1.0 - c1 - c_mu) / (nf * c_mu);
    let amin = a_mu.min(a_eff).min(a_pd);
    let sum_pos: f64 = wp.iter().filter(|w| **w > 0.0).sum();
    let sum_neg: f64 = wp.iter().filter(|w| **w < 0.0).map(|w| -w).sum();
    let w = wp
        .iter()
        .map(|&x| if x >= 0.0 { x / sum_pos } else { x * amin / sum_neg })
        .collect();
    let chi = nf.sqrt() * (1.0 - 1.0 / (4.0 * nf) + 1.0 / (21.0 * nf * nf));
    (
        mu,
        w,
        [mu_eff, mu_eff_m, c_sigma, d_sigma, c_c, c1, c_mu, chi, amin],
        [a_mu, a_eff, a_pd],
    )
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

#[test]
fn k1_parameters_are_those_of_table_1() {
    for (n, lambda) in [(1, 4), (2, 6), (3, 7), (4, 8), (10, 10), (30, 14), (2, 2), (4, 50)] {
        if lambda == cmaes::default_lambda(n) {
            println!("K1 n {n}: default lambda {lambda}");
        }
        let p = Params::new(n, lambda);
        let (mu, w, v, alphas) = table1(n, lambda);
        assert_eq!((p.lambda, p.mu), (lambda, mu), "K1 n {n} lambda {lambda}");
        let lib = [
            p.mu_eff,
            p.mu_eff_minus,
            p.c_sigma,
            p.d_sigma,
            p.c_c,
            p.c1,
            p.c_mu,
            p.chi_n,
        ];
        let names = [
            "mu_eff", "mu_eff^-", "c_sigma", "d_sigma", "c_c", "c_1", "c_mu", "chi_n",
        ];
        for k in 0..8 {
            assert!(
                rel(lib[k], v[k]) <= 1e-14,
                "K1 n {n} lambda {lambda}: {} {} against {}",
                names[k],
                lib[k],
                v[k]
            );
        }
        for (i, (a, b)) in p.weights.iter().zip(&w).enumerate() {
            assert!(
                (a - b).abs() <= 1e-14 * b.abs().max(1e-300) || (a == b),
                "K1 n {n} lambda {lambda}: w_{} {a} against {b}",
                i + 1
            );
        }
        let sp: f64 = p.weights.iter().filter(|x| **x > 0.0).sum();
        let sn: f64 = p.weights.iter().filter(|x| **x < 0.0).sum();
        let which = ["alpha_mu^-", "alpha_mu_eff^-", "alpha_posdef^-"][alphas.iter().position(|a| *a == v[8]).unwrap()];
        println!(
            "K1 n {n} lambda {lambda}: mu {mu}, mu_eff {:.6}, mu_eff^- {:.6}, c_sigma {:.6}, d_sigma {:.6}, c_c {:.6}, c_1 {:.6e}, c_mu {:.6e}; sum w+ {sp}, sum w- {sn:.15}; alphas {:.6} {:.6} {:.6}, the minimum is {which}",
            v[0], v[1], v[2], v[3], v[4], v[5], v[6], alphas[0], alphas[1], alphas[2]
        );
        assert!((sp - 1.0).abs() <= 1e-14, "K1: positive weights sum to {sp}");
        assert!(
            (sn + v[8]).abs() <= 1e-14,
            "K1: negative weights sum to {sn}, not -{}",
            v[8]
        );
    }
}

#[test]
fn k2_eigendecomposition() {
    let check = |a: &[Vec<f64>], tag: &str| -> (Vec<f64>, Vec<Vec<f64>>) {
        let n = a.len();
        let (e, b) = cmaes::sym_eigen(a);
        let amax = a.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
        let (mut rec, mut orth) = (0.0f64, 0.0f64);
        for i in 0..n {
            for j in 0..n {
                let r: f64 = (0..n).map(|k| b[i][k] * e[k] * b[j][k]).sum();
                rec = rec.max((r - a[i][j]).abs());
                let o: f64 = (0..n).map(|k| b[k][i] * b[k][j]).sum();
                orth = orth.max((o - if i == j { 1.0 } else { 0.0 }).abs());
            }
        }
        println!(
            "K2 {tag} n {n}: reconstruction {:.2e} of max {amax:.3}, orthogonality {orth:.2e}",
            rec
        );
        assert!(rec <= 1e-12 * amax, "K2 {tag}: reconstruction {rec}");
        assert!(orth <= 1e-13, "K2 {tag}: orthogonality {orth}");
        assert!(e.windows(2).all(|w| w[0] <= w[1]), "K2 {tag}: not ascending {e:?}");
        (e, b)
    };
    for s in 1..=20u64 {
        let n = 1 + (s % 12) as usize;
        let mut rng = Rng::new(500 + s);
        let m: Vec<Vec<f64>> = (0..n).map(|_| (0..n).map(|_| rng.range(-1.0, 1.0)).collect()).collect();
        let a: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| (0..n).map(|k| m[i][k] * m[j][k]).sum::<f64>() + if i == j { 1e-3 } else { 0.0 })
                    .collect()
            })
            .collect();
        check(&a, &format!("seed {s}"));
    }
    let (e, _) = check(&[vec![2.0, 1.0], vec![1.0, 2.0]], "[[2, 1], [1, 2]]");
    assert!((e[0] - 1.0).abs() <= 1e-15 && (e[1] - 3.0).abs() <= 1e-15, "K2: {e:?}");
    let (e, b) = check(
        &[vec![3.0, 0.0, 0.0], vec![0.0, 1.0, 0.0], vec![0.0, 0.0, 2.0]],
        "diag(3, 1, 2)",
    );
    assert_eq!(e, vec![1.0, 2.0, 3.0], "K2 diag");
    assert_eq!(
        b,
        vec![vec![0.0, 0.0, 1.0], vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]],
        "K2 diag"
    );
}

/// The standard normal distribution function by composite Simpson on `[-10, t]` with 20 000 intervals.
fn phi(t: f64) -> f64 {
    let n = 20_000;
    let (a, h) = (-10.0, (t + 10.0) / n as f64);
    let pdf = |x: f64| (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let mut s = pdf(a) + pdf(t);
    for k in 1..n {
        s += pdf(a + h * k as f64) * if k % 2 == 1 { 4.0 } else { 2.0 };
    }
    s * h / 3.0
}

#[test]
fn k3_normal_draws() {
    let mut rng = Rng::new(7);
    let n = 200_000;
    let z: Vec<f64> = (0..n).map(|_| cmaes::normal(&mut rng)).collect();
    let mean = z.iter().sum::<f64>() / n as f64;
    let var = z.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n as f64;
    println!("K3: mean {mean:.5}, variance {var:.5}");
    assert!(
        mean.abs() <= 0.01 && (var - 1.0).abs() <= 0.01,
        "K3: mean {mean}, variance {var}"
    );
    for t in [-2.0, -1.0, 0.0, 1.0, 2.0] {
        let f = z.iter().filter(|v| **v <= t).count() as f64 / n as f64;
        let p = phi(t);
        println!("K3: F({t}) = {f:.5}, Phi = {p:.6}");
        assert!((f - p).abs() <= 0.005, "K3 at {t}: {f} against {p}");
    }
    let (mut a, mut b) = (Rng::new(7), Rng::new(7));
    for _ in 0..10 {
        let (u1, u2) = (b.uniform(), b.uniform());
        let want = (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        assert_eq!(cmaes::normal(&mut a).to_bits(), want.to_bits(), "K3 replica");
    }
}

/// `M v`.
fn mv(m: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    m.iter().map(|r| r.iter().zip(v).map(|(a, b)| a * b).sum()).collect()
}

/// R1 (41) to (47) written out again, with `C^{-1/2} = B D^-1 B^T` applied as a matrix. Returns the new state and `h_sigma`.
fn replica(p: &Params, s0: &State, z: &[Vec<f64>]) -> (State, bool) {
    let n = p.n;
    let bd: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| s0.b[i][j] * s0.d[j]).collect()).collect();
    let cinv: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| (0..n).map(|k| s0.b[i][k] * s0.b[j][k] / s0.d[k]).sum())
                .collect()
        })
        .collect();
    let y: Vec<Vec<f64>> = z.iter().map(|zk| mv(&bd, zk)).collect();
    let mut yw = vec![0.0; n];
    for i in 0..p.mu {
        for j in 0..n {
            yw[j] += p.weights[i] * y[i][j];
        }
    }
    let mut s = s0.clone();
    for j in 0..n {
        s.m[j] = s0.m[j] + s0.sigma * yw[j];
    }
    let ciy = mv(&cinv, &yw);
    for j in 0..n {
        s.p_sigma[j] = (1.0 - p.c_sigma) * s0.p_sigma[j] + (p.c_sigma * (2.0 - p.c_sigma) * p.mu_eff).sqrt() * ciy[j];
    }
    let nps = s.p_sigma.iter().map(|v| v * v).sum::<f64>().sqrt();
    s.sigma = s0.sigma * ((p.c_sigma / p.d_sigma) * (nps / p.chi_n - 1.0)).exp();
    let h = nps / (1.0 - (1.0 - p.c_sigma).powf(2.0 * (s0.g as f64 + 1.0))).sqrt()
        < (1.4 + 2.0 / (n as f64 + 1.0)) * p.chi_n;
    for j in 0..n {
        s.p_c[j] = (1.0 - p.c_c) * s0.p_c[j]
            + if h {
                (p.c_c * (2.0 - p.c_c) * p.mu_eff).sqrt() * yw[j]
            } else {
                0.0
            };
    }
    let delta = if h { 0.0 } else { p.c_c * (2.0 - p.c_c) };
    let sw: f64 = p.weights.iter().sum();
    for r in 0..n {
        for q in 0..n {
            let mut v = (1.0 + p.c1 * delta - p.c1 - p.c_mu * sw) * s0.c[r][q] + p.c1 * s.p_c[r] * s.p_c[q];
            for (i, yi) in y.iter().enumerate() {
                let w = p.weights[i];
                let wo = if w >= 0.0 {
                    w
                } else {
                    let c = mv(&cinv, yi);
                    w * n as f64 / c.iter().map(|x| x * x).sum::<f64>()
                };
                v += p.c_mu * wo * yi[r] * yi[q];
            }
            s.c[r][q] = v;
        }
    }
    s.g = s0.g + 1;
    (s, h)
}

fn close(tag: &str, a: &[f64], b: &[f64]) {
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() <= 1e-13 * y.abs().max(1.0), "K4 {tag}: {x} against {y}");
    }
}

#[test]
fn k4_one_update_against_the_tutorial() {
    let p = Params::new(3, 7);
    let mut rng = Rng::new(11);
    let z: Vec<Vec<f64>> = (0..7)
        .map(|_| (0..3).map(|_| cmaes::normal(&mut rng)).collect())
        .collect();
    for (case, ps) in [("a", [0.1, -0.2, 0.3]), ("b", [10.0, 0.0, 0.0])] {
        let mut s0 = State::new(vec![0.2, 0.5, 0.7], 0.3);
        s0.c = vec![vec![1.0, 0.3, 0.1], vec![0.3, 2.0, -0.2], vec![0.1, -0.2, 0.5]];
        s0.p_sigma = ps.to_vec();
        s0.p_c = vec![0.05, 0.0, -0.1];
        s0.g = 4;
        s0.decompose().unwrap();
        let (want, h) = replica(&p, &s0, &z);
        let mut s = s0.clone();
        cmaes::update(&p, &mut s, &z).unwrap();
        println!(
            "K4 ({case}): h_sigma {h}; m {:?}, sigma {:.15}, p_sigma {:?}, p_c {:?}",
            s.m, s.sigma, s.p_sigma, s.p_c
        );
        if case == "b" {
            assert!(!h, "K4 (b) set-up: h_sigma is not 0");
        }
        close("m", &s.m, &want.m);
        close("sigma", &[s.sigma], &[want.sigma]);
        close("p_sigma", &s.p_sigma, &want.p_sigma);
        close("p_c", &s.p_c, &want.p_c);
        for r in 0..3 {
            close("C", &s.c[r], &want.c[r]);
        }
        assert_eq!(s.g, 5, "K4: g");
    }
}

/// A state of K5: n = 2, `m = (0.5, 0.5)`, `sigma = 0.1`, `C = I`, `p_c = 0`, `g = 0`.
fn k5_state() -> State {
    State::new(vec![0.5, 0.5], 0.1)
}

fn with_c(mut s: State, c: [[f64; 2]; 2]) -> State {
    s.c = c.iter().map(|r| r.to_vec()).collect();
    s.decompose().unwrap();
    s
}

fn hist(best: Vec<Option<f64>>, median: Vec<Option<f64>>, current: Vec<f64>) -> History {
    History { best, median, current }
}

#[test]
fn k5_termination_criteria() {
    let p = Params::new(2, 6);
    let empty = History::default();
    let t = |s: &State, h: &History| cmaes::termination(&p, s, h, 0.3);
    let mut cases: Vec<(&str, Option<Termination>, Option<Termination>)> = Vec::new();
    let mut s = k5_state();
    s.m = vec![1.0, 1.0];
    s.sigma = 1e-17;
    cases.push(("sigma 1e-17 at m = 1", t(&s, &empty), Some(Termination::NoEffectAxis)));
    s.sigma = 1e-14;
    cases.push(("sigma 1e-14 at m = 1", t(&s, &empty), Some(Termination::TolX)));
    let mut s = with_c(k5_state(), [[1.0, 0.0], [0.0, 1e-30]]);
    s.g = 1;
    cases.push(("C diag(1, 1e-30), g 1", t(&s, &empty), Some(Termination::NoEffectCoord)));
    let s = with_c(k5_state(), [[1.0, 0.0], [0.0, 1e-15]]);
    cases.push(("C diag(1, 1e-15)", t(&s, &empty), Some(Termination::ConditionCov)));
    let s = with_c(k5_state(), [[1.0, 0.0], [0.0, 1e-13]]);
    cases.push(("C diag(1, 1e-13)", t(&s, &empty), None));
    let at = |g: usize| {
        let mut s = k5_state();
        s.g = g;
        s
    };
    let h = hist(vec![Some(1.0); 20], vec![Some(2.0); 20], vec![1.0, 2.0]);
    cases.push((
        "20 equal best values",
        t(&at(20), &h),
        Some(Termination::EqualFunValues),
    ));
    let h = hist(vec![Some(1.0); 19], vec![Some(2.0); 19], vec![1.0, 2.0]);
    cases.push(("19 equal best values", t(&at(19), &h), None));
    let mut b = vec![Some(1.0); 20];
    b[7] = Some(2.0);
    let h = hist(b, vec![Some(2.0); 20], vec![1.0, 2.0]);
    cases.push(("one of 20 larger by 1", t(&at(20), &h), None));
    let up = hist(
        (0..200).map(|g| Some(g as f64)).collect(),
        (0..200).map(|g| Some(g as f64 + 1.0)).collect(),
        vec![199.0],
    );
    cases.push(("increasing over 200", t(&at(200), &up), Some(Termination::Stagnation)));
    let down = hist(
        (0..200).map(|g| Some(-(g as f64))).collect(),
        (0..200).map(|g| Some(-(g as f64) + 1.0)).collect(),
        vec![-199.0],
    );
    cases.push(("decreasing over 200", t(&at(200), &down), None));
    let mut s = k5_state();
    s.sigma = 3001.0;
    cases.push(("sigma 3001", t(&s, &empty), Some(Termination::TolXUp)));
    s.sigma = 2999.0;
    cases.push(("sigma 2999", t(&s, &empty), None));
    let near = hist(
        (0..20).map(|g| Some(1.0 + 5e-15 * g as f64)).collect(),
        vec![Some(2.0); 20],
        vec![1.0, 1.0 + 5e-14],
    );
    cases.push(("range 1e-13", t(&at(20), &near), Some(Termination::TolFun)));
    let wide = hist(
        (0..20).map(|g| Some(1.0 + 2e-12 * g as f64 / 19.0)).collect(),
        vec![Some(2.0); 20],
        vec![1.0],
    );
    cases.push(("range 2e-12", t(&at(20), &wide), None));
    let mut s = k5_state();
    s.sigma = 1e-13;
    cases.push(("sigma 1e-13", t(&s, &empty), Some(Termination::TolX)));
    s.p_c = vec![10.0, 0.0];
    cases.push(("sigma 1e-13, p_c (10, 0)", t(&s, &empty), None));
    let mut b = vec![Some(1.0); 20];
    b[3] = None;
    let h = hist(b, vec![Some(2.0); 20], vec![1.0]);
    cases.push(("20 equal, one missing", t(&at(20), &h), None));
    for (name, got, want) in &cases {
        println!("K5 {name}: {got:?} (want {want:?})");
    }
    for (name, got, want) in cases {
        assert_eq!(got, want, "K5 {name}");
    }
}

// ------------------------------------------------------------------ C and N: the acceptance

#[test]
fn c1_cma_es_reaches_the_target_on_the_shifted_rastrigin_function() {
    let b = vec![(-5.12, 5.12); 4];
    let x0 = start(4);
    let mut ok = 0;
    for seed in 1..=20u64 {
        let t = std::time::Instant::now();
        let (r, runs, calls, first) = cma(&r4, &b, &opts(40_000, 9, seed, Some(x0.clone())));
        assert_eq!(calls, r.evaluations, "S6: evaluations");
        assert!(r.evaluations <= 40_000);
        // S7: every run starts at x0 and doubles the population
        for (k, run) in runs.iter().enumerate() {
            assert!(
                run.m0.iter().zip(&x0).all(|(a, c)| a.to_bits() == c.to_bits()),
                "S7: run {k} starts at {:?}",
                run.m0
            );
            assert_eq!(run.lambda, 8 << k, "S7: run {k} population");
        }
        let at = first.map(|e| {
            let mut acc = 0;
            let k = runs
                .iter()
                .position(|r| {
                    acc += r.evaluations;
                    acc >= e
                })
                .unwrap();
            (runs[k].lambda, e)
        });
        let good = r.fitness.f <= TARGET;
        ok += usize::from(good);
        println!(
            "C1 seed {seed}: best f {:.3e} at {:?}; {} evaluations in {} runs ({:?}); first at or below 1e-8: {at:?}; {:.2} s",
            r.fitness.f,
            r.x,
            r.evaluations,
            runs.len(),
            runs.iter().map(|r| (r.lambda, r.generations, r.evaluations, r.stop)).collect::<Vec<_>>(),
            t.elapsed().as_secs_f64()
        );
    }
    println!("C1: {ok} of 20 seeds reached f <= 1e-8 (limit 18)");
    assert!(ok >= 18, "C1: {ok} of 20 seeds reached the target (limit 18)");
}

#[test]
fn n0_nelder_mead_does_not_reach_the_target_from_the_start() {
    let b = vec![(-5.12, 5.12); 4];
    let mut reached = 0;
    for (tag, simplex, coefficients) in [
        ("default", InitialSimplex::Range, Coefficients::Standard),
        ("start + adaptive", InitialSimplex::Start, Coefficients::Adaptive),
    ] {
        for seed in 1..=20u64 {
            let mut p = Probe::new(&r4, &b);
            let o = NmOptions {
                max_evals: 40_000,
                restarts: 9,
                seed,
                x0: Some(start(4)),
                initial_simplex: simplex,
                coefficients,
                ..Default::default()
            };
            let r = optim::nelder_mead(&b, &o, &mut |xs| p.batch(xs)).unwrap();
            println!(
                "N0 {tag} seed {seed}: best f {:.6} at {:?} after {} evaluations",
                r.fitness.f, r.x, r.evaluations
            );
            reached += usize::from(r.fitness.f <= TARGET);
        }
    }
    assert_eq!(reached, 0, "N0 set-up: Nelder-Mead reached the target from x0");
}

#[test]
fn c1c_latin_hypercube_control_does_not_reach_the_target() {
    let b = vec![(-5.12, 5.12); 4];
    let pts = sampling::scale(&sampling::latin_hypercube(40_000, 4, 1), &b);
    let best = pts.iter().map(|x| r4(x)).fold(f64::INFINITY, f64::min);
    println!("C1c: best f of 40 000 Latin-hypercube points {best:.4}");
    assert!(best > TARGET, "C1c set-up: the control reached the target");
}

#[test]
fn c1r_record_without_restarts() {
    let b = vec![(-5.12, 5.12); 4];
    let mut ok = 0;
    for seed in 1..=20u64 {
        let (r, runs, _, _) = cma(&r4, &b, &opts(40_000, 0, seed, Some(start(4))));
        ok += usize::from(r.fitness.f <= TARGET);
        println!(
            "C1r seed {seed}: best f {:.3e}, {} evaluations, {} generations, stop {:?}",
            r.fitness.f, r.evaluations, runs[0].generations, runs[0].stop
        );
    }
    println!("C1r (recorded): {ok} of 20 seeds reached f <= 1e-8 with one run (lambda 8)");
}

#[test]
fn c2_record_shifted_rastrigin_in_ten_dimensions() {
    let b = vec![(-5.12, 5.12); 10];
    let (mut ok, mut nm_ok) = (0, 0);
    for seed in 1..=20u64 {
        let (r, runs, _, first) = cma(&r10, &b, &opts(100_000, 9, seed, Some(start(10))));
        ok += usize::from(r.fitness.f <= TARGET);
        println!(
            "C2 seed {seed}: best f {:.3e}, {} evaluations in {} runs (last lambda {}), first at or below 1e-8 after {first:?}",
            r.fitness.f,
            r.evaluations,
            runs.len(),
            runs.last().unwrap().lambda
        );
        let mut p = Probe::new(&r10, &b);
        let o = NmOptions {
            max_evals: 100_000,
            restarts: 9,
            seed,
            x0: Some(start(10)),
            ..Default::default()
        };
        let n = optim::nelder_mead(&b, &o, &mut |xs| p.batch(xs)).unwrap();
        nm_ok += usize::from(n.fitness.f <= TARGET);
        println!("C2 seed {seed}: Nelder-Mead best f {:.4}", n.fitness.f);
    }
    println!("C2 (recorded): CMA-ES {ok} of 20, Nelder-Mead {nm_ok} of 20 reached f <= 1e-8");
}

// ------------------------------------------------------------------ U: unimodal

fn unimodal(tag: &str, f: &dyn Fn(&[f64]) -> f64, budget: usize) -> usize {
    let b = vec![(-5.0, 5.0); 10];
    let mut ok = 0;
    for seed in 1..=20u64 {
        let (r, runs, calls, _) = cma(f, &b, &opts(budget, 0, seed, Some(vec![1.0; 10])));
        assert_eq!(calls, r.evaluations);
        let mut first = None;
        // evaluations to 1e-10 are not tracked by the probe (its target is 1e-8): report the total
        if r.fitness.f <= 1e-10 {
            first = Some(r.evaluations);
            ok += 1;
        }
        println!(
            "{tag} seed {seed}: best f {:.3e}, {} evaluations, {} generations, stop {:?} ({first:?})",
            r.fitness.f, r.evaluations, runs[0].generations, runs[0].stop
        );
    }
    ok
}

#[test]
fn u1_sphere() {
    let ok = unimodal("U1", &sphere, 10_000);
    assert_eq!(ok, 20, "U1: {ok} of 20 seeds reached f <= 1e-10");
}

#[test]
fn u2_ellipsoid() {
    let ok = unimodal("U2", &ellipsoid, 20_000);
    assert_eq!(ok, 20, "U2: {ok} of 20 seeds reached f <= 1e-10");
}

#[test]
fn u3_record_bounded_sphere() {
    let b = vec![(-5.12, 5.12); 4];
    let fstar = 4.0 * 0.88f64 * 0.88;
    for seed in 1..=20u64 {
        let (r, runs, _, _) = cma(&bounded_sphere, &b, &opts(10_000, 0, seed, Some(vec![0.0; 4])));
        let gap = r.x.iter().map(|v| 5.12 - v).fold(0.0f64, f64::max);
        println!(
            "U3 seed {seed} (recorded): f - f* {:.3e}, largest 5.12 - x_i {gap:.3e}, {} evaluations, stop {:?}",
            r.fitness.f - fstar,
            r.evaluations,
            runs[0].stop
        );
    }
}

#[test]
fn u4_record_rosenbrock_from_random_starts() {
    let b = [(-2.0, 2.0), (-2.0, 2.0)];
    let mut ok = 0;
    for seed in 1..=20u64 {
        let (r, runs, _, _) = cma(&rosenbrock, &b, &opts(800, 3, seed, None));
        let good = r.fitness.f <= 1e-6 && (r.x[0] - 1.0).abs() <= 1e-3 && (r.x[1] - 1.0).abs() <= 1e-3;
        ok += usize::from(good);
        println!(
            "U4 seed {seed}: f {:.3e} after {} evaluations in {} runs",
            r.fitness.f,
            r.evaluations,
            runs.len()
        );
    }
    println!("U4 (recorded): {ok} of 20 seeds reached the Rosenbrock minimum (ADR 0067 O1 rule)");
}

// ------------------------------------------------------------------ S: the study path

fn def(d: usize, lo: f64, hi: f64, init: Option<Vec<f64>>, workers: usize) -> StudyDef {
    StudyDef {
        name: "cma".into(),
        params: (0..d)
            .map(|i| ParamDef {
                name: format!("x{i}"),
                lo,
                hi,
                init: init.as_ref().map(|v| v[i]),
                targets: vec![],
            })
            .collect(),
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: vec![ObjectiveDef {
            metric: "f".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers,
    }
}

fn result(f: f64) -> CondResult {
    let mut m = BTreeMap::new();
    m.insert("f".to_string(), f);
    CondResult::success("base", m)
}

struct FEval {
    f: fn(&[f64]) -> f64,
    calls: AtomicUsize,
}

impl FEval {
    fn new(f: fn(&[f64]) -> f64) -> Self {
        FEval {
            f,
            calls: AtomicUsize::new(0),
        }
    }
}

impl Evaluator for FEval {
    fn evaluate(&self, d: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        for (v, q) in x.iter().zip(&d.params) {
            assert!(*v >= q.lo && *v <= q.hi, "S6: {v} outside [{}, {}]", q.lo, q.hi);
        }
        vec![result((self.f)(x))]
    }
}

fn method(budget: usize, restarts: usize, seed: u64) -> Method {
    Method::CmaEs {
        budget,
        pop: None,
        sigma0: 0.3,
        restarts,
        seed,
    }
}

fn r4_def(workers: usize) -> StudyDef {
    def(4, -5.12, 5.12, Some(start(4)), workers)
}

fn records(workers: usize, m: &Method) -> Vec<Record> {
    let ev = FEval::new(r4);
    let mut s = Study::new(r4_def(workers), Store::memory("cma"), &ev).unwrap();
    s.run(m).unwrap();
    s.store.records().to_vec()
}

type RecordBits = (usize, String, Vec<u64>, Vec<(String, u64)>);

fn bits(recs: &[Record]) -> Vec<RecordBits> {
    recs.iter()
        .map(|r| {
            (
                r.index,
                r.key.clone(),
                r.params.iter().map(|(_, v)| v.to_bits()).collect(),
                r.conditions
                    .iter()
                    .flat_map(|c| c.metrics.iter().map(|(k, v)| (format!("{}.{k}", c.label), v.to_bits())))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn s1_study_equals_the_library() {
    let ev = FEval::new(r4);
    let mut s = Study::new(r4_def(1), Store::memory("cma"), &ev).unwrap();
    let rs = s.run(&method(5000, 3, 1)).unwrap();
    let b = vec![(-5.12, 5.12); 4];
    let mut asked: Vec<Vec<f64>> = Vec::new();
    let lib = cmaes::run(&b, &opts(5000, 3, 1, Some(start(4))), &mut |xs| {
        asked.extend(xs.iter().cloned());
        Ok(xs.iter().map(|x| Fitness::new(r4(x), 0.0)).collect())
    })
    .unwrap();
    let recs: Vec<Vec<u64>> = s
        .store
        .records()
        .iter()
        .map(|r| r.params.iter().map(|(_, v)| v.to_bits()).collect())
        .collect();
    let lib_bits: Vec<Vec<u64>> = asked.iter().map(|x| x.iter().map(|v| v.to_bits()).collect()).collect();
    println!(
        "S1: {} records, {} library points; study best {:.3e}, library best {:.3e}",
        recs.len(),
        lib_bits.len(),
        rs.fitness.f,
        lib.fitness.f
    );
    assert!(recs == lib_bits, "S1: the study's records are not the library's points");
    assert!(
        rs.x.iter().zip(&lib.x).all(|(a, b)| a.to_bits() == b.to_bits()),
        "S1: best points differ"
    );
}

#[test]
fn s2_determinism_workers_and_seeds() {
    let a = records(1, &method(2000, 2, 4));
    let b = records(1, &method(2000, 2, 4));
    let w4 = records(4, &method(2000, 2, 4));
    let c = records(1, &method(2000, 2, 5));
    println!("S2: {} records (seed 4), {} (seed 5)", a.len(), c.len());
    assert!(bits(&a) == bits(&b), "S2: two runs differ");
    assert!(bits(&a) == bits(&w4), "S2: workers 1 and 4 differ");
    assert!(bits(&a) != bits(&c), "S2: seeds 4 and 5 give the same records");
}

struct NoEval;
impl Evaluator for NoEval {
    fn evaluate(&self, _d: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        panic!("the propose / record loop evaluates nothing itself")
    }
}

#[test]
fn s3_propose_record_replays_the_evaluator_run() {
    let ev = NoEval;
    let m = method(2000, 2, 4);
    let mut s = Study::new(r4_def(1), Store::memory("cma"), &ev).unwrap();
    let mut rounds = 0;
    while let Proposal::Next(points) = s.propose(&m).unwrap() {
        rounds += 1;
        assert!(rounds <= 2000);
        for p in points {
            s.record(&p, vec![result(r4(&p))], "").unwrap();
        }
    }
    let direct = records(1, &m);
    let strip = |r: &[Record]| {
        r.iter()
            .map(|r| {
                (
                    r.index,
                    r.params.iter().map(|(_, v)| v.to_bits()).collect::<Vec<_>>(),
                    r.conditions[0]
                        .metrics
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    println!("S3: {rounds} proposals, {} records", s.store.records().len());
    assert!(strip(s.store.records()) == strip(&direct), "S3");
}

#[test]
fn s4_resume_after_interruption() {
    let m = method(2000, 2, 4);
    let ev = FEval::new(r4);
    let mut full = Study::new(r4_def(1), Store::memory("cma"), &ev).unwrap();
    full.run(&m).unwrap();
    let total = ev.calls.swap(0, Ordering::SeqCst);
    assert_eq!(total, full.store.records().len());
    let k = 37usize;
    let dir = std::env::temp_dir().join(format!("rcfd_f260_s4_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("evaluations.jsonl");
    let mut s = Study::new(r4_def(1), Store::open(&path, "cma").unwrap(), &ev).unwrap();
    s.allowance = Some(k);
    assert_eq!(s.run(&m).unwrap_err(), Stop::Interrupted);
    assert_eq!(ev.calls.swap(0, Ordering::SeqCst), k);
    let mut s2 = Study::new(r4_def(1), Store::open(&path, "cma").unwrap(), &ev).unwrap();
    s2.run(&m).unwrap();
    let resumed = ev.calls.swap(0, Ordering::SeqCst);
    let mut s3 = Study::new(r4_def(1), Store::open(&path, "cma").unwrap(), &ev).unwrap();
    s3.run(&m).unwrap();
    let third = ev.calls.swap(0, Ordering::SeqCst);
    println!("S4 k = {k}: resume evaluated {resumed} of {total}, a third launch {third}");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(resumed, total - k);
    assert_eq!(third, 0);
    assert!(bits(s2.store.records()) == bits(full.store.records()), "S4");
}

#[test]
fn s5_refusals_evaluate_nothing() {
    let mut calls = 0usize;
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        calls += xs.len();
        Ok(xs.iter().map(|x| Fitness::new(sphere(x), 0.0)).collect())
    };
    let b4 = vec![(-5.12, 5.12); 4];
    let mut msgs = Vec::new();
    let mut refuse = |b: &[(f64, f64)], o: Options, ev: &mut optim::Eval<'_>| match cmaes::run(b, &o, ev) {
        Err(Stop::Error(m)) => msgs.push(m),
        other => panic!("S5: not refused: {other:?}"),
    };
    refuse(&[], opts(1000, 0, 1, None), &mut ev);
    refuse(
        &b4,
        Options {
            pop: Some(1),
            ..opts(1000, 0, 1, None)
        },
        &mut ev,
    );
    for s0 in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        refuse(
            &b4,
            Options {
                sigma0: s0,
                ..opts(1000, 0, 1, None)
            },
            &mut ev,
        );
    }
    refuse(&b4, opts(7, 0, 1, None), &mut ev);
    refuse(&[(0.0, 1.0), (2.0, 2.0)], opts(1000, 0, 1, None), &mut ev);
    for m in &msgs {
        println!("S5: {m}");
    }
    assert_eq!(msgs.len(), 8);
    assert_eq!(calls, 0, "S5: a refused run evaluated a point");
}

#[test]
fn s6_budget_ends_the_method() {
    let b = vec![(-5.0, 5.0); 4];
    let (r, runs, calls, _) = cma(&sphere, &b, &opts(1000, 9, 1, None));
    println!(
        "S6: {} evaluations ({calls} counted) in {} runs, stops {:?}",
        r.evaluations,
        runs.len(),
        runs.iter().map(|r| r.stop).collect::<Vec<_>>()
    );
    assert!(r.evaluations <= 1000 && calls == r.evaluations, "S6");
    assert_eq!(runs.last().unwrap().stop, Termination::Budget, "S6: the last stop");
    assert!(!r.converged);
}

#[test]
fn s7_restart_points_without_a_start() {
    let b = vec![(-5.0, 5.0); 2];
    let (_, runs, _, _) = cma(&sphere, &b, &opts(5000, 3, 1, None));
    println!(
        "S7: {} runs, starts {:?}",
        runs.len(),
        runs.iter().map(|r| r.m0.clone()).collect::<Vec<_>>()
    );
    assert!(runs.len() >= 2, "S7 set-up: one run only");
    for (i, a) in runs.iter().enumerate() {
        assert!(
            a.m0.iter().all(|v| (-5.0..=5.0).contains(v)),
            "S7: start outside the box"
        );
        for c in &runs[..i] {
            assert!(a.m0 != c.m0, "S7: two runs start at the same point");
        }
    }
}
