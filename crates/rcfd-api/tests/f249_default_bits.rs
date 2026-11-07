//! ADR 0209 V0b (F-249): the default paths the F-249 change passes through, hashed bit for bit. FNV-1a (64 bit, as
//! `f286_default_bits.rs`) over the little-endian bytes of `f64::to_bits`, of integers as `u64` and of text as bytes. The values
//! below were recorded on main 05f0d73 with the ADR 0209 registration and this file only, before any code of F-249, and must stay
//! bit-identical after it.

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseMapEvaluator, RunLimits};
use rcfd_api::design::opmap::{self, Axis, MapOptions, MapOutcome};
use rcfd_api::design::optim::{self, DeOptions, Fitness, NmOptions, Stop};
use rcfd_api::design::pareto;
use rcfd_api::design::rng::Rng;
use rcfd_api::design::store::{CondResult, Store};
use rcfd_api::design::study::{Condition, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study, StudyDef};
use serde_json::json;
use std::collections::BTreeMap;

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

fn rosenbrock(x: &[f64]) -> f64 {
    100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)
}

/// (1) ADR 0067 O4: the Nelder-Mead and differential-evolution histories (every requested point and value).
fn optimiser_histories() -> (u64, u64) {
    let bounds = [(-2.0, 2.0), (-2.0, 2.0)];
    let mut h = Fnv::new();
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                let f = rosenbrock(x);
                x.iter().for_each(|v| h.f(*v));
                h.f(f);
                Fitness::new(f, 0.0)
            })
            .collect())
    };
    let o = NmOptions {
        max_evals: 300,
        seed: 3,
        ..Default::default()
    };
    optim::nelder_mead(&bounds, &o, &mut ev).unwrap();
    let nm = h.0;
    let mut h = Fnv::new();
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                let f = rosenbrock(x);
                x.iter().for_each(|v| h.f(*v));
                h.f(f);
                Fitness::new(f, 0.0)
            })
            .collect())
    };
    let o = DeOptions {
        max_evals: 400,
        seed: 3,
        ..Default::default()
    };
    optim::differential_evolution(&bounds, &o, &mut ev).unwrap();
    (nm, h.0)
}

/// The T3 study of ADR 0067 (item 5 of ADR 0209) as a library definition.
fn t3_def() -> StudyDef {
    let p = |name: &str, init: f64| ParamDef {
        name: name.into(),
        lo: -2.0,
        hi: 2.0,
        init: Some(init),
        targets: vec![],
    };
    StudyDef {
        name: "rosen".into(),
        params: vec![p("x", -1.2), p("y", 1.0)],
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: vec![ObjectiveDef {
            metric: "f".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers: 1,
    }
}

struct NoEval;
impl rcfd_api::design::study::Evaluator for NoEval {
    fn evaluate(&self, _def: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        panic!("the propose / record loop evaluates nothing itself")
    }
}

/// (2) The default T3 loop: records hash, best f after 120 records, best f at the end, number of records.
fn t3_default() -> (u64, f64, f64, usize) {
    let ev = NoEval;
    let mut s = Study::new(t3_def(), Store::memory("t3"), &ev).unwrap();
    let method = Method::NelderMead {
        budget: 400,
        restarts: 1,
        seed: 1,
    };
    let best = |s: &Study| {
        let b = s.best().unwrap();
        s.fitness_of(&s.store.records()[b]).f
    };
    let mut at_120 = f64::NAN;
    while let Proposal::Next(points) = s.propose(&method).unwrap() {
        for p in points {
            let mut m = BTreeMap::new();
            m.insert("f".to_string(), rosenbrock(&p));
            s.record(&p, vec![CondResult::success("base", m)], "").unwrap();
            if s.store.records().len() == 120 {
                at_120 = best(&s);
            }
        }
    }
    let mut h = Fnv::new();
    for r in s.store.records() {
        h.u(r.index as u64);
        for (n, v) in &r.params {
            h.s(n);
            h.f(*v);
        }
        h.f(r.conditions[0].metrics["f"]);
    }
    (h.0, at_120, best(&s), s.store.records().len())
}

/// (3) `pareto::front` on the ADR 0067 P1 set and on a seeded 3-objective set with infeasible and non-finite points.
fn fronts() -> (u64, u64) {
    let mut rng = Rng::new(7);
    let mut pts: Vec<Vec<f64>> = Vec::new();
    for i in 0..200 {
        let f1 = i as f64 / 199.0;
        pts.push(vec![f1, 1.0 - f1.sqrt()]);
    }
    for _ in 0..300 {
        let f1 = rng.uniform();
        pts.push(vec![f1, 1.0 - f1.sqrt() + 0.05 + 0.1 * rng.uniform()]);
    }
    Rng::new(11).shuffle(&mut pts);
    let mut h = Fnv::new();
    for i in pareto::front(&pts, &vec![0.0; pts.len()]) {
        h.u(i as u64);
    }
    let a = h.0;
    let mut rng = Rng::new(13);
    let mut objs = Vec::new();
    let mut viol = Vec::new();
    for i in 0..300 {
        let mut o: Vec<f64> = (0..3).map(|_| rng.uniform()).collect();
        if i % 37 == 5 {
            o[1] = f64::NAN;
        }
        objs.push(o);
        viol.push(if i % 3 == 0 { rng.uniform() } else { 0.0 });
    }
    let mut h = Fnv::new();
    for i in pareto::front(&objs, &viol) {
        h.u(i as u64);
    }
    (a, h.0)
}

/// The HE case of ADR 0209 (steady SIMPLEC with the energy equation).
fn he() -> Case {
    Case::parse_toml(
        r#"
[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
cp = 1.0
conductivity = 0.05
[energy]
enabled = true
buoyancy = false
[initial]
temperature = 0.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.5, 0.0, 0.0]
temperature = "fixed"
temperature_value = 0.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
temperature = "zero_gradient"
[boundaries.walls]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
[reports.t_out]
type = "area_average"
patch = "outlet"
field = "T"
[reports.q_walls]
type = "heat_flow"
patch = "walls"
"#,
    )
    .unwrap()
}

fn ke() -> Case {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../cases/channel/kepsilon_channel.toml");
    Case::parse_toml(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn rows_hash(o: &MapOutcome) -> u64 {
    let mut h = Fnv::new();
    for r in &o.rows {
        h.u(r.index as u64);
        h.s(r.status.name());
        h.s(r.restart);
        h.u(u64::from(r.exit_code));
        h.s(&r.state);
        h.u(r.iterations as u64);
        for (k, v) in &r.metrics {
            h.s(k);
            h.f(*v);
        }
        h.s(&r.message);
    }
    h.0
}

/// (4) An operating map through the default `CaseMapEvaluator`.
fn map(base: Case, path: &str, values: Vec<serde_json::Value>) -> MapOutcome {
    let pts = opmap::grid_points(
        &[Axis {
            path: path.into(),
            values,
        }],
        false,
    )
    .unwrap();
    let ev = CaseMapEvaluator {
        base,
        limits: RunLimits::default(),
    };
    opmap::run_map(&pts, &ev, &MapOptions::default(), &mut |_| {})
}

#[test]
fn v0b_optimiser_histories() {
    let (nm, de) = optimiser_histories();
    println!("V0b: nelder_mead {nm:#018x}, differential_evolution {de:#018x}");
    assert_eq!((nm, de), (NM_HISTORY, DE_HISTORY));
}

#[test]
fn v0b_t3_default_loop() {
    let (h, at_120, end, n) = t3_default();
    println!(
        "V0b: T3 default records {h:#018x}, best at 120 {at_120:e} ({:#018x}), at the end {end:e} ({:#018x}), {n} records",
        at_120.to_bits(),
        end.to_bits()
    );
    assert_eq!(
        (h, at_120.to_bits(), end.to_bits(), n),
        (T3_RECORDS, T3_AT_120, T3_END, T3_N)
    );
}

#[test]
fn v0b_pareto_fronts() {
    let (a, b) = fronts();
    println!("V0b: fronts {a:#018x} {b:#018x}");
    assert_eq!((a, b), (FRONT_P1, FRONT_3D));
}

#[test]
fn v0b_default_maps() {
    let he_map = map(
        he(),
        "boundaries.inlet.velocity_value",
        vec![json!([0.5, 0.0, 0.0]), json!([0.6, 0.0, 0.0]), json!([0.7, 0.0, 0.0])],
    );
    let ke_map = map(
        ke(),
        "physics.body_force",
        vec![json!([1.0, 0.0, 0.0]), json!([1.1, 0.0, 0.0])],
    );
    for r in he_map.rows.iter().chain(&ke_map.rows) {
        println!(
            "V0b row {}: {} {} {} it, {:?}, '{}'",
            r.index,
            r.status.name(),
            r.restart,
            r.iterations,
            r.metrics,
            r.message
        );
    }
    let (a, b) = (rows_hash(&he_map), rows_hash(&ke_map));
    println!("V0b: HE map {a:#018x}, KE map {b:#018x}");
    assert_eq!((a, b), (HE_MAP, KE_MAP));
}

// recorded on main 05f0d73 (with the ADR 0209 text only)
const NM_HISTORY: u64 = 0x3b38_97c1_05e8_46ba;
const DE_HISTORY: u64 = 0x34bc_ec30_9453_0b32;
const T3_RECORDS: u64 = 0x92cb_6a32_cb59_6cf1;
const T3_AT_120: u64 = 0x3f69_9ab6_bfa9_6f0b;
const T3_END: u64 = 0x3c19_36a3_aec3_5000;
const T3_N: usize = 347;
const FRONT_P1: u64 = 0x5196_f63b_8108_a861;
const FRONT_3D: u64 = 0xfe2c_a393_48f2_4f8b;
const HE_MAP: u64 = 0x6c29_bcc1_14a1_784d;
const KE_MAP: u64 = 0x4c81_37c1_cda1_438f;
