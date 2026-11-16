//! ADR 0651 (F-715): the basin restart of `design::bayes` (opt-in `BayesOptions::restart`) on Hartmann 6, the checks that
//! F-698's results are kept, and the imputation-sign check with the failure classifier off (ADR 0621 mutation M6). The heavy
//! records (A1, A2b) are `#[ignore]`d and run once; the fast tests (A3 to A8) run the same code path.

use rcfd_api::design::bayes::{
    self, BayesOptions, BayesParam, BayesSpec, Observation, Pick, PickKind, Restart, Status,
};
use rcfd_api::design::ledger::{Fidelity, FnModel, Outcome, Quantity};
use rcfd_api::design::optim::{Fitness, Stop};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::study::{parallel_map, Sense};
use std::f64::consts::PI;
use std::time::Instant;

// ---------------------------------------------------------------- set-ups (as ADR 0621 amendment 1)

fn branin(x: &[f64]) -> f64 {
    let (b, c, t) = (5.1 / (4.0 * PI * PI), 5.0 / PI, 1.0 / (8.0 * PI));
    (x[1] - b * x[0] * x[0] + c * x[0] - 6.0).powi(2) + 10.0 * (1.0 - t) * x[0].cos() + 10.0
}
const BRANIN_B: [(f64, f64); 2] = [(-5.0, 10.0), (0.0, 15.0)];
const BRANIN_MIN: f64 = 0.397887;

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
/// From memory (Dixon and Szego 1978; docs/reference-data.md), checked at its minimiser by ADR 0621 G0.
const H6_MIN: f64 = -3.32237;

fn reached(v: f64, fmin: f64) -> bool {
    v <= fmin + 0.01 * fmin.abs()
}

fn first_hit(ys: &[f64], fmin: f64) -> Option<usize> {
    ys.iter().position(|v| reached(*v, fmin)).map(|i| i + 1)
}

fn in_hole(x: &[f64]) -> bool {
    let u = [(x[0] + 5.0) / 15.0, x[1] / 15.0];
    let c = [(PI + 5.0) / 15.0, 2.275 / 15.0];
    ((u[0] - c[0]).powi(2) + (u[1] - c[1]).powi(2)).sqrt() < 0.15
}

fn holed(x: &[f64]) -> Option<f64> {
    if in_hole(x) {
        None
    } else {
        Some(branin(x))
    }
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

fn opts(seed: u64, n0: usize, restart: bool) -> BayesOptions {
    BayesOptions {
        seed,
        n0: Some(n0),
        restart: restart.then(Restart::default),
        ..Default::default()
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

/// The same loop through `propose`, keeping the observations before each pick and the pick records.
struct Trace {
    ys: Vec<f64>,
    hist: Vec<Observation>,
    picks: Vec<Pick>,
}

fn propose_run(f: &(dyn Fn(&[f64]) -> Option<f64> + Sync), b: &[(f64, f64)], o: &BayesOptions, budget: usize) -> Trace {
    let bv = b.to_vec();
    let mut t = Trace {
        ys: Vec::new(),
        hist: Vec::new(),
        picks: Vec::new(),
    };
    while t.hist.len() < budget {
        let p = bayes::propose(&bv, &t.hist, o).unwrap();
        for k in p.picks {
            let v = f(&k.x);
            t.ys.push(v.unwrap_or(f64::INFINITY));
            let status = match v {
                Some(y) => Status::Ok { f: y, c: vec![] },
                None => Status::Failed,
            };
            t.hist.push(Observation { x: k.x.clone(), status });
            t.picks.push(k);
        }
    }
    t
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

fn unit(x: &[f64], b: &[(f64, f64)]) -> Vec<f64> {
    x.iter().zip(b).map(|(v, (lo, hi))| (v - lo) / (hi - lo)).collect()
}

fn bits(xs: &[Vec<f64>]) -> Vec<Vec<u64>> {
    xs.iter().map(|x| x.iter().map(|v| v.to_bits()).collect()).collect()
}

/// A6: restart picks of a trace with `delta < rho` from the incumbent point of their step (lowest `Ok` value before the pick,
/// the first in canonical (lexicographic unit-cube) order on ties), measured in the recorded length scales.
fn a6_violations(t: &Trace, b: &[(f64, f64)], rho: f64) -> (usize, usize) {
    let (mut n, mut bad) = (0, 0);
    for (k, p) in t.picks.iter().enumerate() {
        if p.kind != PickKind::Restart {
            continue;
        }
        n += 1;
        let mut star: Option<(f64, Vec<f64>)> = None;
        for o in &t.hist[..k] {
            if let Status::Ok { f, .. } = o.status {
                let u = unit(&o.x, b);
                let better = match &star {
                    None => true,
                    Some((fs, us)) => {
                        f < *fs
                            || (f == *fs
                                && u.iter().zip(us).map(|(a, c)| a.total_cmp(c)).find(|o| o.is_ne())
                                    == Some(std::cmp::Ordering::Less))
                    }
                };
                if better {
                    star = Some((f, u));
                }
            }
        }
        let us = star.expect("an incumbent").1;
        let u = unit(&p.x, b);
        let delta = u
            .iter()
            .zip(&us)
            .zip(&p.length_scales)
            .map(|((a, c), l)| ((a - c) / l).powi(2))
            .sum::<f64>()
            .sqrt();
        bad += usize::from(delta.is_nan() || delta < rho);
    }
    (n, bad)
}

// ---------------------------------------------------------------- A1, A2b: heavy records

#[test]
#[ignore = "heavy record (ADR 0651 A1): Hartmann 6 with the basin restart, ten seeds of 200, about 10 minutes"]
fn a1_hartmann6_with_the_basin_restart_record() {
    let t = Instant::now();
    let b = [(0.0, 1.0); 6];
    let seeds: Vec<u64> = (1..=10).collect();
    let runs = parallel_map(&seeds, 10, &|&s: &u64| {
        propose_run(&|x| Some(hartmann6(x)), &b, &opts(s, 30, true), 200)
    });
    let (mut hits, mut e2) = (0, 0);
    for (s, tr) in seeds.iter().zip(&runs) {
        let h = first_hit(&tr.ys, H6_MIN);
        let best = tr.ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let nr = tr.picks.iter().filter(|p| p.kind == PickKind::Restart).count();
        let xs: Vec<Vec<f64>> = tr.hist.iter().map(|o| o.x.clone()).collect();
        e2 += e2_violations(&xs, &b);
        hits += usize::from(h.is_some());
        println!("A1 seed {s}: target at {h:?}, best {best:.5}, restart picks {nr}");
    }
    println!("A1: {hits} of 10; E2 {e2}; {:.0} s", t.elapsed().as_secs_f64());
    assert_eq!(e2, 0, "E2");
    assert!(hits >= 9, "A1 count");
}

#[test]
#[ignore = "heavy record (ADR 0651 A2b): the F-698 O2 set-up on the F-715 code, about 3 minutes"]
fn a2b_f698_o2_hit_list_unchanged_record() {
    let t = Instant::now();
    let b = [(0.0, 1.0); 6];
    let seeds: Vec<u64> = (1..=10).collect();
    let runs = parallel_map(&seeds, 10, &|&s: &u64| {
        gp_run(&|x| Some(hartmann6(x)), &b, &opts(s, 30, false), 200)
    });
    let hits: Vec<Option<usize>> = runs.iter().map(|(_, ys)| first_hit(ys, H6_MIN)).collect();
    println!("A2b: first hits {hits:?}; {:.0} s", t.elapsed().as_secs_f64());
    let recorded = [
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
    assert_eq!(hits, recorded, "A2b");
}

// ---------------------------------------------------------------- A3, NC3: the imputation sign with the classifier off

#[test]
fn a3_nc3_imputation_sign_with_the_classifier_off() {
    let seeds: Vec<u64> = (1..=20).collect();
    let mut pooled = [0.0f64; 2];
    let mut hits = [0usize; 2];
    let mut e2 = 0;
    for (k, impute) in [true, false].into_iter().enumerate() {
        let runs = parallel_map(&seeds, 5, &|&s: &u64| {
            let o = BayesOptions {
                impute,
                classify_failures: false,
                ..opts(s, 10, false)
            };
            gp_run(&holed, &BRANIN_B, &o, 60)
        });
        let mut failed = 0;
        for (s, (xs, ys)) in seeds.iter().zip(&runs) {
            let fl = ys[10..].iter().filter(|v| v.is_infinite()).count();
            failed += fl;
            let h = first_hit(ys, BRANIN_MIN);
            hits[k] += usize::from(h.is_some());
            e2 += e2_violations(xs, &BRANIN_B);
            println!(
                "{} seed {s}: target at {h:?}, failed after the initial design {fl} of 50",
                if impute { "A3" } else { "NC3" }
            );
        }
        pooled[k] = failed as f64 / (50.0 * 20.0);
    }
    println!(
        "A3: pooled failed share {:.3}, {} of 20 at the target; NC3: pooled {:.3}, {} of 20; E2 {e2}",
        pooled[0], hits[0], pooled[1], hits[1]
    );
    assert!(pooled[0] <= 0.30, "A3 share");
    assert!(hits[0] >= 14, "A3 count");
    assert!(pooled[1] > 0.30, "NC3");
    assert_eq!(e2, 0, "E2");
}

// ---------------------------------------------------------------- A4 to A7: no harm, the restart picks, determinism

#[test]
fn a4_a6_a7_branin_with_the_restart() {
    let seeds: Vec<u64> = (1..=20).collect();
    let runs = parallel_map(&seeds, 5, &|&s: &u64| {
        propose_run(&|x| Some(branin(x)), &BRANIN_B, &opts(s, 10, true), 60)
    });
    let (mut hits, mut e2, mut nr, mut bad) = (0, 0, 0, 0);
    let mut evals = Vec::new();
    for (s, tr) in seeds.iter().zip(&runs) {
        let h = first_hit(&tr.ys, BRANIN_MIN);
        hits += usize::from(h.is_some());
        evals.push(h.map_or(61.0, |v| v as f64));
        let xs: Vec<Vec<f64>> = tr.hist.iter().map(|o| o.x.clone()).collect();
        e2 += e2_violations(&xs, &BRANIN_B);
        let (n, v) = a6_violations(tr, &BRANIN_B, Restart::default().rho);
        nr += n;
        bad += v;
        println!("A4 seed {s}: target at {h:?}, restart picks {n}");
    }
    let med = median(&mut evals);
    // A7: workers, and the observation order
    let w8 = propose_run(
        &|x| Some(branin(x)),
        &BRANIN_B,
        &BayesOptions {
            workers: 8,
            ..opts(1, 10, true)
        },
        60,
    );
    let w1: Vec<Vec<f64>> = runs[0].hist.iter().map(|o| o.x.clone()).collect();
    let w8x: Vec<Vec<f64>> = w8.hist.iter().map(|o| o.x.clone()).collect();
    let d_workers = bits(&w1) == bits(&w8x);
    let o1 = opts(1, 10, true);
    let p0 = bayes::propose(BRANIN_B.as_ref(), &runs[0].hist, &o1).unwrap();
    let mut sh = runs[0].hist.clone();
    Rng::new(715_001).shuffle(&mut sh);
    let p1 = bayes::propose(BRANIN_B.as_ref(), &sh, &o1).unwrap();
    let xs0: Vec<Vec<f64>> = p0.picks.iter().map(|p| p.x.clone()).collect();
    let xs1: Vec<Vec<f64>> = p1.picks.iter().map(|p| p.x.clone()).collect();
    let d_order = bits(&xs0) == bits(&xs1);
    println!(
        "A4: {hits} of 20, median {med}; A6: {nr} restart picks, {bad} inside the basin; E2 {e2}; A7 workers {d_workers}, order {d_order} (next pick kind {:?})",
        p0.picks[0].kind
    );
    assert!(hits >= 18, "A4 count");
    assert!(med <= 35.0, "A4 median");
    assert_eq!(e2, 0, "E2");
    assert_eq!(bad, 0, "A6 basin");
    assert!(nr >= 1, "A6 fired");
    assert!(d_workers, "A7 workers");
    assert!(d_order, "A7 order");
}

#[test]
fn a5_a6_failure_disc_with_the_restart() {
    let seeds: Vec<u64> = (1..=20).collect();
    let runs = parallel_map(&seeds, 5, &|&s: &u64| {
        propose_run(&holed, &BRANIN_B, &opts(s, 10, true), 60)
    });
    let (mut hits, mut e2, mut nr, mut bad) = (0, 0, 0, 0);
    let mut worst = 0.0f64;
    for (s, tr) in seeds.iter().zip(&runs) {
        let h = first_hit(&tr.ys, BRANIN_MIN);
        hits += usize::from(h.is_some());
        let fl = tr.ys[10..].iter().filter(|v| v.is_infinite()).count();
        worst = worst.max(fl as f64 / 50.0);
        let xs: Vec<Vec<f64>> = tr.hist.iter().map(|o| o.x.clone()).collect();
        e2 += e2_violations(&xs, &BRANIN_B);
        let (n, v) = a6_violations(tr, &BRANIN_B, Restart::default().rho);
        nr += n;
        bad += v;
        println!("A5 seed {s}: target at {h:?}, failed {fl} of 50, restart picks {n}");
    }
    println!(
        "A5: {hits} of 20, worst failed share {worst:.3}; A6: {nr} restart picks, {bad} inside the basin; E2 {e2}"
    );
    assert!(hits >= 18, "A5 count");
    assert!(worst <= 0.25, "A5 share");
    assert_eq!(bad, 0, "A6 basin");
    assert_eq!(e2, 0, "E2");
}

// ---------------------------------------------------------------- A8: the ledger entries

#[test]
fn a8_restart_picks_in_the_ledger_provenance() {
    let model = FnModel {
        id: "test.branin".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| Outcome::ok(vec![Quantity::new("f", branin(&[p[0].value, p[1].value]), "1")]),
    };
    let param = |name: &str, lo: f64, hi: f64| BayesParam {
        name: name.into(),
        unit: "1".into(),
        lo,
        hi,
    };
    let spec = BayesSpec {
        study: "a8".into(),
        params: vec![param("x1", -5.0, 10.0), param("x2", 0.0, 15.0)],
        objective: "f".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget: 60,
        options: opts(1, 10, true),
        clock_ms: Some(1_700_000_000_000),
    };
    let r = bayes::run(&spec, &model, None).unwrap();
    let in_picks = r.picks.iter().filter(|p| p.kind == PickKind::Restart).count();
    let in_entries = r
        .entries
        .iter()
        .filter(|e| e.provenance["acquisition"] == "restart")
        .count();
    println!("A8: restart picks {in_picks}, entries with acquisition restart {in_entries}");
    assert_eq!(in_picks, in_entries, "A8");
}

/// ADR 0651 amendment 1: A8 at seed 2, where the restart fires within 60 evaluations.
#[test]
fn a8b_restart_picks_in_the_ledger_provenance_seed2() {
    let model = FnModel {
        id: "test.branin".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| Outcome::ok(vec![Quantity::new("f", branin(&[p[0].value, p[1].value]), "1")]),
    };
    let param = |name: &str, lo: f64, hi: f64| BayesParam {
        name: name.into(),
        unit: "1".into(),
        lo,
        hi,
    };
    let spec = BayesSpec {
        study: "a8b".into(),
        params: vec![param("x1", -5.0, 10.0), param("x2", 0.0, 15.0)],
        objective: "f".into(),
        sense: Sense::Min,
        constraints: vec![],
        budget: 60,
        options: opts(2, 10, true),
        clock_ms: Some(1_700_000_000_000),
    };
    let r = bayes::run(&spec, &model, None).unwrap();
    let in_picks = r.picks.iter().filter(|p| p.kind == PickKind::Restart).count();
    let in_entries = r
        .entries
        .iter()
        .filter(|e| e.provenance["acquisition"] == "restart")
        .count();
    println!("A8b: restart picks {in_picks}, entries with acquisition restart {in_entries}");
    assert_eq!(in_picks, in_entries, "A8b equal");
    assert!(in_picks >= 1, "A8b fired");
}
