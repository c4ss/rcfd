//! ADR 0652: checks of rung R3 of the fidelity driver (F-718), a zero-mean Gaussian process on the R2 residual. Synthetic
//! sources only.

use rcfd_api::design::fidelity::{
    self, loo_residuals, r3_log_likelihood, r3_loo, Candidates, CorrectedMetric, Correction, FidelityResult,
    FidelitySpec, HighSource, MetricFit, ModelSource, Observation, PairData, ParamBox, Privacy, R3Rule, Rung,
    TrustRule,
};
use rcfd_api::design::ledger::{self, Fidelity, FnModel, Ledger, Model, Outcome, Quantity, Source};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::study::Sense;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;

type EvalFn = Box<dyn Fn(&[Quantity]) -> Outcome + Sync + Send>;
type Fm = FnModel<EvalFn>;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f718_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger_at(p: &Path) -> Ledger {
    let mut l = Ledger::new(p);
    l.clock = || CLOCK;
    l
}

fn p(x: &[Quantity], n: &str) -> f64 {
    x.iter().find(|q| q.name == n).unwrap().value
}

fn normals(seed: u64, n: usize) -> Vec<f64> {
    let mut g = Rng::new(seed);
    (0..n)
        .map(|_| {
            let (u1, u2) = (g.uniform(), g.uniform());
            (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        })
        .collect()
}

fn r3_trust() -> TrustRule {
    TrustRule {
        r3: Some(R3Rule::default()),
        ..TrustRule::default()
    }
}

// ---------------------------------------------------------------- N1, N2

fn n1_params() -> Vec<ParamBox> {
    vec![ParamBox::new("x1", "m", 0.0, 1.0), ParamBox::new("x2", "1", -1.0, 1.0)]
}

fn n1_low(x1: f64, x2: f64) -> f64 {
    (3.0 * x1).sin() + x2 * x2
}

fn n1_affine(x1: f64, x2: f64) -> f64 {
    1.3 * n1_low(x1, x2) - 0.2 + 0.5 * x1 - 0.3 * x2
}

fn n1_high(x1: f64, x2: f64) -> f64 {
    n1_affine(x1, x2) + 0.3 * (2.0 * std::f64::consts::PI * x1).sin() * (1.5 * x2).cos()
}

fn n_pairs(lhs_seed: u64, structured: bool, noise: f64, noise_seed: u64) -> Vec<PairData> {
    let pts = sampling::latin_hypercube(40, 2, lhs_seed);
    let z = normals(noise_seed, pts.len());
    pts.iter()
        .zip(&z)
        .map(|(u, z)| {
            let (x1, x2) = (u[0], -1.0 + 2.0 * u[1]);
            let h = if structured { n1_high(x1, x2) } else { n1_affine(x1, x2) };
            PairData {
                x: vec![x1, x2],
                low: vec![n1_low(x1, x2)],
                high: vec![h + noise * z],
                u_high: vec![noise],
            }
        })
        .collect()
}

fn fit_one(pairs: &[PairData], params: &[ParamBox], trust: &TrustRule, names: &[&str]) -> Correction {
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    Correction::fit(pairs, params, &names, trust, ("low", "1"), ("high", "1"))
}

// ---------------------------------------------------------------- independent arithmetic of the test

fn unit(params: &[ParamBox], x: &[f64]) -> Vec<f64> {
    params.iter().zip(x).map(|(p, v)| (v - p.lo) / (p.hi - p.lo)).collect()
}

fn se_corr(a: &[f64], b: &[f64], l: &[f64]) -> f64 {
    (-0.5
        * a.iter()
            .zip(b)
            .zip(l)
            .map(|((x, y), s)| ((x - y) / s).powi(2))
            .sum::<f64>())
    .exp()
}

/// Gauss-Jordan with partial pivoting.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, piv);
        b.swap(c, piv);
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                if f != 0.0 {
                    for j in c..n {
                        a[r][j] -= f * a[c][j];
                    }
                    b[r] -= f * b[c];
                }
            }
        }
    }
    (0..n).map(|i| b[i] / a[i][i]).collect()
}

/// Brute-force R3 leave-one-out residual of pair `i` for high values `y`: R2 refitted by normal equations on the other
/// pairs, then the zero-mean process with `(l, sf2)` and the stated uncertainties on their residuals.
fn brute_e(u: &[Vec<f64>], yl: &[f64], y: &[f64], sig: &[f64], l: &[f64], sf2: f64, i: usize) -> f64 {
    let n = y.len();
    let row = |k: usize| [vec![yl[k], 1.0], u[k].clone()].concat();
    let rest: Vec<usize> = (0..n).filter(|&k| k != i).collect();
    let p = row(0).len();
    let mut ata = vec![vec![0.0; p]; p];
    let mut aty = vec![0.0; p];
    for &k in &rest {
        let rk = row(k);
        for a in 0..p {
            aty[a] += rk[a] * y[k];
            for b in 0..p {
                ata[a][b] += rk[a] * rk[b];
            }
        }
    }
    let b = solve(ata, aty);
    let fit = |k: usize| row(k).iter().zip(&b).map(|(x, c)| x * c).sum::<f64>();
    let rr: Vec<f64> = rest.iter().map(|&k| y[k] - fit(k)).collect();
    let c: Vec<Vec<f64>> = rest
        .iter()
        .map(|&a| {
            rest.iter()
                .map(|&bb| {
                    if a == bb {
                        sf2 * (1.0 + fidelity::R3_JITTER) + sig[a] * sig[a]
                    } else {
                        sf2 * se_corr(&u[a], &u[bb], l)
                    }
                })
                .collect()
        })
        .collect();
    let w = solve(c, rr);
    let mu: f64 = rest
        .iter()
        .zip(&w)
        .map(|(&k, wk)| sf2 * se_corr(&u[i], &u[k], l) * wk)
        .sum();
    y[i] - fit(i) - mu
}

struct LooCheck {
    max_e: f64,
    max_sd: f64,
    rms_r: f64,
}

/// L3 and L3s for metric `m`: closed form against brute force, both with the fit's hyperparameters.
fn loo_check(pairs: &[PairData], params: &[ParamBox], m: usize, l: &[f64], sf2: f64) -> LooCheck {
    let got = r3_loo(pairs, params, m, &R3Rule::default(), Some((l, sf2))).unwrap();
    let n = pairs.len();
    let u: Vec<Vec<f64>> = pairs.iter().map(|q| unit(params, &q.x)).collect();
    let yl: Vec<f64> = pairs.iter().map(|q| q.low[m]).collect();
    let y: Vec<f64> = pairs.iter().map(|q| q.high[m]).collect();
    let sig: Vec<f64> = pairs.iter().map(|q| q.u_high[m]).collect();
    let rms_r = (got.r.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
    let cfull: Vec<Vec<f64>> = (0..n)
        .map(|a| {
            (0..n)
                .map(|b| {
                    if a == b {
                        sf2 * (1.0 + fidelity::R3_JITTER) + sig[a] * sig[a]
                    } else {
                        sf2 * se_corr(&u[a], &u[b], l)
                    }
                })
                .collect()
        })
        .collect();
    let mut max_e: f64 = 0.0;
    let mut max_sd: f64 = 0.0;
    for i in 0..n {
        let e = brute_e(&u, &yl, &y, &sig, l, sf2, i);
        max_e = max_e.max((got.e[i] - e).abs());
        let a: Vec<f64> = (0..n)
            .map(|k| {
                let mut ek = vec![0.0; n];
                ek[k] = 1.0;
                brute_e(&u, &yl, &ek, &sig, l, sf2, i)
            })
            .collect();
        let var: f64 = (0..n)
            .map(|k| a[k] * (0..n).map(|j| cfull[k][j] * a[j]).sum::<f64>())
            .sum();
        max_sd = max_sd.max((got.sd[i] - var.max(0.0).sqrt()).abs());
    }
    LooCheck { max_e, max_sd, rms_r }
}

fn mean_sq(e: &[f64]) -> f64 {
    e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64
}

fn se_sq(e: &[f64]) -> f64 {
    let n = e.len();
    let m = mean_sq(e);
    (e.iter().map(|x| (x * x - m).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt() / (n as f64).sqrt()
}

/// S1: the one-standard-error rule recomputed in the test from leave-one-out residuals.
fn expected_rung(pairs: &[PairData], params: &[ParamBox], m: usize, f: &MetricFit) -> Rung {
    let mut lv: Vec<(Rung, f64, f64)> = Vec::new();
    let r0 = loo_residuals(pairs, params, m, Rung::R0).unwrap();
    lv.push((Rung::R0, mean_sq(&r0), se_sq(&r0)));
    let add = loo_residuals(pairs, params, m, Rung::R1Add);
    let mul = loo_residuals(pairs, params, m, Rung::R1Mul);
    let l1 = match (add, mul) {
        (Some(a), Some(b)) => Some(if mean_sq(&b) < mean_sq(&a) {
            (Rung::R1Mul, b)
        } else {
            (Rung::R1Add, a)
        }),
        (Some(a), None) => Some((Rung::R1Add, a)),
        (None, Some(b)) => Some((Rung::R1Mul, b)),
        (None, None) => None,
    };
    if let Some((r, e)) = l1 {
        lv.push((r, mean_sq(&e), se_sq(&e)));
    }
    if let Some(e) = loo_residuals(pairs, params, m, Rung::R2) {
        lv.push((Rung::R2, mean_sq(&e), se_sq(&e)));
    }
    if let Some(g) = f.r3.as_ref().filter(|g| g.available) {
        let e = r3_loo(pairs, params, m, &R3Rule::default(), Some((&g.l, g.sf2)))
            .unwrap()
            .e;
        lv.push((Rung::R3, mean_sq(&e), se_sq(&e)));
    }
    let kmin = (0..lv.len()).min_by(|&a, &b| lv[a].1.total_cmp(&lv[b].1)).unwrap();
    let bound = lv[kmin].1 + lv[kmin].2;
    lv.iter().find(|x| x.1 <= bound).unwrap().0
}

// ---------------------------------------------------------------- N1a, N1b, N2: S1, S2, L3, L3s, C1, C2

#[test]
#[ignore = "ADR 0652 amendment 1: registered run, L3s on N1a 6.3e-8 of rms(r) > 1e-8; N1b and N2 not reached"]
fn n1_n2_choice_loo_and_coverage_known_miss() {
    let params = n1_params();
    let cases = [
        ("N1a", n_pairs(718_011, true, 0.0, 718_013)),
        ("N1b", n_pairs(718_012, true, 0.02, 718_013)),
        ("N2", n_pairs(718_021, false, 0.05, 718_022)),
    ];
    for (tag, pairs) in &cases {
        let c = fit_one(pairs, &params, &r3_trust(), &["f"]);
        let f = &c.fits[0];
        let g = f.r3.as_ref().unwrap();
        assert!(g.available, "{tag}: {}", g.why);
        // S1
        let want = expected_rung(pairs, &params, 0, f);
        assert_eq!(f.rung, want, "{tag}: S1");
        // L3, L3s
        let lc = loo_check(pairs, &params, 0, &g.l, g.sf2);
        let ratio = (g.mse / f.mse[3].unwrap()).sqrt();
        eprintln!(
            "{tag}: rung {} trusted {} ({}); l {:?} sf2 {:.4e}; MSE R2 {:.4e} R3 {:.4e} (SE {:.2e}); RMSE ratio {ratio:.4}; coverage {:.3}; L3 {:.2e} L3s {:.2e} of rms(r) {:.4e}",
            f.rung.as_str(),
            f.trusted,
            f.why,
            g.l,
            g.sf2,
            f.mse[3].unwrap(),
            g.mse,
            g.se,
            g.coverage,
            lc.max_e / lc.rms_r,
            lc.max_sd / lc.rms_r,
            lc.rms_r
        );
        assert!(lc.max_e <= 1e-8 * lc.rms_r, "{tag}: L3 {}", lc.max_e / lc.rms_r);
        assert!(lc.max_sd <= 1e-8 * lc.rms_r, "{tag}: L3s {}", lc.max_sd / lc.rms_r);
        match *tag {
            "N1a" => {
                assert_eq!(f.rung, Rung::R3, "S2");
                assert!(g.coverage >= 0.85, "C1 {}", g.coverage);
                assert!(ratio <= 0.2, "C2 {ratio}");
            }
            "N1b" => {
                assert_eq!(f.rung, Rung::R3, "S2");
                assert!(g.coverage >= 0.85, "C1 {}", g.coverage);
                assert!(ratio <= 0.5, "C2 {ratio}");
            }
            _ => assert_ne!(f.rung, Rung::R3, "S2: R3 chosen on the unstructured N2"),
        }
    }
}

// ---------------------------------------------------------------- N3: exact affine, three parameters

fn s1_params() -> Vec<ParamBox> {
    vec![
        ParamBox::new("x1", "m", 0.0, 1.0),
        ParamBox::new("x2", "1", -1.0, 2.0),
        ParamBox::new("x3", "s", 10.0, 20.0),
    ]
}

#[test]
#[ignore = "ADR 0652 amendment 1: registered run, R3 chosen on the rounding-level residual (MSE R2 2.2e-30, R3 1.6e-30)"]
fn n3_exact_affine_keeps_r2_known_miss() {
    let params = s1_params();
    let pairs: Vec<PairData> = sampling::latin_hypercube(40, 3, 718_031)
        .iter()
        .map(|u| {
            let x: Vec<f64> = u.iter().zip(&params).map(|(v, p)| p.lo + v * (p.hi - p.lo)).collect();
            let (x1, x2, x3) = (x[0], x[1], x[2]);
            let f = (3.0 * x1).sin() + x2 * x2 + 0.1 * x3;
            let g = (0.5 * x1).exp() - x2 * x3 / 20.0;
            PairData {
                low: vec![f, g],
                high: vec![
                    1.7 * f - 0.3 + 0.5 * x1 - 2.0 * x2 + 0.04 * x3,
                    0.9 * g + 1.25 - 0.1 * x1 + 0.3 * x2 - 0.02 * x3,
                ],
                u_high: vec![0.0, 0.0],
                x,
            }
        })
        .collect();
    let on = fit_one(&pairs, &params, &r3_trust(), &["f", "g"]);
    let off = fit_one(&pairs, &params, &TrustRule::default(), &["f", "g"]);
    for (m, (a, b)) in on.fits.iter().zip(&off.fits).enumerate() {
        let g = a.r3.as_ref().unwrap();
        eprintln!(
            "N3 {}: rung {} (off {}); R3 available {} ({}); MSE R2 {:.3e} R3 {:.3e}",
            a.metric,
            a.rung.as_str(),
            b.rung.as_str(),
            g.available,
            g.why,
            a.mse[3].unwrap(),
            g.mse
        );
        assert_eq!(a.rung, expected_rung(&pairs, &params, m, a), "S1");
        assert_eq!(a.rung, Rung::R2, "S2");
        assert_eq!(b.rung, Rung::R2);
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&a.coef), bits(&b.coef), "S2: R2 coefficients");
    }
}

// ---------------------------------------------------------------- NC-C, K1

#[test]
fn nc_c_coverage_guard() {
    let params = n1_params();
    let pairs = n_pairs(718_011, true, 0.0, 718_013);
    let t = TrustRule {
        r3: Some(R3Rule {
            coverage_min: 1.01,
            ..R3Rule::default()
        }),
        ..TrustRule::default()
    };
    let c = fit_one(&pairs, &params, &t, &["f"]);
    let f = &c.fits[0];
    eprintln!("NC-C: rung {} trusted {} why '{}'", f.rung.as_str(), f.trusted, f.why);
    assert_eq!(f.rung, Rung::R3);
    assert!(!f.trusted);
    assert!(f.why.contains("coverage"), "{}", f.why);
    for q in &pairs {
        let (v, fl) = c.predict(&q.x, &q.low);
        assert!(!fl[0]);
        assert_eq!(v[0].to_bits(), q.low[0].to_bits());
    }
}

#[test]
fn k1_likelihood_gradient() {
    let params = n1_params();
    let pairs = n_pairs(718_012, true, 0.02, 718_013);
    let o = r3_loo(&pairs, &params, 0, &R3Rule::default(), None).unwrap();
    let u: Vec<Vec<f64>> = pairs.iter().map(|q| unit(&params, &q.x)).collect();
    let r: Vec<f64> = o.r.iter().map(|v| v / o.s).collect();
    let sig: Vec<f64> = pairs.iter().map(|q| q.u_high[0] / o.s).collect();
    let fitted = vec![o.l[0].ln(), o.l[1].ln(), (o.sf2 / (o.s * o.s)).ln()];
    for th in [
        fitted,
        vec![0.3f64.ln(), 0.3f64.ln(), 0.0],
        vec![2f64.ln(), 0.1f64.ln(), 0.01f64.ln()],
    ] {
        let (_, g) = r3_log_likelihood(&u, &r, &sig, &th).unwrap();
        let gmax = g.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        for k in 0..3 {
            let h = 1e-5;
            let mut a = th.clone();
            let mut b = th.clone();
            a[k] += h;
            b[k] -= h;
            let fd = (r3_log_likelihood(&u, &r, &sig, &a).unwrap().0 - r3_log_likelihood(&u, &r, &sig, &b).unwrap().0)
                / (2.0 * h);
            let err = (g[k] - fd).abs();
            eprintln!(
                "K1 theta {th:?} k {k}: analytic {:.8e} fd {:.8e} rel {:.2e}",
                g[k],
                fd,
                err / fd.abs()
            );
            if fd.abs() < 1e-3 * gmax {
                assert!(err <= 1e-7, "K1 small component");
            } else {
                assert!(err <= 1e-5 * fd.abs(), "K1 rel {}", err / fd.abs());
            }
        }
    }
}

// ---------------------------------------------------------------- ND: N1 through the driver

fn n1_low_model() -> Fm {
    FnModel {
        id: "analytic.n1".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: Box::new(|x| Outcome::ok(vec![Quantity::new("f", n1_low(p(x, "x1"), p(x, "x2")), "Pa")])),
    }
}

fn n1_high_model() -> Fm {
    FnModel {
        id: "synthetic.n1".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: Box::new(|x| Outcome::ok(vec![Quantity::new("f", n1_high(p(x, "x1"), p(x, "x2")), "Pa")])),
    }
}

fn nd_spec(workers: usize) -> FidelitySpec {
    let mut s = FidelitySpec::new(
        "nd",
        n1_params(),
        Candidates::Lhs { n: 300, seed: 718_041 },
        vec![CorrectedMetric {
            name: "f".into(),
            floor: 1e-3,
        }],
        "f",
        Sense::Min,
    );
    s.rounds = 6;
    s.trust = r3_trust();
    s.workers = workers;
    s
}

fn pairs_upto(r: &FidelityResult, round: usize) -> Vec<PairData> {
    r.pairs
        .iter()
        .filter(|q| q.round <= round)
        .map(|q| PairData {
            x: r.candidates[q.candidate].clone(),
            low: q.low.clone(),
            high: q.high.clone(),
            u_high: vec![0.0],
        })
        .collect()
}

#[test]
#[ignore = "ADR 0652 amendment 1: registered run, L3s of the final fit 1.63e-8 of rms(r) > 1e-8; D2, X1, T6 not reached"]
fn nd_driver_with_r3_known_miss() {
    let d = tmp("nd");
    let low = n1_low_model();
    let hm = n1_high_model();
    let src = ModelSource {
        level: Some("high".into()),
        ..ModelSource::of(&hm)
    };
    let p1 = d.join("w1.jsonl");
    let p8 = d.join("w8.jsonl");
    let r = fidelity::run(&nd_spec(1), &low, &src, &ledger_at(&p1)).unwrap();
    let r8 = fidelity::run(&nd_spec(8), &low, &src, &ledger_at(&p8)).unwrap();
    // D4
    let (b1, b8) = (std::fs::read(&p1).unwrap(), std::fs::read(&p8).unwrap());
    assert_eq!(b1, b8, "D4");
    assert_eq!(r, r8);
    eprintln!("D4: {} bytes, equal at 1 and 8 workers", b1.len());
    // D3
    let c = ledger::parse_text(&String::from_utf8(b1).unwrap());
    assert_eq!(c.skipped.len(), 0);
    let es = &c.entries;
    let count = |k: &str, ev: &str| es.iter().filter(|e| e.kind == k && e.evaluator == ev).count();
    assert_eq!(count("evaluation", "analytic.n1"), 300);
    assert_eq!(count("evaluation", "synthetic.n1"), 36);
    assert_eq!(es.iter().filter(|e| e.kind == "fidelity_pair").count(), 36);
    let sums: Vec<_> = es.iter().filter(|e| e.kind == "fidelity_summary").collect();
    assert_eq!(sums.len(), 6);
    for (k, e) in sums.iter().enumerate() {
        let text = e.settings["correction"].to_string();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(e.fitted.as_ref().unwrap().hash, format!("fnv1a64:{h:016x}"));
        let r3 = &e.settings["correction"]["fits"][0]["r3"];
        assert!(r3.is_object(), "D3: round {} has no r3 object", k + 1);
        if k >= 3 {
            assert_eq!(r3["available"], serde_json::json!(true), "D3: round {}", k + 1);
        }
    }
    // S1 in every round
    let params = n1_params();
    for (k, rd) in r.rounds.iter().enumerate() {
        let pairs = pairs_upto(&r, k + 1);
        let f = &rd.correction.as_ref().unwrap().fits[0];
        let want = expected_rung(&pairs, &params, 0, f);
        let g = f.r3.as_ref().unwrap();
        eprintln!(
            "ND round {}: {} pairs, rung {} trusted {} ({}); R3 available {} mse {:.3e} coverage {:.3}; R2 mse {:?}",
            k + 1,
            pairs.len(),
            f.rung.as_str(),
            f.trusted,
            f.why,
            g.available,
            g.mse,
            g.coverage,
            f.mse[3]
        );
        assert_eq!(f.rung, want, "S1 round {}", k + 1);
    }
    // D1, C1, L3 on the final fit
    let cor = r.correction.as_ref().unwrap();
    let f = &cor.fits[0];
    let g = f.r3.as_ref().unwrap();
    assert_eq!(f.rung, Rung::R3, "D1");
    assert!(f.trusted, "D1: {}", f.why);
    assert!(g.coverage >= 0.85, "C1 {}", g.coverage);
    let pairs = pairs_upto(&r, 6);
    let lc = loo_check(&pairs, &params, 0, &g.l, g.sf2);
    eprintln!(
        "ND final: L3 {:.2e}, L3s {:.2e} of rms(r) {:.3e}; coverage {:.3}",
        lc.max_e / lc.rms_r,
        lc.max_sd / lc.rms_r,
        lc.rms_r,
        g.coverage
    );
    assert!(lc.max_e <= 1e-8 * lc.rms_r, "L3");
    assert!(lc.max_sd <= 1e-8 * lc.rms_r, "L3s");
    // D2
    let off = fit_one(&pairs, &params, &TrustRule::default(), &["f"]);
    let promoted: Vec<usize> = r.pairs.iter().map(|q| q.candidate).collect();
    let (mut s3, mut s2, mut nh) = (0.0, 0.0, 0usize);
    for (i, x) in r.candidates.iter().enumerate() {
        if promoted.contains(&i) || r.low[i].is_none() || !cor.region.contains(&unit(&params, x)) {
            continue;
        }
        let lv = r.low[i].as_ref().unwrap();
        let h = n1_high(x[0], x[1]);
        s3 += (h - cor.predict(x, lv).0[0]).powi(2);
        s2 += (h - off.predict(x, lv).0[0]).powi(2);
        nh += 1;
    }
    let ratio = (s3 / s2).sqrt();
    eprintln!(
        "D2: {nh} held-out candidates inside the region; RMSE R3 {:.4e}, R2-only fit {:.4e} (rung {} trusted {}); ratio {ratio:.4}",
        (s3 / nh as f64).sqrt(),
        (s2 / nh as f64).sqrt(),
        off.fits[0].rung.as_str(),
        off.fits[0].trusted
    );
    assert!(nh > 0 && ratio <= 0.5, "D2 {ratio}");
    // X1
    let qs: Vec<Vec<f64>> = sampling::latin_hypercube(500, 2, 718_051)
        .iter()
        .map(|u| vec![u[0], -1.0 + 2.0 * u[1]])
        .collect();
    let mut n_flag = 0;
    let mut worst: f64 = 0.0;
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (v, fl) = cor.predict(x, &yl);
        if fl[0] {
            n_flag += 1;
            let want = if f.constant_trusted { yl[0] + f.constant } else { yl[0] };
            assert_eq!(v[0].to_bits(), want.to_bits(), "X1 flagged");
        } else {
            let uq = unit(&params, x);
            let mu: f64 = cor
                .region
                .points
                .iter()
                .zip(&g.beta)
                .map(|(pt, b)| g.sf2 * se_corr(pt, &uq, &g.l) * b)
                .sum();
            let want = f.coef[0] * yl[0] + f.coef[1] + f.coef[2] * x[0] + f.coef[3] * x[1] + mu;
            let e = (v[0] - want).abs() / want.abs().max(1e-300);
            worst = worst.max(e);
            assert!(e <= 1e-12, "X1 unflagged {e}");
        }
    }
    eprintln!("X1: {n_flag} of 500 flagged; unflagged largest relative difference {worst:.2e}");
    assert!(n_flag >= 1, "X1: no query flagged");
    // T6
    let back = Correction::from_json(&cor.to_json()).unwrap();
    assert_eq!(back.hash(), cor.hash());
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (a, fa) = cor.predict(x, &yl);
        let (b, fb) = back.predict(x, &yl);
        assert_eq!(a[0].to_bits(), b[0].to_bits(), "T6");
        assert_eq!(fa, fb);
    }
    eprintln!("T6: round trip bit-equal at 500 queries, hash {}", cor.hash());
}

// ---------------------------------------------------------------- P6, NC-P

struct Private<'m>(&'m dyn Model);

impl HighSource for Private<'_> {
    fn id(&self) -> &str {
        "rig.n1"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Measured
    }
    fn private(&self) -> bool {
        true
    }
    fn source(&self) -> Option<Source> {
        Some(Source {
            id: "rig-n1".into(),
            kind: "measured".into(),
            citation: None,
            file_sha256: None,
        })
    }
    fn observe(&self, x: &[Quantity]) -> Observation {
        Observation {
            outcome: self.0.evaluate(x),
            uncertainty: vec![Quantity::new("f", 0.001, "Pa")],
        }
    }
}

fn r3_texts(r: &FidelityResult) -> Vec<String> {
    let mut vals: Vec<f64> = Vec::new();
    for rd in &r.rounds {
        if let Some(c) = &rd.correction {
            for f in &c.fits {
                vals.extend(&f.coef);
                if let Some(g) = &f.r3 {
                    vals.extend(&g.l);
                    vals.extend(&g.beta);
                    vals.extend([g.sf2, g.mse, g.se, g.coverage, g.loglik]);
                }
            }
        }
    }
    let mut t: Vec<String> = Vec::new();
    for v in vals {
        for s in [format!("{v}"), serde_json::to_string(&v).unwrap()] {
            if s.len() >= 8 && !t.contains(&s) {
                t.push(s);
            }
        }
    }
    t
}

#[test]
fn p6_ncp_privacy_of_r3() {
    let d = tmp("np");
    let low = n1_low_model();
    let hm = n1_high_model();
    let mut spec = nd_spec(1);
    spec.privacy = Some(Privacy::new(&d.join("private").join("ledger.jsonl"), "rig-n1"));
    let pubp = d.join("public").join("ledger.jsonl");
    let r = fidelity::run(&spec, &low, &Private(&hm), &ledger_at(&pubp)).unwrap();
    let texts = r3_texts(&r);
    let bytes = std::fs::read_to_string(&pubp).unwrap();
    let found = texts.iter().filter(|t| bytes.contains(t.as_str())).count();
    eprintln!(
        "P6: {found} of {} texts found in {} public bytes",
        texts.len(),
        bytes.len()
    );
    assert!(texts.len() >= 50, "P6: too few texts");
    assert_eq!(found, 0, "P6");
    // NC-P: the same source public
    let src = ModelSource {
        level: Some("high".into()),
        ..ModelSource::of(&hm)
    };
    let p2 = d.join("control.jsonl");
    let rc = fidelity::run(&nd_spec(1), &low, &src, &ledger_at(&p2)).unwrap();
    let tc = r3_texts(&rc);
    let bc = std::fs::read_to_string(&p2).unwrap();
    let fc = tc.iter().filter(|t| bc.contains(t.as_str())).count();
    eprintln!("NC-P: {fc} of {} texts found without the split", tc.len());
    assert!(fc >= 1, "NC-P");
}

// ---------------------------------------------------------------- ADR 0652 amendment 1: corrected tests

/// One N case: every number computed and printed before any assertion.
fn n_case(tag: &str, pairs: &[PairData], check_l3s: bool) {
    let params = n1_params();
    let c = fit_one(pairs, &params, &r3_trust(), &["f"]);
    let f = &c.fits[0];
    let g = f.r3.as_ref().unwrap();
    assert!(g.available, "{tag}: {}", g.why);
    let want = expected_rung(pairs, &params, 0, f);
    let lc = loo_check(pairs, &params, 0, &g.l, g.sf2);
    let ratio = (g.mse / f.mse[3].unwrap()).sqrt();
    eprintln!(
        "{tag}: rung {} (one-SE recomputed {}) trusted {} ({}); l {:?} sf2 {:.4e}; MSE R2 {:.4e} R3 {:.4e} (SE {:.2e}); RMSE ratio {ratio:.4}; coverage {:.3}; L3 {:.2e} L3s {:.2e} of rms(r) {:.4e}",
        f.rung.as_str(),
        want.as_str(),
        f.trusted,
        f.why,
        g.l,
        g.sf2,
        f.mse[3].unwrap(),
        g.mse,
        g.se,
        g.coverage,
        lc.max_e / lc.rms_r,
        lc.max_sd / lc.rms_r,
        lc.rms_r
    );
    assert_eq!(f.rung, want, "{tag}: S1");
    assert!(lc.max_e <= 1e-8 * lc.rms_r, "{tag}: L3");
    if check_l3s {
        assert!(lc.max_sd <= 1e-8 * lc.rms_r, "{tag}: L3s");
    }
    match tag {
        "N1a" => {
            assert_eq!(f.rung, Rung::R3, "S2");
            assert!(g.coverage >= 0.85, "C1");
            assert!(ratio <= 0.2, "C2");
        }
        "N1b" => {
            assert_eq!(f.rung, Rung::R3, "S2");
            assert!(g.coverage >= 0.85, "C1");
            assert!(ratio <= 0.5, "C2");
        }
        _ => assert_ne!(f.rung, Rung::R3, "S2"),
    }
}

#[test]
fn n1a_corrected() {
    n_case("N1a", &n_pairs(718_011, true, 0.0, 718_013), false);
}

#[test]
fn n1b_corrected() {
    n_case("N1b", &n_pairs(718_012, true, 0.02, 718_013), true);
}

#[test]
fn n2_corrected() {
    n_case("N2", &n_pairs(718_021, false, 0.05, 718_022), true);
}

#[test]
fn n3_corrected() {
    let params = s1_params();
    let pairs: Vec<PairData> = sampling::latin_hypercube(40, 3, 718_031)
        .iter()
        .map(|u| {
            let x: Vec<f64> = u.iter().zip(&params).map(|(v, p)| p.lo + v * (p.hi - p.lo)).collect();
            let (x1, x2, x3) = (x[0], x[1], x[2]);
            let f = (3.0 * x1).sin() + x2 * x2 + 0.1 * x3;
            let g = (0.5 * x1).exp() - x2 * x3 / 20.0;
            PairData {
                low: vec![f, g],
                high: vec![
                    1.7 * f - 0.3 + 0.5 * x1 - 2.0 * x2 + 0.04 * x3,
                    0.9 * g + 1.25 - 0.1 * x1 + 0.3 * x2 - 0.02 * x3,
                ],
                u_high: vec![0.0, 0.0],
                x,
            }
        })
        .collect();
    let on = fit_one(&pairs, &params, &r3_trust(), &["f", "g"]);
    let off = fit_one(&pairs, &params, &TrustRule::default(), &["f", "g"]);
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (m, (a, b)) in on.fits.iter().zip(&off.fits).enumerate() {
        let g = a.r3.as_ref().unwrap();
        let want = expected_rung(&pairs, &params, m, a);
        eprintln!(
            "N3 {}: rung {} (one-SE recomputed {}, off {}); R3 available {} ('{}'); MSE R2 {:.3e}; coef bit-equal {}",
            a.metric,
            a.rung.as_str(),
            want.as_str(),
            b.rung.as_str(),
            g.available,
            g.why,
            a.mse[3].unwrap(),
            bits(&a.coef) == bits(&b.coef)
        );
        rows.push((
            a.rung,
            want,
            b.rung,
            g.available,
            g.why.clone(),
            bits(&a.coef) == bits(&b.coef),
        ));
    }
    for (rung, want, off_rung, avail, why, eq) in rows {
        assert_eq!(rung, want, "S1");
        assert_eq!(rung, Rung::R2, "S2");
        assert_eq!(off_rung, Rung::R2);
        assert!(!avail && why.contains("rounding"), "{why}");
        assert!(eq, "S2: R2 coefficients");
    }
}

#[test]
#[ignore = "ADR 0652 amendment 2: X1 0 of 500 queries flagged (the region covers the box); T6 not reached"]
fn nd_corrected_known_miss() {
    let d = tmp("ndc");
    let low = n1_low_model();
    let hm = n1_high_model();
    let src = ModelSource {
        level: Some("high".into()),
        ..ModelSource::of(&hm)
    };
    let p1 = d.join("w1.jsonl");
    let p8 = d.join("w8.jsonl");
    let r = fidelity::run(&nd_spec(1), &low, &src, &ledger_at(&p1)).unwrap();
    let r8 = fidelity::run(&nd_spec(8), &low, &src, &ledger_at(&p8)).unwrap();
    // D4
    let (b1, b8) = (std::fs::read(&p1).unwrap(), std::fs::read(&p8).unwrap());
    assert_eq!(b1, b8, "D4");
    assert_eq!(r, r8);
    eprintln!("D4: {} bytes, equal at 1 and 8 workers", b1.len());
    // D3
    let c = ledger::parse_text(&String::from_utf8(b1).unwrap());
    assert_eq!(c.skipped.len(), 0);
    let es = &c.entries;
    let count = |k: &str, ev: &str| es.iter().filter(|e| e.kind == k && e.evaluator == ev).count();
    assert_eq!(count("evaluation", "analytic.n1"), 300);
    assert_eq!(count("evaluation", "synthetic.n1"), 36);
    assert_eq!(es.iter().filter(|e| e.kind == "fidelity_pair").count(), 36);
    let sums: Vec<_> = es.iter().filter(|e| e.kind == "fidelity_summary").collect();
    assert_eq!(sums.len(), 6);
    for (k, e) in sums.iter().enumerate() {
        let text = e.settings["correction"].to_string();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(e.fitted.as_ref().unwrap().hash, format!("fnv1a64:{h:016x}"));
        let r3 = &e.settings["correction"]["fits"][0]["r3"];
        assert!(r3.is_object(), "D3: round {} has no r3 object", k + 1);
        if k >= 3 {
            assert_eq!(r3["available"], serde_json::json!(true), "D3: round {}", k + 1);
        }
    }
    // S1 in every round
    let params = n1_params();
    for (k, rd) in r.rounds.iter().enumerate() {
        let pairs = pairs_upto(&r, k + 1);
        let f = &rd.correction.as_ref().unwrap().fits[0];
        let want = expected_rung(&pairs, &params, 0, f);
        let g = f.r3.as_ref().unwrap();
        eprintln!(
            "ND round {}: {} pairs, rung {} trusted {} ({}); R3 available {} mse {:.3e} coverage {:.3}; R2 mse {:?}",
            k + 1,
            pairs.len(),
            f.rung.as_str(),
            f.trusted,
            f.why,
            g.available,
            g.mse,
            g.coverage,
            f.mse[3]
        );
        assert_eq!(f.rung, want, "S1 round {}", k + 1);
    }
    // D1, C1, L3 on the final fit
    let cor = r.correction.as_ref().unwrap();
    let f = &cor.fits[0];
    let g = f.r3.as_ref().unwrap();
    assert_eq!(f.rung, Rung::R3, "D1");
    assert!(f.trusted, "D1: {}", f.why);
    assert!(g.coverage >= 0.85, "C1 {}", g.coverage);
    let pairs = pairs_upto(&r, 6);
    let lc = loo_check(&pairs, &params, 0, &g.l, g.sf2);
    eprintln!(
        "ND final: L3 {:.2e}, L3s {:.2e} of rms(r) {:.3e}; coverage {:.3}",
        lc.max_e / lc.rms_r,
        lc.max_sd / lc.rms_r,
        lc.rms_r,
        g.coverage
    );
    assert!(lc.max_e <= 1e-8 * lc.rms_r, "L3");
    // L3s recorded only (ADR 0652 amendment 1: its miss is the ignored record)
    // D2
    let off = fit_one(&pairs, &params, &TrustRule::default(), &["f"]);
    let promoted: Vec<usize> = r.pairs.iter().map(|q| q.candidate).collect();
    let (mut s3, mut s2, mut nh) = (0.0, 0.0, 0usize);
    for (i, x) in r.candidates.iter().enumerate() {
        if promoted.contains(&i) || r.low[i].is_none() || !cor.region.contains(&unit(&params, x)) {
            continue;
        }
        let lv = r.low[i].as_ref().unwrap();
        let h = n1_high(x[0], x[1]);
        s3 += (h - cor.predict(x, lv).0[0]).powi(2);
        s2 += (h - off.predict(x, lv).0[0]).powi(2);
        nh += 1;
    }
    let ratio = (s3 / s2).sqrt();
    eprintln!(
        "D2: {nh} held-out candidates inside the region; RMSE R3 {:.4e}, R2-only fit {:.4e} (rung {} trusted {}); ratio {ratio:.4}",
        (s3 / nh as f64).sqrt(),
        (s2 / nh as f64).sqrt(),
        off.fits[0].rung.as_str(),
        off.fits[0].trusted
    );
    assert!(nh > 0 && ratio <= 0.5, "D2 {ratio}");
    // X1
    let qs: Vec<Vec<f64>> = sampling::latin_hypercube(500, 2, 718_051)
        .iter()
        .map(|u| vec![u[0], -1.0 + 2.0 * u[1]])
        .collect();
    let mut n_flag = 0;
    let mut worst: f64 = 0.0;
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (v, fl) = cor.predict(x, &yl);
        if fl[0] {
            n_flag += 1;
            let want = if f.constant_trusted { yl[0] + f.constant } else { yl[0] };
            assert_eq!(v[0].to_bits(), want.to_bits(), "X1 flagged");
        } else {
            let uq = unit(&params, x);
            let mu: f64 = cor
                .region
                .points
                .iter()
                .zip(&g.beta)
                .map(|(pt, b)| g.sf2 * se_corr(pt, &uq, &g.l) * b)
                .sum();
            let want = f.coef[0] * yl[0] + f.coef[1] + f.coef[2] * x[0] + f.coef[3] * x[1] + mu;
            let e = (v[0] - want).abs() / want.abs().max(1e-300);
            worst = worst.max(e);
            assert!(e <= 1e-12, "X1 unflagged {e}");
        }
    }
    eprintln!("X1: {n_flag} of 500 flagged; unflagged largest relative difference {worst:.2e}");
    assert!(n_flag >= 1, "X1: no query flagged");
    // T6
    let back = Correction::from_json(&cor.to_json()).unwrap();
    assert_eq!(back.hash(), cor.hash());
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (a, fa) = cor.predict(x, &yl);
        let (b, fb) = back.predict(x, &yl);
        assert_eq!(a[0].to_bits(), b[0].to_bits(), "T6");
        assert_eq!(fa, fb);
    }
    eprintln!("T6: round trip bit-equal at 500 queries, hash {}", cor.hash());
}

#[test]
fn nd_corrected2() {
    let d = tmp("ndc2");
    let low = n1_low_model();
    let hm = n1_high_model();
    let src = ModelSource {
        level: Some("high".into()),
        ..ModelSource::of(&hm)
    };
    let p1 = d.join("w1.jsonl");
    let p8 = d.join("w8.jsonl");
    let r = fidelity::run(&nd_spec(1), &low, &src, &ledger_at(&p1)).unwrap();
    let r8 = fidelity::run(&nd_spec(8), &low, &src, &ledger_at(&p8)).unwrap();
    // D4
    let (b1, b8) = (std::fs::read(&p1).unwrap(), std::fs::read(&p8).unwrap());
    assert_eq!(b1, b8, "D4");
    assert_eq!(r, r8);
    eprintln!("D4: {} bytes, equal at 1 and 8 workers", b1.len());
    // D3
    let c = ledger::parse_text(&String::from_utf8(b1).unwrap());
    assert_eq!(c.skipped.len(), 0);
    let es = &c.entries;
    let count = |k: &str, ev: &str| es.iter().filter(|e| e.kind == k && e.evaluator == ev).count();
    assert_eq!(count("evaluation", "analytic.n1"), 300);
    assert_eq!(count("evaluation", "synthetic.n1"), 36);
    assert_eq!(es.iter().filter(|e| e.kind == "fidelity_pair").count(), 36);
    let sums: Vec<_> = es.iter().filter(|e| e.kind == "fidelity_summary").collect();
    assert_eq!(sums.len(), 6);
    for (k, e) in sums.iter().enumerate() {
        let text = e.settings["correction"].to_string();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(e.fitted.as_ref().unwrap().hash, format!("fnv1a64:{h:016x}"));
        let r3 = &e.settings["correction"]["fits"][0]["r3"];
        assert!(r3.is_object(), "D3: round {} has no r3 object", k + 1);
        if k >= 3 {
            assert_eq!(r3["available"], serde_json::json!(true), "D3: round {}", k + 1);
        }
    }
    // S1 in every round
    let params = n1_params();
    for (k, rd) in r.rounds.iter().enumerate() {
        let pairs = pairs_upto(&r, k + 1);
        let f = &rd.correction.as_ref().unwrap().fits[0];
        let want = expected_rung(&pairs, &params, 0, f);
        let g = f.r3.as_ref().unwrap();
        eprintln!(
            "ND round {}: {} pairs, rung {} trusted {} ({}); R3 available {} mse {:.3e} coverage {:.3}; R2 mse {:?}",
            k + 1,
            pairs.len(),
            f.rung.as_str(),
            f.trusted,
            f.why,
            g.available,
            g.mse,
            g.coverage,
            f.mse[3]
        );
        assert_eq!(f.rung, want, "S1 round {}", k + 1);
    }
    // D1, C1, L3 on the final fit
    let cor = r.correction.as_ref().unwrap();
    let f = &cor.fits[0];
    let g = f.r3.as_ref().unwrap();
    assert_eq!(f.rung, Rung::R3, "D1");
    assert!(f.trusted, "D1: {}", f.why);
    assert!(g.coverage >= 0.85, "C1 {}", g.coverage);
    let pairs = pairs_upto(&r, 6);
    let lc = loo_check(&pairs, &params, 0, &g.l, g.sf2);
    eprintln!(
        "ND final: L3 {:.2e}, L3s {:.2e} of rms(r) {:.3e}; coverage {:.3}",
        lc.max_e / lc.rms_r,
        lc.max_sd / lc.rms_r,
        lc.rms_r,
        g.coverage
    );
    assert!(lc.max_e <= 1e-8 * lc.rms_r, "L3");
    // L3s recorded only (ADR 0652 amendment 1: its miss is the ignored record)
    // D2
    let off = fit_one(&pairs, &params, &TrustRule::default(), &["f"]);
    let promoted: Vec<usize> = r.pairs.iter().map(|q| q.candidate).collect();
    let (mut s3, mut s2, mut nh) = (0.0, 0.0, 0usize);
    for (i, x) in r.candidates.iter().enumerate() {
        if promoted.contains(&i) || r.low[i].is_none() || !cor.region.contains(&unit(&params, x)) {
            continue;
        }
        let lv = r.low[i].as_ref().unwrap();
        let h = n1_high(x[0], x[1]);
        s3 += (h - cor.predict(x, lv).0[0]).powi(2);
        s2 += (h - off.predict(x, lv).0[0]).powi(2);
        nh += 1;
    }
    let ratio = (s3 / s2).sqrt();
    eprintln!(
        "D2: {nh} held-out candidates inside the region; RMSE R3 {:.4e}, R2-only fit {:.4e} (rung {} trusted {}); ratio {ratio:.4}",
        (s3 / nh as f64).sqrt(),
        (s2 / nh as f64).sqrt(),
        off.fits[0].rung.as_str(),
        off.fits[0].trusted
    );
    assert!(nh > 0 && ratio <= 0.5, "D2 {ratio}");
    // X1
    let qs: Vec<Vec<f64>> = sampling::latin_hypercube(500, 2, 718_051)
        .iter()
        .map(|u| vec![u[0], -1.0 + 2.0 * u[1]])
        .collect();
    let qs: Vec<Vec<f64>> = qs
        .into_iter()
        .chain(
            sampling::latin_hypercube(100, 2, 718_052)
                .iter()
                .map(|u| vec![1.1 + 0.4 * u[0], -1.0 + 2.0 * u[1]]),
        )
        .collect();
    let mut n_flag = 0;
    let mut worst: f64 = 0.0;
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (v, fl) = cor.predict(x, &yl);
        if fl[0] {
            n_flag += 1;
            let want = if f.constant_trusted { yl[0] + f.constant } else { yl[0] };
            assert_eq!(v[0].to_bits(), want.to_bits(), "X1 flagged");
        } else {
            let uq = unit(&params, x);
            let mu: f64 = cor
                .region
                .points
                .iter()
                .zip(&g.beta)
                .map(|(pt, b)| g.sf2 * se_corr(pt, &uq, &g.l) * b)
                .sum();
            let want = f.coef[0] * yl[0] + f.coef[1] + f.coef[2] * x[0] + f.coef[3] * x[1] + mu;
            let e = (v[0] - want).abs() / want.abs().max(1e-300);
            worst = worst.max(e);
            assert!(e <= 1e-12, "X1 unflagged {e}");
        }
    }
    eprintln!("X1: {n_flag} of 600 flagged; unflagged largest relative difference {worst:.2e}");
    assert!(n_flag >= 1, "X1: no query flagged");
    // T6
    let back = Correction::from_json(&cor.to_json()).unwrap();
    assert_eq!(back.hash(), cor.hash());
    for x in &qs {
        let yl = vec![n1_low(x[0], x[1])];
        let (a, fa) = cor.predict(x, &yl);
        let (b, fb) = back.predict(x, &yl);
        assert_eq!(a[0].to_bits(), b[0].to_bits(), "T6");
        assert_eq!(fa, fb);
    }
    eprintln!("T6: round trip bit-equal at 600 queries, hash {}", cor.hash());
}
