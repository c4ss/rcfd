//! F-096 (ADR 0089): the MCP tutorials in docs/tutorials are executed against the real `rcfd-mcp` binary, started as
//! `rcfd-mcp --root .` in the tutorial's working directory, so they cannot rot.

#[path = "../../rcfd-cli/tests/tutorial/mod.rs"]
mod tutorial;

use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tutorial::{Block, Tag};

/// Pipes the JSON-RPC lines of a block into one server process; every request needs an error-free response.
fn exec(b: &Block, dir: &Path) -> Result<String, String> {
    if b.lang != "jsonl" {
        return Err(format!("MCP tutorials execute jsonl blocks only, got '{}'", b.lang));
    }
    let reqs: Vec<&str> = b.text.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .args(["--root", "."])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().unwrap();
    for r in &reqs {
        writeln!(stdin, "{r}").map_err(|e| e.to_string())?;
    }
    drop(stdin);
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    let resps: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).map_err(|e| format!("bad response {l}: {e}")))
        .collect::<Result<_, _>>()?;
    if resps.len() != reqs.len() {
        return Err(format!("{} requests, {} responses:\n{out}", reqs.len(), resps.len()));
    }
    for r in &resps {
        if r.get("error").is_some() || r["result"]["isError"] == true {
            return Err(format!("error response: {r}"));
        }
    }
    let Tag::Run { exit, .. } = b.tag else { unreachable!() };
    if o.status.code() != Some(exit) {
        return Err(format!("rcfd-mcp exited with {:?}", o.status.code()));
    }
    Ok(out)
}

fn run_all(slow: bool) {
    let t0 = std::time::Instant::now();
    let mut names = Vec::new();
    for p in tutorial::tutorials() {
        let md = std::fs::read_to_string(&p).unwrap();
        if !tutorial::is_mcp(&md) {
            continue;
        }
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let dir = tutorial::workdir(&format!("mcp_{name}"));
        let r = tutorial::run(&md, &dir, slow, &mut exec).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        println!(
            "{name}: {} blocks executed, {} slow skipped, {} not executed, {} outputs checked",
            r.executed, r.skipped_slow, r.not_executed, r.outputs_checked
        );
        assert!(r.executed > 0, "{name}: no executed block (T3)");
        let _ = std::fs::remove_dir_all(&dir);
        names.push(name);
    }
    println!("{} MCP tutorials in {:.1} s", names.len(), t0.elapsed().as_secs_f64());
    assert!(names.iter().any(|n| n == "mcp-design-loop"));
}

/// T3, T4, T7, R1: the default (fast) blocks of every MCP tutorial.
#[test]
fn mcp_tutorials_run() {
    run_all(false);
}

/// T8: the run-slow blocks too.
#[test]
#[ignore = "slow: runs every run-slow block of the MCP tutorials (ADR 0089 T8)"]
fn mcp_tutorials_run_slow() {
    run_all(true);
}

/// T6: a request that fails makes the runner fail.
#[test]
fn runner_detects_error_responses() {
    let dir = tutorial::workdir("mcp_mutation");
    let good = "```jsonl run\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"list_cases\",\"arguments\":{}}}\n```\n";
    tutorial::run(good, &dir, false, &mut exec).expect("the good fixture passes");
    for bad in [
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"no_such_tool\",\"arguments\":{}}}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"check_mesh\",\"arguments\":{}}}",
    ] {
        let md = format!("```jsonl run\n{bad}\n```\n");
        assert!(tutorial::run(&md, &dir, false, &mut exec).is_err(), "{bad} not detected");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
