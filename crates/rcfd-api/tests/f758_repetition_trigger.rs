//! ADR 0680 (F-758): the repetition trigger of the basin restart (`Restart::trigger = Trigger::Repetition`): the restart
//! fires when the plain pick of a step and the last `d + 1` evaluated observations lie in the incumbent's basin, instead of
//! when the acquisition is small. The heavy records (K1 to K5 on Hartmann 6 and Shekel 10) are `#[ignore]`d and run once; the
//! fast tests (D0, H1, HR, NCR, T1, T2) run the same code paths. Test functions and the loop are copied from
//! `tests/f715_robust_ei.rs` and `tests/f733_restart_second_function.rs`, which are not edited.

use rcfd_api::design::bayes::{self, BayesOptions, Observation, Pick, PickKind, Restart, Status, Trigger};
use rcfd_api::design::optim::{Fitness, Stop};
use rcfd_api::design::study::parallel_map;
use std::f64::consts::PI;
use std::time::Instant;

// ---------------------------------------------------------------- test functions

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
    let al = [1.0, 1.2, 3.0, 3.2];
    -(0..4)
        .map(|i| al[i] * (-(0..6).map(|j| a[i][j] * (x[j] - 1e-4 * p[i][j]).powi(2)).sum::<f64>()).exp())
        .sum::<f64>()
}
const H6_B: [(f64, f64); 6] = [(0.0, 1.0); 6];
/// From memory (Dixon and Szego 1978; docs/reference-data.md), checked at its minimiser by ADR 0621 G0.
const H6_MIN: f64 = -3.32237;

/// Shekel 10 on `[0, 10]^4` (Dixon and Szego 1978, from memory, NOT read; ADR 0673 S0 checks the minimum).
const SH_C: [[f64; 4]; 10] = [
    [4.0, 4.0, 4.0, 4.0],
    [1.0, 1.0, 1.0, 1.0],
    [8.0, 8.0, 8.0, 8.0],
    [6.0, 6.0, 6.0, 6.0],
    [3.0, 7.0, 3.0, 7.0],
    [2.0, 9.0, 2.0, 9.0],
    [5.0, 3.0, 5.0, 3.0],
    [8.0, 1.0, 8.0, 1.0],
    [6.0, 2.0, 6.0, 2.0],
    [7.0, 3.6, 7.0, 3.6],
];
const SH_BETA: [f64; 10] = [0.1, 0.2, 0.2, 0.4, 0.4, 0.6, 0.3, 0.7, 0.5, 0.5];
const SH_B: [(f64, f64); 4] = [(0.0, 10.0); 4];
const SH_MIN: f64 = -10.5364;

fn shekel10(x: &[f64]) -> f64 {
    -(0..10)
        .map(|i| 1.0 / ((0..4).map(|j| (x[j] - SH_C[i][j]).powi(2)).sum::<f64>() + SH_BETA[i]))
        .sum::<f64>()
}

fn branin(x: &[f64]) -> f64 {
    let (b, c, t) = (5.1 / (4.0 * PI * PI), 5.0 / PI, 1.0 / (8.0 * PI));
    (x[1] - b * x[0] * x[0] + c * x[0] - 6.0).powi(2) + 10.0 * (1.0 - t) * x[0].cos() + 10.0
}
const BRANIN_B: [(f64, f64); 2] = [(-5.0, 10.0), (0.0, 15.0)];

fn reached(v: f64, fmin: f64) -> bool {
    v <= fmin + 0.01 * fmin.abs()
}

fn first_hit(ys: &[f64], fmin: f64) -> Option<usize> {
    ys.iter().position(|v| reached(*v, fmin)).map(|i| i + 1)
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------- the variants and the loop

/// The registered rep restart (ADR 0680 D1): the repetition trigger with `W = d + 1`, the fresh process, `rho = 2`.
fn rep() -> Restart {
    Restart {
        trigger: Trigger::Repetition { window: None },
        ..Restart::default()
    }
}

const RHO: f64 = 2.0;

fn opts(seed: u64, n0: usize, restart: Option<Restart>) -> BayesOptions {
    BayesOptions {
        seed,
        n0: Some(n0),
        restart,
        ..Default::default()
    }
}

struct Trace {
    ys: Vec<f64>,
    hist: Vec<Observation>,
    picks: Vec<Pick>,
}

/// The loop of `minimise` through `propose` (batch 1, observations in evaluation order), capped by `budget`.
fn propose_run(f: &(dyn Fn(&[f64]) -> f64 + Sync), b: &[(f64, f64)], o: &BayesOptions, budget: usize) -> Trace {
    let bv = b.to_vec();
    let mut t = Trace {
        ys: Vec::new(),
        hist: Vec::new(),
        picks: Vec::new(),
    };
    while t.hist.len() < budget {
        let p = bayes::propose(&bv, &t.hist, o).unwrap();
        for k in p.picks {
            let y = f(&k.x);
            t.ys.push(y);
            t.hist.push(Observation {
                x: k.x.clone(),
                status: Status::Ok { f: y, c: vec![] },
            });
            t.picks.push(k);
        }
    }
    t
}

fn unit(x: &[f64], b: &[(f64, f64)]) -> Vec<f64> {
    x.iter().zip(b).map(|(v, (lo, hi))| (v - lo) / (hi - lo)).collect()
}

/// E2: points outside the box or within 1e-6 (unit cube) of an earlier point.
fn e2_violations(xs: &[Vec<f64>], b: &[(f64, f64)]) -> usize {
    let u: Vec<Vec<f64>> = xs.iter().map(|x| unit(x, b)).collect();
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

/// The incumbent of a set of `Ok` points (lowest value, the first in lexicographic unit-cube order on ties), unit cube.
fn incumbent_u(pts: &[(Vec<f64>, f64)]) -> Option<Vec<f64>> {
    let mut star: Option<&(Vec<f64>, f64)> = None;
    for p in pts {
        let better = match star {
            None => true,
            Some(s) => {
                p.1 < s.1
                    || (p.1 == s.1
                        && p.0.iter().zip(&s.0).map(|(a, c)| a.total_cmp(c)).find(|o| o.is_ne())
                            == Some(std::cmp::Ordering::Less))
            }
        };
        if better {
            star = Some(p);
        }
    }
    star.map(|s| s.0.clone())
}

fn delta(u: &[f64], star: &[f64], l: &[f64]) -> f64 {
    u.iter()
        .zip(star)
        .zip(l)
        .map(|((a, c), s)| ((a - c) / s).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn xs_of(t: &Trace) -> Vec<Vec<f64>> {
    t.hist.iter().map(|o| o.x.clone()).collect()
}

/// The basin of step `k` (incumbent of `hist[..k]`, unit cube) and whether the last `d + 1` observations lie in it.
fn window_inside(t: &Trace, b: &[(f64, f64)], k: usize, l: &[f64]) -> (Vec<f64>, bool) {
    let pts: Vec<(Vec<f64>, f64)> = t.hist[..k]
        .iter()
        .filter_map(|o| match o.status {
            Status::Ok { f, .. } => Some((unit(&o.x, b), f)),
            _ => None,
        })
        .collect();
    let star = incumbent_u(&pts).expect("an incumbent");
    let w = b.len() + 1;
    let ok = k >= w && t.hist[k - w..k].iter().all(|o| delta(&unit(&o.x, b), &star, l) < RHO);
    (star, ok)
}

/// W1, A6 and the K5 count over one rep run: (restart picks, W1 violations, A6 violations, Ei steps at which the condition
/// held and the restart did not fire).
fn w1_a6(t: &Trace, b: &[(f64, f64)], o: &BayesOptions) -> (usize, usize, usize, usize) {
    let bv = b.to_vec();
    let plain = BayesOptions {
        restart: None,
        ..o.clone()
    };
    let (mut n, mut w1, mut a6, mut held) = (0, 0, 0, 0);
    for (k, p) in t.picks.iter().enumerate() {
        match p.kind {
            PickKind::Restart => {
                n += 1;
                let (star, win) = window_inside(t, b, k, &p.length_scales);
                let pp = bayes::propose(&bv, &t.hist[..k], &plain).unwrap();
                let dp = delta(&unit(&pp.picks[0].x, b), &star, &p.length_scales);
                w1 += usize::from(!win || dp.is_nan() || dp >= RHO);
                let dl = delta(&unit(&p.x, b), &star, &p.length_scales);
                a6 += usize::from(dl.is_nan() || dl < RHO);
            }
            PickKind::Ei => {
                let (star, win) = window_inside(t, b, k, &p.length_scales);
                held += usize::from(win && delta(&unit(&p.x, b), &star, &p.length_scales) < RHO);
            }
            _ => {}
        }
    }
    (n, w1, a6, held)
}

/// One heavy record: plain and rep on `seeds`; returns (plain first hits, rep first hits, restart picks, W1, A6, E2).
#[allow(clippy::type_complexity)]
fn record(
    name: &str,
    f: &(dyn Fn(&[f64]) -> f64 + Sync),
    b: &[(f64, f64)],
    fmin: f64,
    n0: usize,
    budget: usize,
    seeds: &[u64],
) -> (Vec<Option<usize>>, Vec<Option<usize>>, usize, usize, usize, usize) {
    let jobs: Vec<(bool, u64)> = [false, true]
        .iter()
        .flat_map(|&r| seeds.iter().map(move |&s| (r, s)))
        .collect();
    let runs = parallel_map(&jobs, 10, &|&(r, s): &(bool, u64)| {
        let t1 = Instant::now();
        let o = opts(s, n0, r.then(rep));
        let tr = propose_run(f, b, &o, budget);
        let (n, w1, a6, held) = if r { w1_a6(&tr, b, &o) } else { (0, 0, 0, 0) };
        let e2 = e2_violations(&xs_of(&tr), b);
        println!(
            "{name} {} seed {s}: target at {:?}, best {:.5}, restart picks {n}, W1 {w1}, A6 {a6}, held without a fire {held}, E2 {e2}, {:.0} s",
            if r { "rep" } else { "plain" },
            first_hit(&tr.ys, fmin),
            tr.ys.iter().cloned().fold(f64::INFINITY, f64::min),
            t1.elapsed().as_secs_f64()
        );
        (first_hit(&tr.ys, fmin), n, w1, a6, e2)
    });
    let ns = seeds.len();
    let plain: Vec<Option<usize>> = runs[..ns].iter().map(|r| r.0).collect();
    let repv: Vec<Option<usize>> = runs[ns..].iter().map(|r| r.0).collect();
    let nr = runs.iter().map(|r| r.1).sum();
    let w1 = runs.iter().map(|r| r.2).sum();
    let a6 = runs.iter().map(|r| r.3).sum();
    let e2 = runs.iter().map(|r| r.4).sum();
    (plain, repv, nr, w1, a6, e2)
}

fn count(v: &[Option<usize>]) -> usize {
    v.iter().filter(|h| h.is_some()).count()
}

fn seeds_where(a: &[Option<usize>], b: &[Option<usize>], seeds: &[u64]) -> Vec<u64> {
    seeds
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(_, (x, y))| x.is_some() && y.is_none())
        .map(|(s, _)| *s)
        .collect()
}

// ---------------------------------------------------------------- K1 to K5: heavy records

#[test]
#[ignore = "heavy record (ADR 0680 K1, P0, K3 to K5 on Hartmann 6): 20 runs of 200, about 5 to 8 minutes on ten threads"]
fn k1_hartmann6_plain_and_repetition_record() {
    let t = Instant::now();
    let seeds: Vec<u64> = (1..=10).collect();
    let (plain, repv, nr, w1, a6, e2) = record("K1", &hartmann6, &H6_B, H6_MIN, 30, 200, &seeds);
    println!(
        "K1: plain {} of 10 {plain:?}, rep {} of 10 {repv:?}; rescued {:?}, lost {:?}; K3 restart picks {nr}; K4 W1 {w1}, A6 {a6}, E2 {e2}; {:.0} s",
        count(&plain),
        count(&repv),
        seeds_where(&repv, &plain, &seeds),
        seeds_where(&plain, &repv, &seeds),
        t.elapsed().as_secs_f64()
    );
    let p0 = vec![
        Some(48),
        None,
        Some(55),
        Some(41),
        Some(52),
        None,
        Some(54),
        Some(50),
        None,
        None,
    ];
    assert_eq!(plain, p0, "P0 Hartmann");
    assert!(count(&repv) >= count(&plain), "K1");
    assert!(nr >= 1, "K3 Hartmann");
    assert_eq!((w1, a6, e2), (0, 0, 0), "K4 Hartmann");
}

#[test]
#[ignore = "heavy record (ADR 0680 K2, P0, K3 to K5 on Shekel 10): 40 runs of 150, about 3 minutes on ten threads"]
fn k2_shekel10_plain_and_repetition_record() {
    let t = Instant::now();
    let seeds: Vec<u64> = (1..=20).collect();
    let (plain, repv, nr, w1, a6, e2) = record("K2", &shekel10, &SH_B, SH_MIN, 10, 150, &seeds);
    let misses: Vec<u64> = seeds
        .iter()
        .zip(&plain)
        .filter(|(_, h)| h.is_none())
        .map(|(s, _)| *s)
        .collect();
    println!(
        "K2: plain {} of 20 (misses {misses:?}), rep {} of 20; rescued {:?}, lost {:?}; K3 restart picks {nr}; K4 W1 {w1}, A6 {a6}, E2 {e2}; {:.0} s",
        count(&plain),
        count(&repv),
        seeds_where(&repv, &plain, &seeds),
        seeds_where(&plain, &repv, &seeds),
        t.elapsed().as_secs_f64()
    );
    assert_eq!(misses, vec![5, 18], "P0 Shekel");
    assert!(count(&repv) >= count(&plain), "K2");
    assert!(nr >= 1, "K3 Shekel");
    assert_eq!((w1, a6, e2), (0, 0, 0), "K4 Shekel");
}

// ---------------------------------------------------------------- D0, H1, HR, NCR: defaults and the O1 points

/// ADR 0673 Z6: the hash of the default O1 points, computed on 5f501379.
const H1_BASE: u64 = 0xe2cf_1413_2077_4cfc;
/// ADR 0673 addendum 1 (NCH): the same hash with `restart: Some(Restart::default())`.
const HR_BASE: u64 = 0xfcb3_94c6_7ca8_392f;

/// ADR 0673 D3: FNV-1a 64 of the little-endian bits of every point of the O1 runs (Branin, seeds 1 to 20, budget 60).
fn o1_hash(restart: Option<Restart>) -> u64 {
    let mut bytes = Vec::new();
    for s in 1..=20u64 {
        let base = BayesOptions {
            seed: s,
            n0: Some(10),
            ..Default::default()
        };
        let o = match restart {
            Some(r) => BayesOptions {
                restart: Some(r),
                ..base
            },
            None => base,
        };
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            for x in p {
                for v in x {
                    bytes.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            Ok(p.iter().map(|x| Fitness::new(branin(x), 0.0)).collect())
        };
        let r = bayes::minimise(&BRANIN_B, &o, 60, &mut ev).unwrap();
        assert_eq!(r.evaluations, 60);
    }
    assert_eq!(bytes.len(), 20 * 60 * 2 * 8);
    fnv(&bytes)
}

#[test]
fn h1_default_o1_points_hash_unchanged() {
    let h = o1_hash(None);
    println!("H1: {h:016x}, base {H1_BASE:016x}");
    assert_eq!(h, H1_BASE, "H1");
}

#[test]
fn hr_acquisition_restart_hash_unchanged() {
    let d0 = Restart::default().trigger;
    let h = o1_hash(Some(Restart::default()));
    println!("D0: default trigger {d0:?}; HR: {h:016x}, base {HR_BASE:016x}");
    assert_eq!(d0, Trigger::Acquisition, "D0");
    assert_eq!(h, HR_BASE, "HR");
}

#[test]
fn ncr_repetition_restart_hash_differs() {
    let h = o1_hash(Some(rep()));
    println!("NCR: {h:016x}; H1 {H1_BASE:016x}, HR {HR_BASE:016x}");
    assert_ne!(h, H1_BASE, "NCR against H1");
    assert_ne!(h, HR_BASE, "NCR against HR");
}

// ---------------------------------------------------------------- T1, T2: fast paths

#[test]
fn t1_shekel10_repetition_fast_path() {
    let t = Instant::now();
    let o = opts(1, 10, Some(rep()));
    let tr = propose_run(&shekel10, &SH_B, &o, 150);
    let (n, w1, a6, held) = w1_a6(&tr, &SH_B, &o);
    let e2 = e2_violations(&xs_of(&tr), &SH_B);
    println!(
        "T1: restart picks {n}, W1 {w1}, A6 {a6}, held without a fire {held}, E2 {e2}, target at {:?}, best {:.5}, {:.0} s",
        first_hit(&tr.ys, SH_MIN),
        tr.ys.iter().cloned().fold(f64::INFINITY, f64::min),
        t.elapsed().as_secs_f64()
    );
    assert!(n >= 1, "T1 fired");
    assert_eq!((w1, a6, e2), (0, 0, 0), "T1 W1, A6, E2");
}

#[test]
fn t2_repetition_workers_bit_equal() {
    let run = |workers: usize| -> Vec<Vec<f64>> {
        let mut xs = Vec::new();
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            xs.extend(p.iter().cloned());
            Ok(p.iter().map(|x| Fitness::new(branin(x), 0.0)).collect())
        };
        let o = BayesOptions {
            workers,
            ..opts(2, 10, Some(rep()))
        };
        bayes::minimise(&BRANIN_B, &o, 60, &mut ev).unwrap();
        xs
    };
    let eq = bits(&run(1)) == bits(&run(8));
    println!("T2: workers 1 and 8 bit-equal {eq}");
    assert!(eq, "T2");
}
