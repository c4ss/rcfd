//! ADR 0135 F1: without `--project` the server is unchanged. `tools/list` is byte-identical to the golden recorded from
//! the `rcfd-mcp` binary of main 15b00ef (amendment 1 item 1; `crates/rcfd-mcp` equals 1248a76 there), and `initialize`
//! answers as before. The golden is never re-recorded.
//!
//! ADR 0731 P3 (Q2 (a)) restates the list check for F-835: without `--project` the list is the golden with exactly one
//! tool added, `create_project`, as the last tool; every other byte is the golden's. The golden is not edited.

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
fn f1_tools_list_without_project_is_the_golden_plus_create_project() {
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let gold: serde_json::Value = serde_json::from_str(GOLDEN).unwrap();
    // the golden's text is the serialisation of its value, so equal values are equal bytes
    assert_eq!(serde_json::to_string(&gold).unwrap(), GOLDEN);
    let gold = gold["tools"].as_array().unwrap();
    assert_eq!(gold.len(), 35);
    for args in [&[][..], &["--root", "."][..], &["--read-only"][..]] {
        let lines = exchange(args, &[req]);
        assert_eq!(lines.len(), 1, "{args:?}");
        let got = result_text(&lines[0]);
        let live: serde_json::Value = serde_json::from_str(got).unwrap();
        assert_eq!(serde_json::to_string(&live).unwrap(), got);
        let live = live["tools"].as_array().unwrap();
        assert_eq!(
            live.len(),
            36,
            "{args:?}: the 35 tools of the golden and create_project"
        );
        // ADR 0731 P3: create_project last, writes files, takes path and an optional name
        let cp = &live[35];
        assert_eq!(cp["name"], "create_project");
        assert_eq!(cp["_meta"]["rcfd/access"], "write_files");
        assert_eq!(cp["_meta"]["rcfd/needsApproval"], true);
        assert_eq!(cp["inputSchema"]["required"], serde_json::json!(["path"]));
        assert_eq!(cp["inputSchema"]["additionalProperties"], false);
        let mut props: Vec<&String> = cp["inputSchema"]["properties"].as_object().unwrap().keys().collect();
        props.sort();
        assert_eq!(props, ["name", "path"]);
        // the whole text: the golden with create_project appended is the live text, byte for byte
        let mut want = gold.clone();
        want.push(cp.clone());
        assert!(
            serde_json::to_string(&serde_json::json!({"tools": want})).unwrap() == got,
            "{args:?}: tools/list differs from the golden with create_project ({} vs {} bytes)",
            got.len(),
            GOLDEN.len()
        );
    }
    eprintln!(
        "F1/ADR 0731: {} golden tools + create_project, golden {} bytes",
        gold.len(),
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
