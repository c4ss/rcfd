use super::*;
use crate::policy::{Approval, PERMISSION_DENIED};
use rcfd_api::service::ServiceConfig;
use std::sync::atomic::{AtomicUsize, Ordering};

fn server(policy: Policy) -> McpServer {
    McpServer::new(Service::new(ServiceConfig::default()), policy)
}

fn rpc(s: &McpServer, id: u64, method: &str, params: Value) -> Value {
    s.handle(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
        .unwrap()
}

fn call(s: &McpServer, tool: &str, args: Value) -> Value {
    let r = rpc(s, 1, "tools/call", json!({"name": tool, "arguments": args}));
    assert!(r.get("error").is_none(), "{r}");
    r["result"].clone()
}

#[test]
fn initialize_echoes_the_client_protocol_version() {
    let s = server(Policy::default());
    let r = rpc(
        &s,
        1,
        "initialize",
        json!({"protocolVersion": "2024-11-05", "capabilities": {}}),
    );
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
    assert!(r["result"]["capabilities"]["tools"].is_object());
    assert!(s
        .handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .is_none());
    assert_eq!(rpc(&s, 2, "ping", json!({}))["result"], json!({}));
    assert_eq!(rpc(&s, 3, "nope", json!({}))["error"]["code"], -32601);
}

#[test]
fn every_tool_has_a_schema_and_access_annotations() {
    let s = server(Policy::default());
    let list = rpc(&s, 1, "tools/list", json!({}))["result"]["tools"].clone();
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in [
        "list_cases",
        "create_case",
        "set_setting",
        "get_setting",
        "load_mesh",
        "check_mesh",
        "run",
        "status",
        "get_residuals",
        "get_field_stats",
        "export_vtk",
        "render_image",
    ] {
        assert!(names.contains(&n), "missing tool {n}");
    }
    for t in list.as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
        assert!(t["description"].as_str().unwrap().len() > 20);
        assert!(t["annotations"]["readOnlyHint"].is_boolean());
        let ro = t["annotations"]["readOnlyHint"] == true;
        assert_eq!(t["_meta"]["rcfd/needsApproval"], !ro);
    }
    let by = |n: &str| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(by("check_mesh")["annotations"]["readOnlyHint"], true);
    assert_eq!(by("run")["annotations"]["readOnlyHint"], false);
    assert_eq!(by("export_vtk")["_meta"]["rcfd/access"], "write_files");
}

#[test]
fn drives_a_cavity_case_end_to_end() {
    let s = server(Policy::default());
    let cases = call(&s, "list_cases", json!({"dir": "../../cases"}));
    assert_eq!(cases["isError"], false);
    assert!(cases["structuredContent"]["templates"]
        .as_array()
        .unwrap()
        .contains(&json!("lid_driven_cavity")));
    assert!(cases["structuredContent"]["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "cavity"));

    let created = call(&s, "create_case", json!({"template": "lid_driven_cavity"}));
    assert_eq!(created["isError"], false);
    let set = call(&s, "set_setting", json!({"path": "mesh.nx", "value": 12}));
    assert_eq!(set["isError"], false);
    call(&s, "set_setting", json!({"path": "mesh.ny", "value": 12}));
    let got = call(&s, "get_setting", json!({"path": "mesh.nx"}));
    assert_eq!(got["structuredContent"]["value"], 12);

    let mesh = call(&s, "check_mesh", json!({}));
    assert_eq!(mesh["structuredContent"]["ok"], true);
    assert_eq!(mesh["structuredContent"]["report"]["n_cells"], 144);

    let run = call(&s, "run", json!({"wait_seconds": 60}));
    assert_eq!(run["isError"], false, "{run}");
    assert_eq!(run["structuredContent"]["state"], "finished");
    assert_eq!(run["structuredContent"]["converged"], true);

    let st = call(&s, "status", json!({}));
    assert!(st["structuredContent"]["simulation"]["iteration"].as_u64().unwrap() > 10);
    assert!(st["structuredContent"]["reports"][0]["value"].as_f64().unwrap() < 0.0);

    let res = call(&s, "get_residuals", json!({"names": ["res_u", "res_p"]}));
    let y = res["structuredContent"]["series"]["res_u"]["y"].as_array().unwrap();
    assert!(y.first().unwrap().as_f64().unwrap() > 100.0 * y.last().unwrap().as_f64().unwrap());

    let stats = call(&s, "get_field_stats", json!({"name": "ux"}));
    assert!(stats["structuredContent"]["max"].as_f64().unwrap() <= 1.0 + 1e-9);

    let img = call(
        &s,
        "render_image",
        json!({"field": "speed", "vectors": true, "width": 200}),
    );
    assert_eq!(img["content"][0]["type"], "image");
    assert_eq!(img["content"][0]["mimeType"], "image/png");
    assert!(img["content"][0]["data"].as_str().unwrap().starts_with("iVBOR"));

    let dir = std::env::temp_dir().join(format!("rcfd_mcp_{}", std::process::id()));
    let ex = call(&s, "export_vtk", json!({"dir": dir.to_str().unwrap()}));
    assert_eq!(ex["isError"], false, "{ex}");
    assert!(dir.join("solution.vtu").is_file());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn argument_errors_are_json_rpc_errors_and_failures_are_tool_errors() {
    let s = server(Policy::default());
    let r = rpc(&s, 1, "tools/call", json!({"name": "nope", "arguments": {}}));
    assert_eq!(r["error"]["code"], -32602);
    let r = rpc(
        &s,
        1,
        "tools/call",
        json!({"name": "set_setting", "arguments": {"path": "a"}}),
    );
    assert_eq!(r["error"]["code"], -32602);
    assert!(r["error"]["message"].as_str().unwrap().contains("value"));
    let r = rpc(
        &s,
        1,
        "tools/call",
        json!({"name": "get_field_stats", "arguments": {"name": 3}}),
    );
    assert_eq!(r["error"]["code"], -32602);
    let r = rpc(&s, 1, "tools/call", json!({"name": "run", "arguments": {"bogus": 1}}));
    assert_eq!(r["error"]["code"], -32602);
    // valid call that fails: no session yet
    let r = call(&s, "status", json!({}));
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["error"]["code"], 1001);
    // validation failure carries diagnostics
    call(&s, "create_case", json!({"template": "lid_driven_cavity"}));
    let r = call(&s, "set_setting", json!({"path": "solver.alpha_u", "value": 7.5}));
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["error"]["code"], 1003);
    assert_eq!(
        r["structuredContent"]["error"]["data"]["diagnostics"][0]["path"],
        "solver.alpha_u"
    );
    // field before any run
    let r = call(&s, "get_field_stats", json!({"name": "p"}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1002);
}

#[test]
fn read_only_policy_blocks_mutating_tools() {
    let s = server(Policy::read_only());
    for (tool, args) in [
        ("create_case", json!({"template": "lid_driven_cavity"})),
        ("set_setting", json!({"path": "mesh.nx", "value": 4})),
        ("load_mesh", json!({"path": "x.msh"})),
        ("run", json!({})),
        ("export_vtk", json!({})),
    ] {
        let r = call(&s, tool, args);
        assert_eq!(r["isError"], true, "{tool}");
        assert_eq!(r["structuredContent"]["error"]["code"], PERMISSION_DENIED, "{tool}");
    }
    assert_eq!(call(&s, "list_cases", json!({"dir": "../../cases"}))["isError"], false);
}

#[test]
fn approval_hook_sees_every_mutating_call_and_can_refuse() {
    let asked = Arc::new(AtomicUsize::new(0));
    let a2 = asked.clone();
    let s = server(Policy::with_hook(move |c| {
        a2.fetch_add(1, Ordering::SeqCst);
        c.tool != "run"
    }));
    assert_eq!(
        call(&s, "create_case", json!({"template": "lid_driven_cavity"}))["isError"],
        false
    );
    assert_eq!(
        call(&s, "set_setting", json!({"path": "mesh.nx", "value": 6}))["isError"],
        false
    );
    assert_eq!(call(&s, "get_setting", json!({}))["isError"], false);
    let r = call(&s, "run", json!({}));
    assert_eq!(r["structuredContent"]["error"]["code"], PERMISSION_DENIED);
    assert_eq!(asked.load(Ordering::SeqCst), 3, "read tools must not trigger approval");
    assert!(matches!(Policy::default().approval, Approval::Auto));
}

#[test]
fn allow_list_hides_other_tools_from_calls() {
    let p = Policy {
        allow: Some(vec!["list_cases".into()]),
        ..Default::default()
    };
    let s = server(p);
    let r = call(&s, "create_case", json!({"template": "lid_driven_cavity"}));
    assert_eq!(r["structuredContent"]["error"]["code"], PERMISSION_DENIED);
}

#[test]
fn sandbox_blocks_path_escape_and_caps_are_enforced() {
    let root = std::env::temp_dir().join(format!("rcfd_mcp_sbx_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        max_cells: 100,
        max_steps_per_call: 20,
        ..Default::default()
    });
    let s = McpServer::new(svc, Policy::default());
    call(&s, "create_case", json!({"template": "lid_driven_cavity"}));
    // path escape
    let r = call(&s, "load_mesh", json!({"path": "../outside.msh"}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1004);
    let r = call(&s, "export_vtk", json!({"dir": "../outside"}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1004);
    let r = call(&s, "create_case", json!({"path": "/etc/passwd"}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1004);
    // cell cap: template is 16x16 = 256 > 100
    let r = call(&s, "run", json!({"max_steps": 5}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1004);
    call(&s, "set_setting", json!({"path": "mesh.nx", "value": 8}));
    call(&s, "set_setting", json!({"path": "mesh.ny", "value": 8}));
    // step cap
    let r = call(&s, "run", json!({"max_steps": 21}));
    assert_eq!(r["structuredContent"]["error"]["code"], 1004);
    let r = call(&s, "run", json!({"max_steps": 5}));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["simulation"]["iteration"], 5);
    // export inside the sandbox works and stays inside
    let r = call(&s, "export_vtk", json!({"dir": "out"}));
    assert_eq!(r["isError"], false, "{r}");
    assert!(root.join("out").join("solution.vtu").is_file());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn stdio_loop_handles_a_scripted_session() {
    let s = server(Policy::default());
    let input = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\nnot json\n";
    let mut out = Vec::new();
    s.serve(input.as_bytes(), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-03-26");
    // 29 + 4 design ledger tools (wave 46D, ADR 0121)
    assert_eq!(lines[1]["result"]["tools"].as_array().unwrap().len(), 33);
    assert_eq!(lines[2]["error"]["code"], -32700);
}

#[test]
fn workflow_and_headless_tools_run_a_case_and_read_the_status_stream() {
    let dir = std::env::temp_dir().join(format!("rcfd_mcp_wf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = McpServer::new(
        Service::new(ServiceConfig {
            root: Some(dir.clone()),
            ..Default::default()
        }),
        Policy::default(),
    );
    let payload = |r: &Value| -> Value { serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap() };
    call(&s, "create_case", json!({"template": "lid_driven_cavity"}));
    call(&s, "set_setting", json!({"path": "mesh.nx", "value": 8}));
    call(&s, "set_setting", json!({"path": "mesh.ny", "value": 8}));
    // listing before creating a workflow is an error result, not a protocol error
    let r = call(&s, "list_workflow_tasks", json!({}));
    assert_eq!(r["isError"], true);
    let r = payload(&call(&s, "create_workflow", json!({"template": "mesh_to_results"})));
    assert_eq!(r["next"], "mesh_check");
    let r = payload(&call(&s, "check_case_headless", json!({})));
    assert_eq!(r["exit_code"], 0, "{r}");
    // edit: insert then refuse a delete that others depend on
    let r = call(
        &s,
        "edit_workflow",
        json!({"action": "insert", "after": "report", "task": {"id": "export", "kind": "export_vtk", "deps": ["solve"], "params": {"dir": "res"}}}),
    );
    assert!(r.get("isError").is_none() || r["isError"] == false, "{r}");
    let r = call(&s, "edit_workflow", json!({"action": "delete", "id": "solve"}));
    assert_eq!(r["isError"], true);
    let r = payload(&call(&s, "run_workflow_task", json!({"task": "export"})));
    assert_eq!(r["ok"], true, "{r}");
    assert!(dir.join("res/solution.vtu").exists());
    let l = payload(&call(&s, "list_workflow_tasks", json!({})));
    assert_eq!(l["tasks"].as_array().unwrap().len(), 6);
    // headless: step-limited launch writes a status stream the reader tool summarises
    call(
        &s,
        "set_setting",
        json!({"path": "solver.max_iterations", "value": 500}),
    );
    let r = payload(&call(
        &s,
        "run_headless",
        json!({"max_steps": 12, "status_path": "st.jsonl", "checkpoint": "c.rcfd"}),
    ));
    assert_eq!(r["outcome"]["state"], "step_limit", "{r}");
    let sum = payload(&call(&s, "read_status_stream", json!({"path": "st.jsonl"})));
    assert_eq!(
        (sum["state"].clone(), sum["iterations"].clone()),
        (json!("step_limit"), json!(12))
    );
    // the sandbox applies to the stream reader
    let r = call(&s, "read_status_stream", json!({"path": "../x.jsonl"}));
    assert_eq!(r["isError"], true);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn workflow_tools_are_gated_by_the_permission_model() {
    let s = server(Policy::read_only());
    let list = rpc(&s, 1, "tools/list", json!({}))["result"]["tools"].clone();
    for (name, read) in [
        ("list_workflow_tasks", true),
        ("check_case_headless", true),
        ("read_status_stream", true),
        ("create_workflow", false),
        ("edit_workflow", false),
        ("run_workflow_task", false),
        ("run_headless", false),
    ] {
        let t = list.as_array().unwrap().iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(t["annotations"]["readOnlyHint"], read, "{name}");
    }
    let r = call(&s, "create_workflow", json!({"template": "mesh_to_results"}));
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"].as_str().unwrap().contains("permission denied"));
}
