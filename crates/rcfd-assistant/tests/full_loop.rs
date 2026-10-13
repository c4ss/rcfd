//! F-147: the whole assistant loop driven by the mock provider, no network.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_assistant::mock::MockProvider;
use rcfd_assistant::*;
use serde_json::{json, Value};
use std::sync::Arc;

struct Rig {
    svc: Arc<Service>,
    tb: Arc<Toolbox>,
    mock: Arc<MockProvider>,
}

fn rig(mode: ApprovalMode) -> Rig {
    let svc = Service::new(ServiceConfig::default());
    let tb = Arc::new(Toolbox::new(svc.clone(), "cases", mode));
    Rig {
        svc,
        tb,
        mock: Arc::new(MockProvider::empty()),
    }
}

fn agent(r: &Rig) -> Agent {
    Agent::new(r.mock.clone(), r.tb.clone(), "test-model")
}

fn setting(r: &Rig, path: &str) -> Value {
    let sid = r.tb.session().expect("session");
    r.svc
        .call("settings.get", &json!({"session": sid, "path": path}))
        .unwrap()["value"]
        .clone()
}

fn last_tool_result_text(req: &ChatRequest) -> String {
    for m in req.messages.iter().rev() {
        for b in &m.blocks {
            if let Block::ToolResult { content, .. } = b {
                return content.clone();
            }
        }
    }
    String::new()
}

fn script_setup_and_run(r: &Rig) {
    r.mock.push(ChatResponse::tool_calls(
        "I will create a lid-driven cavity case.",
        vec![("c1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    r.mock.push(ChatResponse::tool_calls(
        "Use a coarse mesh and a short run.",
        vec![
            ("c2", "set_setting", json!({"path": "mesh.nx", "value": 8})),
            ("c3", "set_setting", json!({"path": "mesh.ny", "value": 8})),
            (
                "c4",
                "set_setting",
                json!({"path": "solver.max_iterations", "value": 20}),
            ),
        ],
    ));
    r.mock.push(ChatResponse::tool_calls(
        "Checking mesh and case first.",
        vec![("c5", "assess_mesh", json!({})), ("c6", "validate_case", json!({}))],
    ));
    r.mock.push(ChatResponse::tool_calls(
        "Running.",
        vec![("c7", "run", json!({"max_steps": 20, "wait_seconds": 120}))],
    ));
    r.mock.push(ChatResponse::tool_calls(
        "Looking at the residuals.",
        vec![("c8", "explain_residuals", json!({}))],
    ));
    r.mock.push_fn(|req| {
        let t = last_tool_result_text(req);
        let v: Value = serde_json::from_str(&t).unwrap_or(json!({}));
        ChatResponse::text(format!(
            "Done. The run finished 20 iterations; residual verdict: {}.",
            v["verdict"].as_str().unwrap_or("unknown")
        ))
    });
}

#[test]
fn request_to_tool_calls_to_approval_to_edit_to_run_to_summary() {
    let r = rig(ApprovalMode::ApproveWrites);
    script_setup_and_run(&r);
    let mut a = agent(&r);
    let mut asked: Vec<String> = vec![];
    let mut executed: Vec<String> = vec![];
    let mut streamed = String::new();
    let text = a
        .run_turn(
            "Set up a lid-driven cavity at Re=100, check it and run it.",
            &mut |p| {
                asked.push(p.name.clone());
                Decision::Approve
            },
            &mut |e| match e {
                AgentEvent::ToolResult { name, output, .. } => {
                    assert!(!output.is_error, "{name}: {}", output.text);
                    executed.push(name);
                }
                AgentEvent::Text(t) => streamed.push_str(&t),
                _ => {}
            },
        )
        .unwrap();
    // Only mutating/executing calls prompted.
    assert_eq!(
        asked,
        ["create_case", "set_setting", "set_setting", "set_setting", "run"]
    );
    assert_eq!(
        executed,
        [
            "create_case",
            "set_setting",
            "set_setting",
            "set_setting",
            "assess_mesh",
            "validate_case",
            "run",
            "explain_residuals"
        ]
    );
    // The edits really happened and the run really iterated.
    assert_eq!(setting(&r, "mesh.nx"), json!(8));
    assert_eq!(setting(&r, "solver.max_iterations"), json!(20));
    let sid = r.tb.session().unwrap();
    let st = r.svc.call("run.status", &json!({"session": sid})).unwrap();
    assert_eq!(st["simulation"]["iteration"], 20, "{st}");
    // Summary derived from a real tool result.
    assert!(
        text.starts_with("Done. The run finished 20 iterations; residual verdict: "),
        "{text}"
    );
    assert!(!text.contains("unknown"), "{text}");
    assert!(streamed.contains("lid-driven cavity case"));
    // Six model calls; later requests carry tool results back.
    assert_eq!(r.mock.request_count(), 6);
    let reqs = r.mock.requests.lock().unwrap();
    assert!(reqs[0].tools.iter().any(|t| t.name == "suggest_boundary_conditions"));
    assert!(reqs[0].tools.iter().any(|t| t.name == "run"));
    assert!(reqs[0].system.contains("Current context"));
    assert!(matches!(
        reqs[1].messages.last().unwrap().blocks[0],
        Block::ToolResult { .. }
    ));
    // History is well formed: every tool_use is answered.
    let uses: usize = a
        .history()
        .iter()
        .flat_map(|m| &m.blocks)
        .filter(|b| matches!(b, Block::ToolUse { .. }))
        .count();
    let results: usize = a
        .history()
        .iter()
        .flat_map(|m| &m.blocks)
        .filter(|b| matches!(b, Block::ToolResult { .. }))
        .count();
    assert_eq!((uses, results), (8, 8));
    assert_eq!(a.transcript()["format"], "rcfd-assistant-transcript");
}

#[test]
fn denied_call_returns_permission_denied_and_leaves_the_case_unchanged() {
    let r = rig(ApprovalMode::ApproveWrites);
    r.mock.push(ChatResponse::tool_calls(
        "",
        vec![("c1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    r.mock.push(ChatResponse::tool_calls(
        "",
        vec![("c2", "set_setting", json!({"path": "mesh.nx", "value": 99}))],
    ));
    r.mock.push_fn(|req| {
        let t = last_tool_result_text(req);
        ChatResponse::text(format!("Understood, leaving it. ({})", t.contains("permission denied")))
    });
    let mut a = agent(&r);
    let mut denied_events = 0;
    let text = a
        .run_turn(
            "make the mesh finer",
            &mut |p| {
                if p.name == "set_setting" {
                    Decision::Deny("not now".into())
                } else {
                    Decision::Approve
                }
            },
            &mut |e| {
                if let AgentEvent::ToolResult {
                    denied: true, output, ..
                } = e
                {
                    denied_events += 1;
                    assert!(output.is_error);
                }
            },
        )
        .unwrap();
    assert_eq!(denied_events, 1);
    assert_eq!(text, "Understood, leaving it. (true)");
    assert_eq!(setting(&r, "mesh.nx"), json!(16), "denied edit must not apply");
    let last_results: Vec<_> = a
        .history()
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(|b| match b {
            Block::ToolResult { content, is_error, .. } => Some((content.clone(), *is_error)),
            _ => None,
        })
        .collect();
    assert!(last_results.last().unwrap().0.contains("1100"));
    assert!(last_results.last().unwrap().1);
}

#[test]
fn read_only_mode_hides_and_refuses_every_mutating_tool() {
    let r = rig(ApprovalMode::ReadOnly);
    r.mock.push(ChatResponse::tool_calls(
        "",
        vec![
            ("c1", "create_case", json!({"template": "lid_driven_cavity"})),
            ("c2", "run", json!({})),
            ("c3", "export_vtk", json!({})),
            ("c4", "list_cases", json!({})),
        ],
    ));
    r.mock.push(ChatResponse::text("ok"));
    let mut a = agent(&r);
    let mut asked = 0;
    let mut outputs = vec![];
    a.run_turn(
        "go",
        &mut |_| {
            asked += 1;
            Decision::Approve
        },
        &mut |e| {
            if let AgentEvent::ToolResult { name, output, .. } = e {
                outputs.push((name, output.is_error));
            }
        },
    )
    .unwrap();
    assert_eq!(asked, 0, "read-only mode must never prompt");
    assert_eq!(
        outputs,
        [
            ("create_case".to_string(), true),
            ("run".to_string(), true),
            ("export_vtk".to_string(), true),
            ("list_cases".to_string(), false)
        ]
    );
    let reqs = r.mock.requests.lock().unwrap();
    for t in &reqs[0].tools {
        assert_eq!(
            r.tb.info(&t.name).unwrap().access,
            rcfd_mcp::policy::Access::Read,
            "{}",
            t.name
        );
    }
    assert!(r.tb.session().is_none(), "no case was created");
}

#[test]
fn the_gate_is_enforced_below_the_ui() {
    let r = rig(ApprovalMode::ApproveWrites);
    // Calling a mutating tool without a grant is refused by the server-side policy.
    let o = r.tb.execute("create_case", &json!({"template": "lid_driven_cavity"}));
    assert!(o.is_error && o.text.contains("1100"), "{}", o.text);
    assert!(r.tb.session().is_none());
    // A grant for different arguments does not authorise this call.
    r.tb.grant("create_case", &json!({"template": "other"}));
    let o = r.tb.execute("create_case", &json!({"template": "lid_driven_cavity"}));
    assert!(o.is_error, "{}", o.text);
    // A matching grant is single use.
    r.tb.grant("create_case", &json!({"template": "lid_driven_cavity"}));
    assert!(
        !r.tb
            .execute("create_case", &json!({"template": "lid_driven_cavity"}))
            .is_error
    );
    assert!(
        r.tb.execute("create_case", &json!({"template": "lid_driven_cavity"}))
            .is_error
    );
    // Switching to read-only drops outstanding grants and refuses writes.
    r.tb.grant("set_setting", &json!({"path": "mesh.nx", "value": 4}));
    r.tb.set_mode(ApprovalMode::ReadOnly);
    r.tb.set_mode(ApprovalMode::ApproveWrites);
    assert!(
        r.tb.execute("set_setting", &json!({"path": "mesh.nx", "value": 4}))
            .is_error
    );
}

#[test]
fn bad_tool_calls_become_tool_errors_not_panics() {
    let r = rig(ApprovalMode::ApproveWrites);
    r.mock.push(ChatResponse::tool_calls(
        "",
        vec![
            ("a", "no_such_tool", json!({})),
            ("b", "get_setting", json!({"path": 5})),
            ("c", "get_setting", json!({ INVALID_ARGS_KEY: "{path: " })),
        ],
    ));
    r.mock.push(ChatResponse::text("sorry"));
    let mut a = agent(&r);
    let mut errs = vec![];
    a.run_turn("x", &mut |_| Decision::Approve, &mut |e| {
        if let AgentEvent::ToolResult { output, .. } = e {
            errs.push(output.text);
        }
    })
    .unwrap();
    assert_eq!(errs.len(), 3);
    assert!(errs[0].contains("unknown tool"));
    assert!(errs[1].contains("invalid arguments"));
    assert!(errs[2].contains("not a valid JSON object"));
}

#[test]
fn provider_failure_keeps_history_valid_and_can_be_retried() {
    let r = rig(ApprovalMode::ApproveWrites);
    let mut a = agent(&r);
    a.submit("hello").unwrap();
    // No scripted reply: the mock errors.
    assert!(matches!(a.step(&mut |_| {}), Err(AgentError::Provider(_))));
    r.mock.push(ChatResponse::text("hi"));
    assert_eq!(a.step(&mut |_| {}).unwrap(), Step::Done("hi".into()));
}

#[test]
fn cancel_closes_the_turn_cleanly() {
    let r = rig(ApprovalMode::ApproveWrites);
    r.mock.push(ChatResponse::tool_calls(
        "",
        vec![("c1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    let mut a = agent(&r);
    a.submit("go").unwrap();
    assert!(matches!(a.step(&mut |_| {}).unwrap(), Step::NeedApproval(_)));
    a.cancel_flag().store(true, std::sync::atomic::Ordering::SeqCst);
    // Misuse while a decision is pending is a protocol error.
    assert!(matches!(a.step(&mut |_| {}), Err(AgentError::Protocol(_))));
    a.resolve(Decision::Deny("cancel".into()), &mut |_| {}).unwrap();
    assert_eq!(a.step(&mut |_| {}), Err(AgentError::Cancelled));
    // Next turn works.
    r.mock.push(ChatResponse::text("fine"));
    a.submit("again").unwrap();
    assert_eq!(a.step(&mut |_| {}).unwrap(), Step::Done("fine".into()));
}

#[test]
fn large_results_are_bounded_for_the_model() {
    let r = rig(ApprovalMode::ApproveWrites);
    r.tb.grant("create_case", &json!({"template": "lid_driven_cavity"}));
    assert!(
        !r.tb
            .execute("create_case", &json!({"template": "lid_driven_cavity"}))
            .is_error
    );
    for (k, v) in [
        ("mesh.nx", json!(6)),
        ("mesh.ny", json!(6)),
        ("solver.max_iterations", json!(400)),
        ("solver.tolerance_u", json!(1e-14)),
        ("solver.tolerance_p", json!(1e-14)),
    ] {
        let args = json!({"path": k, "value": v});
        r.tb.grant("set_setting", &args);
        assert!(!r.tb.execute("set_setting", &args).is_error);
    }
    let args = json!({"max_steps": 400, "wait_seconds": 120});
    r.tb.grant("run", &args);
    assert!(!r.tb.execute("run", &args).is_error);
    let o = r.tb.execute("get_residuals", &json!({}));
    let v: Value = serde_json::from_str(&o.text).unwrap();
    let ys = v["series"]["res_u"]["y"].as_array().unwrap();
    assert!(ys.len() > 100, "run stopped early: {}", ys.len());
    assert!(ys.len() <= rcfd_assistant::tools::MAX_SERIES_POINTS + 1, "{}", ys.len());
    assert_eq!(v["series"]["res_u"]["thinned_from"], 400);
    // The structured copy kept for the UI is complete.
    assert_eq!(o.structured["series"]["res_u"]["y"].as_array().unwrap().len(), 400);
    // Images are not sent to the model.
    r.tb.execute("render_image", &json!({"width": 128}));
    let o = r.tb.execute("render_image", &json!({"width": 128}));
    assert!(o.image_png_base64.is_some());
    assert!(
        o.text.len() < 600 && o.text.contains("not visible to you"),
        "{}",
        o.text
    );
}
