//! ADR 0739 (F-848 phase 1, Q6 (a)): `list_sites` lists the user's sites file and starts nothing; `start_run {site}`
//! is refused for a site without `assistant = true` (and for an unknown one) before anything is written or sent; on a
//! site with `assistant = true` (the exec fake of the tests) the run is queued there, its `run.json` names the site, and
//! it finishes with its results copied back. One test: the sites file is named by environment variables the server
//! inherits.

mod common;

use common::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

fn msg(r: &Value) -> String {
    assert_eq!(r["isError"], true, "{r}");
    r["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Shuts down the runners this test made (the project's and the site mirror's), also when it fails.
struct Cleanup(Vec<PathBuf>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let runner = binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let mut dirs = self.0.clone();
        if let Ok(rd) = std::fs::read_dir(self.0[1].join("projects")) {
            dirs.extend(rd.flatten().map(|e| e.path()));
        }
        for d in dirs {
            let out = std::fs::File::create(self.0[0].join("shutdown.txt")).unwrap();
            let _ = std::process::Command::new(&runner)
                .args(["shutdown", "--quiet", "--project"])
                .arg(&d)
                .stdin(std::process::Stdio::null())
                .stdout(out.try_clone().unwrap())
                .stderr(out)
                .status();
        }
    }
}

#[test]
fn list_sites_and_start_run_on_a_site_are_gated_by_assistant() {
    let mcp = binaries();
    let engine = mcp.parent().unwrap().to_string_lossy().replace('\\', "/");
    let dir = project("adr0739-sites");
    let root = dir.parent().unwrap().to_path_buf();
    let work = root.join("site");
    let sites = root.join("sites.toml");
    let w = work.to_string_lossy().replace('\\', "/");
    std::fs::write(
        &sites,
        format!(
            "[site.open]\nkind = \"exec\"\nengine = \"{engine}\"\nwork = \"{w}\"\nassistant = true\nconfirmed = \"exec\"\n\n\
             [site.closed]\nkind = \"exec\"\nengine = \"{engine}\"\nwork = \"{w}\"\nconfirmed = \"exec\"\n"
        ),
    )
    .unwrap();
    // the server inherits these (one test in this binary, so no other thread reads the environment)
    std::env::set_var("RCFD_SITES_FILE", &sites);
    std::env::set_var("RCFD_TEST_EXEC_SITES", "1");
    let _cleanup = Cleanup(vec![root.clone(), work.clone(), dir.clone()]);
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let _ = m.request("initialize", json!({}));

    // list_sites: both, with assistant and confirmed; nothing started (no site folder)
    let r = m.call("list_sites", json!({}));
    let l = r["structuredContent"].clone();
    assert_eq!(l["file"], sites.display().to_string(), "{l}");
    let names: Vec<&str> = l["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["closed", "open"], "{l}");
    let assistant: Vec<bool> = l["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["assistant"].as_bool().unwrap())
        .collect();
    assert_eq!(assistant, [false, true]);
    assert!(!work.exists(), "list_sites started an agent");

    // refused: a site without assistant = true, and an unknown site; nothing written, nothing sent
    let before = files(&dir);
    let r = m.request(
        "tools/call",
        json!({"name": "start_run", "arguments": {"case": "cases/cavity.toml", "site": "closed"}}),
    );
    let e = msg(&r["result"]);
    assert!(
        e.contains("not open to assistants") && e.contains("assistant = true"),
        "{e}"
    );
    let r = m.request(
        "tools/call",
        json!({"name": "start_run", "arguments": {"case": "cases/cavity.toml", "site": "nowhere"}}),
    );
    assert!(msg(&r["result"]).contains("nowhere"), "{r}");
    let r = m.request(
        "tools/call",
        json!({"name": "start_run", "arguments": {"new_case": {"name": "x"}, "site": "closed"}}),
    );
    assert!(msg(&r["result"]).contains("not open to assistants"), "{r}");
    assert_eq!(files(&dir), before, "a refused start_run wrote into the project");
    assert!(!work.exists(), "a refused start_run reached the site");

    // the open site: queued there, run.json names it, finished with its results copied back
    let r = m.call("start_run", json!({"case": "cases/cavity.toml", "site": "open"}));
    let st = r["structuredContent"].clone();
    assert_eq!(st["site"], "open", "{r}");
    let id = st["run"].as_str().unwrap().to_string();
    let rec: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("runs").join(&id).join("run.json")).unwrap()).unwrap();
    assert_eq!(rec["site"], "open", "{rec}");
    let t0 = Instant::now();
    loop {
        let w = m.call("wait_run", json!({"run": id, "timeout_s": 30}));
        if w["structuredContent"]["terminal"] == true || t0.elapsed() > Duration::from_secs(300) {
            assert_eq!(w["structuredContent"]["state"], "finished", "{w}");
            break;
        }
    }
    let run = dir.join("runs").join(&id);
    assert!(run.join("site.json").is_file());
    let log = std::fs::read_to_string(run.join("site.jsonl")).unwrap();
    for k in ["\"uploaded\"", "\"submitted\"", "\"downloaded\""] {
        assert!(log.contains(k), "site.jsonl has no {k}: {log}");
    }
    assert!(
        std::fs::read_to_string(run.join("residuals.jsonl"))
            .unwrap()
            .lines()
            .count()
            > 1
    );
    drop(m);
    eprintln!("ADR 0739 MCP: list_sites 2 sites; closed and unknown refused before any write; {id} finished on open");
}
