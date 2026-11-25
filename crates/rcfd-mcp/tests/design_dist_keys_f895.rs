//! ADR 0793 (F-895): a key of another distribution in `params[i].dist` of `design_uq` and `design_sobol` is refused by name,
//! and every other call gives main's bytes. Rows K1 to K8 (the eight keys), C (the same calls without the key), T (which of
//! several is named), N1 and N2 (before any sample, no file); U (what keeps its message), F1 (the forms `Dist::to_json`
//! writes), F2 (every form of the open sides of a normal), F3 (both drivers on three inputs).
//!
//! The rows F and U are compared with the record made on main bc6ddd16 before the change (`design_dist_keys_f895.tsv`: group,
//! number of lines, FNV-1a hash of the text), and the rows F with the library's own study. `F895_DUMP=<folder>` also writes
//! the table and every text there. The hashes of F are of one mathematical library, so there is one record per platform:
//! Windows x86_64 and Linux x86_64 (`design_dist_keys_f895.linux.tsv`). Another platform compares the names, the counts, the
//! refusals and the library of the same build.

mod common;

use common::*;
use rcfd_api::design::fan_rom_eval::{FanRomModel, RomCondition};
use rcfd_api::design::ledger::Ledger;
use rcfd_api::design::sensitivity::{self, SobolSpec};
use rcfd_api::design::tools::{sobol_study, uq_study};
use rcfd_api::design::uq::{self, Dist, Sampler, StopRule, UncertainParam, UqSpec};
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use rcfd_post::fan_rom::{free_vortex_design, FanRom, Operating, RomFan, RomModel, RomSection, SystemCurve};
use serde_json::{json, Value};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const Q_D: f64 = 0.045;
const SEED: u64 = 895_001;
const INF: f64 = f64::INFINITY;
const TOOLS: [&str; 2] = ["design_uq", "design_sobol"];
const WINDOWS: &str = include_str!("design_dist_keys_f895.tsv");
const LINUX: &str = include_str!("design_dist_keys_f895.linux.tsv");

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

// ---------------------------------------------------------------- the inputs U, N and B

fn normal(lo: f64, hi: f64) -> Dist {
    Dist::Normal {
        mean: 0.1,
        sd: 0.3,
        lo,
        hi,
    }
}

/// The input of a distribution: its name and unit go with its kind.
fn input(dist: Dist) -> UncertainParam {
    let (name, unit) = match dist {
        Dist::Uniform { .. } => ("tip_gap_delta", "m"),
        Dist::Normal { .. } => ("stagger_common_deg", "deg"),
        Dist::Beta { .. } => ("chord_rel", "1"),
    };
    UncertainParam {
        name: name.into(),
        unit: unit.into(),
        dist,
    }
}

/// U, N or B as the library takes it.
fn lib(kind: &str) -> UncertainParam {
    input(match kind {
        "uniform" => Dist::Uniform {
            lo: -1.5e-4,
            hi: 2.5e-4,
        },
        "normal" => normal(-0.2, 0.25),
        _ => Dist::Beta {
            a: 2.0,
            b: 3.5,
            lo: -0.012,
            hi: 0.018,
        },
    })
}

/// U, N or B as a call sends it, with the keys of `more` in `dist`.
fn sent(kind: &str, more: Value) -> Value {
    let mut v = match kind {
        "uniform" => {
            json!({"name": "tip_gap_delta", "unit": "m", "dist": {"type": "uniform", "lo": -1.5e-4, "hi": 2.5e-4}})
        }
        "normal" => json!({
            "name": "stagger_common_deg", "unit": "deg",
            "dist": {"type": "normal", "mean": 0.1, "sd": 0.3, "lo": -0.2, "hi": 0.25},
        }),
        _ => json!({
            "name": "chord_rel", "unit": "1", "dist": {"type": "beta", "a": 2.0, "b": 3.5, "lo": -0.012, "hi": 0.018},
        }),
    };
    for (k, x) in more.as_object().unwrap() {
        v["dist"][k.as_str()] = x.clone();
    }
    v
}

fn without(mut input: Value, key: &str) -> Value {
    input["dist"].as_object_mut().unwrap().remove(key).unwrap();
    input
}

// ---------------------------------------------------------------- calls

fn service(tag: &str, cap: Option<usize>) -> (PathBuf, Arc<Service>, McpServer) {
    let root = scratch(&format!("f895-{tag}")).join("root");
    std::fs::create_dir_all(&root).unwrap();
    let mut cfg = ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    };
    if let Some(c) = cap {
        cfg.max_design_evaluations = c;
    }
    let svc = Service::new(cfg);
    (root, svc.clone(), McpServer::new(svc, Policy::default()))
}

fn args(tool: &str, ledger: &str, m: &FanRomModel, params: Vec<Value>) -> Value {
    let mut a = json!({
        "ledger": ledger, "name": "s", "evaluator": "rcfd.fan_rom", "evaluator_settings": settings(m),
        "params": params, "seed": SEED,
    });
    if tool == "design_uq" {
        a["stop"] = json!({"batch": 5, "max_samples": 10});
    } else {
        a["n"] = json!(3);
    }
    a
}

/// The code and the message of a refused call through the server.
fn by_server(s: &McpServer, tool: &str, args: &Value) -> (i64, String) {
    let r = call(s, tool, args.clone());
    let code = err_code(&r);
    let t = r["structuredContent"]["error"]["message"].as_str().unwrap();
    (code, t.to_string())
}

/// The code and the message of a refused call of the handler itself.
fn by_handler(svc: &Service, tool: &str, args: &Value) -> (i64, String) {
    let r = if tool == "design_uq" {
        uq_study(svc, args)
    } else {
        sobol_study(svc, args)
    };
    match r {
        Ok(v) => panic!("{tool}: accepted: {v}"),
        Err(e) => (e.code, e.message),
    }
}

/// Nothing in the sandbox (the folder itself is there).
fn empty(root: &Path, what: &str) {
    let left: Vec<_> = std::fs::read_dir(root).unwrap().flatten().map(|e| e.path()).collect();
    assert!(left.is_empty(), "{what}: a refused call wrote {left:?}");
}

/// `params` in both tools, through the server and the handler, are refused with -32602 and the whole message `want`, and
/// nothing is written. The number of calls.
fn refused(
    what: &str,
    env: &(PathBuf, Arc<Service>, McpServer),
    m: &FanRomModel,
    params: Vec<Value>,
    want: &str,
) -> usize {
    let (root, svc, s) = env;
    for tool in TOOLS {
        let a = args(tool, "l.jsonl", m, params.clone());
        let want = (-32602, want.to_string());
        assert_eq!(by_server(s, tool, &a), want, "{what}: {tool}, server");
        empty(root, what);
        assert_eq!(by_handler(svc, tool, &a), want, "{what}: {tool}, handler");
        empty(root, what);
    }
    4
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
            v.to_string()
        })
        .collect()
}

/// The library's study: the text of the result and the lines of its ledger.
fn library(
    tag: &str,
    tool: &str,
    m: &FanRomModel,
    params: Vec<UncertainParam>,
    sampler: Sampler,
) -> (Value, Vec<String>) {
    let mut l = Ledger::new(&scratch(&format!("f895-{tag}-{tool}-ref")).join("ledger.jsonl"));
    l.clock = || 1;
    let result = if tool == "design_uq" {
        let spec = UqSpec {
            name: "s".into(),
            params,
            sampler,
            seed: SEED,
            stop: StopRule {
                batch: 5,
                min_batches: if sampler == Sampler::Lhs { 2 } else { 1 },
                max_samples: 10,
                se_abs: 0.0,
                se_rel: 0.0,
                metrics: Vec::new(),
            },
            quantiles: vec![0.05, 0.5, 0.95],
            confidence: 0.95,
            requirements: Vec::new(),
            workers: 1,
        };
        let r = uq::run(&spec, m, Some(&l)).unwrap();
        assert!(r.n_ok >= 2, "{tag}: set-up: {} usable samples", r.n_ok);
        uq::result_json(&spec, &r)
    } else {
        let spec = SobolSpec {
            name: "s".into(),
            params,
            n: 3,
            seed: SEED,
            workers: 1,
        };
        let r = sensitivity::run(&spec, m, Some(&l)).unwrap();
        assert!(r.n_rows_used >= 2, "{tag}: set-up: {} usable rows", r.n_rows_used);
        sensitivity::result_json(&spec, &r)
    };
    (result, ledger_lines(l.path()))
}

/// The accepted call `args` is the library's study `want`; its text for the record.
fn equal(what: &str, root: &Path, s: &McpServer, tool: &str, args: Value, want: &(Value, Vec<String>)) -> String {
    let file = root.join(args["ledger"].as_str().unwrap());
    let r = ok(&call(s, tool, args));
    let (got, expected) = (r["result"].to_string(), want.0.to_string());
    assert!(got == expected, "{what}: the result differs from the library's");
    let lines = ledger_lines(&file);
    assert!(lines == want.1, "{what}: the ledger differs from the library's");
    assert_eq!(r["entries_appended"], lines.len(), "{what}");
    format!("{}\n{}", r["result"], lines.join("\n"))
}

// ---------------------------------------------------------------- the record of main

/// The groups of one row: a line of the table each, and every text for the dump.
#[derive(Default)]
struct Groups {
    table: Vec<String>,
    dump: String,
}

impl Groups {
    fn text(&mut self, name: &str, t: &str) {
        let h = t.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        self.table.push(format!("{name}\t{}\t{h:016x}", t.lines().count()));
        for l in t.lines() {
            writeln!(self.dump, "{name}\t{l}").unwrap();
        }
    }

    /// Compare with the lines of the record that start with `row`, and write the dump when asked to. `everywhere`: the
    /// groups call no mathematical library, so that the record of Windows holds on every platform.
    fn check(self, row: &str, everywhere: bool) {
        if let Ok(dir) = std::env::var("F895_DUMP") {
            let dir = PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{row}.table.tsv")), self.table.join("\n") + "\n").unwrap();
            std::fs::write(dir.join(format!("{row}.texts.txt")), &self.dump).unwrap();
        }
        let of = |record: &str| -> Vec<String> {
            record
                .lines()
                .filter(|l| l.starts_with(row) && l[row.len()..].starts_with(' '))
                .map(str::to_string)
                .collect()
        };
        let head = |l: &String| l.rsplit_once('\t').unwrap().0.to_string();
        assert_eq!(
            self.table.iter().map(head).collect::<Vec<_>>(),
            of(WINDOWS).iter().map(head).collect::<Vec<_>>(),
            "{row}: groups of the record"
        );
        let record = if cfg!(all(windows, target_arch = "x86_64")) || everywhere {
            WINDOWS
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            LINUX
        } else {
            eprintln!("{row}: no record for this platform: {:#?}", self.table);
            return;
        };
        assert_eq!(self.table, of(record), "{row}: not the record of main");
        eprintln!("{row}: {} groups equal to main", self.table.len());
    }
}

// ---------------------------------------------------------------- what must not move

/// U: the key of no distribution, the type that is not known and the malformed numbers keep code and message.
#[test]
fn u_what_keeps_its_message() {
    let env = service("u", None);
    let m = naca();
    let mut cases: Vec<(Value, String)> = Vec::new();
    let unknown_key = |k: &str| format!("params[0].dist.{k}: unknown key (type, lo, hi, mean, sd, a, b)");
    let unknown_type = "params[0].dist.type: expected uniform, normal or beta".to_string();
    for (kind, foreign) in [("uniform", "mean"), ("normal", "a"), ("beta", "sd")] {
        for k in ["zz", "Mean", "min"] {
            cases.push((sent(kind, json!({ k: 1.0 })), unknown_key(k)));
            cases.push((sent(kind, json!({ k: 1.0, foreign: 1.0 })), unknown_key(k)));
            cases.push((sent(kind, json!({ k: 1.0, "type": "triangular" })), unknown_key(k)));
        }
        for t in [json!(null), json!(3), json!("Uniform"), json!("triangular"), json!("")] {
            cases.push((sent(kind, json!({ "type": t })), unknown_type.clone()));
        }
        cases.push((without(sent(kind, json!({})), "type"), unknown_type.clone()));
        let mut lo = vec![sent(kind, json!({"lo": "x"}))];
        if kind != "normal" {
            lo.push(without(sent(kind, json!({})), "lo"));
        }
        for v in lo {
            cases.push((v, "params[0].dist.lo: expected a finite number".into()));
        }
    }
    for k in ["mean", "sd", "a", "b"] {
        let types = [json!(null), json!(3), json!("Uniform"), json!("triangular"), json!("")];
        for t in types.into_iter().map(Some).chain([None]) {
            for x in [json!(0.5), json!(null)] {
                let mut v = json!({"name": "chord_rel", "unit": "1", "dist": {"lo": -0.01, "hi": 0.01, k: x}});
                if let Some(t) = &t {
                    v["dist"]["type"] = t.clone();
                }
                cases.push((v, unknown_type.clone()));
            }
        }
    }
    for d in [json!(null), json!("uniform"), json!([])] {
        let v = json!({"name": "chord_rel", "unit": "1", "dist": d});
        cases.push((v, "params[0].dist: expected an object".into()));
    }
    let v = json!({"name": "chord_rel", "unit": "1"});
    cases.push((v, "params[0].dist: expected an object".into()));
    let mut g = Groups::default();
    let mut n = 0;
    for (i, (v, want)) in cases.iter().enumerate() {
        let what = format!("U {i} {}", v["dist"]);
        // the texts of the four calls first, so that the record is of the replies and not of `want`
        for tool in TOOLS {
            let a = args(tool, "l.jsonl", &m, vec![v.clone()]);
            let (a, b) = (by_server(&env.2, tool, &a), by_handler(&env.1, tool, &a));
            let t = format!("{} {}\n{} {}", a.0, a.1, b.0, b.1);
            g.text(&format!("U {i} {tool}"), &t);
        }
        n += refused(&what, &env, &m, vec![v.clone()], want);
    }
    eprintln!("U: {} inputs, {n} calls refused with main's message", cases.len());
    g.check("U", true);
}

/// F1: the forms `Dist::to_json` writes, sent back.
#[test]
fn f1_the_forms_of_to_json() {
    let (root, _, s) = service("f1", None);
    let m = naca();
    let mut g = Groups::default();
    let dists = [
        lib("uniform").dist,
        lib("beta").dist,
        normal(-0.2, 0.25),
        normal(-INF, 0.25),
        normal(-0.2, INF),
        normal(-INF, INF),
    ];
    for (i, d) in dists.into_iter().enumerate() {
        let p = vec![input(d)];
        // the form as the library writes it
        let uq_ref = library(&format!("f1-{i}"), "design_uq", &m, p.clone(), Sampler::MonteCarlo);
        let written = uq_ref.0["spec"]["params"].as_array().unwrap().clone();
        assert_eq!(written.len(), 1);
        for tool in TOOLS {
            let want = if tool == "design_uq" {
                uq_ref.clone()
            } else {
                library(&format!("f1-{i}"), tool, &m, p.clone(), Sampler::MonteCarlo)
            };
            let what = format!("F1 {} {tool}", written[0]["dist"]);
            let a = args(tool, &format!("{tool}-{i}.jsonl"), &m, written.clone());
            g.text(&what, &equal(&what, &root, &s, tool, a, &want));
        }
    }
    assert_eq!(g.table.len(), 12);
    g.check("F1", false);
}

/// F2: every form of the open sides of a normal.
#[test]
fn f2_every_form_of_the_open_sides() {
    let (root, _, s) = service("f2", None);
    let m = naca();
    let mut g = Groups::default();
    let side = |open: &str, x: f64| [None, Some(json!(null)), Some(json!(open)), Some(json!(x))];
    for tool in TOOLS {
        for (i, lo) in side("-inf", -0.2).into_iter().enumerate() {
            for (j, hi) in side("inf", 0.25).into_iter().enumerate() {
                let mut v = without(without(sent("normal", json!({})), "lo"), "hi");
                if let Some(x) = &lo {
                    v["dist"]["lo"] = x.clone();
                }
                if let Some(x) = &hi {
                    v["dist"]["hi"] = x.clone();
                }
                let bounds = normal(if i == 3 { -0.2 } else { -INF }, if j == 3 { 0.25 } else { INF });
                let want = library("f2", tool, &m, vec![input(bounds)], Sampler::MonteCarlo);
                let what = format!("F2 {} {tool}", v["dist"]);
                let a = args(tool, &format!("{tool}-{i}{j}.jsonl"), &m, vec![v]);
                g.text(&what, &equal(&what, &root, &s, tool, a, &want));
            }
        }
    }
    assert_eq!(g.table.len(), 32);
    g.check("F2", false);
}

/// F3: both drivers on U, N and B in one study.
#[test]
fn f3_both_drivers_on_three_inputs() {
    let (root, _, s) = service("f3", None);
    let m = naca();
    let mut g = Groups::default();
    let kinds = ["uniform", "normal", "beta"];
    let p: Vec<UncertainParam> = kinds.into_iter().map(lib).collect();
    let v: Vec<Value> = kinds.into_iter().map(|k| sent(k, json!({}))).collect();
    for (i, (tool, sampler)) in [
        ("design_uq", Sampler::MonteCarlo),
        ("design_uq", Sampler::Lhs),
        ("design_sobol", Sampler::MonteCarlo),
    ]
    .into_iter()
    .enumerate()
    {
        let want = library(&format!("f3-{i}"), tool, &m, p.clone(), sampler);
        let mut a = args(tool, &format!("{i}.jsonl"), &m, v.clone());
        if sampler == Sampler::Lhs {
            a["sampler"] = json!("lhs");
            a["stop"]["min_batches"] = json!(2);
        }
        let what = format!("F3 {tool} {sampler:?}");
        g.text(&what, &equal(&what, &root, &s, tool, a, &want));
    }
    g.check("F3", false);
}

// ---------------------------------------------------------------- the foreign keys

/// The eight: a type and a key it does not have.
const FOREIGN: [(&str, &str); 8] = [
    ("uniform", "mean"),
    ("uniform", "sd"),
    ("uniform", "a"),
    ("uniform", "b"),
    ("normal", "a"),
    ("normal", "b"),
    ("beta", "mean"),
    ("beta", "sd"),
];

/// The registered message: `key` in the input `i`, of the type `kind`.
fn message(i: usize, kind: &str, key: &str) -> String {
    let keys = match kind {
        "uniform" => "type, lo, hi",
        "normal" => "type, mean, sd, lo, hi",
        _ => "type, a, b, lo, hi",
    };
    format!("params[{i}].dist.{key}: not a key of a {kind} distribution ({keys})")
}

/// The type of the other of two inputs.
fn beside(kind: &str) -> &'static str {
    if kind == "normal" {
        "beta"
    } else {
        "normal"
    }
}

/// `key` in a distribution of the type `kind` with every kind of value, in the first and in the second of two inputs.
fn foreign_key(row: &str, kind: &str, key: &str) {
    assert!(FOREIGN.contains(&(kind, key)));
    let env = service(row, None);
    let m = naca();
    let mut n = 0;
    for x in [
        json!(0.25),
        json!(null),
        json!("0.5"),
        json!(false),
        json!([]),
        json!({}),
    ] {
        let what = format!("{row}: {key} {x} in a {kind}");
        let (bad, right) = (sent(kind, json!({ key: x })), sent(beside(kind), json!({})));
        let first = vec![bad.clone(), right.clone()];
        n += refused(&what, &env, &m, first, &message(0, kind, key));
        n += refused(&what, &env, &m, vec![right, bad], &message(1, kind, key));
    }
    assert_eq!(n, 48);
    eprintln!("{row}: {key} in a {kind}: {n} calls refused, nothing written");
}

#[test]
fn k1_mean_in_a_uniform() {
    foreign_key("K1", "uniform", "mean");
}

#[test]
fn k2_sd_in_a_uniform() {
    foreign_key("K2", "uniform", "sd");
}

#[test]
fn k3_a_in_a_uniform() {
    foreign_key("K3", "uniform", "a");
}

#[test]
fn k4_b_in_a_uniform() {
    foreign_key("K4", "uniform", "b");
}

#[test]
fn k5_a_in_a_normal() {
    foreign_key("K5", "normal", "a");
}

#[test]
fn k6_b_in_a_normal() {
    foreign_key("K6", "normal", "b");
}

#[test]
fn k7_mean_in_a_beta() {
    foreign_key("K7", "beta", "mean");
}

#[test]
fn k8_sd_in_a_beta() {
    foreign_key("K8", "beta", "sd");
}

/// C: the calls of K without the key are accepted.
#[test]
fn c_the_same_calls_without_the_key() {
    let (root, _, s) = service("c", None);
    let m = naca();
    let mut n = 0;
    for kind in ["uniform", "normal", "beta"] {
        for (i, pair) in [[kind, beside(kind)], [beside(kind), kind]].into_iter().enumerate() {
            for tool in TOOLS {
                let what = format!("C {pair:?} {tool}");
                let want = library("c", tool, &m, pair.map(lib).to_vec(), Sampler::MonteCarlo);
                let params = pair.map(|k| sent(k, json!({}))).to_vec();
                let a = args(tool, &format!("{kind}-{i}-{tool}.jsonl"), &m, params);
                equal(&what, &root, &s, tool, a, &want);
                n += 1;
            }
        }
    }
    assert_eq!(n, 12);
    eprintln!("C: {n} calls without the key equal to the library");
}

/// T: which key is named.
#[test]
fn t_which_key_is_named() {
    let env = service("t", None);
    let m = naca();
    let mut n = 0;
    // two keys and four, in either order of the text, after and before the keys of the type
    let many: [(&str, &[&str]); 9] = [
        ("uniform", &["mean", "sd"]),
        ("uniform", &["mean", "a"]),
        ("uniform", &["mean", "b"]),
        ("uniform", &["sd", "a"]),
        ("uniform", &["sd", "b"]),
        ("uniform", &["a", "b"]),
        ("uniform", &["mean", "sd", "a", "b"]),
        ("normal", &["a", "b"]),
        ("beta", &["mean", "sd"]),
    ];
    for (kind, keys) in many {
        let mut reversed = keys.to_vec();
        reversed.reverse();
        for order in [keys.to_vec(), reversed] {
            let own = sent(kind, json!({}))["dist"].to_string();
            let own = &own[1..own.len() - 1];
            let more: Vec<String> = order.iter().map(|k| format!("\"{k}\": 0.5")).collect();
            let more = more.join(", ");
            for text in [format!("{{{own}, {more}}}"), format!("{{{more}, {own}}}")] {
                let mut v = sent(kind, json!({}));
                v["dist"] = serde_json::from_str(&text).unwrap();
                n += refused(&format!("T {text}"), &env, &m, vec![v], &message(0, kind, keys[0]));
            }
        }
    }
    // a foreign key in both inputs
    let (u, w) = (sent("uniform", json!({"b": 0.5})), sent("normal", json!({"a": 0.5})));
    n += refused(
        "T both",
        &env,
        &m,
        vec![u.clone(), w.clone()],
        &message(0, "uniform", "b"),
    );
    n += refused("T both", &env, &m, vec![w, u], &message(0, "normal", "a"));
    // beside a missing and a malformed number of the type
    for (kind, key, v) in [
        ("uniform", "mean", without(sent("uniform", json!({"mean": 0.5})), "lo")),
        ("uniform", "mean", sent("uniform", json!({"mean": 0.5, "lo": "x"}))),
        ("normal", "a", without(sent("normal", json!({"a": 0.5})), "sd")),
        ("beta", "mean", sent("beta", json!({"mean": 0.5, "a": null}))),
    ] {
        n += refused(
            &format!("T {}", v["dist"]),
            &env,
            &m,
            vec![v.clone()],
            &message(0, kind, key),
        );
    }
    assert_eq!(n, 4 * (9 * 4 + 2 + 4));
    eprintln!("T: {n} calls name the registered key");
}

/// N1: the refusal comes before the limit on the evaluations, which comes before the driver.
#[test]
fn n1_before_the_limit_and_so_before_any_sample() {
    let env = service("n1", Some(1));
    let m = naca();
    let mut n = 0;
    for (kind, key) in FOREIGN {
        let v = sent(kind, json!({ key: null }));
        n += refused(&format!("N1 {kind} {key}"), &env, &m, vec![v], &message(0, kind, key));
        for tool in TOOLS {
            let a = args(tool, "l.jsonl", &m, vec![sent(kind, json!({}))]);
            assert_eq!(by_server(&env.2, tool, &a).0, 1004, "N1 control: {kind} {tool}");
            assert_eq!(by_handler(&env.1, tool, &a).0, 1004, "N1 control: {kind} {tool}");
        }
        empty(&env.0, "N1 control");
    }
    eprintln!("N1: {n} calls refused before the limit; without the key 1004");
}

/// N2: a ledger that is there is not touched, a folder that is not there is not made.
#[test]
fn n2_no_file_is_written() {
    let (root, svc, s) = service("n2", None);
    let m = naca();
    let files = || {
        let mut f: Vec<_> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).collect();
        f.sort();
        // a folder that should not be there reads as no bytes
        f.into_iter()
            .map(|p| (p.clone(), std::fs::read(&p).unwrap_or_default()))
            .collect::<Vec<_>>()
    };
    for tool in TOOLS {
        let ledger = format!("{tool}.jsonl");
        ok(&call(
            &s,
            tool,
            args(tool, &ledger, &m, vec![sent("normal", json!({}))]),
        ));
    }
    let before = files();
    assert_eq!(before.len(), 2);
    let mut n = 0;
    for tool in TOOLS {
        for (kind, key) in FOREIGN {
            for ledger in [format!("{tool}.jsonl"), "sub/l.jsonl".to_string()] {
                let a = args(tool, &ledger, &m, vec![sent(kind, json!({ key: 0.5 }))]);
                let want = (-32602, message(0, kind, key));
                assert_eq!(by_server(&s, tool, &a), want, "N2 {tool} {ledger}");
                assert!(files() == before, "N2 {tool} {ledger}: a file was written");
                assert_eq!(by_handler(&svc, tool, &a), want, "N2 {tool} {ledger}");
                assert!(files() == before, "N2 {tool} {ledger}: a file was written");
                n += 2;
            }
        }
        // without the key: the study is there
        let a = args(tool, &format!("{tool}.jsonl"), &m, vec![sent("normal", json!({}))]);
        let (code, t) = by_server(&s, tool, &a);
        assert!(code == -32602 && t.starts_with("name:"), "N2 control: {code} {t}");
        assert!(files() == before, "N2 control");
    }
    eprintln!("N2: {n} calls left the two ledgers and the folder as they were");
}
