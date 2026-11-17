//! ADR 0734 (F-828): the project tools' part. L5 `hash_project_file` of a `.msh` file is `hash_bytes` of its bytes, of an
//! OpenFOAM folder is stable and changes with one byte, and is refused outside the project (1004) and for a missing path
//! (1202); L7 `run_status` of a run without a launch record says `launch: null`, and `run_status` and `list_runs` say
//! `time_unit: "s"`; P3 `start_run` with `from_run` reruns a run (its `run.json` says `rerun_of`, the residuals are the
//! original's) and with `max_steps` caps it, and refuses `from_run` beside `case` (-32602), an unknown run (1202) and a
//! `continue_from` that is no run (1202). The tool lists are checked by `project_list_f270.rs` and `tool_list_f275.rs`,
//! restated there.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[test]
fn l5_hash_project_file_and_l7_launch_null_and_time_unit() {
    let dir = project("adr0734-l5");
    let s = server(&dir, Policy::default());
    std::fs::create_dir_all(dir.join("meshes/foam/polyMesh")).unwrap();
    let msh = b"$MeshFormat\n2.2 0 8\n$EndMeshFormat\n".to_vec();
    std::fs::write(dir.join("meshes/a.msh"), &msh).unwrap();
    let r = ok(&call(&s, "hash_project_file", json!({"path": "meshes/a.msh"})));
    assert_eq!(r["hash"], json!(rcfd_project::records::hash_bytes(&msh)));
    assert_eq!(
        (r["kind"].clone(), r["bytes"].clone(), r["files"].clone()),
        (json!("file"), json!(msh.len()), json!(1))
    );
    assert_eq!(r["path"], "meshes/a.msh");
    let foam = dir.join("meshes/foam/polyMesh");
    for (n, b) in [
        ("points", "(0 0 0)"),
        ("faces", "()"),
        ("owner", "()"),
        ("boundary", "()"),
    ] {
        std::fs::write(foam.join(n), b).unwrap();
    }
    let a = ok(&call(&s, "hash_project_file", json!({"path": "meshes/foam"})));
    assert_eq!((a["kind"].clone(), a["files"].clone()), (json!("folder"), json!(4)));
    // stable: written again in another order, the same hash
    for n in ["boundary", "owner", "faces", "points"] {
        let b = std::fs::read(foam.join(n)).unwrap();
        std::fs::remove_file(foam.join(n)).unwrap();
        std::fs::write(foam.join(n), b).unwrap();
    }
    assert_eq!(
        ok(&call(&s, "hash_project_file", json!({"path": "meshes/foam"})))["hash"],
        a["hash"]
    );
    std::fs::write(foam.join("faces"), "(]").unwrap();
    let b = ok(&call(&s, "hash_project_file", json!({"path": "meshes/foam"})));
    assert_ne!(b["hash"], a["hash"], "one byte changed");
    // the CLI gives the same
    let o = std::process::Command::new(binaries().with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX)))
        .args(["project", "hash"])
        .arg(dir.join("meshes/foam"))
        .output()
        .unwrap();
    assert!(o.status.success());
    let cli: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(cli["hash"], b["hash"]);
    for (path, code) in [("../x.msh", 1004), ("meshes/none.msh", 1202)] {
        let r = call(&s, "hash_project_file", json!({"path": path}));
        eprintln!("L5: {path}: {}", r["structuredContent"]["error"]);
        assert_eq!(err_code(&r), code, "{path}: {r}");
    }
    let r = rpc_call(&s, "hash_project_file", json!({"path": "meshes/a.msh", "x": 1}));
    assert_eq!(r["error"]["code"], -32602, "{r}");
    eprintln!(
        "L5: file {} folder {} -> {} after one byte",
        r["hash"], a["hash"], b["hash"]
    );

    // L7: a run made without the runner has no launch record
    let p = rcfd_project::Project::open(&dir).unwrap();
    let rec = p.create_run("cases/cavity.toml", None, "rcfd test", json!({})).unwrap();
    let st = ok(&call(&s, "run_status", json!({"run": rec.id})));
    assert_eq!(st["launch"], Value::Null);
    assert_eq!(st["time_unit"], "s");
    assert_eq!(ok(&call(&s, "list_runs", json!({})))["time_unit"], "s");
}

/// Call `wait_run` until the run is terminal.
fn wait_terminal(m: &mut Mcp, run: &str) -> Value {
    let t0 = Instant::now();
    loop {
        let v = m.call("wait_run", json!({"run": run, "timeout_s": 60}))["structuredContent"].clone();
        if v["terminal"] == true {
            return v;
        }
        assert!(t0.elapsed() < Duration::from_secs(300), "run {run} not ended: {v}");
    }
}

fn lines(p: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

struct Runner(std::path::PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

#[test]
fn p3_start_run_from_run_and_max_steps() {
    binaries();
    let dir = project("adr0734-p3");
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d, "--root", &d]);
    let a = m.call("start_run", json!({"case": "cases/cavity.toml", "max_steps": 12}))["structuredContent"]["run"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(wait_terminal(&mut m, &a)["state"], "finished");
    let run_a = dir.join("runs").join(&a);
    let res_a = lines(&run_a.join("residuals.jsonl"));
    assert_eq!(res_a.len(), 12, "max_steps 12");
    let rec_a: Value = serde_json::from_slice(&std::fs::read(run_a.join("run.json")).unwrap()).unwrap();
    assert_eq!(rec_a["settings"]["max_steps"], 12);
    // the launch record through run_status
    let st = m.call("run_status", json!({"run": a, "events": 0}))["structuredContent"].clone();
    assert_eq!(st["launch"]["argv"][1], "run");
    assert!(st["launch"]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "--max-steps"));
    assert!(st["launch"]["solver"]["build"]["exe_hash"]
        .as_str()
        .unwrap()
        .starts_with("fnv64:"));
    // refusals: nothing written
    let before = std::fs::read_dir(dir.join("runs")).unwrap().count();
    for (args, code) in [
        (json!({"from_run": a, "case": "cases/cavity.toml"}), -32602),
        (json!({"from_run": a, "set": ["solver.max_iterations=3"]}), -32602),
        (json!({"from_run": "r-20000101T000000-abcd"}), 1202),
        (
            json!({"case": "cases/cavity.toml", "continue_from": "r-20000101T000000-abcd"}),
            1202,
        ),
        (json!({"case": "cases/cavity.toml", "max_steps": 0}), -32602),
    ] {
        // a refusal by the tool is an error result; one by the schema (max_steps below its minimum) a JSON-RPC error
        let r = m.request("tools/call", json!({"name": "start_run", "arguments": args}));
        let got = r["error"]["code"].as_i64().unwrap_or_else(|| err_code(&r["result"]));
        eprintln!(
            "P3 refused {args}: {got} {}",
            r["error"]
                .get("message")
                .unwrap_or(&r["result"]["structuredContent"]["error"]["message"])
        );
        assert_eq!(got, code, "{args}: {r}");
    }
    assert_eq!(
        std::fs::read_dir(dir.join("runs")).unwrap().count(),
        before,
        "a refused start wrote a run"
    );
    // the rerun
    let r = m.call("start_run", json!({"from_run": a, "reason": "the same run again"}))["structuredContent"].clone();
    assert_eq!(r["rerun_of"], json!(a));
    assert_eq!(r["case"], "cases/cavity.toml");
    let b = r["run"].as_str().unwrap().to_string();
    assert_eq!(wait_terminal(&mut m, &b)["state"], "finished");
    let run_b = dir.join("runs").join(&b);
    let rec_b: Value = serde_json::from_slice(&std::fs::read(run_b.join("run.json")).unwrap()).unwrap();
    assert_eq!(rec_b["rerun_of"], json!(a));
    assert_eq!(rec_b["settings"], rec_a["settings"]);
    assert_eq!(rec_b["case_dir"], rec_a["case_dir"]);
    assert_eq!(
        lines(&run_b.join("residuals.jsonl")),
        res_a,
        "the rerun's residuals differ"
    );
    assert_eq!(
        std::fs::read(run_b.join("fields/store.rts")).unwrap(),
        std::fs::read(run_a.join("fields/store.rts")).unwrap()
    );
    // the journal names the rerun
    let j: Vec<Value> = lines(&dir.join(".rcfd/edits.jsonl"));
    let sub = j.iter().rfind(|l| l["kind"] == "run_submitted").unwrap();
    assert_eq!(
        (sub["run"].clone(), sub["rerun_of"].clone()),
        (json!(b), json!(a)),
        "{sub}"
    );
    assert_eq!(sub["reason"], "the same run again");
    eprintln!("P3: {a} (12 steps) rerun as {b}: residuals and store equal; journal {sub}");
}
