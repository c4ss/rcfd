//! ADR 0622 amendment 1: checks of the fidelity management driver (F-699). Synthetic high sources only.

use rcfd_api::design::fan_rom_eval::{
    FanRomModel, RomCondition, FAN_ROM_MODEL_ID, FAN_ROM_MODEL_VERSION, FAN_ROM_PARAMS,
};
use rcfd_api::design::fidelity::{
    self, loo_residuals, Candidates, CorrectedMetric, Correction, FidelityResult, FidelitySpec, HighSource,
    MeasuredTable, ModelSource, Observation, PairData, ParamBox, Privacy, Promotion, Reason, Rung, TableColumn,
    TrustRule,
};
use rcfd_api::design::ledger::{self, Entry, Fidelity, FnModel, Ledger, Model, Outcome, Quantity, Verdict};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::sampling;
use rcfd_api::design::study::{parallel_map, Sense};
use rcfd_mesh::fan::SectionFamily;
use rcfd_post::fan_rom::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const CLOCK: u64 = 1_700_000_000_000;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f699_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ledger_at(p: &Path) -> Ledger {
    let mut l = Ledger::new(p);
    l.clock = || CLOCK;
    l
}

fn q(e: &Entry, name: &str) -> f64 {
    e.metric(name).unwrap_or_else(|| panic!("metric {name}")).value
}

fn p(params: &[Quantity], name: &str) -> f64 {
    params.iter().find(|q| q.name == name).unwrap().value
}

// ---------------------------------------------------------------- S1 and friends

fn s1_params() -> Vec<ParamBox> {
    vec![
        ParamBox::new("x1", "m", 0.0, 1.0),
        ParamBox::new("x2", "1", -1.0, 2.0),
        ParamBox::new("x3", "s", 10.0, 20.0),
    ]
}

fn s1_low_fg(x: &[Quantity]) -> (f64, f64) {
    let (x1, x2, x3) = (p(x, "x1"), p(x, "x2"), p(x, "x3"));
    ((3.0 * x1).sin() + x2 * x2 + 0.1 * x3, (0.5 * x1).exp() - x2 * x3 / 20.0)
}

fn fg(f: f64, g: f64) -> Outcome {
    Outcome::ok(vec![Quantity::new("f", f, "Pa"), Quantity::new("g", g, "W")])
}

type EvalFn = Box<dyn Fn(&[Quantity]) -> Outcome + Sync + Send>;
type Fm = FnModel<EvalFn>;

fn model(id: &str, fid: Fidelity, f: EvalFn) -> Fm {
    FnModel {
        id: id.into(),
        version: "1".into(),
        fidelity: fid,
        f,
    }
}

fn s1_low() -> Fm {
    model(
        "analytic.s1",
        Fidelity::Analytic,
        Box::new(|x| {
            let (f, g) = s1_low_fg(x);
            fg(f, g)
        }),
    )
}

const S1_F: [f64; 5] = [1.7, -0.3, 0.5, -2.0, 0.04];
const S1_G: [f64; 5] = [0.9, 1.25, -0.1, 0.3, -0.02];

fn s1_high_model() -> Fm {
    model(
        "synthetic.s1",
        Fidelity::Analytic,
        Box::new(|x| {
            let (f, g) = s1_low_fg(x);
            let (x1, x2, x3) = (p(x, "x1"), p(x, "x2"), p(x, "x3"));
            let a = |c: &[f64; 5], y: f64| c[0] * y + c[1] + c[2] * x1 + c[3] * x2 + c[4] * x3;
            fg(a(&S1_F, f), a(&S1_G, g))
        }),
    )
}

fn src<'m>(m: &'m dyn Model, level: &str) -> ModelSource<'m> {
    ModelSource {
        level: Some(level.into()),
        ..ModelSource::of(m)
    }
}

fn fg_metrics() -> Vec<CorrectedMetric> {
    ["f", "g"]
        .iter()
        .map(|n| CorrectedMetric {
            name: (*n).into(),
            floor: 1e-3,
        })
        .collect()
}

fn s1_spec(name: &str, cands: Candidates) -> FidelitySpec {
    FidelitySpec::new(name, s1_params(), cands, fg_metrics(), "f", Sense::Min)
}

fn s1_run(workers: usize, tag: &str) -> (FidelitySpec, FidelityResult, PathBuf) {
    let d = tmp(tag);
    let path = d.join("public.jsonl");
    let mut spec = s1_spec("s1", Candidates::Lhs { n: 200, seed: 699_001 });
    spec.workers = workers;
    let low = s1_low();
    let hm = s1_high_model();
    let r = fidelity::run(&spec, &low, &src(&hm, "high"), &ledger_at(&path)).unwrap();
    (spec, r, path)
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn a1_exact_affine_recovered() {
    let (_, r, _) = s1_run(1, "a1");
    let c = r.correction.as_ref().unwrap();
    for (f, want) in c.fits.iter().zip([S1_F, S1_G]) {
        assert_eq!(f.rung, Rung::R2, "{}: {:?}", f.metric, f);
        assert!(f.trusted, "{}: {}", f.metric, f.why);
        for (k, (&got, &w)) in f.coef.iter().zip(&want).enumerate() {
            eprintln!("A1 {} coef {k}: {got:e} want {w:e} rel {:.2e}", f.metric, rel(got, w));
            assert!(rel(got, w) <= 1e-8, "A1 {} coef {k}", f.metric);
        }
    }
}

fn fh(x: f64) -> f64 {
    (6.0 * x - 2.0).powi(2) * (12.0 * x - 4.0).sin()
}

fn s2_low(alt: bool) -> Fm {
    model(
        if alt { "analytic.s2b" } else { "analytic.s2" },
        Fidelity::Analytic,
        Box::new(move |x| {
            let v = x[0].value;
            let f = if alt {
                0.5 * fh(v) + 10.0 * v
            } else {
                0.5 * fh(v) + 10.0 * (v - 0.5) - 5.0
            };
            Outcome::ok(vec![Quantity::new("f", f, "1")])
        }),
    )
}

fn s2_high() -> Fm {
    model(
        "synthetic.s2",
        Fidelity::Analytic,
        Box::new(|x| Outcome::ok(vec![Quantity::new("f", fh(x[0].value), "1")])),
    )
}

#[test]
fn a2_published_pair_through_the_driver() {
    let d = tmp("a2");
    let pts: Vec<Vec<f64>> = (0..=100).map(|i| vec![i as f64 / 100.0]).collect();
    let mut spec = FidelitySpec::new(
        "s2",
        vec![ParamBox::new("x", "1", 0.0, 1.0)],
        Candidates::List(pts.clone()),
        vec![CorrectedMetric {
            name: "f".into(),
            floor: 1e-3,
        }],
        "f",
        Sense::Min,
    );
    spec.rounds = 2;
    let (lo, hi) = (s2_low(false), s2_high());
    let r = fidelity::run(&spec, &lo, &src(&hi, "high"), &ledger_at(&d.join("l.jsonl"))).unwrap();
    let f = &r.correction.as_ref().unwrap().fits[0];
    assert_eq!(f.rung, Rung::R2, "{f:?}");
    let (rho, b0, c) = (f.coef[0], f.coef[1], f.coef[2]);
    eprintln!(
        "A2 rho {rho:e} ({:.2e}) intercept {b0:e} ({:.2e}) slope {c:e} ({:.2e})",
        rel(rho, 2.0),
        rel(b0, 20.0),
        rel(c, -20.0)
    );
    assert!(rel(rho, 2.0) <= 1e-8 && rel(b0, 20.0) <= 1e-8 && rel(c, -20.0) <= 1e-8);
    let truth: Vec<f64> = pts.iter().map(|x| fh(x[0])).collect();
    let range =
        truth.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - truth.iter().cloned().fold(f64::INFINITY, f64::min);
    let un: Vec<usize> = (0..pts.len()).filter(|&i| !r.final_extrapolated[i][0]).collect();
    let rmse = (un
        .iter()
        .map(|&i| (r.final_predictions[i][0] - truth[i]).powi(2))
        .sum::<f64>()
        / un.len() as f64)
        .sqrt();
    eprintln!(
        "A2 RMSE over {} unflagged of 101: {rmse:e} = {:.2e} of the range {range:e}",
        un.len(),
        rmse / range
    );
    assert!(!un.is_empty() && rmse <= 1e-8 * range);
}

#[test]
fn a2b_paper_sign_direct_fit() {
    let lo = s2_low(true);
    let pairs: Vec<PairData> = [0.0, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .map(|&x| PairData {
            x: vec![x],
            low: vec![lo.evaluate(&[Quantity::new("x", x, "1")]).metrics[0].value],
            high: vec![fh(x)],
            u_high: vec![0.0],
        })
        .collect();
    let c = Correction::fit(
        &pairs,
        &[ParamBox::new("x", "1", 0.0, 1.0)],
        &["f".to_string()],
        &TrustRule::default(),
        ("analytic.s2b", "1"),
        ("synthetic.s2", "1"),
    );
    let f = &c.fits[0];
    eprintln!("A2b rung {:?} coef {:?}", f.rung, f.coef);
    assert_eq!(f.rung, Rung::R2);
    assert!(rel(f.coef[0], 2.0) <= 1e-8 && rel(f.coef[2], -20.0) <= 1e-8 && f.coef[1].abs() <= 1e-8);
}

#[test]
fn a3_t4b_e1_t5_ledger_records_region_explore_and_identity() {
    let (spec, r, path) = s1_run(1, "a3");
    let c = ledger::parse_text(&std::fs::read_to_string(&path).unwrap());
    assert_eq!(c.skipped.len(), 0, "{:?}", c.skipped);
    let es = &c.entries;
    let count = |k: &str, ev: &str| es.iter().filter(|e| e.kind == k && e.evaluator == ev).count();
    assert_eq!(count("evaluation", "analytic.s1"), 200);
    assert_eq!(count("evaluation", "synthetic.s1"), 24);
    assert_eq!(es.iter().filter(|e| e.kind == "fidelity_pair").count(), 24);
    assert_eq!(count("fidelity_summary", fidelity::DRIVER_ID), 4);
    // A3: pairs
    let pe: Vec<&Entry> = es.iter().filter(|e| e.kind == "fidelity_pair").collect();
    for (k, e) in pe.iter().enumerate() {
        let i0 = ledger::resolve(es, &e.parents[0]).unwrap();
        let i1 = ledger::resolve(es, &e.parents[1]).unwrap();
        assert_eq!(es[i0].evaluator, "analytic.s1");
        assert_eq!(es[i1].evaluator, "synthetic.s1");
        assert_eq!(es[i0].variant, e.variant);
        assert_eq!(es[i1].variant, e.variant);
        for (m, mi) in [("f", 0), ("g", 1)] {
            assert_eq!(
                q(e, &format!("{m}.diff")).to_bits(),
                (q(e, &format!("{m}.high")) - q(e, &format!("{m}.low"))).to_bits()
            );
            assert_eq!(q(e, &format!("{m}.pred")).to_bits(), r.pairs[k].pred[mi].to_bits());
        }
    }
    for e in es.iter().filter(|e| e.kind == "fidelity_summary") {
        let text = e.settings["correction"].to_string();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(e.fitted.as_ref().unwrap().hash, format!("fnv1a64:{h:016x}"));
    }
    eprintln!("A3: 200 low, 24 high, 24 pairs, 4 summaries, 0 skipped; parents, diff, pred and fitted hashes hold");
    // E1: explore picks
    let unit = |x: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(&spec.params)
            .map(|(v, b)| (v - b.lo) / (b.hi - b.lo))
            .collect()
    };
    let units: Vec<Vec<f64>> = r.candidates.iter().map(|x| unit(x)).collect();
    let d = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt();
    let mut promoted: Vec<usize> = Vec::new();
    for (ri, rd) in r.rounds.iter().enumerate() {
        let reasons: Vec<Reason> = rd.picks.iter().map(|p| p.1).collect();
        assert_eq!(
            reasons,
            [vec![Reason::Exploit; 4], vec![Reason::Explore; 2]].concat(),
            "round {}",
            ri + 1
        );
        let mut set: Vec<usize> = promoted.clone();
        set.extend(rd.picks[..4].iter().map(|p| p.0));
        for k in 4..6 {
            let mut best: Option<(usize, f64)> = None;
            for i in 0..units.len() {
                if set.contains(&i) {
                    continue;
                }
                let m = set
                    .iter()
                    .map(|&j| d(&units[i], &units[j]))
                    .fold(f64::INFINITY, f64::min);
                if best.is_none_or(|b| m > b.1) {
                    best = Some((i, m));
                }
            }
            assert_eq!(rd.picks[k].0, best.unwrap().0, "E1 round {} pick {k}", ri + 1);
            set.push(best.unwrap().0);
        }
        promoted.extend(rd.picks.iter().map(|p| p.0));
    }
    eprintln!("E1: 4 rounds, reasons and explore picks as computed in the test");
    // T4b: region flags
    let mut flagged_total = 0;
    for ri in 1..r.rounds.len() {
        let c = r.rounds[ri - 1].correction.as_ref().unwrap();
        let pts: Vec<Vec<f64>> = r
            .pairs
            .iter()
            .filter(|p| p.round <= ri)
            .map(|p| units[p.candidate].clone())
            .collect();
        for (i, u) in units.iter().enumerate() {
            let inbox = (0..3).all(|j| {
                let lo = pts.iter().map(|p| p[j]).fold(f64::INFINITY, f64::min) - 0.05;
                let hi = pts.iter().map(|p| p[j]).fold(f64::NEG_INFINITY, f64::max) + 0.05;
                lo <= u[j] && u[j] <= hi
            });
            let near = pts.iter().map(|p| d(p, u)).fold(f64::INFINITY, f64::min) <= 0.25;
            let flag = !(inbox && near);
            for m in 0..2 {
                assert_eq!(
                    r.rounds[ri].extrapolated[i][m],
                    flag,
                    "T4b round {} candidate {i}",
                    ri + 1
                );
                if flag {
                    flagged_total += 1;
                    let yl = r.low[i].as_ref().unwrap()[m];
                    let f = &c.fits[m];
                    let want = if f.constant_trusted { yl + f.constant } else { yl };
                    assert_eq!(r.rounds[ri].predictions[i][m].to_bits(), want.to_bits());
                }
            }
        }
    }
    eprintln!(
        "T4b: flags equal the region rule in rounds 2 to 4 ({flagged_total} flagged (candidate, metric) predictions)"
    );
    // T5
    let c = r.correction.as_ref().unwrap();
    let back = Correction::from_json(&serde_json::from_str(&c.to_json().to_string()).unwrap()).unwrap();
    for i in 0..50 {
        let yl = r.low[i].as_ref().unwrap();
        let (a, fa) = c.predict(&r.candidates[i], yl);
        let (b, fb) = back.predict(&r.candidates[i], yl);
        assert_eq!(fa, fb);
        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
    }
    let yl = r.low[0].as_ref().unwrap();
    let e1 = c
        .predict_for("analytic.s1", "2", "synthetic.s1", "1", &r.candidates[0], yl)
        .unwrap_err();
    let e2 = c
        .predict_for("analytic.s1", "1", "synthetic.s1", "9", &r.candidates[0], yl)
        .unwrap_err();
    eprintln!("T5: round trip bit-equal at 50 points; refusals: '{e1}'; '{e2}'");
    assert!(e1.contains("version 1") && e1.contains("version 2"));
    assert!(e2.contains("version 1") && e2.contains("version 9"));
}

// ---------------------------------------------------------------- S4, S5, S6, S1h

fn fnv_bits(x: &[Quantity]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for q in x {
        for b in q.value.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

struct Noisy;

impl HighSource for Noisy {
    fn id(&self) -> &str {
        "synthetic.s4"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn level(&self) -> Option<String> {
        Some("noisy".into())
    }
    fn observe(&self, x: &[Quantity]) -> Observation {
        let mut g = Rng::new(699_004 ^ fnv_bits(x));
        let (u1, u2) = (g.uniform(), g.uniform());
        let z = (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let (f, gg) = s1_low_fg(x);
        Observation {
            outcome: fg(f + 0.05 * z, gg),
            uncertainty: vec![Quantity::new("f", 0.05, "Pa")],
        }
    }
}

fn f_only_spec(name: &str, n: usize, seed: u64) -> FidelitySpec {
    FidelitySpec::new(
        name,
        s1_params(),
        Candidates::Lhs { n, seed },
        vec![CorrectedMetric {
            name: "f".into(),
            floor: 1e-3,
        }],
        "f",
        Sense::Min,
    )
}

fn low_lines(path: &Path, n: usize) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .take(n)
        .map(str::to_string)
        .collect()
}

fn same_screening(a: &FidelityResult, b: &FidelityResult) {
    assert_eq!(a.rounds.len(), b.rounds.len());
    for (x, y) in a.rounds.iter().zip(&b.rounds) {
        assert_eq!(x.picks, y.picks);
        for (p, q) in x.predictions.iter().zip(&y.predictions) {
            assert!(p.iter().zip(q).all(|(u, v)| u.to_bits() == v.to_bits()));
        }
    }
}

#[test]
fn t1_nc2_noise_is_not_corrected() {
    let d = tmp("t1");
    let low = s1_low();
    let spec = f_only_spec("s4", 300, 699_041);
    let on = fidelity::run(&spec, &low, &Noisy, &ledger_at(&d.join("on.jsonl"))).unwrap();
    let off = fidelity::run(
        &FidelitySpec {
            correct: false,
            ..spec.clone()
        },
        &low,
        &Noisy,
        &ledger_at(&d.join("off.jsonl")),
    )
    .unwrap();
    for (k, rd) in on.rounds.iter().enumerate() {
        let f = &rd.correction.as_ref().unwrap().fits[0];
        eprintln!(
            "T1 round {}: rung {} trusted {} ({}), constant trusted {}, rmse0 {:.4e}, u_rms {:.4e}",
            k + 1,
            f.rung.as_str(),
            f.trusted,
            f.why,
            f.constant_trusted,
            f.rmse0,
            f.u_rms
        );
        assert!(!f.trusted && !f.constant_trusted);
    }
    same_screening(&on, &off);
    assert_eq!(
        low_lines(&d.join("on.jsonl"), 300),
        low_lines(&d.join("off.jsonl"), 300)
    );
    // NC2: every guard off
    let nc = FidelitySpec {
        trust: TrustRule {
            in_sample: true,
            one_se: false,
            k_u: 0.0,
            f_trust: 1.0,
            ..TrustRule::default()
        },
        ..spec
    };
    let r = fidelity::run(&nc, &low, &Noisy, &ledger_at(&d.join("nc2.jsonl"))).unwrap();
    let trusted: Vec<bool> = r
        .rounds
        .iter()
        .map(|rd| rd.correction.as_ref().unwrap().fits[0].trusted)
        .collect();
    eprintln!("NC2 trusted per round {trusted:?}");
    assert!(trusted.iter().any(|&t| t), "NC2 must be trusted in some round");
}

#[test]
fn l1_leave_one_out_against_brute_force() {
    let d = tmp("l1");
    let low = s1_low();
    let spec = f_only_spec("s4", 300, 699_041);
    let r = fidelity::run(&spec, &low, &Noisy, &ledger_at(&d.join("l.jsonl"))).unwrap();
    let pairs: Vec<PairData> = r
        .pairs
        .iter()
        .map(|p| PairData {
            x: r.candidates[p.candidate].clone(),
            low: p.low.clone(),
            high: p.high.clone(),
            u_high: vec![0.05],
        })
        .collect();
    let n = pairs.len();
    let solve = |a: Vec<Vec<f64>>, b: Vec<f64>| -> Vec<f64> {
        // normal equations by Gaussian elimination with partial pivoting
        let k = a[0].len();
        let mut m = vec![vec![0.0; k + 1]; k];
        for (row, &y) in a.iter().zip(&b) {
            for i in 0..k {
                for j in 0..k {
                    m[i][j] += row[i] * row[j];
                }
                m[i][k] += row[i] * y;
            }
        }
        for c in 0..k {
            let piv = (c..k).max_by(|&x, &y| m[x][c].abs().total_cmp(&m[y][c].abs())).unwrap();
            m.swap(c, piv);
            for rr in 0..k {
                if rr != c {
                    let f = m[rr][c] / m[c][c];
                    for j in c..=k {
                        m[rr][j] -= f * m[c][j];
                    }
                }
            }
        }
        (0..k).map(|i| m[i][k] / m[i][i]).collect()
    };
    for rung in [Rung::R1Add, Rung::R1Mul, Rung::R2] {
        let got = loo_residuals(&pairs, &spec.params, 0, rung).unwrap();
        let mut worst: f64 = 0.0;
        let rms = (got.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt();
        for i in 0..n {
            let rest: Vec<&PairData> = pairs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, p)| p)
                .collect();
            let pi = &pairs[i];
            let pred = match rung {
                Rung::R1Add => pi.low[0] + rest.iter().map(|p| p.high[0] - p.low[0]).sum::<f64>() / rest.len() as f64,
                Rung::R1Mul => {
                    pi.low[0] * rest.iter().map(|p| p.low[0] * p.high[0]).sum::<f64>()
                        / rest.iter().map(|p| p.low[0] * p.low[0]).sum::<f64>()
                }
                _ => {
                    let row = |p: &PairData| [vec![p.low[0], 1.0], p.x.clone()].concat();
                    let c = solve(
                        rest.iter().map(|p| row(p)).collect(),
                        rest.iter().map(|p| p.high[0]).collect(),
                    );
                    row(pi).iter().zip(&c).map(|(a, b)| a * b).sum()
                }
            };
            let brute = pi.high[0] - pred;
            let e = (got[i] - brute).abs();
            let ok = if brute.abs() < 1e-3 * rms {
                e <= 1e-12
            } else {
                e <= 1e-9 * brute.abs()
            };
            worst = worst.max(if brute.abs() < 1e-3 * rms { 0.0 } else { e / brute.abs() });
            assert!(ok, "L1 {} pair {i}: {} vs {}", rung.as_str(), got[i], brute);
        }
        eprintln!(
            "L1 {}: {n} pairs, largest relative difference {worst:.2e}",
            rung.as_str()
        );
    }
}

#[test]
fn t2_always_failing_source() {
    let d = tmp("t2");
    let low = s1_low();
    let fail = model(
        "synthetic.s5",
        Fidelity::Analytic,
        Box::new(|_| Outcome {
            verdict: Verdict::Diverged,
            reason: "synthetic: the high tier diverged".into(),
            ..Default::default()
        }),
    );
    let spec = s1_spec("s5", Candidates::Lhs { n: 200, seed: 699_001 });
    let on = fidelity::run(&spec, &low, &src(&fail, "failing"), &ledger_at(&d.join("on.jsonl"))).unwrap();
    let off = fidelity::run(
        &FidelitySpec { correct: false, ..spec },
        &low,
        &src(&fail, "failing"),
        &ledger_at(&d.join("off.jsonl")),
    )
    .unwrap();
    assert!(on.pairs.is_empty() && on.correction.is_none());
    let c = ledger::parse_text(&std::fs::read_to_string(d.join("on.jsonl")).unwrap());
    let last = c.entries.iter().rev().find(|e| e.kind == "fidelity_summary").unwrap();
    eprintln!("T2: 0 pairs, n_failed_high {}", q(last, "n_failed_high"));
    assert_eq!(q(last, "n_failed_high"), 24.0);
    same_screening(&on, &off);
    assert_eq!(
        low_lines(&d.join("on.jsonl"), 200),
        low_lines(&d.join("off.jsonl"), 200)
    );
}

#[test]
fn t3_identical_source() {
    let d = tmp("t3");
    let low = s1_low();
    let copy = s1_low();
    let s6 = ModelSource {
        id: "synthetic.s6".into(),
        level: Some("copy".into()),
        ..ModelSource::of(&copy)
    };
    let spec = s1_spec("s6", Candidates::Lhs { n: 200, seed: 699_001 });
    let r = fidelity::run(&spec, &low, &s6, &ledger_at(&d.join("l.jsonl"))).unwrap();
    let c = ledger::parse_text(&std::fs::read_to_string(d.join("l.jsonl")).unwrap());
    let pe: Vec<&Entry> = c.entries.iter().filter(|e| e.kind == "fidelity_pair").collect();
    assert_eq!(pe.len(), 24);
    for e in &pe {
        assert!(q(e, "f.diff") == 0.0 && q(e, "g.diff") == 0.0);
    }
    for rd in &r.rounds {
        for (i, pr) in rd.predictions.iter().enumerate() {
            let yl = r.low[i].as_ref().unwrap();
            assert!(pr.iter().zip(yl).all(|(a, b)| a.to_bits() == b.to_bits()));
        }
    }
    for f in &r.correction.as_ref().unwrap().fits {
        eprintln!(
            "T3 {}: rung {} trusted {} ({})",
            f.metric,
            f.rung.as_str(),
            f.trusted,
            f.why
        );
        assert!(f.rung == Rung::R0 && !f.trusted);
    }
}

#[test]
fn t4_outside_the_calibrated_region_only_the_constant() {
    let d = tmp("t4");
    let low = s1_low();
    let hi = model(
        "synthetic.s1h",
        Fidelity::Analytic,
        Box::new(|x| {
            let (f, g) = s1_low_fg(x);
            let x1 = p(x, "x1");
            fg(f + 2.0 + 0.2 * x1, g + 2.0 + 0.2 * x1)
        }),
    );
    let half: Vec<Vec<f64>> = sampling::latin_hypercube(200, 3, 699_051)
        .iter()
        .map(|u| vec![0.5 * u[0], -1.0 + 3.0 * u[1], 10.0 + 10.0 * u[2]])
        .collect();
    let spec = s1_spec("s1h", Candidates::List(half));
    let r = fidelity::run(&spec, &low, &src(&hi, "high"), &ledger_at(&d.join("l.jsonl"))).unwrap();
    let c = r.correction.as_ref().unwrap();
    let queries: Vec<Vec<f64>> = sampling::latin_hypercube(500, 3, 699_052)
        .iter()
        .map(|u| vec![0.6 + 0.4 * u[0], -1.0 + 3.0 * u[1], 10.0 + 10.0 * u[2]])
        .collect();
    let mut flagged = 0;
    for m in 0..2 {
        let b = r.pairs.iter().map(|p| p.high[m] - p.low[m]).sum::<f64>() / r.pairs.len() as f64;
        assert!(c.fits[m].constant_trusted, "T4 constant of metric {m} not trusted");
        assert_eq!(c.fits[m].constant.to_bits(), b.to_bits());
        for x in &queries {
            let qs: Vec<Quantity> = spec
                .params
                .iter()
                .zip(x)
                .map(|(pb, &v)| Quantity::new(&pb.name, v, &pb.unit))
                .collect();
            let (f, g) = s1_low_fg(&qs);
            let (y, fl) = c.predict(x, &[f, g]);
            assert!(fl[m]);
            flagged += 1;
            assert_eq!(y[m].to_bits(), ([f, g][m] + b).to_bits());
        }
        eprintln!("T4 metric {m}: constant {b:e}, rung inside {}", c.fits[m].rung.as_str());
    }
    eprintln!("T4: {flagged} of 1000 (query, metric) flagged and given the constant");
}

#[test]
fn r1_s1_ledger_bytes_do_not_depend_on_workers() {
    let (_, _, a) = s1_run(1, "r1a");
    let (_, _, b) = s1_run(8, "r1b");
    let (x, y) = (std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
    eprintln!("R1 S1: {} and {} bytes", x.len(), y.len());
    assert_eq!(x, y);
}

// ---------------------------------------------------------------- S7, privacy

fn s7_candidates() -> Vec<Vec<f64>> {
    s1_spec("s7", Candidates::Lhs { n: 200, seed: 699_071 }).candidate_points()
}

fn s7_table(dir: &Path) -> PathBuf {
    let hm = s1_high_model();
    let path = dir.join("table.csv");
    let mut t = String::from(
        "variant,point,rpm,flow_m3_s,dp_static_Pa,dp_side,supply_V,supply_A,p_amb_Pa,T_amb_K,rh,lwa_dB,eta_s,notes\n",
    );
    for (i, x) in s7_candidates().iter().enumerate() {
        let qs: Vec<Quantity> = s1_params()
            .iter()
            .zip(x)
            .map(|(pb, &v)| Quantity::new(&pb.name, v, &pb.unit))
            .collect();
        let o = hm.evaluate(&qs);
        t.push_str(&format!(
            "v{i},{i},2999.5,0.0425,{:?},outlet,12.25,0.105,101325.5,296.15,0.425,33.75,{:?},synthetic\n",
            o.metrics[0].value, o.metrics[1].value
        ));
    }
    std::fs::write(&path, t).unwrap();
    path
}

fn s7_source(table: &Path) -> MeasuredTable {
    let map: HashMap<[u64; 3], String> = s7_candidates()
        .iter()
        .enumerate()
        .map(|(i, x)| ([x[0].to_bits(), x[1].to_bits(), x[2].to_bits()], format!("v{i}")))
        .collect();
    MeasuredTable::read(
        table,
        vec![
            TableColumn {
                column: "dp_static_Pa".into(),
                metric: "f".into(),
                unit: "Pa".into(),
                uncertainty: 0.01,
            },
            TableColumn {
                column: "eta_s".into(),
                metric: "g".into(),
                unit: "W".into(),
                uncertainty: 0.001,
            },
        ],
        Box::new(move |x: &[Quantity]| {
            let k = [x[0].value.to_bits(), x[1].value.to_bits(), x[2].value.to_bits()];
            map.get(&k).cloned()
        }),
        "rig-synthetic",
        "1",
    )
    .unwrap()
}

fn s7_run(
    tag: &str,
    workers: usize,
    rounds: usize,
    promote: usize,
    explore: usize,
) -> (PathBuf, PathBuf, FidelityResult) {
    let d = tmp(tag);
    let table = s7_table(&d);
    let src = s7_source(&table);
    let (pubp, privp) = (
        d.join("public").join("ledger.jsonl"),
        d.join("private").join("ledger.jsonl"),
    );
    let mut spec = s1_spec("s7", Candidates::Lhs { n: 200, seed: 699_071 });
    spec.workers = workers;
    spec.rounds = rounds;
    spec.promote_per_round = promote;
    spec.explore_per_round = explore;
    spec.privacy = Some(Privacy::new(&privp, "rig-synthetic"));
    let low = s1_low();
    let r = fidelity::run(&spec, &low, &src, &ledger_at(&pubp)).unwrap();
    (pubp, privp, r)
}

fn texts(v: f64) -> Vec<String> {
    vec![format!("{v}"), serde_json::to_string(&v).unwrap()]
}

fn search(public: &str, values: &[f64]) -> (usize, usize) {
    let mut searched = 0;
    let mut found = 0;
    for &v in values {
        for t in texts(v) {
            if t.len() >= 8 {
                searched += 1;
                if public.contains(&t) {
                    found += 1;
                }
            }
        }
    }
    (searched, found)
}

fn private_values(privp: &Path, r: &FidelityResult, corrected: bool) -> Vec<f64> {
    let c = Ledger::private(privp).unwrap().read().unwrap();
    let mut v = Vec::new();
    for e in c.entries.iter().filter(|e| e.kind == "fidelity_pair") {
        for m in ["f", "g"] {
            for s in ["high", "diff", "rel", "pred"] {
                let x = q(e, &format!("{m}.{s}"));
                // amendment 2: an uncorrected prediction is the public low value
                if corrected && s == "pred" && x.to_bits() == q(e, &format!("{m}.low")).to_bits() {
                    continue;
                }
                v.push(x);
            }
        }
    }
    for rd in &r.rounds {
        if let Some(c) = &rd.correction {
            for f in &c.fits {
                v.extend(&f.coef);
                v.push(f.constant);
            }
        }
    }
    for (i, pr) in r.final_predictions.iter().enumerate() {
        let yl = r.low[i].as_ref().unwrap();
        for (a, b) in pr.iter().zip(yl) {
            if a.to_bits() != b.to_bits() {
                v.push(*a);
            }
        }
    }
    v
}

#[test]
#[ignore = "defective set-up record (ADR 0622 amendment 2): searched round-1 pred values equal to the public low values; 24 of 1260 texts found; corrected test p1_nc4_p5_corrected"]
fn p1_nc4_p5_no_private_value_in_the_public_ledger() {
    p1_nc4_p5(false);
}

#[test]
fn p1_nc4_p5_corrected() {
    p1_nc4_p5(true);
}

fn p1_nc4_p5(corrected: bool) {
    let (pubp, privp, r) = s7_run(if corrected { "p1c" } else { "p1" }, 1, 4, 6, 2);
    let public = std::fs::read_to_string(&pubp).unwrap();
    let vals = private_values(&privp, &r, corrected);
    let (searched, found) = search(&public, &vals);
    eprintln!(
        "P1: {} values, {searched} texts searched, {found} found in {} public bytes",
        vals.len(),
        public.len()
    );
    assert!(searched >= 400 && found == 0);
    // P5
    let pc = Ledger::private(&privp).unwrap().read().unwrap();
    assert_eq!(pc.skipped.len(), 0);
    assert_eq!(pc.entries.iter().filter(|e| e.kind == "fidelity_pair").count(), 24);
    assert_eq!(pc.entries.iter().filter(|e| e.kind == "fidelity_summary").count(), 4);
    let uc = ledger_at(&pubp).read().unwrap();
    assert_eq!(uc.skipped.len(), 0);
    assert_eq!(uc.entries.iter().filter(|e| e.kind == "evaluation").count(), 200);
    assert_eq!(uc.entries.iter().filter(|e| e.kind == "fidelity_summary").count(), 4);
    assert!(uc
        .entries
        .iter()
        .all(|e| e.fidelity != Fidelity::Measured && e.kind != "fidelity_pair"));
    eprintln!("P5: private 0 skipped, 24 pairs, 4 summaries; public 0 skipped, 200 low entries, 4 summaries, no measured, no pair");
    // NC4: the same table as a public source
    struct Public<'a>(&'a MeasuredTable);
    impl HighSource for Public<'_> {
        fn id(&self) -> &str {
            "table.public"
        }
        fn version(&self) -> &str {
            "1"
        }
        fn fidelity(&self) -> Fidelity {
            Fidelity::Analytic
        }
        fn level(&self) -> Option<String> {
            Some("table".into())
        }
        fn observe(&self, x: &[Quantity]) -> Observation {
            self.0.observe(x)
        }
    }
    let d = tmp("nc4");
    let table = s7_table(&d);
    let t = s7_source(&table);
    let spec = s1_spec("s7", Candidates::Lhs { n: 200, seed: 699_071 });
    let low = s1_low();
    let pp = d.join("public.jsonl");
    let r2 = fidelity::run(&spec, &low, &Public(&t), &ledger_at(&pp)).unwrap();
    let public2 = std::fs::read_to_string(&pp).unwrap();
    let mut vals2 = Vec::new();
    for pr in &r2.pairs {
        vals2.extend(&pr.high);
    }
    let (s2, f2) = search(&public2, &vals2);
    eprintln!("NC4: {s2} texts searched, {f2} found");
    assert!(f2 >= 1);
}

fn public_summaries(pubp: &Path) -> Vec<Entry> {
    ledger_at(pubp)
        .read()
        .unwrap()
        .entries
        .into_iter()
        .filter(|e| e.kind == "fidelity_summary")
        .collect()
}

fn has_agg(e: &Entry) -> bool {
    e.metrics.iter().any(|m| m.name.ends_with(".n_pairs"))
}

#[test]
fn p2_aggregates_only_for_at_least_five_pairs_rounded() {
    let (a, _, _) = s7_run("p2a", 1, 1, 4, 0);
    let s = public_summaries(&a);
    assert_eq!(s.len(), 1);
    assert!(!has_agg(&s[0]), "4 pairs must not publish");
    let (b, _, _) = s7_run("p2b", 1, 1, 5, 0);
    let s = public_summaries(&b);
    assert!(has_agg(&s[0]));
    assert_eq!(q(&s[0], "f.n_pairs"), 5.0);
    for m in &s[0].metrics {
        let r3: f64 = format!("{:.2e}", m.value).parse().unwrap();
        assert_eq!(m.value.to_bits(), r3.to_bits(), "P2 {} not rounded", m.name);
    }
    eprintln!(
        "P2: 4 pairs none; 5 pairs: {:?}",
        s[0].metrics
            .iter()
            .map(|m| (m.name.as_str(), m.value))
            .collect::<Vec<_>>()
    );
    let (c, _, _) = s7_run("p2c", 1, 5, 2, 0);
    let s = public_summaries(&c);
    let pubs: Vec<bool> = s.iter().map(has_agg).collect();
    eprintln!("P2 blocks: {pubs:?}");
    assert_eq!(pubs, [false, false, true, false, true]);
}

#[test]
fn p3_private_path_in_the_repository_and_public_inside_private_refused() {
    let calls = AtomicUsize::new(0);
    let counted = FnModel {
        id: "analytic.s1".to_string(),
        version: "1".to_string(),
        fidelity: Fidelity::Analytic,
        f: |x: &[Quantity]| {
            calls.fetch_add(1, Ordering::SeqCst);
            let (f, g) = s1_low_fg(x);
            fg(f, g)
        },
    };
    let d = tmp("p3");
    let table = s7_table(&d);
    let t = s7_source(&table);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("f699_private")
        .join("ledger.jsonl");
    let pubp = d.join("public.jsonl");
    let mut spec = s1_spec("s7", Candidates::Lhs { n: 200, seed: 699_071 });
    spec.privacy = Some(Privacy::new(&repo, "rig-synthetic"));
    let e1 = fidelity::run(&spec, &counted, &t, &ledger_at(&pubp)).unwrap_err();
    eprintln!("P3 (a): {e1}");
    assert!(e1.contains("f699_private") && e1.contains("repository"));
    assert!(!repo.exists() && !pubp.exists());
    let privp = d.join("priv").join("ledger.jsonl");
    let inside = d.join("priv").join("sub").join("public.jsonl");
    spec.privacy = Some(Privacy::new(&privp, "rig-synthetic"));
    let e2 = fidelity::run(&spec, &counted, &t, &ledger_at(&inside)).unwrap_err();
    eprintln!("P3 (b): {e2}");
    assert!(e2.contains("public.jsonl") && e2.contains("inside"));
    assert!(!privp.exists() && !inside.exists());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn p4_table_errors_name_row_and_column_never_a_value() {
    let d = tmp("p4");
    let good = std::fs::read_to_string(s7_table(&d)).unwrap();
    let cells: Vec<String> = good
        .lines()
        .skip(1)
        .flat_map(|l| l.split(',').map(str::to_string).collect::<Vec<_>>())
        .filter(|c| c.len() >= 3)
        .collect();
    let mk = |name: &str, text: String| {
        let p = d.join(name);
        std::fs::write(&p, text).unwrap();
        p
    };
    let mut lines: Vec<String> = good.lines().map(str::to_string).collect();
    let mut f: Vec<String> = lines[3].split(',').map(str::to_string).collect();
    f[4] = "12.5x".into();
    lines[3] = f.join(",");
    let bad_cell = mk("bad_cell.csv", lines.join("\n"));
    let no_col = mk("no_col.csv", good.replacen(",eta_s,", ",eta_x,", 1));
    let no_var = mk("no_var.csv", good.replacen("variant,", "variant_id,", 1));
    let mut msgs = Vec::new();
    for (p, want) in [
        (&bad_cell, vec!["row 4", "column 'dp_static_Pa'"]),
        (&no_col, vec!["eta_s"]),
        (&no_var, vec!["variant"]),
    ] {
        let e = match MeasuredTable::read(
            p,
            vec![
                TableColumn {
                    column: "dp_static_Pa".into(),
                    metric: "f".into(),
                    unit: "Pa".into(),
                    uncertainty: 0.01,
                },
                TableColumn {
                    column: "eta_s".into(),
                    metric: "g".into(),
                    unit: "W".into(),
                    uncertainty: 0.001,
                },
            ],
            Box::new(|_| None),
            "rig-synthetic",
            "1",
        ) {
            Err(e) => e,
            Ok(_) => panic!("P4: {} accepted", p.display()),
        };
        eprintln!("P4: {e}");
        for w in want {
            assert!(e.contains(w), "P4: '{e}' lacks '{w}'");
        }
        let leaked: Vec<&String> = cells.iter().filter(|c| e.contains(c.as_str())).collect();
        assert!(leaked.is_empty() && !e.contains("12.5x"), "P4 leaked {leaked:?}");
        msgs.push(e);
    }
    eprintln!(
        "P4: {} cells of at least 3 characters searched in 3 messages, 0 found",
        cells.len()
    );
}

#[test]
fn r1_s7_corrected() {
    let mut bytes = Vec::new();
    for w in [1, 8] {
        let (a, b, _) = s7_run("r1s7c", w, 4, 6, 2);
        bytes.push((std::fs::read(a).unwrap(), std::fs::read(b).unwrap()));
    }
    eprintln!(
        "R1 S7 (corrected): public {} / {} bytes, private {} / {} bytes",
        bytes[0].0.len(),
        bytes[1].0.len(),
        bytes[0].1.len(),
        bytes[1].1.len()
    );
    assert_eq!(bytes[0].0, bytes[1].0);
    assert_eq!(bytes[0].1, bytes[1].1);
}

#[test]
#[ignore = "defective set-up record (ADR 0622 amendment 2): the two runs used different private paths, which the private summaries record; public equal, private 201673 bytes each but differing; corrected test r1_s7_corrected"]
fn r1_s7_public_and_private_bytes_do_not_depend_on_workers() {
    let (a1, a2, _) = s7_run("r1s7a", 1, 4, 6, 2);
    let (b1, b2, _) = s7_run("r1s7b", 8, 4, 6, 2);
    let (pa, pb) = (std::fs::read(a1).unwrap(), std::fs::read(b1).unwrap());
    let (qa, qb) = (std::fs::read(a2).unwrap(), std::fs::read(b2).unwrap());
    eprintln!(
        "R1 S7: public {} / {} bytes, private {} / {} bytes",
        pa.len(),
        pb.len(),
        qa.len(),
        qb.len()
    );
    assert_eq!(pa, pb);
    assert_eq!(qa, qb);
}

// ---------------------------------------------------------------- S3, the reduced-order fan

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn fan_model(m: RomModel) -> FanRomModel {
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
        model: m,
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

fn perturbed() -> RomModel {
    let d = RomModel::default();
    RomModel {
        tip_cd: d.tip_cd * 1.3,
        annulus_drag: d.annulus_drag * 1.5,
        secondary_drag: d.secondary_drag * 1.5,
        ..d
    }
}

fn s3_spec(n: usize, seed: u64, workers: usize) -> FidelitySpec {
    let b = [(-10.0, 2.0), (-0.45, 0.3), (-5e-4, 1e-3)];
    let mut s = FidelitySpec::new(
        "s3",
        FAN_ROM_PARAMS
            .iter()
            .zip(b)
            .map(|((n, u), (lo, hi))| ParamBox::new(n, u, lo, hi))
            .collect(),
        Candidates::Lhs { n, seed },
        vec![
            CorrectedMetric {
                name: "dp_s".into(),
                floor: 1.0,
            },
            CorrectedMetric {
                name: "eta_s".into(),
                floor: 1e-3,
            },
        ],
        "eta_s",
        Sense::Max,
    );
    s.workers = workers;
    s
}

fn high_of(m: &dyn Model) -> ModelSource<'_> {
    ModelSource {
        model: m,
        id: "synthetic.fan_rom_perturbed".into(),
        version: "1".into(),
        fidelity: Fidelity::ReducedOrder,
        level: Some("perturbed-losses".into()),
    }
}

fn s3_counts(path: &Path, n: usize, r: &FidelityResult) {
    let c = ledger_at(path).read().unwrap();
    assert_eq!(c.skipped.len(), 0);
    let low = c
        .entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.evaluator == FAN_ROM_MODEL_ID)
        .count();
    let high = c
        .entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.evaluator == "synthetic.fan_rom_perturbed")
        .count();
    let ok_high = c
        .entries
        .iter()
        .filter(|e| e.kind == "evaluation" && e.evaluator == "synthetic.fan_rom_perturbed" && e.verdict == Verdict::Ok)
        .count();
    let pairs = c.entries.iter().filter(|e| e.kind == "fidelity_pair").count();
    let sums = c.entries.iter().filter(|e| e.kind == "fidelity_summary").count();
    eprintln!(
        "S3 counts: {low} low, {high} high ({ok_high} ok), {pairs} pairs, {sums} summaries; result pairs {}",
        r.pairs.len()
    );
    assert_eq!((low, high, pairs, sums), (n, 24, ok_high, 4));
}

#[test]
fn b6_reduced_order_fan_fast_runs_and_is_worker_independent() {
    let d = tmp("b6");
    let (lo, hm) = (fan_model(RomModel::default()), fan_model(perturbed()));
    let hs = high_of(&hm);
    let mut bytes = Vec::new();
    for w in [1, 8] {
        let path = d.join(format!("w{w}.jsonl"));
        let r = fidelity::run(&s3_spec(200, 699_311, w), &lo, &hs, &ledger_at(&path)).unwrap();
        s3_counts(&path, 200, &r);
        bytes.push(std::fs::read(&path).unwrap());
    }
    eprintln!("B6: {} bytes at 1 and 8 workers", bytes[0].len());
    assert_eq!(bytes[0], bytes[1]);
}

type Cache = HashMap<Vec<u64>, Outcome>;

fn cached(id: &str, version: &str, cache: Cache) -> Fm {
    model_v(
        id,
        version,
        Box::new(move |x: &[Quantity]| cache[&x.iter().map(|q| q.value.to_bits()).collect::<Vec<_>>()].clone()),
    )
}

fn model_v(id: &str, version: &str, f: EvalFn) -> Fm {
    FnModel {
        id: id.into(),
        version: version.into(),
        fidelity: Fidelity::ReducedOrder,
        f,
    }
}

fn gap(r: &FidelityResult, hi: &[Option<f64>]) -> f64 {
    let ok: Vec<f64> = (0..hi.len()).filter_map(|i| r.low[i].as_ref().and(hi[i])).collect();
    let best = ok.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = best - ok.iter().cloned().fold(f64::INFINITY, f64::min);
    let bp = r.pairs.iter().map(|p| p.high[1]).fold(f64::NEG_INFINITY, f64::max);
    (best - bp) / range
}

#[test]
#[ignore = "heavy record (about 150 s): ADR 0622 B1, B2, NC1, B4, B5; B6 is the fast test of the same path"]
fn b_reduced_order_fan_heavy() {
    let d = tmp("bheavy");
    let spec = s3_spec(2000, 699_301, 4);
    let cands = spec.candidate_points();
    let (lo, hm) = (fan_model(RomModel::default()), fan_model(perturbed()));
    let t = std::time::Instant::now();
    let named = |x: &Vec<f64>| -> Vec<Quantity> {
        spec.params
            .iter()
            .zip(x)
            .map(|(p, &v)| Quantity::new(&p.name, v, &p.unit))
            .collect()
    };
    let lo_out: Vec<Outcome> = parallel_map(&cands, 4, &|x: &Vec<f64>| lo.evaluate(&named(x)));
    let hi_out: Vec<Outcome> = parallel_map(&cands, 4, &|x: &Vec<f64>| hm.evaluate(&named(x)));
    eprintln!("B truth: 4000 evaluations in {:.1} s", t.elapsed().as_secs_f64());
    let key = |x: &Vec<f64>| x.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    let lc: Cache = cands.iter().zip(&lo_out).map(|(x, o)| (key(x), o.clone())).collect();
    let hc: Cache = cands.iter().zip(&hi_out).map(|(x, o)| (key(x), o.clone())).collect();
    let lo_c = cached(FAN_ROM_MODEL_ID, FAN_ROM_MODEL_VERSION, lc);
    let hi_m = cached("synthetic.fan_rom_perturbed", "1", hc);
    let hs = ModelSource {
        level: Some("perturbed-losses".into()),
        ..ModelSource::of(&hi_m)
    };
    let hv = |m: &str| -> Vec<Option<f64>> {
        hi_out
            .iter()
            .map(|o| (o.verdict == Verdict::Ok).then(|| o.metrics.iter().find(|q| q.name == m).unwrap().value))
            .collect()
    };
    let (h_dp, h_eta) = (hv("dp_s"), hv("eta_s"));
    let path = d.join("rule.jsonl");
    let r = fidelity::run(&spec, &lo_c, &hs, &ledger_at(&path)).unwrap();
    s3_counts(&path, 2000, &r);
    // B1
    let held: Vec<usize> = (0..cands.len())
        .filter(|&i| {
            r.low[i].is_some() && h_eta[i].is_some() && !r.rounds.iter().any(|rd| rd.picks.iter().any(|p| p.0 == i))
        })
        .collect();
    let mut ratios = Vec::new();
    for (m, h) in [(0, &h_dp), (1, &h_eta)] {
        let rm = |f: &dyn Fn(usize) -> f64| {
            (held.iter().map(|&i| (f(i) - h[i].unwrap()).powi(2)).sum::<f64>() / held.len() as f64).sqrt()
        };
        let c = rm(&|i| r.final_predictions[i][m]);
        let u = rm(&|i| r.low[i].as_ref().unwrap()[m]);
        eprintln!(
            "B1 metric {m}: held-out {} RMSE corrected {c:.4e} uncorrected {u:.4e} ratio {:.4}",
            held.len(),
            c / u
        );
        ratios.push(c / u);
    }
    let fits = &r.correction.as_ref().unwrap().fits;
    for f in fits {
        eprintln!(
            "  final {}: rung {} trusted {} constant {} ({}) n {}",
            f.metric,
            f.rung.as_str(),
            f.trusted,
            f.constant_trusted,
            f.why,
            f.n
        );
    }
    let ext = held.iter().filter(|&&i| r.final_extrapolated[i][1]).count();
    eprintln!("  held-out flagged extrapolated: {ext} of {}", held.len());
    // B2, NC1, correction off
    let g = gap(&r, &h_eta);
    let rr = fidelity::run(
        &FidelitySpec {
            promotion: Promotion::Random { seed: 699_302 },
            ..spec.clone()
        },
        &lo_c,
        &hs,
        &ledger_at(&d.join("random.jsonl")),
    )
    .unwrap();
    let g_rand = gap(&rr, &h_eta);
    let ro = fidelity::run(
        &FidelitySpec {
            correct: false,
            ..spec.clone()
        },
        &lo_c,
        &hs,
        &ledger_at(&d.join("off.jsonl")),
    )
    .unwrap();
    let g_off = gap(&ro, &h_eta);
    eprintln!("B2 gap {g:.4}; NC1 random gap {g_rand:.4}; correction off gap {g_off:.4} (recorded)");
    // B4
    let mean_abs = |round: usize| {
        let v: Vec<f64> = r
            .pairs
            .iter()
            .filter(|p| p.round == round)
            .map(|p| (p.high[1] - p.pred[1]).abs())
            .collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    let b4 = mean_abs(4) / mean_abs(1);
    eprintln!(
        "B4: round 4 {:.4e} / round 1 {:.4e} = {b4:.4}",
        mean_abs(4),
        mean_abs(1)
    );
    for rd in &r.rounds {
        eprintln!("  round picks {:?} failed {:?}", rd.picks, rd.failed_high);
    }
    eprintln!("B total time {:.1} s", t.elapsed().as_secs_f64());
    assert!(ratios[0] <= 0.5 && ratios[1] <= 0.5, "B1");
    assert!(g <= 0.02, "B2");
    assert!(g_rand > 0.02, "NC1 must miss");
    assert!(b4 <= 0.75, "B4");
}
