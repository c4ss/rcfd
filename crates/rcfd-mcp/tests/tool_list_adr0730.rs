//! ADR 0730 M3: the UI metadata and the help methods add no MCP tool (open question 8, answer (a): `rcfd help` covers
//! Claude Code). The `tools/list` result, without and with `--project`, has the bytes recorded from the base commit
//! f86339e8 before any ADR 0730 code (`crates/rcfd-api/tests/fixtures/adr0730_base_replies.json`).

mod common;

use common::*;
use serde_json::{json, Value};

/// FNV-1a, 64 bit: `rcfd_core::hash::fnv64`, which recorded the fixture (this crate does not depend on rcfd-core).
fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

const BASE: &str = include_str!("../../rcfd-api/tests/fixtures/adr0730_base_replies.json");

fn recorded(key: &str) -> (usize, String) {
    let v: Value = serde_json::from_str(BASE).unwrap();
    (
        v[key]["bytes"].as_u64().unwrap() as usize,
        v[key]["fnv64"].as_str().unwrap().to_string(),
    )
}

fn check(args: &[&str], key: &str) {
    let mut m = Mcp::start(args);
    let _ = m.request(
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let r = m.request("tools/list", json!({}));
    let text = serde_json::to_string(&r["result"]).unwrap();
    let (bytes, hash) = recorded(key);
    assert_eq!(
        (text.len(), format!("{:#018x}", fnv64(text.as_bytes()))),
        (bytes, hash),
        "tools/list ({key}) differs from the base commit"
    );
    let names: Vec<&str> = r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        !names.iter().any(|n| n.contains("help") || n.contains("glossary")),
        "{names:?}"
    );
    eprintln!("M3 {key}: {} tools, {} bytes, as at f86339e8", names.len(), text.len());
}

#[test]
fn m3_tool_list_without_project_is_the_base_commit_s() {
    check(&[], "mcp_tools_list_result");
}

#[test]
fn m3_tool_list_with_project_is_the_base_commit_s() {
    let dir = project("adr0730-m3");
    let d = dir.to_string_lossy().to_string();
    check(&["--project", &d], "mcp_tools_list_project_result");
}
