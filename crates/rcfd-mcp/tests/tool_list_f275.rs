//! ADR 0195 (F-275, ADR 0173 C5): the project-mode tool list. C5-L the list through the binary, C5-H the hidden tools are
//! not callable, C5-S which session tools need a session, C5-I the `initialize` text. The F-273 list itself is checked by
//! `project_list_f270.rs` (C5-L restated there).
//!
//! ADR 0196 B: under `--project`, `design_run_study` and `design_evaluate` have the class `execute` (C5-L's registered edit E1,
//! and B1); without a project they keep `write_files`.

mod common;

use common::*;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::{Access, Policy};
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

/// ADR 0196 B: the session tools that have the class `execute` under `--project` (`write_files` without it);
/// `design_run_operating_map` since ADR 0200 S9 (registered edit E1).
const EXECUTE: [&str; 3] = ["design_run_study", "design_evaluate", "design_run_operating_map"];

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

/// C5-L: under `--project`, 40 tools: the list without `--project` less the 16 of P1, in its order and byte for byte (but the
/// class `execute` of `design_run_study` and `design_evaluate`, ADR 0196 B), then the 23 project tools.
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
    let mut execute = 0;
    for (l, k) in live.iter().zip(&kept) {
        let mut l = l.clone();
        // ADR 0196 B (registered edit E1): under --project these two have the class execute, every other byte as without it
        if EXECUTE.contains(&k["name"].as_str().unwrap()) {
            assert_eq!(l["_meta"]["rcfd/access"], "execute", "{}", k["name"]);
            assert_eq!(k["_meta"]["rcfd/access"], "write_files", "{}", k["name"]);
            l["_meta"]["rcfd/access"] = k["_meta"]["rcfd/access"].clone();
            execute += 1;
        }
        assert_eq!(
            serde_json::to_string(&l).unwrap(),
            serde_json::to_string(k).unwrap(),
            "{} differs from its entry without --project",
            k["name"]
        );
    }
    assert_eq!(execute, EXECUTE.len());
    let names: Vec<&str> = live[17..].iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, PROJECT_TOOLS);
    for t in live {
        assert!(!hidden.contains(&t["name"].as_str().unwrap()), "{} listed", t["name"]);
    }
    eprintln!(
        "C5-L: {} tools under --project: {} session tools byte-identical to the list without it ({execute} with the class execute, ADR 0196), then {} project tools; {} hidden",
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

/// C5-A: the Studio bridge's first allow-list is main's 11 read tools, then the four read tools added since (P5); a file
/// written from main's 11 reads back as those 11 rules and is written back byte for byte (no migration).
#[test]
fn c5a_allow_list_has_the_read_tools_and_old_files_stay() {
    use rcfd_claude::allow::{AllowList, AllowRule, PINNED_SERVER};
    const MAIN: [&str; 11] = [
        "project_info",
        "list_runs",
        "run_status",
        "read_metrics",
        "read_residuals",
        "field_summary",
        "render_field",
        "compare_runs",
        "list_changes",
        "read_project_file",
        "wait_run",
    ];
    // a new project's first list: main's 11 in their order, then the four (the set is checked against the live list by
    // project_w48f.rs F2)
    let first = AllowList::project_initial();
    let want: Vec<AllowRule> = MAIN
        .iter()
        .chain(&["read_feed", "check_case_mesh", "preview_mesh", "list_meshes"])
        .map(|t| AllowRule::Tool(format!("mcp__{PINNED_SERVER}__{t}")))
        .collect();
    assert_eq!(first.rules(), &want[..]);
    // an allow-list file of a project made before F-275
    let mut old = AllowList::new();
    for t in MAIN {
        old.add(AllowRule::Tool(format!("mcp__{PINNED_SERVER}__{t}"))).unwrap();
    }
    let file = AllowList::file_of(&scratch("c5a"));
    old.to_file(&file).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    let back = AllowList::from_file(&file).unwrap();
    assert_eq!(back.rules().len(), 11);
    assert_eq!(back, old);
    back.to_file(&file).unwrap();
    assert_eq!(
        std::fs::read(&file).unwrap(),
        bytes,
        "the old file was rewritten differently"
    );
    eprintln!(
        "C5-A: {} rules for a new project; an old file of {} rules read and written back byte for byte ({} bytes)",
        first.rules().len(),
        back.rules().len(),
        bytes.len()
    );
}

/// C5-F (the binary part): an edit made through one server process is replayed by a second one's `read_feed
/// {from_seq: 1}`, with its reason and the first process's actor.
#[test]
fn c5f_a_later_server_replays_an_earlier_servers_edit() {
    let dir = project("c5f");
    let d = dir.to_string_lossy().to_string();
    let mut a = Mcp::start(&["--project", &d]);
    let pid = a.pid();
    let h = ok(&a.call("read_project_file", json!({"path": "cases/cavity.toml"})))["hash"].clone();
    ok(&a.call(
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"physics.nu": 0.02}, "reason": "to reach Re 50"}),
    ));
    drop(a);
    let mut b = Mcp::start(&["--project", &d]);
    let live = ok(&b.call("read_feed", json!({"since": 0})));
    assert_eq!(live["entries"], json!([]), "a new feed starts at the present");
    let r = ok(&b.call("read_feed", json!({"from_seq": 1})));
    let e = &r["entries"][0];
    assert_eq!(r["total"], 1, "{r}");
    assert_eq!(
        (e["seq"].clone(), e["who"].clone(), e["reason"].clone()),
        (json!(1), json!("edit_case"), json!("to reach Re 50"))
    );
    assert_eq!(e["actor"]["client"], "mcp");
    assert_eq!(e["actor"]["instance"]["pid"], pid);
    assert_eq!(e["changes"], json!([{"path": "physics.nu", "old": 0.01, "new": 0.02}]));
    assert!(e.get("n").is_none());
    eprintln!("C5-F binary: {}", e["text"]);
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

/// ADR 0196 B1: under a project `design_run_study` and `design_evaluate` are `execute` in `tools/list` and in the call the policy
/// sees (a refusing hook: the call is denied with that class and changes nothing); without a project they stay `write_files`.
#[test]
fn b1_design_tools_are_execute_under_project_adr0196() {
    use std::sync::{Arc, Mutex};
    let asked: Arc<Mutex<Vec<(String, Access)>>> = Arc::new(Mutex::new(Vec::new()));
    let hook = |seen: Arc<Mutex<Vec<(String, Access)>>>| {
        Policy::with_hook(move |c| {
            seen.lock().unwrap().push((c.tool.to_string(), c.access));
            false
        })
    };
    let args = |name: &str| match name {
        "design_run_study" => json!({"dir": "designs/s", "max_evaluations": 1}),
        // ADR 0200 E1
        "design_run_operating_map" => {
            json!({"out_dir": "designs/m", "base": "cases/cavity.toml", "axes": [{"path": "physics.nu", "values": [0.02]}]})
        }
        _ => json!({"dir": "designs/s", "params": {"nu": 0.02}}),
    };
    let dir = project("b1-0196");
    let with_project = server(&dir, hook(asked.clone()));
    let root = scratch("b1-0196-plain");
    let plain = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(root.clone()),
            ..Default::default()
        }),
        hook(asked.clone()),
    );
    for (s, class) in [(&with_project, "execute"), (&plain, "write_files")] {
        let list = s.tool_list();
        for name in EXECUTE {
            let t = list["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == name)
                .unwrap_or_else(|| panic!("{name} not listed"));
            assert_eq!(t["_meta"]["rcfd/access"], class, "{name}");
            assert_eq!(t["_meta"]["rcfd/needsApproval"], true, "{name}");
            assert_eq!(t["annotations"]["readOnlyHint"], false, "{name}");
            assert_eq!(t["annotations"]["idempotentHint"], false, "{name}");
        }
    }
    let before = files(&dir);
    for name in EXECUTE {
        asked.lock().unwrap().clear();
        let r = call(&with_project, name, args(name));
        assert_eq!(err_code(&r), 1100, "{name}: {r}");
        assert_eq!(
            r["structuredContent"]["error"]["data"]["access"], "execute",
            "{name}: {r}"
        );
        assert_eq!(*asked.lock().unwrap(), [(name.to_string(), Access::Execute)], "{name}");
        asked.lock().unwrap().clear();
        let r = call(&plain, name, args(name));
        assert_eq!(err_code(&r), 1100, "{name}: {r}");
        assert_eq!(
            r["structuredContent"]["error"]["data"]["access"], "write_files",
            "{name}: {r}"
        );
        assert_eq!(
            *asked.lock().unwrap(),
            [(name.to_string(), Access::WriteFiles)],
            "{name}"
        );
    }
    assert!(files(&dir) == before, "a denied call changed the project");
    eprintln!("B1: {EXECUTE:?} execute under a project (listed, and asked of the hook), write_files without one; denied calls changed nothing");
}
