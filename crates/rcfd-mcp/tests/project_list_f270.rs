//! ADR 0185 A2: with `--project`, `tools/list` of the `rcfd-mcp` binary is byte-identical to the golden recorded from the
//! binary of main 7c960bb, before the project tools moved onto `rcfd-workspace` (`golden/tools_list_project_f270.json`,
//! never re-recorded). The tool list without `--project` is checked by `default_f1.rs`.

use std::io::Write;
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("golden/tools_list_project_f270.json");

#[test]
fn a2_tools_list_with_project_is_byte_identical_to_the_golden() {
    let dir = std::env::temp_dir().join(format!("rcfd-mcp-f270-a2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    rcfd_project::Project::create(&dir, "f270 a2", "test").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .arg("--project")
        .arg(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("rcfd-mcp starts");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{{}}}}"#
    )
    .unwrap();
    drop(stdin);
    let o = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(o.status.success(), "rcfd-mcp exit {:?}", o.status);
    let out = String::from_utf8(o.stdout).unwrap();
    let line = out.lines().next().expect("one response line");
    let got = line
        .strip_prefix(r#"{"id":1,"jsonrpc":"2.0","result":"#)
        .and_then(|b| b.strip_suffix('}'))
        .unwrap_or_else(|| panic!("unexpected response {}", &line[..line.len().min(80)]));
    let tools = serde_json::from_str::<serde_json::Value>(got).unwrap()["tools"]
        .as_array()
        .map_or(0, Vec::len);
    eprintln!("A2: {tools} tools, {} bytes, golden {} bytes", got.len(), GOLDEN.len());
    assert_eq!(tools, 50, "33 session tools and 17 project tools");
    assert!(
        got == GOLDEN,
        "tools/list with --project differs from the golden ({} vs {} bytes)",
        got.len(),
        GOLDEN.len()
    );
}
