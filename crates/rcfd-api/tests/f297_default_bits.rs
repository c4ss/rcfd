//! ADR 0220 V0b (F-297): the default NSGA-II paths the F-297 change passes through, hashed bit for bit. FNV-1a (64 bit, as
//! `f249_default_bits.rs`) over the little-endian bytes of `f64::to_bits`, of integers as `u64` and of text as bytes. The values
//! below were recorded on main 36466d0 with the ADR 0220 registration and this file only, before any code of F-297, and must stay
//! bit-identical after it.

use rcfd_api::design::nsga2::{self, MoFitness, Options};
use rcfd_api::design::optim::Stop;
use rcfd_api::design::store::{CondResult, Record, Store};
use rcfd_api::design::study::{
    Condition, Evaluator, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study, StudyDef,
};
use std::collections::BTreeMap;

/// The records, recorded on main 36466d0.
const LIB_ZDT1: u64 = 0x3d40_dd97_932d_aaa3;
const LIB_CONSTRAINED: u64 = 0x80ad_a5d6_a437_c4da;
const STUDY_ZDT1: u64 = 0xfff2_34ff_1846_99f2;
const STUDY_FON: u64 = 0xa5de_8eb8_efbe_8d28;
const PROPOSALS: u64 = 0x7a6b_79cc_2fd2_ed45;

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn f(&mut self, x: f64) {
        self.bytes(&x.to_bits().to_le_bytes());
    }
    fn u(&mut self, x: u64) {
        self.bytes(&x.to_le_bytes());
    }
    fn s(&mut self, t: &str) {
        self.u(t.len() as u64);
        self.bytes(t.as_bytes());
    }
}

fn zdt1(x: &[f64]) -> [f64; 2] {
    let f1 = x[0];
    let g = 1.0 + 9.0 * x[1..].iter().sum::<f64>() / (x.len() - 1) as f64;
    [f1, g * (1.0 - (f1 / g).sqrt())]
}

fn fon(x: &[f64]) -> [f64; 2] {
    let a = 1.0 / 3f64.sqrt();
    let s1: f64 = x.iter().map(|v| (v - a).powi(2)).sum();
    let s2: f64 = x.iter().map(|v| (v + a).powi(2)).sum();
    [1.0 - (-s1).exp(), 1.0 - (-s2).exp()]
}

fn hash_result(h: &mut Fnv, r: &nsga2::MoResult) {
    for (x, f) in &r.population {
        x.iter().for_each(|v| h.f(*v));
        f.objectives.iter().for_each(|v| h.f(*v));
        h.f(f.violation);
    }
    r.rank.iter().for_each(|k| h.u(*k as u64));
    h.u(r.evaluations as u64);
    h.u(r.generations as u64);
}

/// (1) `nsga2::run` on ZDT1 d = 30, pop 20, budget 400, seed 4, x0 = 0.5.
fn library_zdt1() -> u64 {
    let mut h = Fnv::new();
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                let f = zdt1(x);
                x.iter().for_each(|v| h.f(*v));
                f.iter().for_each(|v| h.f(*v));
                MoFitness::new(f.to_vec(), 0.0)
            })
            .collect())
    };
    let r = nsga2::run(
        &[(0.0, 1.0); 30],
        &Options {
            max_evals: 400,
            pop: 20,
            seed: 4,
            x0: Some(vec![0.5; 30]),
        },
        &mut ev,
    )
    .unwrap();
    hash_result(&mut h, &r);
    h.0
}

/// (2) `nsga2::run` with constraints and failures: ZDT1 d = 5, pop 20, budget 200, seed 7.
fn library_constrained() -> u64 {
    let mut h = Fnv::new();
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                x.iter().for_each(|v| h.f(*v));
                if x[2] > 0.9 {
                    h.u(1);
                    return MoFitness::failed(2);
                }
                let f = zdt1(x);
                let v = (0.3 - x[0]).max(0.0) + (x[1] - 0.6).max(0.0);
                f.iter().for_each(|y| h.f(*y));
                h.f(v);
                MoFitness::new(f.to_vec(), v)
            })
            .collect())
    };
    let r = nsga2::run(
        &[(0.0, 1.0); 5],
        &Options {
            max_evals: 200,
            pop: 20,
            seed: 7,
            x0: None,
        },
        &mut ev,
    )
    .unwrap();
    hash_result(&mut h, &r);
    h.0
}

#[derive(Clone, Copy)]
enum Problem {
    Zdt1,
    Fon,
}

fn def(p: Problem) -> StudyDef {
    let (d, lo, hi) = match p {
        Problem::Zdt1 => (30, 0.0, 1.0),
        Problem::Fon => (3, -4.0, 4.0),
    };
    StudyDef {
        name: "mo".into(),
        params: (0..d)
            .map(|i| ParamDef {
                name: format!("x{i:02}"),
                lo,
                hi,
                init: None,
                targets: vec![],
            })
            .collect(),
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: ["f1", "f2"]
            .iter()
            .map(|m| ObjectiveDef {
                metric: (*m).into(),
                sense: Sense::Min,
                weight: 1.0,
                robust: Robust::Mean,
            })
            .collect(),
        constraints: vec![],
        workers: 1,
    }
}

fn result(f: [f64; 2]) -> CondResult {
    let mut m = BTreeMap::new();
    m.insert("f1".to_string(), f[0]);
    m.insert("f2".to_string(), f[1]);
    CondResult::success("base", m)
}

struct Ev(Problem);

impl Evaluator for Ev {
    fn evaluate(&self, _d: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        vec![result(match self.0 {
            Problem::Zdt1 => zdt1(x),
            Problem::Fon => fon(x),
        })]
    }
}

fn hash_records(recs: &[Record]) -> u64 {
    let mut h = Fnv::new();
    for r in recs {
        h.u(r.index as u64);
        h.s(&r.key);
        for (n, v) in &r.params {
            h.s(n);
            h.f(*v);
        }
        h.s(&r.source);
        for c in &r.conditions {
            h.s(&c.label);
            h.u(u64::from(c.ok));
            h.s(&c.status);
            h.u(c.iterations as u64);
            for (n, v) in &c.metrics {
                h.s(n);
                h.f(*v);
            }
            h.s(&c.message);
        }
        h.s(&r.message);
    }
    h.0
}

/// (3) the records of a `Study` run with the default `Method::Nsga2`.
fn study_records(p: Problem, budget: usize, seed: u64) -> u64 {
    let ev = Ev(p);
    let mut s = Study::new(def(p), Store::memory("mo"), &ev).unwrap();
    s.run(&Method::Nsga2 { budget, pop: 100, seed }).unwrap();
    hash_records(s.store.records())
}

struct NoEval;
impl Evaluator for NoEval {
    fn evaluate(&self, _d: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        panic!("the propose / record loop evaluates nothing itself")
    }
}

/// (4) the proposals of the ZDT1 d = 30, pop 20, budget 400, seed 4 study driven by propose / record.
fn proposals() -> u64 {
    let ev = NoEval;
    let mut s = Study::new(def(Problem::Zdt1), Store::memory("mo"), &ev).unwrap();
    let m = Method::Nsga2 {
        budget: 400,
        pop: 20,
        seed: 4,
    };
    let mut h = Fnv::new();
    let mut rounds = 0;
    while let Proposal::Next(points) = s.propose(&m).unwrap() {
        rounds += 1;
        assert!(rounds <= 40);
        h.u(points.len() as u64);
        for p in points {
            p.iter().for_each(|v| h.f(*v));
            s.record(&p, vec![result(zdt1(&p))], "").unwrap();
        }
    }
    h.u(rounds as u64);
    h.0
}

#[test]
fn v0b_library_runs() {
    let a = library_zdt1();
    let b = library_constrained();
    println!("V0b library: zdt1 {a:#018x}, constrained {b:#018x}");
    assert_eq!(a, LIB_ZDT1, "V0b (1)");
    assert_eq!(b, LIB_CONSTRAINED, "V0b (2)");
}

#[test]
fn v0b_study_records() {
    let z = study_records(Problem::Zdt1, 25_000, 1);
    let f = study_records(Problem::Fon, 10_000, 2);
    println!("V0b study: zdt1 {z:#018x}, fon {f:#018x}");
    assert_eq!(z, STUDY_ZDT1, "V0b (3) ZDT1");
    assert_eq!(f, STUDY_FON, "V0b (3) FON");
}

#[test]
fn v0b_proposals() {
    let p = proposals();
    println!("V0b proposals: {p:#018x}");
    assert_eq!(p, PROPOSALS, "V0b (4)");
}
