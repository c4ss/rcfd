//! ADR 0220 T1 and T2 (F-297) through the MCP design tools, the server in process as in `design_f249.rs`: an `nsga2` study with
//! `"crossover": "exchange"` driven by propose / record, and the refusals and the written method of the new key. Limits are those
//! registered in ADR 0220 before the code.

use rcfd_api::design::pareto;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::{McpServer, LID_DRIVEN_CAVITY};
use serde_json::{json, Value};
use std::path::PathBuf;

fn sandbox(tag: &str) -> (PathBuf, McpServer) {
    let root = std::env::temp_dir().join(format!("rcfd_mcp_f297_{tag}_{}", std::process::id()));
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

/// The ADR 0209 MO4 study (ZDT1 with d = 2 on dummy targets) with the given method.
fn zdt1_study(dir: &str, method: Value) -> Value {
    json!({
        "dir": dir, "base": "cavity.toml", "name": "zdt1",
        "parameters": [
            {"name": "x1", "lo": 0.0, "hi": 1.0, "targets": [{"path": "physics.nu", "scale": 0.01, "offset": 0.01}]},
            {"name": "x2", "lo": 0.0, "hi": 1.0, "targets": [{"path": "physics.rho", "scale": 0.1, "offset": 1.0}]}
        ],
        "objectives": [{"metric": "f1", "sense": "min"}, {"metric": "f2", "sense": "min"}],
        "method": method,
    })
}

fn nsga2(extra: Value) -> Value {
    let mut m = json!({"type": "nsga2", "budget": 400, "pop": 20, "seed": 1});
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    m
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

/// Propose once; record every candidate. Returns the candidates, or `None` when the study is complete.
fn round(s: &McpServer, dir: &str) -> Option<Value> {
    let p = ok(s, "design_propose", json!({"dir": dir}));
    if p["complete"] == true {
        return None;
    }
    for c in p["candidates"].as_array().unwrap() {
        let (f1, f2) = zdt1_2(c["x1"].as_f64().unwrap(), c["x2"].as_f64().unwrap());
        ok(
            s,
            "design_record",
            json!({"dir": dir, "params": c, "metrics": {"f1": f1, "f2": f2}}),
        );
    }
    Some(p["candidates"].clone())
}

/// The sorted keys and the object of the `method` written to `study.json`.
fn method_of(root: &std::path::Path, dir: &str) -> (Vec<String>, Value) {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(root.join(dir).join("study.json")).unwrap()).unwrap();
    let mut k: Vec<String> = v["method"].as_object().unwrap().keys().cloned().collect();
    k.sort();
    (k, v["method"].clone())
}

#[test]
fn t1_exchange_study_through_the_tools() {
    let (root, s) = sandbox("t1");
    ok(
        &s,
        "design_create_study",
        zdt1_study("a", nsga2(json!({"crossover": "exchange"}))),
    );
    ok(&s, "design_create_study", zdt1_study("b", nsga2(json!({}))));
    let (ka, ma) = method_of(&root, "a");
    println!("T1: study.json method {ma}");
    assert_eq!(ka, ["budget", "crossover", "pop", "seed", "type"]);
    assert_eq!(ma["crossover"], "exchange");
    // the first batch does not depend on the crossover; the second does
    let a1 = round(&s, "a").unwrap();
    let b1 = round(&s, "b").unwrap();
    assert_eq!(a1, b1, "T1: the first batches differ");
    let a2 = round(&s, "a").unwrap();
    let b2 = round(&s, "b").unwrap();
    assert_ne!(a2, b2, "T1: the second batches are equal");
    let mut proposals = 2usize;
    while round(&s, "a").is_some() {
        proposals += 1;
        assert!(proposals <= 40, "T1: the loop does not end");
    }
    let list = ok(&s, "design_list_study", json!({"dir": "a"}));
    let text = std::fs::read_to_string(root.join("a").join("evaluations.jsonl")).unwrap();
    let recs: Vec<Value> = text.lines().skip(1).map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(list["evaluations"], recs.len());
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
        "T1: {proposals} proposals, {} records, {} Pareto members, max D {max_d:.3e} (recorded)",
        recs.len(),
        got.len()
    );
    let _ = std::fs::remove_dir_all(&root);
    assert!(!got.is_empty());
    assert_eq!(got, want);
}

#[test]
fn t2_refusals_and_the_written_method() {
    let (root, s) = sandbox("t2");
    let cases: Vec<(&str, Value)> = vec![
        ("value", zdt1_study("r1", nsga2(json!({"crossover": "uniform"})))),
        ("number", zdt1_study("r2", nsga2(json!({"crossover": 1})))),
        (
            "on nelder_mead",
            zdt1_study(
                "r3",
                json!({"type": "nelder_mead", "budget": 100, "restarts": 1, "seed": 1, "crossover": "exchange"}),
            ),
        ),
        (
            "on lhs",
            zdt1_study("r4", json!({"type": "lhs", "n": 4, "crossover": "sbx"})),
        ),
    ];
    for (tag, args) in cases {
        let (code, msg) = refusal(&s, "design_create_study", args);
        println!("T2 {tag}: {code} {msg}");
        assert_eq!(code, -32602, "{tag}");
        assert!(
            msg.contains("method.crossover"),
            "{tag}: '{msg}' does not name method.crossover"
        );
    }
    let entries: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entries,
        vec!["cavity.toml".to_string()],
        "T2: a refused call wrote something"
    );
    // "sbx" given explicitly: written, and the proposals of the default
    ok(
        &s,
        "design_create_study",
        zdt1_study("x", nsga2(json!({"crossover": "sbx"}))),
    );
    ok(&s, "design_create_study", zdt1_study("d", nsga2(json!({}))));
    let (kx, mx) = method_of(&root, "x");
    let (kd, md) = method_of(&root, "d");
    println!("T2: explicit sbx {mx}, default {md}");
    assert_eq!(kx, ["budget", "crossover", "pop", "seed", "type"]);
    assert_eq!(mx["crossover"], "sbx");
    assert_eq!(kd, ["budget", "pop", "seed", "type"]);
    let x1 = round(&s, "x").unwrap();
    let d1 = round(&s, "d").unwrap();
    let x2 = round(&s, "x").unwrap();
    let d2 = round(&s, "d").unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(x1, d1, "T2: the first batches differ");
    assert_eq!(x2, d2, "T2: the second batches differ");
}
