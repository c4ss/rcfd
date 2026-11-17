//! ADR 0730 M3: the UI metadata and the help methods add no MCP tool (open question 8, answer (a): `rcfd help` covers
//! Claude Code). `tools/list`, without and with `--project`, names no help or glossary tool.
//!
//! Restated 2026-11-17 (ADR 0730, "Result", the fixture note): the byte-identity of the plain lists is held by the
//! pinned tests `default_f1.rs` (the golden of ADR 0135 F1) and `tool_list_f275.rs` (the project list), which other ADRs
//! restate when they add a tool; this test records no list of its own, so it needs nothing re-recorded on a merge.

mod common;

use common::*;
use serde_json::json;

fn names(args: &[&str]) -> Vec<String> {
    let mut m = Mcp::start(args);
    let _ = m.request(
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let r = m.request("tools/list", json!({}));
    r["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect()
}

fn check(args: &[&str], what: &str) {
    let n = names(args);
    assert!(!n.is_empty(), "{what}: the server listed its tools");
    let help: Vec<&String> = n
        .iter()
        .filter(|t| t.contains("help") || t.contains("glossary") || t.contains("catalogue"))
        .collect();
    assert!(help.is_empty(), "{what}: ADR 0730 adds no MCP tool, found {help:?}");
    eprintln!("M3 {what}: {} tools, none of them a help tool", n.len());
}

#[test]
fn m3_no_help_tool_without_project() {
    check(&[], "without --project");
}

#[test]
fn m3_no_help_tool_with_project() {
    let dir = project("adr0730-m3");
    let d = dir.to_string_lossy().to_string();
    check(&["--project", &d], "with --project");
}
