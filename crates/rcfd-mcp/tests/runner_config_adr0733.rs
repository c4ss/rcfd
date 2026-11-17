//! ADR 0733 P3 (F-837): the project tools `runner_config` and `set_runner_config`. R3 a stale base is 1200 with
//! `data.current_hash`; R5 one journal line with the actor and the reason; R6 `--read-only` refuses the write with 1100
//! and changes nothing (the tool lists are checked by `project_list_f270.rs`, `tool_list_f275.rs` and `project_w48f.rs`,
//! restated there); refusals are 1003 at the key; a bad argument shape is -32602.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use serde_json::{json, Value};
use std::path::Path;

fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn set_runner_config_writes_through_the_runner_and_refuses_as_it_does() {
    let dir = project("adr0733");
    let file = dir.join(".rcfd/runner-config.json");
    let s = server(&dir, Policy::default());
    let st = ok(&call(&s, "runner_config", json!({})));
    assert_eq!(st["exists"], false);
    assert_eq!(st["applies"], "next_start");
    assert!(st["defaults"]["max_runs"].is_u64());
    // created: base null
    let w = ok(&call(
        &s,
        "set_runner_config",
        json!({"set": {"max_runs": 3, "mem_budget_mb": 2048}, "base_hash": null, "reason": "two big runs at once"}),
    ));
    assert_eq!(w["values"], json!({"max_runs": 3, "mem_budget_mb": 2048}));
    let h1 = w["hash"].as_str().unwrap().to_string();
    assert!(h1.starts_with("fnv64:"));
    assert_eq!(rcfd_runner::runner::Config::load(&dir).unwrap().max_runs, 3);
    // R5: one journal line with the actor, the reason and the changes
    let j = journal(&dir);
    let lines = j.iter().filter(|l| l["path"] == ".rcfd/runner-config.json").count();
    assert_eq!(lines, 1);
    let l = j.last().unwrap();
    assert_eq!(l["kind"], "edit");
    assert_eq!(l["who"], "set_runner_config");
    assert_eq!(l["reason"], "two big runs at once");
    assert_eq!(l["actor"]["client"], "mcp");
    assert_eq!(l["new_hash"], json!(h1));
    assert_eq!(l["changes"][0], json!({"path": "max_runs", "old": null, "new": 3}));
    // the client's undo: the previous values back, the new hash as base
    let u = ok(&call(
        &s,
        "set_runner_config",
        json!({"set": {"max_runs": null, "mem_budget_mb": null}, "base_hash": h1}),
    ));
    assert_eq!(u["values"], json!({}));
    let h2 = u["hash"].as_str().unwrap().to_string();
    // R3: a stale base: 1200 with the current hash, nothing written
    let before = std::fs::read(&file).unwrap();
    let r = call(
        &s,
        "set_runner_config",
        json!({"set": {"max_runs": 4}, "base_hash": h1}),
    );
    assert_eq!(err_code(&r), 1200, "{r}");
    assert_eq!(r["structuredContent"]["error"]["data"]["current_hash"], json!(h2));
    let r = call(
        &s,
        "set_runner_config",
        json!({"set": {"max_runs": 4}, "base_hash": null}),
    );
    assert_eq!(err_code(&r), 1200, "{r}");
    // refused: 1003 at the key
    for (set, key) in [
        (json!({"max_runs": 0}), "max_runs"),
        (json!({"cpu_threads": 1.5}), "cpu_threads"),
        (json!({"gpu": 1}), "gpu"),
    ] {
        let r = call(&s, "set_runner_config", json!({"set": set, "base_hash": h2}));
        assert_eq!(err_code(&r), 1003, "{r}");
        assert_eq!(r["structuredContent"]["error"]["data"]["key"], key, "{r}");
    }
    // bad shapes: -32602
    for args in [
        json!({"set": {"max_runs": "2"}, "base_hash": h2}),
        json!({"set": {}, "base_hash": h2}),
        json!({"set": {"max_runs": 2}}),
        json!({"set": {"max_runs": 2}, "base_hash": 5}),
    ] {
        let r = rpc_call(&s, "set_runner_config", args.clone());
        let code = r["error"]["code"]
            .as_i64()
            .or_else(|| r["result"]["structuredContent"]["error"]["code"].as_i64());
        assert_eq!(code, Some(-32602), "{args}: {r}");
    }
    assert_eq!(std::fs::read(&file).unwrap(), before, "a refused call changed the file");
    // R6: read-only refuses the write with 1100 and still reads
    let ro = server(&dir, Policy::read_only());
    let r = call(
        &ro,
        "set_runner_config",
        json!({"set": {"max_runs": 2}, "base_hash": h2}),
    );
    assert_eq!(err_code(&r), 1100, "{r}");
    assert_eq!(ok(&call(&ro, "runner_config", json!({})))["hash"], json!(h2));
    assert_eq!(std::fs::read(&file).unwrap(), before);
    eprintln!("ADR 0733 P3: write, undo by values, 1200, 1003 x3, -32602 x4, 1100 read-only");
}
