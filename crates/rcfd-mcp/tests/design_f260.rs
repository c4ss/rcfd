//! ADR 0222 T1 and T2 (F-260) through the MCP design tools, the server in process as in `design_f297.rs`: a `cma_es` study
//! driven by propose / record, and the refusals and the written method of the new type. Limits are those registered in ADR 0222
//! before the code.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::{McpServer, LID_DRIVEN_CAVITY};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn sandbox(tag: &str) -> (PathBuf, McpServer) {
    let root = std::env::temp_dir().join(format!("rcfd_mcp_f260_{tag}_{}", std::process::id()));
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

/// The d = 2 shifted Rastrigin study (box `[-5.12, 5.12]^2`) on dummy targets, objective `f` min.
fn r2_study(dir: &str, method: Value) -> Value {
    json!({
        "dir": dir, "base": "cavity.toml", "name": "r2",
        "parameters": [
            {"name": "x1", "lo": -5.12, "hi": 5.12, "targets": [{"path": "physics.nu", "scale": 0.001, "offset": 0.01}]},
            {"name": "x2", "lo": -5.12, "hi": 5.12, "targets": [{"path": "physics.rho", "scale": 0.1, "offset": 1.0}]}
        ],
        "objectives": [{"metric": "f", "sense": "min"}],
        "method": method,
    })
}

/// `R_2(x) = 20 + sum (t_i^2 - 10 cos 2 pi t_i)`, `t_i = x_i - 1.5 cos(i)`.
fn r2(x1: f64, x2: f64) -> f64 {
    20.0 + [(x1, 1.0f64), (x2, 2.0)]
        .iter()
        .map(|(x, i)| {
            let t = x - 1.5 * i.cos();
            t * t - 10.0 * (2.0 * std::f64::consts::PI * t).cos()
        })
        .sum::<f64>()
}

/// Propose once; record every candidate. Returns the candidates, or `None` when the study is complete.
fn round(s: &McpServer, dir: &str) -> Option<Value> {
    let p = ok(s, "design_propose", json!({"dir": dir}));
    if p["complete"] == true {
        return None;
    }
    for c in p["candidates"].as_array().unwrap() {
        let f = r2(c["x1"].as_f64().unwrap(), c["x2"].as_f64().unwrap());
        ok(
            s,
            "design_record",
            json!({"dir": dir, "params": c, "metrics": {"f": f}}),
        );
    }
    Some(p["candidates"].clone())
}

fn method_of(root: &std::path::Path, dir: &str) -> Value {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(root.join(dir).join("study.json")).unwrap()).unwrap();
    v["method"].clone()
}

#[test]
fn t1_cma_es_study_through_the_tools() {
    let (root, s) = sandbox("t1");
    let m = json!({"type": "cma_es", "budget": 300, "restarts": 2, "seed": 1});
    ok(&s, "design_create_study", r2_study("a", m.clone()));
    ok(&s, "design_create_study", r2_study("b", m));
    let ma = method_of(&root, "a");
    println!("T1: study.json method {ma}");
    assert_eq!(
        ma,
        json!({"budget": 300, "pop": null, "restarts": 2, "seed": 1, "sigma0": 0.3, "type": "cma_es"})
    );
    let a1 = round(&s, "a").unwrap();
    let b1 = round(&s, "b").unwrap();
    assert_eq!(a1, b1, "T1: the first batches differ");
    let mut proposed: BTreeSet<String> = a1.as_array().unwrap().iter().map(|c| c.to_string()).collect();
    let mut proposals = 1usize;
    while let Some(c) = round(&s, "a") {
        proposals += 1;
        assert!(proposals <= 400, "T1: the loop does not end");
        proposed.extend(c.as_array().unwrap().iter().map(|c| c.to_string()));
    }
    let list = ok(&s, "design_list_study", json!({"dir": "a"}));
    let text = std::fs::read_to_string(root.join("a").join("evaluations.jsonl")).unwrap();
    let recs: Vec<Value> = text.lines().skip(1).map(|l| serde_json::from_str(l).unwrap()).collect();
    let fs: Vec<f64> = recs
        .iter()
        .map(|r| r["conditions"][0]["metrics"]["f"].as_f64().unwrap())
        .collect();
    let mut bi = 0;
    for (i, f) in fs.iter().enumerate() {
        if *f < fs[bi] {
            bi = i;
        }
    }
    println!(
        "T1: {proposals} proposals, {} distinct candidates, {} records; best record {} with f {:.3e} (recorded); list best {}",
        proposed.len(),
        recs.len(),
        recs[bi]["index"],
        fs[bi],
        list["best"]["index"]
    );
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(list["evaluations"], recs.len());
    assert_eq!(proposed.len(), recs.len(), "T1: records against distinct candidates");
    assert_eq!(list["best"]["index"], recs[bi]["index"], "T1: the best record");
}

#[test]
fn t2_refusals_and_the_written_method() {
    let (root, s) = sandbox("t2");
    let cases: Vec<(&str, &str, Value)> = vec![
        ("no budget", "method.budget", json!({"type": "cma_es"})),
        (
            "pop 1",
            "method.pop",
            json!({"type": "cma_es", "budget": 300, "pop": 1}),
        ),
        (
            "sigma0 0",
            "method.sigma0",
            json!({"type": "cma_es", "budget": 300, "sigma0": 0}),
        ),
        (
            "sigma0 a",
            "method.sigma0",
            json!({"type": "cma_es", "budget": 300, "sigma0": "a"}),
        ),
        ("budget 5", "method.budget", json!({"type": "cma_es", "budget": 5})),
        (
            "sigma0 on nelder_mead",
            "method.sigma0",
            json!({"type": "nelder_mead", "budget": 100, "sigma0": 0.2}),
        ),
        ("type cmaes", "cma_es", json!({"type": "cmaes", "budget": 300})),
    ];
    for (i, (tag, key, m)) in cases.into_iter().enumerate() {
        let (code, msg) = refusal(&s, "design_create_study", r2_study(&format!("r{i}"), m));
        println!("T2 {tag}: {code} {msg}");
        assert_eq!(code, -32602, "{tag}");
        assert!(msg.contains(key), "{tag}: '{msg}' does not name {key}");
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
    ok(
        &s,
        "design_create_study",
        r2_study("x", json!({"type": "cma_es", "budget": 300, "pop": 10, "sigma0": 0.2})),
    );
    let mx = method_of(&root, "x");
    println!("T2: written {mx}");
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(
        mx,
        json!({"budget": 300, "pop": 10, "restarts": 0, "seed": 1, "sigma0": 0.2, "type": "cma_es"})
    );
}
