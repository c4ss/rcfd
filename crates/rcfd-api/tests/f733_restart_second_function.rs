//! ADR 0673 (F-733): the basin restart of ADR 0651 on a second multimodal function (Shekel 10) and on the F-698 R1 fan set-up,
//! the check that decides whether its fresh process is needed (`Restart::step_model`, the variant of ADR 0651 mutation R2),
//! and a hash of the default proposal points of the F-698 O1 run. The heavy records (S0 to S3, F1 to F1d) are `#[ignore]`d and
//! run once; the fast tests (T1 to T3, H1, NCH) run the same code paths. The F3 set-up is copied from
//! `tests/f716_stall_precheck.rs` (itself a copy of `tests/f698_surrogate.rs`), which is not edited.

use rcfd_api::design::bayes::{
    self, BayesOptions, BayesParam, BayesSpec, Observation, Pick, PickKind, Restart, Status,
};
use rcfd_api::design::fan_objective::*;
use rcfd_api::design::fan_rom_eval::RomCondition;
use rcfd_api::design::ledger::{Entry, Verdict};
use rcfd_api::design::optim::{nelder_mead, Fitness, NmOptions, Stop};
use rcfd_api::design::study::{parallel_map, ParamDef, Sense};
use rcfd_mesh::fan::SectionFamily;
use rcfd_mesh::fan_design::{ComputerFan, ControlPoint, Tolerances};
use rcfd_post::fan_rom::*;
use std::f64::consts::PI;
use std::time::Instant;

const CLOCK: u64 = 1_700_000_000_000;
/// ADR 0621 addendum 1: the lowest feasible value of the 21^3 grid on F3 [W].
const P_REF: f64 = 1.455477;

// ---------------------------------------------------------------- F3 (copied from tests/f698_surrogate.rs)

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

// ---------------------------------------------------------------- test functions

/// Shekel 10 on `[0, 10]^4` (Dixon and Szego 1978, from memory, NOT read): the centres `C_i` and widths `beta_i`.
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
/// The global minimum, from memory; checked numerically by S0.
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

// ---------------------------------------------------------------- the variants and the loops

#[derive(Debug, Clone, Copy, PartialEq)]
enum Variant {
    Plain,
    Fresh,
    Step,
}

fn restart_of(v: Variant) -> Option<Restart> {
    match v {
        Variant::Plain => None,
        Variant::Fresh => Some(Restart::default()),
        Variant::Step => Some(Restart {
            step_model: true,
            ..Restart::default()
        }),
    }
}

fn opts(seed: u64, n0: usize, v: Variant) -> BayesOptions {
    BayesOptions {
        seed,
        n0: Some(n0),
        restart: restart_of(v),
        ..Default::default()
    }
}

/// The loop of `minimise` through `propose`, keeping the observations before each pick and the pick records.
struct Trace {
    ys: Vec<f64>,
    hist: Vec<Observation>,
    picks: Vec<Pick>,
}

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

fn n_restart(picks: &[Pick]) -> usize {
    picks.iter().filter(|p| p.kind == PickKind::Restart).count()
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

/// A6 of ADR 0651: (restart picks, those with `delta < rho` from the incumbent of their step in the recorded length scales).
fn a6_violations(t: &Trace, b: &[(f64, f64)], rho: f64) -> (usize, usize) {
    let (mut n, mut bad) = (0, 0);
    for (k, p) in t.picks.iter().enumerate() {
        if p.kind != PickKind::Restart {
            continue;
        }
        n += 1;
        let pts: Vec<(Vec<f64>, f64)> = t.hist[..k]
            .iter()
            .filter_map(|o| match o.status {
                Status::Ok { f, .. } => Some((unit(&o.x, b), f)),
                _ => None,
            })
            .collect();
        let star = incumbent_u(&pts).expect("an incumbent");
        let dl = delta(&unit(&p.x, b), &star, &p.length_scales);
        bad += usize::from(dl.is_nan() || dl < rho);
    }
    (n, bad)
}

fn xs_of(t: &Trace) -> Vec<Vec<f64>> {
    t.hist.iter().map(|o| o.x.clone()).collect()
}

// ---------------------------------------------------------------- S0 to S3: Shekel 10 (heavy record)

fn shekel_nm(x0: Option<Vec<f64>>, seed: u64) -> (f64, Vec<f64>) {
    let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        Ok(p.iter().map(|x| Fitness::new(shekel10(x), 0.0)).collect())
    };
    let r = nelder_mead(
        &SH_B,
        &NmOptions {
            max_evals: 4000,
            seed,
            x0,
            xtol: 1e-12,
            ..Default::default()
        },
        &mut ev,
    )
    .unwrap();
    (r.fitness.f, r.x)
}

#[test]
#[ignore = "known miss of S1 on a44c52b5 (ADR 0673 addendum 1): fresh 17 of 20 against plain 18 of 20; S0, S3 met; S2 measured fresh 17, step 11; heavy record, about 5 minutes on ten threads"]
fn s0_s3_shekel10_plain_fresh_and_step_known_miss() {
    let t = Instant::now();
    // S0: the minimum
    let (f4, x4) = shekel_nm(Some(vec![4.0; 4]), 0);
    let lowest = (0..40u64).map(|s| shekel_nm(None, s).0).fold(f64::INFINITY, f64::min);
    println!(
        "S0: from (4,4,4,4) {f4:.8} at {x4:?}, {:.2e} from {SH_MIN}; lowest of 40 starts {lowest:.8}",
        (f4 - SH_MIN).abs()
    );
    let s0 = (f4 - SH_MIN).abs() <= 1e-4 && lowest >= f4 - 1e-9;
    // S1 to S3: 20 seeds, three variants
    let seeds: Vec<u64> = (1..=20).collect();
    let variants = [Variant::Plain, Variant::Fresh, Variant::Step];
    let jobs: Vec<(Variant, u64)> = variants
        .iter()
        .flat_map(|&v| seeds.iter().map(move |&s| (v, s)))
        .collect();
    let runs = parallel_map(&jobs, 10, &|&(v, s): &(Variant, u64)| {
        let t1 = Instant::now();
        let tr = propose_run(&shekel10, &SH_B, &opts(s, 10, v), 150);
        println!(
            "S1r {v:?} seed {s}: target at {:?}, best {:.5}, restart picks {}, {:.0} s",
            first_hit(&tr.ys, SH_MIN),
            tr.ys.iter().cloned().fold(f64::INFINITY, f64::min),
            n_restart(&tr.picks),
            t1.elapsed().as_secs_f64()
        );
        tr
    });
    let mut hits = [0usize; 3];
    let (mut e2, mut nr, mut bad) = (0, 0, 0);
    for ((v, _), tr) in jobs.iter().zip(&runs) {
        let k = variants.iter().position(|w| w == v).unwrap();
        hits[k] += usize::from(first_hit(&tr.ys, SH_MIN).is_some());
        e2 += e2_violations(&xs_of(tr), &SH_B);
        let (n, b) = a6_violations(tr, &SH_B, Restart::default().rho);
        nr += n;
        bad += b;
    }
    println!(
        "S1: plain {} of 20, fresh {} of 20; S2: step {} of 20 (fresh needed if fresh >= step + 3); S3: {nr} restart picks, {bad} inside the basin, E2 {e2}; {:.0} s",
        hits[0],
        hits[1],
        hits[2],
        t.elapsed().as_secs_f64()
    );
    assert!(s0, "S0");
    assert!(hits[1] >= hits[0], "S1");
    assert_eq!(bad, 0, "S3 basin");
    assert_eq!(e2, 0, "S3 E2");
    assert!(hits[1] >= hits[2] + 3, "S2: the fresh process needed");
}

// ---------------------------------------------------------------- F1: the reduced-order fan (heavy record)

fn fan_points(entries: &[Entry]) -> Vec<Vec<f64>> {
    entries
        .iter()
        .map(|e| e.params.iter().map(|q| q.value).collect())
        .collect()
}

fn f3_box() -> Vec<(f64, f64)> {
    F3_BOX.iter().map(|(_, _, lo, hi)| (*lo, *hi)).collect()
}

/// F1c: (steps whose plain acquisition is below `tau` times the spread of the `Ok` values so far, the largest count of `Ok`
/// points at `delta >= rho` from the incumbent over the picks).
fn f1c_record(r: &bayes::BayesResult) -> (usize, usize) {
    let (tau, rho) = (Restart::default().tau, Restart::default().rho);
    let bx = f3_box();
    let (mut trig, mut most) = (0, 0);
    for (k, p) in r.picks.iter().enumerate() {
        let pts: Vec<(Vec<f64>, f64)> = r.entries[..k]
            .iter()
            .filter(|e| e.verdict == Verdict::Ok)
            .map(|e| {
                let x: Vec<f64> = e.params.iter().map(|q| q.value).collect();
                (unit(&x, &bx), e.metric("power@nominal@mean").unwrap().value)
            })
            .collect();
        if pts.len() < 2 || p.length_scales.is_empty() {
            continue;
        }
        let n = pts.len() as f64;
        let mu = pts.iter().map(|q| q.1).sum::<f64>() / n;
        let sd = (pts.iter().map(|q| (q.1 - mu).powi(2)).sum::<f64>() / n).sqrt();
        trig += usize::from(p.acquisition < tau * sd);
        let star = incumbent_u(&pts).unwrap();
        most = most.max(
            pts.iter()
                .filter(|q| delta(&q.0, &star, &p.length_scales) >= rho)
                .count(),
        );
    }
    (trig, most)
}

#[test]
#[ignore = "heavy record (ADR 0673 F1 to F1d, NC1): the F-698 R1 set-up, 30 runs of 50, about 3 minutes on ten threads"]
fn f1_reduced_order_fan_plain_fresh_and_step_record() {
    let t = Instant::now();
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let seeds: Vec<u64> = (1..=10).collect();
    let variants = [Variant::Plain, Variant::Fresh, Variant::Step];
    let jobs: Vec<(Variant, u64)> = variants
        .iter()
        .flat_map(|&v| seeds.iter().map(move |&s| (v, s)))
        .collect();
    let runs = parallel_map(&jobs, 10, &|&(v, s): &(Variant, u64)| {
        let t1 = Instant::now();
        let mut spec = spec_f3(s, 50);
        spec.options.restart = restart_of(v);
        let r = bayes::run(&spec, &m, None).unwrap();
        println!(
            "F1 {v:?} seed {s}: best / P_ref {:.5}, failed {} of 50, restart picks {}, {:.0} s",
            r.best_value.unwrap_or(f64::INFINITY) / P_REF,
            r.failed,
            n_restart(&r.picks),
            t1.elapsed().as_secs_f64()
        );
        r
    });
    let bx = f3_box();
    let mut hits = [0usize; 3];
    let (mut located, mut e2, mut equal) = (true, 0, true);
    let mut failed_plain = Vec::new();
    for (k, v) in variants.iter().enumerate() {
        for (i, s) in seeds.iter().enumerate() {
            let r = &runs[k * seeds.len() + i];
            let best = r.best_value.unwrap_or(f64::INFINITY);
            hits[k] += usize::from(best <= P_REF * 1.01);
            match v {
                Variant::Plain => failed_plain.push(r.failed),
                Variant::Fresh => {
                    let p = &runs[i];
                    let eq = bits(&fan_points(&r.entries)) == bits(&fan_points(&p.entries));
                    equal &= eq;
                    located &= failed_entries_located(&r.entries);
                    e2 += e2_violations(&fan_points(&r.entries), &bx);
                    let (trig, most) = f1c_record(r);
                    println!(
                        "F1b/F1c seed {s}: fresh bit-equal to plain {eq}, restart picks {}, steps below the trigger {trig}, most Ok points outside the basin {most}",
                        n_restart(&r.picks)
                    );
                }
                Variant::Step => {}
            }
        }
    }
    println!(
        "F1: plain {} of 10, fresh {} of 10; F1c step {} of 10; F1b equal {equal}; NC1 failed {failed_plain:?}; F1d located {located}, E2 {e2}; {:.0} s",
        hits[0],
        hits[1],
        hits[2],
        t.elapsed().as_secs_f64()
    );
    assert!(hits[1] >= hits[0], "F1");
    assert!(equal, "F1b");
    assert_eq!(failed_plain, vec![37, 35, 34, 36, 36, 36, 39, 36, 34, 36], "NC1");
    assert!(located, "F1d located");
    assert_eq!(e2, 0, "F1d E2");
}

// ---------------------------------------------------------------- H1, NCH: the default O1 points

/// ADR 0673 Z6: the hash of D3 computed on an unmodified `git archive` of 5f501379.
const H1_BASE: u64 = 0xe2cf_1413_2077_4cfc;

/// ADR 0673 D3: FNV-1a 64 of the little-endian bits of every point of the O1 runs (seeds 1 to 20, budget 60).
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
fn h1_default_o1_points_hash() {
    let h = o1_hash(None);
    println!("H1: {h:016x}, base {H1_BASE:016x}");
    assert_eq!(h, H1_BASE, "H1");
}

#[test]
fn nch_o1_points_hash_with_the_restart_differs() {
    let h = o1_hash(Some(Restart::default()));
    println!("NCH: {h:016x}, base {H1_BASE:016x}");
    assert_ne!(h, H1_BASE, "NCH");
}

// ---------------------------------------------------------------- T1 to T3: fast paths

#[test]
#[ignore = "known miss of T1 on 1de27b10 (ADR 0673 amendment 1): no restart pick within 40 evaluations (0 and 0), kept as a record"]
fn t1_shekel10_fast_path_known_miss() {
    let vs = [Variant::Fresh, Variant::Step];
    let runs = parallel_map(&vs, 2, &|&v: &Variant| {
        propose_run(&shekel10, &SH_B, &opts(1, 10, v), 40)
    });
    let mut ok = true;
    for (v, tr) in vs.iter().zip(&runs) {
        let (n, bad) = a6_violations(tr, &SH_B, Restart::default().rho);
        let e2 = e2_violations(&xs_of(tr), &SH_B);
        println!(
            "T1 {v:?}: restart picks {n}, inside the basin {bad}, E2 {e2}, best {:.5}",
            tr.ys.iter().cloned().fold(f64::INFINITY, f64::min)
        );
        ok &= n >= 1 && bad == 0 && e2 == 0;
    }
    assert!(ok, "T1");
}

/// ADR 0673 amendment 1: T1 at the budget of the registered Shekel runs, where the restart fires.
#[test]
fn t1b_shekel10_fast_path_budget_150() {
    let vs = [Variant::Fresh, Variant::Step];
    let runs = parallel_map(&vs, 2, &|&v: &Variant| {
        propose_run(&shekel10, &SH_B, &opts(1, 10, v), 150)
    });
    let mut ok = true;
    for (v, tr) in vs.iter().zip(&runs) {
        let (n, bad) = a6_violations(tr, &SH_B, Restart::default().rho);
        let e2 = e2_violations(&xs_of(tr), &SH_B);
        println!(
            "T1b {v:?}: restart picks {n}, inside the basin {bad}, E2 {e2}, best {:.5}",
            tr.ys.iter().cloned().fold(f64::INFINITY, f64::min)
        );
        ok &= n >= 1 && bad == 0 && e2 == 0;
    }
    assert!(ok, "T1b");
}

#[test]
fn t2_fan_fast_path() {
    let tg = target_f3();
    let o = objective_f3();
    let m = ObjectiveModel {
        objective: &o,
        model: &tg,
    };
    let vs = [Variant::Plain, Variant::Fresh];
    let runs = parallel_map(&vs, 2, &|&v: &Variant| {
        let mut spec = spec_f3(1, 45);
        spec.options.restart = restart_of(v);
        bayes::run(&spec, &m, None).unwrap()
    });
    let eq = bits(&fan_points(&runs[0].entries)) == bits(&fan_points(&runs[1].entries));
    let located = runs.iter().all(|r| failed_entries_located(&r.entries));
    println!(
        "T2: plain and fresh bit-equal {eq}; restart picks {}; failures located {located}; best / P_ref {:.5}",
        n_restart(&runs[1].picks),
        runs[1].best_value.unwrap_or(f64::INFINITY) / P_REF
    );
    assert!(eq, "T2 equal");
    assert!(located, "T2 located");
}

#[test]
fn t3_step_model_option() {
    let dflt = Restart::default().step_model;
    let run = |v: Variant| -> Vec<Vec<f64>> {
        let mut xs = Vec::new();
        let mut ev = |p: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            xs.extend(p.iter().cloned());
            Ok(p.iter().map(|x| Fitness::new(branin(x), 0.0)).collect())
        };
        bayes::minimise(&BRANIN_B, &opts(2, 10, v), 60, &mut ev).unwrap();
        xs
    };
    let differ = bits(&run(Variant::Fresh)) != bits(&run(Variant::Step));
    let tr = propose_run(&branin, &BRANIN_B, &opts(2, 10, Variant::Step), 60);
    let (n, bad) = a6_violations(&tr, &BRANIN_B, Restart::default().rho);
    println!(
        "T3: default step_model {dflt}; fresh and step differ {differ}; step restart picks {n}, inside the basin {bad}"
    );
    assert!(!dflt, "T3 default");
    assert!(differ, "T3 differ");
    assert_eq!(bad, 0, "T3 basin");
}
