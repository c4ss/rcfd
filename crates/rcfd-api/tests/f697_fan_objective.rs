//! ADR 0620 (F-697): the fan objective layer over impedance points and tolerances. Checks C (closed-form objective), T
//! (tolerance statistics), K (constraints), W (determinism), R (reduced-order fan), N (noise metrics), O (optimisers through
//! the layer), as registered in ADR 0620 amendment 1. Set-ups L, G, F and U are those of the registration.

use rcfd_api::design::fan_objective::*;
use rcfd_api::design::fan_rom_eval::{FanRomEvaluator, RomCondition};
use rcfd_api::design::ledger::{self, Fidelity, Ledger};
use rcfd_api::design::optim::{Fitness, OptResult};
use rcfd_api::design::rng::Rng;
use rcfd_api::design::store::{Record, Store};
use rcfd_api::design::study::{Cmp, Doe, Method, ParamDef, Robust, Sense, Study};
use rcfd_mesh::fan::SectionFamily;
use rcfd_mesh::fan_design::{ComputerFan, ControlPoint, Dist, Tolerances};
use rcfd_post::acoustics::{a_weighting_db, spl_db};
use rcfd_post::fan_rom::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

const CLOCK: u64 = 1_700_000_000_000;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn tn(sd: f64) -> Dist {
    Dist::TruncatedNormal {
        mean: 0.0,
        sd,
        lo: -10.0 * sd,
        hi: 10.0 * sd,
    }
}

fn draws(sd: f64, n: usize, seed: u64) -> Vec<f64> {
    let d = tn(sd);
    let mut rng = Rng::new(seed);
    (0..n).map(|_| d.sample(&mut rng)).collect()
}

// ---------------------------------------------------------------- set-up L: the linear fan

struct LinearFan {
    sigma_s: f64,
}

fn eta(t: f64) -> f64 {
    0.5 - (t - 0.3) * (t - 0.3)
}

impl SpecimenModel for LinearFan {
    type Specimen = (f64, f64);
    fn id(&self) -> &str {
        "analytic.linear_fan"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn metrics(&self) -> Vec<(String, String)> {
        [("q", "m3/s"), ("dp", "Pa"), ("power", "W"), ("neg_q", "m3/s")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }
    fn describe(&self) -> Value {
        json!({"sigma_s": self.sigma_s})
    }
    fn nominal(&self, x: &[f64]) -> Result<(f64, f64), String> {
        Ok((x[0], x[1]))
    }
    fn perturbed(&self, x: &[f64], n: usize, seed: u64) -> Result<Vec<(f64, f64)>, String> {
        Ok(draws(self.sigma_s, n, seed)
            .into_iter()
            .map(|e| (x[0] * (1.0 + e), x[1]))
            .collect())
    }
    fn evaluate(&self, spec: &(f64, f64), point: &ImpedancePoint, _: &BTreeSet<String>) -> Result<Metrics, String> {
        let (s, t) = *spec;
        let qs = 0.005 * s;
        let fan = |q: f64| -> Result<f64, String> {
            Ok(if q >= qs {
                100.0 * s * s * (1.0 - q / (0.05 * s))
            } else {
                90.0 * s * s * q / qs
            })
        };
        let q_hi = 0.1 * s;
        let p = operating_point(&fan, &point.system, 1e-3 * q_hi, q_hi, 100)?;
        Ok([
            ("q", p.q),
            ("dp", p.dp),
            ("power", p.q * p.dp / eta(t)),
            ("neg_q", -p.q),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), *b))
        .collect())
    }
}

fn pt(label: &str, k: f64, weight: f64) -> ImpedancePoint {
    ImpedancePoint {
        label: label.into(),
        system: SystemCurve { k, l: 0.0 },
        weight,
    }
}

fn lin_points() -> Vec<ImpedancePoint> {
    vec![
        pt("I1", 75.0 / (0.0125 * 0.0125), 0.5),
        pt("I2", 50.0 / (0.025 * 0.025), 0.3),
        pt("I3", 25.0 / (0.0375 * 0.0375), 0.2),
    ]
}

fn lin_params() -> Vec<(ParamDef, String)> {
    vec![
        (
            ParamDef {
                name: "s".into(),
                lo: 0.5,
                hi: 2.0,
                init: Some(1.5),
                targets: vec![],
            },
            "1".into(),
        ),
        (
            ParamDef {
                name: "t".into(),
                lo: 0.0,
                hi: 1.0,
                init: Some(0.6),
                targets: vec![],
            },
            "1".into(),
        ),
    ]
}

fn term(metric: &str, sense: Sense, tol: TolStat, over: Over) -> Term {
    Term {
        metric: metric.into(),
        sense,
        tol,
        over,
        weight: 1.0,
    }
}

fn limit(metric: &str, cmp: Cmp, value: f64, scale: f64, tol: TolStat, over: Option<Over>) -> Limit {
    Limit {
        metric: metric.into(),
        cmp,
        limit: value,
        scale,
        tol,
        over,
    }
}

fn objective(
    points: Vec<ImpedancePoint>,
    terms: Vec<Term>,
    limits: Vec<Limit>,
    plan: Option<TolerancePlan>,
    workers: usize,
) -> FanObjective {
    FanObjective {
        name: "f697".into(),
        params: lin_params(),
        points,
        terms,
        limits,
        plan,
        workers,
    }
}

fn plan(n: usize, seed: u64) -> Option<TolerancePlan> {
    Some(TolerancePlan {
        n,
        seed,
        max_failed_fraction: 0.0,
    })
}

/// Evaluate the candidates `xs` in one batch of a fresh study; the records in request order, objective vectors, violations.
struct Eval {
    recs: Vec<Record>,
    obj: Vec<Option<Vec<f64>>>,
    viol: Vec<f64>,
    fit: Vec<Fitness>,
}

fn eval<M: SpecimenModel>(o: &FanObjective, m: &M, xs: &[Vec<f64>]) -> Eval {
    o.validate(m).expect("valid objective");
    let ev = TolerantEvaluator { objective: o, model: m };
    let mut s = Study::new(o.study_def(), Store::memory("f697"), &ev).unwrap();
    let pos = s.eval_points(xs, "evaluated").unwrap();
    let recs: Vec<Record> = pos.iter().map(|&p| s.store.records()[p].clone()).collect();
    Eval {
        obj: recs.iter().map(|r| s.objective_vector(r)).collect(),
        viol: recs.iter().map(|r| s.violation_of(r)).collect(),
        fit: recs.iter().map(|r| s.fitness_of(r)).collect(),
        recs,
    }
}

fn point_values(r: &Record, name: &str) -> Vec<f64> {
    r.conditions.iter().map(|c| c.metrics[name]).collect()
}

fn closed_q(k: f64) -> f64 {
    let (p0, q0) = (100.0, 0.05);
    (-p0 / q0 + ((p0 / q0) * (p0 / q0) + 4.0 * k * p0).sqrt()) / (2.0 * k)
}

const X0: [f64; 2] = [1.0, 0.3];

#[test]
fn c1_c4_closed_form_objective() {
    let m = LinearFan { sigma_s: 0.02 };
    let overs = [Over::Mean, Over::Worst, Over::Weighted, Over::MeanPlusKSigma(1.0)];
    let mut terms: Vec<Term> = overs
        .iter()
        .map(|&o| term("q", Sense::Max, TolStat::Nominal, o))
        .collect();
    terms.push(term("power", Sense::Min, TolStat::Nominal, Over::Mean));
    terms.push(term("power", Sense::Min, TolStat::Nominal, Over::Worst));
    let o = objective(lin_points(), terms, vec![], None, 1);
    let e = eval(&o, &m, &[X0.to_vec()]);
    let r = &e.recs[0];
    let ov = e.obj[0].clone().unwrap();
    // C1
    let q = point_values(r, "q@nominal");
    for (p, &qi) in lin_points().iter().zip(&q) {
        let c = closed_q(p.system.k);
        println!(
            "C1 {}: q {qi:.17e}, closed form {c:.17e}, rel {:.3e}",
            p.label,
            rel(qi, c)
        );
        assert!(rel(qi, c) <= 1e-9, "C1 {}", p.label);
    }
    // C2: Robust::apply by hand on the record's per-point values (minimised: -q)
    let neg: Vec<f64> = q.iter().map(|v| -v).collect();
    let hand = [
        Robust::Mean.apply(&neg),
        Robust::Worst.apply(&neg),
        Robust::Weighted(vec![0.5, 0.3, 0.2]).apply(&neg),
        Robust::MeanPlusKSigma(1.0).apply(&neg),
    ];
    let closed = [0.025, 0.0125, 0.02125, 0.025 - 0.0125 * (2.0f64 / 3.0).sqrt()];
    for i in 0..4 {
        println!(
            "C2/C3 {:?}: layer {:.17e}, hand {:.17e} (rel {:.3e}), closed {:.17e} (rel {:.3e})",
            overs[i],
            -ov[i],
            -hand[i],
            rel(ov[i], hand[i]),
            closed[i],
            rel(-ov[i], closed[i])
        );
        assert!(rel(ov[i], hand[i]) <= 1e-12, "C2 {:?}", overs[i]);
        assert!(rel(-ov[i], closed[i]) <= 1e-9, "C3 {:?}", overs[i]);
    }
    // C4
    println!(
        "C4 mean power {:.17e} (rel {:.3e}), worst {:.17e} (rel {:.3e})",
        ov[4],
        rel(ov[4], 25.0 / 12.0),
        ov[5],
        rel(ov[5], 2.5)
    );
    assert!(rel(ov[4], 25.0 / 12.0) <= 1e-9, "C4 mean");
    assert!(rel(ov[5], 2.5) <= 1e-9, "C4 worst");
}

#[test]
fn c5_max_and_min_of_the_negated_metric_agree() {
    let m = LinearFan { sigma_s: 0.02 };
    let o = objective(
        lin_points(),
        vec![
            term("q", Sense::Max, TolStat::Nominal, Over::Weighted),
            term("neg_q", Sense::Min, TolStat::Nominal, Over::Weighted),
        ],
        vec![],
        None,
        1,
    );
    let ov = eval(&o, &m, &[X0.to_vec()]).obj[0].clone().unwrap();
    println!("C5: {:.17e} {:.17e}", ov[0], ov[1]);
    assert_eq!(ov[0].to_bits(), ov[1].to_bits(), "C5");
}

#[test]
fn c6_negative_controls() {
    let m = LinearFan { sigma_s: 0.02 };
    let terms = vec![
        term("q", Sense::Max, TolStat::Nominal, Over::Mean),
        term("q", Sense::Max, TolStat::Nominal, Over::Weighted),
    ];
    let base = eval(
        &objective(lin_points(), terms.clone(), vec![], None, 1),
        &m,
        &[X0.to_vec()],
    )
    .obj[0]
        .clone()
        .unwrap();
    let mut p2 = lin_points();
    p2[1].system.k *= 2.0;
    let k2 = eval(&objective(p2, terms.clone(), vec![], None, 1), &m, &[X0.to_vec()]).obj[0]
        .clone()
        .unwrap();
    let mut pw = lin_points();
    pw[0].weight = 0.2;
    pw[2].weight = 0.5;
    let w = eval(&objective(pw, terms, vec![], None, 1), &m, &[X0.to_vec()]).obj[0]
        .clone()
        .unwrap();
    println!(
        "C6: mean flow change {:.3e} (k of I2 doubled); weighted flow change {:.3e} (weights 0.2, 0.3, 0.5)",
        rel(k2[0], base[0]),
        rel(w[1], base[1])
    );
    assert!(rel(k2[0], base[0]) > 1e-3, "C6 k");
    assert!(rel(w[1], base[1]) > 1e-3, "C6 weights");
}

// ---------------------------------------------------------------- T: tolerance statistics

struct Gauss {
    sigma: f64,
}

impl SpecimenModel for Gauss {
    type Specimen = f64;
    fn id(&self) -> &str {
        "analytic.gauss"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn metrics(&self) -> Vec<(String, String)> {
        vec![("m".into(), "1".into())]
    }
    fn describe(&self) -> Value {
        json!({"sigma": self.sigma})
    }
    fn nominal(&self, _: &[f64]) -> Result<f64, String> {
        Ok(0.0)
    }
    fn perturbed(&self, _: &[f64], n: usize, seed: u64) -> Result<Vec<f64>, String> {
        Ok(draws(self.sigma, n, seed))
    }
    fn evaluate(&self, xi: &f64, _: &ImpedancePoint, _: &BTreeSet<String>) -> Result<Metrics, String> {
        Ok([("m".to_string(), 2.0 - 3.0 * xi)].into_iter().collect())
    }
}

#[test]
fn t1_linear_metric_in_one_gaussian_tolerance() {
    let (a, b, sigma, n, k) = (2.0, -3.0f64, 0.1, 4096usize, 2.0);
    let o = objective(
        vec![pt("P", 1.0, 1.0)],
        vec![
            term("m", Sense::Min, TolStat::Mean, Over::Mean),
            term("m", Sense::Min, TolStat::MeanPlusKSigma(k), Over::Mean),
            term("m", Sense::Max, TolStat::MeanPlusKSigma(k), Over::Mean),
        ],
        vec![],
        plan(n, 697_001),
        4,
    );
    let ov = eval(&o, &Gauss { sigma }, &[X0.to_vec()]).obj[0].clone().unwrap();
    let nf = n as f64;
    let se_m = b.abs() * sigma / nf.sqrt();
    let se_k = b.abs() * sigma * (1.0 / nf + k * k / (2.0 * (nf - 1.0))).sqrt();
    let (mean, up, down) = (ov[0], ov[1], -ov[2]);
    let z = [
        (mean - a) / se_m,
        (up - (a + k * b.abs() * sigma)) / se_k,
        (down - (a - k * b.abs() * sigma)) / se_k,
    ];
    println!(
        "T1: mean {mean:.6} (z {:.3}), mean+2sd {up:.6} (z {:.3}), mean-2sd {down:.6} (z {:.3})",
        z[0], z[1], z[2]
    );
    for (i, zi) in z.iter().enumerate() {
        assert!(zi.abs() <= 4.0, "T1 part {i}: |z| {zi}");
    }
}

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

fn target(fan: ComputerFan, tolerances: Tolerances) -> FanRomTarget {
    FanRomTarget {
        geometry: Box::new(move |x: &[f64]| {
            let mut f = fan.clone();
            for s in &mut f.stations {
                s.stagger_deg += x[0];
            }
            Ok(f)
        }),
        tolerances,
        model: RomModel::default(),
        base: base_cond(),
    }
}

fn rom_params() -> Vec<(ParamDef, String)> {
    vec![(
        ParamDef {
            name: "stagger_offset_deg".into(),
            lo: -3.0,
            hi: 3.0,
            init: Some(0.0),
            targets: vec![],
        },
        "deg".into(),
    )]
}

fn rom_objective(
    points: Vec<ImpedancePoint>,
    terms: Vec<Term>,
    plan: Option<TolerancePlan>,
    workers: usize,
) -> FanObjective {
    FanObjective {
        name: "f697_rom".into(),
        params: rom_params(),
        points,
        terms,
        limits: vec![],
        plan,
        workers,
    }
}

#[test]
fn t2_t4_static_unbalance_of_blade_mass_errors() {
    let (sigma, n) = (0.01, 4096usize);
    let tol = Tolerances {
        mass_rel: tn(sigma),
        ..Default::default()
    };
    let m = target(ComputerFan::default(), tol);
    let o = rom_objective(
        vec![pt("P", 1.0e4, 1.0)],
        vec![term("unbalance", Sense::Min, TolStat::Mean, Over::Mean)],
        plan(n, 697_002),
        4,
    );
    let t0 = std::time::Instant::now();
    let ov = eval(&o, &m, &[vec![0.0]]).obj[0].clone().unwrap();
    let ev = TolerantEvaluator {
        objective: &o,
        model: &m,
    };
    let table = ev.specimen_table(&[0.0]).unwrap();
    println!("T2: {n} specimens twice in {:.1} s", t0.elapsed().as_secs_f64());
    let nominal = ComputerFan::default();
    let (mb, c) = nominal.blade_mass(0).unwrap();
    let rc = c[0].hypot(c[1]);
    let u: Vec<f64> = table[1..].iter().map(|r| r[0].as_ref().unwrap()["unbalance"]).collect();
    let nf = n as f64;
    let sd_of = |v: &[f64]| {
        let mu = v.iter().sum::<f64>() / nf;
        (v.iter().map(|x| (x - mu) * (x - mu)).sum::<f64>() / (nf - 1.0)).sqrt()
    };
    let expect = rc * mb * sigma * (7.0 * std::f64::consts::PI / 4.0).sqrt();
    let z1 = (ov[0] - expect) / (sd_of(&u) / nf.sqrt());
    let u2: Vec<f64> = u.iter().map(|v| v * v).collect();
    let mean2 = u2.iter().sum::<f64>() / nf;
    let expect2 = 7.0 * (mb * rc * sigma).powi(2);
    let z2 = (mean2 - expect2) / (sd_of(&u2) / nf.sqrt());
    println!(
        "T2: mean |U| {:.6e} kg m against {expect:.6e} (z {z1:.3}); mean |U|^2 {mean2:.6e} against {expect2:.6e} (z {z2:.3}); m {mb:.6e} kg, r_c {rc:.6e} m",
        ov[0]
    );
    // T3
    let w = op3000().omega();
    let mut worst: f64 = 0.0;
    for r in &table[1..] {
        let mm = r[0].as_ref().unwrap();
        worst = worst.max(rel(mm["unbalance_force"], mm["unbalance"] * w * w));
    }
    println!("T3: largest relative difference of unbalance_force from |U| Omega^2: {worst:.3e}");
    // T4
    let u0 = table[0][0].as_ref().unwrap()["unbalance"];
    let t4 = u0 / (7.0 * mb * rc);
    println!("T4: nominal |U| {u0:.3e} kg m, |U| / (Z m r_c) = {t4:.3e}");
    assert!(z1.abs() <= 4.0, "T2 mean |U|: z {z1}");
    assert!(z2.abs() <= 4.0, "T2 mean |U|^2: z {z2}");
    assert!(worst <= 1e-14, "T3");
    assert!(t4 <= 1e-12, "T4");
}

fn lin_plan_objective(terms: Vec<Term>, seed: u64, workers: usize) -> FanObjective {
    objective(lin_points(), terms, vec![], plan(64, seed), workers)
}

#[test]
fn t5_mean_on_both_levels_is_the_mean_over_all_pairs() {
    let m = LinearFan { sigma_s: 0.02 };
    let o = lin_plan_objective(vec![term("q", Sense::Max, TolStat::Mean, Over::Mean)], 697_003, 2);
    let ov = eval(&o, &m, &[X0.to_vec()]).obj[0].clone().unwrap();
    let ev = TolerantEvaluator {
        objective: &o,
        model: &m,
    };
    let table = ev.specimen_table(&X0).unwrap();
    let all: Vec<f64> = table[1..]
        .iter()
        .flat_map(|r| r.iter().map(|c| c.as_ref().unwrap()["q"]))
        .collect();
    assert_eq!(all.len(), 192);
    let hand = all.iter().sum::<f64>() / 192.0;
    println!(
        "T5: layer {:.17e}, all pairs {hand:.17e}, rel {:.3e}",
        -ov[0],
        rel(-ov[0], hand)
    );
    assert!(rel(-ov[0], hand) <= 1e-12, "T5");
}

fn bits_of(r: &Record) -> Vec<(String, String, u64)> {
    r.conditions
        .iter()
        .flat_map(|c| c.metrics.iter().map(|(k, v)| (c.label.clone(), k.clone(), v.to_bits())))
        .collect()
}

#[test]
fn t6_common_random_numbers() {
    let m = LinearFan { sigma_s: 0.02 };
    let o = lin_plan_objective(vec![term("q", Sense::Max, TolStat::Mean, Over::Mean)], 697_003, 2);
    let (a, b) = (vec![1.0, 0.3], vec![1.5, 0.3]);
    let ra = eval(&o, &m, std::slice::from_ref(&a)).recs.remove(0);
    let rb = eval(&o, &m, std::slice::from_ref(&b)).recs.remove(0);
    let ab = eval(&o, &m, &[a.clone(), b.clone()]).recs;
    let ba = eval(&o, &m, &[b.clone(), a.clone()]).recs;
    let same = bits_of(&ra) == bits_of(&ab[0])
        && bits_of(&ra) == bits_of(&ba[1])
        && bits_of(&rb) == bits_of(&ab[1])
        && bits_of(&rb) == bits_of(&ba[0]);
    let qa = point_values(&ra, "q@mean");
    let qb = point_values(&rb, "q@mean");
    let worst = qa.iter().zip(&qb).map(|(x, y)| rel(y / 1.5, *x)).fold(0.0, f64::max);
    println!("T6: (a) alone and batched bit-equal: {same}; (b) largest rel |q@mean(B)/1.5 - q@mean(A)| {worst:.3e}");
    assert!(same, "T6 (a)");
    assert!(worst <= 1e-13, "T6 (b)");
}

#[test]
fn t7_nominal_statistic_ignores_the_plan() {
    let m = LinearFan { sigma_s: 0.02 };
    let terms = vec![
        term("q", Sense::Max, TolStat::Nominal, Over::Mean),
        term("power", Sense::Min, TolStat::Nominal, Over::Worst),
    ];
    let with = eval(&lin_plan_objective(terms.clone(), 697_003, 2), &m, &[X0.to_vec()]);
    let without = eval(&objective(lin_points(), terms, vec![], None, 2), &m, &[X0.to_vec()]);
    let ob = |e: &Eval| {
        e.obj[0]
            .clone()
            .unwrap()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    };
    let pv = |e: &Eval| {
        ["q@nominal", "power@nominal"]
            .iter()
            .flat_map(|n| point_values(&e.recs[0], n))
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    };
    println!(
        "T7: objective bits equal {}, per-point bits equal {}",
        ob(&with) == ob(&without),
        pv(&with) == pv(&without)
    );
    assert_eq!(ob(&with), ob(&without), "T7 objective");
    assert_eq!(pv(&with), pv(&without), "T7 per point");
}

struct Listed(Vec<f64>);

impl SpecimenModel for Listed {
    type Specimen = f64;
    fn id(&self) -> &str {
        "analytic.listed"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Analytic
    }
    fn metrics(&self) -> Vec<(String, String)> {
        vec![("m".into(), "1".into())]
    }
    fn describe(&self) -> Value {
        json!(self.0)
    }
    fn nominal(&self, _: &[f64]) -> Result<f64, String> {
        Ok(0.0)
    }
    fn perturbed(&self, _: &[f64], n: usize, _: u64) -> Result<Vec<f64>, String> {
        Ok(self.0[..n].to_vec())
    }
    fn evaluate(&self, v: &f64, _: &ImpedancePoint, _: &BTreeSet<String>) -> Result<Metrics, String> {
        Ok([("m".to_string(), *v)].into_iter().collect())
    }
}

#[test]
fn t8_sample_divisor_and_constant_sample() {
    let terms = vec![
        term("m", Sense::Min, TolStat::Sd, Over::Mean),
        term("m", Sense::Min, TolStat::Mean, Over::Mean),
    ];
    let o3 = objective(vec![pt("P", 1.0, 1.0)], terms.clone(), vec![], plan(3, 1), 1);
    let ov = eval(&o3, &Listed(vec![1.0, 2.0, 4.0]), &[X0.to_vec()]).obj[0]
        .clone()
        .unwrap();
    let want = (7.0f64 / 3.0).sqrt();
    println!(
        "T8: sd of 1, 2, 4: {:.17e} against sqrt(7/3) {want:.17e}, rel {:.3e}",
        ov[0],
        rel(ov[0], want)
    );
    let o5 = objective(vec![pt("P", 1.0, 1.0)], terms, vec![], plan(5, 1), 1);
    let oc = eval(&o5, &Listed(vec![0.1; 5]), &[X0.to_vec()]).obj[0].clone().unwrap();
    println!("T8: constant 0.1 x 5: sd {:e}, mean {:.17e}", oc[0], oc[1]);
    assert!(rel(ov[0], want) <= 1e-15, "T8 divisor");
    assert!(oc[0] == 0.0, "T8 constant sd");
    assert_eq!(oc[1].to_bits(), 0.1f64.to_bits(), "T8 constant mean");
}

// ---------------------------------------------------------------- K: constraints

fn k_eval(limits: Vec<Limit>, points: Vec<ImpedancePoint>) -> Eval {
    let o = objective(
        points,
        vec![term("power", Sense::Min, TolStat::Nominal, Over::Mean)],
        limits,
        None,
        1,
    );
    eval(&o, &LinearFan { sigma_s: 0.02 }, &[X0.to_vec()])
}

#[test]
fn k1_k3_k6_violations() {
    let k1 = k_eval(
        vec![limit("q", Cmp::Ge, 0.02, 0.01, TolStat::Nominal, None)],
        lin_points(),
    );
    println!(
        "K1: violation {:.17e} against 0.75, rel {:.3e}",
        k1.viol[0],
        rel(k1.viol[0], 0.75)
    );
    let k2 = k_eval(
        vec![limit("q", Cmp::Ge, 0.01, 0.01, TolStat::Nominal, None)],
        lin_points(),
    );
    println!("K2: violation {:e}", k2.viol[0]);
    let k3 = k_eval(
        vec![
            limit("q", Cmp::Ge, 0.02, 0.01, TolStat::Nominal, None),
            limit("power", Cmp::Le, 2.0, 1.0, TolStat::Nominal, None),
        ],
        lin_points(),
    );
    let r = &k3.recs[0];
    let v1 = point_values(r, "q@nominal")
        .iter()
        .map(|q| ((0.02 - q) / 0.01).max(0.0))
        .fold(0.0, f64::max);
    let v2 = point_values(r, "power@nominal")
        .iter()
        .map(|p| ((p - 2.0) / 1.0).max(0.0))
        .fold(0.0, f64::max);
    println!(
        "K3: total {:.17e}, hand {:.17e} ({v1:.6} + {v2:.6}), rel {:.3e}",
        k3.viol[0],
        v1 + v2,
        rel(k3.viol[0], v1 + v2)
    );
    let k6 = k_eval(
        vec![limit("q", Cmp::Ge, 0.03, 0.01, TolStat::Nominal, Some(Over::Mean))],
        lin_points(),
    );
    println!(
        "K6: violation {:.17e} against 0.5, rel {:.3e}",
        k6.viol[0],
        rel(k6.viol[0], 0.5)
    );
    assert!(rel(k1.viol[0], 0.75) <= 1e-9, "K1");
    assert!(k2.viol[0] == 0.0, "K2");
    assert!(rel(k3.viol[0], v1 + v2) <= 1e-12, "K3");
    assert!(rel(k6.viol[0], 0.5) <= 1e-9, "K6");
}

#[test]
fn k4_stalled_point_fails_the_candidate() {
    let mut p4 = lin_points();
    p4.push(pt("I4", 1.0e7, 0.0));
    let bad = k_eval(vec![], p4);
    let good = k_eval(
        vec![limit("q", Cmp::Ge, 0.02, 0.01, TolStat::Nominal, None)],
        lin_points(),
    );
    let f = bad.fit[0];
    println!("K4: fitness {f:?}; message: {}", bad.recs[0].message);
    assert!(
        f.f == Fitness::FAILED.f && f.violation == Fitness::FAILED.violation,
        "K4 failed"
    );
    assert!(
        bad.recs[0].message.contains("I4") && bad.recs[0].message.contains("stall"),
        "K4 reason"
    );
    assert!(good.fit[0].better(&f) && !f.better(&good.fit[0]), "K4 ranking");
}

#[test]
fn k5_unknown_metric_is_refused() {
    let o = objective(
        lin_points(),
        vec![term("power", Sense::Min, TolStat::Nominal, Over::Mean)],
        vec![limit("nope", Cmp::Le, 1.0, 1.0, TolStat::Nominal, None)],
        None,
        1,
    );
    let e = o.validate(&LinearFan { sigma_s: 0.02 }).unwrap_err();
    println!("K5: {e}");
    assert!(e.contains("nope"), "K5");
}

// ---------------------------------------------------------------- W: determinism

fn scratch(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("f697_{}_{name}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

fn fixed_clock() -> u64 {
    CLOCK
}

type RunBits = (Vec<Vec<u64>>, Vec<u64>, Vec<u8>);

fn w_run(workers: usize, seed: u64, tag: &str) -> RunBits {
    let m = LinearFan { sigma_s: 0.02 };
    let o = objective(
        lin_points(),
        vec![
            term("q", Sense::Max, TolStat::Mean, Over::Mean),
            term("power", Sense::Min, TolStat::MeanPlusKSigma(1.0), Over::Worst),
        ],
        vec![limit("q", Cmp::Ge, 0.02, 0.01, TolStat::Nominal, None)],
        plan(64, seed),
        workers,
    );
    o.validate(&m).unwrap();
    let path = scratch(tag);
    let mut lg = Ledger::new(&path);
    lg.clock = fixed_clock;
    let ev = TolerantEvaluator {
        objective: &o,
        model: &m,
    };
    let mut s = Study::new(o.study_def(), Store::memory("w"), &ev).unwrap();
    s.set_ledger_sink(o.ledger_sink(&m, &lg)).unwrap();
    s.run(&Method::Doe(Doe::Lhs { n: 32, seed: 697_022 })).unwrap();
    o.append_scores(&m, &s, &lg).unwrap();
    let recs = s.store.records();
    let obj = recs
        .iter()
        .map(|r| s.objective_vector(r).unwrap().iter().map(|v| v.to_bits()).collect())
        .collect();
    let viol = recs.iter().map(|r| s.violation_of(r).to_bits()).collect();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    (obj, viol, bytes)
}

#[test]
fn w1_w2_workers_and_seeds() {
    let r1 = w_run(1, 697_021, "w1_1");
    let lines = r1.2.iter().filter(|&&b| b == b'\n').count();
    println!(
        "W1: workers 1: {} candidates, ledger {} bytes, {lines} lines",
        r1.0.len(),
        r1.2.len()
    );
    assert_eq!(r1.0.len(), 32);
    let mut eq = true;
    for w in [2usize, 3, 8] {
        let r = w_run(w, 697_021, &format!("w1_{w}"));
        let e = (r.0 == r1.0, r.1 == r1.1, r.2 == r1.2);
        println!(
            "W1: workers {w}: objective bits {}, violation bits {}, ledger bytes {} ({} bytes)",
            e.0,
            e.1,
            e.2,
            r.2.len()
        );
        eq &= e.0 && e.1 && e.2;
    }
    let again = w_run(1, 697_021, "w2_again");
    let same = again == r1;
    let other = w_run(1, 697_023, "w2_other");
    let differ = other.0.iter().zip(&r1.0).filter(|(a, b)| a != b).count();
    println!("W2: same seed equal {same}; another seed: {differ} of 32 objective vectors differ");
    assert!(eq, "W1");
    assert!(same, "W2 same seed");
    assert_eq!(differ, 32, "W2 other seed");
}

// ---------------------------------------------------------------- R, N: the reduced-order fan

/// Set-up F: the ADR 0119 set-up C free-vortex row as a ComputerFan.
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

fn r_points() -> Vec<ImpedancePoint> {
    vec![pt("R1", 5.0e3, 0.5), pt("R2", 1.5e4, 0.3), pt("R3", 2.5e4, 0.2)]
}

fn direct_metrics(fan: &ComputerFan, p: &ImpedancePoint) -> Metrics {
    let ev = FanRomEvaluator {
        geometry: Box::new(|_| Err("unused".into())),
        model: RomModel::default(),
        base: base_cond(),
    };
    let mut c = base_cond();
    c.system = p.system;
    ev.metrics(RomFan::from_axial_fan(&fan.axial_fan(0, 0.0, 0.0)).unwrap(), &c)
        .unwrap()
}

#[test]
fn r1_r2_layer_equals_direct_reduced_order_metrics() {
    let m = target(fan_f(), Tolerances::default());
    let terms = vec![
        term("q", Sense::Max, TolStat::Nominal, Over::Mean),
        term("power", Sense::Min, TolStat::Nominal, Over::Worst),
        term("tone1_pa", Sense::Min, TolStat::Nominal, Over::Weighted),
        term("dp_s", Sense::Min, TolStat::Nominal, Over::MeanPlusKSigma(1.0)),
    ];
    let o = rom_objective(r_points(), terms, None, 3);
    let e = eval(&o, &m, &[vec![0.0]]);
    let ov = e.obj[0].clone().unwrap();
    let d: Vec<Metrics> = r_points().iter().map(|p| direct_metrics(&fan_f(), p)).collect();
    let col = |k: &str, sign: f64| d.iter().map(|m| sign * m[k]).collect::<Vec<f64>>();
    let hand = [
        Robust::Mean.apply(&col("q", -1.0)),
        Robust::Worst.apply(&col("power", 1.0)),
        Robust::Weighted(vec![0.5, 0.3, 0.2]).apply(&col("tone1_pa", 1.0)),
        Robust::MeanPlusKSigma(1.0).apply(&col("dp_s", 1.0)),
    ];
    let mut ok = true;
    for i in 0..4 {
        let eq = ov[i].to_bits() == hand[i].to_bits();
        println!(
            "R1 term {i}: layer {:.17e}, direct {:.17e}, bit-equal {eq}",
            ov[i], hand[i]
        );
        ok &= eq;
    }
    let q = point_values(&e.recs[0], "q@nominal");
    let dp = point_values(&e.recs[0], "dp_s@nominal");
    println!("R2: q {q:?}, dp_s {dp:?}");
    assert!(ok, "R1");
    assert!(q[0] > q[1] && q[1] > q[2], "R2 q");
    assert!(dp[0] < dp[1] && dp[1] < dp[2], "R2 dp_s");
}

#[test]
fn r3_tolerance_plan_on_the_reduced_order_fan() {
    let tol = Tolerances {
        stagger_common_deg: Dist::TruncatedNormal {
            mean: 0.0,
            sd: 0.5,
            lo: -1.0,
            hi: 1.0,
        },
        tip_gap_delta: Dist::TruncatedNormal {
            mean: 0.0,
            sd: 1e-4,
            lo: -2e-4,
            hi: 2e-4,
        },
        mass_rel: Dist::TruncatedNormal {
            mean: 0.0,
            sd: 0.01,
            lo: -0.03,
            hi: 0.03,
        },
        ..Default::default()
    };
    let m = target(fan_f(), tol);
    let o = rom_objective(
        r_points(),
        vec![
            term("q", Sense::Max, TolStat::Mean, Over::Mean),
            term("unbalance_force", Sense::Min, TolStat::Mean, Over::Mean),
        ],
        Some(TolerancePlan {
            n: 64,
            seed: 697_031,
            max_failed_fraction: 0.5,
        }),
        4,
    );
    let t0 = std::time::Instant::now();
    let e = eval(&o, &m, &[vec![0.0]]);
    let wall = t0.elapsed().as_secs_f64();
    let ev = TolerantEvaluator {
        objective: &o,
        model: &m,
    };
    let table = ev.specimen_table(&[0.0]).unwrap();
    let mut counts_equal = true;
    for (q, c) in e.recs[0].conditions.iter().enumerate() {
        let direct = table[1..].iter().filter(|r| r[q].is_err()).count();
        let layer = if c.ok {
            c.metrics[SPECIMENS_FAILED] as usize
        } else {
            c.message
                .split(" of ")
                .next()
                .and_then(|s| s.rsplit(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX)
        };
        println!(
            "R3 {}: ok {}, failed specimens layer {layer}, direct {direct}; {}",
            c.label, c.ok, c.message
        );
        counts_equal &= layer == direct;
    }
    println!(
        "R3: objective {:?}, wall {wall:.1} s for 65 specimens x 3 points",
        e.obj[0]
    );
    assert!(counts_equal, "R3 counts");
}

#[test]
fn r4_zero_tolerances_give_sd_exactly_zero() {
    let m = target(fan_f(), Tolerances::default());
    let mut terms = Vec::new();
    for k in ["q", "power", "tone1_pa", "unbalance"] {
        for t in [TolStat::Sd, TolStat::Mean, TolStat::Nominal] {
            terms.push(term(k, Sense::Min, t, Over::Mean));
        }
    }
    let o = rom_objective(r_points(), terms, plan(13, 697_032), 4);
    let e = eval(&o, &m, &[vec![0.0]]);
    let r = &e.recs[0];
    let mut bad = 0;
    for k in ["q", "power", "tone1_pa", "unbalance"] {
        let sd = point_values(r, &format!("{k}@sd"));
        let mean = point_values(r, &format!("{k}@mean"));
        let nom = point_values(r, &format!("{k}@nominal"));
        for i in 0..3 {
            if sd[i] != 0.0 || mean[i].to_bits() != nom[i].to_bits() {
                bad += 1;
            }
        }
        println!("R4 {k}: sd {sd:?}, mean {mean:?}, nominal {nom:?}");
    }
    assert!(r.ok(), "R4 record failed: {}", r.message);
    assert_eq!(bad, 0, "R4");
}

#[test]
fn r5_ledger_entries_and_rerun() {
    let m = target(fan_f(), Tolerances::default());
    let o = rom_objective(
        r_points(),
        vec![
            term("q", Sense::Max, TolStat::Nominal, Over::Mean),
            term("tone_a3_db", Sense::Min, TolStat::Nominal, Over::Worst),
        ],
        None,
        4,
    );
    o.validate(&m).unwrap();
    let path = scratch("r5");
    let mut lg = Ledger::new(&path);
    lg.clock = fixed_clock;
    let ev = TolerantEvaluator {
        objective: &o,
        model: &m,
    };
    let mut s = Study::new(o.study_def(), Store::memory("r5"), &ev).unwrap();
    s.set_ledger_sink(o.ledger_sink(&m, &lg)).unwrap();
    s.run(&Method::Doe(Doe::Factorial { levels: vec![4] })).unwrap();
    let n_scores = o.append_scores(&m, &s, &lg).unwrap();
    let c = lg.read().unwrap();
    let evals: Vec<usize> = (0..c.entries.len())
        .filter(|&i| c.entries[i].kind == "evaluation")
        .collect();
    let scores: Vec<usize> = (0..c.entries.len())
        .filter(|&i| c.entries[i].kind == "objective_score")
        .collect();
    println!(
        "R5: {} entries, {} skipped, {} point entries, {} score entries ({n_scores} appended)",
        c.entries.len(),
        c.skipped.len(),
        evals.len(),
        scores.len()
    );
    let id = o.identity(&m);
    let js = o.to_json(&m);
    let om = ObjectiveModel {
        objective: &o,
        model: &m,
    };
    let mut problems = Vec::new();
    let mut reruns = 0;
    for (j, &i) in scores.iter().enumerate() {
        let e = &c.entries[i];
        let rec = &s.store.records()[j];
        if e.fidelity != Fidelity::ReducedOrder {
            problems.push(format!("{i}: fidelity"));
        }
        if e.objective.as_ref() != Some(&id) {
            problems.push(format!("{i}: objective identity"));
        }
        if e.settings.get("objective") != Some(&js) {
            problems.push(format!("{i}: settings.objective"));
        }
        if e.parents.len() != 3 {
            problems.push(format!("{i}: {} parents", e.parents.len()));
        }
        for (pr, cnd) in e.parents.iter().zip(&s.def.conditions) {
            match ledger::resolve(&c.entries, pr) {
                Ok(k) => {
                    let pe = &c.entries[k];
                    if pe.provenance["store_key"] != json!(rec.key) || pe.provenance["condition"] != json!(cnd.label) {
                        problems.push(format!("{i}: parent {k} is not record {} at {}", rec.index, cnd.label));
                    }
                }
                Err(err) => problems.push(format!("{i}: {err}")),
            }
        }
        if e.verdict == ledger::Verdict::Ok {
            let r = ledger::rerun_check(e, &om, 0.0, 0.0);
            reruns += 1;
            let dmax = r.metrics.iter().map(|m| m.3).fold(0.0, f64::max);
            if !r.pass || dmax != 0.0 {
                problems.push(format!("{i}: rerun {:?} {:?}", r.problems, r.metrics));
            }
        }
        println!(
            "R5 score {i}: verdict {}, metrics {:?}, reason '{}'",
            e.verdict.as_str(),
            e.metrics,
            e.reason
        );
    }
    let _ = std::fs::remove_file(&path);
    println!("R5: {reruns} reruns; problems {problems:?}");
    assert_eq!(c.skipped.len(), 0, "R5 skipped");
    assert_eq!(evals.len(), 12, "R5 point entries");
    assert_eq!(scores.len(), 4, "R5 score entries");
    assert!(problems.is_empty(), "R5");
}

#[test]
fn r6_stalled_reduced_order_point_fails_the_candidate() {
    let m = target(fan_f(), Tolerances::default());
    let mut p = r_points();
    p.push(pt("R4", 4.0e4, 0.0));
    let o = rom_objective(p, vec![term("q", Sense::Max, TolStat::Nominal, Over::Mean)], None, 4);
    let e = eval(&o, &m, &[vec![0.0]]);
    println!("R6: ok {}, message: {}", e.recs[0].ok(), e.recs[0].message);
    assert!(e.obj[0].is_none() && !e.recs[0].ok(), "R6 failed");
    assert!(
        e.recs[0].message.contains("R4") && e.recs[0].message.contains("stalled"),
        "R6 reason"
    );
}

#[test]
fn n1_n2_noise_metrics_by_hand() {
    let m = target(fan_f(), Tolerances::default());
    let o = rom_objective(
        r_points(),
        vec![
            term("tone1_db", Sense::Min, TolStat::Nominal, Over::Mean),
            term("tone_a3_db", Sense::Min, TolStat::Nominal, Over::Mean),
        ],
        None,
        3,
    );
    let e = eval(&o, &m, &[vec![0.0]]);
    let l1 = point_values(&e.recs[0], "tone1_db@nominal");
    let la = point_values(&e.recs[0], "tone_a3_db@nominal");
    let fan = fan_f();
    let rf = RomFan::from_axial_fan(&fan.axial_fan(0, 0.0, 0.0)).unwrap();
    let (mut n1, mut n2) = (true, 0.0f64);
    for (i, p) in r_points().iter().enumerate() {
        let d = direct_metrics(&fan, p);
        let h1 = spl_db(d["tone1_pa"] / std::f64::consts::SQRT_2).unwrap();
        // N2 by hand: operating point, tones m = 1..3, A-weighted energy sum
        let mut c = base_cond();
        c.system = p.system;
        let rom = FanRom::new(rf.clone(), RomModel::default()).unwrap();
        let q_hi = rom.area() * c.op.omega() * rom.fan.r_tip;
        let (_, fp) = rom
            .operating_point(&c.op, &c.system, 1e-3 * q_hi, q_hi, c.n_scan)
            .unwrap();
        let mut sum = 0.0;
        for h in 1..=3usize {
            let ph = rom.tone(&c.op, &fp, c.c0, h, c.observer_r, c.observer_theta).unwrap();
            let f = (h * 7) as f64 * c.op.omega() / (2.0 * std::f64::consts::PI);
            sum += 10f64.powf((spl_db(ph / 2f64.sqrt()).unwrap() + a_weighting_db(f).unwrap()) / 10.0);
        }
        let h2 = 10.0 * sum.log10();
        println!(
            "N1 {}: {:.17e} hand {h1:.17e}; N2: {:.17e} hand {h2:.17e} rel {:.3e}",
            p.label,
            l1[i],
            la[i],
            rel(la[i], h2)
        );
        n1 &= l1[i].to_bits() == h1.to_bits();
        n2 = n2.max(rel(la[i], h2));
    }
    assert!(n1, "N1");
    assert!(n2 <= 1e-12, "N2");
}

// ---------------------------------------------------------------- O: optimisers through the layer

fn o_run(o: &FanObjective, method: &Method) -> (OptResult, Vec<String>, Vec<(f64, f64)>) {
    let m = LinearFan { sigma_s: 0.02 };
    o.validate(&m).unwrap();
    let ev = TolerantEvaluator {
        objective: o,
        model: &m,
    };
    let mut s = Study::new(o.study_def(), Store::memory("o"), &ev).unwrap();
    let r = s.run(method).unwrap();
    let recs = s.store.records().iter().map(|r| r.to_json().to_string()).collect();
    let front = s
        .pareto()
        .iter()
        .map(|&i| {
            let v = s.objective_vector(&s.store.records()[i]).unwrap();
            (-v[0], v.get(1).copied().unwrap_or(f64::NAN))
        })
        .collect();
    (r, recs, front)
}

fn same_run(a: &(OptResult, Vec<String>, Vec<(f64, f64)>), b: &(OptResult, Vec<String>, Vec<(f64, f64)>)) -> bool {
    a.1 == b.1
        && a.0.x.iter().map(|v| v.to_bits()).eq(b.0.x.iter().map(|v| v.to_bits()))
        && a.0.fitness.f.to_bits() == b.0.fitness.f.to_bits()
        && a.0.fitness.violation.to_bits() == b.0.fitness.violation.to_bits()
        && a.0.evaluations == b.0.evaluations
}

fn o1_objective(plan_: Option<TolerancePlan>, workers: usize) -> FanObjective {
    let robust = plan_.is_some();
    objective(
        lin_points(),
        vec![term(
            "power",
            Sense::Min,
            if robust { TolStat::Mean } else { TolStat::Nominal },
            Over::Mean,
        )],
        vec![limit(
            "q",
            Cmp::Ge,
            0.03,
            0.01,
            if robust {
                TolStat::MeanPlusKSigma(2.0)
            } else {
                TolStat::Nominal
            },
            Some(Over::Mean),
        )],
        plan_,
        workers,
    )
}

fn check_opt(tag: &str, r: &OptResult, s_star: f64, p_star: f64) -> bool {
    let (s, t) = (r.x[0], r.x[1]);
    let p = r.fitness.f;
    let ok = r.fitness.violation == 0.0
        && p <= p_star * (1.0 + 1e-3)
        && (s - s_star).abs() <= 1e-3
        && (t - 0.3).abs() <= 3e-2;
    println!(
        "{tag}: s {s:.6} (s* {s_star:.6}, diff {:.2e}), t {t:.6}, P {p:.8} (P* {p_star:.8}, rel {:.2e}), violation {:e}, {} evaluations: {}",
        s - s_star,
        p / p_star - 1.0,
        r.fitness.violation,
        r.evaluations,
        if ok { "met" } else { "MISSED" }
    );
    ok
}

fn nm(seed: u64) -> Method {
    Method::NelderMead {
        budget: 400,
        restarts: 0,
        seed,
    }
}

fn cma(seed: u64) -> Method {
    Method::CmaEs {
        budget: 2000,
        pop: None,
        sigma0: 0.3,
        restarts: 0,
        seed,
    }
}

#[test]
fn o1_w3_constrained_optimum_by_nelder_mead_and_cma_es() {
    let mut ok = true;
    let mut w3 = true;
    for (tag, method) in [("O1 NM", nm(697_011)), ("O1 CMA-ES", cma(697_012))] {
        let a = o_run(&o1_objective(None, 1), &method);
        ok &= check_opt(tag, &a.0, 1.2, 3.6);
        let b = o_run(&o1_objective(None, 4), &method);
        let same = same_run(&a, &b);
        println!("W3 {tag}: workers 1 and 4 bit-equal: {same}");
        w3 &= same;
    }
    assert!(w3, "W3");
    assert!(ok, "O1");
}

#[test]
fn o2_w3_nsga2_front() {
    let o = |w| {
        objective(
            lin_points(),
            vec![
                term("q", Sense::Max, TolStat::Nominal, Over::Mean),
                term("power", Sense::Min, TolStat::Nominal, Over::Mean),
            ],
            vec![],
            None,
            w,
        )
    };
    let method = Method::Nsga2 {
        budget: 4000,
        pop: 40,
        seed: 697_013,
    };
    let a = o_run(&o(1), &method);
    let b = o_run(&o(4), &method);
    let front = &a.2;
    let (mut lo, mut hi, mut worst_hi, mut worst_lo) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY, f64::INFINITY);
    for &(q, p) in front {
        let d = p / ((25.0 / 12.0) * (q / 0.025).powi(3)) - 1.0;
        worst_hi = worst_hi.max(d);
        worst_lo = worst_lo.min(d);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let span = (hi - lo) / (0.05 - 0.0125);
    let same = same_run(&a, &b);
    println!(
        "O2: {} Pareto members, P / P_f - 1 in [{worst_lo:.3e}, {worst_hi:.3e}], flows [{lo:.5}, {hi:.5}], span {span:.3}; W3 workers 1 and 4 bit-equal: {same}",
        front.len()
    );
    assert!(same, "W3 NSGA-II");
    assert!(
        !front.is_empty() && worst_lo >= -1e-9 && worst_hi <= 0.02,
        "O2 distance"
    );
    assert!(span >= 0.8, "O2 span");
}

#[test]
fn o3_robust_optimum_by_nelder_mead_and_cma_es() {
    let p = plan(64, 697_014);
    let e = draws(0.02, 64, 697_014);
    let nf = 64.0;
    let ebar = e.iter().sum::<f64>() / nf;
    let se = (e.iter().map(|x| (x - ebar) * (x - ebar)).sum::<f64>() / (nf - 1.0)).sqrt();
    let m3 = e.iter().map(|x| (1.0 + x).powi(3)).sum::<f64>() / nf;
    let s_star = 1.2 / (1.0 + ebar - 2.0 * se);
    let p_star = (25.0 / 24.0) * s_star.powi(3) * m3 / 0.5;
    println!("O3: sample ebar {ebar:.3e}, s_e {se:.5}, m3 {m3:.6}; s* {s_star:.6} (population 1.25), P* {p_star:.6}");
    let mut ok = true;
    for (tag, method) in [("O3 NM", nm(697_015)), ("O3 CMA-ES", cma(697_016))] {
        let a = o_run(&o1_objective(p, 1), &method);
        ok &= check_opt(tag, &a.0, s_star, p_star);
    }
    assert!(ok, "O3");
}
