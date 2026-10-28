#![allow(dead_code)]
//! Shared helpers of the ADR 0135 project-tool checks.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::policy::Policy;
use rcfd_mcp::project::ProjectTools;
use rcfd_mcp::McpServer;
use rcfd_project::Project;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// The 12 x 12 lid-driven cavity with the `lid_drag` report, `steps` iterations (tolerances too small to stop it
/// earlier) and the field store every 10 steps.
pub fn cavity(n: usize, steps: u64) -> String {
    format!(
        r#"title = "w48f cavity" # a comment edit_case keeps
[mesh]
generator = "cavity_2d"
nx = {n}
ny = {n}
[physics]
nu = 0.01   # viscosity
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = {steps}
tolerance_u = 1e-14
tolerance_p = 1e-14
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
[output]
store_interval = 10
"#
    )
}

/// A fresh empty directory under the temp area.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-mcp-w48f-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A project folder with `cases/cavity.toml`.
pub fn project(name: &str) -> PathBuf {
    let dir = scratch(name).join("proj");
    Project::create(&dir, name, "test").unwrap();
    std::fs::write(dir.join("cases/cavity.toml"), cavity(12, 30)).unwrap();
    dir
}

/// An in-process server with the project tools of `dir`.
pub fn server(dir: &Path, policy: Policy) -> McpServer {
    let cfg = ServiceConfig {
        root: Some(dir.to_path_buf()),
        ..Default::default()
    };
    McpServer::new(Service::new(cfg), policy).with_project(ProjectTools::open(dir).unwrap())
}

/// `tools/call` through `handle`; the JSON-RPC response.
pub fn rpc_call(s: &McpServer, tool: &str, args: Value) -> Value {
    s.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}}))
        .unwrap()
}

/// A tool result (asserting there was no JSON-RPC error).
pub fn call(s: &McpServer, tool: &str, args: Value) -> Value {
    let r = rpc_call(s, tool, args);
    assert!(r.get("error").is_none(), "{tool}: {r}");
    r["result"].clone()
}

/// `structuredContent` of a successful tool result.
pub fn ok(r: &Value) -> Value {
    assert_eq!(r["isError"], false, "{r}");
    r["structuredContent"].clone()
}

/// The error code of a failed tool result.
pub fn err_code(r: &Value) -> i64 {
    assert_eq!(r["isError"], true, "{r}");
    r["structuredContent"]["error"]["code"].as_i64().unwrap()
}

/// ADR 0135 F8: bytes of every `content[*].text` and `content[*].data`.
pub fn result_bytes(r: &Value) -> usize {
    r["content"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| c["text"].as_str().map_or(0, str::len) + c["data"].as_str().map_or(0, str::len))
        .sum()
}

fn matches(pattern: &str, path: &str) -> bool {
    let (p, q): (Vec<&str>, Vec<&str>) = (pattern.split('/').collect(), path.split('/').collect());
    p.len() == q.len() && p.iter().zip(&q).all(|(a, b)| *a == "*" || a == b)
}

/// ADR 0135 amendment 1 item 3 and amendment 2: leaves by type name, arrays by their first element, `opaque` paths as
/// `object`, `maps` paths as `{"*": <shape of the first value>}` (paths like `/runs/*/metrics`).
pub fn shape(v: &Value, opaque: &[&str], maps: &[&str]) -> Value {
    fn go(v: &Value, path: &str, opaque: &[&str], maps: &[&str]) -> Value {
        if opaque.iter().any(|p| matches(p, path)) {
            return json!("object");
        }
        match v {
            Value::Null => json!("null"),
            Value::Bool(_) => json!("bool"),
            Value::Number(_) => json!("number"),
            Value::String(_) => json!("string"),
            Value::Array(a) => match a.first() {
                None => json!([]),
                Some(x) => json!([go(x, &format!("{path}/0"), opaque, maps)]),
            },
            Value::Object(o) if maps.iter().any(|p| matches(p, path)) => match o.iter().next() {
                None => json!({}),
                Some((k, x)) => json!({"*": go(x, &format!("{path}/{k}"), opaque, maps)}),
            },
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), go(x, &format!("{path}/{k}"), opaque, maps)))
                    .collect(),
            ),
        }
    }
    go(v, "", opaque, maps)
}

/// The `rcfd-mcp` binary and the `rcfd` and `rcfd-runner` binaries next to it (W48-13 (a) install layout).
pub fn binaries() -> PathBuf {
    let mcp = PathBuf::from(env!("CARGO_BIN_EXE_rcfd-mcp"));
    for b in ["rcfd", "rcfd-runner"] {
        let p = mcp.with_file_name(format!("{b}{}", std::env::consts::EXE_SUFFIX));
        assert!(
            p.is_file(),
            "{} missing: the project run tools need it next to rcfd-mcp (cargo build -p rcfd-cli -p rcfd-runner)",
            p.display()
        );
    }
    mcp
}

/// One `rcfd-mcp` process spoken to over stdio.
pub struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    pub fn start(args: &[&str]) -> Mcp {
        Self::start_in(args, &std::env::current_dir().unwrap())
    }

    /// [`Mcp::start`] with the working folder `cwd`.
    pub fn start_in(args: &[&str], cwd: &Path) -> Mcp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("rcfd-mcp starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Mcp {
            child,
            stdin,
            stdout,
            next: 1,
        }
    }

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin.as_mut().unwrap(), "{req}").unwrap();
        self.stdin.as_mut().unwrap().flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
        assert_eq!(v["id"], id, "{v}");
        v
    }

    /// A tool result (no JSON-RPC error).
    pub fn call(&mut self, tool: &str, args: Value) -> Value {
        let r = self.request("tools/call", json!({"name": tool, "arguments": args}));
        assert!(r.get("error").is_none(), "{tool}: {r}");
        r["result"].clone()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

/// A field store written by the service's run path: the cavity of `n` x `n` cells run `steps` steps through
/// `run.headless` with `output.store_interval` set (the hook writes every 10 steps and the final state). Returns
/// the service, the session (holding the final state) and the store folder.
pub fn service_store(root: &Path, n: usize, steps: u64) -> (std::sync::Arc<Service>, String, PathBuf) {
    // the sandbox root must exist (the service resolves paths against its canonical form)
    std::fs::create_dir_all(root).unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.to_path_buf()),
        ..Default::default()
    });
    let s = svc.call("session.create", &json!({"toml": cavity(n, steps)})).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc
        .call(
            "run.headless",
            &json!({"session": s, "max_steps": steps, "out_dir": "out"}),
        )
        .unwrap();
    assert!(r["outcome"].is_object(), "{r}");
    let store = root.join("out/fields");
    assert!(store.join("store.rts").is_file(), "{}", store.display());
    (svc, s, store)
}

/// Copy a store folder into `runs/<id>/fields` of a run made with `rcfd-project` from `cases/cavity.toml`.
pub fn run_with_store(dir: &Path, store: &Path) -> String {
    let p = Project::open(dir).unwrap();
    let rec = p.create_run("cases/cavity.toml", None, "test", json!({})).unwrap();
    let dst = dir.join("runs").join(&rec.id).join("fields");
    std::fs::create_dir_all(&dst).unwrap();
    for e in std::fs::read_dir(store).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
    rec.id
}
