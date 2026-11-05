//! ADR 0185 A2: with `--project`, `tools/list` of the `rcfd-mcp` binary is byte-identical to the golden recorded from the
//! binary of main 7c960bb, before the project tools moved onto `rcfd-workspace` (`golden/tools_list_project_f270.json`,
//! never re-recorded). The tool list without `--project` is checked by `default_f1.rs`.
//!
//! ADR 0186 C-L restates it for F-271, which changes the list by design (G6, G7): the golden with exactly the registered
//! changes. `new_case` is added right after `write_note`; `start_run` gains the properties `new_case` and `store` and an
//! empty `required`; every other byte is the golden's.

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("golden/tools_list_project_f270.json");

#[test]
fn a2_tools_list_with_project_is_the_golden_with_the_f271_changes() {
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
    let live: Value = serde_json::from_str(got).unwrap();
    let gold: Value = serde_json::from_str(GOLDEN).unwrap();
    // the golden's own text is the serialisation of its value, so equal values are equal bytes
    assert_eq!(serde_json::to_string(&gold).unwrap(), GOLDEN);
    let (live, gold) = (live["tools"].as_array().unwrap(), gold["tools"].as_array().unwrap());
    eprintln!(
        "A2/C-L: {} tools, {} bytes, golden {} tools, {} bytes",
        live.len(),
        got.len(),
        gold.len(),
        GOLDEN.len()
    );
    assert_eq!(gold.len(), 50, "the golden: 33 session tools and 17 project tools");
    assert_eq!(live.len(), 51, "33 session tools and 18 project tools");
    let name = |t: &Value| t["name"].as_str().unwrap().to_string();
    // new_case: right after write_note, writes files
    let at = live
        .iter()
        .position(|t| t["name"] == "new_case")
        .expect("new_case listed");
    assert_eq!(live[at - 1]["name"], "write_note");
    assert_eq!(live[at]["_meta"]["rcfd/access"], "write_files");
    assert_eq!(live[at]["inputSchema"]["required"], json!(["name"]));
    // every other tool in the golden's order, byte-identical but start_run
    let rest: Vec<&Value> = live.iter().filter(|t| t["name"] != "new_case").collect();
    assert_eq!(
        rest.iter().map(|t| name(t)).collect::<Vec<_>>(),
        gold.iter().map(name).collect::<Vec<_>>()
    );
    for (l, g) in rest.iter().zip(gold) {
        if g["name"] != "start_run" {
            assert_eq!(l.to_string(), g.to_string(), "{} differs from the golden", name(g));
        }
    }
    // start_run: name, description, annotations, _meta and every golden property unchanged; required empty; exactly
    // new_case and store added
    let (l, g) = (
        rest.iter().find(|t| t["name"] == "start_run").unwrap(),
        gold.iter().find(|t| t["name"] == "start_run").unwrap(),
    );
    for k in ["name", "description", "annotations", "_meta"] {
        assert_eq!(l[k].to_string(), g[k].to_string(), "start_run {k}");
    }
    let (lp, gp) = (
        l["inputSchema"]["properties"].as_object().unwrap(),
        g["inputSchema"]["properties"].as_object().unwrap(),
    );
    for (k, v) in gp {
        assert_eq!(lp[k].to_string(), v.to_string(), "start_run property {k}");
    }
    let mut added: Vec<&str> = lp.keys().filter(|k| !gp.contains_key(*k)).map(String::as_str).collect();
    added.sort();
    assert_eq!(added, ["new_case", "store"]);
    assert_eq!(g["inputSchema"]["required"], json!(["case"]));
    assert_eq!(l["inputSchema"]["required"], json!([]));
    for k in ["type", "additionalProperties"] {
        assert_eq!(l["inputSchema"][k], g["inputSchema"][k], "start_run inputSchema {k}");
    }
    assert_eq!(
        l["inputSchema"].as_object().unwrap().len(),
        g["inputSchema"].as_object().unwrap().len()
    );
    // the whole text: the golden with these changes applied is the live text, byte for byte
    let mut want: Vec<Value> = gold.clone();
    let sr = want.iter().position(|t| t["name"] == "start_run").unwrap();
    for k in ["new_case", "store"] {
        want[sr]["inputSchema"]["properties"][k] = lp[k].clone();
    }
    want[sr]["inputSchema"]["required"] = json!([]);
    let wn = want.iter().position(|t| t["name"] == "write_note").unwrap();
    want.insert(wn + 1, live[at].clone());
    assert!(
        serde_json::to_string(&json!({"tools": want})).unwrap() == got,
        "tools/list with --project differs from the golden with the registered changes"
    );
}
