//! ADR 0792 (F-894): `design_uq` and `design_sobol` with a normal distribution open on one side. Rows D2 (every form of the
//! open side gives the library's study, bit for bit, and differs from the untruncated one), K1g (the limits of the record K1
//! of ADR 0791, `design_uq_sobol_args_f339.rs`), C4 (the refusal) and E1 (the server answers such a call on stdio and lives).
//! The set-up is the fan, the model and the input d1 of ADR 0791.

mod common;

use common::*;
use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition};
use rcfd_api::design::ledger::Ledger;
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::uq::{self, Dist, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_post::fan_rom::{free_vortex_design, FanRom, Operating, RomFan, RomModel, RomSection, SystemCurve};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const Q_D: f64 = 0.045;
const SEED: u64 = 792_004;
const INF: f64 = f64::INFINITY;

// ---------------------------------------------------------------- the set-up of ADR 0791

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

/// F(naca4) with the model Mo and the condition of ADR 0791.
fn naca() -> FanRomModel {
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
        family: serde_json::from_value(json!("naca4")).unwrap(),
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

/// `evaluator_settings` of `m`, every field by hand.
fn settings(m: &FanRomModel) -> Value {
    let (c, r) = (&m.cond, &m.model);
    json!({
        "fan": {
            "n_blades": m.fan.n_blades, "r_hub": m.fan.r_hub, "r_tip": m.fan.r_tip, "tip_gap": m.fan.tip_gap,
            "family": "naca4",
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

fn sobol_args(ledger: &str, m: &FanRomModel, bounds: Value) -> Value {
    json!({
        "ledger": format!("{ledger}.jsonl"), "name": "s", "evaluator": "rcfd.fan_rom",
        "evaluator_settings": settings(m), "params": d1_json(bounds), "seed": SEED, "n": 3,
    })
}

fn tool_args(tool: &str, ledger: &str, m: &FanRomModel, bounds: Value) -> Value {
    let mut args = sobol_args(ledger, m, bounds);
    if tool == "design_uq" {
        args.as_object_mut().unwrap().remove("n").unwrap();
        args["stop"] = json!({"batch": 5, "max_samples": 10});
    }
    args
}

fn sandbox(tag: &str) -> (PathBuf, McpServer) {
    let root = scratch(&format!("f894-{tag}")).join("root");
    std::fs::create_dir_all(&root).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    (root, McpServer::new(svc, Policy::default()))
}

/// The lines of a ledger file as text with the stamp set to 0.
fn ledger_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            assert!(v["recorded_at_ms"].as_u64().unwrap() > 0, "an entry without a stamp");
            v["recorded_at_ms"] = json!(0);
            v.to_string()
        })
        .collect()
}

/// The staggers of the evaluations in a ledger.
fn staggers(path: &Path) -> Vec<f64> {
    ledger_lines(path)
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["kind"] == "evaluation")
        .map(|e| e["params"][0]["value"].as_f64().unwrap())
        .collect()
}

/// The library's study with its ledger lines: `design_uq` or `design_sobol` on d1 with the bounds.
fn reference(tag: &str, tool: &str, m: &FanRomModel, lo: f64, hi: f64) -> (Value, Vec<String>) {
    let mut l = Ledger::new(&scratch(&format!("f894-{tag}-ref")).join("ledger.jsonl"));
    l.clock = || 1;
    let result = if tool == "design_uq" {
        let spec = uq_spec("s", d1(lo, hi));
        let r = uq::run(&spec, m, Some(&l)).unwrap();
        assert!(r.n_ok >= 2, "{tag}: set-up: {} usable samples", r.n_ok);
        uq::result_json(&spec, &r)
    } else {
        let spec = sobol_spec("s", d1(lo, hi));
        let r = sensitivity::run(&spec, m, Some(&l)).unwrap();
        assert!(r.n_rows_used >= 2, "{tag}: set-up: {} usable rows", r.n_rows_used);
        sensitivity::result_json(&spec, &r)
    };
    (result, ledger_lines(l.path()))
}

/// The two inputs: one finite bound, the other side in each of its three forms.
fn inputs() -> [(f64, f64, [Value; 3]); 2] {
    [
        (
            -INF,
            0.25,
            [
                json!({"hi": 0.25}),
                json!({"hi": 0.25, "lo": null}),
                json!({"hi": 0.25, "lo": "-inf"}),
            ],
        ),
        (
            -0.2,
            INF,
            [
                json!({"lo": -0.2}),
                json!({"lo": -0.2, "hi": null}),
                json!({"lo": -0.2, "hi": "inf"}),
            ],
        ),
    ]
}

// ---------------------------------------------------------------- the rows

/// D2: every form of the open side is the library's study, and not the untruncated one.
#[test]
fn d2_every_form_of_the_open_side() {
    let (root, s) = sandbox("d2");
    let m = naca();
    let mut calls = 0;
    for tool in ["design_uq", "design_sobol"] {
        let open = reference(&format!("d2-{tool}-open"), tool, &m, -INF, INF);
        for (i, (lo, hi, forms)) in inputs().into_iter().enumerate() {
            let (result, lines) = reference(&format!("d2-{tool}-{i}"), tool, &m, lo, hi);
            assert!(
                lines != open.1,
                "D2 control: {tool} [{lo}, {hi}] is the untruncated study"
            );
            for (j, b) in forms.into_iter().enumerate() {
                let ledger = format!("{tool}-{i}{j}");
                let r = ok(&call(&s, tool, tool_args(tool, &ledger, &m, b.clone())));
                let file = root.join(format!("{ledger}.jsonl"));
                assert_eq!(r["result"].to_string(), result.to_string(), "D2 {tool} {b}");
                assert_eq!(ledger_lines(&file), lines, "D2 {tool} {b}: the ledger");
                assert_eq!(r["entries_appended"], lines.len(), "D2 {tool} {b}");
                for x in staggers(&file) {
                    assert!((lo..=hi).contains(&x), "D2 {tool} {b}: {x}");
                }
                calls += 1;
            }
        }
    }
    assert_eq!(calls, 12);
    eprintln!("D2: {calls} calls equal to the library's one-sided study, the untruncated one differs");
}

/// K1g: the limits of the record K1 of ADR 0791, and the same for `design_sobol`.
#[test]
fn k1_gated_one_open_side() {
    let (root, s) = sandbox("k1");
    let m = naca();
    for (tool, entries) in [("design_uq", 11), ("design_sobol", 10)] {
        for (i, (b, lo, hi)) in [(json!({"hi": 0.25}), -INF, 0.25), (json!({"lo": -0.2}), -0.2, INF)]
            .into_iter()
            .enumerate()
        {
            let ledger = format!("{tool}-{i}");
            let r = ok(&call(&s, tool, tool_args(tool, &ledger, &m, b.clone())));
            assert_eq!(r["entries_appended"], entries, "K1g {tool} {b}");
            let x = staggers(&root.join(format!("{ledger}.jsonl")));
            assert_eq!(x.len(), entries - 1, "K1g {tool} {b}");
            for x in x {
                assert!((lo..=hi).contains(&x), "K1g {tool} {b}: {x}");
            }
        }
    }
}

/// C4: a kept mass below 1e-12 is refused by name and nothing is written.
#[test]
fn c4_a_kept_mass_below_the_limit_is_refused() {
    let (root, s) = sandbox("c4");
    let m = naca();
    for tool in ["design_uq", "design_sobol"] {
        for b in [json!({"lo": 2.212}), json!({"hi": -2.012})] {
            let r = call(&s, tool, tool_args(tool, "c4", &m, b.clone()));
            assert_eq!(err_code(&r), -32602, "C4 {tool} {b}: {r}");
            let t = r["structuredContent"]["error"]["message"].as_str().unwrap();
            assert!(
                t.starts_with("parameter 'stagger_common_deg': truncation") && t.contains("keeps a probability"),
                "C4 {tool} {b}: {t}"
            );
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "C4 {tool} {b}: a file");
        }
    }
}

/// E1: the built server answers a one-sided call of each tool and the ping after it.
#[test]
fn e1_the_server_answers_and_lives() {
    let root = scratch("f894-e1").join("root");
    std::fs::create_dir_all(&root).unwrap();
    let m = naca();
    let mut server = Mcp::start(&["--root", root.to_str().unwrap()]);
    for (tool, b) in [
        ("design_uq", json!({"hi": 0.25})),
        ("design_sobol", json!({"lo": -0.2})),
    ] {
        let r = server.call(tool, tool_args(tool, tool, &m, b.clone()));
        let pong = server.request("ping", json!({}));
        eprintln!("E1 {tool} {b}: isError {}, ping answered", r["isError"]);
        assert_eq!(pong["result"], json!({}), "E1: {pong}");
        assert_eq!(r["isError"], false, "E1 {tool} {b}: {r}");
        assert!(root.join(format!("{tool}.jsonl")).is_file());
    }
}
