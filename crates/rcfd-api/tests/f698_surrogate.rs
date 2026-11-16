//! ADR 0621 (F-698): the Gaussian-process surrogate (`design::surrogate`) and the expected-improvement proposal
//! (`design::bayes`). Checks G (the model), E (acquisition, exclusion), P (batch), O (optimisation), F (failures), N (noise),
//! K (constraint), R (reduced-order fan), L (ledger), D (determinism) as registered in ADR 0621 amendment 1. The heavy records
//! (O2 with O3, R1 with R2) are `#[ignore]`d and run once; fast tests cover the same paths.

use rcfd_api::design::bayes::{
    self, expected_improvement, BayesConstraint, BayesOptions, BayesParam, BayesSpec, Observation, Status,
};
use rcfd_api::design::cmaes;
use rcfd_api::design::fan_objective::*;
use rcfd_api::design::fan_rom_eval::RomCondition;
use rcfd_api::design::ledger::{self, Entry, Fidelity, FnModel, Ledger, Model, Outcome, Quantity, Verdict};
use rcfd_api::design::optim::{differential_evolution, nelder_mead, DeOptions, Fitness, NmOptions, Stop};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling::{full_factorial, latin_hypercube};
use rcfd_api::design::study::{parallel_map, Cmp, ParamDef, Sense};
use rcfd_api::design::surrogate::{cholesky, correlation, Gp, GpOptions, Kernel, Noise, JITTER};
use rcfd_mesh::fan::SectionFamily;
use rcfd_mesh::fan_design::{ComputerFan, ControlPoint, Tolerances};
use rcfd_post::fan_rom::*;
use std::f64::consts::PI;
use std::path::PathBuf;
use std::time::Instant;

const CLOCK: u64 = 1_700_000_000_000;

// ---------------------------------------------------------------- test functions

fn branin(x: &[f64]) -> f64 {
    let (b, c, t) = (5.1 / (4.0 * PI * PI), 5.0 / PI, 1.0 / (8.0 * PI));
    (x[1] - b * x[0] * x[0] + c * x[0] - 6.0).powi(2) + 10.0 * (1.0 - t) * x[0].cos() + 10.0
}
const BRANIN_B: [(f64, f64); 2] = [(-5.0, 10.0), (0.0, 15.0)];
const BRANIN_MIN: f64 = 0.397887;

fn hartmann(x: &[f64], a: &[[f64; 6]; 4], p: &[[f64; 6]; 4], d: usize) -> f64 {
    let al = [1.0, 1.2, 3.0, 3.2];
    -(0..4)
        .map(|i| al[i] * (-(0..d).map(|j| a[i][j] * (x[j] - 1e-4 * p[i][j]).powi(2)).sum::<f64>()).exp())
        .sum::<f64>()
}

fn hartmann3(x: &[f64]) -> f64 {
    let a = [
        [3.0, 10.0, 30.0, 0.0, 0.0, 0.0],
        [0.1, 10.0, 35.0, 0.0, 0.0, 0.0],
        [3.0, 10.0, 30.0, 0.0, 0.0, 0.0],
        [0.1, 10.0, 35.0, 0.0, 0.0, 0.0],
    ];
    let p = [
        [3689.0, 1170.0, 2673.0, 0.0, 0.0, 0.0],
        [4699.0, 4387.0, 7470.0, 0.0, 0.0, 0.0],
        [1091.0, 8732.0, 5547.0, 0.0, 0.0, 0.0],
        [381.0, 5743.0, 8828.0, 0.0, 0.0, 0.0],
    ];
    hartmann(x, &a, &p, 3)
}
const H3_MIN: f64 = -3.86278;

fn hartmann6(x: &[f64]) -> f64 {
    let a = [
        [10.0, 3.0, 17.0, 3.5, 1.7, 8.0],
        [0.05, 10.0, 17.0, 0.1, 8.0, 14.0],
        [3.0, 3.5, 1.7, 10.0, 17.0, 8.0],
        [17.0, 8.0, 0.05, 10.0, 0.1, 14.0],
    ];
    let p = [
        [1312.0, 1696.0, 5569.0, 124.0, 8283.0, 5886.0],
        [2329.0, 4135.0, 8307.0, 3736.0, 1004.0, 9991.0],
        [2348.0, 1451.0, 3522.0, 2883.0, 3047.0, 6650.0],
        [4047.0, 8828.0, 8732.0, 5743.0, 1091.0, 381.0],
    ];
    hartmann(x, &a, &p, 6)
}
const H6_MIN: f64 = -3.32237;

fn reached(v: f64, fmin: f64) -> bool {
    v <= fmin + 0.01 * fmin.abs()
}

fn first_hit(ys: &[f64], fmin: f64) -> Option<usize> {
    ys.iter().position(|v| reached(*v, fmin)).map(|i| i + 1)
}

fn branin_u(u: &[f64]) -> f64 {
    branin(&[-5.0 + 15.0 * u[0], 15.0 * u[1]])
}

fn sd_pop(y: &[f64]) -> f64 {
    let n = y.len() as f64;
    let m = y.iter().sum::<f64>() / n;
    (y.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n).sqrt()
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// `bayes::minimise` on `f` (None = failed): the evaluated points and values (inf for failed).
fn gp_run(
    f: &(dyn Fn(&[f64]) -> Option<f64> + Sync),
    b: &[(f64, f64)],
    o: &BayesOptions,
    budget: usize,
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut ev = |pts: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        Ok(pts
            .iter()
            .map(|x| {
                let v = f(x);
                xs.push(x.clone());
                ys.push(v.unwrap_or(f64::INFINITY));
                v.map_or(Fitness::FAILED, |v| Fitness::new(v, 0.0))
            })
            .collect())
    };
    let r = bayes::minimise(b, o, budget, &mut ev).unwrap();
    assert_eq!(r.evaluations, budget);
    (xs, ys)
}

/// E2: points outside the box or within 1e-6 (unit cube) of an earlier point.
fn e2_violations(xs: &[Vec<f64>], b: &[(f64, f64)]) -> usize {
    let u: Vec<Vec<f64>> = xs
        .iter()
        .map(|x| x.iter().zip(b).map(|(v, (lo, hi))| (v - lo) / (hi - lo)).collect())
        .collect();
    let mut bad = 0;
    for (i, p) in u.iter().enumerate() {
        let outside = p.iter().any(|v| !(0.0..=1.0).contains(v));
        let close = u[..i]
            .iter()
            .any(|q| q.iter().zip(p).map(|(a, c)| (a - c) * (a - c)).sum::<f64>().sqrt() < 1e-6);
        bad += usize::from(outside || close);
    }
    bad
}

fn bits(xs: &[Vec<f64>]) -> Vec<Vec<u64>> {
    xs.iter().map(|x| x.iter().map(|v| v.to_bits()).collect()).collect()
}

fn opts(seed: u64, n0: usize) -> BayesOptions {
    BayesOptions {
        seed,
        n0: Some(n0),
        ..Default::default()
    }
}

// ---------------------------------------------------------------- G: the model

#[test]
fn g0_test_functions_at_their_minimisers() {
    let bm = [(-PI, 12.275), (PI, 2.275), (9.42478, 2.475)];
    for (a, b) in bm {
        let v = branin(&[a, b]);
        println!("G0 Branin({a}, {b}) = {v:.9}");
        assert!((v - BRANIN_MIN).abs() <= 1e-6, "G0 Branin");
    }
    let h3 = hartmann3(&[0.114614, 0.555649, 0.852547]);
    let h6 = hartmann6(&[0.20169, 0.150011, 0.476874, 0.275332, 0.311652, 0.6573]);
    println!("G0 Hartmann 3 {h3:.8}, Hartmann 6 {h6:.8}");
    assert!((h3 - H3_MIN).abs() <= 1e-5, "G0 H3");
    assert!((h6 - H6_MIN).abs() <= 1e-5, "G0 H6");
}

#[test]
fn g1_cholesky_and_jitter() {
    let n = 50;
    let mut rng = Rng::new(698_001);
    let bm: Vec<f64> = (0..n * n).map(|_| rng.range(-1.0, 1.0)).collect();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| bm[i * n + k] * bm[j * n + k]).sum::<f64>();
        }
        a[i * n + i] += 50.0;
    }
    let l = cholesky(&a, n).unwrap();
    let amax = a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let mut e = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            let s: f64 = (0..n).map(|k| l[i * n + k] * l[j * n + k]).sum();
            e = e.max((s - a[i * n + j]).abs());
        }
    }
    println!("G1a reconstruction {:.2e}", e / amax);
    assert!(e / amax <= 1e-12, "G1a");
    let r = cholesky(&[1.0, 0.0, 0.0, -1.0], 2);
    println!("G1a diag(1, -1): {r:?}");
    assert_eq!(r.unwrap_err().0, 1, "G1a refusal row");
    let mut u = latin_hypercube(10, 2, 698_002);
    u.push(u[0].clone());
    let y: Vec<f64> = u.iter().map(|p| branin_u(p)).collect();
    let gp = Gp::with_hyper(&u, &y, Kernel::Matern52, &[0.3, 0.3], 0.0);
    println!("G1b duplicate point: {:?}", gp.as_ref().map(|g| g.jitter()));
    assert_eq!(gp.unwrap().jitter(), JITTER, "G1b");
}

fn k_hand(kernel: Kernel, a: f64, b: f64, l: f64) -> f64 {
    let d = (a - b).abs() / l;
    match kernel {
        Kernel::Matern52 => {
            let r = 5f64.sqrt() * d;
            (1.0 + r + r * r / 3.0) * (-r).exp()
        }
        Kernel::SquaredExponential => (-0.5 * d * d).exp(),
    }
}

#[test]
fn g2_two_point_closed_form() {
    let (u1, u2, y1, y2, l) = (0.2, 0.7, 1.0, 3.0, 0.4);
    let mut worst = 0.0f64;
    for kernel in [Kernel::Matern52, Kernel::SquaredExponential] {
        let gp = Gp::with_hyper(&[vec![u1], vec![u2]], &[y1, y2], kernel, &[l], 0.0).unwrap();
        let a = 1.0 + 1e-8;
        let rho = k_hand(kernel, u1, u2, l);
        let delta = (y1 - y2) / 2.0;
        let mu = (y1 + y2) / 2.0;
        let s2 = delta * delta / (a - rho);
        let mut rels = vec![(gp.process_variance() - s2).abs() / s2, (gp.mean() - mu).abs() / mu];
        for u in [0.45, 0.9] {
            let (k1, k2) = (k_hand(kernel, u, u1, l), k_hand(kernel, u, u2, l));
            let m = mu + delta * (k1 - k2) / (a - rho);
            let var = s2
                * (1.0 - (a * (k1 * k1 + k2 * k2) - 2.0 * rho * k1 * k2) / (a * a - rho * rho)
                    + (1.0 - (k1 + k2) / (a + rho)).powi(2) * (a + rho) / 2.0);
            let (pm, ps) = gp.predict(&[u]);
            rels.push((pm - m).abs() / m.abs());
            rels.push((ps * ps - var).abs() / var);
        }
        println!(
            "G2 {kernel:?}: relative differences {:?}",
            rels.iter().map(|v| format!("{v:.2e}")).collect::<Vec<_>>()
        );
        worst = rels.iter().fold(worst, |w, v| w.max(*v));
    }
    assert!(worst <= 1e-12, "G2 {worst:e}");
}

#[test]
fn g3_interpolation_and_g6b_branin_coverage() {
    let mut g3_ok = true;
    let mut g6_ok = true;
    for s in 0..10u64 {
        for (tag, seed) in [("G3", 698_101 + s), ("G6b", 698_501 + s)] {
            let u = latin_hypercube(40, 2, seed);
            let y: Vec<f64> = u.iter().map(|p| branin_u(p)).collect();
            let gp = Gp::fit(
                &u,
                &y,
                &GpOptions {
                    seed,
                    ..Default::default()
                },
            )
            .unwrap();
            let sd = sd_pop(&y);
            if tag == "G3" {
                let mut e = 0.0f64;
                let mut sm = 0.0f64;
                for (p, v) in u.iter().zip(&y) {
                    let (m, sp) = gp.predict(p);
                    e = e.max((m - v).abs() / sd);
                    sm = sm.max(sp / gp.process_variance().sqrt());
                }
                println!(
                    "G3 seed {seed}: interpolation {e:.2e}, sd {sm:.2e}, l {:?}, jitter {:e}",
                    gp.length_scales(),
                    gp.jitter()
                );
                g3_ok &= e <= 1e-2 && sm <= 1e-2;
            } else {
                let mut rng = Rng::new(seed + 1000);
                let (mut inside, mut se) = (0usize, 0.0);
                for _ in 0..2000 {
                    let p = vec![rng.uniform(), rng.uniform()];
                    let v = branin_u(&p);
                    let (m, sp) = gp.predict(&p);
                    inside += usize::from((v - m).abs() <= 1.96 * sp);
                    se += (v - m).powi(2);
                }
                let cov = inside as f64 / 2000.0;
                let rms = (se / 2000.0).sqrt() / sd;
                println!("G6b seed {seed}: coverage {cov:.4}, rms/sd {rms:.4}");
                g6_ok &= cov >= 0.90 && rms <= 0.10;
            }
        }
    }
    assert!(g3_ok, "G3");
    assert!(g6_ok, "G6b");
}

#[test]
fn g4_likelihood_gradient() {
    let u = latin_hypercube(30, 3, 698_201);
    let y: Vec<f64> = u.iter().map(|p| hartmann3(p)).collect();
    let mut rng = Rng::new(698_202);
    let mut worst = 0.0f64;
    for _ in 0..5 {
        let mut t: Vec<f64> = (0..3).map(|_| rng.range(0.1f64.ln(), 0.0)).collect();
        t.push(rng.range(1e-4f64.ln(), 0.1f64.ln()));
        for kernel in [Kernel::Matern52, Kernel::SquaredExponential] {
            let mk =
                |t: &[f64]| Gp::with_hyper(&u, &y, kernel, &[t[0].exp(), t[1].exp(), t[2].exp()], t[3].exp()).unwrap();
            let (_, g) = mk(&t).log_likelihood_and_gradient(true);
            let h = 1e-5;
            let fd: Vec<f64> = (0..4)
                .map(|i| {
                    let mut tp = t.clone();
                    tp[i] += h;
                    let mut tm = t.clone();
                    tm[i] -= h;
                    (mk(&tp).log_likelihood() - mk(&tm).log_likelihood()) / (2.0 * h)
                })
                .collect();
            let mx = fd.iter().fold(0.0f64, |a, b| a.max(b.abs()));
            let e = g.iter().zip(&fd).fold(0.0f64, |a, (x, z)| a.max((x - z).abs())) / mx;
            println!("G4 {kernel:?} t {t:.3?}: relative {e:.2e}");
            worst = worst.max(e);
        }
    }
    assert!(worst <= 1e-5, "G4 {worst:e}");
}

fn prior_draw(u: &[Vec<f64>], l: &[f64], g: f64, rng: &mut Rng) -> Vec<f64> {
    let n = u.len();
    let mut k = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            k[i * n + j] = correlation(Kernel::Matern52, &u[i], &u[j], l);
        }
        k[i * n + i] += g + 1e-10;
    }
    let lf = cholesky(&k, n).unwrap();
    let z: Vec<f64> = (0..n).map(|_| cmaes::normal(rng)).collect();
    (0..n).map(|i| (0..=i).map(|j| lf[i * n + j] * z[j]).sum()).collect()
}

#[test]
fn g5_hyperparameter_recovery() {
    let res = parallel_map(&(0..20u64).collect::<Vec<_>>(), 4, &|&k: &u64| {
        let mut rng = Rng::new(698_300 + k);
        let u = latin_hypercube(100, 1, 698_300 + k);
        let y = prior_draw(&u, &[0.2], 0.01, &mut rng);
        let gp = Gp::fit(
            &u,
            &y,
            &GpOptions {
                noise: Noise::Fitted,
                seed: k,
                ..Default::default()
            },
        )
        .unwrap();
        (gp.length_scales()[0], gp.nugget())
    });
    let mut ls: Vec<f64> = res.iter().map(|r| r.0).collect();
    let mut gs: Vec<f64> = res.iter().map(|r| r.1).collect();
    println!("G5 l {ls:.4?}");
    println!("G5 g {:?}", gs.iter().map(|v| format!("{v:.3e}")).collect::<Vec<_>>());
    let (ml, mg) = (median(&mut ls), median(&mut gs));
    println!("G5 median l {ml:.4} (limit [0.1333, 0.3]), median g {mg:.4e} (limit [0.00333, 0.03])");
    assert!((0.2 / 1.5..=0.3).contains(&ml), "G5 l");
    assert!((0.01 / 3.0..=0.03).contains(&mg), "G5 g");
}

fn coverage_of_prior(l_model: f64) -> (f64, f64) {
    let (mut inside, mut total, mut se) = (0usize, 0usize, 0.0);
    for k in 0..50u64 {
        let mut rng = Rng::new(698_400 + k);
        let mut u = latin_hypercube(40, 2, 698_400 + k);
        for _ in 0..200 {
            u.push(vec![rng.uniform(), rng.uniform()]);
        }
        let y = prior_draw(&u, &[0.3, 0.3], 0.0, &mut rng);
        let gp = Gp::with_hyper(&u[..40], &y[..40], Kernel::Matern52, &[l_model, l_model], 0.0).unwrap();
        for i in 40..240 {
            let (m, s) = gp.predict(&u[i]);
            total += 1;
            inside += usize::from((y[i] - m).abs() <= 1.96 * s);
            se += (y[i] - m).powi(2);
        }
    }
    (inside as f64 / total as f64, (se / total as f64).sqrt())
}

#[test]
fn g6a_coverage_on_prior_draws_and_nc2() {
    let (c, e) = coverage_of_prior(0.3);
    let (c2, e2) = coverage_of_prior(3.0);
    println!("G6a pooled coverage {c:.4} (rms error {e:.4}); NC2 (l = 3) {c2:.4} (rms error {e2:.4})");
    assert!((0.92..=0.98).contains(&c), "G6a");
    assert!(c2 < 0.92, "NC2");
}

#[test]
fn g6c_hartmann6_fit_record() {
    for seed in 698_601..=698_605u64 {
        let u = latin_hypercube(120, 6, seed);
        let y: Vec<f64> = u.iter().map(|p| hartmann6(p)).collect();
        let gp = Gp::fit(
            &u,
            &y,
            &GpOptions {
                seed,
                ..Default::default()
            },
        )
        .unwrap();
        let mut rng = Rng::new(seed + 1000);
        let (mut inside, mut se) = (0usize, 0.0);
        for _ in 0..2000 {
            let p: Vec<f64> = (0..6).map(|_| rng.uniform()).collect();
            let v = hartmann6(&p);
            let (m, s) = gp.predict(&p);
            inside += usize::from((v - m).abs() <= 1.96 * s);
            se += (v - m).powi(2);
        }
        println!(
            "G6c seed {seed}: coverage {:.4}, rms/sd {:.4} (recorded)",
            inside as f64 / 2000.0,
            (se / 2000.0).sqrt() / sd_pop(&y)
        );
    }
}

#[test]
fn g7_leave_one_out() {
    let u = latin_hypercube(25, 2, 698_701);
    let y: Vec<f64> = u.iter().map(|p| branin_u(p)).collect();
    let gp = Gp::with_hyper(&u, &y, Kernel::Matern52, &[0.3, 0.3], 0.0).unwrap();
    let loo = gp.loo();
    let sd = sd_pop(&y);
    let mut e = 0.0f64;
    for i in 0..25 {
        let mut ui = u.clone();
        ui.remove(i);
        let mut yi = y.clone();
        yi.remove(i);
        let g2 = Gp::with_hyper(&ui, &yi, Kernel::Matern52, &[0.3, 0.3], 0.0).unwrap();
        let (m, s) = g2.predict(&u[i]);
        let s_obs =
            (s * s * gp.process_variance() / g2.process_variance() + gp.process_variance() * g2.jitter()).sqrt();
        e = e.max((m - loo[i].0).abs() / sd).max((s_obs - loo[i].1).abs() / sd);
    }
    println!("G7 max difference / sd(y) {e:.2e}");
    assert!(e <= 1e-9, "G7");
}

// ---------------------------------------------------------------- E, P: acquisition

fn norm_pdf(t: f64) -> f64 {
    (-0.5 * t * t).exp() / (2.0 * PI).sqrt()
}

#[test]
fn e1_expected_improvement_against_quadrature() {
    let mut worst = 0.0f64;
    for s in [0.01, 0.1, 1.0, 3.0] {
        for z in [-6.0, -2.0, 0.0, 1.0, 4.0] {
            let ei = expected_improvement(0.0, s, z * s);
            let (a, b, n) = (z - 40.0, z, 400_000usize);
            let h = (b - a) / n as f64;
            let f = |t: f64| (z - t) * norm_pdf(t);
            let mut q = f(a) + f(b);
            for k in 1..n {
                q += if k % 2 == 1 { 4.0 } else { 2.0 } * f(a + k as f64 * h);
            }
            q *= s * h / 3.0;
            println!(
                "E1 s {s} z {z}: EI {ei:.15e}, quadrature {q:.15e}, diff {:.2e}",
                (ei - q).abs()
            );
            worst = worst.max((ei - q).abs());
        }
    }
    assert!(worst <= 1e-10, "E1 {worst:e}");
    assert_eq!(expected_improvement(2.0, 0.0, 1.0), 0.0, "E1 s = 0");
    assert_eq!(expected_improvement(1.0, 0.0, 2.0), 1.0, "E1 s = 0");
}

#[test]
fn p1_batch_and_pending_points_are_spread() {
    let u = latin_hypercube(15, 2, 698_950);
    let obs: Vec<Observation> = u
        .iter()
        .map(|p| {
            let x = vec![-5.0 + 15.0 * p[0], 15.0 * p[1]];
            Observation {
                status: Status::Ok {
                    f: branin(&x),
                    c: vec![],
                },
                x,
            }
        })
        .collect();
    let uo = |x: &[f64]| vec![(x[0] + 5.0) / 15.0, x[1] / 15.0];
    let dist = |a: &[f64], b: &[f64]| {
        let (a, b) = (uo(a), uo(b));
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
    };
    let o4 = BayesOptions {
        batch: 4,
        ..opts(698_951, 10)
    };
    let p = bayes::propose(&BRANIN_B, &obs, &o4).unwrap();
    let mut dmin = f64::INFINITY;
    for i in 0..4 {
        for j in 0..i {
            dmin = dmin.min(dist(&p.picks[i].x, &p.picks[j].x));
        }
    }
    let o1 = opts(698_951, 10);
    let p1 = bayes::propose(&BRANIN_B, &obs, &o1).unwrap();
    let mut obs2 = obs.clone();
    obs2.push(Observation {
        x: p1.picks[0].x.clone(),
        status: Status::Pending,
    });
    let p2 = bayes::propose(&BRANIN_B, &obs2, &o1).unwrap();
    let dp = dist(&p2.picks[0].x, &p1.picks[0].x);
    println!(
        "P1 batch of 4: min pairwise distance {dmin:.4}; pending: {dp:.4}; kinds {:?}",
        p.picks.iter().map(|k| k.kind).collect::<Vec<_>>()
    );
    assert!(dmin >= 1e-3, "P1 batch");
    assert!(dp >= 1e-3, "P1 pending");
}

// ---------------------------------------------------------------- O: optimisation

#[test]
fn o1_branin_nc1_random_o4_nelder_mead_d1_d2() {
    let t = Instant::now();
    let seeds: Vec<u64> = (1..=20).collect();
    let runs = parallel_map(&seeds, 5, &|&s: &u64| {
        gp_run(&|x| Some(branin(x)), &BRANIN_B, &opts(s, 10), 60)
    });
    let mut hits = 0;
    let mut evals = Vec::new();
    let mut e2 = 0;
    for (s, (xs, ys)) in seeds.iter().zip(&runs) {
        let h = first_hit(ys, BRANIN_MIN);
        println!(
            "O1 seed {s}: target at {h:?}, best {:.6}",
            ys.iter().cloned().fold(f64::INFINITY, f64::min)
        );
        hits += usize::from(h.is_some());
        evals.push(h.map_or(61.0, |v| v as f64));
        e2 += e2_violations(xs, &BRANIN_B);
    }
    let med = median(&mut evals);
    let mut rand_hits = 0;
    let mut nm_hits = 0;
    for &s in &seeds {
        let mut rng = Rng::new(s);
        let ys: Vec<f64> = (0..60)
            .map(|_| branin(&[rng.range(-5.0, 10.0), rng.range(0.0, 15.0)]))
            .collect();
        rand_hits += usize::from(first_hit(&ys, BRANIN_MIN).is_some());
        let mut yn = Vec::new();
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            Ok(p.iter()
                .map(|x| {
                    let v = branin(x);
                    yn.push(v);
                    Fitness::new(v, 0.0)
                })
                .collect())
        };
        let _ = nelder_mead(
            &BRANIN_B,
            &NmOptions {
                max_evals: 60,
                seed: s,
                ..Default::default()
            },
            &mut ev,
        );
        nm_hits += usize::from(first_hit(&yn, BRANIN_MIN).is_some());
    }
    println!(
        "O1: {hits} of 20, median {med}; NC1 random search {rand_hits} of 20; O4 Nelder-Mead {nm_hits} of 20 (recorded); E2 violations {e2}; {:.1} s",
        t.elapsed().as_secs_f64()
    );
    // D1: seed 1 again at workers 1 and at workers 8
    let again = gp_run(&|x| Some(branin(x)), &BRANIN_B, &opts(1, 10), 60).0;
    let w8 = gp_run(
        &|x| Some(branin(x)),
        &BRANIN_B,
        &BayesOptions {
            workers: 8,
            ..opts(1, 10)
        },
        60,
    )
    .0;
    let d1 = bits(&again) == bits(&runs[0].0) && bits(&w8) == bits(&runs[0].0);
    // D2: seed 2 replayed from the table of the direct run
    let table: Vec<(Vec<u64>, f64)> = runs[1]
        .0
        .iter()
        .zip(&runs[1].1)
        .map(|(x, y)| (x.iter().map(|v| v.to_bits()).collect(), *y))
        .collect();
    let mut replay = Vec::new();
    let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        let mut out = Vec::new();
        for x in p {
            let k: Vec<u64> = x.iter().map(|v| v.to_bits()).collect();
            match table.iter().find(|t| t.0 == k) {
                Some(t) => out.push(Fitness::new(t.1, 0.0)),
                None => return Err(Stop::Missing(vec![x.clone()])),
            }
            replay.push(x.clone());
        }
        Ok(out)
    };
    let d2 = bayes::minimise(&BRANIN_B, &opts(2, 10), 60, &mut ev).is_ok() && bits(&replay) == bits(&runs[1].0);
    println!("D1 repeat and workers 8 bit-equal: {d1}; D2 replay bit-equal: {d2}");
    assert!(hits >= 18, "O1 count");
    assert!(med <= 35.0, "O1 median");
    assert!(rand_hits <= 4, "NC1");
    assert_eq!(e2, 0, "E2");
    assert!(d1, "D1");
    assert!(d2, "D2");
}

#[test]
fn o_fast_hartmann3_path() {
    let b = [(0.0, 1.0); 3];
    let (xs, ys) = gp_run(&|x| Some(hartmann3(x)), &b, &opts(1, 15), 40);
    println!(
        "O fast: Hartmann 3 target at {:?}, best {:.5} (recorded)",
        first_hit(&ys, H3_MIN),
        ys.iter().cloned().fold(f64::INFINITY, f64::min)
    );
    assert_eq!(e2_violations(&xs, &b), 0, "E2");
}

#[test]
#[ignore = "heavy record (ADR 0621 O2, O3): about 100 s per seed"]
fn o2_o3_hartmann6_record() {
    let t = Instant::now();
    let b = [(0.0, 1.0); 6];
    let seeds: Vec<u64> = (1..=10).collect();
    let runs = parallel_map(&seeds, 10, &|&s: &u64| {
        gp_run(&|x| Some(hartmann6(x)), &b, &opts(s, 30), 200)
    });
    let (mut gp_hits, mut cma_hits, mut de_hits, mut e2) = (0, 0, 0, 0);
    let mut largest = 0;
    let mut worst_best = f64::NEG_INFINITY;
    for (s, (xs, ys)) in seeds.iter().zip(&runs) {
        let h = first_hit(ys, H6_MIN);
        let best = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        gp_hits += usize::from(h.is_some());
        largest = largest.max(h.unwrap_or(0));
        worst_best = worst_best.max(best);
        e2 += e2_violations(xs, &b);
        let mut yc = Vec::new();
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            Ok(p.iter()
                .map(|x| {
                    let v = hartmann6(x);
                    yc.push(v);
                    Fitness::new(v, 0.0)
                })
                .collect())
        };
        let _ = cmaes::run(
            &b,
            &cmaes::Options {
                max_evals: 200,
                seed: *s,
                ..Default::default()
            },
            &mut ev,
        );
        let hc = first_hit(&yc, H6_MIN);
        let mut yd = Vec::new();
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            Ok(p.iter()
                .map(|x| {
                    let v = hartmann6(x);
                    yd.push(v);
                    Fitness::new(v, 0.0)
                })
                .collect())
        };
        let _ = differential_evolution(
            &b,
            &DeOptions {
                max_evals: 200,
                seed: *s,
                ..Default::default()
            },
            &mut ev,
        );
        let hd = first_hit(&yd, H6_MIN);
        cma_hits += usize::from(hc.is_some());
        de_hits += usize::from(hd.is_some());
        println!(
            "O2 seed {s}: GP target at {h:?}, best {best:.5}; O3 CMA-ES {hc:?} (best {:.4}, {} evals), DE {hd:?} (best {:.4}, {} evals)",
            yc.iter().cloned().fold(f64::INFINITY, f64::min),
            yc.len(),
            yd.iter().cloned().fold(f64::INFINITY, f64::min),
            yd.len()
        );
    }
    println!(
        "O2: {gp_hits} of 10, largest evaluations to target {largest}, worst best {worst_best:.5}; O3: CMA-ES {cma_hits}, DE {de_hits}; E2 {e2}; {:.0} s",
        t.elapsed().as_secs_f64()
    );
    assert_eq!(e2, 0, "E2");
    assert!(gp_hits >= 5, "O2 count");
    assert!(largest <= 100, "O2 largest");
    assert!(worst_best <= -3.15, "O2 best");
    assert!(gp_hits >= cma_hits.max(de_hits) + 2, "O3");
}

// ---------------------------------------------------------------- F, N, K

fn in_hole(x: &[f64]) -> bool {
    let u = [(x[0] + 5.0) / 15.0, x[1] / 15.0];
    let c = [(PI + 5.0) / 15.0, 2.275 / 15.0];
    ((u[0] - c[0]).powi(2) + (u[1] - c[1]).powi(2)).sqrt() < 0.15
}

#[test]
fn f1_f2_failure_region_and_d1_with_failures() {
    let f = |x: &[f64]| if in_hole(x) { None } else { Some(branin(x)) };
    let seeds: Vec<u64> = (1..=20).collect();
    let mut pooled = [0.0f64; 2];
    let mut hits = 0;
    let mut worst_share = 0.0f64;
    let mut e2 = 0;
    for (k, on) in [true, false].into_iter().enumerate() {
        let runs = parallel_map(&seeds, 5, &|&s: &u64| {
            let o = BayesOptions {
                impute: on,
                classify_failures: on,
                ..opts(s, 10)
            };
            gp_run(&f, &BRANIN_B, &o, 60)
        });
        let mut failed = 0;
        for (s, (xs, ys)) in seeds.iter().zip(&runs) {
            let fl = ys[10..].iter().filter(|v| v.is_infinite()).count();
            failed += fl;
            let h = first_hit(ys, BRANIN_MIN);
            println!(
                "{} seed {s}: target at {h:?}, failed after the initial design {fl} of 50",
                if on { "F1" } else { "F2" }
            );
            if on {
                hits += usize::from(h.is_some());
                worst_share = worst_share.max(fl as f64 / 50.0);
            }
            e2 += e2_violations(xs, &BRANIN_B);
        }
        pooled[k] = failed as f64 / (50.0 * 20.0);
    }
    let w1 = gp_run(&f, &BRANIN_B, &opts(1, 10), 60).0;
    let w8 = gp_run(
        &f,
        &BRANIN_B,
        &BayesOptions {
            workers: 8,
            ..opts(1, 10)
        },
        60,
    )
    .0;
    let d1 = bits(&w1) == bits(&w8);
    println!(
        "F1: {hits} of 20, worst failed share {worst_share:.3}, pooled {:.3}; F2 pooled {:.3}; E2 {e2}; D1 with failures bit-equal {d1}",
        pooled[0], pooled[1]
    );
    assert!(hits >= 18, "F1 count");
    assert!(worst_share <= 0.25, "F1 share");
    assert!(pooled[1] > pooled[0], "F2");
    assert_eq!(e2, 0, "E2");
    assert!(d1, "D1 failures");
}

#[test]
fn n1_noisy_branin() {
    let seeds: Vec<u64> = (1..=20).collect();
    let res = parallel_map(&seeds, 5, &|&s: &u64| {
        let o = BayesOptions {
            gp: GpOptions {
                noise: Noise::Fitted,
                ..Default::default()
            },
            ..opts(s, 10)
        };
        let noise = std::sync::Mutex::new(Rng::new(698_800 + s));
        let f = |x: &[f64]| {
            let mut r = noise.lock().unwrap();
            Some(branin(x) + cmaes::normal(&mut r))
        };
        let (xs, ys) = gp_run(&f, &BRANIN_B, &o, 80);
        let u: Vec<Vec<f64>> = xs.iter().map(|x| vec![(x[0] + 5.0) / 15.0, x[1] / 15.0]).collect();
        let gp = Gp::fit(
            &u,
            &ys,
            &GpOptions {
                noise: Noise::Fitted,
                seed: 698_899,
                ..Default::default()
            },
        )
        .unwrap();
        let mut bi = 0;
        let mut bm = f64::INFINITY;
        for (i, p) in u.iter().enumerate() {
            let m = gp.predict(p).0;
            if m < bm {
                bm = m;
                bi = i;
            }
        }
        (branin(&xs[bi]), gp.nugget(), e2_violations(&xs, &BRANIN_B))
    });
    let mut ok = 0;
    let mut e2 = 0;
    for (s, r) in seeds.iter().zip(&res) {
        println!("N1 seed {s}: true value {:.4}, nugget {:.2e}", r.0, r.1);
        ok += usize::from(r.0 <= BRANIN_MIN + 1.0);
        e2 += r.2;
    }
    println!("N1: {ok} of 20 (limit 16); E2 {e2}");
    assert!(ok >= 16, "N1");
    assert_eq!(e2, 0, "E2");
}

#[test]
fn k1_constraint_through_run() {
    let m = FnModel {
        id: "test.quad".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| {
            let (a, b) = (p[0].value, p[1].value);
            Outcome::ok(vec![
                Quantity::new("f", (a - 0.8).powi(2) + (b - 0.8).powi(2), "1"),
                Quantity::new("g", a + b, "1"),
            ])
        },
    };
    let mut ok = 0;
    let mut e2 = 0;
    for s in 1..=10u64 {
        let spec = BayesSpec {
            study: "k1".into(),
            params: vec![
                BayesParam {
                    name: "a".into(),
                    unit: "1".into(),
                    lo: 0.0,
                    hi: 1.0,
                },
                BayesParam {
                    name: "b".into(),
                    unit: "1".into(),
                    lo: 0.0,
                    hi: 1.0,
                },
            ],
            objective: "f".into(),
            sense: Sense::Min,
            constraints: vec![BayesConstraint {
                metric: "g".into(),
                cmp: Cmp::Le,
                limit: 1.0,
            }],
            budget: 40,
            options: opts(s, 10),
            clock_ms: Some(CLOCK),
        };
        let r = bayes::run(&spec, &m, None).unwrap();
        let infeasible = r.entries[10..]
            .iter()
            .filter(|e| e.metric("g").unwrap().value > 1.0)
            .count();
        let met = r.best_value.is_some_and(|v| v <= 0.18 * 1.01);
        println!(
            "K1 seed {s}: best feasible {:?}, infeasible after the initial design {infeasible} (recorded)",
            r.best_value
        );
        ok += usize::from(met);
        let xs: Vec<Vec<f64>> = r
            .entries
            .iter()
            .map(|e| e.params.iter().map(|q| q.value).collect())
            .collect();
        e2 += e2_violations(&xs, &[(0.0, 1.0), (0.0, 1.0)]);
    }
    println!("K1: {ok} of 10; E2 {e2}");
    assert!(ok >= 9, "K1");
    assert_eq!(e2, 0, "E2");
}

// ---------------------------------------------------------------- R: the reduced-order fan

fn op3000() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn base_cond() -> RomCondition {
    RomCondition {
        op: op3000(),
        system: SystemCurve { k: 1.0e4, l: 0.0 },
        c0: 343.0,
        observer_r: 1.0,
        observer_theta: std::f64::consts::FRAC_PI_2,
        n_scan: 100,
    }
}

/// ADR 0620 set-up F.
fn fan_f() -> ComputerFan {
    let s = |radius: f64| RomSection {
        radius,
        chord: 0.030,
        stagger: 0.0,
        camber: 0.04,
        camber_pos: 0.5,
    };
    let base = RomFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        family: SectionFamily::Naca4,
        sections: vec![s(0.030), s(0.058)],
    };
    let m = RomModel::default();
    let fv = free_vortex_design(&base, &m, &op3000(), 0.045, 45.0 / 1.2 / op3000().omega()).unwrap();
    let span = |r: f64| (r - fv.r_hub) / (fv.r_tip - fv.r_hub);
    let cp = |sp: f64, sec: &RomSection| ControlPoint {
        span: sp,
        chord: sec.chord,
        stagger_deg: sec.stagger.to_degrees(),
        camber: sec.camber,
        camber_pos: sec.camber_pos,
        thickness: 0.08,
        sweep: 0.0,
        skew_deg: 0.0,
    };
    let mut st = vec![cp(0.0, &fv.sections[0])];
    for sec in &fv.sections {
        st.push(cp(span(sec.radius), sec));
    }
    st.push(cp(1.0, fv.sections.last().unwrap()));
    ComputerFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        stations: st,
        ..ComputerFan::default()
    }
}

fn target_f3() -> FanRomTarget {
    let fan = fan_f();
    FanRomTarget {
        geometry: Box::new(move |x: &[f64]| {
            let mut f = fan.clone();
            for s in &mut f.stations {
                s.stagger_deg += x[0];
                s.chord *= x[1];
                s.camber *= x[2];
            }
            Ok(f)
        }),
        tolerances: Tolerances::default(),
        model: RomModel::default(),
        base: base_cond(),
    }
}

const F3_BOX: [(&str, &str, f64, f64); 3] = [
    ("stagger_offset_deg", "deg", -3.0, 3.0),
    ("chord_scale", "1", 0.8, 1.2),
    ("camber_scale", "1", 0.5, 1.5),
];

fn objective_f3() -> FanObjective {
    let pt = |label: &str, k: f64, weight: f64| ImpedancePoint {
        label: label.into(),
        system: SystemCurve { k, l: 0.0 },
        weight,
    };
    FanObjective {
        name: "f698_rom".into(),
        params: F3_BOX
            .iter()
            .map(|(n, u, lo, hi)| {
                (
                    ParamDef {
                        name: n.to_string(),
                        lo: *lo,
                        hi: *hi,
                        init: None,
                        targets: vec![],
                    },
                    u.to_string(),
                )
            })
            .collect(),
        points: vec![pt("R1", 5.0e3, 0.5), pt("R2", 1.5e4, 0.3), pt("R3", 2.5e4, 0.2)],
        terms: vec![Term {
            metric: "power".into(),
            sense: Sense::Min,
            tol: TolStat::Nominal,
            over: Over::Mean,
            weight: 1.0,
        }],
        limits: vec![],
        plan: None,
        workers: 1,
    }
}

fn spec_f3(seed: u64, budget: usize) -> BayesSpec {
    BayesSpec {
        study: "f698_rom".into(),
        params: F3_BOX
            .iter()
            .map(|(n, u, lo, hi)| BayesParam {
                name: n.to_string(),
                unit: u.to_string(),
                lo: *lo,
                hi: *hi,
            })
            .collect(),
        objective: "power@nominal@mean".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget,
        options: BayesOptions {
            seed,
            ..Default::default()
        },
        clock_ms: Some(CLOCK),
    }
}

fn failed_entries_located(entries: &[Entry]) -> bool {
    entries.iter().filter(|e| e.verdict != Verdict::Ok).all(|e| {
        e.verdict == Verdict::Diverged
            && ["R1", "R2", "R3"]
                .iter()
                .any(|p| e.reason.contains(&format!("point '{p}'")))
    })
}

fn f3_values(m: &dyn Model, xs: &[Vec<f64>], workers: usize) -> Vec<Option<f64>> {
    parallel_map(xs, workers, &|x: &Vec<f64>| {
        let params: Vec<Quantity> = F3_BOX
            .iter()
            .zip(x)
            .map(|((n, u, _, _), v)| Quantity::new(n, *v, u))
            .collect();
        let out = m.evaluate(&params);
        (out.verdict == Verdict::Ok).then(|| {
            out.metrics
                .iter()
                .find(|q| q.name == "power@nominal@mean")
                .unwrap()
                .value
        })
    })
}

#[test]
fn r_fast_fan_path() {
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    assert_eq!(o.terms[0].label(), "power@nominal@mean");
    let r = bayes::run(&spec_f3(1, 18), &m, None).unwrap();
    let ok_prov = r
        .entries
        .iter()
        .all(|e| e.provenance["driver"] == "gp_ei" && e.provenance.get("predicted_sd").is_some());
    let modelled = r.entries[15..]
        .iter()
        .all(|e| e.provenance["predicted_mean"].as_f64().is_some_and(f64::is_finite));
    println!(
        "R fast: best {:?} W, failed {} of 18, provenance {ok_prov}, modelled picks have a mean {modelled}",
        r.best_value, r.failed
    );
    assert_eq!(r.entries.len(), 18);
    assert!(ok_prov && modelled, "R fast provenance");
    assert!(failed_entries_located(&r.entries), "R fast failure reasons");
}

#[test]
#[ignore = "heavy record (ADR 0621 R1, R2): the 9261-point reference, about 12 minutes on 12 threads"]
fn r1_r2_reduced_order_fan_record() {
    let t = Instant::now();
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let grid: Vec<Vec<f64>> = full_factorial(&[21, 21, 21])
        .iter()
        .map(|u| {
            F3_BOX
                .iter()
                .zip(u)
                .map(|((_, _, lo, hi), v)| lo + (hi - lo) * v)
                .collect()
        })
        .collect();
    let vals = f3_values(&m, &grid, 12);
    let ok: Vec<f64> = vals.iter().flatten().cloned().collect();
    let p_ref = ok.iter().cloned().fold(f64::INFINITY, f64::min);
    let p_max = ok.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let bi = vals.iter().position(|v| *v == Some(p_ref)).unwrap();
    println!(
        "R1 reference: {} of 9261 feasible, P_ref {p_ref:.6} W at {:?}, largest {p_max:.4} W, {:.0} s",
        ok.len(),
        grid[bi],
        t.elapsed().as_secs_f64()
    );
    let seeds: Vec<u64> = (1..=10).collect();
    let runs = parallel_map(&seeds, 10, &|&s: &u64| bayes::run(&spec_f3(s, 50), &m, None).unwrap());
    let (mut hits, mut range_hits, mut located, mut e2) = (0, 0, true, 0);
    let b: Vec<(f64, f64)> = F3_BOX.iter().map(|(_, _, lo, hi)| (*lo, *hi)).collect();
    for (s, r) in seeds.iter().zip(&runs) {
        let best = r.best_value.unwrap_or(f64::INFINITY);
        let met = best <= p_ref * 1.01;
        let met_range = best <= p_ref + 0.01 * (p_max - p_ref);
        let at = r
            .entries
            .iter()
            .position(|e| e.verdict == Verdict::Ok && e.metric("power@nominal@mean").unwrap().value <= p_ref * 1.01)
            .map(|i| i + 1);
        println!(
            "R1 seed {s}: best {best:.6} W (ratio {:.5}), target at {at:?}, failed {} of 50, within 1 percent of the range {met_range}",
            best / p_ref,
            r.failed
        );
        hits += usize::from(met);
        range_hits += usize::from(met_range);
        located &= failed_entries_located(&r.entries);
        let xs: Vec<Vec<f64>> = r
            .entries
            .iter()
            .map(|e| e.params.iter().map(|q| q.value).collect())
            .collect();
        e2 += e2_violations(&xs, &b);
    }
    // R2: CMA-ES and Nelder-Mead at the same budget
    let (mut cma_hits, mut nm_hits) = (0, 0);
    for &s in &seeds {
        for which in 0..2 {
            let mut ys: Vec<f64> = Vec::new();
            let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
                let v = f3_values(&m, p, 4);
                Ok(v.into_iter()
                    .map(|v| {
                        ys.push(v.unwrap_or(f64::INFINITY));
                        v.map_or(Fitness::FAILED, |v| Fitness::new(v, 0.0))
                    })
                    .collect())
            };
            if which == 0 {
                let _ = cmaes::run(
                    &b,
                    &cmaes::Options {
                        max_evals: 50,
                        seed: s,
                        ..Default::default()
                    },
                    &mut ev,
                );
            } else {
                let _ = nelder_mead(
                    &b,
                    &NmOptions {
                        max_evals: 50,
                        seed: s,
                        ..Default::default()
                    },
                    &mut ev,
                );
            }
            let best = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let met = best <= p_ref * 1.01;
            println!(
                "R2 seed {s} {}: best {best:.5} W, {} evals, failed {}, within 1 percent {met}",
                if which == 0 { "CMA-ES" } else { "Nelder-Mead" },
                ys.len(),
                ys.iter().filter(|v| v.is_infinite()).count()
            );
            if which == 0 {
                cma_hits += usize::from(met);
            } else {
                nm_hits += usize::from(met);
            }
        }
    }
    println!(
        "R1: {hits} of 10 (limit 9), within 1 percent of the range {range_hits} of 10 (recorded); failures located {located}; E2 {e2}; R2: CMA-ES {cma_hits}, Nelder-Mead {nm_hits} of 10 (recorded); {:.0} s",
        t.elapsed().as_secs_f64()
    );
    assert!(hits >= 9, "R1");
    assert!(located, "R1 reasons");
    assert_eq!(e2, 0, "E2");
}

// ---------------------------------------------------------------- L: ledger

fn branin_model() -> FnModel<impl Fn(&[Quantity]) -> Outcome + Sync> {
    FnModel {
        id: "test.branin".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| Outcome::ok(vec![Quantity::new("f", branin(&[p[0].value, p[1].value]), "1")]),
    }
}

fn branin_spec(budget: usize) -> BayesSpec {
    BayesSpec {
        study: "l1".into(),
        params: vec![
            BayesParam {
                name: "x1".into(),
                unit: "1".into(),
                lo: -5.0,
                hi: 10.0,
            },
            BayesParam {
                name: "x2".into(),
                unit: "1".into(),
                lo: 0.0,
                hi: 15.0,
            },
        ],
        objective: "f".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget,
        options: opts(698_901, 10),
        clock_ms: Some(CLOCK),
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("f698");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(name);
    let _ = std::fs::remove_file(&p);
    p
}

fn evaluation_lines(p: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter(|l| l.contains("\"kind\":\"evaluation\""))
        .map(str::to_string)
        .collect()
}

#[test]
fn l1_l2_ledger_resume_and_line_order() {
    let m = branin_model();
    let p = tmp("l1.jsonl");
    let led = Ledger::new(&p);
    let r = bayes::run(&branin_spec(25), &m, Some(&led)).unwrap();
    let entries = led.read().unwrap().entries;
    let keys = [
        "driver",
        "iteration",
        "batch_index",
        "acquisition",
        "acquisition_value",
        "ei",
        "predicted_mean",
        "predicted_sd",
        "feasibility",
        "incumbent",
        "objective",
        "sense",
        "length_scales",
        "nugget",
        "jitter",
        "n_obs",
        "n_imputed",
        "n_pending",
    ];
    let evals: Vec<&Entry> = entries.iter().filter(|e| e.kind == "evaluation").collect();
    let prov_ok = evals.len() == 25
        && evals.iter().all(|e| keys.iter().all(|k| e.provenance.get(k).is_some()))
        && evals[10..].iter().all(|e| {
            e.provenance["predicted_mean"].as_f64().is_some_and(f64::is_finite)
                && e.provenance["predicted_sd"].as_f64().is_some_and(f64::is_finite)
        });
    let summary = entries.iter().find(|e| e.kind == "surrogate_summary").unwrap();
    println!(
        "L1: {} evaluation entries, provenance complete {prov_ok}; summary metrics {:?}, fitted {:?}, {} parents",
        evals.len(),
        summary
            .metrics
            .iter()
            .map(|q| (q.name.clone(), q.value))
            .collect::<Vec<_>>(),
        summary.fitted,
        summary.parents.len()
    );
    let direct = bayes::run(&branin_spec(26), &m, None).unwrap();
    let next = bayes::propose_from_entries(&entries, &branin_spec(1), "test.branin").unwrap();
    let l1_next = bits(&[next.picks[0].x.clone()]) == bits(&[direct.picks[25].x.clone()]);
    let first25 = bits(&direct.picks[..25].iter().map(|k| k.x.clone()).collect::<Vec<_>>())
        == bits(&r.picks.iter().map(|k| k.x.clone()).collect::<Vec<_>>());
    // resume: 15 then 10 on one ledger
    let p2 = tmp("l1_resume.jsonl");
    let led2 = Ledger::new(&p2);
    bayes::run(&branin_spec(15), &m, Some(&led2)).unwrap();
    bayes::run(&branin_spec(10), &m, Some(&led2)).unwrap();
    let resume_eq = evaluation_lines(&p2) == evaluation_lines(&p);
    // L2: lines shuffled
    let text = std::fs::read_to_string(&p).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    Rng::new(698_902).shuffle(&mut lines);
    let p3 = tmp("l2_shuffled.jsonl");
    std::fs::write(&p3, lines.join("\n") + "\n").unwrap();
    let shuffled = Ledger::new(&p3).read().unwrap().entries;
    let next2 = bayes::propose_from_entries(&shuffled, &branin_spec(1), "test.branin").unwrap();
    let l2 = bits(&[next2.picks[0].x.clone()]) == bits(&[next.picks[0].x.clone()]);
    println!("L1 next point bit-equal {l1_next}, first 25 equal {first25}, resume lines equal {resume_eq}; L2 {l2}");
    assert!(prov_ok, "L1 provenance");
    assert!(l1_next && first25, "L1 next");
    assert!(resume_eq, "L1 resume");
    assert!(l2, "L2");
}

#[test]
fn l3_refusals_and_skips() {
    let spec = branin_spec(1);
    let e = |ev: &str, fid: Fidelity, x1: Option<f64>| Entry {
        study: "l1".into(),
        evaluator: ev.into(),
        evaluator_version: "1".into(),
        fidelity: fid,
        verdict: Verdict::Ok,
        params: x1
            .into_iter()
            .map(|v| Quantity::new("x1", v, "1"))
            .chain(std::iter::once(Quantity::new("x2", 1.0, "1")))
            .collect(),
        metrics: vec![Quantity::new("f", 1.0, "1")],
        ..Default::default()
    };
    let tiers = vec![
        e("a", Fidelity::Analytic, Some(0.0)),
        e("a", Fidelity::ReducedOrder, Some(1.0)),
    ];
    let r1 = bayes::observations(&tiers, &[0, 1], &spec);
    let evs = vec![
        e("a", Fidelity::Analytic, Some(0.0)),
        e("b", Fidelity::Analytic, Some(1.0)),
    ];
    let r2 = bayes::observations(&evs, &[0, 1], &spec);
    let miss = vec![e("a", Fidelity::Analytic, Some(0.0)), e("a", Fidelity::Analytic, None)];
    let r3 = bayes::observations(&miss, &[0, 1], &spec).unwrap();
    println!("L3 tiers: {r1:?}");
    println!("L3 evaluators: {r2:?}");
    println!(
        "L3 missing parameter: skipped {}, rows {}",
        r3.skipped_missing_param,
        r3.obs.len()
    );
    assert!(r1.unwrap_err().contains("mixed levels"), "L3 tiers");
    assert!(r2.unwrap_err().contains("mixed evaluators"), "L3 evaluators");
    assert_eq!((r3.skipped_missing_param, r3.obs.len()), (1, 1), "L3 skip");
    let _ = ledger::FORMAT;
}
