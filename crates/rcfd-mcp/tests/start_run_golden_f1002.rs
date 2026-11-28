//! ADR 0842 (F-1002), D2: `start_run`'s entry in the live project-mode `tools/list` of the `rcfd-mcp` binary is the
//! golden `golden/start_run_project_f1002.json` byte for byte, with the properties added since 7c960bb (`new_case`,
//! `store`, `continue_from`, `max_steps`, `from_run`, `run_time`, `dry_run` and `site`) compared one by one first. The
//! project golden's own `start_run` entry stays 7c960bb's and is held by `project_list_f270.rs`.

mod common;

use common::*;
use serde_json::{json, Value};

const GOLDEN: &str = include_str!("golden/start_run_project_f1002.json");

/// The properties `start_run` gained since 7c960bb (ADR 0842, finding 1).
const EIGHT: [&str; 8] = [
    "new_case",
    "store",
    "continue_from",
    "max_steps",
    "from_run",
    "run_time",
    "dry_run",
    "site",
];

#[test]
fn start_run_live_entry_is_its_golden() {
    let gold: Value = serde_json::from_str(GOLDEN).unwrap();
    assert_eq!(serde_json::to_string(&gold).unwrap(), GOLDEN);
    assert_eq!(gold["name"], "start_run");
    let gp = &gold["inputSchema"]["properties"];
    for p in EIGHT {
        assert!(
            gp[p]["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{p} has no description in the golden"
        );
    }
    let dir = project("f1002-start-run-golden");
    let mut m = Mcp::start(&["--project", dir.to_str().unwrap()]);
    let r = m.request("tools/list", json!({}));
    drop(m);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let tools = r["result"]["tools"].as_array().expect("a tools list");
    let sr = tools
        .iter()
        .find(|t| t["name"] == "start_run")
        .expect("start_run listed with --project");
    let lp = &sr["inputSchema"]["properties"];
    for p in EIGHT {
        assert_eq!(lp[p].to_string(), gp[p].to_string(), "{p} differs from the golden");
    }
    assert_eq!(
        serde_json::to_string(sr).unwrap(),
        GOLDEN,
        "start_run differs from the golden"
    );
}
