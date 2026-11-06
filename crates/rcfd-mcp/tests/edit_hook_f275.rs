//! ADR 0195 C5-K and C5-R (ADR 0173 D5 (b)): the plugin's edit hook, `rcfd-mcp --edit-hook`. C5-K feeds the binary
//! inputs shaped as Claude Code 2.1.319 gave them to a probe hook (ADR 0195, "The hook contract") and checks the plugin's
//! `hooks/hooks.json`; C5-R (ignored: it spends a Claude Code turn) runs a real `claude -p` session with the plugin.

mod common;

use common::*;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The hook's answer: exit code, stdout, stderr.
fn hook(input: &str, arg: Option<&Path>, env: Option<&Path>) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"));
    cmd.arg("--edit-hook");
    if let Some(a) = arg {
        cmd.arg(a);
    }
    cmd.env_remove("CLAUDE_PROJECT_DIR");
    if let Some(e) = env {
        cmd.env("CLAUDE_PROJECT_DIR", e);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rcfd-mcp starts");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// A `PreToolUse` input with every key the probe saw (ADR 0195).
fn input(cwd: &Path, tool: &str, tool_input: Value) -> String {
    json!({
        "session_id": "5d6c97b4-e0c5-4582-91a1-d3a0ba4913d9",
        "transcript_path": cwd.join("transcript.jsonl").to_string_lossy(),
        "cwd": cwd.to_string_lossy(),
        "prompt_id": "943120f6-83a3-4084-b171-ed3ab9843d13",
        "permission_mode": "acceptEdits",
        "effort": {"level": "medium"},
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": tool_input,
        "tool_use_id": "toolu_01DgmzcYqCk5s6KPor1y176t"
    })
    .to_string()
}

fn write(path: &str) -> Value {
    json!({"file_path": path, "content": "x = 1"})
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn dir_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// C5-K: refused, allowed and unreadable inputs, and the plugin's hook file.
#[test]
fn c5k_the_hook_refuses_the_guarded_folders_only() {
    let root = project("c5k");
    let sep = std::path::MAIN_SEPARATOR;
    let at = |rel: &str| s(&root.join(rel.replace('/', &sep.to_string())));
    let r = Some(root.as_path());
    // (label, input, argument, environment)
    let mut refused: Vec<(String, String, Option<&Path>, Option<&Path>)> = vec![
        (
            "Write cases/new.toml".into(),
            input(&root, "Write", write(&at("cases/new.toml"))),
            r,
            None,
        ),
        (
            "Edit cases/cavity.toml".into(),
            input(
                &root,
                "Edit",
                json!({"file_path": at("cases/cavity.toml"), "old_string": "nu = 0.01", "new_string": "nu = 0.02", "replace_all": false}),
            ),
            r,
            None,
        ),
        (
            "Write runs/<id>/run.json".into(),
            input(&root, "Write", write(&at("runs/r-20260101T000000-0000/run.json"))),
            r,
            None,
        ),
        (
            "Write designs/s/study.json".into(),
            input(&root, "Write", write(&at("designs/s/study.json"))),
            r,
            None,
        ),
        (
            "NotebookEdit cases/n.ipynb".into(),
            input(
                &root,
                "NotebookEdit",
                json!({"notebook_path": at("cases/n.ipynb"), "new_source": "x"}),
            ),
            r,
            None,
        ),
        (
            "MultiEdit cases/cavity.toml".into(),
            input(
                &root,
                "MultiEdit",
                json!({"file_path": at("cases/cavity.toml"), "edits": [{"old_string": "a", "new_string": "b"}]}),
            ),
            r,
            None,
        ),
        (
            "Write with / separators".into(),
            input(&root, "Write", write(&at("cases/new.toml").replace('\\', "/"))),
            r,
            None,
        ),
        (
            "Write notes/../cases/a.toml".into(),
            input(
                &root,
                "Write",
                write(&format!("{}{sep}notes{sep}..{sep}cases{sep}a.toml", s(&root))),
            ),
            r,
            None,
        ),
        (
            "Write cases/a.toml relative".into(),
            input(&root, "Write", write("cases/a.toml")),
            r,
            None,
        ),
        (
            "Write cases/new.toml, the folder in CLAUDE_PROJECT_DIR".into(),
            input(&root, "Write", write(&at("cases/new.toml"))),
            None,
            r,
        ),
        (
            "Write cases/new.toml, the folder as cwd".into(),
            input(&root, "Write", write(&at("cases/new.toml"))),
            None,
            None,
        ),
    ];
    let link = root.join("notes").join("link");
    let linked = dir_link(&root.join("cases"), &link);
    if linked {
        refused.push((
            "Write notes/link/a.toml (a link to cases)".into(),
            input(&root, "Write", write(&at("notes/link/a.toml"))),
            r,
            None,
        ));
    } else {
        eprintln!("C5-K: skipped the link input: the directory link could not be made");
    }
    if cfg!(windows) {
        refused.push((
            r"Write CASES\New.Toml".into(),
            input(&root, "Write", write(&at("CASES/New.Toml"))),
            r,
            None,
        ));
        refused.push((
            r"Write cases.\a.toml".into(),
            input(&root, "Write", write(&format!("{}{sep}cases.{sep}a.toml", s(&root)))),
            r,
            None,
        ));
    }
    let before = std::fs::read(root.join("cases/cavity.toml")).unwrap();
    for (label, inp, arg, env) in &refused {
        let (code, out, err) = hook(inp, *arg, *env);
        assert_eq!(code, 2, "{label}: {err}");
        assert!(out.is_empty(), "{label}: stdout {out}");
        assert!(
            err.contains("edit_case") && err.contains("new_case") && err.starts_with("rcfd: "),
            "{label}: {err}"
        );
    }
    let first = hook(&refused[0].1, Some(&root), None).2;
    assert!(first.contains("Write of cases/new.toml refused"), "{first}");
    eprintln!("C5-K refused message: {}", first.trim());
    // allowed
    let outside = scratch("c5k-outside").join("a.toml");
    let plain = scratch("c5k-plain");
    std::fs::create_dir_all(plain.join("cases")).unwrap();
    let allowed: Vec<(&str, String, &Path)> = vec![
        (
            "Write notes/a.md",
            input(&root, "Write", write(&at("notes/a.md"))),
            &root,
        ),
        (
            "Write meshes/a.msh",
            input(&root, "Write", write(&at("meshes/a.msh"))),
            &root,
        ),
        ("Write cases.md", input(&root, "Write", write(&at("cases.md"))), &root),
        (
            "Write casesx/a.toml",
            input(&root, "Write", write(&at("casesx/a.toml"))),
            &root,
        ),
        ("Write README.md", input(&root, "Write", write(&at("README.md"))), &root),
        (
            "Write cases/../notes/a.md",
            input(
                &root,
                "Write",
                write(&format!("{}{sep}cases{sep}..{sep}notes{sep}a.md", s(&root))),
            ),
            &root,
        ),
        (
            "Write outside the project",
            input(&root, "Write", write(&s(&outside))),
            &root,
        ),
        (
            "Read cases/cavity.toml",
            input(&root, "Read", json!({"file_path": at("cases/cavity.toml")})),
            &root,
        ),
        (
            "Bash",
            input(
                &root,
                "Bash",
                json!({"command": "echo x > cases/a.toml", "description": "d"}),
            ),
            &root,
        ),
        (
            "Write cases/new.toml in a folder that is no rcfd project",
            input(&plain, "Write", write(&s(&plain.join("cases").join("new.toml")))),
            &plain,
        ),
    ];
    for (label, inp, arg) in &allowed {
        let (code, out, err) = hook(inp, Some(arg), None);
        assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""), "{label}");
    }
    // unreadable: a non-blocking error
    for (label, inp) in [
        ("not JSON", "this is not json".to_string()),
        (
            "a Write without file_path",
            input(&root, "Write", json!({"content": "x"})),
        ),
    ] {
        let (code, out, err) = hook(&inp, Some(&root), None);
        assert_eq!(code, 1, "{label}");
        assert!(out.is_empty() && !err.trim().is_empty(), "{label}: {out} / {err}");
    }
    assert_eq!(std::fs::read(root.join("cases/cavity.toml")).unwrap(), before);
    eprintln!(
        "C5-K: {} refused (exit 2), {} allowed (exit 0), 2 not read (exit 1){}",
        refused.len(),
        allowed.len(),
        if linked { "" } else { "; the link input skipped" }
    );
    // the plugin's hook file
    let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients/claude-plugin");
    let h: Value = serde_json::from_str(&std::fs::read_to_string(plugin.join("hooks/hooks.json")).unwrap()).unwrap();
    let pre = h["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert_eq!(h["hooks"].as_object().unwrap().len(), 1);
    assert_eq!(pre[0]["matcher"], "Edit|Write|MultiEdit|NotebookEdit");
    let hs = pre[0]["hooks"].as_array().unwrap();
    assert_eq!(hs.len(), 1);
    assert_eq!(
        (hs[0]["type"].clone(), hs[0]["command"].clone(), hs[0]["args"].clone()),
        (
            json!("command"),
            json!("rcfd-mcp"),
            json!(["--edit-hook", "${CLAUDE_PROJECT_DIR}"])
        )
    );
}

/// A tool use of a transcript: id, tool name, input.
type Use = (String, String, Value);
/// A tool result: the tool use id, whether it is an error, its text.
type Outcome = (String, bool, String);

/// The tool calls of a stream-json transcript and their results.
fn calls(stream: &str) -> (Vec<Use>, Vec<Outcome>) {
    let (mut uses, mut results) = (Vec::new(), Vec::new());
    for l in stream.lines() {
        let Ok(v) = serde_json::from_str::<Value>(l) else {
            continue;
        };
        for c in v["message"]["content"].as_array().into_iter().flatten() {
            match c["type"].as_str() {
                Some("tool_use") => uses.push((
                    c["id"].as_str().unwrap_or_default().to_string(),
                    c["name"].as_str().unwrap_or_default().to_string(),
                    c["input"].clone(),
                )),
                Some("tool_result") => {
                    let text = match &c["content"] {
                        Value::String(t) => t.clone(),
                        other => other.to_string(),
                    };
                    results.push((
                        c["tool_use_id"].as_str().unwrap_or_default().to_string(),
                        c["is_error"] == true,
                        text,
                    ))
                }
                _ => {}
            }
        }
    }
    (uses, results)
}

/// C5-R: a real Claude Code session with the plugin refuses a Write and an Edit under cases/ and writes notes/.
#[test]
#[ignore = "spends a Claude Code turn: needs a logged-in claude on PATH; run by hand with --ignored (ADR 0195 C5-R)"]
fn c5r_a_real_session_refuses_case_edits() {
    let root = project("c5r");
    let before = std::fs::read(root.join("cases/cavity.toml")).unwrap();
    let logs = scratch("c5r-logs");
    let debug = logs.join("debug.txt");
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_rcfd-mcp"));
    let mut paths = vec![bin.parent().unwrap().to_path_buf()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    let path = std::env::join_paths(paths).unwrap();
    let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients/claude-plugin");
    let prompt = "Do exactly these tool calls, in order, and nothing else. (1) Use the Write tool to create cases/new.toml \
                  with the content 'x = 1'. (2) Use the Read tool on cases/cavity.toml, then use the Edit tool on it to \
                  replace 'nu = 0.01' with 'nu = 0.02'. (3) Use the Write tool to create notes/ok.md with the content 'ok'. \
                  Then report the result of each call in one line, quoting any error message.";
    let o = Command::new("claude")
        .arg("-p")
        .arg(prompt)
        .arg("--plugin-dir")
        .arg(&plugin)
        .args([
            "--permission-mode",
            "acceptEdits",
            "--output-format",
            "stream-json",
            "--verbose",
        ])
        .arg("--debug-file")
        .arg(&debug)
        .current_dir(&root)
        .env("PATH", &path)
        .env_remove("CLAUDE_PROJECT_DIR")
        .stdin(Stdio::null())
        .output()
        .expect("claude runs (ADR 0195 C5-R needs a logged-in claude on PATH)");
    let stream = String::from_utf8_lossy(&o.stdout).into_owned();
    std::fs::write(logs.join("stream.jsonl"), &stream).unwrap();
    assert!(
        o.status.success(),
        "claude exit {:?}: {}",
        o.status,
        String::from_utf8_lossy(&o.stderr)
    );
    let version = stream
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["subtype"] == "init")
        .map(|v| v["claude_code_version"].clone())
        .unwrap_or(Value::Null);
    let (uses, results) = calls(&stream);
    let result_of = |pred: &dyn Fn(&str, &Value) -> bool| {
        let id = &uses
            .iter()
            .find(|(_, n, i)| pred(n, i))
            .unwrap_or_else(|| panic!("the model did not make the call; tool uses: {uses:?}"))
            .0;
        results.iter().find(|r| &r.0 == id).cloned().expect("a result")
    };
    let file = |i: &Value| i["file_path"].as_str().unwrap_or_default().replace('\\', "/");
    let w = result_of(&|n, i| n == "Write" && file(i).ends_with("cases/new.toml"));
    let e = result_of(&|n, i| n == "Edit" && file(i).ends_with("cases/cavity.toml"));
    let ok = result_of(&|n, i| n == "Write" && file(i).ends_with("notes/ok.md"));
    for (label, r) in [("Write cases/new.toml", &w), ("Edit cases/cavity.toml", &e)] {
        assert!(r.1, "{label} was not refused: {}", r.2);
        assert!(
            r.2.contains("edit_case") && r.2.contains("new_case"),
            "{label}: {}",
            r.2
        );
    }
    assert!(!ok.1, "notes/ok.md: {}", ok.2);
    assert!(!root.join("cases/new.toml").exists());
    assert_eq!(std::fs::read(root.join("cases/cavity.toml")).unwrap(), before);
    assert!(root.join("notes/ok.md").is_file());
    let log = std::fs::read_to_string(&debug).unwrap_or_default();
    let status2: Vec<&str> = log
        .lines()
        .filter(|l| {
            l.contains("Hooks: PreToolUse:")
                && l.contains("rcfd-mcp --edit-hook")
                && l.contains("finished with status 2")
        })
        .collect();
    for l in &status2 {
        eprintln!("C5-R log: {l}");
    }
    assert_eq!(status2.len(), 2, "hook lines with status 2");
    eprintln!(
        "C5-R: Claude Code {version}; Write and Edit under cases/ refused ({} / {}), notes/ok.md written; result texts:\n  {}\n  {}",
        w.1,
        e.1,
        w.2.replace('\n', " "),
        e.2.replace('\n', " ")
    );
}
