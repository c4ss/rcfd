//! ADR 0195 (F-275, ADR 0173 C5): the project-mode tool list. C5-L the list through the binary, C5-H the hidden tools are
//! not callable, C5-S which session tools need a session, C5-I the `initialize` text. The F-273 list itself is checked by
//! `project_list_f270.rs` (C5-L restated there).

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::McpServer;
use serde_json::{json, Value};
use std::path::Path;

/// The list without `--project` (ADR 0135 F1, never re-recorded).
const DEFAULT: &str = include_str!("golden/tools_list_default.json");

/// ADR 0195 P1: the session tools not offered with `--project`, each with arguments its schema accepts.
fn hidden() -> Vec<(&'static str, Value)> {
    vec![
        ("set_setting", json!({"path": "physics.nu", "value": 0.02})),
        ("get_setting", json!({})),
        ("load_mesh", json!({"path": "meshes/x.msh"})),
        ("check_mesh", json!({})),
        ("run", json!({"max_steps": 1, "wait_seconds": 0})),
        ("status", json!({})),
        ("get_residuals", json!({})),
        ("get_field_stats", json!({"name": "p"})),
        ("export_vtk", json!({})),
        ("list_workflow_tasks", json!({})),
        ("create_workflow", json!({"template": "empty"})),
        ("edit_workflow", json!({"action": "delete", "id": "x"})),
        ("run_workflow_task", json!({})),
        ("run_headless", json!({"max_steps": 1})),
        ("render_image", json!({})),
        // last: on a server without a project it makes a session
        ("create_case", json!({"template": "lid_driven_cavity"})),
    ]
}

/// ADR 0195 P1: the 23 project tools in the order of the F-273 list.
const PROJECT_TOOLS: [&str; 23] = [
    "project_info",
    "list_runs",
    "run_status",
    "read_metrics",
    "read_residuals",
    "field_summary",
    "render_field",
    "compare_runs",
    "list_changes",
    "read_feed",
    "read_project_file",
    "edit_case",
    "write_note",
    "new_case",
    "check_case_mesh",
    "preview_mesh",
    "import_mesh",
    "list_meshes",
    "start_run",
    "pause_run",
    "resume_run",
    "cancel_run",
    "wait_run",
];

/// Sorted relative paths and bytes of every file under `dir`.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    out.sort();
    out
}

/// C5-L: under `--project`, 40 tools: the list without `--project` less the 16 of P1, in its order and byte for byte,
/// then the 23 project tools.
#[test]
fn c5l_project_list_has_no_session_only_tool() {
    let dir = project("c5l");
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let list = m.request("tools/list", json!({}))["result"]["tools"].clone();
    let live = list.as_array().unwrap();
    let default: Value = serde_json::from_str(DEFAULT).unwrap();
    let session = default["tools"].as_array().unwrap();
    let hidden: Vec<&str> = hidden().iter().map(|h| h.0).collect();
    assert_eq!(hidden.len(), 16);
    let kept: Vec<&Value> = session
        .iter()
        .filter(|t| !hidden.contains(&t["name"].as_str().unwrap()))
        .collect();
    assert_eq!(kept.len(), 17);
    assert_eq!(live.len(), 40, "17 session tools and 23 project tools");
    for (l, k) in live.iter().zip(&kept) {
        assert_eq!(
            serde_json::to_string(l).unwrap(),
            serde_json::to_string(k).unwrap(),
            "{} differs from its entry without --project",
            k["name"]
        );
    }
    let names: Vec<&str> = live[17..].iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, PROJECT_TOOLS);
    for t in live {
        assert!(!hidden.contains(&t["name"].as_str().unwrap()), "{} listed", t["name"]);
    }
    eprintln!(
        "C5-L: {} tools under --project: {} session tools byte-identical to the list without it, then {} project tools; {} hidden",
        live.len(),
        kept.len(),
        names.len(),
        hidden.len()
    );
}

/// C5-H: each hidden tool is an unknown tool under `--project` and changes nothing; control: without a project each reaches
/// its handler.
#[test]
fn c5h_hidden_tools_are_unknown_under_project() {
    let dir = project("c5h");
    let d = dir.to_string_lossy().to_string();
    let before = files(&dir);
    let mut m = Mcp::start(&["--project", &d]);
    for (name, args) in hidden() {
        let r = m.request("tools/call", json!({"name": name, "arguments": args}));
        assert_eq!(r["error"]["code"], -32602, "{name}: {r}");
        let msg = r["error"]["message"].as_str().unwrap();
        assert!(msg.contains("unknown tool") && msg.contains(name), "{name}: {msg}");
    }
    drop(m);
    assert!(files(&dir) == before, "a refused call changed the project");
    // control: a server without a project, its sandbox a separate folder
    let other = scratch("c5h-control");
    let o = other.to_string_lossy().to_string();
    let mut c = Mcp::start(&["--root", &o]);
    let mut reached = 0;
    for (name, args) in hidden() {
        let r = c.request("tools/call", json!({"name": name, "arguments": args}));
        assert!(r.get("error").is_none(), "{name}: {r}");
        reached += 1;
    }
    eprintln!("C5-H: 16 hidden tools -32602 unknown under --project, project unchanged; control: {reached} reached their handlers");
}

/// C5-S: on a server without a project and without a session, each hidden tool but `create_case` answers 1001 and
/// `create_case` makes a session; each kept session tool, called without `session`, reaches its handler, never answers 1001,
/// and leaves no session.
#[test]
fn c5s_hidden_tools_need_a_session_kept_tools_do_not() {
    let root = scratch("c5s");
    std::fs::create_dir_all(root.join("cases")).unwrap();
    std::fs::write(root.join("cases/cavity.toml"), cavity(12, 30)).unwrap();
    std::fs::write(root.join("s.jsonl"), "{\"event\":\"start\"}\n").unwrap();
    let svc = || {
        Service::new(ServiceConfig {
            root: Some(root.clone()),
            ..Default::default()
        })
    };
    let s = McpServer::new(svc(), Policy::default());
    for (name, args) in hidden() {
        let r = call(&s, name, args);
        if name == "create_case" {
            assert!(ok(&r)["session"].is_string(), "{r}");
        } else {
            assert_eq!(err_code(&r), 1001, "{name}: {r}");
        }
    }
    let study = "designs/s";
    let ledger = "ledger.jsonl";
    let param = json!([{"name": "nu", "value": 0.01, "unit": "m2/s"}]);
    let metric = json!([{"name": "lid_drag", "value": 1.0, "unit": "N"}]);
    let kept: Vec<(&str, Value)> = vec![
        ("list_cases", json!({})),
        ("check_case_headless", json!({"path": "cases/cavity.toml"})),
        ("read_status_stream", json!({"path": "s.jsonl"})),
        (
            "design_create_study",
            json!({"dir": study, "base": "cases/cavity.toml", "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
                   "objectives": [{"metric": "lid_drag", "sense": "min"}], "method": {"type": "factorial", "levels": [2]}}),
        ),
        (
            "design_create_variant",
            json!({"path": "cases/v.toml", "base": "cases/cavity.toml", "overrides": {"physics.nu": 0.02}}),
        ),
        (
            "design_run_operating_map",
            json!({"out_dir": "maps/m", "base": "cases/cavity.toml", "points": [{"physics.nu": 0.01}], "max_steps": 2}),
        ),
        ("design_run_study", json!({"dir": study, "max_evaluations": 1})),
        ("design_evaluate", json!({"dir": study, "params": {"nu": 0.02}})),
        (
            "design_record",
            json!({"dir": study, "params": {"nu": 0.015}, "metrics": {"lid_drag": 1.0}}),
        ),
        ("design_get_metrics", json!({"dir": study, "index": 0})),
        ("design_compare_variants", json!({"dir": study})),
        ("design_propose", json!({"dir": study})),
        ("design_list_study", json!({"dir": study})),
        (
            "design_ledger_append",
            json!({"ledger": ledger, "params": param, "metrics": metric, "evaluator": "t", "evaluator_version": "1",
                   "verdict": "ok", "fidelity": "cfd"}),
        ),
        ("design_ledger_query", json!({"ledger": ledger})),
        ("design_ledger_export", json!({"ledger": ledger, "out": "ledger.csv"})),
        (
            "design_ledger_rerun_check",
            json!({"ledger": ledger, "index": 0, "metrics": metric}),
        ),
    ];
    assert_eq!(kept.len(), 17);
    let s2 = McpServer::new(svc(), Policy::default());
    let mut answered_ok = 0;
    for (name, args) in kept {
        let r = call(&s2, name, args);
        if r["isError"] == true {
            let code = r["structuredContent"]["error"]["code"].as_i64().unwrap();
            assert_ne!(code, 1001, "{name}: {r}");
            eprintln!("C5-S {name}: error {code} (not 1001)");
        } else {
            answered_ok += 1;
        }
    }
    let sessions = s2.service().call("session.list", &json!({})).unwrap();
    assert_eq!(sessions["sessions"], json!([]), "a kept tool made a session");
    eprintln!("C5-S: 15 hidden tools 1001, create_case a session; 17 kept tools without a session ({answered_ok} answered without error), no session made");
}

/// C5-I: under `--project` the instructions name no hidden tool; without it they are unchanged (`default_f1.rs`).
#[test]
fn c5i_initialize_under_project_names_no_hidden_tool() {
    let dir = project("c5i");
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let r = m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let text = r["result"]["instructions"].as_str().unwrap().to_string();
    assert!(
        text.starts_with("Clean-room open source CFD solver. This server also has a project folder"),
        "{text}"
    );
    assert!(!text.contains("create_case, adjust settings, check_mesh, run"));
    let words: Vec<&str> = text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).collect();
    let with_underscore: Vec<&str> = hidden().iter().map(|h| h.0).filter(|n| n.contains('_')).collect();
    assert_eq!(with_underscore.len(), 14);
    for n in &with_underscore {
        assert!(!words.contains(n), "{n} named in: {text}");
    }
    eprintln!(
        "C5-I: {} characters, none of the 14 hidden names with '_' a word of them",
        text.len()
    );
}
