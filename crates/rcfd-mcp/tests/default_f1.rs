//! ADR 0135 F1: without `--project` the server is unchanged. `tools/list` is byte-identical to the golden recorded from
//! the `rcfd-mcp` binary of main 15b00ef (amendment 1 item 1; `crates/rcfd-mcp` equals 1248a76 there), and `initialize`
//! answers as before. The golden is never re-recorded.

use std::io::Write;
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("golden/tools_list_default.json");

/// Response lines of one `rcfd-mcp` process fed `reqs`, started with `args`.
fn exchange(args: &[&str], reqs: &[&str]) -> Vec<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("rcfd-mcp starts");
    let mut stdin = child.stdin.take().unwrap();
    for r in reqs {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "rcfd-mcp exit {:?}", o.status);
    String::from_utf8(o.stdout).unwrap().lines().map(String::from).collect()
}

fn result_text(line: &str) -> &str {
    let prefix = r#"{"id":1,"jsonrpc":"2.0","result":"#;
    line.strip_prefix(prefix)
        .and_then(|b| b.strip_suffix('}'))
        .unwrap_or_else(|| panic!("unexpected response {}", &line[..line.len().min(80)]))
}

#[test]
fn f1_tools_list_without_project_is_byte_identical_to_the_golden() {
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    for args in [&[][..], &["--root", "."][..], &["--read-only"][..]] {
        let lines = exchange(args, &[req]);
        assert_eq!(lines.len(), 1, "{args:?}");
        let got = result_text(&lines[0]);
        assert!(
            got == GOLDEN,
            "{args:?}: tools/list differs from the golden ({} vs {} bytes)",
            got.len(),
            GOLDEN.len()
        );
    }
    let v: serde_json::Value = serde_json::from_str(GOLDEN).unwrap();
    eprintln!(
        "F1: {} tools, {} bytes, byte-identical",
        v["tools"].as_array().unwrap().len(),
        GOLDEN.len()
    );
}

#[test]
fn f1_initialize_without_project_is_unchanged() {
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#;
    let lines = exchange(&[], &[req]);
    let v: serde_json::Value = serde_json::from_str(result_text(&lines[0])).unwrap();
    assert_eq!(v["capabilities"], serde_json::json!({"tools": {"listChanged": false}}));
    assert_eq!(v["serverInfo"]["name"], "rcfd");
    assert_eq!(
        v["instructions"],
        "Clean-room open source CFD solver. create_case, adjust settings, check_mesh, run, then inspect residuals, fields and images."
    );
}
