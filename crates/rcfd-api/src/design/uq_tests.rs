//! ADR 0121 checks U1-U8 of the tolerance study driver. Each check is a function of its seeds, run by the registered test with
//! the registered seeds and by `smoke_w46d` (ignored) with other seeds.

use super::*;
use crate::design::ledger::{Fidelity, FnModel, Ledger};
use crate::design::rng::Rng;

/// Registered k (standard errors) of the closed-form checks.
const K: f64 = 4.0;

fn normal(name: &str, mean: f64, sd: f64) -> UncertainParam {
    UncertainParam {
        name: name.into(),
        unit: "1".into(),
        dist: Dist::Normal {
            mean,
            sd,
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
        },
    }
}

fn model<F: Fn(&[f64]) -> f64 + Sync>(id: &str, f: F) -> FnModel<impl Fn(&[Quantity]) -> Outcome + Sync> {
    FnModel {
        id: id.into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: move |p: &[Quantity]| {
            let x: Vec<f64> = p.iter().map(|q| q.value).collect();
            Outcome::ok(vec![Quantity::new("y", f(&x), "1")])
        },
    }
}

/// Fixed-n spec: `n` samples (MC batches of 2000, LHS replicates of 1000), no SE target.
fn fixed_spec(params: Vec<UncertainParam>, sampler: Sampler, seed: u64, n: usize) -> UqSpec {
    let batch = if sampler == Sampler::MonteCarlo { 2000 } else { 1000 };
    UqSpec {
        name: "u".into(),
        params,
        sampler,
        seed,
        stop: StopRule {
            batch,
            min_batches: 2,
            max_samples: n,
            se_abs: 0.0,
            se_rel: 0.0,
            metrics: vec![],
        },
        quantiles: vec![0.05, 0.5, 0.95],
        confidence: 0.999,
        requirements: vec![],
        workers: 1,
    }
}

// ---------------------------------------------------------------- the test functions

/// Linear y = 3 + 2 x1 - 1.5 x2 + 0.5 x3, x1 ~ N(1, 0.5^2), x2 ~ N(-2, 0.2^2), x3 ~ N(0, 1).
fn lin_params() -> Vec<UncertainParam> {
    vec![normal("x1", 1.0, 0.5), normal("x2", -2.0, 0.2), normal("x3", 0.0, 1.0)]
}
fn lin(x: &[f64]) -> f64 {
    3.0 + 2.0 * x[0] - 1.5 * x[1] + 0.5 * x[2]
}
/// Quadratic y = x1^2 + 3 x2^2 + 2 x3 with x1 ~ N(1, 0.5^2), x2 ~ N(0, 0.3^2), x3 ~ N(0, 1).
fn quad_params() -> Vec<UncertainParam> {
    vec![normal("x1", 1.0, 0.5), normal("x2", 0.0, 0.3), normal("x3", 0.0, 1.0)]
}
fn quad(x: &[f64]) -> f64 {
    x[0] * x[0] + 3.0 * x[1] * x[1] + 2.0 * x[2]
}
/// Mean, second and fourth cumulants of c X^2 + b X, X ~ N(mu, s^2) (with beta = b + 2 c mu):
/// kappa2 = beta^2 s^2 + 2 c^2 s^4, kappa4 = 48 c^2 beta^2 s^6 + 48 c^4 s^8.
fn term(c: f64, b: f64, mu: f64, s: f64) -> (f64, f64, f64) {
    let beta = b + 2.0 * c * mu;
    (
        c * (mu * mu + s * s) + b * mu,
        beta * beta * s * s + 2.0 * c * c * s.powi(4),
        48.0 * c * c * beta * beta * s.powi(6) + 48.0 * c.powi(4) * s.powi(8),
    )
}
fn sum3(t: [(f64, f64, f64); 3], c0: f64) -> (f64, f64, f64) {
    (
        c0 + t[0].0 + t[1].0 + t[2].0,
        t[0].1 + t[1].1 + t[2].1,
        t[0].2 + t[1].2 + t[2].2,
    )
}
fn lin_exact() -> (f64, f64, f64) {
    sum3(
        [
            term(0.0, 2.0, 1.0, 0.5),
            term(0.0, -1.5, -2.0, 0.2),
            term(0.0, 0.5, 0.0, 1.0),
        ],
        3.0,
    )
}
fn quad_exact() -> (f64, f64, f64) {
    sum3(
        [
            term(1.0, 0.0, 1.0, 0.5),
            term(3.0, 0.0, 0.0, 0.3),
            term(0.0, 2.0, 0.0, 1.0),
        ],
        0.0,
    )
}
/// The wrong model of the negative controls: every input with its sd times 1.1 and its mean moved by `shift` sd.
fn wrong(params: &[UncertainParam], f: fn(&[f64]) -> f64, shift: f64) -> impl Fn(&[f64]) -> f64 + Sync {
    let ms: Vec<(f64, f64)> = params
        .iter()
        .map(|p| match p.dist {
            Dist::Normal { mean, sd, .. } => (mean, sd),
            _ => unreachable!(),
        })
        .collect();
    move |x: &[f64]| {
        let xw: Vec<f64> = x
            .iter()
            .zip(&ms)
            .map(|(v, (m, s))| m + 1.1 * (v - m) + shift * s)
            .collect();
        f(&xw)
    }
}

/// z-scores of the sample mean and variance against the closed forms.
fn zs(res: &UqResult, exact: (f64, f64, f64)) -> (f64, f64) {
    let m = &res.metrics[0];
    let n = m.n as f64;
    let (mu, k2, k4) = exact;
    let zm = (m.mean - mu) / (k2 / n).sqrt();
    let zv = (m.sd * m.sd - k2) / (k4 / n + 2.0 * k2 * k2 / (n - 1.0)).sqrt();
    (zm, zv)
}

/// U1: (label, z of the mean, z of the variance) for the linear and quadratic functions, both samplers, and the wrong model.
fn u1(seed_mc: u64, seed_lhs: u64, n: usize) -> Vec<(String, f64, f64)> {
    let mut out = Vec::new();
    for (sampler, seed) in [(Sampler::MonteCarlo, seed_mc), (Sampler::Lhs, seed_lhs)] {
        for (label, params, f, exact) in [
            ("linear", lin_params(), lin as fn(&[f64]) -> f64, lin_exact()),
            ("quadratic", quad_params(), quad as fn(&[f64]) -> f64, quad_exact()),
        ] {
            let spec = fixed_spec(params.clone(), sampler, seed, n);
            let r = run(&spec, &model("analytic", f), None).unwrap();
            let (zm, zv) = zs(&r, exact);
            out.push((format!("{sampler:?} {label}"), zm, zv));
            let r = run(&spec, &model("wrong", wrong(&params, f, 0.1)), None).unwrap();
            let (zm, zv) = zs(&r, exact);
            out.push((format!("{sampler:?} {label} WRONG"), zm, zv));
        }
    }
    out
}

#[test]
fn u1_mean_and_variance_against_closed_forms() {
    for (label, zm, zv) in u1(46_001, 46_002, 20_000) {
        eprintln!("U1 {label}: z_mean {zm:+.3}, z_var {zv:+.3}");
        if label.ends_with("WRONG") {
            assert!(
                zm.abs() > K && zv.abs() > K,
                "{label}: the negative control must fail both"
            );
        } else {
            assert!(zm.abs() <= K && zv.abs() <= K, "{label}");
        }
    }
}

/// U2: (label, p, exact, lo, hi, coverage) for the linear (normal) output and y = exp(x), x ~ N(0, 0.5^2); the wrong model
/// scales the input sd by 1.1 (no mean shift: a shift would cancel the scaling at the lower tail, found by the smoke).
fn u2(seed_mc: u64, seed_lhs: u64, n: usize) -> Vec<(String, f64, f64, f64, f64, f64)> {
    let (mu, k2, _) = lin_exact();
    let lognormal = |x: &[f64]| x[0].exp();
    let mut out = Vec::new();
    for (sampler, seed) in [(Sampler::MonteCarlo, seed_mc), (Sampler::Lhs, seed_lhs)] {
        for wrong_model in [false, true] {
            let tag = if wrong_model { " WRONG" } else { "" };
            let spec = fixed_spec(lin_params(), sampler, seed, n);
            let r = if wrong_model {
                run(&spec, &model("w", wrong(&lin_params(), lin, 0.0)), None)
            } else {
                run(&spec, &model("a", lin), None)
            }
            .unwrap();
            for q in &r.metrics[0].quantiles {
                let exact = mu + k2.sqrt() * norm_inv(q.p);
                out.push((format!("{sampler:?} linear{tag}"), q.p, exact, q.lo, q.hi, q.coverage));
            }
            let spec = fixed_spec(vec![normal("x", 0.0, 0.5)], sampler, seed, n);
            let r = if wrong_model {
                run(&spec, &model("w", move |x: &[f64]| (1.1 * x[0]).exp()), None)
            } else {
                run(&spec, &model("a", lognormal), None)
            }
            .unwrap();
            for q in &r.metrics[0].quantiles {
                let exact = (0.5 * norm_inv(q.p)).exp();
                out.push((
                    format!("{sampler:?} lognormal{tag}"),
                    q.p,
                    exact,
                    q.lo,
                    q.hi,
                    q.coverage,
                ));
            }
        }
    }
    out
}

#[test]
fn u2_quantiles_against_exact_quantiles() {
    for (label, p, exact, lo, hi, cov) in u2(46_003, 46_004, 20_000) {
        let inside = lo <= exact && exact <= hi;
        eprintln!("U2 {label} p {p}: exact {exact:.6} in [{lo:.6}, {hi:.6}] coverage {cov:.5}: {inside}");
        assert!(cov >= 0.999, "{label}: coverage {cov}");
        if label.ends_with("WRONG") {
            if p != 0.5 {
                assert!(!inside, "{label} p {p}: the negative control must fall outside");
            }
        } else {
            assert!(inside, "{label} p {p}");
        }
    }
}

/// U3: empirical coverage over `reps` replications of n = 100: (order-statistic interval of the 0.9 quantile at 0.9,
/// the rank +-1 interval, Clopper-Pearson for p = 0.05 at 0.9, Wald for p = 0.01 at 0.9).
fn u3(seed: u64, reps: usize) -> (f64, f64, f64, f64) {
    let n = 100;
    let mut rng = Rng::new(seed);
    let d = Dist::Normal {
        mean: 0.0,
        sd: 1.0,
        lo: f64::NEG_INFINITY,
        hi: f64::INFINITY,
    };
    let q = norm_inv(0.9);
    let (mut c_os, mut c_naive, mut c_cp, mut c_wald) = (0, 0, 0, 0);
    let z = norm_inv(0.95);
    for _ in 0..reps {
        let mut x: Vec<f64> = (0..n).map(|_| d.inv_cdf(rng.uniform())).collect();
        x.sort_by(f64::total_cmp);
        let (lo, hi, _) = quantile_interval(&x, 0.9, 0.9);
        c_os += usize::from(lo <= q && q <= hi);
        c_naive += usize::from(x[88] <= q && q <= x[90]);
        let k = (0..n).filter(|_| rng.uniform() < 0.05).count();
        let (lo, hi) = clopper_pearson(k, n, 0.9);
        c_cp += usize::from(lo <= 0.05 && 0.05 <= hi);
        let k = (0..n).filter(|_| rng.uniform() < 0.01).count();
        let ph = k as f64 / n as f64;
        let w = z * (ph * (1.0 - ph) / n as f64).sqrt();
        c_wald += usize::from(ph - w <= 0.01 && 0.01 <= ph + w);
    }
    let f = |c: usize| c as f64 / reps as f64;
    (f(c_os), f(c_naive), f(c_cp), f(c_wald))
}

#[test]
fn u3_interval_coverage_is_the_stated_confidence() {
    let reps = 2000;
    let floor = 0.9 - K * (0.9 * 0.1 / reps as f64).sqrt();
    let (os, naive, cp, wald) = u3(46_005, reps);
    eprintln!(
        "U3: order statistic {os:.4}, rank+-1 {naive:.4}, Clopper-Pearson {cp:.4}, Wald {wald:.4}; floor {floor:.4}"
    );
    assert!(os >= floor && cp >= floor);
    assert!(naive < floor && wald < floor, "the negative controls must under-cover");
}

/// SRC test function y = x1 + 4 x2 + 10 x3 + 0.5 x1 x2, x ~ N(0, (2, 0.4, 0.1)^2): SRC_i = a_i s_i / s_y with
/// s_y^2 = sum a_i^2 s_i^2 + c^2 s_1^2 s_2^2 = 7.72, R^2 = 7.56 / 7.72.
fn src_params() -> Vec<UncertainParam> {
    vec![normal("x1", 0.0, 2.0), normal("x2", 0.0, 0.4), normal("x3", 0.0, 0.1)]
}
fn src_fn(x: &[f64]) -> f64 {
    x[0] + 4.0 * x[1] + 10.0 * x[2] + 0.5 * x[0] * x[1]
}
fn src_exact() -> (Vec<f64>, f64) {
    let (a, s) = ([1.0, 4.0, 10.0], [2.0, 0.4, 0.1]);
    let lin: f64 = (0..3).map(|i| (a[i] * s[i]) * (a[i] * s[i])).sum();
    let var = lin + 0.25 * 4.0 * 0.16;
    ((0..3).map(|i| a[i] * s[i] / var.sqrt()).collect(), lin / var)
}

/// U4: per sampler (ranking, max |SRC error|, |R^2 error|, ranking by raw coefficients).
#[allow(clippy::type_complexity)]
fn u4(seed_mc: u64, seed_lhs: u64, n: usize) -> Vec<(Sampler, Vec<String>, f64, f64, Vec<String>)> {
    let (src, r2) = src_exact();
    [(Sampler::MonteCarlo, seed_mc), (Sampler::Lhs, seed_lhs)]
        .iter()
        .map(|&(sampler, seed)| {
            let r = run(&fixed_spec(src_params(), sampler, seed, n), &model("src", src_fn), None).unwrap();
            let m = &r.metrics[0];
            let err = m.src.iter().zip(&src).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
            let mut raw: Vec<usize> = (0..3).collect();
            raw.sort_by(|&a, &b| m.coef[b].abs().total_cmp(&m.coef[a].abs()));
            let raw = raw.iter().map(|&j| format!("x{}", j + 1)).collect();
            (sampler, m.ranking.clone(), err, (m.r2 - r2).abs(), raw)
        })
        .collect()
}

#[test]
fn u4_src_ranking_recovers_the_known_order() {
    let truth = vec!["x1".to_string(), "x2".into(), "x3".into()];
    for (sampler, ranking, err, r2err, raw) in u4(46_006, 46_007, 20_000) {
        eprintln!("U4 {sampler:?}: ranking {ranking:?}, max |SRC err| {err:.5}, |R2 err| {r2err:.5}, raw-coefficient ranking {raw:?}");
        assert_eq!(ranking, truth);
        assert!(err <= 0.02 && r2err <= 0.01);
        assert_ne!(raw, truth, "negative control: unstandardised coefficients mis-rank");
    }
}

/// U5: stopping rule. Returns (MC linear: n, se, |mean err|, converged), (LHS SRC function: same), (capped MC: n, se, converged).
#[allow(clippy::type_complexity)]
fn u5(seed_mc: u64, seed_lhs: u64) -> ((usize, f64, f64, bool), (usize, f64, f64, bool), (usize, f64, bool)) {
    let rule = |batch, min_batches, max_samples, se_abs| StopRule {
        batch,
        min_batches,
        max_samples,
        se_abs,
        se_rel: 0.0,
        metrics: vec!["y".into()],
    };
    let mut s = fixed_spec(lin_params(), Sampler::MonteCarlo, seed_mc, 0);
    s.stop = rule(500, 1, 100_000, 0.01);
    let r = run(&s, &model("a", lin), None).unwrap();
    let mc = (
        r.n_samples,
        r.metrics[0].se,
        (r.metrics[0].mean - lin_exact().0).abs(),
        r.converged,
    );
    let mut s = fixed_spec(src_params(), Sampler::Lhs, seed_lhs, 0);
    s.stop = rule(200, 10, 100_000, 0.005);
    let r = run(&s, &model("a", src_fn), None).unwrap();
    let lhs = (r.n_samples, r.metrics[0].se, r.metrics[0].mean.abs(), r.converged);
    let mut s = fixed_spec(lin_params(), Sampler::MonteCarlo, seed_mc, 0);
    s.stop = rule(500, 1, 2000, 0.01);
    let r = run(&s, &model("a", lin), None).unwrap();
    (mc, lhs, (r.n_samples, r.metrics[0].se, r.converged))
}

#[test]
fn u5_stopping_rule_on_the_standard_error() {
    let (mc, lhs, capped) = u5(46_008, 46_009);
    eprintln!("U5 MC {mc:?}, LHS {lhs:?}, capped {capped:?}");
    let n_star = lin_exact().1 / 0.01f64.powi(2);
    assert!(mc.3 && mc.1 <= 0.01 && mc.2 <= K * 0.01);
    assert!(
        mc.0 as f64 >= 0.95 * n_star && mc.0 as f64 <= 1.05 * n_star + 500.0,
        "n {} vs {n_star}",
        mc.0
    );
    assert!(lhs.3 && lhs.1 <= 0.005 && lhs.2 <= 5.0 * 0.005);
    assert!(
        !capped.2 && capped.0 == 2000 && capped.1 > 0.01,
        "the capped run must not claim the target"
    );
}

/// U6: ledger bytes of a study (fixed clock) for the given seed, sampler and worker count.
fn u6_bytes(tag: &str, sampler: Sampler, seed: u64, workers: usize) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!("rcfd_w46d_u6_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut l = Ledger::new(&dir.join("ledger.jsonl"));
    l.clock = || 1;
    let mut s = fixed_spec(src_params(), sampler, seed, 2000);
    s.stop.batch = 500;
    s.workers = workers;
    s.requirements = vec![Requirement {
        metric: "y".into(),
        cmp: Cmp::Le,
        limit: 2.0,
    }];
    let r = run(&s, &model("src", src_fn), Some(&l)).unwrap();
    assert_eq!(r.ledger_range, Some((0, 2000)));
    let c = l.read().unwrap();
    assert_eq!(c.entries.len(), 2001);
    assert_eq!(c.entries[2000].kind, "uq_summary");
    std::fs::read(l.path()).unwrap()
}

#[test]
fn u6_same_seed_gives_a_byte_identical_ledger() {
    for sampler in [Sampler::MonteCarlo, Sampler::Lhs] {
        let a = u6_bytes("a", sampler, 46_010, 1);
        let b = u6_bytes("b", sampler, 46_010, 3);
        let c = u6_bytes("c", sampler, 46_011, 1);
        eprintln!(
            "U6 {sampler:?}: {} bytes, identical {}, other seed differs {}",
            a.len(),
            a == b,
            a != c
        );
        assert!(a == b, "{sampler:?}: same seed, 1 and 3 workers");
        assert!(a != c, "{sampler:?}: negative control, another seed");
    }
}

/// U7: z of mean and variance of each input distribution (the last one is the negative control).
fn u7(seed: u64, n: usize) -> Vec<(String, f64, f64)> {
    let phi = |x: f64| (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let (a, b) = (-1.0, 2.0);
    let zt = norm_cdf(b) - norm_cdf(a);
    let tn_mean = 1.0 + 0.5 * (phi(a) - phi(b)) / zt;
    let tn_var = 0.25 * (1.0 + (a * phi(a) - b * phi(b)) / zt - ((phi(a) - phi(b)) / zt).powi(2));
    let beta = |p: f64, q: f64| (p / (p + q), p * q / ((p + q).powi(2) * (p + q + 1.0)));
    let cases = [
        ("uniform(2, 5)", Dist::Uniform { lo: 2.0, hi: 5.0 }, (3.5, 0.75)),
        (
            "normal(1, 0.5) on [0.5, 2]",
            Dist::Normal {
                mean: 1.0,
                sd: 0.5,
                lo: 0.5,
                hi: 2.0,
            },
            (tn_mean, tn_var),
        ),
        (
            "beta(2, 5)",
            Dist::Beta {
                a: 2.0,
                b: 5.0,
                lo: 0.0,
                hi: 1.0,
            },
            beta(2.0, 5.0),
        ),
        (
            "beta(2, 5) vs beta(2, 4) WRONG",
            Dist::Beta {
                a: 2.0,
                b: 5.0,
                lo: 0.0,
                hi: 1.0,
            },
            beta(2.0, 4.0),
        ),
    ];
    cases
        .iter()
        .map(|(label, dist, (mu, var))| {
            let spec = fixed_spec(
                vec![UncertainParam {
                    name: "x".into(),
                    unit: "1".into(),
                    dist: dist.clone(),
                }],
                Sampler::MonteCarlo,
                seed,
                n,
            );
            let r = run(&spec, &model("id", |x: &[f64]| x[0]), None).unwrap();
            let m = &r.metrics[0];
            let nn = m.n as f64;
            // fourth central moment estimated from the sample (needed only for the SE of the variance)
            let ys: Vec<f64> = {
                let mut rng = Rng::new(seed);
                (0..n)
                    .map(|_| dist.inv_cdf(((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64))
                    .collect()
            };
            let mean_y = ys.iter().sum::<f64>() / nn;
            let m4 = ys.iter().map(|y| (y - mean_y).powi(4)).sum::<f64>() / nn;
            let s2 = m.sd * m.sd;
            (
                (*label).to_string(),
                (m.mean - mu) / (var / nn).sqrt(),
                (s2 - var) / ((m4 - s2 * s2) / nn).sqrt(),
            )
        })
        .collect()
}

#[test]
fn u7_input_distributions_and_the_normal_quantile() {
    // standard normal quantiles (standard table values quoted from memory, UNVERIFIED in the last digit; limit 1e-12)
    for (p, x) in [
        (0.975, 1.959963984540054),
        (0.95, 1.6448536269514722),
        (0.999, 3.090232306167813),
        (0.5, 0.0),
    ] {
        let got = norm_inv(p);
        eprintln!("U7 norm_inv({p}) = {got:.16} (table {x})");
        assert!((got - x).abs() <= 1e-12 && (norm_inv(1.0 - p) + x).abs() <= 1e-12);
    }
    for (label, zm, zv) in u7(46_012, 20_000) {
        eprintln!("U7 {label}: z_mean {zm:+.3}, z_var {zv:+.3}");
        if label.ends_with("WRONG") {
            assert!(zm.abs() > K);
        } else {
            assert!(zm.abs() <= K && zv.abs() <= K, "{label}");
        }
    }
}

/// U8: failures and requirements. x ~ U(0, 1) mm; diverged for x > 0.9, panics for x < 0.002, else y = x; requirement y <= 0.8.
/// Truth: P(fail) = 0.102, P(violation or fail) = 0.202. Returns (p, lo, hi, n_ok / n, ledger diverged entries with a reason,
/// p ignoring failures).
fn u8(seed: u64, n: usize) -> (f64, f64, f64, f64, usize, f64) {
    let dir = std::env::temp_dir().join(format!("rcfd_w46d_u8_{seed}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let l = Ledger::new(&dir.join("ledger.jsonl"));
    let m = FnModel {
        id: "gap".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| {
            let x = p[0].value;
            assert!(x >= 0.002, "deliberate panic at gap {x}");
            if x > 0.9 {
                return Outcome {
                    verdict: Verdict::Diverged,
                    reason: format!("gap {x} mm > 0.9 mm: blade rubs the casing"),
                    ..Default::default()
                };
            }
            Outcome::ok(vec![Quantity::new("y", x, "mm")])
        },
    };
    let mut s = fixed_spec(
        vec![UncertainParam {
            name: "gap".into(),
            unit: "mm".into(),
            dist: Dist::Uniform { lo: 0.0, hi: 1.0 },
        }],
        Sampler::MonteCarlo,
        seed,
        n,
    );
    s.stop.batch = 1000;
    s.requirements = vec![Requirement {
        metric: "y".into(),
        cmp: Cmp::Le,
        limit: 0.8,
    }];
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = run(&s, &m, Some(&l));
    std::panic::set_hook(hook);
    let r = r.unwrap();
    let rq = &r.requirements[0];
    let failed_with_reason = l
        .read()
        .unwrap()
        .entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.verdict == Verdict::Diverged && !e.reason.is_empty())
        .count();
    assert_eq!(failed_with_reason, rq.failed);
    (
        rq.p,
        rq.lo,
        rq.hi,
        r.n_ok as f64 / r.n_samples as f64,
        rq.failed,
        rq.violations as f64 / r.n_samples as f64,
    )
}

#[test]
fn u8_failures_count_as_violations() {
    let n = 5000;
    let (p, lo, hi, ok_frac, failed, p_ignoring) = u8(46_013, n);
    eprintln!("U8: p {p:.4} in [{lo:.4}, {hi:.4}], ok fraction {ok_frac:.4}, failed {failed}, p ignoring failures {p_ignoring:.4}");
    assert!(lo <= 0.202 && 0.202 <= hi);
    assert!((ok_frac - 0.898).abs() <= K * (0.898 * 0.102 / n as f64).sqrt());
    assert!(
        !(lo <= p_ignoring && p_ignoring <= hi),
        "negative control: ignoring failures falls outside"
    );
}

/// Rule-d smoke (ignored): every set-up with seeds other than the registered ones, and the cumulant formula at large n.
#[test]
#[ignore]
fn smoke_w46d() {
    for x in u1(1, 2, 20_000) {
        eprintln!("smoke U1 {x:?}");
    }
    let r = run(
        &fixed_spec(quad_params(), Sampler::MonteCarlo, 7, 400_000),
        &model("q", quad),
        None,
    )
    .unwrap();
    eprintln!("smoke cumulants at n = 4e5: z {:?}", zs(&r, quad_exact()));
    for x in u2(3, 4, 20_000) {
        eprintln!("smoke U2 {x:?}");
    }
    eprintln!("smoke U3 {:?}", u3(5, 2000));
    for x in u4(6, 7, 20_000) {
        eprintln!("smoke U4 {:?} {:?} {} {} {:?}", x.0, x.1, x.2, x.3, x.4);
    }
    eprintln!("smoke U5 {:?}", u5(8, 9));
    eprintln!(
        "smoke U6 identical {}",
        u6_bytes("s1", Sampler::Lhs, 10, 1) == u6_bytes("s2", Sampler::Lhs, 10, 3)
    );
    for x in u7(12, 20_000) {
        eprintln!("smoke U7 {x:?}");
    }
    eprintln!("smoke U8 {:?}", u8(13, 5000));
    // binomial CDF against direct summation
    let (n, p) = (40usize, 0.3f64);
    let mut acc = 0.0;
    let mut worst: f64 = 0.0;
    for k in 0..n {
        let lnc: f64 = (1..=k).map(|i| ((n - k + i) as f64 / i as f64).ln()).sum();
        acc += (lnc + k as f64 * p.ln() + (n - k) as f64 * (1.0 - p).ln()).exp();
        worst = worst.max((binom_cdf(k as i64, n, p) - acc).abs());
    }
    eprintln!("smoke binomial CDF max error {worst:e}");
}
