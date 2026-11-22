//! ADR 0774 (F-862), row 1: the description of `start_run`'s property `store` in the live project-mode tools list says
//! that plan is the default (ADR 0741) and that none is the way back, and no longer says "none with case". The property
//! is compared as a whole value, so its type and enum are held too. The rest of the list is held by
//! `project_list_f270.rs`, `tool_list_f275.rs` and `default_f1.rs`, and was compared before and after once (ADR 0774 D4).

mod common;

use common::*;
use serde_json::json;

/// The text approved by the maintainer (ADR 0741, question 1, option (a)).
const STORE: &str =
    "plan: the Studio's start plan stores the run's fields (about 100 snapshots, f32) unless the case or \
set gives the store; none: only what the case asks for. Default plan; none is the way back to a run that stores only \
what its case asks for";

#[test]
fn store_description_names_plan_as_the_default() {
    let dir = project("f862-store-description");
    let mut m = Mcp::start(&["--project", dir.to_str().unwrap()]);
    let _ = m.request(
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let r = m.request("tools/list", json!({}));
    let tools = r["result"]["tools"].as_array().expect("a tools list");
    let sr = tools
        .iter()
        .find(|t| t["name"] == "start_run")
        .expect("start_run listed with --project");
    let store = &sr["inputSchema"]["properties"]["store"];
    assert_eq!(
        *store,
        json!({"type": "string", "enum": ["plan", "none"], "description": STORE})
    );
    let d = store["description"].as_str().unwrap();
    assert!(!d.contains("none with case"), "{d}");
    assert!(d.contains("Default plan;"), "{d}");
    assert!(
        d.contains("none is the way back to a run that stores only what its case asks for"),
        "{d}"
    );
    drop(m);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
