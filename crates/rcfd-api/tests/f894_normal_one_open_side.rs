//! ADR 0792 (F-894): a normal distribution open on one side. Rows B1 (`erfc` at the infinities), A1 (the closed form of the
//! truncated inverse CDF, against `f894_reference.tsv`, made by `f894_reference.py` in 760-digit arithmetic), A2 (every
//! sample within the finite bound), A3 (the control of A1: the plain form misses its limit), D1 (the samples of `uq::run`
//! and `sensitivity::run`), C1 and C2 (the refusals).

use rcfd_api::design::ledger::{Fidelity, FnModel, Ledger, Model, Outcome, Quantity};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::uq::{self, norm_cdf, norm_inv, Dist, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_reactor::mixfrac::{erf, erfc};
use serde_json::Value;
use std::path::PathBuf;

const INF: f64 = f64::INFINITY;
const REFERENCE: &str = include_str!("f894_reference.tsv");

/// A line of the reference table.
#[derive(Debug, Clone)]
struct Row {
    kind: String,
    mean: f64,
    sd: f64,
    lo: f64,
    hi: f64,
    index: usize,
    u: f64,
    x: f64,
}

impl Row {
    fn dist(&self) -> Dist {
        Dist::Normal {
            mean: self.mean,
            sd: self.sd,
            lo: self.lo,
            hi: self.hi,
        }
    }

    /// The registered limit of the error: `1e-12 sd + 4 v (|x_ref| + |mean|)`, `v = 2^-53`.
    fn limit(&self) -> f64 {
        1e-12 * self.sd + 2.0 * f64::EPSILON * (self.x.abs() + self.mean.abs())
    }
}

fn rows(kind: &str) -> Vec<Row> {
    let out: Vec<Row> = REFERENCE
        .lines()
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            let f = |i: usize| c[i].parse::<f64>().unwrap();
            Row {
                kind: c[0].into(),
                mean: f(1),
                sd: f(2),
                lo: f(3),
                hi: f(4),
                index: c[5].parse().unwrap(),
                u: f(6),
                x: f(7),
            }
        })
        .filter(|r| r.kind == kind)
        .collect();
    assert!(!out.is_empty(), "{kind}");
    out
}

fn normal(mean: f64, sd: f64, lo: f64, hi: f64) -> Dist {
    Dist::Normal { mean, sd, lo, hi }
}

/// B1: `erfc`, `erf` and `norm_cdf` at the infinities, by their bits.
#[test]
fn b1_erfc_at_the_infinities() {
    for (what, got, want) in [
        ("erfc(+inf)", erfc(INF), 0.0f64),
        ("erfc(-inf)", erfc(-INF), 2.0),
        ("erf(+inf)", erf(INF), 1.0),
        ("erf(-inf)", erf(-INF), -1.0),
        ("norm_cdf(+inf)", norm_cdf(INF), 1.0),
        ("norm_cdf(-inf)", norm_cdf(-INF), 0.0),
    ] {
        assert_eq!(got.to_bits(), want.to_bits(), "B1 {what}: {got}, not {want}");
    }
    assert!(erfc(f64::NAN).is_nan());
}

/// A1: the inverse CDF on the registered grid.
#[test]
fn a1_the_closed_form_of_the_truncated_inverse_cdf() {
    let rows = rows("grid");
    assert_eq!(rows.len(), 884);
    let (mut worst, mut where_) = (0.0f64, String::new());
    let mut worst_z = 0.0f64;
    let mut misses = Vec::new();
    for r in &rows {
        assert_eq!(r.dist().validate(), Ok(()), "A1 {r:?}");
        let x = r.dist().inv_cdf(r.u);
        let e = (x - r.x).abs();
        if e / r.limit() > worst {
            (worst, where_) = (e / r.limit(), format!("{r:?}: {x:e}"));
        }
        if r.mean == 0.0 {
            worst_z = worst_z.max(e);
        }
        // not-a-number is a miss
        if e.is_nan() || e > r.limit() {
            misses.push(format!("{r:?}: {x:e}, error {e:e}, limit {:e}", r.limit()));
        }
    }
    eprintln!(
        "A1: 884 points, worst error {worst:.3} of the limit ({where_}); parent (0, 1): worst {worst_z:.3e} (1e-12)"
    );
    assert!(
        misses.is_empty(),
        "A1: {} misses: {:#?}",
        misses.len(),
        &misses[..misses.len().min(5)]
    );
}

/// A2: every sample lies within the finite bound.
#[test]
fn a2_every_sample_is_within_the_finite_bound() {
    let mut rng = Rng::new(792_002);
    let mut n = 0;
    for r in rows("grid") {
        let d = r.dist();
        let mut us = vec![r.u];
        if r.index == 0 {
            us.extend((0..200).map(|_| rng.uniform()));
        }
        for u in us {
            let x = d.inv_cdf(u);
            assert!(x.is_finite() && x >= r.lo && x <= r.hi, "A2 {d:?} at {u:e}: {x:e}");
            n += 1;
        }
    }
    eprintln!("A2: {n} samples within their finite bound");
    assert_eq!(n, 884 + 68 * 200);
}

/// The plain form of the inverse CDF, which loses the upper tail.
fn plain(a: f64, b: f64, u: f64) -> f64 {
    let open = |u: f64| u.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON / 2.0);
    let (fa, fb) = (
        if a == -INF { 0.0 } else { norm_cdf(a) },
        if b == INF { 1.0 } else { norm_cdf(b) },
    );
    norm_inv(open(fa + open(u) * (fb - fa))).clamp(a, b)
}

/// A3: the control of A1.
#[test]
fn a3_the_plain_form_misses_the_limit() {
    for (lo, hi) in [(7.0, INF), (7.034483685, INF), (-INF, 8.0), (-INF, 30.0)] {
        let of: Vec<Row> = rows("grid")
            .into_iter()
            .filter(|r| r.mean == 0.0 && r.lo == lo && r.hi == hi)
            .collect();
        assert_eq!(of.len(), 13, "A3 [{lo}, {hi}]");
        let worst = of
            .iter()
            .map(|r| (plain(lo, hi, r.u) - r.x).abs())
            .fold(0.0f64, f64::max);
        eprintln!("A3 [{lo}, {hi}]: the plain form is off by up to {worst:.3e} (at least 1e-9)");
        assert!(worst >= 1e-9, "A3 [{lo}, {hi}]: {worst:e}");
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-f894-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn identity() -> impl Model {
    FnModel {
        id: "identity".into(),
        version: "1".into(),
        fidelity: Fidelity::Analytic,
        f: |p: &[Quantity]| Outcome::ok(vec![Quantity::new("y", p[0].value, "1")]),
    }
}

fn params(dist: Dist) -> Vec<UncertainParam> {
    vec![UncertainParam {
        name: "x".into(),
        unit: "1".into(),
        dist,
    }]
}

fn uq_spec(dist: Dist, sampler: Sampler) -> UqSpec {
    UqSpec {
        name: "d1".into(),
        params: params(dist),
        sampler,
        seed: 792_001,
        stop: StopRule {
            batch: 5,
            min_batches: 2,
            max_samples: 10,
            se_abs: 0.0,
            se_rel: 0.0,
            metrics: Vec::new(),
        },
        quantiles: vec![0.5],
        confidence: 0.95,
        requirements: Vec::new(),
        workers: 1,
    }
}

fn sobol_spec(dist: Dist) -> SobolSpec {
    SobolSpec {
        name: "d1".into(),
        params: params(dist),
        n: 3,
        seed: 792_001,
        workers: 1,
    }
}

/// The samples in the ledger of a study.
fn samples(tag: &str, f: impl FnOnce(&Ledger)) -> Vec<f64> {
    let dir = scratch(tag);
    let l = Ledger::new(&dir.join("ledger.jsonl"));
    f(&l);
    let text = std::fs::read_to_string(l.path()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    text.lines()
        .map(|t| serde_json::from_str::<Value>(t).unwrap())
        .filter(|e| e["kind"] == "evaluation")
        .map(|e| e["params"][0]["value"].as_f64().unwrap())
        .collect()
}

/// D1: the samples of both drivers against the reference, which makes its own draws.
#[test]
fn d1_the_samples_of_both_drivers() {
    let (mut n, mut worst) = (0, 0.0f64);
    for kind in ["uq-mc", "uq-lhs", "sobol"] {
        let table = rows(kind);
        for (c, of) in table.chunk_by(|a, b| a.index < b.index).enumerate() {
            let d = of[0].dist();
            let got = samples(&format!("d1-{kind}-{c}"), |l| match kind {
                "uq-mc" => drop(uq::run(&uq_spec(d.clone(), Sampler::MonteCarlo), &identity(), Some(l)).unwrap()),
                "uq-lhs" => drop(uq::run(&uq_spec(d.clone(), Sampler::Lhs), &identity(), Some(l)).unwrap()),
                _ => drop(sensitivity::run(&sobol_spec(d.clone()), &identity(), Some(l)).unwrap()),
            });
            assert_eq!(got.len(), of.len(), "D1 {kind} {d:?}");
            for (x, r) in got.iter().zip(of) {
                assert!(*x >= r.lo && *x <= r.hi, "D1 {r:?}: {x:e}");
                let e = (x - r.x).abs();
                assert!(e <= r.limit(), "D1 {r:?}: {x:e}, error {e:e}, limit {:e}", r.limit());
                worst = worst.max(e / r.limit());
                n += 1;
            }
        }
    }
    eprintln!("D1: {n} samples, worst error {worst:.3} of the limit");
    assert_eq!(n, 4 * 29);
}

/// C1: the smallest kept mass.
#[test]
fn c1_the_smallest_kept_mass() {
    let (inside, outside) = (7.034483685, 7.034483965);
    assert_eq!(normal(0.0, 1.0, inside, INF).validate(), Ok(()));
    assert_eq!(normal(0.0, 1.0, -INF, -inside).validate(), Ok(()));
    for b in [outside, 50.0] {
        for d in [normal(0.0, 1.0, b, INF), normal(0.0, 1.0, -INF, -b)] {
            let e = d.validate().unwrap_err();
            eprintln!("C1: {e}");
            assert!(e.contains("keeps a probability"), "C1 {d:?}: {e}");
        }
    }
}

/// C2: both drivers refuse by name and write nothing.
#[test]
fn c2_both_drivers_refuse_and_write_nothing() {
    let dir = scratch("c2");
    let l = Ledger::new(&dir.join("ledger.jsonl"));
    for (lo, hi) in [
        (7.034483965, INF),
        (-INF, -7.034483965),
        (50.0, INF),
        (f64::NAN, INF),
        (-INF, f64::NAN),
        (f64::NAN, 1.0),
    ] {
        let d = normal(0.0, 1.0, lo, hi);
        let a = uq::run(&uq_spec(d.clone(), Sampler::MonteCarlo), &identity(), Some(&l)).unwrap_err();
        let b = sensitivity::run(&sobol_spec(d.clone()), &identity(), Some(&l)).unwrap_err();
        for e in [a, b] {
            assert!(e.starts_with("parameter 'x': truncation"), "C2 {d:?}: {e}");
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "C2 {d:?}: a file");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
