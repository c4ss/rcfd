//! ADR 0209 N1, N1d, N6 and MO4 (F-249) through the MCP design tools, the server in process as in `src/design_tests.rs`: the
//! ADR 0067 T3 loop with the two Nelder-Mead options, the same loop without them, the refusals of the new method keys, and an
//! NSGA-II study driven by propose / record. Limits are those registered in ADR 0209 before the code.

use rcfd_api::design::pareto;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::{McpServer, LID_DRIVEN_CAVITY};
use serde_json::{json, Value};
use std::path::PathBuf;

/// ADR 0209 V0b: bits of the default T3 best f after 120 records (recorded on main 05f0d73).
const T3_AT_120: u64 = 0x3f69_9ab6_bfa9_6f0b;

fn sandbox(tag: &str) -> (PathBuf, McpServer) {
    let root = std::env::temp_dir().join(format!("rcfd_mcp_f249_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("cavity.toml"),
        LID_DRIVEN_CAVITY
            .replace("nx = 16", "nx = 6")
            .replace("ny = 16", "ny = 6"),
    )
    .unwrap();
    let s = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(root.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );
    (root, s)
}

fn call(s: &McpServer, tool: &str, args: Value) -> Value {
    let r = s
        .handle(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}}),
        )
        .unwrap();
    assert!(r.get("error").is_none(), "{tool}: {r}");
    r["result"].clone()
}

fn ok(s: &McpServer, tool: &str, args: Value) -> Value {
    let r = call(s, tool, args);
    assert_eq!(r["isError"], false, "{tool}: {r}");
    r["structuredContent"].clone()
}

/// The error code and message of a refused call.
fn refusal(s: &McpServer, tool: &str, args: Value) -> (i64, String) {
    let r = call(s, tool, args);
    assert_eq!(r["isError"], true, "{tool}: {r}");
    let e = &r["structuredContent"]["error"];
    (e["code"].as_i64().unwrap(), e["message"].as_str().unwrap().to_string())
}

fn rosen_study(dir: &str, method: Value) -> Value {
    json!({
        "dir": dir, "base": "cavity.toml", "name": "rosen",
        "parameters": [
            {"name": "x", "lo": -2.0, "hi": 2.0, "init": -1.2, "targets": [{"path": "physics.nu", "scale": 0.1, "offset": 0.5}]},
            {"name": "y", "lo": -2.0, "hi": 2.0, "init": 1.0, "targets": [{"path": "physics.rho", "scale": 0.1, "offset": 1.0}]}
        ],
        "objectives": [{"metric": "f", "sense": "min"}],
        "method": method,
    })
}

fn rosenbrock(x: f64, y: f64) -> f64 {
    100.0 * (y - x * x).powi(2) + (1.0 - x).powi(2)
}

/// The T3 propose / record loop (as `t3_best_after_120_known_miss`): best f after 120 records, at the end, records.
fn t3_loop(s: &McpServer, dir: &str) -> (f64, f64, usize) {
    let mut recorded = 0usize;
    let mut at_120 = f64::NAN;
    while recorded < 400 {
        let p = ok(s, "design_propose", json!({"dir": dir}));
        if p["complete"] == true {
            break;
        }
        for c in p["candidates"].as_array().unwrap() {
            let (x, y) = (c["x"].as_f64().unwrap(), c["y"].as_f64().unwrap());
            ok(
                s,
                "design_record",
                json!({"dir": dir, "params": c, "metrics": {"f": rosenbrock(x, y)}}),
            );
            recorded += 1;
            if recorded == 120 {
                at_120 = ok(s, "design_list_study", json!({"dir": dir}))["best"]["f"]
                    .as_f64()
                    .unwrap();
            }
        }
    }
    let end = ok(s, "design_list_study", json!({"dir": dir}))["best"]["f"]
        .as_f64()
        .unwrap();
    (at_120, end, recorded)
}

#[test]
fn n1_t3_with_the_start_simplex_and_adaptive_coefficients() {
    let (root, s) = sandbox("n1");
    let method = json!({"type": "nelder_mead", "budget": 400, "restarts": 1, "seed": 1,
                        "initial_simplex": "start", "coefficients": "adaptive"});
    ok(&s, "design_create_study", rosen_study("a", method));
    let (at_120, end, n) = t3_loop(&s, "a");
    println!("N1: best f after 120 recorded candidates {at_120:e}; at {n} {end:e}");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        at_120 <= 1e-3,
        "N1: best f after 120 candidates {at_120:e} (limit 1e-3)"
    );
}

#[test]
fn n1d_t3_without_the_keys_is_the_default() {
    let (root, s) = sandbox("n1d");
    let method = json!({"type": "nelder_mead", "budget": 400, "restarts": 1, "seed": 1});
    ok(&s, "design_create_study", rosen_study("a", method));
    let (at_120, end, n) = t3_loop(&s, "a");
    println!(
        "N1d: best f after 120 {at_120:e} ({:#018x}); at {n} {end:e}",
        at_120.to_bits()
    );
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(at_120.to_bits(), T3_AT_120);
}

#[test]
fn n6_refusals_and_the_written_method() {
    let (root, s) = sandbox("n6");
    let nm = |extra: Value| {
        let mut m = json!({"type": "nelder_mead", "budget": 100, "restarts": 1, "seed": 1});
        for (k, v) in extra.as_object().unwrap() {
            m[k] = v.clone();
        }
        m
    };
    let one_param = |method: Value| {
        let mut a = rosen_study("one", method);
        a["parameters"] = json!([{"name": "x", "lo": -2.0, "hi": 2.0, "init": -1.2, "path": "physics.nu"}]);
        a
    };
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "simplex value",
            rosen_study("r1", nm(json!({"initial_simplex": "simplex"}))),
            "method.initial_simplex",
        ),
        (
            "coefficients number",
            rosen_study("r2", nm(json!({"coefficients": 3}))),
            "method.coefficients",
        ),
        (
            "simplex on lhs",
            rosen_study("r3", json!({"type": "lhs", "n": 4, "initial_simplex": "start"})),
            "method.initial_simplex",
        ),
        (
            "adaptive with one parameter",
            one_param(nm(json!({"coefficients": "adaptive"}))),
            "method.coefficients",
        ),
        (
            "nsga2 pop 7",
            rosen_study("r5", json!({"type": "nsga2", "budget": 100, "pop": 7})),
            "method.pop",
        ),
        (
            "nsga2 budget 10",
            rosen_study("r6", json!({"type": "nsga2", "budget": 10, "pop": 20})),
            "method.budget",
        ),
        (
            "nsga2 without budget",
            rosen_study("r7", json!({"type": "nsga2", "pop": 20})),
            "method.budget",
        ),
        (
            "nsga2 without pop",
            rosen_study("r8", json!({"type": "nsga2", "budget": 100})),
            "method.pop",
        ),
    ];
    for (tag, args, needle) in cases {
        let (code, msg) = refusal(&s, "design_create_study", args);
        println!("N6 {tag}: {code} {msg}");
        assert_eq!(code, -32602, "{tag}");
        assert!(msg.contains(needle), "{tag}: '{msg}' does not name {needle}");
    }
    let entries: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entries,
        vec!["cavity.toml".to_string()],
        "N6: a refused call wrote something"
    );
    // the method object written to study.json
    let keys = |dir: &str| {
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(dir).join("study.json")).unwrap()).unwrap();
        let mut k: Vec<String> = v["method"].as_object().unwrap().keys().cloned().collect();
        k.sort();
        (k, v["method"].clone())
    };
    ok(&s, "design_create_study", rosen_study("d", nm(json!({}))));
    ok(
        &s,
        "design_create_study",
        rosen_study("g", json!({"type": "nsga2", "budget": 100, "pop": 20})),
    );
    let (kd, md) = keys("d");
    let (kg, mg) = keys("g");
    println!("N6: default nelder_mead {md}, nsga2 {mg}");
    assert_eq!(kd, ["budget", "restarts", "seed", "type"]);
    assert_eq!(kg, ["budget", "pop", "seed", "type"]);
    let _ = std::fs::remove_dir_all(&root);
}

fn zdt1_2(x1: f64, x2: f64) -> (f64, f64) {
    let g = 1.0 + 9.0 * x2;
    (x1, g * (1.0 - (x1 / g).sqrt()))
}

/// Distance of `(f1, f2)` to the ZDT1 front `f2 = 1 - sqrt(f1)` (dense sampling; recorded only).
fn zdt1_distance(f1: f64, f2: f64) -> f64 {
    (0..=20_000)
        .map(|k| {
            let s = k as f64 / 20_000.0;
            ((f1 - s).powi(2) + (f2 - (1.0 - s.sqrt())).powi(2)).sqrt()
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn mo4_nsga2_study_through_the_tools() {
    let (root, s) = sandbox("mo4");
    let study = |dir: &str| {
        json!({
            "dir": dir, "base": "cavity.toml", "name": "zdt1",
            "parameters": [
                {"name": "x1", "lo": 0.0, "hi": 1.0, "targets": [{"path": "physics.nu", "scale": 0.01, "offset": 0.01}]},
                {"name": "x2", "lo": 0.0, "hi": 1.0, "targets": [{"path": "physics.rho", "scale": 0.1, "offset": 1.0}]}
            ],
            "objectives": [{"metric": "f1", "sense": "min"}, {"metric": "f2", "sense": "min"}],
            "method": {"type": "nsga2", "budget": 400, "pop": 20, "seed": 1},
        })
    };
    ok(&s, "design_create_study", study("a"));
    ok(&s, "design_create_study", study("b"));
    let pa = ok(&s, "design_propose", json!({"dir": "a"}));
    let pb = ok(&s, "design_propose", json!({"dir": "b"}));
    assert_eq!(pa["candidates"], pb["candidates"]);
    let mut proposals = 0usize;
    let mut recorded = 0usize;
    loop {
        let p = ok(&s, "design_propose", json!({"dir": "a"}));
        if p["complete"] == true {
            break;
        }
        proposals += 1;
        assert!(proposals <= 40, "MO4: the loop does not end");
        for c in p["candidates"].as_array().unwrap() {
            let (f1, f2) = zdt1_2(c["x1"].as_f64().unwrap(), c["x2"].as_f64().unwrap());
            ok(
                &s,
                "design_record",
                json!({"dir": "a", "params": c, "metrics": {"f1": f1, "f2": f2}}),
            );
            recorded += 1;
        }
    }
    let list = ok(&s, "design_list_study", json!({"dir": "a"}));
    assert_eq!(list["evaluations"], recorded);
    // the Pareto set recomputed from the store file
    let text = std::fs::read_to_string(root.join("a").join("evaluations.jsonl")).unwrap();
    let recs: Vec<Value> = text.lines().skip(1).map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(recs.len(), recorded);
    let objs: Vec<Vec<f64>> = recs
        .iter()
        .map(|r| {
            let m = &r["conditions"][0]["metrics"];
            vec![m["f1"].as_f64().unwrap(), m["f2"].as_f64().unwrap()]
        })
        .collect();
    let want: Vec<u64> = pareto::front(&objs, &vec![0.0; objs.len()])
        .iter()
        .map(|&i| recs[i]["index"].as_u64().unwrap())
        .collect();
    let got: Vec<u64> = list["pareto"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let max_d = got
        .iter()
        .map(|&i| {
            let o = &objs[recs.iter().position(|r| r["index"] == i).unwrap()];
            zdt1_distance(o[0], o[1])
        })
        .fold(0.0, f64::max);
    println!(
        "MO4: {proposals} proposals, {recorded} records, {} Pareto members, max D {max_d:.3e} (recorded)",
        got.len()
    );
    assert!(!got.is_empty());
    assert_eq!(got, want);
    let _ = std::fs::remove_dir_all(&root);
}
