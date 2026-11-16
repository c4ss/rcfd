//! ADR 0656: R3's leave-one-out standard deviation (F-734) against a double-double brute force. Synthetic sources only.

use rcfd_api::design::fidelity::{
    self, r3_loo, Candidates, CorrectedMetric, Correction, FidelitySpec, ModelSource, PairData, ParamBox, R3Rule,
    TrustRule,
};
use rcfd_api::design::ledger::{Fidelity, FnModel, Ledger, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::study::Sense;
use std::path::PathBuf;

type EvalFn = Box<dyn Fn(&[Quantity]) -> Outcome + Sync + Send>;
type Fm = FnModel<EvalFn>;

// ---------------------------------------------------------------- double-double (Dekker 1971; ADR 0656)

#[derive(Clone, Copy, Debug)]
struct Dd {
    hi: f64,
    lo: f64,
}

fn dd(v: f64) -> Dd {
    Dd { hi: v, lo: 0.0 }
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

fn quick(a: f64, b: f64) -> Dd {
    let s = a + b;
    Dd { hi: s, lo: b - (s - a) }
}

fn add(x: Dd, y: Dd) -> Dd {
    let (s, e) = two_sum(x.hi, y.hi);
    let (t, f) = two_sum(x.lo, y.lo);
    let r = quick(s, e + t);
    quick(r.hi, r.lo + f)
}

fn neg(x: Dd) -> Dd {
    Dd { hi: -x.hi, lo: -x.lo }
}

fn sub(x: Dd, y: Dd) -> Dd {
    add(x, neg(y))
}

fn mul(x: Dd, y: Dd) -> Dd {
    let p = x.hi * y.hi;
    let e = x.hi.mul_add(y.hi, -p);
    quick(p, e + (x.hi * y.lo + x.lo * y.hi))
}

fn div(x: Dd, y: Dd) -> Dd {
    let q1 = x.hi / y.hi;
    let r = sub(x, mul(y, dd(q1)));
    let q2 = r.hi / y.hi;
    let r = sub(r, mul(y, dd(q2)));
    let q3 = r.hi / y.hi;
    add(quick(q1, q2), dd(q3))
}

/// Gauss-Jordan with partial pivoting in double-double, several right-hand sides (columns of `b`).
fn solve_dd(mut a: Vec<Vec<Dd>>, mut b: Vec<Vec<Dd>>) -> Vec<Vec<Dd>> {
    let n = a.len();
    for c in 0..n {
        let piv = (c..n)
            .max_by(|&i, &j| a[i][c].hi.abs().total_cmp(&a[j][c].hi.abs()))
            .unwrap();
        a.swap(c, piv);
        b.swap(c, piv);
        for r in 0..n {
            if r != c {
                let f = div(a[r][c], a[c][c]);
                if f.hi != 0.0 {
                    for j in c..n {
                        a[r][j] = sub(a[r][j], mul(f, a[c][j]));
                    }
                    for j in 0..b[r].len() {
                        b[r][j] = sub(b[r][j], mul(f, b[c][j]));
                    }
                }
            }
        }
    }
    (0..n)
        .map(|i| b[i].iter().map(|v| div(*v, a[i][i])).collect())
        .collect()
}

// ---------------------------------------------------------------- set-ups of ADR 0652

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

fn normals(seed: u64, n: usize) -> Vec<f64> {
    let mut g = Rng::new(seed);
    (0..n)
        .map(|_| {
            let (u1, u2) = (g.uniform(), g.uniform());
            (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        })
        .collect()
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

fn r3_trust() -> TrustRule {
    TrustRule {
        r3: Some(R3Rule::default()),
        ..TrustRule::default()
    }
}

fn p(x: &[Quantity], n: &str) -> f64 {
    x.iter().find(|q| q.name == n).unwrap().value
}

fn model(id: &str, high: bool) -> Fm {
    FnModel {
        id: id.into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: Box::new(move |x| {
            let (a, b) = (p(x, "x1"), p(x, "x2"));
            let v = if high { n1_high(a, b) } else { n1_low(a, b) };
            Outcome::ok(vec![Quantity::new("f", v, "Pa")])
        }),
    }
}

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

// ---------------------------------------------------------------- the check of one fit

struct SdCheck {
    v: f64,
    b0: f64,
    b1: f64,
    ncs: f64,
    rms_r: f64,
    coverage: f64,
    min_sd: f64,
}

fn sd_check(pairs: &[PairData], params: &[ParamBox], l: &[f64], sf2: f64) -> SdCheck {
    let got = r3_loo(pairs, params, 0, &R3Rule::default(), Some((l, sf2))).unwrap();
    let n = pairs.len();
    let u: Vec<Vec<f64>> = pairs.iter().map(|q| unit(params, &q.x)).collect();
    let y: Vec<f64> = pairs.iter().map(|q| q.high[0]).collect();
    let sig: Vec<f64> = pairs.iter().map(|q| q.u_high[0]).collect();
    let xr: Vec<Vec<f64>> = (0..n)
        .map(|k| [vec![pairs[k].low[0], 1.0], u[k].clone()].concat())
        .collect();
    let pp = xr[0].len();
    let c: Vec<Vec<f64>> = (0..n)
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
    let rms_r = (got.r.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
    let (mut v, mut b0, mut b1, mut ncs) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut inside = 0;
    for i in 0..n {
        let rest: Vec<usize> = (0..n).filter(|&k| k != i).collect();
        // R2 refit on the others, applied to every unit vector e_k: b_k = N^-1 x_k (k != i), 0 for k = i
        let mut nm = vec![vec![dd(0.0); pp]; pp];
        for &k in &rest {
            for a in 0..pp {
                for b in 0..pp {
                    nm[a][b] = add(nm[a][b], mul(dd(xr[k][a]), dd(xr[k][b])));
                }
            }
        }
        let rhs: Vec<Vec<Dd>> = (0..pp)
            .map(|a| (0..n).map(|k| dd(if k == i { 0.0 } else { xr[k][a] })).collect())
            .collect();
        let bm = solve_dd(nm, rhs); // pp x n
        let fit = |j: usize, k: usize| -> Dd {
            let mut s = dd(0.0);
            for a in 0..pp {
                s = add(s, mul(dd(xr[j][a]), bm[a][k]));
            }
            s
        };
        // residuals of the others for each unit vector, then the zero-mean process on them
        let rr: Vec<Vec<Dd>> = rest
            .iter()
            .map(|&j| {
                (0..n)
                    .map(|k| sub(dd(if j == k { 1.0 } else { 0.0 }), fit(j, k)))
                    .collect()
            })
            .collect();
        let cm: Vec<Vec<Dd>> = rest
            .iter()
            .map(|&a| rest.iter().map(|&b| dd(c[a][b])).collect())
            .collect();
        let w = solve_dd(cm, rr); // (n-1) x n
        let a: Vec<Dd> = (0..n)
            .map(|k| {
                let mut mu = dd(0.0);
                for (r, &j) in rest.iter().enumerate() {
                    mu = add(mu, mul(dd(c[i][j]), w[r][k]));
                }
                sub(sub(dd(if k == i { 1.0 } else { 0.0 }), fit(i, k)), mu)
            })
            .collect();
        // variance a^T C a and e = a . y, in double-double
        let mut var = dd(0.0);
        let mut e = dd(0.0);
        for k in 0..n {
            let mut ca = dd(0.0);
            for j in 0..n {
                ca = add(ca, mul(dd(c[k][j]), a[j]));
            }
            var = add(var, mul(a[k], ca));
            e = add(e, mul(a[k], dd(y[k])));
        }
        let sd_dd = var.hi.max(0.0).sqrt();
        v = v.max((got.sd[i] - sd_dd).abs());
        b0 = b0.max((got.e[i] - e.hi).abs());
        // ADR 0652's double brute force (and NC-S): a rounded to double, a^T C a in double
        let af: Vec<f64> = a.iter().map(|t| t.hi).collect();
        let var_f: f64 = (0..n)
            .map(|k| af[k] * (0..n).map(|j| c[k][j] * af[j]).sum::<f64>())
            .sum();
        let dsd = (var_f.max(0.0).sqrt() - sd_dd).abs();
        b1 = b1.max(dsd);
        ncs = ncs.max(dsd);
        if got.e[i].abs() <= 2.0 * got.sd[i] {
            inside += 1;
        }
    }
    let min_sd = got.sd.iter().fold(f64::INFINITY, |m, s| m.min(*s));
    SdCheck {
        v,
        b0,
        b1,
        ncs,
        rms_r,
        coverage: inside as f64 / n as f64,
        min_sd,
    }
}

fn report(tag: &str, k: &SdCheck) {
    eprintln!(
        "{tag}: V {:.3e}  B0 {:.3e}  double a^T C a (B1 / NC-S) {:.3e}  of rms(r) {:.4e}; smallest sd {:.3e} of rms(r); coverage {:.3}",
        k.v / k.rms_r,
        k.b0 / k.rms_r,
        k.b1 / k.rms_r,
        k.rms_r,
        k.min_sd / k.rms_r,
        k.coverage
    );
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f734_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// V1, V2, V3, B0, B1 and NC-S of ADR 0656: every number printed before any assertion.
#[test]
fn r3_sd_against_double_double_brute_force() {
    let t0 = std::time::Instant::now();
    let params = n1_params();
    let mut checks: Vec<(&str, SdCheck)> = Vec::new();
    for (tag, pairs) in [
        ("N1a", n_pairs(718_011, true, 0.0, 718_013)),
        ("N1b", n_pairs(718_012, true, 0.02, 718_013)),
        ("N2", n_pairs(718_021, false, 0.05, 718_022)),
    ] {
        let names = vec!["f".to_string()];
        let c = Correction::fit(&pairs, &params, &names, &r3_trust(), ("low", "1"), ("high", "1"));
        let g = c.fits[0].r3.as_ref().unwrap();
        assert!(g.available, "{tag}: {}", g.why);
        let k = sd_check(&pairs, &params, &g.l, g.sf2);
        eprintln!("{tag}: fit coverage {:.3}", g.coverage);
        report(tag, &k);
        checks.push((tag, k));
    }
    // ND final: the driver of ADR 0652 (workers 1)
    let low = model("analytic.n1", false);
    let hm = model("synthetic.n1", true);
    let src = ModelSource {
        level: Some("high".into()),
        ..ModelSource::of(&hm)
    };
    let mut spec = FidelitySpec::new(
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
    spec.rounds = 6;
    spec.trust = r3_trust();
    spec.workers = 1;
    let d = tmp("nd");
    let mut led = Ledger::new(&d.join("w1.jsonl"));
    led.clock = || 1_700_000_000_000;
    let r = fidelity::run(&spec, &low, &src, &led).unwrap();
    let pairs: Vec<PairData> = r
        .pairs
        .iter()
        .map(|q| PairData {
            x: r.candidates[q.candidate].clone(),
            low: q.low.clone(),
            high: q.high.clone(),
            u_high: vec![0.0],
        })
        .collect();
    let g = r.correction.as_ref().unwrap().fits[0].r3.clone().unwrap();
    let k = sd_check(&pairs, &params, &g.l, g.sf2);
    eprintln!("ND final: {} pairs, fit coverage {:.3}", pairs.len(), g.coverage);
    report("ND", &k);
    checks.push(("ND", k));
    eprintln!("T: {:.1} s", t0.elapsed().as_secs_f64());
    for (tag, k) in &checks {
        assert!(k.v <= 1e-8 * k.rms_r, "{tag}: V (sd against brute force)");
        assert!(k.b0 <= 1e-8 * k.rms_r, "{tag}: B0");
        if *tag == "N1b" || *tag == "N2" {
            assert!(k.b1 <= 1e-12 * k.rms_r, "{tag}: B1");
        }
    }
    let ncs = &checks[0].1;
    eprintln!(
        "NC-S (N1a, diagnostic): double a^T C a against double-double {:.3e} of rms(r), expected above 1e-8: {}",
        ncs.ncs / ncs.rms_r,
        ncs.ncs > 1e-8 * ncs.rms_r
    );
}
