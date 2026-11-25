//! ADR 0791 (F-339): every argument of the MCP tools `design_uq` and `design_sobol` sent at a value that tells a wrong
//! reading from the right one. Rows B1 to B4: the string, null and absent bounds of a normal distribution; D1: the numbers
//! of the distributions; E1, E2: the evaluator's settings; S1, S2: the seed; C1: the requirements; A1, A2: the stopping rule
//! and the sampler; N1, N2: null for the defaults; W1: workers; G1: the limit of `design_sobol`. K1 is a known miss of the
//! library (a normal distribution open on one side).
//!
//! "Equal" is: the reply's `result` is the library's `result_json` as text, and the two ledger files are equal line by line
//! but for the stamp. Each row also asserts that the library's result for the wrong readings differs (without the `spec` it
//! repeats, so that a number differs), so the equality can fail. The references share no code with the tools' parsers.

mod common;

use common::*;
use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition};
use rcfd_api::design::ledger::Ledger;
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::study::Cmp;
use rcfd_api::design::tools::{sobol_study, uq_study};
use rcfd_api::design::uq::{self, Dist, Requirement, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_post::fan_rom::{free_vortex_design, FanRom, Operating, RomFan, RomModel, RomSection, SystemCurve};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const Q_D: f64 = 0.045;
const SEED: u64 = 339101;
const INF: f64 = f64::INFINITY;

// ---------------------------------------------------------------- the registered set-up

fn op() -> Operating {
    Operating {
        rpm: 2950.0,
        rho: 1.18,
        nu: 1.55e-5,
    }
}

/// Mo: no key at its default.
fn mo() -> RomModel {
    RomModel {
        n_strips: 8,
        cl_max: 1.05,
        cd_normal: 1.7,
        friction: false,
        annulus_drag: 0.025,
        secondary_drag: 0.015,
        tip_cd: 0.5,
    }
}

/// F(family) with the model Mo and the registered condition (`family` by its serialised name).
fn model(family: &str) -> FanRomModel {
    let s = |radius: f64, chord: f64, camber: f64, camber_pos: f64| RomSection {
        radius,
        chord,
        stagger: 0.0,
        camber,
        camber_pos,
    };
    let base = RomFan {
        n_blades: 7,
        r_hub: 0.0305,
        r_tip: 0.0575,
        tip_gap: 0.0011,
        family: serde_json::from_value(json!(family)).unwrap(),
        sections: vec![s(0.029, 0.032, 0.045, 0.42), s(0.059, 0.027, 0.035, 0.46)],
    };
    let fan = free_vortex_design(&base, &mo(), &op(), Q_D, 45.0 / 1.2 / op().omega()).unwrap();
    let dp = FanRom::new(fan.clone(), mo()).unwrap().point(&op(), Q_D).unwrap().dp_s;
    FanRomModel {
        fan,
        model: mo(),
        cond: RomCondition {
            op: op(),
            system: SystemCurve {
                k: 0.7 * dp / (Q_D * Q_D),
                l: 0.3 * dp / Q_D,
            },
            c0: 345.0,
            observer_r: 1.5,
            observer_theta: 1.1,
            n_scan: 24,
        },
    }
}

fn naca() -> FanRomModel {
    model("naca4")
}

/// `evaluator_settings` of `m`, every field by hand; `family` as sent.
fn settings(m: &FanRomModel, family: &str) -> Value {
    let (c, r) = (&m.cond, &m.model);
    json!({
        "fan": {
            "n_blades": m.fan.n_blades, "r_hub": m.fan.r_hub, "r_tip": m.fan.r_tip, "tip_gap": m.fan.tip_gap,
            "family": family,
            "sections": m.fan.sections.iter().map(|s| json!({
                "radius": s.radius, "chord": s.chord, "stagger": s.stagger, "camber": s.camber, "camber_pos": s.camber_pos,
            })).collect::<Vec<_>>(),
        },
        "model": {
            "n_strips": r.n_strips, "cl_max": r.cl_max, "cd_normal": r.cd_normal, "friction": r.friction,
            "annulus_drag": r.annulus_drag, "secondary_drag": r.secondary_drag, "tip_cd": r.tip_cd,
        },
        "condition": {
            "rpm": c.op.rpm, "rho": c.op.rho, "nu": c.op.nu, "system_k": c.system.k, "system_l": c.system.l,
            "c0": c.c0, "observer_r": c.observer_r, "observer_theta": c.observer_theta, "n_scan": c.n_scan,
        },
    })
}

fn param(name: &str, unit: &str, dist: Dist) -> UncertainParam {
    UncertainParam {
        name: name.into(),
        unit: unit.into(),
        dist,
    }
}

/// d1 with the bounds `lo` and `hi`.
fn d1(lo: f64, hi: f64) -> Vec<UncertainParam> {
    let dist = Dist::Normal {
        mean: 0.1,
        sd: 0.3,
        lo,
        hi,
    };
    vec![param("stagger_common_deg", "deg", dist)]
}

/// d1 as JSON; `bounds` adds the keys of the two bounds.
fn d1_json(bounds: Value) -> Value {
    let mut dist = json!({"type": "normal", "mean": 0.1, "sd": 0.3});
    for (k, v) in bounds.as_object().unwrap() {
        dist[k.as_str()] = v.clone();
    }
    json!([{"name": "stagger_common_deg", "unit": "deg", "dist": dist}])
}

/// d1 untruncated.
fn d1_open() -> Vec<UncertainParam> {
    d1(-INF, INF)
}

fn d3() -> Vec<UncertainParam> {
    vec![
        param(
            "tip_gap_delta",
            "m",
            Dist::Uniform {
                lo: -1.5e-4,
                hi: 2.5e-4,
            },
        ),
        param(
            "stagger_common_deg",
            "deg",
            Dist::Normal {
                mean: 0.2,
                sd: 0.4,
                lo: -0.6,
                hi: 0.9,
            },
        ),
        param(
            "chord_rel",
            "1",
            Dist::Beta {
                a: 2.0,
                b: 3.5,
                lo: -0.012,
                hi: 0.018,
            },
        ),
    ]
}

fn d3_json() -> Value {
    json!([
        {"name": "tip_gap_delta", "unit": "m", "dist": {"type": "uniform", "lo": -1.5e-4, "hi": 2.5e-4}},
        {"name": "stagger_common_deg", "unit": "deg", "dist": {"type": "normal", "mean": 0.2, "sd": 0.4, "lo": -0.6, "hi": 0.9}},
        {"name": "chord_rel", "unit": "1", "dist": {"type": "beta", "a": 2.0, "b": 3.5, "lo": -0.012, "hi": 0.018}},
    ])
}

fn uq_spec(name: &str, params: Vec<UncertainParam>) -> UqSpec {
    UqSpec {
        name: name.into(),
        params,
        sampler: Sampler::MonteCarlo,
        seed: SEED,
        stop: StopRule {
            batch: 5,
            min_batches: 1,
            max_samples: 10,
            se_abs: 0.0,
            se_rel: 0.0,
            metrics: Vec::new(),
        },
        quantiles: vec![0.05, 0.5, 0.95],
        confidence: 0.95,
        requirements: Vec::new(),
        workers: 1,
    }
}

fn sobol_spec(name: &str, params: Vec<UncertainParam>) -> SobolSpec {
    SobolSpec {
        name: name.into(),
        params,
        n: 3,
        seed: SEED,
        workers: 1,
    }
}

/// Arguments both tools share, for the fan `m` of family `naca4`.
fn base_args(name: &str, m: &FanRomModel, params: Value) -> Value {
    json!({
        "ledger": format!("{name}.jsonl"), "name": name, "evaluator": "rcfd.fan_rom",
        "evaluator_settings": settings(m, "naca4"), "params": params, "seed": SEED,
    })
}

fn with(mut args: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        args[k.as_str()] = v.clone();
    }
    args
}

fn without(mut args: Value, key: &str) -> Value {
    args.as_object_mut().unwrap().remove(key).unwrap();
    args
}

/// `args` with the registered `stop` of `design_uq`.
fn uq_args(args: Value) -> Value {
    with(args, json!({"stop": {"batch": 5, "max_samples": 10}}))
}

fn sobol_args(args: Value) -> Value {
    with(args, json!({"n": 3}))
}

// ---------------------------------------------------------------- servers, files, texts

fn service(tag: &str, cap: Option<usize>) -> (PathBuf, Arc<Service>) {
    let root = scratch(&format!("f339-args-{tag}")).join("root");
    std::fs::create_dir_all(&root).unwrap();
    let mut cfg = ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    };
    if let Some(c) = cap {
        cfg.max_design_evaluations = c;
    }
    (root, Service::new(cfg))
}

fn sandbox(tag: &str) -> (PathBuf, McpServer) {
    let (root, svc) = service(tag, None);
    (root, McpServer::new(svc, Policy::default()))
}

fn text(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

/// The result without the spec it repeats, as text: the numbers.
fn numbers(v: &Value) -> String {
    let mut v = v.clone();
    v.as_object_mut().unwrap().remove("spec").unwrap();
    text(&v)
}

/// The lines of a ledger file as text with the stamp (which must be above 0) set to 0.
fn ledger_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            assert!(v["recorded_at_ms"].as_u64().unwrap() > 0, "an entry without a stamp");
            v["recorded_at_ms"] = json!(0);
            text(&v)
        })
        .collect()
}

fn message(r: &Value) -> String {
    r["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// No file in the sandbox (the folder itself is there).
fn empty(root: &Path, what: &str) {
    let left: Vec<_> = std::fs::read_dir(root).unwrap().flatten().map(|e| e.path()).collect();
    assert!(left.is_empty(), "{what}: a refused call wrote {left:?}");
}

/// A run of the library: its result and the lines of its ledger.
struct Reference {
    result: Value,
    lines: Vec<String>,
}

impl Reference {
    fn numbers(&self) -> String {
        numbers(&self.result)
    }
}

fn reference_ledger(tag: &str) -> Ledger {
    let mut l = Ledger::new(&scratch(&format!("f339-args-{tag}-ref")).join("ledger.jsonl"));
    l.clock = || 1;
    l
}

/// `uq::run` with the guard of the set-up: at least two usable samples.
fn uq_reference(tag: &str, spec: &UqSpec, m: &FanRomModel) -> Reference {
    let l = reference_ledger(tag);
    let res = uq::run(spec, m, Some(&l)).unwrap();
    assert!(res.n_ok >= 2, "{tag}: set-up: {} usable samples", res.n_ok);
    Reference {
        result: uq::result_json(spec, &res),
        lines: ledger_lines(l.path()),
    }
}

/// `sensitivity::run` with the guard of the set-up: at least two usable rows.
fn sobol_reference(tag: &str, spec: &SobolSpec, m: &FanRomModel) -> Reference {
    let l = reference_ledger(tag);
    let res = sensitivity::run(spec, m, Some(&l)).unwrap();
    assert!(res.n_rows_used >= 2, "{tag}: set-up: {} usable rows", res.n_rows_used);
    Reference {
        result: sensitivity::result_json(spec, &res),
        lines: ledger_lines(l.path()),
    }
}

/// The numbers of `uq::run` on a wrong input (no guard: it only has to differ).
fn uq_numbers(spec: &UqSpec, m: &FanRomModel) -> String {
    numbers(&uq::result_json(spec, &uq::run(spec, m, None).unwrap()))
}

fn sobol_numbers(spec: &SobolSpec, m: &FanRomModel) -> String {
    numbers(&sensitivity::result_json(
        spec,
        &sensitivity::run(spec, m, None).unwrap(),
    ))
}

/// The reply `r` (and the ledger `file`) is the reference.
fn equal(what: &str, r: &Value, file: &Path, reference: &Reference) {
    assert!(
        text(&r["result"]) == text(&reference.result),
        "{what}: the result differs from the library's"
    );
    let lines = ledger_lines(file);
    assert_eq!(lines.len(), reference.lines.len(), "{what}: ledger lines");
    assert_eq!(r["entries_appended"], lines.len(), "{what}: entries_appended");
    for (k, (a, b)) in lines.iter().zip(&reference.lines).enumerate() {
        assert!(a == b, "{what}: ledger line {k} differs");
    }
}

/// `args` sent to `tool` on `s` give the reference.
fn sent(what: &str, root: &Path, s: &McpServer, tool: &str, args: Value, reference: &Reference) {
    let file = root.join(args["ledger"].as_str().unwrap());
    let r = ok(&call(s, tool, args));
    equal(what, &r, &file, reference);
}

/// `args` with another study name (and ledger) for the references of one name: the name is in the texts, so a row that
/// sends several forms of one study uses a ledger each.
fn in_ledger(args: Value, ledger: &str) -> Value {
    with(args, json!({ "ledger": ledger }))
}

/// `args` sent to `tool` are refused with -32602 and a message that starts with `key`.
fn refused(what: &str, s: &McpServer, tool: &str, args: Value, key: &str) {
    let r = call(s, tool, args);
    assert_eq!(err_code(&r), -32602, "{what}: {r}");
    assert!(
        message(&r).starts_with(key),
        "{what}: '{}' does not start with '{key}'",
        message(&r)
    );
}

// ---------------------------------------------------------------- the rows

/// B1: `lo` and `hi` as strings, null and absent, in every pair, both tools: no truncation.
#[test]
fn b1_open_bounds_in_every_form() {
    let (root, s) = sandbox("b1");
    let m = naca();
    let uq_ref = uq_reference("b1-uq", &uq_spec("b1", d1_open()), &m);
    let sobol_ref = sobol_reference("b1-sobol", &sobol_spec("b1", d1_open()), &m);
    // a finite bound on either side changes the numbers of both drivers
    for (lo, hi) in [(-0.2, 8.0), (-8.0, 0.25)] {
        assert!(
            uq_numbers(&uq_spec("b1", d1(lo, hi)), &m) != uq_ref.numbers(),
            "B1 control: uq on [{lo}, {hi}]"
        );
        assert!(
            sobol_numbers(&sobol_spec("b1", d1(lo, hi)), &m) != sobol_ref.numbers(),
            "B1 control: sobol on [{lo}, {hi}]"
        );
    }
    let mut calls = 0;
    for (i, lo) in [Some(json!("-inf")), Some(Value::Null), None].into_iter().enumerate() {
        for (j, hi) in [Some(json!("inf")), Some(Value::Null), None].into_iter().enumerate() {
            let mut b = Map::new();
            if let Some(v) = &lo {
                b.insert("lo".into(), v.clone());
            }
            if let Some(v) = &hi {
                b.insert("hi".into(), v.clone());
            }
            let what = format!("B1 lo {lo:?} hi {hi:?}");
            let args = base_args("b1", &m, d1_json(Value::Object(b)));
            let uq = in_ledger(uq_args(args.clone()), &format!("uq-{i}{j}.jsonl"));
            sent(&what, &root, &s, "design_uq", uq, &uq_ref);
            let sobol = in_ledger(sobol_args(args), &format!("sobol-{i}{j}.jsonl"));
            sent(&what, &root, &s, "design_sobol", sobol, &sobol_ref);
            calls += 2;
        }
    }
    assert_eq!(calls, 18);
    eprintln!("B1: {calls} calls equal to the library's untruncated study; a finite bound on either side differs");
}

/// B2: any other string (or type) as a bound is refused with its place.
#[test]
fn b2_other_strings_are_refused() {
    let (root, s) = sandbox("b2");
    let m = naca();
    let others = [
        json!("Infinity"),
        json!("-Infinity"),
        json!("-Inf"),
        json!("INF"),
        json!("+inf"),
        json!("nan"),
        json!("NaN"),
        json!("none"),
        json!(""),
        json!(" inf"),
        json!("-0.5"),
        json!("0.5"),
        json!(true),
        json!([]),
        json!({}),
    ];
    for key in ["lo", "hi"] {
        for v in &others {
            let args = base_args("b2", &m, d1_json(json!({ key: v })));
            let place = format!("params[0].dist.{key}: expected a finite number");
            refused(&format!("B2 {key} {v}"), &s, "design_uq", uq_args(args.clone()), &place);
            refused(&format!("B2 {key} {v}"), &s, "design_sobol", sobol_args(args), &place);
        }
    }
    empty(&root, "B2");
    eprintln!(
        "B2: {} values refused for lo and hi, both tools, nothing written",
        others.len()
    );
}

/// B3: the infinity of the other side is not taken as an open bound: the spec's check refuses it.
#[test]
fn b3_the_infinity_of_the_other_side_is_refused() {
    let (root, s) = sandbox("b3");
    let m = naca();
    for b in [
        json!({"lo": "inf"}),
        json!({"lo": "inf", "hi": 0.5}),
        json!({"hi": "-inf"}),
        json!({"lo": -0.5, "hi": "-inf"}),
    ] {
        let args = base_args("b3", &m, d1_json(b.clone()));
        let key = "parameter 'stagger_common_deg': truncation";
        refused(&format!("B3 {b}"), &s, "design_uq", uq_args(args.clone()), key);
        refused(&format!("B3 {b}"), &s, "design_sobol", sobol_args(args), key);
    }
    empty(&root, "B3");
    eprintln!("B3: 4 pairs refused by the spec's check, both tools, nothing written");
}

/// B4: a uniform and a beta distribution have no open bound.
#[test]
fn b4_uniform_and_beta_bounds_are_numbers() {
    let (root, s) = sandbox("b4");
    let m = naca();
    let dists = [
        json!({"type": "uniform", "lo": -0.5, "hi": 0.5}),
        json!({"type": "beta", "a": 2.0, "b": 3.5, "lo": -0.5, "hi": 0.5}),
    ];
    let mut n = 0;
    for dist in &dists {
        for (key, string) in [("lo", "-inf"), ("hi", "inf")] {
            for form in [Some(json!(string)), Some(Value::Null), None] {
                let mut d = dist.clone();
                d.as_object_mut().unwrap().remove(key);
                if let Some(v) = &form {
                    d[key] = v.clone();
                }
                let what = format!("B4 {d}");
                let params = json!([{"name": "stagger_common_deg", "unit": "deg", "dist": d}]);
                let place = format!("params[0].dist.{key}: expected a finite number");
                refused(&what, &s, "design_uq", uq_args(base_args("b4", &m, params)), &place);
                n += 1;
            }
        }
    }
    assert_eq!(n, 12);
    empty(&root, "B4");
    eprintln!("B4: {n} open bounds of a uniform and a beta refused, nothing written");
}

/// D1: the numbers of the three distributions and the order of the inputs.
#[test]
fn d1_the_numbers_of_the_distributions() {
    let (root, s) = sandbox("d1");
    let m = naca();
    let uq_ref = uq_reference("d1-uq", &uq_spec("d1", d3()), &m);
    let sobol_ref = sobol_reference("d1-sobol", &sobol_spec("d1", d3()), &m);
    let wrong = |i: usize, dist: Dist| {
        let mut p = d3();
        p[i].dist = dist;
        p
    };
    let beta = |a: f64, b: f64| Dist::Beta {
        a,
        b,
        lo: -0.012,
        hi: 0.018,
    };
    let normal = |mean: f64, lo: f64, hi: f64| Dist::Normal { mean, sd: 0.4, lo, hi };
    let d = d3();
    let cases = [
        ("beta a and b swapped", wrong(2, beta(3.5, 2.0))),
        ("beta b read as a", wrong(2, beta(2.0, 2.0))),
        ("mean dropped", wrong(1, normal(0.0, -0.6, 0.9))),
        ("bounds dropped", wrong(1, normal(0.2, -INF, INF))),
        ("normal bounds mirrored", wrong(1, normal(0.2, -0.9, 0.6))),
        (
            "uniform bounds mirrored",
            wrong(
                0,
                Dist::Uniform {
                    lo: -2.5e-4,
                    hi: 1.5e-4,
                },
            ),
        ),
        ("the evaluator's order", vec![d[1].clone(), d[2].clone(), d[0].clone()]),
    ];
    for (what, p) in &cases {
        assert!(
            uq_numbers(&uq_spec("d1", p.clone()), &m) != uq_ref.numbers(),
            "D1 control, uq: {what}"
        );
        assert!(
            sobol_numbers(&sobol_spec("d1", p.clone()), &m) != sobol_ref.numbers(),
            "D1 control, sobol: {what}"
        );
    }
    let args = base_args("d1", &m, d3_json());
    let uq = in_ledger(uq_args(args.clone()), "uq.jsonl");
    sent("D1 design_uq", &root, &s, "design_uq", uq, &uq_ref);
    let sobol = in_ledger(sobol_args(args), "sobol.jsonl");
    sent("D1 design_sobol", &root, &s, "design_sobol", sobol, &sobol_ref);
    eprintln!("D1: both tools equal; {} wrong readings differ in both", cases.len());
}

/// E1: every field of the evaluator's settings, none at a natural value.
#[test]
fn e1_every_field_of_the_settings() {
    let (root, s) = sandbox("e1");
    let m = naca();
    // set-up: 8 sections, no two with an equal value in a field
    assert_eq!(m.fan.sections.len(), 8);
    for (i, a) in m.fan.sections.iter().enumerate() {
        for b in &m.fan.sections[..i] {
            assert!(
                a.radius != b.radius
                    && a.chord != b.chord
                    && a.stagger != b.stagger
                    && a.camber != b.camber
                    && a.camber_pos != b.camber_pos
            );
        }
    }
    let uq_ref = uq_reference("e1-uq", &uq_spec("e1", d3()), &m);
    let sobol_ref = sobol_reference("e1-sobol", &sobol_spec("e1", d3()), &m);
    type Wrong = (&'static str, fn(&mut FanRomModel));
    let wrong: Vec<Wrong> = vec![
        ("n_strips", |m| m.model.n_strips = RomModel::default().n_strips),
        ("cl_max", |m| m.model.cl_max = RomModel::default().cl_max),
        ("cd_normal", |m| m.model.cd_normal = RomModel::default().cd_normal),
        ("friction", |m| m.model.friction = RomModel::default().friction),
        ("annulus_drag", |m| {
            m.model.annulus_drag = RomModel::default().annulus_drag
        }),
        ("secondary_drag", |m| {
            m.model.secondary_drag = RomModel::default().secondary_drag
        }),
        ("tip_cd", |m| m.model.tip_cd = RomModel::default().tip_cd),
        ("rpm", |m| m.cond.op.rpm = 3000.0),
        ("rho", |m| m.cond.op.rho = 1.2),
        ("nu", |m| m.cond.op.nu = 1.5e-5),
        ("system_k", |m| m.cond.system.k = 0.0),
        ("system_l", |m| m.cond.system.l = 0.0),
        ("c0", |m| m.cond.c0 = 343.0),
        ("observer_r", |m| m.cond.observer_r = 1.0),
        ("observer_theta", |m| {
            m.cond.observer_theta = std::f64::consts::FRAC_PI_2
        }),
        ("n_scan", |m| m.cond.n_scan = 100),
        ("n_blades", |m| m.fan.n_blades = 8),
        ("r_hub", |m| m.fan.r_hub = m.fan.sections[0].radius),
        ("r_tip", |m| m.fan.r_tip = m.fan.sections[7].radius),
        ("tip_gap", |m| m.fan.tip_gap = 0.001),
        ("family", |m| {
            m.fan.family = serde_json::from_value(json!("circular_arc")).unwrap()
        }),
        // a radius of the second section in the first would not be increasing: the mean of the two
        ("radius", |m| {
            m.fan.sections[0].radius = 0.5 * (m.fan.sections[0].radius + m.fan.sections[1].radius)
        }),
        ("chord", |m| m.fan.sections[0].chord = m.fan.sections[1].chord),
        ("stagger", |m| m.fan.sections[0].stagger = m.fan.sections[1].stagger),
        ("camber", |m| m.fan.sections[0].camber = m.fan.sections[1].camber),
        ("camber_pos", |m| {
            m.fan.sections[0].camber_pos = m.fan.sections[1].camber_pos
        }),
    ];
    let mut same_numbers = Vec::new();
    for (what, f) in &wrong {
        let mut w = m.clone();
        f(&mut w);
        assert!(w != m, "E1 control: {what} is not another input");
        let l = reference_ledger(&format!("e1-{what}"));
        let spec = uq_spec("e1", d3());
        let res = uq::run(&spec, &w, Some(&l)).unwrap();
        let first = ledger_lines(l.path()).swap_remove(0);
        assert!(
            first != uq_ref.lines[0],
            "E1 control: {what} gives the same ledger entry"
        );
        if numbers(&uq::result_json(&spec, &res)) == uq_ref.numbers() {
            same_numbers.push(*what);
        }
    }
    let args = base_args("e1", &m, d3_json());
    let uq = in_ledger(uq_args(args.clone()), "uq.jsonl");
    sent("E1 design_uq", &root, &s, "design_uq", uq, &uq_ref);
    let sobol = in_ledger(sobol_args(args), "sobol.jsonl");
    sent("E1 design_sobol", &root, &s, "design_sobol", sobol, &sobol_ref);
    eprintln!(
        "E1: both tools equal; {} wrong fields differ in the ledger, all but {same_numbers:?} in the numbers too",
        wrong.len()
    );
}

/// E2: the family `circular_arc`.
#[test]
fn e2_circular_arc() {
    let (root, s) = sandbox("e2");
    let m = model("circular_arc");
    let uq_ref = uq_reference("e2-uq", &uq_spec("e2", d3()), &m);
    let sobol_ref = sobol_reference("e2-sobol", &sobol_spec("e2", d3()), &m);
    let mut w = m.clone();
    w.fan.family = serde_json::from_value(json!("naca4")).unwrap();
    assert!(
        uq_numbers(&uq_spec("e2", d3()), &w) != uq_ref.numbers(),
        "E2 control, uq"
    );
    assert!(
        sobol_numbers(&sobol_spec("e2", d3()), &w) != sobol_ref.numbers(),
        "E2 control, sobol"
    );
    let args = with(
        base_args("e2", &m, d3_json()),
        json!({"evaluator_settings": settings(&m, "circular_arc")}),
    );
    let uq = in_ledger(uq_args(args.clone()), "uq.jsonl");
    sent("E2 design_uq", &root, &s, "design_uq", uq, &uq_ref);
    let sobol = in_ledger(sobol_args(args), "sobol.jsonl");
    sent("E2 design_sobol", &root, &s, "design_sobol", sobol, &sobol_ref);
    eprintln!("E2: both tools equal on the circular arc; naca4 differs");
}

fn seeded_uq(name: &str, seed: u64) -> UqSpec {
    let mut u = uq_spec(name, d3());
    u.seed = seed;
    u
}

fn seeded_sobol(name: &str, seed: u64) -> SobolSpec {
    let mut b = sobol_spec(name, d3());
    b.seed = seed;
    b
}

/// S1: the seed as sent, also above 2^53.
#[test]
fn s1_the_seed_is_the_one_sent() {
    let (root, s) = sandbox("s1");
    let m = naca();
    let big: u64 = (1 << 53) + 1;
    for (seed, wrong) in [(SEED, 1), (big, 1 << 53)] {
        let uq_ref = uq_reference("s1-uq", &seeded_uq("s1", seed), &m);
        let sobol_ref = sobol_reference("s1-sobol", &seeded_sobol("s1", seed), &m);
        assert!(
            uq_numbers(&seeded_uq("s1", wrong), &m) != uq_ref.numbers(),
            "S1 control, uq: {seed}"
        );
        assert!(
            sobol_numbers(&seeded_sobol("s1", wrong), &m) != sobol_ref.numbers(),
            "S1 control, sobol: {seed}"
        );
        let args = with(base_args("s1", &m, d3_json()), json!({ "seed": seed }));
        let uq = in_ledger(uq_args(args.clone()), &format!("uq-{seed}.jsonl"));
        sent("S1 design_uq", &root, &s, "design_uq", uq, &uq_ref);
        let sobol = in_ledger(sobol_args(args), &format!("sobol-{seed}.jsonl"));
        sent("S1 design_sobol", &root, &s, "design_sobol", sobol, &sobol_ref);
    }
    eprintln!("S1: both tools equal for {SEED} and 2^53 + 1; seed 1 and 2^53 differ");
}

/// S2: no seed is seed 1.
#[test]
fn s2_no_seed_is_seed_1() {
    let (root, s) = sandbox("s2");
    let m = naca();
    let uq_ref = uq_reference("s2-uq", &seeded_uq("s2", 1), &m);
    let sobol_ref = sobol_reference("s2-sobol", &seeded_sobol("s2", 1), &m);
    for wrong in [0, 2] {
        assert!(
            uq_numbers(&seeded_uq("s2", wrong), &m) != uq_ref.numbers(),
            "S2 control, uq: {wrong}"
        );
        assert!(
            sobol_numbers(&seeded_sobol("s2", wrong), &m) != sobol_ref.numbers(),
            "S2 control, sobol: {wrong}"
        );
    }
    let args = without(base_args("s2", &m, d3_json()), "seed");
    let uq = in_ledger(uq_args(args.clone()), "uq.jsonl");
    sent("S2 design_uq", &root, &s, "design_uq", uq, &uq_ref);
    let sobol = in_ledger(sobol_args(args), "sobol.jsonl");
    sent("S2 design_sobol", &root, &s, "design_sobol", sobol, &sobol_ref);
    eprintln!("S2: both tools without a seed equal seed 1; seeds 0 and 2 differ");
}

/// C1: both comparisons, two requirements in their order.
#[test]
fn c1_both_comparisons_of_the_requirements() {
    let (root, s) = sandbox("c1");
    let m = naca();
    let req = |metric: &str, cmp: Cmp, limit: f64| Requirement {
        metric: metric.into(),
        cmp,
        limit,
    };
    let spec_with = |a: Requirement, b: Requirement| {
        let mut spec = uq_spec("c1", d1_open());
        spec.requirements = vec![a, b];
        spec
    };
    let spec = spec_with(req("q", Cmp::Le, 1.0), req("eta_s", Cmp::Ge, 2.0));
    let reference = uq_reference("c1", &spec, &m);
    let wrong = [
        spec_with(req("q", Cmp::Ge, 1.0), req("eta_s", Cmp::Ge, 2.0)),
        spec_with(req("q", Cmp::Le, 1.0), req("eta_s", Cmp::Le, 2.0)),
        spec_with(req("q", Cmp::Ge, 1.0), req("eta_s", Cmp::Le, 2.0)),
        spec_with(req("eta_s", Cmp::Ge, 2.0), req("q", Cmp::Le, 1.0)),
    ];
    for w in &wrong {
        assert!(
            uq_numbers(w, &m) != reference.numbers(),
            "C1 control: {:?}",
            w.requirements
        );
    }
    let args = with(
        uq_args(base_args("c1", &m, d1_json(json!({})))),
        json!({"requirements": [
            {"metric": "q", "cmp": "<=", "limit": 1.0}, {"metric": "eta_s", "cmp": ">=", "limit": 2.0},
        ]}),
    );
    sent("C1", &root, &s, "design_uq", args, &reference);
    eprintln!("C1: equal; the comparisons exchanged one by one, both, and the order differ");
}

/// A1: a stopping rule on an absolute standard error over two metrics.
#[test]
fn a1_an_absolute_target() {
    let (root, s) = sandbox("a1");
    let m = naca();
    let spec_with = |se_abs: f64, min_batches: usize| {
        let mut spec = uq_spec("a1", d1_open());
        spec.stop = StopRule {
            batch: 4,
            min_batches,
            max_samples: 16,
            se_abs,
            se_rel: 0.0,
            metrics: vec!["q".into(), "torque".into()],
        };
        spec
    };
    let reference = uq_reference("a1", &spec_with(1.0, 2), &m);
    assert_eq!(reference.result["batches"], 2, "A1 set-up");
    assert!(
        uq_numbers(&spec_with(0.0, 2), &m) != reference.numbers(),
        "A1 control: se_abs dropped"
    );
    assert!(
        uq_numbers(&spec_with(1.0, 1), &m) != reference.numbers(),
        "A1 control: min_batches dropped"
    );
    let args = with(
        base_args("a1", &m, d1_json(json!({}))),
        json!({"stop": {"batch": 4, "max_samples": 16, "min_batches": 2, "se_abs": 1.0, "metrics": ["q", "torque"]}}),
    );
    sent("A1", &root, &s, "design_uq", args, &reference);
    eprintln!("A1: equal, 2 batches of 4; without se_abs or min_batches the numbers differ");
}

/// A2: the sampler by name and the defaults of `min_batches`.
#[test]
fn a2_the_sampler_and_the_default_batches() {
    let (root, s) = sandbox("a2");
    let m = naca();
    let spec_with = |sampler: Sampler, min_batches: usize| {
        let mut spec = uq_spec("a2", d1_open());
        spec.sampler = sampler;
        spec.stop = StopRule {
            batch: 4,
            min_batches,
            max_samples: 16,
            se_abs: 1.0,
            se_rel: 0.0,
            metrics: vec!["q".into()],
        };
        spec
    };
    use Sampler::{Lhs, MonteCarlo};
    type Row = (&'static str, Value, (Sampler, usize), Vec<(Sampler, usize)>);
    let rows: Vec<Row> = vec![
        ("default", json!({}), (MonteCarlo, 1), vec![(MonteCarlo, 2), (Lhs, 2)]),
        (
            "monte_carlo",
            json!({"sampler": "monte_carlo"}),
            (MonteCarlo, 1),
            vec![(MonteCarlo, 2), (Lhs, 2)],
        ),
        (
            "lhs",
            json!({"sampler": "lhs"}),
            (Lhs, 2),
            vec![(Lhs, 3), (MonteCarlo, 2)],
        ),
        (
            "lhs-3",
            json!({"sampler": "lhs", "min_batches": 3}),
            (Lhs, 3),
            vec![(Lhs, 2), (MonteCarlo, 3)],
        ),
    ];
    for (what, sent_args, (sampler, batches), wrong) in &rows {
        let reference = uq_reference("a2", &spec_with(*sampler, *batches), &m);
        assert_eq!(reference.result["batches"], *batches, "A2 set-up: {what}");
        for (ws, wb) in wrong {
            assert!(
                uq_numbers(&spec_with(*ws, *wb), &m) != reference.numbers(),
                "A2 control: {what} read as {ws:?} with {wb}"
            );
        }
        let mut stop = json!({"batch": 4, "max_samples": 16, "se_abs": 1.0, "metrics": ["q"]});
        if let Some(b) = sent_args.get("min_batches") {
            stop["min_batches"] = b.clone();
        }
        let mut args = with(base_args("a2", &m, d1_json(json!({}))), json!({ "stop": stop }));
        if let Some(v) = sent_args.get("sampler") {
            args["sampler"] = v.clone();
        }
        let args = in_ledger(args, &format!("{what}.jsonl"));
        sent(&format!("A2 {what}"), &root, &s, "design_uq", args, &reference);
    }
    eprintln!("A2: 4 forms equal; the other sampler and other batch counts differ");
}

/// N1: null in a nested object is the default, as an absent key is.
#[test]
fn n1_nested_null_is_the_default() {
    let (root, s) = sandbox("n1");
    let m = FanRomModel {
        model: RomModel::default(),
        ..naca()
    };
    let mut spec = uq_spec("n1", d1_open());
    spec.stop.batch = 3;
    spec.stop.max_samples = 3;
    let reference = uq_reference("n1", &spec, &m);
    assert!(
        uq_numbers(&spec, &naca()) != reference.numbers(),
        "N1 control: another model"
    );
    let args = with(
        base_args("n1", &m, d1_json(json!({}))),
        json!({"stop": {"batch": 3, "max_samples": 3}}),
    );
    let mut absent = in_ledger(args.clone(), "absent.jsonl");
    absent["evaluator_settings"].as_object_mut().unwrap().remove("model");
    sent("N1 absent", &root, &s, "design_uq", absent, &reference);
    let mut null = in_ledger(args, "null.jsonl");
    null["evaluator_settings"]["model"] = Value::Null;
    for key in ["min_batches", "se_abs", "se_rel", "metrics"] {
        null["stop"][key] = Value::Null;
    }
    sent("N1 null", &root, &s, "design_uq", null, &reference);
    eprintln!("N1: the default model and stop from absent keys and from null");
}

/// N2: null for an optional top-level argument is the default in the handlers; the server's argument check refuses it.
#[test]
fn n2_top_level_null_is_the_default_of_the_handlers() {
    let (root, svc) = service("n2", None);
    let s = McpServer::new(svc.clone(), Policy::default());
    let m = naca();
    let uq_ref = uq_reference("n2-uq", &seeded_uq("n2", 1), &m);
    let sobol_ref = sobol_reference("n2-sobol", &seeded_sobol("n2", 1), &m);
    let args = base_args("n2", &m, d3_json());
    let mut uq = in_ledger(uq_args(args.clone()), "uq.jsonl");
    let uq_keys = ["seed", "sampler", "quantiles", "confidence", "requirements", "workers"];
    for key in uq_keys {
        // through the server: the argument check refuses the type
        let mut one = uq.clone();
        one[key] = Value::Null;
        let r = rpc_call(&s, "design_uq", one);
        assert_eq!(r["error"]["code"], -32602, "N2 {key}: {r}");
    }
    for key in ["seed", "workers"] {
        let mut one = sobol_args(args.clone());
        one[key] = Value::Null;
        let r = rpc_call(&s, "design_sobol", one);
        assert_eq!(r["error"]["code"], -32602, "N2 sobol {key}: {r}");
    }
    empty(&root, "N2");
    for key in uq_keys {
        uq[key] = Value::Null;
    }
    let r = uq_study(&svc, &uq).unwrap();
    equal("N2 uq_study", &r, &root.join("uq.jsonl"), &uq_ref);
    let mut sobol = in_ledger(sobol_args(args), "sobol.jsonl");
    for key in ["seed", "workers"] {
        sobol[key] = Value::Null;
    }
    let r = sobol_study(&svc, &sobol).unwrap();
    equal("N2 sobol_study", &r, &root.join("sobol.jsonl"), &sobol_ref);
    eprintln!("N2: null refused by the server for 8 arguments; the handlers take it as the default");
}

/// W1: `workers` reaches the spec: 0 is refused by its check.
#[test]
fn w1_workers_0_is_refused() {
    let (root, svc) = service("w1", None);
    let args = with(base_args("w1", &naca(), d1_json(json!({}))), json!({"workers": 0}));
    for (what, r) in [
        ("uq", uq_study(&svc, &uq_args(args.clone()))),
        ("sobol", sobol_study(&svc, &sobol_args(args.clone()))),
    ] {
        let e = r.expect_err(what);
        assert_eq!(e.code, -32602, "W1 {what}: {}", e.message);
        assert!(e.message.starts_with("workers:"), "W1 {what}: {}", e.message);
    }
    empty(&root, "W1");
    eprintln!("W1: workers 0 refused by both handlers, nothing written");
}

/// G1: the limit of `design_sobol` counts n (d + 2) evaluations, here with d = 1.
#[test]
fn g1_the_limit_counts_the_inputs() {
    let (root, svc) = service("g1", Some(100));
    let s = McpServer::new(svc, Policy::default());
    let m = naca();
    let args = |n: usize| with(base_args("g1", &m, d1_json(json!({}))), json!({ "n": n }));
    let r = call(&s, "design_sobol", args(34));
    assert_eq!(err_code(&r), 1004, "G1: {r}");
    assert!(
        message(&r).contains("102 evaluations") && message(&r).contains("max_design_evaluations is 100"),
        "G1: {r}"
    );
    empty(&root, "G1");
    let r = ok(&call(&s, "design_sobol", args(33)));
    assert_eq!(r["entries_appended"], 100);
    assert_eq!(r["result"]["n_evaluations"], 99);
    assert_eq!(ledger_lines(&root.join("g1.jsonl")).len(), 100);
    eprintln!("G1: 102 evaluations refused at the limit 100, 99 accepted with one input");
}

/// K1, a record outside the decision rule of ADR 0791: one open side and one finite bound.
#[test]
#[ignore = "known miss, ADR 0791 K1: the library panics on a normal distribution open on one side (erfc of infinity is NaN)"]
fn k1_one_open_side_known_miss() {
    let (root, s) = sandbox("k1");
    let m = naca();
    for (i, (b, lo, hi)) in [(json!({"hi": 0.25}), -INF, 0.25), (json!({"lo": -0.2}), -0.2, INF)]
        .into_iter()
        .enumerate()
    {
        let args = in_ledger(uq_args(base_args("k1", &m, d1_json(b.clone()))), &format!("{i}.jsonl"));
        let r = ok(&call(&s, "design_uq", args));
        assert_eq!(r["entries_appended"], 11, "K1 {b}");
        for l in ledger_lines(&root.join(format!("{i}.jsonl"))).iter().take(10) {
            let e: Value = serde_json::from_str(l).unwrap();
            let x = e["params"][0]["value"].as_f64().unwrap();
            assert!((lo..=hi).contains(&x), "K1 {b}: {x}");
        }
    }
}
