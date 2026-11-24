//! ADR 0784 (F-339): the MCP tools `design_uq` and `design_sobol` run `uq::run` and `sensitivity::run` on the named
//! evaluator `rcfd.fan_rom` (the reduced-order fan) inside the sandbox, with the ledger. U1, U2, S1: the reply is the
//! library's result bit for bit; L1 to L3: the ledger; B1 to B3 and P1: the sandbox, the limit, the permissions and project
//! mode; E1: the refusals; G1, G2: the tool lists are the earlier ones with the two tools added.
//!
//! The reference model and specs are built here by hand from the Rust types and share no code with the tools' parsers.

mod common;

use common::*;
use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition};
use rcfd_api::design::ledger::{Ledger, Model};
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::study::Cmp;
use rcfd_api::design::uq::{self, Dist, Requirement, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::{Policy, PERMISSION_DENIED};
use rcfd_mcp::McpServer;
use rcfd_post::fan_rom::{free_vortex_design, FanRom, Operating, RomFan, RomModel, RomSection, SystemCurve};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The list without `--project` as the golden holds it.
const DEFAULT: &str = include_str!("golden/tools_list_default.json");

/// The two tools of this item, in the order of the list.
const NEW: [&str; 2] = ["design_uq", "design_sobol"];

const Q_D: f64 = 0.045;

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

/// The fan of set-up C of ADR 0119 with its system curve through the design point, as `fan_rom_bands_f268.rs` builds it,
/// with section and loss model `m`.
fn model_with(m: RomModel) -> FanRomModel {
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
        family: serde_json::from_value(json!("naca4")).unwrap(),
        sections: vec![s(0.030), s(0.058)],
    };
    let d = RomModel::default();
    let fan = free_vortex_design(&base, &d, &op(), Q_D, 45.0 / 1.2 / op().omega()).unwrap();
    let k = FanRom::new(fan.clone(), d).unwrap().point(&op(), Q_D).unwrap().dp_s / (Q_D * Q_D);
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

fn model() -> FanRomModel {
    model_with(RomModel::default())
}

/// `evaluator_settings` of `m`, written field by field (no `model` object: the default model).
fn settings(m: &FanRomModel) -> Value {
    let c = &m.cond;
    json!({
        "fan": {
            "n_blades": m.fan.n_blades, "r_hub": m.fan.r_hub, "r_tip": m.fan.r_tip, "tip_gap": m.fan.tip_gap,
            "family": "naca4",
            "sections": m.fan.sections.iter().map(|s| json!({
                "radius": s.radius, "chord": s.chord, "stagger": s.stagger, "camber": s.camber, "camber_pos": s.camber_pos,
            })).collect::<Vec<_>>(),
        },
        "condition": {
            "rpm": c.op.rpm, "rho": c.op.rho, "nu": c.op.nu, "system_k": c.system.k, "system_l": c.system.l,
            "c0": c.c0, "observer_r": c.observer_r, "observer_theta": c.observer_theta, "n_scan": c.n_scan,
        },
    })
}

/// The three inputs over the ranges of T3 of ADR 0623: a normal (truncated or not), a beta and a uniform distribution.
fn params(truncated: bool) -> Vec<UncertainParam> {
    let (lo, hi) = if truncated {
        (-1.0, 1.0)
    } else {
        (f64::NEG_INFINITY, f64::INFINITY)
    };
    vec![
        UncertainParam {
            name: "stagger_common_deg".into(),
            unit: "deg".into(),
            dist: Dist::Normal {
                mean: 0.0,
                sd: 0.5,
                lo,
                hi,
            },
        },
        UncertainParam {
            name: "chord_rel".into(),
            unit: "1".into(),
            dist: Dist::Beta {
                a: 2.0,
                b: 2.0,
                lo: -0.015,
                hi: 0.02,
            },
        },
        UncertainParam {
            name: "tip_gap_delta".into(),
            unit: "m".into(),
            dist: Dist::Uniform { lo: -2e-4, hi: 2e-4 },
        },
    ]
}

fn params_json(truncated: bool) -> Value {
    let stagger = if truncated {
        json!({"type": "normal", "mean": 0.0, "sd": 0.5, "lo": -1.0, "hi": 1.0})
    } else {
        json!({"type": "normal", "mean": 0.0, "sd": 0.5})
    };
    json!([
        {"name": "stagger_common_deg", "unit": "deg", "dist": stagger},
        {"name": "chord_rel", "unit": "1", "dist": {"type": "beta", "a": 2.0, "b": 2.0, "lo": -0.015, "hi": 0.02}},
        {"name": "tip_gap_delta", "unit": "m", "dist": {"type": "uniform", "lo": -2e-4, "hi": 2e-4}},
    ])
}

/// Arguments both tools share.
fn base_args(ledger: &str, name: &str, seed: u64, truncated: bool) -> Value {
    json!({
        "ledger": ledger, "name": name, "evaluator": "rcfd.fan_rom", "evaluator_settings": settings(&model()),
        "params": params_json(truncated), "seed": seed,
    })
}

fn with(mut args: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        args[k.as_str()] = v.clone();
    }
    args
}

fn uq_spec(name: &str, seed: u64, batch: usize, max_samples: usize) -> UqSpec {
    UqSpec {
        name: name.into(),
        params: params(true),
        sampler: Sampler::MonteCarlo,
        seed,
        stop: StopRule {
            batch,
            min_batches: 1,
            max_samples,
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

fn sobol_spec(name: &str, seed: u64, n: usize, workers: usize) -> SobolSpec {
    SobolSpec {
        name: name.into(),
        params: params(true),
        n,
        seed,
        workers,
    }
}

/// U1's spec: Monte Carlo, 4 batches of 64 (targets 0 are never met).
fn u1_spec(seed: u64) -> UqSpec {
    let mut s = uq_spec("u1", seed, 64, 256);
    s.stop.min_batches = 2;
    s
}

fn u1_args(ledger: &str) -> Value {
    with(
        base_args(ledger, "u1", 339001, true),
        json!({"stop": {"batch": 64, "min_batches": 2, "max_samples": 256}}),
    )
}

fn s1_args(ledger: &str) -> Value {
    with(base_args(ledger, "s1", 339003, true), json!({"n": 64, "workers": 2}))
}

/// A server over a sandbox root that sits in a folder of its own (so `../x` stays inside the test's scratch folder).
fn sandbox(tag: &str, cap: Option<usize>, policy: Policy) -> (PathBuf, McpServer) {
    let root = scratch(&format!("f339-{tag}")).join("root");
    std::fs::create_dir_all(&root).unwrap();
    let mut cfg = ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    };
    if let Some(c) = cap {
        cfg.max_design_evaluations = c;
    }
    (root, McpServer::new(Service::new(cfg), policy))
}

/// A ledger of the reference runs, outside every sandbox, with the fixed clock 1.
fn reference_ledger(tag: &str) -> Ledger {
    let mut l = Ledger::new(&scratch(&format!("f339-{tag}-ref")).join("ledger.jsonl"));
    l.clock = || 1;
    l
}

fn text(v: &Value) -> String {
    serde_json::to_string(v).unwrap()
}

fn lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// A ledger line as text with its time stamp (which must be above 0) set to 0.
fn unstamped(v: &Value) -> String {
    let mut v = v.clone();
    assert!(v["recorded_at_ms"].as_u64().unwrap() > 0, "an entry without a stamp");
    v["recorded_at_ms"] = json!(0);
    text(&v)
}

fn message(r: &Value) -> String {
    r["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Every file under `dir`.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push(e.path());
            }
        }
    }
    out
}

/// The ledger of the tool against the ledger of the library: the same number of lines, each equal as text but for the stamp.
fn same_ledger(tool: &Path, reference: &Ledger, entries: usize, last_kind: &str, what: &str) {
    let (a, b) = (lines(tool), lines(reference.path()));
    assert_eq!(
        (a.len(), b.len()),
        (entries, entries),
        "{what}: lines of the two ledgers"
    );
    for (k, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!(unstamped(x) == unstamped(y), "{what}: ledger line {k} differs");
    }
    assert_eq!(a[entries - 1]["kind"], last_kind, "{what}");
    assert!(
        a[..entries - 1].iter().all(|e| e["kind"] == "evaluation"),
        "{what}: every entry before the summary is an evaluation"
    );
}

/// U1, L1, L3 with the controls NC1 and NC3: `design_uq` on the Monte Carlo set-up.
#[test]
fn u1_l1_l3_design_uq_is_uq_run_with_the_ledger() {
    let (root, s) = sandbox("u1", None, Policy::default());
    let r = ok(&call(&s, "design_uq", u1_args("fan/ledger.jsonl")));
    let m = model();
    let spec = u1_spec(339001);
    let reference = reference_ledger("u1");
    let res = uq::run(&spec, &m, Some(&reference)).unwrap();
    // set-up: 4 batches of 64, the eight metrics of the evaluator, usable samples
    assert_eq!((res.n_samples, res.batches, res.metrics.len()), (256, 4, 8));
    assert!(res.n_ok >= 1);
    // U1: the reply's result is the library's, byte for byte (so every float by bits)
    let want = text(&uq::result_json(&spec, &res));
    assert!(text(&r["result"]) == want, "U1: design_uq differs from uq::run");
    assert_eq!(r["evaluator"], "rcfd.fan_rom");
    assert_eq!(r["evaluator_version"], m.version());
    assert_eq!(r["fidelity"], "reduced_order");
    // NC1: another seed gives another text
    let other = u1_spec(339002);
    let other_res = uq::run(&other, &m, None).unwrap();
    let mut other_json = uq::result_json(&other, &other_res);
    other_json["spec"]["seed"] = json!(339001);
    assert!(text(&other_json) != want, "NC1: the result does not depend on the seed");
    // L1: the ledger file is the one the library writes, but for the stamps
    let file = root.join("fan/ledger.jsonl");
    assert_eq!(r["entries_appended"], 257);
    assert_eq!(r["ledger"], file.display().to_string());
    same_ledger(&file, &reference, 257, "uq_summary", "L1");
    // NC3: as raw bytes the two files differ (the reference has the clock 1, the tool the system clock)
    let raw = std::fs::read(&file).unwrap();
    assert!(raw != std::fs::read(reference.path()).unwrap(), "NC3");
    assert!(lines(&file).iter().all(|e| e["recorded_at_ms"].as_u64().unwrap() > 1));
    // L3: the ledger tools read it
    let q = ok(&call(
        &s,
        "design_ledger_query",
        json!({"ledger": "fan/ledger.jsonl", "kind": "uq_summary"}),
    ));
    assert_eq!((q["total"].as_u64(), q["matched"].as_u64()), (Some(257), Some(1)));
    assert_eq!(q["entries"][0]["study"], "u1");
    assert!(q["skipped_lines"].as_array().unwrap().is_empty());
    // L3: the same study name again is refused and the file keeps its bytes
    let again = call(&s, "design_uq", u1_args("fan/ledger.jsonl"));
    assert_eq!(err_code(&again), -32602, "{again}");
    assert!(
        message(&again).starts_with("name:") && message(&again).contains("'u1'"),
        "{again}"
    );
    assert!(
        std::fs::read(&file).unwrap() == raw,
        "L3: a refused call changed the ledger"
    );
    // L3: another study appends after it
    let a = ok(&call(
        &s,
        "design_sobol",
        with(base_args("fan/ledger.jsonl", "s-after", 339006, true), json!({"n": 2})),
    ));
    assert_eq!(a["entries_appended"], 11);
    let after = std::fs::read(&file).unwrap();
    assert!(
        after.starts_with(&raw) && lines(&file).len() == 268,
        "L3: the second study appends"
    );
    eprintln!(
        "U1 L1 L3: 256 samples ({} ok), 257 entries equal but for the stamp, {} bytes of result; NC1 NC3 hold",
        res.n_ok,
        want.len()
    );
}

/// U2: `design_uq` with the Latin hypercube, a stopping rule on `q` and a requirement, the stagger not truncated.
#[test]
fn u2_design_uq_lhs_with_a_stopping_rule_and_a_requirement() {
    let (root, s) = sandbox("u2", None, Policy::default());
    let args = with(
        base_args("ledger.jsonl", "u2", 339002, false),
        json!({
            "sampler": "lhs", "stop": {"batch": 32, "max_samples": 128, "se_rel": 0.05, "metrics": ["q"]},
            "requirements": [{"metric": "q", "cmp": ">=", "limit": Q_D}],
        }),
    );
    let r = ok(&call(&s, "design_uq", args));
    let spec = UqSpec {
        name: "u2".into(),
        params: params(false),
        sampler: Sampler::Lhs,
        seed: 339002,
        stop: StopRule {
            batch: 32,
            min_batches: 2,
            max_samples: 128,
            se_abs: 0.0,
            se_rel: 0.05,
            metrics: vec!["q".into()],
        },
        quantiles: vec![0.05, 0.5, 0.95],
        confidence: 0.95,
        requirements: vec![Requirement {
            metric: "q".into(),
            cmp: Cmp::Ge,
            limit: Q_D,
        }],
        workers: 1,
    };
    let reference = reference_ledger("u2");
    let res = uq::run(&spec, &model(), Some(&reference)).unwrap();
    assert!((2..=4).contains(&res.batches) && res.n_samples == 32 * res.batches);
    assert_eq!(res.requirements.len(), 1);
    assert!(
        text(&r["result"]) == text(&uq::result_json(&spec, &res)),
        "U2: design_uq differs from uq::run"
    );
    assert_eq!(r["entries_appended"], res.n_samples + 1);
    same_ledger(
        &root.join("ledger.jsonl"),
        &reference,
        res.n_samples + 1,
        "uq_summary",
        "U2",
    );
    eprintln!(
        "U2: {} replicates of 32, converged {}, P(q < Q_D) = {} ({} of {})",
        res.batches,
        res.converged,
        res.requirements[0].p,
        res.requirements[0].violations + res.requirements[0].failed,
        res.requirements[0].n
    );
}

/// S1, L2: `design_sobol`, 64 rows, 2 workers.
#[test]
fn s1_l2_design_sobol_is_sensitivity_run_with_the_ledger() {
    let (root, s) = sandbox("s1", None, Policy::default());
    let r = ok(&call(&s, "design_sobol", s1_args("sobol/ledger.jsonl")));
    let m = model();
    let spec = sobol_spec("s1", 339003, 64, 2);
    let reference = reference_ledger("s1");
    let res = sensitivity::run(&spec, &m, Some(&reference)).unwrap();
    assert_eq!((res.n_rows, res.n_evaluations, res.metrics.len()), (64, 320, 8));
    assert!(res.n_rows_used >= 2);
    assert!(
        text(&r["result"]) == text(&sensitivity::result_json(&spec, &res)),
        "S1: design_sobol differs from sensitivity::run"
    );
    assert_eq!(r["evaluator"], "rcfd.fan_rom");
    assert_eq!(r["evaluator_version"], m.version());
    assert_eq!(r["fidelity"], "reduced_order");
    assert_eq!(r["entries_appended"], 321);
    same_ledger(&root.join("sobol/ledger.jsonl"), &reference, 321, "sobol_summary", "L2");
    eprintln!(
        "S1 L2: 320 evaluations, {} of 64 rows used, 321 entries equal but for the stamp",
        res.n_rows_used
    );
}

/// B1: a ledger path that leaves the sandbox and a name against the name rule are refused, and nothing is written.
#[test]
fn b1_paths_stay_inside_the_sandbox() {
    let (root, s) = sandbox("b1", None, Policy::default());
    let outside = scratch("f339-b1-outside").join("x.jsonl");
    let parent = root.parent().unwrap().to_path_buf();
    for (tool, extra) in [
        ("design_uq", json!({"stop": {"batch": 4, "max_samples": 4}})),
        ("design_sobol", json!({"n": 2})),
    ] {
        for path in ["../out.jsonl", "a/../../out.jsonl", outside.to_str().unwrap()] {
            let r = call(&s, tool, with(base_args(path, "b1", 1, true), extra.clone()));
            assert_eq!(err_code(&r), 1004, "{tool} {path}: {r}");
            assert!(message(&r).contains("sandbox"), "{tool} {path}: {r}");
        }
        let r = call(&s, tool, with(base_args("ok.jsonl", "a/b", 1, true), extra.clone()));
        assert_eq!(err_code(&r), -32602, "{tool}: {r}");
        assert!(message(&r).starts_with("name "), "{tool}: {r}");
    }
    assert!(!outside.exists() && !parent.join("out.jsonl").exists());
    assert!(
        files(&parent).is_empty(),
        "B1: a refused call wrote {:?}",
        files(&parent)
    );
    eprintln!("B1: 6 paths refused with 1004, 2 names with -32602, nothing written");
}

/// B2 with the control NC2: the limit `max_design_evaluations` of the service bounds both tools.
#[test]
fn b2_the_evaluation_limit_of_the_service_applies() {
    let (root, s) = sandbox("b2", Some(100), Policy::default());
    let uq_args = |name: &str, max: usize| {
        with(
            base_args(&format!("{name}.jsonl"), name, 339004, true),
            json!({"stop": {"batch": 50, "max_samples": max}}),
        )
    };
    let sobol_args =
        |name: &str, n: usize| with(base_args(&format!("{name}.jsonl"), name, 339004, true), json!({"n": n}));
    for (tool, args, count) in [
        ("design_uq", uq_args("uq-over", 101), "101"),
        ("design_sobol", sobol_args("sobol-over", 21), "105"),
    ] {
        let r = call(&s, tool, args);
        assert_eq!(err_code(&r), 1004, "{tool}: {r}");
        let msg = message(&r);
        assert!(
            msg.contains("max_design_evaluations is 100") && msg.contains(count),
            "{tool}: {msg}"
        );
    }
    assert!(files(&root).is_empty(), "B2: a refused call wrote {:?}", files(&root));
    // NC2: at the limit both are accepted
    let r = ok(&call(&s, "design_uq", uq_args("uq-at", 100)));
    assert_eq!(r["entries_appended"], 101);
    assert_eq!(lines(&root.join("uq-at.jsonl")).len(), 101);
    let r = ok(&call(&s, "design_sobol", sobol_args("sobol-at", 20)));
    assert_eq!(r["entries_appended"], 101);
    assert_eq!(lines(&root.join("sobol-at.jsonl")).len(), 101);
    eprintln!("B2: 101 samples and 105 evaluations refused with 1004 at the limit 100; 100 and 100 accepted (NC2)");
}

/// B3: the class `write_files` of both tools, their descriptors, and the refusal of a read-only server.
#[test]
fn b3_both_tools_write_files_and_a_read_only_server_denies_them() {
    let (root, s) = sandbox("b3", None, Policy::read_only());
    for (tool, args) in [
        ("design_uq", u1_args("ledger.jsonl")),
        ("design_sobol", s1_args("ledger.jsonl")),
    ] {
        let r = call(&s, tool, args);
        assert_eq!(err_code(&r), PERMISSION_DENIED, "{tool}: {r}");
    }
    assert!(files(&root).is_empty());
    let list = s.tool_list();
    let list = list["tools"].as_array().unwrap();
    let required = [
        json!(["ledger", "name", "evaluator", "evaluator_settings", "params", "stop"]),
        json!(["ledger", "name", "evaluator", "evaluator_settings", "params", "n"]),
    ];
    for (name, req) in NEW.iter().zip(&required) {
        let t = list
            .iter()
            .find(|t| t["name"] == *name)
            .unwrap_or_else(|| panic!("{name} is not listed"));
        assert_eq!(t["_meta"]["rcfd/access"], "write_files", "{name}");
        assert_eq!(t["_meta"]["rcfd/needsApproval"], true, "{name}");
        assert_eq!(t["annotations"]["readOnlyHint"], false, "{name}");
        assert_eq!(t["annotations"]["destructiveHint"], false, "{name}");
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
        assert_eq!(&t["inputSchema"]["required"], req, "{name}");
    }
    eprintln!("B3: both tools write_files with approval, denied by a read-only server, nothing written");
}

/// P1: with a project both tools are listed with the same descriptors, and `design_sobol` runs inside the project folder.
#[test]
fn p1_project_mode_lists_and_runs_the_tools() {
    let dir = project("f339-p1");
    let s = server(&dir, Policy::default());
    let plain = McpServer::new(Service::new(ServiceConfig::default()), Policy::default()).tool_list();
    let with_project = s.tool_list();
    let entry = |list: &Value, name: &str| -> Value {
        list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} is not listed"))
            .clone()
    };
    for name in NEW {
        assert!(
            text(&entry(&with_project, name)) == text(&entry(&plain, name)),
            "P1: {name} differs with a project"
        );
    }
    let r = ok(&call(
        &s,
        "design_sobol",
        with(
            base_args("designs/f339/ledger.jsonl", "p1", 339005, true),
            json!({"n": 2}),
        ),
    ));
    let spec = sobol_spec("p1", 339005, 2, 1);
    let res = sensitivity::run(&spec, &model(), None).unwrap();
    assert_eq!(res.n_evaluations, 10);
    assert!(
        text(&r["result"]) == text(&sensitivity::result_json(&spec, &res)),
        "P1: design_sobol differs from sensitivity::run"
    );
    assert_eq!(r["entries_appended"], 11);
    assert_eq!(lines(&dir.join("designs/f339/ledger.jsonl")).len(), 11);
    eprintln!("P1: both descriptors as without a project; 10 evaluations, 11 entries in the project folder");
}

/// E1: the handler's refusals, each with its code and its key, and no ledger file.
#[test]
fn e1_refusals_name_their_key_and_write_nothing() {
    let (root, s) = sandbox("e1", None, Policy::default());
    let uq = |extra: Value| {
        with(
            with(
                base_args("ledger.jsonl", "e1", 1, true),
                json!({"stop": {"batch": 4, "max_samples": 8}}),
            ),
            extra,
        )
    };
    let m = model();
    let settings_with = |f: &dyn Fn(&mut Value)| {
        let mut v = settings(&m);
        f(&mut v);
        json!({"evaluator_settings": v})
    };
    let param = |name: &str, unit: &str, dist: Value| json!({"params": [{"name": name, "unit": unit, "dist": dist}]});
    let uniform = json!({"type": "uniform", "lo": -1.0, "hi": 1.0});
    let cases: Vec<(&str, &str, Value, &str)> = vec![
        (
            "evaluator",
            "design_uq",
            uq(json!({"evaluator": "rcfd.other"})),
            "evaluator: 'rcfd.other'",
        ),
        (
            "no fan",
            "design_uq",
            uq(settings_with(&|v| {
                v.as_object_mut().unwrap().remove("fan");
            })),
            "evaluator_settings.fan:",
        ),
        (
            "unknown key",
            "design_uq",
            uq(settings_with(&|v| v["fan"]["blades"] = json!(7))),
            "evaluator_settings.fan.blades:",
        ),
        (
            "missing section key",
            "design_uq",
            uq(settings_with(&|v| {
                v["fan"]["sections"][0].as_object_mut().unwrap().remove("camber");
            })),
            "evaluator_settings.fan.sections[0].camber:",
        ),
        (
            "invalid blade row",
            "design_uq",
            uq(settings_with(&|v| v["fan"]["sections"][1]["chord"] = json!(-0.03))),
            "evaluator_settings.fan.sections[1].chord:",
        ),
        (
            "invalid model",
            "design_uq",
            uq(settings_with(&|v| v["model"] = json!({"cl_max": -1.0}))),
            "evaluator_settings.model.cl_max:",
        ),
        (
            "missing condition key",
            "design_uq",
            uq(settings_with(&|v| {
                v["condition"].as_object_mut().unwrap().remove("rpm");
            })),
            "evaluator_settings.condition.rpm:",
        ),
        ("name", "design_uq", uq(json!({"name": ".hidden"})), "name '.hidden'"),
        ("no parameter", "design_uq", uq(json!({"params": []})), "params:"),
        (
            "foreign parameter",
            "design_uq",
            uq(param("blade_count", "1", uniform.clone())),
            "params[0].name: 'blade_count'",
        ),
        (
            "foreign unit",
            "design_uq",
            uq(param("stagger_common_deg", "rad", uniform.clone())),
            "params[0].unit:",
        ),
        (
            "distribution type",
            "design_uq",
            uq(param(
                "chord_rel",
                "1",
                json!({"type": "triangular", "lo": 0.0, "hi": 1.0}),
            )),
            "params[0].dist.type:",
        ),
        (
            "distribution refused by validate",
            "design_uq",
            uq(param(
                "tip_gap_delta",
                "m",
                json!({"type": "uniform", "lo": 1.0, "hi": 1.0}),
            )),
            "parameter 'tip_gap_delta':",
        ),
        (
            "stop key",
            "design_uq",
            uq(json!({"stop": {"max_samples": 8}})),
            "stop.batch:",
        ),
        (
            "stop refused by validate",
            "design_uq",
            uq(json!({"stop": {"batch": 8, "max_samples": 4}})),
            "stop:",
        ),
        (
            "lhs replicates",
            "design_uq",
            uq(json!({"sampler": "lhs", "stop": {"batch": 4, "max_samples": 8, "min_batches": 1}})),
            "stop.min_batches:",
        ),
        ("quantiles", "design_uq", uq(json!({"quantiles": [1.5]})), "quantiles:"),
        ("confidence", "design_uq", uq(json!({"confidence": 1.0})), "confidence:"),
        (
            "requirement",
            "design_uq",
            uq(json!({"requirements": [{"metric": "q", "cmp": "<", "limit": 1.0}]})),
            "requirements[0].cmp:",
        ),
        (
            "rows",
            "design_sobol",
            with(base_args("ledger.jsonl", "e1", 1, true), json!({"n": 1})),
            "n:",
        ),
        (
            "sobol evaluator",
            "design_sobol",
            with(
                base_args("ledger.jsonl", "e1", 1, true),
                json!({"n": 4, "evaluator": "case"}),
            ),
            "evaluator: 'case'",
        ),
    ];
    for (what, tool, args, key) in &cases {
        let r = call(&s, tool, args.clone());
        assert_eq!(err_code(&r), -32602, "{what}: {r}");
        assert!(
            message(&r).starts_with(key),
            "{what}: '{}' does not start with '{key}'",
            message(&r)
        );
    }
    // the argument check of the server refuses what the inputSchema excludes, as a JSON-RPC error
    let mut no_stop = uq(json!({}));
    no_stop.as_object_mut().unwrap().remove("stop");
    for (what, args) in [
        ("missing stop", no_stop),
        ("unknown argument", uq(json!({"bogus": 1}))),
        ("sampler", uq(json!({"sampler": "sobol"}))),
    ] {
        let r = rpc_call(&s, "design_uq", args);
        assert_eq!(r["error"]["code"], -32602, "{what}: {r}");
    }
    assert!(files(&root).is_empty(), "E1: a refused call wrote {:?}", files(&root));
    // control: the unchanged arguments are accepted
    let r = ok(&call(&s, "design_uq", uq(json!({}))));
    assert_eq!(r["entries_appended"], 9);
    eprintln!(
        "E1: {} handler refusals with -32602 and their key, 3 schema refusals, nothing written; control accepted",
        cases.len()
    );
}

/// The names of a tool list.
fn names(list: &[Value]) -> Vec<&str> {
    list.iter().map(|t| t["name"].as_str().unwrap()).collect()
}

/// The entries of `list` as text, without the two tools of this item and without `drop`.
fn without(list: &[Value], drop: &[&str]) -> Vec<String> {
    list.iter()
        .filter(|t| {
            let n = t["name"].as_str().unwrap();
            !NEW.contains(&n) && !drop.contains(&n)
        })
        .map(text)
        .collect()
}

/// The two tools sit right after `design_ledger_rerun_check`, in the order of [`NEW`].
fn placed(list: &[Value], what: &str) {
    let n = names(list);
    let at = n
        .iter()
        .position(|x| *x == "design_ledger_rerun_check")
        .unwrap_or_else(|| panic!("{what}: design_ledger_rerun_check is not listed"));
    assert_eq!(&n[at + 1..at + 3], &NEW[..], "{what}");
    assert_eq!(n.iter().filter(|x| NEW.contains(x)).count(), 2, "{what}");
}

/// G1: without a project the list is the golden's with the two tools added (and `create_project`, ADR 0731), nothing
/// else changed. The golden's own entries of the two tools, once it is re-recorded, are taken out on both sides.
#[test]
fn g1_the_list_without_a_project_is_the_golden_with_two_tools_added() {
    let live = McpServer::new(Service::new(ServiceConfig::default()), Policy::default()).tool_list();
    let live = live["tools"].as_array().unwrap();
    let gold: Value = serde_json::from_str(DEFAULT).unwrap();
    let gold = gold["tools"].as_array().unwrap();
    placed(live, "G1");
    let (l, g) = (without(live, &["create_project"]), without(gold, &[]));
    assert_eq!(l.len(), g.len(), "G1: the number of other tools");
    for (a, b) in l.iter().zip(&g) {
        assert!(
            a == b,
            "G1: an entry differs from the golden: {}",
            &a[..a.len().min(60)]
        );
    }
    assert_eq!(live.len(), l.len() + 3);
    eprintln!(
        "G1: {} live tools: {} byte-identical to the golden, design_uq, design_sobol, create_project",
        live.len(),
        l.len()
    );
}

/// G2: with a project the two tools are the entries of the list without one, and the kept session tools are the golden's
/// (but the class `execute` of ADR 0196's three).
#[test]
fn g2_the_project_list_gains_the_same_two_tools() {
    let dir = project("f339-g2");
    let s = server(&dir, Policy::default());
    let live = s.tool_list();
    let live = live["tools"].as_array().unwrap();
    placed(live, "G2");
    let gold: Value = serde_json::from_str(DEFAULT).unwrap();
    let kept: Vec<&Value> = gold["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            let n = t["name"].as_str().unwrap();
            !rcfd_mcp::SESSION_ONLY.contains(&n) && !NEW.contains(&n)
        })
        .collect();
    let others: Vec<&Value> = live
        .iter()
        .filter(|t| !NEW.contains(&t["name"].as_str().unwrap()))
        .collect();
    let mut execute = 0;
    for (l, k) in others.iter().zip(&kept) {
        let mut l = (*l).clone();
        if rcfd_mcp::PROJECT_EXECUTE.contains(&k["name"].as_str().unwrap()) {
            assert_eq!(l["_meta"]["rcfd/access"], "execute");
            l["_meta"]["rcfd/access"] = k["_meta"]["rcfd/access"].clone();
            execute += 1;
        }
        assert!(text(&l) == text(k), "G2: {} differs from the golden", k["name"]);
    }
    assert_eq!(execute, 3);
    assert!(others.len() > kept.len(), "the project tools follow");
    assert!(
        others[kept.len()..]
            .iter()
            .all(|t| !t["name"].as_str().unwrap().starts_with("design_")),
        "G2: no further design tool"
    );
    eprintln!(
        "G2: {} tools with a project: {} session tools byte-identical to the golden, the two new ones, {} project tools",
        live.len(),
        kept.len(),
        others.len() - kept.len()
    );
}

/// Not registered (an extra): a `model` object and explicit `quantiles` and `confidence` reach the evaluator and the spec.
#[test]
fn extra_model_and_quantiles_reach_the_library() {
    let (_root, s) = sandbox("extra", None, Policy::default());
    let rom = RomModel {
        n_strips: 20,
        friction: false,
        tip_cd: 0.5,
        ..RomModel::default()
    };
    let m = model_with(rom);
    let mut set = settings(&m);
    set["model"] = json!({"n_strips": 20, "friction": false, "tip_cd": 0.5});
    let args = with(
        base_args("ledger.jsonl", "extra", 339007, true),
        json!({
            "evaluator_settings": set, "stop": {"batch": 8, "max_samples": 16},
            "quantiles": [0.1, 0.9], "confidence": 0.9, "workers": 3,
        }),
    );
    let r = ok(&call(&s, "design_uq", args));
    let mut spec = uq_spec("extra", 339007, 8, 16);
    spec.quantiles = vec![0.1, 0.9];
    spec.confidence = 0.9;
    spec.workers = 3;
    let res = uq::run(&spec, &m, None).unwrap();
    assert!(text(&r["result"]) == text(&uq::result_json(&spec, &res)));
    let default_res = uq::run(&spec, &model(), None).unwrap();
    assert!(
        text(&uq::result_json(&spec, &default_res)) != text(&r["result"]),
        "the model object changes the result"
    );
}
