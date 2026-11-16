//! ADR 0663: checks of F-698's zero-mean, absolute-noise mode of the Gaussian process (F-735) and of rung R3 on it.
//! Synthetic data only.

use rcfd_api::design::fidelity::{r3_log_likelihood, r3_loo, Correction, PairData, ParamBox, R3Rule, TrustRule};
use rcfd_api::design::optim::{projected_gradient, PgOptions, Stop};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::surrogate::{
    self, cholesky, solve_lower, solve_upper_t, Gp, GpOptions, Kernel, Noise, ZeroMeanOptions, JITTER,
};

// ---------------------------------------------------------------- set-ups of ADR 0652 (copied from f718_fidelity_r3.rs)

fn normals(seed: u64, n: usize) -> Vec<f64> {
    let mut g = Rng::new(seed);
    (0..n)
        .map(|_| {
            let (u1, u2) = (g.uniform(), g.uniform());
            (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        })
        .collect()
}

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

/// `hetero`: the stated (and drawn) noise `noise (0.5 + 1.5 x1)` per pair (set-up H1).
fn n_pairs(lhs_seed: u64, structured: bool, noise: f64, noise_seed: u64, hetero: bool) -> Vec<PairData> {
    let pts = sampling::latin_hypercube(40, 2, lhs_seed);
    let z = normals(noise_seed, pts.len());
    pts.iter()
        .zip(&z)
        .map(|(u, z)| {
            let (x1, x2) = (u[0], -1.0 + 2.0 * u[1]);
            let h = if structured { n1_high(x1, x2) } else { n1_affine(x1, x2) };
            let s = if hetero { noise * (0.5 + 1.5 * x1) } else { noise };
            PairData {
                x: vec![x1, x2],
                low: vec![n1_low(x1, x2)],
                high: vec![h + s * z],
                u_high: vec![s],
            }
        })
        .collect()
}

fn r3_cases() -> Vec<(&'static str, Vec<PairData>)> {
    vec![
        ("N1a", n_pairs(718_011, true, 0.0, 718_013, false)),
        ("N1b", n_pairs(718_012, true, 0.02, 718_013, false)),
        ("N2", n_pairs(718_021, false, 0.05, 718_022, false)),
        ("H1", n_pairs(735_001, true, 0.03, 735_002, true)),
    ]
}

fn unit(params: &[ParamBox], x: &[f64]) -> Vec<f64> {
    params.iter().zip(x).map(|(p, v)| (v - p.lo) / (p.hi - p.lo)).collect()
}

fn fnv(v: &[f64]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn rel(a: f64, b: f64) -> f64 {
    if a.to_bits() == b.to_bits() {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs()).max(f64::MIN_POSITIVE)
    }
}

// ---------------------------------------------------------------- frozen copy of main 9f06e866's R3 process (fidelity.rs)

fn frozen_r3_cov(u: &[Vec<f64>], l: &[f64], tau: f64, sig2: &[f64]) -> Vec<f64> {
    let n = u.len();
    let mut c = vec![0.0; n * n];
    for i in 0..n {
        c[i * n + i] = tau * (1.0 + surrogate::JITTER) + sig2[i];
        for j in 0..i {
            let v = tau * surrogate::correlation(Kernel::SquaredExponential, &u[i], &u[j], l);
            c[i * n + j] = v;
            c[j * n + i] = v;
        }
    }
    c
}

fn frozen_chol_inverse(chol: &[f64], n: usize) -> Vec<f64> {
    let mut g = vec![0.0; n * n];
    for k in 0..n {
        let mut e = vec![0.0; n];
        e[k] = 1.0;
        let col = surrogate::solve_upper_t(chol, n, &surrogate::solve_lower(chol, n, &e));
        for (i, v) in col.into_iter().enumerate() {
            g[i * n + k] = v;
        }
    }
    g
}

fn frozen_r3_lik(u: &[Vec<f64>], r: &[f64], sig2: &[f64], theta: &[f64], grad: bool) -> Option<(f64, Vec<f64>)> {
    let n = u.len();
    let d = theta.len() - 1;
    let l: Vec<f64> = theta[..d].iter().map(|v| v.exp()).collect();
    let tau = theta[d].exp();
    let c = frozen_r3_cov(u, &l, tau, sig2);
    let chol = surrogate::cholesky(&c, n).ok()?;
    let alpha = surrogate::solve_upper_t(&chol, n, &surrogate::solve_lower(&chol, n, r));
    let value = -0.5 * r.iter().zip(&alpha).map(|(a, b)| a * b).sum::<f64>()
        - (0..n).map(|i| chol[i * n + i].ln()).sum::<f64>();
    if !value.is_finite() {
        return None;
    }
    if !grad {
        return Some((value, Vec::new()));
    }
    let g = frozen_chol_inverse(&chol, n);
    let mut out = vec![0.0; d + 1];
    let mut quad_t = 0.0;
    let mut tr_t = 0.0;
    for i in 0..n {
        let dii = tau * (1.0 + surrogate::JITTER);
        quad_t += alpha[i] * alpha[i] * dii;
        tr_t += g[i * n + i] * dii;
        for j in 0..i {
            let dij = tau * surrogate::correlation(Kernel::SquaredExponential, &u[i], &u[j], &l);
            quad_t += 2.0 * alpha[i] * alpha[j] * dij;
            tr_t += 2.0 * g[i * n + j] * dij;
            for k in 0..d {
                let t = dij * ((u[i][k] - u[j][k]) / l[k]).powi(2);
                out[k] += 2.0 * (alpha[i] * alpha[j] - g[i * n + j]) * t;
            }
        }
    }
    out[d] = quad_t - tr_t;
    for v in &mut out {
        *v *= 0.5;
    }
    Some((value, out))
}

fn frozen_r3_search(u: &[Vec<f64>], r: &[f64], sig2: &[f64], rule: &R3Rule) -> Option<Vec<f64>> {
    let d = u.first().map_or(0, Vec::len);
    let mut bx: Vec<(f64, f64)> = vec![(rule.l_bounds.0.ln(), rule.l_bounds.1.ln()); d];
    bx.push((rule.tau_bounds.0.ln(), rule.tau_bounds.1.ln()));
    let value = |t: &[f64]| frozen_r3_lik(u, r, sig2, t, false).map_or(f64::NEG_INFINITY, |v| v.0);
    let mut starts: Vec<Vec<f64>> = vec![bx.iter().map(|(a, b)| 0.5 * (a + b)).collect()];
    for p in sampling::latin_hypercube(rule.starts - 1, d + 1, rule.seed) {
        starts.push(p.iter().zip(&bx).map(|(v, (a, b))| a + (b - a) * v).collect());
    }
    let vals: Vec<f64> = starts.iter().map(|t| value(t)).collect();
    let mut order: Vec<usize> = (0..starts.len()).collect();
    order.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]).then(a.cmp(&b)));
    let mut best: Option<(Vec<f64>, f64)> = None;
    for &s in order.iter().take(rule.refine) {
        let opts = PgOptions {
            max_iters: rule.iters,
            step: 0.1,
            armijo: 1e-4,
            max_halvings: 30,
            x0: starts[s].clone(),
        };
        let mut ev = |t: &[f64], grad: bool| -> Result<(f64, Vec<f64>), Stop> {
            match frozen_r3_lik(u, r, sig2, t, grad) {
                None => Ok((f64::INFINITY, vec![0.0; d + 1])),
                Some((v, g)) => Ok((-v, g.iter().map(|x| -x).collect())),
            }
        };
        let (t, v) = match projected_gradient(&bx, &opts, &mut ev) {
            Ok(res) if res.fitness.f.is_finite() => (res.x, -res.fitness.f),
            _ => (starts[s].clone(), vals[s]),
        };
        if best.as_ref().is_none_or(|b| v > b.1) {
            best = Some((t, v));
        }
    }
    best.filter(|b| b.1.is_finite()).map(|b| b.0)
}

/// R3's scaled data of metric 0: unit-box inputs, `r / s`, `(sigma / s)^2` (as `r3_core`), and `s`.
type Scaled = (Vec<Vec<f64>>, Vec<f64>, Vec<f64>, Vec<f64>, f64);

fn scaled(pairs: &[PairData], params: &[ParamBox]) -> Scaled {
    let o = r3_loo(pairs, params, 0, &R3Rule::default(), None).unwrap();
    let u: Vec<Vec<f64>> = pairs.iter().map(|q| unit(params, &q.x)).collect();
    let rs: Vec<f64> = o.r.iter().map(|v| v / o.s).collect();
    let sig: Vec<f64> = pairs.iter().map(|q| q.u_high[0] / o.s).collect();
    let sig2: Vec<f64> = pairs
        .iter()
        .map(|q| (q.u_high[0] / o.s) * (q.u_high[0] / o.s))
        .collect();
    (u, rs, sig, sig2, o.s)
}

fn r3_opts() -> ZeroMeanOptions {
    let r = R3Rule::default();
    ZeroMeanOptions {
        kernel: Kernel::SquaredExponential,
        starts: r.starts,
        refine: r.refine,
        iters: r.iters,
        l_bounds: r.l_bounds,
        sf2_bounds: r.tau_bounds,
        seed: r.seed,
        workers: 1,
    }
}

// ---------------------------------------------------------------- Z1: the mode against the frozen R3 process

#[test]
fn z1_mode_equals_frozen_r3_process() {
    let params = n1_params();
    let mut worst = 0.0f64;
    let mut unequal = 0usize;
    let mut compared = 0usize;
    let mut cmp = |tag: &str, what: &str, a: f64, b: f64| {
        let e = rel(a, b);
        compared += 1;
        if e != 0.0 {
            unequal += 1;
            eprintln!("Z1 {tag} {what}: {a:e} vs frozen {b:e}, rel {e:.2e}");
        }
        worst = worst.max(e);
    };
    for (tag, pairs) in r3_cases() {
        let (u, rs, sig, sig2, s) = scaled(&pairs, &params);
        let d = u[0].len();
        let th = frozen_r3_search(&u, &rs, &sig2, &R3Rule::default()).unwrap();
        let gp = Gp::fit_zero_mean(&u, &rs, &sig2, &r3_opts()).unwrap();
        for k in 0..d {
            cmp(tag, "l", gp.length_scales()[k], th[k].exp());
        }
        cmp(tag, "tau", gp.process_variance(), th[d].exp());
        let (fv, _) = frozen_r3_lik(&u, &rs, &sig2, &th, false).unwrap();
        cmp(tag, "loglik", gp.log_likelihood(), fv);
        // the factor and alpha at the fitted point
        let tau = th[d].exp();
        let lf: Vec<f64> = th[..d].iter().map(|v| v.exp()).collect();
        let n = u.len();
        let chol = cholesky(&frozen_r3_cov(&u, &lf, tau, &sig2), n).unwrap();
        let alpha = solve_upper_t(&chol, n, &solve_lower(&chol, n, &rs));
        for (a, b) in gp.cholesky_factor().iter().zip(&chol) {
            cmp(tag, "chol", *a, *b);
        }
        for (a, b) in gp.weights().iter().zip(&alpha) {
            cmp(tag, "alpha", *a, *b);
        }
        // R3 itself: r3_loo's l and sf2
        let o = r3_loo(&pairs, &params, 0, &R3Rule::default(), None).unwrap();
        for k in 0..d {
            cmp(tag, "r3_loo l", o.l[k], th[k].exp());
        }
        cmp(tag, "r3_loo sf2", o.sf2, tau * s * s);
        cmp(tag, "r3_loo loglik", o.loglik, fv);
        for t in [
            th.clone(),
            vec![0.3f64.ln(), 0.3f64.ln(), 0.0],
            vec![2f64.ln(), 0.1f64.ln(), 0.01f64.ln()],
        ] {
            let (fv, fg) = frozen_r3_lik(&u, &rs, &sig2, &t, true).unwrap();
            let (nv, ng) = r3_log_likelihood(&u, &rs, &sig, &t).unwrap();
            cmp(tag, "lik", nv, fv);
            for (a, b) in ng.iter().zip(&fg) {
                cmp(tag, "grad", *a, *b);
            }
            let g2 = Gp::zero_mean_with_hyper(
                &u,
                &rs,
                &sig2,
                Kernel::SquaredExponential,
                &t[..d].iter().map(|v| v.exp()).collect::<Vec<_>>(),
                t[d].exp(),
            )
            .unwrap();
            let (gv, gg) = g2.log_likelihood_and_gradient(true);
            cmp(tag, "gp lik", gv, fv);
            for (a, b) in gg.iter().zip(&fg) {
                cmp(tag, "gp grad", *a, *b);
            }
        }
        eprintln!("Z1 {tag}: theta {th:?}");
    }
    eprintln!("Z1: {compared} numbers compared, {unequal} not bit-equal, largest relative difference {worst:.2e}");
    assert!(worst <= 1e-12, "Z1");
}

// ---------------------------------------------------------------- Z2: R3 fits against the probe on main 9f06e866

#[test]
fn z2_r3_fits_equal_main() {
    let params = n1_params();
    let want = [
        ("N1a", "fnv1a64:45da7c9c7db509b3", "2d53dcb4b5b1dfc1"),
        ("N1b", "fnv1a64:9d93b31564bf8555", "9b3ca685c94031f2"),
        ("N2", "fnv1a64:83d3c555da6b8adc", "be3435a70f99e7fb"),
        ("H1", "fnv1a64:528df5f492569d7d", "f64c44a1f528b3da"),
    ];
    let trust = TrustRule {
        r3: Some(R3Rule::default()),
        ..TrustRule::default()
    };
    let mut ok = true;
    for ((tag, pairs), (wtag, wh, wd)) in r3_cases().into_iter().zip(want) {
        assert_eq!(tag, wtag);
        let c = Correction::fit(&pairs, &params, &["f".to_string()], &trust, ("low", "1"), ("high", "1"));
        let o = r3_loo(&pairs, &params, 0, &R3Rule::default(), None).unwrap();
        let mut all = o.l.clone();
        all.push(o.sf2);
        all.push(o.loglik);
        all.extend(&o.beta);
        all.extend(&o.e);
        all.extend(&o.sd);
        all.extend(&o.in_sample);
        let (h, dg) = (c.hash(), fnv(&all));
        eprintln!(
            "Z2 {tag}: hash {h} (main {wh}) digest {dg} (main {wd}) rung {}",
            c.fits[0].rung.as_str()
        );
        ok &= h == wh && dg == wd;
    }
    assert!(ok, "Z2");
}

// ---------------------------------------------------------------- NC-K: the ordinary-kriging mode is a different model

#[test]
fn nc_k_ordinary_kriging_differs() {
    let params = n1_params();
    let pairs = n_pairs(718_012, true, 0.02, 718_013, false);
    let (u, rs, _, sig2, _) = scaled(&pairs, &params);
    let th = frozen_r3_search(&u, &rs, &sig2, &R3Rule::default()).unwrap();
    let ms = rs.iter().map(|v| v * v).sum::<f64>() / rs.len() as f64;
    let g = sig2.iter().sum::<f64>() / sig2.len() as f64 / ms;
    let ok = Gp::fit(
        &u,
        &rs,
        &GpOptions {
            kernel: Kernel::SquaredExponential,
            noise: Noise::Nugget(g),
            ..GpOptions::default()
        },
    )
    .unwrap();
    let e = (0..2)
        .map(|k| rel(ok.length_scales()[k], th[k].exp()))
        .fold(0.0f64, f64::max);
    eprintln!(
        "NC-K: ordinary kriging l {:?} vs R3 {:?}: largest relative difference {e:.2e} (expected > 1e-12)",
        ok.length_scales(),
        [th[0].exp(), th[1].exp()]
    );
}

// ---------------------------------------------------------------- Z4: the mode on its own (set-up M)

struct M {
    u: Vec<Vec<f64>>,
    y: Vec<f64>,
    v: Vec<f64>,
    q: Vec<Vec<f64>>,
}

fn m_setup() -> M {
    let u = sampling::latin_hypercube(20, 2, 735_201);
    let y = u.iter().map(|p| (4.0 * p[0]).sin() * (2.0 * p[1]).cos()).collect();
    let v = (0..20).map(|i| 1e-4 * (1 + i % 3) as f64).collect();
    M {
        u,
        y,
        v,
        q: sampling::latin_hypercube(5, 2, 735_202),
    }
}

const M_L: [f64; 2] = [0.4, 0.6];
const M_SF2: f64 = 0.5;

/// The test's own correlation (ADR 0621's formulas).
fn kcorr(kernel: Kernel, a: &[f64], b: &[f64], l: &[f64]) -> f64 {
    let q: f64 = a.iter().zip(b).zip(l).map(|((x, y), s)| ((x - y) / s).powi(2)).sum();
    match kernel {
        Kernel::SquaredExponential => (-0.5 * q).exp(),
        Kernel::Matern52 => {
            let r = (5.0 * q).sqrt();
            (1.0 + r + r * r / 3.0) * (-r).exp()
        }
    }
}

fn cov(kernel: Kernel, u: &[Vec<f64>], v: &[f64], l: &[f64], sf2: f64) -> Vec<Vec<f64>> {
    let n = u.len();
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        sf2 * (1.0 + JITTER) + v[i]
                    } else {
                        sf2 * kcorr(kernel, &u[i], &u[j], l)
                    }
                })
                .collect()
        })
        .collect()
}

/// Gauss-Jordan with partial pivoting: the solution of `a x = b` and `ln |det a|`.
fn gj(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> (Vec<f64>, f64) {
    let n = b.len();
    let mut logdet = 0.0;
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        b.swap(c, p);
        let piv = a[c][c];
        logdet += piv.abs().ln();
        for k in c..n {
            a[c][k] /= piv;
        }
        b[c] /= piv;
        for i in 0..n {
            if i != c {
                let f = a[i][c];
                if f != 0.0 {
                    for k in c..n {
                        a[i][k] -= f * a[c][k];
                    }
                    b[i] -= f * b[c];
                }
            }
        }
    }
    (b, logdet)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[test]
fn z4a_gradient_against_central_differences() {
    let m = m_setup();
    let mut worst = 0.0f64;
    for kernel in [Kernel::SquaredExponential, Kernel::Matern52] {
        for th in [
            vec![M_L[0].ln(), M_L[1].ln(), M_SF2.ln()],
            vec![0.2f64.ln(), 1.5f64.ln(), 2f64.ln()],
        ] {
            let at = |t: &[f64]| {
                Gp::zero_mean_with_hyper(&m.u, &m.y, &m.v, kernel, &[t[0].exp(), t[1].exp()], t[2].exp()).unwrap()
            };
            let (_, g) = at(&th).log_likelihood_and_gradient(false);
            let gmax = g.iter().fold(0.0f64, |a, b| a.max(b.abs()));
            for k in 0..3 {
                let h = 1e-5;
                let mut a = th.clone();
                let mut b = th.clone();
                a[k] += h;
                b[k] -= h;
                let fd = (at(&a).log_likelihood() - at(&b).log_likelihood()) / (2.0 * h);
                let err = (g[k] - fd).abs();
                eprintln!(
                    "Z4a {kernel:?} theta {th:?} k {k}: analytic {:.8e} fd {fd:.8e} rel {:.2e}",
                    g[k],
                    err / fd.abs()
                );
                if fd.abs() < 1e-3 * gmax {
                    assert!(err <= 1e-7, "Z4a small component");
                } else {
                    worst = worst.max(err / fd.abs());
                    assert!(err <= 1e-5 * fd.abs(), "Z4a");
                }
            }
        }
    }
    eprintln!("Z4a: largest relative error {worst:.2e}");
}

#[test]
fn z4bcd_likelihood_predict_loo_against_brute_force() {
    let m = m_setup();
    let n = m.u.len();
    let sd0 = M_SF2.sqrt();
    let (mut eb, mut ec, mut ed) = (0.0f64, 0.0f64, 0.0f64);
    for kernel in [Kernel::SquaredExponential, Kernel::Matern52] {
        let gp = Gp::zero_mean_with_hyper(&m.u, &m.y, &m.v, kernel, &M_L, M_SF2).unwrap();
        assert!(gp.is_zero_mean());
        let c = cov(kernel, &m.u, &m.v, &M_L, M_SF2);
        let (w, logdet) = gj(c.clone(), m.y.clone());
        let lb = -0.5 * dot(&m.y, &w) - 0.5 * logdet;
        let e = (gp.log_likelihood() - lb).abs() / lb.abs();
        eprintln!(
            "Z4b {kernel:?}: loglik {:.15e} brute {lb:.15e} rel {e:.2e}",
            gp.log_likelihood()
        );
        eb = eb.max(e);
        for q in &m.q {
            let k: Vec<f64> = m.u.iter().map(|p| M_SF2 * kcorr(kernel, p, q, &M_L)).collect();
            let (w, _) = gj(c.clone(), k.clone());
            let mb = dot(&w, &m.y);
            let sb = (M_SF2 - dot(&k, &w)).max(0.0).sqrt();
            let (mm, ss) = gp.predict(q);
            let e = (mm - mb).abs().max((ss - sb).abs()) / sd0;
            eprintln!("Z4c {kernel:?} {q:?}: mean {mm:.12e} ({mb:.12e}) sd {ss:.12e} ({sb:.12e}) err {e:.2e}");
            ec = ec.max(e);
        }
        let loo = gp.loo();
        for i in 0..n {
            let idx: Vec<usize> = (0..n).filter(|&j| j != i).collect();
            let a: Vec<Vec<f64>> = idx.iter().map(|&r| idx.iter().map(|&s| c[r][s]).collect()).collect();
            let k: Vec<f64> = idx.iter().map(|&r| c[r][i]).collect();
            let ym: Vec<f64> = idx.iter().map(|&r| m.y[r]).collect();
            let (w, _) = gj(a, k.clone());
            let mb = dot(&w, &ym);
            let sb = (c[i][i] - dot(&k, &w)).sqrt();
            let e = (loo[i].0 - mb).abs().max((loo[i].1 - sb).abs()) / sd0;
            ed = ed.max(e);
        }
        eprintln!("Z4d {kernel:?}: largest loo error {ed:.2e} of sqrt(sf2)");
    }
    eprintln!("Z4b {eb:.2e} (limit 1e-10), Z4c {ec:.2e}, Z4d {ed:.2e} (limits 1e-10 sqrt(sf2))");
    assert!(eb <= 1e-10, "Z4b");
    assert!(ec <= 1e-10, "Z4c");
    assert!(ed <= 1e-10, "Z4d");
}

#[test]
fn z4e_noise_is_absolute_per_observation() {
    let m = m_setup();
    let mut v = m.v.clone();
    v[0] = 1e8;
    let big = Gp::zero_mean_with_hyper(&m.u, &m.y, &v, Kernel::SquaredExponential, &M_L, M_SF2).unwrap();
    let drop =
        Gp::zero_mean_with_hyper(&m.u[1..], &m.y[1..], &m.v[1..], Kernel::SquaredExponential, &M_L, M_SF2).unwrap();
    let mut e = 0.0f64;
    for q in &m.q {
        let (a, sa) = big.predict(q);
        let (b, sb) = drop.predict(q);
        e = e.max((a - b).abs().max((sa - sb).abs()) / M_SF2.sqrt());
    }
    eprintln!("Z4e: v_0 = 1e8 against observation 0 removed: {e:.2e} of sqrt(sf2) (limit 1e-6)");
    assert_eq!(big.noise_variances()[0], 1e8);
    assert!(e <= 1e-6, "Z4e");
}

#[test]
fn z4f_search_workers_and_improvement() {
    let m = m_setup();
    let o1 = ZeroMeanOptions {
        kernel: Kernel::SquaredExponential,
        ..ZeroMeanOptions::default()
    };
    let o4 = ZeroMeanOptions {
        workers: 4,
        ..o1.clone()
    };
    let a = Gp::fit_zero_mean(&m.u, &m.y, &m.v, &o1).unwrap();
    let b = Gp::fit_zero_mean(&m.u, &m.y, &m.v, &o4).unwrap();
    let at = Gp::zero_mean_with_hyper(&m.u, &m.y, &m.v, Kernel::SquaredExponential, &M_L, M_SF2).unwrap();
    eprintln!(
        "Z4f: l {:?} sf2 {:e} loglik {:.12e} (4 workers {:?} {:e} {:.12e}); at (0.4, 0.6, 0.5) {:.12e}",
        a.length_scales(),
        a.process_variance(),
        a.log_likelihood(),
        b.length_scales(),
        b.process_variance(),
        b.log_likelihood(),
        at.log_likelihood()
    );
    let bits = |g: &Gp| {
        let mut v: Vec<u64> = g.length_scales().iter().map(|x| x.to_bits()).collect();
        v.push(g.process_variance().to_bits());
        v.push(g.log_likelihood().to_bits());
        v
    };
    assert_eq!(bits(&a), bits(&b), "Z4f workers");
    assert!(a.log_likelihood() >= at.log_likelihood(), "Z4f improvement");
}

#[test]
fn z4g_refusals() {
    let m = m_setup();
    let se = Kernel::SquaredExponential;
    let mut bad: Vec<Result<Gp, String>> = vec![
        Gp::zero_mean_with_hyper(&m.u, &m.y, &m.v[1..], se, &M_L, M_SF2),
        Gp::zero_mean_with_hyper(&m.u, &m.y, &m.v, se, &M_L, 0.0),
    ];
    for x in [-1.0, f64::NAN] {
        let mut v = m.v.clone();
        v[3] = x;
        bad.push(Gp::zero_mean_with_hyper(&m.u, &m.y, &v, se, &M_L, M_SF2));
        bad.push(Gp::fit_zero_mean(&m.u, &m.y, &v, &ZeroMeanOptions::default()));
    }
    bad.push(Gp::fit_zero_mean(
        &m.u,
        &m.y,
        &m.v,
        &ZeroMeanOptions {
            sf2_bounds: (1.0, 1.0),
            ..ZeroMeanOptions::default()
        },
    ));
    bad.push(Gp::fit_zero_mean(
        &m.u,
        &m.y,
        &m.v,
        &ZeroMeanOptions {
            starts: 0,
            ..ZeroMeanOptions::default()
        },
    ));
    for (i, r) in bad.iter().enumerate() {
        eprintln!("Z4g {i}: {:?}", r.as_ref().err());
        assert!(r.is_err(), "Z4g {i}");
    }
}

// ---------------------------------------------------------------- Z5: F-698's existing modes against main 9f06e866

fn digest(gp: &Gp, q: &[Vec<f64>], with_g: bool) -> String {
    let mut all = gp.length_scales().to_vec();
    all.extend([
        gp.nugget(),
        gp.jitter(),
        gp.log_likelihood(),
        gp.mean(),
        gp.process_variance(),
    ]);
    for p in q {
        let (m, s) = gp.predict(p);
        all.extend([m, s]);
    }
    for (m, s) in gp.loo() {
        all.extend([m, s]);
    }
    let (v, g) = gp.log_likelihood_and_gradient(with_g);
    all.push(v);
    all.extend(g);
    fnv(&all)
}

#[test]
fn z5_existing_modes_equal_main() {
    let u = sampling::latin_hypercube(15, 2, 735_101);
    let y: Vec<f64> = u
        .iter()
        .enumerate()
        .map(|(i, p)| (3.0 * p[0]).sin() + p[1] * p[1] + 1e-3 * ((i * 7 % 5) as f64 - 2.0))
        .collect();
    let q = sampling::latin_hypercube(7, 2, 735_102);
    let a = Gp::fit(&u, &y, &GpOptions::default()).unwrap();
    let b = Gp::fit(
        &u,
        &y,
        &GpOptions {
            kernel: Kernel::SquaredExponential,
            noise: Noise::Fitted,
            ..GpOptions::default()
        },
    )
    .unwrap();
    let c = Gp::fit(
        &u,
        &y,
        &GpOptions {
            noise: Noise::Nugget(1e-3),
            workers: 4,
            ..GpOptions::default()
        },
    )
    .unwrap();
    let d = Gp::with_hyper(&u, &y, Kernel::SquaredExponential, &[0.3, 0.5], 1e-4).unwrap();
    let got = [
        digest(&a, &q, false),
        digest(&b, &q, true),
        digest(&c, &q, false),
        digest(&d, &q, true),
    ];
    let want = [
        "2a8fd6de3d74ce5a",
        "e614e23323bfb49d",
        "8617aa91cf0047b4",
        "9f131f2c8570d07d",
    ];
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        eprintln!("Z5 {}: {g} (main {w})", ["A", "B", "C", "D"][i]);
    }
    for gp in [&a, &b, &c, &d] {
        assert!(!gp.is_zero_mean() && gp.noise_variances().is_empty());
    }
    assert_eq!(got, want, "Z5");
}
