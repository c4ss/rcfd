//! F-120/F-121: a scripted end-to-end workflow driven through the assistant with the mock
//! provider (no network): create a case, create the task workflow, run it task by task through
//! the approval gate, run headless with a status stream, read it back.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_assistant::mock::MockProvider;
use rcfd_assistant::*;
use serde_json::{json, Value};
use std::sync::Arc;

fn last_tool_result(req: &ChatRequest) -> Value {
    for m in req.messages.iter().rev() {
        for b in m.blocks.iter().rev() {
            if let Block::ToolResult { content, .. } = b {
                return serde_json::from_str(content).unwrap_or(Value::Null);
            }
        }
    }
    Value::Null
}

#[test]
fn assistant_drives_the_workflow_engine_and_the_headless_contract() {
    let root = std::env::temp_dir().join(format!("rcfd_asst_wf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    let tb = Arc::new(Toolbox::new(svc.clone(), "cases", ApprovalMode::ApproveWrites));
    let mock = Arc::new(MockProvider::empty());

    mock.push(ChatResponse::tool_calls(
        "Create the case and a standard workflow.",
        vec![
            ("c1", "create_case", json!({"template": "lid_driven_cavity"})),
            ("c2", "set_setting", json!({"path": "mesh.nx", "value": 10})),
            ("c3", "set_setting", json!({"path": "mesh.ny", "value": 10})),
            ("c4", "create_workflow", json!({"template": "mesh_to_results"})),
            ("c5", "list_workflow_tasks", json!({})),
        ],
    ));
    mock.push(ChatResponse::tool_calls(
        "Check the case the way a batch launch would, then run the solve task with its prerequisites.",
        vec![
            ("c6", "check_case_headless", json!({})),
            ("c7", "run_workflow_task", json!({"task": "solve"})),
        ],
    ));
    mock.push(ChatResponse::tool_calls(
        "Add an export and finish the workflow.",
        vec![
            (
                "c8",
                "edit_workflow",
                json!({"action": "insert", "after": "report", "task": {"id": "export", "kind": "export_vtk", "deps": ["solve"], "params": {"dir": "results"}}}),
            ),
            ("c9", "run_workflow_task", json!({})),
        ],
    ));
    mock.push(ChatResponse::tool_calls(
        "Change a setting: the finished tasks must become stale.",
        vec![
            ("c10", "set_setting", json!({"path": "physics.nu", "value": 0.02})),
            ("c11", "list_workflow_tasks", json!({})),
        ],
    ));
    mock.push(ChatResponse::tool_calls(
        "Run headless with a status stream, then read it.",
        vec![
            (
                "c12",
                "run_headless",
                json!({"max_steps": 15, "status_path": "status.jsonl", "checkpoint": "restart.rcfd"}),
            ),
            ("c13", "read_status_stream", json!({"path": "status.jsonl"})),
        ],
    ));
    mock.push_fn(|req| {
        let v = last_tool_result(req);
        ChatResponse::text(format!(
            "Stream says {} after {} iterations.",
            v["state"].as_str().unwrap_or("?"),
            v["iterations"]
        ))
    });

    let mut agent = Agent::new(mock.clone(), tb.clone(), "test-model");
    let mut asked: Vec<String> = vec![];
    let mut results: Vec<(String, Value)> = vec![];
    let text = agent
        .run_turn(
            "Set up the cavity workflow, run it, change the viscosity and launch headless.",
            &mut |p| {
                asked.push(p.name.clone());
                Decision::Approve
            },
            &mut |e| {
                if let AgentEvent::ToolResult { name, output, .. } = e {
                    assert!(!output.is_error, "{name}: {}", output.text);
                    results.push((name, serde_json::from_str(&output.text).unwrap_or(Value::Null)));
                }
            },
        )
        .unwrap();

    // read-only tools never prompt; everything that changes state or writes files did
    assert_eq!(
        asked,
        [
            "create_case",
            "set_setting",
            "set_setting",
            "create_workflow",
            "run_workflow_task",
            "edit_workflow",
            "run_workflow_task",
            "set_setting",
            "run_headless"
        ]
    );
    let get = |n: &str, i: usize| results.iter().filter(|r| r.0 == n).nth(i).unwrap().1.clone();
    // workflow listing before anything ran
    let l0 = get("list_workflow_tasks", 0);
    assert_eq!(l0["next"], "mesh_check");
    assert_eq!(l0["tasks"].as_array().unwrap().len(), 5);
    // headless check, exit code 0
    assert_eq!(get("check_case_headless", 0)["exit_code"], 0);
    // solve with prerequisites: four tasks ran, the solve converged
    let r1 = get("run_workflow_task", 0);
    let ran: Vec<&str> = r1["ran"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ran, ["mesh_check", "setup", "initialize", "solve"]);
    assert_eq!(r1["ran"][3]["result"]["outcome"]["state"], "converged", "{r1}");
    // the rest of the workflow ran and it is complete
    let r2 = get("run_workflow_task", 1);
    assert_eq!(r2["workflow"]["complete"], true, "{r2}");
    assert!(root.join("results/solution.vtu").exists());
    // changing the viscosity staled everything
    let l1 = get("list_workflow_tasks", 1);
    assert!(
        l1["tasks"].as_array().unwrap().iter().all(|t| t["state"] == "stale"),
        "{l1}"
    );
    // headless launch: step limit, status file written inside the sandbox, summary agrees
    let h = get("run_headless", 0);
    assert_eq!(h["outcome"]["state"], "step_limit");
    assert!(root.join("restart.rcfd").exists());
    let sum = get("read_status_stream", 0);
    assert_eq!(
        (sum["state"].clone(), sum["iterations"].clone(), sum["complete"].clone()),
        (json!("step_limit"), json!(15), json!(true))
    );
    assert_eq!(text, "Stream says step_limit after 15 iterations.");
    assert_eq!(mock.request_count(), 6);
    // the new tools are offered to the model
    let reqs = mock.requests.lock().unwrap();
    for t in [
        "list_workflow_tasks",
        "create_workflow",
        "run_workflow_task",
        "edit_workflow",
        "check_case_headless",
        "run_headless",
        "read_status_stream",
    ] {
        assert!(reqs[0].tools.iter().any(|x| x.name == t), "model is not offered {t}");
    }
    drop(reqs);

    // a denied approval leaves the workflow untouched
    let mock2 = Arc::new(MockProvider::empty());
    let tb2 = Arc::new(Toolbox::new(svc.clone(), "cases", ApprovalMode::ApproveWrites));
    mock2.push(ChatResponse::tool_calls(
        "",
        vec![("d1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    mock2.push(ChatResponse::tool_calls(
        "",
        vec![("d2", "create_workflow", json!({"template": "mesh_to_results"}))],
    ));
    mock2.push(ChatResponse::tool_calls(
        "",
        vec![("d3", "list_workflow_tasks", json!({}))],
    ));
    mock2.push_fn(|req| {
        let t = serde_json::to_string(&last_tool_result(req)).unwrap();
        ChatResponse::text(t)
    });
    let mut a2 = Agent::new(mock2, tb2, "test-model");
    let mut saw_error = false;
    a2.run_turn(
        "make a workflow",
        &mut |p| {
            if p.name == "create_workflow" {
                Decision::Deny("not now".into())
            } else {
                Decision::Approve
            }
        },
        &mut |e| {
            if let AgentEvent::ToolResult { name, output, .. } = e {
                if name == "list_workflow_tasks" {
                    saw_error = output.is_error;
                    assert!(output.text.contains("workflow.create"), "{}", output.text);
                }
            }
        },
    )
    .unwrap();
    assert!(saw_error, "no workflow may exist after a denied create_workflow");
    std::fs::remove_dir_all(&root).ok();
}
