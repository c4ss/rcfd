//! Task-based workflow engine and headless run methods of the service (F-120, F-121).

use rcfd_api::service::{Service, ServiceConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

const CAVITY: &str = r#"
title = "workflow cavity"
[mesh]
generator = "cavity_2d"
nx = 10
ny = 10
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_wf_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn setup(root: Option<PathBuf>) -> (Arc<Service>, String) {
    let svc = Service::new(ServiceConfig {
        root,
        ..Default::default()
    });
    let sid = svc.call("session.create", &json!({"toml": CAVITY})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    (svc, sid)
}

fn states(svc: &Arc<Service>, sid: &str) -> Vec<(String, String)> {
    let l = svc.call("workflow.list", &json!({"session": sid})).unwrap();
    l["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["id"].as_str().unwrap().into(), t["state"].as_str().unwrap().into()))
        .collect()
}

fn st(v: &[(String, String)], id: &str) -> String {
    v.iter().find(|x| x.0 == id).unwrap().1.clone()
}

#[test]
fn template_runs_end_to_end_and_setting_changes_make_tasks_stale() {
    let (svc, sid) = setup(None);
    // no workflow yet
    let e = svc.call("workflow.list", &json!({"session": sid})).unwrap_err();
    assert!(e.message.contains("workflow.create"), "{e:?}");
    let e = svc
        .call("workflow.create", &json!({"session": sid, "template": "nope"}))
        .unwrap_err();
    assert!(e.message.contains("mesh_to_results"), "{e:?}");
    let c = svc
        .call(
            "workflow.create",
            &json!({"session": sid, "template": "mesh_to_results"}),
        )
        .unwrap();
    assert_eq!(c["next"], "mesh_check");
    assert!(states(&svc, &sid).iter().all(|s| s.1 == "not_run"));

    // asking for the solve task pulls in its prerequisites, but not the report
    let r = svc
        .call("workflow.run", &json!({"session": sid, "task": "solve"}))
        .unwrap();
    assert_eq!(r["ok"], true, "{r}");
    let ran: Vec<&str> = r["ran"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ran, ["mesh_check", "setup", "initialize", "solve"]);
    let s = states(&svc, &sid);
    assert_eq!(st(&s, "solve"), "done");
    assert_eq!(st(&s, "report"), "not_run");
    let solve = &r["ran"][3];
    assert_eq!(solve["result"]["outcome"]["state"], "converged", "{solve}");

    // without prerequisites a task whose inputs are not done is refused
    let (svc2, sid2) = setup(None);
    svc2.call("workflow.create", &json!({"session": sid2})).unwrap();
    let e = svc2
        .call(
            "workflow.run",
            &json!({"session": sid2, "task": "solve", "with_prerequisites": false}),
        )
        .unwrap_err();
    assert!(e.message.contains("not done"), "{e:?}");

    // running everything outstanding finishes the workflow and reports real values
    let r = svc.call("workflow.run", &json!({"session": sid})).unwrap();
    assert_eq!(r["ran"].as_array().unwrap().len(), 1);
    assert_eq!(r["workflow"]["complete"], true);
    let drag = &r["ran"][0]["result"]["reports"];
    assert!(drag.to_string().contains("lid_drag"), "{drag}");

    // a setting change from outside the workflow makes every done task stale
    svc.call(
        "settings.set",
        &json!({"session": sid, "path": "physics.nu", "value": 0.02}),
    )
    .unwrap();
    let s = states(&svc, &sid);
    assert!(s.iter().all(|x| x.1 == "stale"), "{s:?}");
    // and re-running recomputes with the new viscosity
    let r = svc
        .call("workflow.run", &json!({"session": sid, "task": "report"}))
        .unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["ran"].as_array().unwrap().len(), 5);
}

#[test]
fn set_task_changes_the_case_and_marks_its_dependants_stale() {
    let (svc, sid) = setup(None);
    svc.call("workflow.create", &json!({"session": sid})).unwrap();
    svc.call(
        "workflow.insert",
        &json!({"session": sid, "after": "mesh_check", "task": {
            "id": "finer", "kind": "set", "deps": ["mesh_check"],
            "params": {"path": "mesh.nx", "value": 14}}}),
    )
    .unwrap();
    // initialize now also needs the set task
    let e = svc
        .call("workflow.delete", &json!({"session": sid, "id": "mesh_check"}))
        .unwrap_err();
    assert!(e.message.contains("depend"), "{e:?}");
    let r = svc
        .call("workflow.run", &json!({"session": sid, "task": "finer"}))
        .unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(
        svc.call("settings.get", &json!({"session": sid, "path": "mesh.nx"}))
            .unwrap()["value"],
        14
    );
    // the set task came from the workflow, so its own state survives
    assert_eq!(st(&states(&svc, &sid), "finer"), "done");
    // a set task changes the case under every other result: they are stale, the set task is done
    let s = states(&svc, &sid);
    assert_eq!(st(&s, "mesh_check"), "stale");
    svc.call("workflow.run", &json!({"session": sid, "task": "report"}))
        .unwrap();
    assert_eq!(st(&states(&svc, &sid), "report"), "done");
    svc.call(
        "workflow.run",
        &json!({"session": sid, "task": "finer", "with_prerequisites": false}),
    )
    .unwrap();
    let s = states(&svc, &sid);
    assert_eq!(st(&s, "report"), "stale");
    assert_eq!(st(&s, "solve"), "stale");
    assert_eq!(st(&s, "finer"), "done");
}

#[test]
fn failing_tasks_stop_the_run_and_say_why() {
    let (svc, sid) = setup(None);
    svc.call("workflow.create", &json!({"session": sid})).unwrap();
    // cap the iterations below convergence: the solve task fails, nothing after it runs
    let r = svc
        .call(
            "workflow.run",
            &json!({"session": sid, "task": "report", "max_steps": 5}),
        )
        .unwrap();
    assert_eq!(r["ok"], false);
    let last = r["ran"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["id"], "solve");
    assert!(last["message"].as_str().unwrap().contains("step_limit"), "{last}");
    let s = states(&svc, &sid);
    assert_eq!(
        (st(&s, "solve").as_str(), st(&s, "report").as_str()),
        ("failed", "not_run")
    );
    // an invalid set-up fails the validate task with the diagnostics
    let (svc, sid) = setup(None);
    svc.call("workflow.create", &json!({"session": sid})).unwrap();
    svc.call(
        "settings.set",
        &json!({"session": sid, "path": "boundaries.nonexistent.velocity", "value": "fixed"}),
    )
    .unwrap();
    let r = svc.call("workflow.run", &json!({"session": sid})).unwrap();
    assert_eq!(r["ok"], false);
    let bad = r["ran"].as_array().unwrap().iter().find(|t| t["ok"] == false).unwrap();
    assert!(bad["message"].as_str().unwrap().contains("nonexistent"), "{bad}");
}

#[test]
fn workflow_files_are_sandboxed_and_round_trip() {
    let root = tmp("files");
    let (svc, sid) = setup(Some(root.clone()));
    svc.call("workflow.create", &json!({"session": sid, "template": "channel_flow"}))
        .unwrap();
    svc.call("workflow.save", &json!({"session": sid, "path": "wf/flow.json"}))
        .unwrap();
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(root.join("wf/flow.json")).unwrap()).unwrap();
    assert_eq!(doc["format"], "rcfd-workflow");
    assert_eq!(doc["tasks"].as_array().unwrap().len(), 6);
    let (svc2, sid2) = setup(Some(root.clone()));
    let l = svc2
        .call("workflow.load", &json!({"session": sid2, "path": "wf/flow.json"}))
        .unwrap();
    assert_eq!(l["tasks"].as_array().unwrap().len(), 6);
    for bad in ["../outside.json", "/etc/passwd"] {
        let e = svc2
            .call("workflow.save", &json!({"session": sid2, "path": bad}))
            .unwrap_err();
        assert!(e.message.contains("sandbox"), "{bad}: {e:?}");
    }
    // a hand-edited file with a cycle is rejected
    let mut bad = doc.clone();
    bad["tasks"][0]["deps"] = json!(["export_results"]);
    std::fs::write(root.join("wf/bad.json"), bad.to_string()).unwrap();
    let e = svc2
        .call("workflow.load", &json!({"session": sid2, "path": "wf/bad.json"}))
        .unwrap_err();
    assert!(e.message.contains("cycle"), "{e:?}");
    // the export task writes inside the sandbox
    let r = svc2
        .call("workflow.run", &json!({"session": sid2, "task": "export_results"}))
        .unwrap();
    assert_eq!(r["ok"], true, "{r}");
    assert!(root.join("results/solution.vtu").exists());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn headless_run_check_stop_file_checkpoint_and_status_file() {
    let root = tmp("headless");
    let (svc, sid) = setup(Some(root.clone()));
    // check-only
    let c = svc.call("run.check", &json!({"session": sid})).unwrap();
    assert_eq!((c["ok"].clone(), c["exit_code"].clone()), (json!(true), json!(0)));
    // run 25 steps and checkpoint, writing the status stream to a file
    let r = svc
        .call(
            "run.headless",
            &json!({"session": sid, "max_steps": 25, "checkpoint": "ck/a.rcfd",
                    "status_path": "out/status.jsonl", "status_every": 10}),
        )
        .unwrap();
    assert_eq!(r["outcome"]["state"], "step_limit", "{r}");
    assert_eq!(r["status"]["state"], "ready");
    assert!(root.join("ck/a.rcfd").exists());
    let sum = svc.call("status.read", &json!({"path": "out/status.jsonl"})).unwrap();
    assert_eq!(
        (
            sum["state"].clone(),
            sum["complete"].clone(),
            sum["checkpoints"].clone()
        ),
        (json!("step_limit"), json!(true), json!(1))
    );
    // a stop file interrupts the next launch before it steps
    std::fs::write(root.join("STOP"), "").unwrap();
    let r = svc
        .call("run.headless", &json!({"session": sid, "stop_file": "STOP"}))
        .unwrap();
    assert_eq!(r["outcome"]["exit_code"], 7, "{r}");
    assert_eq!(r["status"]["state"], "stopped");
    std::fs::remove_file(root.join("STOP")).unwrap();
    // continue from the checkpoint to convergence and write results
    let r = svc
        .call(
            "run.headless",
            &json!({"session": sid, "continue_from": "ck/a.rcfd", "out_dir": "res"}),
        )
        .unwrap();
    assert_eq!(r["outcome"]["state"], "converged", "{r}");
    assert!(r["outcome"]["iterations"].as_u64().unwrap() > 25);
    assert!(root.join("res/solution.vtu").exists());
    // sandbox applies to every path argument
    for key in ["continue_from", "stop_file", "checkpoint", "status_path", "out_dir"] {
        let e = svc
            .call("run.headless", &json!({"session": sid, key: "../x"}))
            .unwrap_err();
        assert!(e.message.contains("sandbox"), "{key}: {e:?}");
    }
    // a run in progress cannot be started again
    let methods = svc.call("service.info", &json!({})).unwrap()["methods"].clone();
    for m in ["workflow.run", "run.headless", "run.check", "status.read"] {
        assert!(methods.to_string().contains(m), "{m} not advertised");
    }
    std::fs::remove_dir_all(&root).ok();
}
