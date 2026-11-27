//! ADR 0841 (F-1001), D3: with `--project`, every tool of the live `tools/list` of the `rcfd-mcp` binary has its name in
//! the project golden (`golden/tools_list_project_f270.json`), and every entry the golden holds after the 52 of 7c960bb
//! (the 3 canvas tools of ADR 0839 and the 19 tools of ADR 0841) is the live entry of its name, byte for byte. The rest
//! of the list is held by `project_list_f270.rs`.

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("golden/tools_list_project_f270.json");

#[test]
fn every_live_tool_is_in_the_golden_and_every_added_entry_is_live() {
    let dir = std::env::temp_dir().join(format!("rcfd-mcp-f1001-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    rcfd_project::Project::create(&dir, "f1001", "test").unwrap();
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
    let live: Value = serde_json::from_str(got).unwrap();
    let gold: Value = serde_json::from_str(GOLDEN).unwrap();
    assert_eq!(serde_json::to_string(&live).unwrap(), got);
    assert_eq!(serde_json::to_string(&gold).unwrap(), GOLDEN);
    let (live, gold) = (live["tools"].as_array().unwrap(), gold["tools"].as_array().unwrap());
    for t in live {
        assert!(
            gold.iter().any(|g| g["name"] == t["name"]),
            "{} is not in the golden",
            t["name"]
        );
    }
    assert!(gold.len() > 52, "the golden holds no added entry");
    for g in &gold[52..] {
        let l = live
            .iter()
            .find(|t| t["name"] == g["name"])
            .unwrap_or_else(|| panic!("{} is not listed", g["name"]));
        assert_eq!(l.to_string(), g.to_string(), "{} differs from the golden", g["name"]);
    }
    eprintln!(
        "F-1001: {} live tools, golden {} entries, {} added compared",
        live.len(),
        gold.len(),
        gold.len() - 52
    );
}
