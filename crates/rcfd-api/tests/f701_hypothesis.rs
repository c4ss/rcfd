//! ADR 0624 amendment 1: checks of the hypothesis record of the long-horizon loop (F-701). Synthetic evaluators and the
//! reduced-order fan only; no CFD, no person, no language model.

use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition, FAN_ROM_MODEL_VERSION};
use rcfd_api::design::fidelity::{self, Candidates, CorrectedMetric, FidelitySpec, ModelSource, ParamBox};
use rcfd_api::design::hypothesis::*;
use rcfd_api::design::ledger::{
    entry_of, entry_ref, Entry, Fidelity, Filter, FnModel, Ledger, Model, Outcome, Quantity, Verdict,
};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::study::{parallel_map, Sense};
use rcfd_core::hash::fnv64;
use rcfd_mesh::fan::SectionFamily;
use rcfd_post::fan_rom::*;
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CLOCK: u64 = 1_700_000_000_000;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f701_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger_at(p: &Path) -> Ledger {
    let mut l = Ledger::new(p);
    l.clock = || CLOCK;
    l
}

fn entries(l: &Ledger) -> Vec<Entry> {
    let c = l.read().unwrap();
    assert!(c.skipped.is_empty(), "skipped lines: {:?}", c.skipped);
    c.entries
}

/// Deterministic standard normal of a point (ADR 0624 amendment 1, set-ups).
fn noise(world: u64, metric: u64, params: &[Quantity]) -> f64 {
    let mut bytes = Vec::with_capacity(8 * params.len());
    for q in params {
        bytes.extend_from_slice(&q.value.to_bits().to_le_bytes());
    }
    let mut r = Rng::new(fnv64(&bytes) ^ world ^ (metric + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let (u1, u2) = (r.uniform(), r.uniform());
    (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * PI * u2).cos()
}

fn val(params: &[Quantity], name: &str) -> f64 {
    params.iter().find(|q| q.name == name).map_or(0.0, |q| q.value)
}

// ---------------------------------------------------------------- W1

#[derive(Clone)]
struct W1 {
    seed: u64,
    b: [[f64; 4]; 12],
    tier: Fidelity,
    version: String,
}

fn w1(seed: u64, tier: Fidelity) -> W1 {
    let mut rng = Rng::new(seed);
    let mut cells: Vec<usize> = (0..48).collect();
    rng.shuffle(&mut cells);
    let mut b = [[0.0; 4]; 12];
    for (r, &c) in cells.iter().take(16).enumerate() {
        let slope = if r < 4 {
            3.0
        } else if r < 12 {
            1.0
        } else {
            0.5
        };
        let sign = if rng.below(2) == 0 { 1.0 } else { -1.0 };
        b[c / 4][c % 4] = slope * sign;
    }
    W1 {
        seed,
        b,
        tier,
        version: "1".into(),
    }
}

impl Model for W1 {
    fn id(&self) -> &str {
        "synthetic.w1"
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn fidelity(&self) -> Fidelity {
        self.tier
    }
    fn evaluate(&self, params: &[Quantity]) -> Outcome {
        let mut x = [0.0; 12];
        for q in params {
            x[q.name[1..].parse::<usize>().unwrap()] = q.value;
        }
        Outcome::ok(
            (0..4)
                .map(|j| {
                    let y: f64 = (0..12).map(|k| self.b[k][j] * x[k]).sum::<f64>() + noise(self.seed, j as u64, params);
                    Quantity::new(&format!("y{j}"), y, "1")
                })
                .collect(),
        )
    }
}

fn explore_w1(l: &Ledger, m: &W1) {
    let pts = sampling::latin_hypercube(60, 12, m.seed ^ 0xE);
    let es: Vec<Entry> = pts
        .iter()
        .enumerate()
        .map(|(i, u)| {
            let p: Vec<Quantity> = u
                .iter()
                .enumerate()
                .map(|(k, &v)| Quantity::new(&format!("x{k}"), v, "1"))
                .collect();
            let o = m.evaluate(&p);
            let mut e = entry_of(m, p, o);
            e.kind = "evaluation".into();
            e.study = "explore".into();
            e.variant = format!("e{i}");
            e
        })
        .collect();
    l.append(&es).unwrap();
}

#[derive(Debug, Clone)]
struct Claim {
    k: usize,
    j: usize,
    true_change: f64,
    slope: f64,
    id: String,
    finding: Finding,
    d: Vec<f64>,
}

fn explore_filter() -> Filter {
    Filter {
        kind: Some("evaluation".into()),
        study: Some("explore".into()),
        ..Default::default()
    }
}

fn w1_claim(m: &W1, k: usize, j: usize, s: f64, origin: Origin) -> Hypothesis {
    let mut h = Hypothesis::new(
        &format!("raising x{k} {} y{j}", if s > 0.0 { "raises" } else { "lowers" }),
        Factor {
            name: format!("x{k}"),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: format!("y{j}"),
            unit: "1".into(),
            resolution: 0.0,
        },
        if s > 0.0 {
            Direction::Increases
        } else {
            Direction::Decreases
        },
        0.5,
        (0..12)
            .filter(|&q| q != k)
            .map(|q| Bound::new(&format!("x{q}"), "1", 0.0, 1.0))
            .collect(),
        64,
        m.seed + (4 * k + j) as u64,
        Scope::of(m, false),
    );
    h.origin = origin;
    h
}

/// The protocol agent: explore, the `kmax` strongest cells, registered strongest first, each run right after it.
fn protocol(l: &Ledger, m: &W1, kmax: usize, workers: usize, do_run: bool) -> Vec<Claim> {
    let es = entries(l);
    let refs: Vec<_> = (0..es.len())
        .filter(|&i| es[i].study == "explore")
        .map(|i| entry_ref(i, &es[i]))
        .collect();
    let ex = explore(&es, &explore_filter());
    assert!(ex.exploratory);
    let mut out = Vec::new();
    for c in ex.cells.iter().take(kmax) {
        let k: usize = c.param[1..].parse().unwrap();
        let j: usize = c.metric[1..].parse().unwrap();
        let s = if c.rho > 0.0 { 1.0 } else { -1.0 };
        let h = w1_claim(m, k, j, s, Origin::Exploratory { entries: refs.clone() });
        let r = register(l, &h).unwrap();
        let mut claim = Claim {
            k,
            j,
            true_change: s * m.b[k][j] * 0.5,
            slope: m.b[k][j].abs(),
            id: r.id.clone(),
            finding: Finding::Void,
            d: Vec::new(),
        };
        if do_run {
            let rep = run(l, &r.id, m, &RunOptions { workers, chunk: 8 }).unwrap();
            assert!(rep.verification.problems.is_empty(), "{:?}", rep.verification.problems);
            let res = rep.verification.result.expect("result");
            claim.finding = res.finding;
            claim.d = res.d.iter().flatten().copied().collect();
        }
        out.push(claim);
    }
    out
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = x.iter().map(|a| (a - mx) * (a - mx)).sum();
    let syy: f64 = y.iter().map(|b| (b - my) * (b - my)).sum();
    sxy / (sxx * syy).sqrt()
}

#[derive(Debug, Default, Clone, Copy)]
struct WorldStats {
    false_series: bool,
    false_const: bool,
    false_naive: bool,
    large_reg: usize,
    large_conf: usize,
    zero_conf: usize,
    level_sum: f64,
    results: usize,
    claims: usize,
}

fn world_run(w: u64, dir: &Path) -> WorldStats {
    let seed = 701_000 + w;
    let m = w1(seed, Fidelity::Analytic);
    let path = dir.join(format!("w{w}.jsonl"));
    let _ = std::fs::remove_file(&path);
    let l = ledger_at(&path);
    explore_w1(&l, &m);
    let claims = protocol(&l, &m, 12, 1, true);
    let es = entries(&l);
    let mut st = WorldStats {
        claims: claims.len(),
        ..Default::default()
    };
    st.level_sum = registrations(&es).iter().map(|r| r.level).sum();
    st.results = es.iter().filter(|e| e.kind == KIND_RESULT).count();
    // naive agent on the same cells: Pearson r over the 60 exploratory points
    let ex: Vec<&Entry> = es.iter().filter(|e| e.study == "explore").collect();
    let t58 = student_t_quantile(0.95, 58.0);
    for c in &claims {
        let x: Vec<f64> = ex
            .iter()
            .map(|e| e.param(&format!("x{}", c.k)).unwrap().value)
            .collect();
        let y: Vec<f64> = ex
            .iter()
            .map(|e| e.metric(&format!("y{}", c.j)).unwrap().value)
            .collect();
        let r = pearson(&x, &y);
        let t = r.abs() * (58.0 / (1.0 - r * r)).sqrt();
        let true_naive = r.signum() * m.b[c.k][c.j] * 0.5;
        if t >= t58 && true_naive <= 0.5 + 1e-12 {
            st.false_naive = true;
        }
        let false_cell = c.true_change <= 0.5 + 1e-12;
        if false_cell && c.finding == Finding::Confirmed {
            st.false_series = true;
        }
        let (fc, _) = paired_test(&c.d, 0.05, 0.5, Direction::Increases, Statistic::PairedT);
        if false_cell && fc == Finding::Confirmed {
            st.false_const = true;
        }
        if c.slope == 3.0 {
            st.large_reg += 1;
            st.large_conf += usize::from(c.finding == Finding::Confirmed);
        }
        if c.slope == 0.0 && c.finding == Finding::Confirmed {
            st.zero_conf += 1;
        }
    }
    let _ = std::fs::remove_file(&path);
    st
}

fn worlds(ws: &[u64], tag: &str) -> Vec<WorldStats> {
    let dir = tmp(tag);
    let out = parallel_map(ws, 4, &|&w| world_run(w, &dir));
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// S1, S2, S3 (per world), S5, NC1, NC2: the registered 400 worlds (heavy record).
#[test]
#[ignore]
fn s1_s2_s5_nc1_nc2_worlds_heavy() {
    let t0 = std::time::Instant::now();
    let ws: Vec<u64> = (0..400).collect();
    let st = worlds(&ws, "s1");
    let n_false = st.iter().filter(|s| s.false_series).count();
    let n_const = st.iter().filter(|s| s.false_const).count();
    let n_naive = st.iter().filter(|s| s.false_naive).count();
    let lreg: usize = st.iter().map(|s| s.large_reg).sum();
    let lconf: usize = st.iter().map(|s| s.large_conf).sum();
    let zconf: usize = st.iter().map(|s| s.zero_conf).sum();
    let max_sum = st.iter().map(|s| s.level_sum).fold(0.0, f64::max);
    eprintln!(
        "S1 false-confirmation worlds {n_false} / 400 (limit <= 37); NC2 constant 0.05 {n_const} (limit >= 38); NC1 naive {n_naive} \
         (limit >= 300); S2 large confirmed {lconf} / {lreg} = {:.4} (limit >= 0.95); S5 zero cells confirmed {zconf} (limit 0); \
         S3 largest level sum {max_sum:.6} (limit <= 0.05); results {}; {:.1} s",
        lconf as f64 / lreg as f64,
        st.iter().map(|s| s.results).sum::<usize>(),
        t0.elapsed().as_secs_f64()
    );
    assert!(st.iter().all(|s| s.claims == 12 && s.results == 12));
    assert!(n_false <= 37, "S1");
    assert!(lconf as f64 >= 0.95 * lreg as f64, "S2");
    assert!(max_sum <= 0.05, "S3");
    assert_eq!(zconf, 0, "S5");
    assert!(n_naive >= 300, "NC1");
    assert!(n_const >= 38, "NC2");
}

/// Fast counterpart of the heavy record: worlds 0 to 7, structural assertions only.
#[test]
fn s1_worlds_fast() {
    let ws: Vec<u64> = (0..8).collect();
    let st = worlds(&ws, "s1f");
    for s in &st {
        assert_eq!(s.claims, 12);
        assert_eq!(s.results, 12);
        assert!(s.level_sum <= 0.05);
    }
}

// ---------------------------------------------------------------- W1b

struct W1b {
    seed: u64,
}

impl Model for W1b {
    fn id(&self) -> &str {
        "synthetic.w1b"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        Outcome::ok(vec![Quantity::new("y", val(p, "x0") + noise(self.seed, 0, p), "1")])
    }
}

fn w1b_claim(m: &W1b, seed: u64) -> Hypothesis {
    Hypothesis::new(
        "raising x0 raises y by at least 0.5",
        Factor {
            name: "x0".into(),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: "y".into(),
            unit: "1".into(),
            resolution: 0.0,
        },
        Direction::Increases,
        0.5,
        vec![Bound::new("x1", "1", 0.0, 1.0)],
        64,
        seed,
        Scope::of(m, false),
    )
}

/// One fresh ledger, one boundary test: (finding, d).
fn w1b_single(seed: u64, dir: &Path) -> (Finding, Vec<f64>, f64) {
    let path = dir.join(format!("b{seed}.jsonl"));
    let _ = std::fs::remove_file(&path);
    let l = ledger_at(&path);
    let m = W1b { seed };
    let r = register(&l, &w1b_claim(&m, seed)).unwrap();
    let rep = run(&l, &r.id, &m, &RunOptions::default()).unwrap();
    let res = rep.verification.result.unwrap();
    let _ = std::fs::remove_file(&path);
    (res.finding, res.d.iter().flatten().copied().collect(), res.level)
}

fn s4_count(seeds: &[u64], tag: &str) -> (usize, f64) {
    let dir = tmp(tag);
    let out = parallel_map(seeds, 4, &|&s| w1b_single(s, &dir));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.iter().all(|o| o.1.len() == 64));
    (out.iter().filter(|o| o.0 == Finding::Confirmed).count(), out[0].2)
}

/// S4: 4000 single boundary tests at a_1 (heavy record).
#[test]
#[ignore]
fn s4_boundary_size_heavy() {
    let t0 = std::time::Instant::now();
    let seeds: Vec<u64> = (0..4000).map(|r| 702_000 + r).collect();
    let (c, level) = s4_count(&seeds, "s4");
    let e = 4000.0 * level;
    let se = (4000.0 * level * (1.0 - level)).sqrt();
    eprintln!(
        "S4 confirmations {c} of 4000 at level {level:.6} (expected {e:.1}, z {:.2}; limit 79 to 165); {:.1} s",
        (c as f64 - e) / se,
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(level.to_bits(), series_level(1).to_bits());
    assert!((79..=165).contains(&c), "S4");
}

#[test]
fn s4_boundary_fast() {
    let seeds: Vec<u64> = (0..100).map(|r| 702_000 + r).collect();
    let (_c, level) = s4_count(&seeds, "s4f");
    assert_eq!(level.to_bits(), series_level(1).to_bits());
}

/// NC3: the peeking agent stops at the first favourable look (level 0.05, looks after 8..64 pairs) on a boundary effect.
#[test]
fn nc3_peeking_agent() {
    let dir = tmp("nc3");
    let seeds: Vec<u64> = (0..400).map(|r| 703_000 + r).collect();
    let out = parallel_map(&seeds, 4, &|&s| w1b_single(s, &dir));
    let _ = std::fs::remove_dir_all(&dir);
    let mut peek = 0;
    for (_, d, _) in &out {
        assert_eq!(d.len(), 64);
        if (8..=64)
            .any(|k| paired_test(&d[..k], 0.05, 0.5, Direction::Increases, Statistic::PairedT).0 == Finding::Confirmed)
        {
            peek += 1;
        }
    }
    let registered = out.iter().filter(|o| o.0 == Finding::Confirmed).count();
    eprintln!(
        "NC3 peeking false confirmations {peek} of 400 (limit >= 40); the registered tests confirmed {registered}"
    );
    assert!(peek >= 40, "NC3");
}

// ---------------------------------------------------------------- K, L

#[test]
fn k1_t_quantile() {
    let mut worst: f64 = 0.0;
    for &p in &[0.9, 0.95, 0.99, 0.9999, 1.0 - 1e-6] {
        let c1 = (PI * (p - 0.5)).tan();
        let c2 = (2.0 * p - 1.0) / (2.0 * p * (1.0 - p)).sqrt();
        for (got, want) in [(student_t_quantile(p, 1.0), c1), (student_t_quantile(p, 2.0), c2)] {
            let rel = ((got - want) / want).abs();
            worst = worst.max(rel);
            assert!(rel <= 1e-9, "K1 p {p}: {got} vs {want} (rel {rel:.2e})");
        }
    }
    let mut worst_cdf: f64 = 0.0;
    for &p in &[0.95, 0.99, 1.0 - 3e-5] {
        let t = student_t_quantile(p, 63.0);
        let e = (student_t_cdf(t, 63.0) - p).abs();
        worst_cdf = worst_cdf.max(e);
        assert!(e <= 1e-12, "K1 cdf p {p}: {e:.2e}");
    }
    eprintln!("K1 largest relative error {worst:.2e}; cdf error {worst_cdf:.2e}");
}

const D16: [f64; 16] = [
    0.62, 0.41, 0.77, 0.55, 0.38, 0.91, 0.66, 0.47, 0.58, 0.73, 0.35, 0.69, 0.52, 0.81, 0.44, 0.60,
];

fn hand(d: &[f64], level: f64) -> (f64, f64, f64, f64, f64, f64) {
    let n = d.len() as f64;
    let m = d.iter().sum::<f64>() / n;
    let sd = (d.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    let se = sd / n.sqrt();
    let t = student_t_quantile(1.0 - level, n - 1.0);
    (m, sd, se, t, m - t * se, m + t * se)
}

fn binom_sf_hand(k: usize, n: usize) -> f64 {
    let mut c = 1.0f64;
    let mut tot = 0.0;
    for i in 0..=n {
        if i >= k {
            tot += c;
        }
        c = c * (n - i) as f64 / (i + 1) as f64;
    }
    tot / 2f64.powi(n as i32)
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        ((a - b) / b).abs()
    }
}

#[test]
fn k2_paired_test_by_hand() {
    let lv = 0.05;
    let mut worst: f64 = 0.0;
    let mut check = |d: &[f64], min: f64, dir: Direction, want: Finding, opposite: bool| {
        let (f, st) = paired_test(d, lv, min, dir, Statistic::PairedT);
        let (m, sd, se, t, l, u) = hand(d, lv);
        let p = if dir == Direction::NoEffect {
            student_t_sf((m + min) / se, 15.0).max(student_t_sf((min - m) / se, 15.0))
        } else {
            student_t_sf((m - min) / se, 15.0)
        };
        for (a, b) in [
            (st.mean, m),
            (st.sd, sd),
            (st.se, se),
            (st.t, t),
            (st.lower, l),
            (st.upper, u),
            (st.p_value, p),
        ] {
            let r = rel(a, b);
            worst = worst.max(r);
            assert!(r <= 1e-12, "K2: {a} vs {b}");
        }
        assert_eq!(f, want, "K2 finding for min {min}");
        assert_eq!(st.opposite, opposite);
        (m, l)
    };
    let (m, l) = check(&D16, 0.55, Direction::Increases, Finding::Inconclusive, false);
    assert!(m > 0.55 && l < 0.55, "set-up: m {m}, L {l}");
    let up: Vec<f64> = D16.iter().map(|x| x + 0.2).collect();
    check(&up, 0.55, Direction::Increases, Finding::Confirmed, false);
    let dn: Vec<f64> = D16.iter().map(|x| x - 0.6).collect();
    check(&dn, 0.55, Direction::Decreases, Finding::Refuted, false);
    let dn2: Vec<f64> = D16.iter().map(|x| x - 0.8).collect();
    check(&dn2, 0.55, Direction::Increases, Finding::Refuted, true);
    let m0 = D16.iter().sum::<f64>() / 16.0;
    let c: Vec<f64> = D16.iter().map(|x| x - m0).collect();
    check(&c, 0.55, Direction::NoEffect, Finding::Confirmed, false);
    let far: Vec<f64> = D16.iter().map(|x| x + 1.0).collect();
    check(&far, 0.55, Direction::NoEffect, Finding::Refuted, false);
    // sign test
    let (f, st) = paired_test(&D16, lv, 0.55, Direction::Increases, Statistic::Sign);
    let kp = D16.iter().filter(|&&x| x > 0.55).count();
    let r = rel(st.p_value, binom_sf_hand(kp, 16));
    assert!(r <= 1e-12, "K2 sign p {} vs {}", st.p_value, binom_sf_hand(kp, 16));
    assert_eq!(
        f,
        if binom_sf_hand(kp, 16) <= lv {
            Finding::Confirmed
        } else {
            Finding::Inconclusive
        }
    );
    let (f2, st2) = paired_test(&dn2, lv, 0.55, Direction::Increases, Statistic::Sign);
    assert_eq!(f2, Finding::Refuted);
    assert!(st2.opposite);
    assert!(rel(st2.p_value, binom_sf_hand(0, 16)) <= 1e-12);
    eprintln!(
        "K2 largest relative error {worst:.2e}; sign p {:.6e} ({kp} of 16 above)",
        st.p_value
    );
}

#[test]
fn l1_s3_levels_and_reopen() {
    let dir = tmp("l1");
    let path = dir.join("l.jsonl");
    let m = W1b { seed: 701_700 };
    {
        let l = ledger_at(&path);
        for i in 0..3 {
            register(&l, &w1b_claim(&m, 701_700 + i)).unwrap();
        }
    }
    // a new handle (a new session)
    let l = ledger_at(&path);
    let r4 = register(&l, &w1b_claim(&m, 701_703)).unwrap();
    assert_eq!((r4.id.as_str(), r4.slot), ("H-4", 4), "S3 reopen");
    assert_eq!(r4.level.to_bits(), series_level(4).to_bits());
    for i in 4..12 {
        register(&l, &w1b_claim(&m, 701_700 + i)).unwrap();
    }
    let regs = registrations(&entries(&l));
    assert_eq!(regs.len(), 12);
    let mut sum = 0.0;
    for (k, r) in regs.iter().enumerate() {
        let i = (k + 1) as f64;
        let want = 0.05 * 6.0 / (PI * PI * i * i);
        assert_eq!(series_level(k + 1).to_bits(), want.to_bits(), "L1 {k}");
        assert_eq!(r.level.to_bits(), want.to_bits(), "L1 stored {k}");
        assert_eq!(r.slot, k + 1);
        sum += r.level;
    }
    eprintln!("L1/S3: 12 levels as the series, sum {sum:.6}; H-4 slot 4 after a new handle");
    assert!(sum <= 0.05);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- G

#[test]
fn g1_round_trip() {
    let dir = tmp("g1");
    let path = dir.join("l.jsonl");
    let m = W1b { seed: 701_710 };
    let mut h = w1b_claim(&m, 701_710);
    h.predicted_size = Some((0.4, 0.6));
    h.held = vec![Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target: HeldTarget::AsLowArm { at: 0.8 },
        tolerance: 1e-9,
        by: "n".into(),
        by_unit: "1/s".into(),
        bracket: (0.1, 10.0),
    }];
    h.statistic = Statistic::Sign;
    {
        let l = ledger_at(&path);
        register(&l, &h).unwrap();
    }
    let es = entries(&Ledger::new(&path));
    let regs = registrations(&es);
    let back = regs[0].record.clone().unwrap();
    assert_eq!(back, h, "G1 record");
    let want = format!("fnv1a64:{:016x}", fnv64(h.to_json().to_string().as_bytes()));
    assert_eq!(regs[0].hash, want);
    assert_eq!(regs[0].recomputed_hash, want);
    assert_eq!(h.hash(), want);
    eprintln!("G1: record equal, hash {want}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A W1 ledger: exploration, one claim (the strongest cell), run. Returns the file text and the model.
fn w1_one(path: &Path, seed: u64) -> W1 {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let m = w1(seed, Fidelity::Analytic);
    explore_w1(&l, &m);
    protocol(&l, &m, 1, 1, true);
    m
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn write_lines(path: &Path, ls: &[String]) {
    std::fs::write(path, ls.iter().map(|s| format!("{s}\n")).collect::<String>()).unwrap();
}

#[test]
fn g2_order_rules() {
    let dir = tmp("g2");
    let base = dir.join("base.jsonl");
    let m = w1_one(&base, 701_720);
    let ls = lines(&base);
    assert_eq!(ls.len(), 60 + 1 + 128 + 1);
    let v0 = verify(&entries(&ledger_at(&base)), "H-1", None);
    assert!(v0.problems.is_empty() && v0.genuine == Some(189));
    let mut cases: Vec<(&str, Vec<String>, &str)> = Vec::new();
    // (a) one evidence line moved before the registration
    let mut a = ls.clone();
    let ev = a.remove(61);
    a.insert(60, ev);
    cases.push(("a", a, "precedes its registration"));
    // (b1) a 65th pair before the result
    let mut b1 = ls.clone();
    let extra: Vec<String> = ls[187..189]
        .iter()
        .map(|s| s.replace("\"pair\":63", "\"pair\":64").replace("#p63.", "#p64."))
        .collect();
    b1.splice(189..189, extra);
    cases.push(("b1", b1, "outside the registered 64 pairs"));
    // (b2) the last pair removed, the result kept
    let mut b2 = ls.clone();
    b2.drain(187..189);
    cases.push(("b2", b2, "result of H-1 with evidence for 126 of the registered 128"));
    // (c) the registration's min_effect edited
    let mut c = ls.clone();
    assert!(c[60].contains("\"min_effect\":0.5"));
    c[60] = c[60].replace("\"min_effect\":0.5", "\"min_effect\":0.4");
    cases.push(("c", c, "was altered"));
    // (d) one evidence line's evaluator version edited
    let mut d = ls.clone();
    assert!(d[61].contains("\"evaluator_version\":\"1\""));
    d[61] = d[61].replace("\"evaluator_version\":\"1\"", "\"evaluator_version\":\"2\"");
    cases.push(("d", d, "evidence evaluator synthetic.w1 version '2'"));
    let mut refused = 0;
    for (tag, text, want) in &cases {
        let p = dir.join(format!("{tag}.jsonl"));
        write_lines(&p, text);
        let l = ledger_at(&p);
        let v = verify(&l.read().unwrap().entries, "H-1", None);
        let hit = v.problems.iter().find(|x| x.contains(want) && x.contains("entry "));
        let r = run(&l, "H-1", &m, &RunOptions::default());
        let run_refused = r.as_ref().err().is_some_and(|e| e.contains(want));
        eprintln!(
            "G2 ({tag}): {} problems; located: {:?}; run refused: {run_refused}",
            v.problems.len(),
            hit
        );
        if hit.is_some() && run_refused {
            refused += 1;
        }
    }
    assert_eq!(refused, 5, "G2: 5 of 5 cases");
    let _ = std::fs::remove_dir_all(&dir);
}

fn w1_ro_world(path: &Path, seed: u64, k: usize, do_run: bool) -> (W1, Vec<Claim>) {
    let _ = std::fs::remove_file(path);
    let l = ledger_at(path);
    let m = w1(seed, Fidelity::ReducedOrder);
    explore_w1(&l, &m);
    let c = protocol(&l, &m, k, 1, do_run);
    (m, c)
}

fn q_all(cur: &[(&str, &str)]) -> LessonQuery {
    LessonQuery {
        current: cur.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        ..Default::default()
    }
}

#[test]
fn g3_nc5_hand_written_results() {
    let dir = tmp("g3");
    for (tag, do_run) in [("a", true), ("b", false)] {
        let path = dir.join(format!("{tag}.jsonl"));
        let (_m, claims) = w1_ro_world(&path, 701_730, 5, do_run);
        let l = ledger_at(&path);
        let es = entries(&l);
        let n0 = es.len();
        let forged: Vec<Entry> = claims
            .iter()
            .map(|c| {
                let genuine = es.iter().find(|e| e.kind == KIND_RESULT && e.variant == c.id);
                match genuine {
                    Some(g) => {
                        let mut f = g.clone();
                        f.settings["verdict"] = json!("confirmed");
                        f.recorded_at_ms = 0;
                        f
                    }
                    None => Entry {
                        kind: KIND_RESULT.into(),
                        study: "hypothesis".into(),
                        variant: c.id.clone(),
                        evaluator: DRIVER_ID.into(),
                        evaluator_version: DRIVER_VERSION.into(),
                        fidelity: Fidelity::ReducedOrder,
                        verdict: Verdict::Ok,
                        metrics: vec![Quantity::new("mean", 1.0, "1"), Quantity::new("lower", 0.9, "1")],
                        settings: json!({"verdict": "confirmed"}),
                        provenance: json!({"driver": "hypothesis", "hypothesis": c.id}),
                        ..Default::default()
                    },
                }
            })
            .collect();
        l.append(&forged).unwrap();
        let forged_idx: Vec<usize> = (n0..n0 + 5).collect();
        let es = entries(&l);
        let q = q_all(&[("synthetic.w1", "1")]);
        let les = lessons(&es, &q, &[]);
        let returned = les.rows.iter().filter(|r| forged_idx.contains(&r.result)).count();
        let flagged = forged_idx
            .iter()
            .filter(|k| les.flagged.iter().any(|f| f.index == **k))
            .count();
        let mut qt = q.clone();
        qt.trust_stored = true;
        let nc5 = lessons(&es, &qt, &[]);
        let nc5_ret = nc5
            .rows
            .iter()
            .filter(|r| forged_idx.contains(&r.result) && r.finding == Finding::Confirmed)
            .count();
        eprintln!(
            "G3 ({tag}): forged verdicts returned {returned} of 5, flagged {flagged} of 5; rows {}; NC5 trust_stored returns {nc5_ret} of 5",
            les.rows.len()
        );
        assert_eq!(returned, 0, "G3 returned");
        assert_eq!(flagged, 5, "G3 flagged");
        assert_eq!(nc5_ret, 5, "NC5");
        if do_run {
            assert_eq!(les.rows.len(), 5);
        } else {
            assert_eq!(les.rows.len(), 0);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn g4_resume_after_kill() {
    let dir = tmp("g4");
    let full = dir.join("full.jsonl");
    let m = w1_one(&full, 701_740);
    let ls = lines(&full);
    let bytes = std::fs::read(&full).unwrap();
    let result_line = ls.last().unwrap().clone();
    assert!(result_line.contains("hypothesis_result"));
    for (k, torn) in [
        (0usize, false),
        (1, false),
        (63, false),
        (127, false),
        (128, false),
        (63, true),
    ] {
        let p = dir.join(format!("k{k}{}.jsonl", if torn { "t" } else { "" }));
        let mut text: String = ls[..61 + k].iter().map(|s| format!("{s}\n")).collect();
        if torn {
            let next = &ls[61 + k];
            text.push_str(&next[..next.len() / 2]);
        }
        std::fs::write(&p, text).unwrap();
        let l = ledger_at(&p);
        let rep = run(&l, "H-1", &m, &RunOptions::default()).unwrap();
        let c = l.read().unwrap();
        let v = verify(&c.entries, "H-1", None);
        let missing = v.evidence.iter().filter(|x| x.is_none()).count();
        let n_ev = c
            .entries
            .iter()
            .filter(|e| e.kind == "evaluation" && e.study == "hypothesis")
            .count();
        let dup = n_ev - (128 - missing);
        let new_ls = lines(&p);
        let same_result = new_ls.last() == Some(&result_line);
        let same_file = std::fs::read(&p).unwrap() == bytes;
        eprintln!(
            "G4 cut {k}{}: appended {} evidence; duplicates {dup}, missing {missing}; skipped lines {}; result line identical {same_result}; file identical {same_file}",
            if torn { " torn" } else { "" },
            rep.appended_evidence,
            c.skipped.len()
        );
        assert!(v.problems.is_empty(), "{:?}", v.problems);
        assert_eq!((dup, missing), (0, 0));
        assert_eq!(rep.appended_evidence, 128 - k);
        assert!(same_result);
        if !torn {
            assert!(same_file);
        } else {
            assert_eq!(c.skipped.len(), 1);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn g5_impostor_evidence() {
    let dir = tmp("g5");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let real = W1b { seed: 701_750 };
    let imp = FnModel {
        id: "synthetic.w1b".into(),
        version: "1".into(),
        fidelity: Fidelity::ReducedOrder,
        f: |p: &[Quantity]| {
            let r = W1b { seed: 701_750 }.evaluate(p);
            Outcome::ok(
                r.metrics
                    .iter()
                    .map(|q| Quantity::new(&q.name, q.value + 0.3, &q.unit))
                    .collect(),
            )
        },
    };
    let r = register(&l, &w1b_claim(&real, 701_751)).unwrap();
    run(&l, &r.id, &imp, &RunOptions::default()).unwrap();
    let es = entries(&l);
    let v = verify(&es, &r.id, Some(&real));
    let per_entry = v
        .problems
        .iter()
        .filter(|p| p.contains("not reproduced by a re-run"))
        .count();
    let q = q_all(&[("synthetic.w1b", "1")]);
    let with = lessons(&es, &q, &[&real]);
    let without = lessons(&es, &q, &[]);
    eprintln!(
        "G5: {per_entry} re-run problems of 128 evidence entries; lessons with re-run {} rows, without {} rows (evidence_rerun {:?})",
        with.rows.len(),
        without.rows.len(),
        without.rows.first().map(|x| x.evidence_rerun)
    );
    assert_eq!(per_entry, 128);
    assert_eq!(v.evidence_rerun, Some(false));
    assert_eq!(with.rows.len(), 0);
    assert_eq!(without.rows.len(), 1);
    assert_eq!(without.rows[0].evidence_rerun, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn v1_validation_refusals() {
    let dir = tmp("v1");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let m = W1b { seed: 701_760 };
    let base = w1b_claim(&m, 701_760);
    // a supersedes chain H-1 <- H-2 <- H-3 for the re-test cap
    register(&l, &base).unwrap();
    let mut s2 = base.clone();
    s2.supersedes = Some("H-1".into());
    register(&l, &s2).unwrap();
    let mut s3 = base.clone();
    s3.supersedes = Some("H-2".into());
    register(&l, &s3).unwrap();
    let held = Held {
        metric: "q".into(),
        unit: "m3/s".into(),
        target: HeldTarget::Value(1.0),
        tolerance: 1e-9,
        by: "n".into(),
        by_unit: "1/s".into(),
        bracket: (0.1, 10.0),
    };
    type Mutate = Box<dyn Fn(&mut Hypothesis)>;
    let cases: Vec<(&str, Mutate)> = vec![
        ("claim_text", Box::new(|h| h.claim_text = " ".into())),
        ("factor: low", Box::new(|h| h.factor.low = 0.9)),
        ("min_effect", Box::new(|h| h.min_effect = 0.0)),
        ("min_effect", Box::new(|h| h.response.resolution = 0.1)),
        ("response.resolution", Box::new(|h| h.response.resolution = -1.0)),
        ("design.n_pairs", Box::new(|h| h.n_pairs = 15)),
        ("domain 'x1'", Box::new(|h| h.domain[0].hi = 0.0)),
        (
            "domain 'x1': given twice",
            Box::new(|h| h.domain.push(Bound::new("x1", "1", 0.0, 1.0))),
        ),
        (
            "the factor cannot be in the domain",
            Box::new(|h| h.domain.push(Bound::new("x0", "1", 0.0, 1.0))),
        ),
        ("held: 2 held metrics", {
            let x = held.clone();
            Box::new(move |h| h.held = vec![x.clone(), x.clone()])
        }),
        ("the response cannot be held", {
            let x = held.clone();
            Box::new(move |h| {
                let mut y = x.clone();
                y.metric = "y".into();
                y.unit = "1".into();
                h.held = vec![y];
            })
        }),
        ("compensating parameter 'x1'", {
            let x = held.clone();
            Box::new(move |h| {
                let mut y = x.clone();
                y.by = "x1".into();
                h.held = vec![y];
            })
        }),
        ("tolerance", {
            let x = held.clone();
            Box::new(move |h| {
                let mut y = x.clone();
                y.tolerance = 0.0;
                h.held = vec![y];
            })
        }),
        ("bracket", {
            let x = held.clone();
            Box::new(move |h| {
                let mut y = x.clone();
                y.bracket = (2.0, 1.0);
                h.held = vec![y];
            })
        }),
        ("as_low_arm at 20", {
            let x = held.clone();
            Box::new(move |h| {
                let mut y = x.clone();
                y.target = HeldTarget::AsLowArm { at: 20.0 };
                h.held = vec![y];
            })
        }),
        ("test.max_failed_share", Box::new(|h| h.max_failed_share = 1.0)),
        (
            "origin",
            Box::new(|h| {
                h.origin = Origin::Exploratory {
                    entries: vec![rcfd_api::design::ledger::EntryRef {
                        index: 0,
                        hash: Some("fnv1a64:0000000000000000".into()),
                    }],
                }
            }),
        ),
        (
            "supersedes: no hypothesis 'H-9'",
            Box::new(|h| h.supersedes = Some("H-9".into())),
        ),
        ("at most 2 re-tests", Box::new(|h| h.supersedes = Some("H-3".into()))),
        ("predicted_size", Box::new(|h| h.predicted_size = Some((1.0, 0.0)))),
        (
            "evaluator: id and version",
            Box::new(|h| h.evaluator.id = String::new()),
        ),
        (
            "replicates: no hypothesis 'H-7'",
            Box::new(|h| h.replicates = Some("H-7".into())),
        ),
    ];
    let before = std::fs::read(&path).unwrap();
    let mut ok = 0;
    for (want, f) in &cases {
        let mut h = base.clone();
        f(&mut h);
        let r = register(&l, &h);
        let msg = r.as_ref().err().cloned().unwrap_or_default();
        let hit = msg.contains(want);
        if !hit {
            eprintln!("V1 case '{want}': {r:?}");
        }
        ok += usize::from(hit);
    }
    let after = std::fs::read(&path).unwrap();
    eprintln!(
        "V1: {ok} of {} refused with the field named; file unchanged {}",
        cases.len(),
        before == after
    );
    assert_eq!(ok, cases.len());
    assert_eq!(before, after);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- T1, R1

#[test]
fn t1_staleness() {
    let dir = tmp("t1");
    let path = dir.join("l.jsonl");
    let (_m, claims) = w1_ro_world(&path, 701_770, 12, true);
    assert_eq!(claims.len(), 12);
    let es = entries(&ledger_at(&path));
    let fresh = lessons(&es, &q_all(&[("synthetic.w1", "1")]), &[]);
    let after = lessons(&es, &q_all(&[("synthetic.w1", "2")]), &[]);
    let mut qs = q_all(&[("synthetic.w1", "2")]);
    qs.include_stale = true;
    let with_stale = lessons(&es, &qs, &[]);
    let n_stale = with_stale.rows.iter().filter(|r| r.stale).count();
    eprintln!(
        "T1: rows with version 1 held {}, with version 2 held {} by default, {} with include_stale ({n_stale} stale)",
        fresh.rows.len(),
        after.rows.len(),
        with_stale.rows.len()
    );
    assert_eq!(fresh.rows.len(), 12);
    assert_eq!(after.rows.len(), 0);
    assert_eq!((with_stale.rows.len(), n_stale), (12, 12));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn r1_workers_bytes() {
    let dir = tmp("r1");
    let mut texts = Vec::new();
    for w in [1, 8] {
        let path = dir.join(format!("w1_{w}.jsonl"));
        let l = ledger_at(&path);
        let m = w1(701_780, Fidelity::Analytic);
        explore_w1(&l, &m);
        protocol(&l, &m, 12, w, true);
        let p3 = dir.join(format!("w3_{w}.jsonl"));
        let l3 = ledger_at(&p3);
        let m3 = W3 { b: -0.1 };
        let h = w3_claim(
            &m3,
            Direction::Decreases,
            701_502,
            true,
            (0.1, 10.0),
            HeldTarget::Value(1.0),
        );
        let r = register(&l3, &h).unwrap();
        run(&l3, &r.id, &m3, &RunOptions { workers: w, chunk: 8 }).unwrap();
        texts.push((std::fs::read(&path).unwrap(), std::fs::read(&p3).unwrap()));
    }
    eprintln!(
        "R1: W1 {} / {} bytes, W3 {} / {} bytes",
        texts[0].0.len(),
        texts[1].0.len(),
        texts[0].1.len(),
        texts[1].1.len()
    );
    assert_eq!(texts[0].0, texts[1].0);
    assert_eq!(texts[0].1, texts[1].1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- W3, equal flow

struct W3 {
    b: f64,
}

impl Model for W3 {
    fn id(&self) -> &str {
        "synthetic.w3"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let (s, n) = (val(p, "s"), val(p, "n"));
        let q = n * (1.0 + 0.5 * s);
        let tone = n * n * (1.0 + 0.5 * s) * (1.0 + 0.5 * s) * (1.0 + self.b * s);
        Outcome::ok(vec![Quantity::new("q", q, "m3/s"), Quantity::new("tone", tone, "Pa")])
    }
}

fn w3_claim(m: &W3, dir: Direction, seed: u64, hold: bool, bracket: (f64, f64), target: HeldTarget) -> Hypothesis {
    let mut domain = vec![Bound::new("c", "1", 0.0, 1.0)];
    if !hold {
        domain.push(Bound::new("n", "1/s", 0.5, 1.5));
    }
    let mut h = Hypothesis::new(
        "raising s changes the tone at equal flow",
        Factor {
            name: "s".into(),
            unit: "1".into(),
            low: 0.0,
            high: 1.0,
        },
        Response {
            name: "tone".into(),
            unit: "Pa".into(),
            resolution: 0.0,
        },
        dir,
        0.01,
        domain,
        32,
        seed,
        Scope::of(m, true),
    );
    if hold {
        h.held = vec![Held {
            metric: "q".into(),
            unit: "m3/s".into(),
            target,
            tolerance: 1e-13,
            by: "n".into(),
            by_unit: "1/s".into(),
            bracket,
        }];
    }
    h
}

fn w3_run(
    tag: &str,
    b: f64,
    dir: Direction,
    seed: u64,
    hold: bool,
    bracket: (f64, f64),
    target: HeldTarget,
) -> (TestResult, Ledger, PathBuf) {
    let d = tmp(tag);
    let path = d.join("l.jsonl");
    let l = ledger_at(&path);
    let m = W3 { b };
    let r = register(&l, &w3_claim(&m, dir, seed, hold, bracket, target)).unwrap();
    let rep = run(&l, &r.id, &m, &RunOptions::default()).unwrap();
    assert!(rep.verification.problems.is_empty(), "{:?}", rep.verification.problems);
    (rep.verification.result.unwrap(), l, d)
}

#[test]
fn e1_e2_e3_nc4_equal_flow() {
    // E1, case A: b = 0, "raising s raises tone" at equal flow
    let (ra, _, da) = w3_run(
        "e1",
        0.0,
        Direction::Increases,
        701_501,
        true,
        (0.1, 10.0),
        HeldTarget::Value(1.0),
    );
    let ma = ra.stats.mean;
    eprintln!("E1: mean change {ma:.3e} (limit |.| <= 1e-9); {}", ra.finding.as_str());
    assert!(ma.abs() <= 1e-9, "E1");
    assert_eq!(ra.finding, Finding::Refuted);
    // E2, case B: b = -0.1, "decreases": d = -(change), closed form change -0.1
    let (rb, _, db) = w3_run(
        "e2",
        -0.1,
        Direction::Decreases,
        701_502,
        true,
        (0.1, 10.0),
        HeldTarget::Value(1.0),
    );
    let e2 = rel(-rb.stats.mean, -0.1);
    eprintln!(
        "E2: mean change {:.15} against -0.1 (rel {e2:.2e}); {}",
        -rb.stats.mean,
        rb.finding.as_str()
    );
    assert!(e2 <= 1e-9, "E2");
    assert_eq!(rb.finding, Finding::Confirmed);
    // E2b: as_low_arm at n = 0.8: change -0.1 * 0.8^2
    let (rc, _, dc) = w3_run(
        "e2b",
        -0.1,
        Direction::Decreases,
        701_503,
        true,
        (0.1, 10.0),
        HeldTarget::AsLowArm { at: 0.8 },
    );
    let e2b = rel(-rc.stats.mean, -0.1 * 0.8 * 0.8);
    eprintln!(
        "E2b: mean change {:.15} against {} (rel {e2b:.2e}); {}",
        -rc.stats.mean,
        -0.1 * 0.8 * 0.8,
        rc.finding.as_str()
    );
    assert!(e2b <= 1e-9, "E2b");
    assert_eq!(rc.finding, Finding::Confirmed);
    // NC4: case A without the hold
    let (rn, _, dn) = w3_run(
        "nc4",
        0.0,
        Direction::Increases,
        701_504,
        false,
        (0.1, 10.0),
        HeldTarget::Value(1.0),
    );
    eprintln!(
        "NC4: without the hold mean change {:.4} -> {}",
        rn.stats.mean,
        rn.finding.as_str()
    );
    assert_eq!(rn.finding, Finding::Confirmed, "NC4");
    // E3: bracket [0.1, 0.5] cannot reach q = 1
    let (re, l, de) = w3_run(
        "e3",
        -0.1,
        Direction::Decreases,
        701_505,
        true,
        (0.1, 0.5),
        HeldTarget::Value(1.0),
    );
    let why = re.void_reason.clone().unwrap_or_default();
    let next = register(
        &l,
        &w3_claim(
            &W3 { b: -0.1 },
            Direction::Decreases,
            701_506,
            true,
            (0.1, 10.0),
            HeldTarget::Value(1.0),
        ),
    )
    .unwrap();
    eprintln!(
        "E3: {} ({why}); next registration {} slot {}",
        re.finding.as_str(),
        next.id,
        next.slot
    );
    assert_eq!(re.finding, Finding::Void);
    assert!(why.contains("pair ") && why.contains("arm ") && why.contains("'q'"));
    assert_eq!(next.slot, 2);
    for d in [da, db, dc, dn, de] {
        let _ = std::fs::remove_dir_all(d);
    }
}

// ---------------------------------------------------------------- W2, two tiers

struct W2 {
    seed: u64,
    slopes: [f64; 6],
    id: &'static str,
    tier: Fidelity,
}

impl Model for W2 {
    fn id(&self) -> &str {
        self.id
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        self.tier
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let y: f64 =
            (0..6).map(|k| self.slopes[k] * val(p, &format!("x{k}"))).sum::<f64>() + 0.25 * noise(self.seed, 0, p);
        Outcome::ok(vec![Quantity::new("y", y, "1")])
    }
}

fn w2_claim(m: &W2, k: usize, s: f64, seed: u64, replicates: Option<String>) -> Hypothesis {
    let mut h = Hypothesis::new(
        &format!("raising x{k} {} y", if s > 0.0 { "raises" } else { "lowers" }),
        Factor {
            name: format!("x{k}"),
            unit: "1".into(),
            low: 0.25,
            high: 0.75,
        },
        Response {
            name: "y".into(),
            unit: "1".into(),
            resolution: 0.0,
        },
        if s > 0.0 {
            Direction::Increases
        } else {
            Direction::Decreases
        },
        0.5,
        (0..6)
            .filter(|&q| q != k)
            .map(|q| Bound::new(&format!("x{q}"), "1", 0.0, 1.0))
            .collect(),
        64,
        seed,
        Scope::of(m, false),
    );
    h.origin = Origin::Prior {
        citation: "planted (ADR 0624 W2)".into(),
    };
    h.replicates = replicates;
    h
}

#[test]
fn f1_f2_two_tiers() {
    let dir = tmp("f1");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let low = W2 {
        seed: 701_601,
        slopes: [3.0, 3.0, 3.0, -3.0, 3.0, 0.0],
        id: "synthetic.w2.low",
        tier: Fidelity::ReducedOrder,
    };
    let high = W2 {
        seed: 701_602,
        slopes: [0.0, 0.0, -3.0, 3.0, 3.0, 0.0],
        id: "synthetic.w2.high",
        tier: Fidelity::Cfd,
    };
    let mut low_ids = Vec::new();
    for k in 0..5 {
        let s = low.slopes[k].signum();
        let r = register(&l, &w2_claim(&low, k, s, 701_610 + k as u64, None)).unwrap();
        run(&l, &r.id, &low, &RunOptions::default()).unwrap();
        low_ids.push(r.id);
    }
    let cur = [("synthetic.w2.low", "1"), ("synthetic.w2.high", "1")];
    let es = entries(&l);
    let all = lessons(&es, &q_all(&cur), &[&low]);
    let artefacts: Vec<&String> = low_ids[..4].iter().collect();
    let low_ok = all.rows.iter().filter(|r| artefacts.contains(&&r.id)).all(|r| {
        r.finding == Finding::Confirmed
            && r.scope.tier == Fidelity::ReducedOrder
            && r.model_error.flag() == "model_error_unknown"
    });
    let mut qc = q_all(&cur);
    qc.min_tier = Some(Fidelity::Cfd);
    let before = lessons(&es, &qc, &[]);
    eprintln!(
        "F1: low rows {} (artefacts confirmed at reduced_order with model_error_unknown: {low_ok}); rows at min tier cfd before the re-test {}",
        all.rows.len(),
        before.rows.len()
    );
    assert_eq!(all.rows.len(), 5);
    assert!(low_ok, "F1 set-up");
    assert_eq!(before.rows.len(), 0, "F1");
    // F1b, a declared model error
    let mut qd = q_all(&cur);
    qd.declared_errors = vec![DeclaredError {
        id: "synthetic.w2.low".into(),
        version: "1".into(),
        metric: "y".into(),
        value: 2.0,
    }];
    let decl = lessons(&es, &qd, &[]);
    let below = decl
        .rows
        .iter()
        .filter(|r| artefacts.contains(&&r.id))
        .filter(
            |r| matches!(&r.model_error, ModelError::Known { below: true, source, .. } if source.contains("declared")),
        )
        .count();
    eprintln!("F1b: {below} of 4 artefact rows below_model_error (declared)");
    assert_eq!(below, 4, "F1b");
    // F2, re-tests at the high tier
    let mut high_ids = Vec::new();
    for k in 0..5 {
        let s = low.slopes[k].signum();
        let r = register(&l, &w2_claim(&high, k, s, 701_620 + k as u64, Some(low_ids[k].clone()))).unwrap();
        run(&l, &r.id, &high, &RunOptions::default()).unwrap();
        high_ids.push(r.id);
    }
    let es = entries(&l);
    let all2 = lessons(&es, &q_all(&cur), &[&low]);
    let row = |id: &str| all2.rows.iter().find(|r| r.id == id).cloned();
    let mut refuted = 0;
    let mut both = 0;
    for k in 0..4 {
        let h = row(&high_ids[k]).expect("high row");
        let lo = row(&low_ids[k]).expect("low row");
        let opp_ok = if k >= 2 { h.opposite } else { !h.opposite };
        if h.finding == Finding::Refuted && opp_ok {
            refuted += 1;
        }
        if lo.finding == Finding::Confirmed
            && h.replicates.as_deref() == Some(low_ids[k].as_str())
            && lo.replicated_by == vec![high_ids[k].clone()]
            && lo.statement.contains(&format!("Re-tested at cfd as {}", high_ids[k]))
        {
            both += 1;
        }
    }
    let genuine = row(&high_ids[4]).map(|r| r.finding);
    let after = lessons(&es, &qc, &[]);
    let conf_art = after
        .rows
        .iter()
        .filter(|r| high_ids[..4].contains(&r.id) && r.finding == Finding::Confirmed)
        .count();
    eprintln!(
        "F2: artefacts refuted at cfd {refuted} of 4; both rows kept and linked {both} of 4; genuine x4 at cfd {:?}; rows at min tier cfd {} ({conf_art} confirmed artefact rows)",
        genuine,
        after.rows.len()
    );
    assert_eq!(refuted, 4, "F2");
    assert_eq!(both, 4, "F2 both kept");
    assert_eq!(genuine, Some(Finding::Confirmed));
    assert_eq!(conf_art, 0, "F1 after");
    // F1c: a fidelity summary of the low model in the same ledger
    let spec = {
        let mut s = FidelitySpec::new(
            "w2fid",
            (0..6).map(|k| ParamBox::new(&format!("x{k}"), "1", 0.0, 1.0)).collect(),
            Candidates::Lhs { n: 60, seed: 701_630 },
            vec![CorrectedMetric {
                name: "y".into(),
                floor: 1e-3,
            }],
            "y",
            Sense::Max,
        );
        s.rounds = 1;
        s
    };
    fidelity::run(&spec, &low, &ModelSource::of(&high), &l).unwrap();
    let es = entries(&l);
    let (k_sum, oos) = es
        .iter()
        .enumerate()
        .rev()
        .find(|(_, e)| e.kind == "fidelity_summary")
        .map(|(k, e)| (k, e.metric("y.oos_rmse").unwrap().value))
        .unwrap();
    let c = lessons(&es, &q_all(&cur), &[]);
    let from_summary = c
        .rows
        .iter()
        .filter(|r| low_ids.contains(&r.id))
        .filter(|r| {
            matches!(&r.model_error, ModelError::Known { value, source, .. }
                if value.to_bits() == oos.to_bits() && source == &format!("fidelity_summary at entry {k_sum}"))
        })
        .count();
    eprintln!("F1c: {from_summary} of 5 low rows take the model error {oos:.4e} of the summary at entry {k_sum}");
    assert_eq!(from_summary, 5, "F1c");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- HB1, Holm within a batch

struct W5 {
    mu: [f64; 4],
}

impl Model for W5 {
    fn id(&self) -> &str {
        "synthetic.w5"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let x1 = val(p, "x1");
        let y: f64 = (0..4)
            .map(|j| val(p, &format!("f{j}")) * (self.mu[j] + (x1 - 0.5)))
            .sum();
        Outcome::ok(vec![Quantity::new("y", y, "1")])
    }
}

#[test]
fn hb1_holm_batch() {
    let dir = tmp("hb1");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let a = series_level(1);
    // mu of the second member from the closed form: p* the midpoint of (a/4, a/3], sd of d = sqrt(1/12), 64 pairs
    let p_star = 0.5 * (a / 4.0 + a / 3.0);
    let mu_b = 0.5 + student_t_quantile(1.0 - p_star, 63.0) * (1.0f64 / 12.0).sqrt() / 8.0;
    let m = W5 {
        mu: [2.0, mu_b, 0.5, 0.4],
    };
    let hs: Vec<Hypothesis> = (0..4)
        .map(|j| {
            Hypothesis::new(
                &format!("f{j} raises y"),
                Factor {
                    name: format!("f{j}"),
                    unit: "1".into(),
                    low: 0.0,
                    high: 1.0,
                },
                Response {
                    name: "y".into(),
                    unit: "1".into(),
                    resolution: 0.0,
                },
                Direction::Increases,
                0.5,
                vec![Bound::new("x1", "1", 0.0, 1.0)],
                64,
                701_800 + j as u64,
                Scope::of(&m, true),
            )
        })
        .collect();
    let regs = register_batch(&l, &hs).unwrap();
    assert!(regs.iter().all(|r| r.slot == 1 && r.level.to_bits() == a.to_bits()));
    let mut early_results = 0;
    for r in &regs[..3] {
        let rep = run(&l, &r.id, &m, &RunOptions::default()).unwrap();
        assert!(rep.pending);
        early_results += entries(&l).iter().filter(|e| e.kind == KIND_RESULT).count();
    }
    let rep = run(&l, &regs[3].id, &m, &RunOptions::default()).unwrap();
    let es = entries(&l);
    let results: Vec<&Entry> = es.iter().filter(|e| e.kind == KIND_RESULT).collect();
    let p: Vec<f64> = regs
        .iter()
        .map(|r| {
            results
                .iter()
                .find(|e| e.variant == r.id)
                .unwrap()
                .metric("p_value")
                .unwrap()
                .value
        })
        .collect();
    // Holm by hand
    let mut order: Vec<usize> = (0..4).collect();
    order.sort_by(|&x, &y| p[x].total_cmp(&p[y]).then(x.cmp(&y)));
    let mut holm = [false; 4];
    for (r, &j) in order.iter().enumerate() {
        if p[j] <= a / (4 - r) as f64 {
            holm[j] = true;
        } else {
            break;
        }
    }
    let lib: Vec<bool> = regs
        .iter()
        .map(|r| verify(&es, &r.id, None).result.unwrap().finding == Finding::Confirmed)
        .collect();
    let next = register(&l, &hs[0]).unwrap();
    eprintln!(
        "HB1: mu_b {mu_b:.6}; p {:?}; a/4 {:.5} a/3 {:.5}; Holm by hand {holm:?}; library {lib:?}; early results {early_results}; appended {}; next slot {}",
        p,
        a / 4.0,
        a / 3.0,
        rep.appended_results,
        next.slot
    );
    assert!(p[1] > a / 4.0 && p[1] <= a / 3.0, "HB1 set-up condition");
    assert_eq!(early_results, 0);
    assert_eq!(rep.appended_results, 4);
    assert_eq!(lib, holm.to_vec(), "HB1");
    assert!(lib[1], "HB1 second member confirmed by Holm");
    assert_eq!(next.slot, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- X1

fn ranks_hand(v: &[f64]) -> Vec<f64> {
    v.iter()
        .map(|&x| {
            let below = v.iter().filter(|&&y| y < x).count() as f64;
            let equal = v.iter().filter(|&&y| y == x).count() as f64;
            below + 0.5 * (equal - 1.0)
        })
        .collect()
}

#[test]
fn x1_explore_spearman() {
    let mut es = Vec::new();
    for i in 0..20 {
        let a = ((i * 7) % 20) as f64 / 20.0;
        let b = ((i % 5) as f64).floor(); // ties
        let y = (3.0 * a).sin() + 0.1 * b;
        let z = -(a - 0.3).powi(2) + 0.05 * (i as f64);
        let mut e = Entry {
            kind: "evaluation".into(),
            study: "fx".into(),
            evaluator: "fx".into(),
            evaluator_version: "1".into(),
            verdict: Verdict::Ok,
            params: vec![Quantity::new("a", a, "1"), Quantity::new("b", b, "1")],
            metrics: vec![Quantity::new("y", y, "1"), Quantity::new("z", z, "1")],
            ..Default::default()
        };
        e.variant = format!("v{i}");
        es.push(e);
    }
    let ex = explore(&es, &Filter::default());
    let mut worst: f64 = 0.0;
    for c in &ex.cells {
        let x: Vec<f64> = es.iter().map(|e| e.param(&c.param).unwrap().value).collect();
        let y: Vec<f64> = es.iter().map(|e| e.metric(&c.metric).unwrap().value).collect();
        let want = pearson(&ranks_hand(&x), &ranks_hand(&y));
        worst = worst.max((c.rho - want).abs());
    }
    let sorted = ex.cells.windows(2).all(|w| w[0].rho.abs() >= w[1].rho.abs());
    eprintln!(
        "X1: {} cells, largest error {worst:.2e}, sorted {sorted}",
        ex.cells.len()
    );
    assert!(ex.exploratory);
    assert_eq!(ex.cells.len(), 4);
    assert!(worst <= 1e-12);
    assert!(sorted);
}

// ---------------------------------------------------------------- W4, the reduced-order fan

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn fan_model() -> FanRomModel {
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
    let d = RomModel::default();
    let fan = free_vortex_design(&base, &d, &op(), 0.045, 45.0 / 1.2 / op().omega()).unwrap();
    let k = FanRom::new(fan.clone(), d).unwrap().point(&op(), 0.045).unwrap().dp_s / (0.045 * 0.045);
    FanRomModel {
        fan,
        model: d,
        cond: RomCondition {
            op: op(),
            system: SystemCurve { k, l: 0.0 },
            c0: 343.0,
            observer_r: 1.0,
            observer_theta: std::f64::consts::FRAC_PI_2,
            n_scan: 100,
        },
    }
}

struct DummyFan {
    inner: FanRomModel,
}

impl Model for DummyFan {
    fn id(&self) -> &str {
        "rcfd.fan_rom+dummy"
    }
    fn version(&self) -> &str {
        FAN_ROM_MODEL_VERSION
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn evaluate(&self, p: &[Quantity]) -> Outcome {
        let q: Vec<Quantity> = p.iter().filter(|q| q.name != "sweep_deg").cloned().collect();
        self.inner.evaluate(&q)
    }
}

fn fan_claim(
    m: &DummyFan,
    factor: Factor,
    response: Response,
    min_effect: f64,
    domain: Vec<Bound>,
    seed: u64,
) -> Hypothesis {
    Hypothesis::new(
        &format!("raising {} lowers {}", factor.name, response.name),
        factor,
        response,
        Direction::Decreases,
        min_effect,
        domain,
        32,
        seed,
        Scope::of(m, true),
    )
}

#[test]
fn f3_f4_f5_reduced_order_fan() {
    let dir = tmp("f3");
    let path = dir.join("l.jsonl");
    let l = ledger_at(&path);
    let m = DummyFan { inner: fan_model() };
    let opts = RunOptions { workers: 4, chunk: 8 };
    let h3 = fan_claim(
        &m,
        Factor {
            name: "sweep_deg".into(),
            unit: "deg".into(),
            low: 0.0,
            high: 20.0,
        },
        Response {
            name: "tone1_pa".into(),
            unit: "Pa".into(),
            resolution: 1e-15,
        },
        1e-12,
        vec![
            Bound::new("stagger_common_deg", "deg", -3.0, 3.0),
            Bound::new("chord_rel", "1", -0.2, 0.2),
            Bound::new("tip_gap_delta", "m", 0.0, 5e-4),
        ],
        701_401,
    );
    let r3 = register(&l, &h3).unwrap();
    let res3 = run(&l, &r3.id, &m, &opts).unwrap().verification.result.unwrap();
    let h4 = fan_claim(
        &m,
        Factor {
            name: "tip_gap_delta".into(),
            unit: "m".into(),
            low: 0.0,
            high: 1e-3,
        },
        Response {
            name: "eta_s".into(),
            unit: "1".into(),
            resolution: 1e-6,
        },
        1e-3,
        vec![
            Bound::new("stagger_common_deg", "deg", -3.0, 3.0),
            Bound::new("chord_rel", "1", -0.2, 0.2),
            Bound::new("sweep_deg", "deg", 0.0, 20.0),
        ],
        701_402,
    );
    let r4 = register(&l, &h4).unwrap();
    let res4 = run(&l, &r4.id, &m, &opts).unwrap().verification.result.unwrap();
    let es = entries(&l);
    let v4 = verify(&es, &r4.id, Some(&m));
    let les = lessons(&es, &q_all(&[("rcfd.fan_rom+dummy", FAN_ROM_MODEL_VERSION)]), &[&m]);
    let row3 = les.rows.iter().find(|r| r.id == r3.id).cloned().unwrap();
    let row4 = les.rows.iter().find(|r| r.id == r4.id).cloned().unwrap();
    let all_zero = res3.d.iter().all(|d| *d == Some(0.0));
    eprintln!(
        "F3: d all exactly 0 {all_zero}; {}; statement: {}",
        res3.finding.as_str(),
        row3.statement
    );
    eprintln!(
        "F4: {} mean change {:.4e} bounds ({:.4e}, {:.4e}) level {:.4e}; scope {}; {}; re-run {:?}; statement: {}",
        res4.finding.as_str(),
        -res4.stats.mean,
        -res4.stats.upper,
        -res4.stats.lower,
        res4.level,
        row4.scope.tier.as_str(),
        row4.model_error.flag(),
        v4.evidence_rerun,
        row4.statement
    );
    assert!(all_zero, "F3");
    assert_eq!(res3.finding, Finding::Refuted, "F3");
    assert!(row3.statement.contains("does not respond"), "F3");
    assert_eq!(res4.finding, Finding::Confirmed, "F4");
    assert_eq!(row4.scope.tier, Fidelity::ReducedOrder);
    assert_eq!(row4.model_error.flag(), "model_error_unknown");
    assert!(
        row4.statement.contains("Not tested at cfd") && row4.statement.contains("Model error of this tier unknown")
    );
    assert!(v4.problems.is_empty());
    assert_eq!(v4.evidence_rerun, Some(true));
    // F5: min_effect below 10 x the declared resolution
    let mut h5 = h4.clone();
    h5.min_effect = 5e-6;
    let e5 = register(&l, &h5).unwrap_err();
    eprintln!("F5: {e5}");
    assert!(e5.contains("min_effect") && e5.contains("resolution"));
    let _ = std::fs::remove_dir_all(&dir);
}
