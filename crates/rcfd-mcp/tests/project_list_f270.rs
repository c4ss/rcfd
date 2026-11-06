//! ADR 0185 A2: with `--project`, `tools/list` of the `rcfd-mcp` binary is byte-identical to the golden recorded from the
//! binary of main 7c960bb, before the project tools moved onto `rcfd-workspace` (`golden/tools_list_project_f270.json`,
//! never re-recorded). The tool list without `--project` is checked by `default_f1.rs`.
//!
//! ADR 0186 C-L restates it for F-271, which changes the list by design (G6, G7): the golden with exactly the registered
//! changes. `new_case` is added right after `write_note`; `start_run` gains the properties `new_case` and `store` and an
//! empty `required`; every other byte is the golden's.
//!
//! ADR 0187 C-L restates it again for F-272 (M1, M8): the four mesh tools `check_case_mesh`, `preview_mesh`,
//! `import_mesh` and `list_meshes` come right after `new_case`; without them the list is the F-271 one, byte for byte.
//!
//! ADR 0188 C-L restates it for F-273 (J4, J7): `read_feed` comes right after `list_changes`, and eight write tools gain
//! the property `reason`; without `read_feed` and those eight properties the list is the F-272 one, byte for byte.
//!
//! ADR 0195 C5-L restates it for F-275 (P1): with `--project` the 16 session tools of P1 are not listed; put back from the
//! list without `--project` (`golden/tools_list_default.json`, F1's golden) at their places, and with `read_feed`'s added
//! property `from_seq` (P7) removed, the list is the F-273 one.
//!
//! ADR 0196 B restates it for the class of `design_run_study` and `design_evaluate`, `execute` with `--project`: set back to
//! `write_files`, the list is the F-273 one. The goldens are not edited.

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

const GOLDEN: &str = include_str!("golden/tools_list_project_f270.json");

/// The list without `--project` (ADR 0135 F1, never re-recorded).
const DEFAULT: &str = include_str!("golden/tools_list_default.json");

/// ADR 0195 P1: the session tools not listed with `--project`.
const HIDDEN: [&str; 16] = [
    "create_case",
    "set_setting",
    "get_setting",
    "load_mesh",
    "check_mesh",
    "run",
    "status",
    "get_residuals",
    "get_field_stats",
    "export_vtk",
    "list_workflow_tasks",
    "create_workflow",
    "edit_workflow",
    "run_workflow_task",
    "run_headless",
    "render_image",
];

/// ADR 0187 M1: the mesh tools, in order, with their access class and required arguments.
const MESH_TOOLS: [(&str, &str, &[&str]); 4] = [
    ("check_case_mesh", "read", &["case"]),
    ("preview_mesh", "read", &["case"]),
    ("import_mesh", "write_files", &["path"]),
    ("list_meshes", "read", &[]),
];

/// ADR 0188 J4: the tools that take a `reason`.
const REASON_TOOLS: [&str; 8] = [
    "edit_case",
    "write_note",
    "new_case",
    "import_mesh",
    "start_run",
    "pause_run",
    "resume_run",
    "cancel_run",
];

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
    let live_value: Value = serde_json::from_str(got).unwrap();
    let gold: Value = serde_json::from_str(GOLDEN).unwrap();
    // the golden's own text is the serialisation of its value, so equal values are equal bytes; the same holds for the
    // live text
    assert_eq!(serde_json::to_string(&gold).unwrap(), GOLDEN);
    assert_eq!(serde_json::to_string(&live_value).unwrap(), got);
    let (live_f275, gold) = (
        live_value["tools"].as_array().unwrap(),
        gold["tools"].as_array().unwrap(),
    );
    eprintln!(
        "A2/C-L: {} tools, {} bytes, golden {} tools, {} bytes",
        live_f275.len(),
        got.len(),
        gold.len(),
        GOLDEN.len()
    );
    assert_eq!(gold.len(), 50, "the golden: 33 session tools and 17 project tools");
    // ADR 0195 C5-L: 17 session tools and 23 project tools; the 16 hidden ones put back from the list without --project
    // at their places give the F-273 list
    assert_eq!(live_f275.len(), 40, "17 session tools and 23 project tools");
    let default: Value = serde_json::from_str(DEFAULT).unwrap();
    let session = default["tools"].as_array().unwrap();
    assert_eq!(session.len(), 33);
    for t in live_f275 {
        assert!(
            !HIDDEN.contains(&t["name"].as_str().unwrap()),
            "{} is listed",
            t["name"]
        );
    }
    let mut kept = live_f275.iter();
    let mut live_f273: Vec<Value> = Vec::new();
    for s in session {
        if HIDDEN.contains(&s["name"].as_str().unwrap()) {
            live_f273.push(s.clone());
        } else {
            let k = kept.next().unwrap();
            assert_eq!(k["name"], s["name"], "the kept session tools in their order");
            live_f273.push(k.clone());
        }
    }
    live_f273.extend(kept.cloned());
    // ADR 0195 P7: read_feed's one added property, from_seq
    let rf = live_f273.iter_mut().find(|t| t["name"] == "read_feed").unwrap();
    let fs = rf["inputSchema"]["properties"]
        .as_object_mut()
        .unwrap()
        .remove("from_seq")
        .expect("read_feed has from_seq");
    assert_eq!(
        (fs["type"].clone(), fs["minimum"].clone()),
        (json!("integer"), json!(1))
    );
    // ADR 0196 B (registered edit E2): under --project these two have the class execute; the F-273 list had write_files.
    // ADR 0200 S9 (registered edit E2 there): design_run_operating_map too
    for n in ["design_run_study", "design_evaluate", "design_run_operating_map"] {
        let t = live_f273.iter_mut().find(|t| t["name"] == n).unwrap();
        assert_eq!(t["_meta"]["rcfd/access"], "execute", "{n}");
        t["_meta"]["rcfd/access"] = json!("write_files");
    }
    let live_f273 = &live_f273;
    assert_eq!(live_f273.len(), 56, "33 session tools and 23 project tools");
    // ADR 0188 C-L: read_feed right after list_changes, a read tool with no required argument
    let fp = live_f273
        .iter()
        .position(|t| t["name"] == "read_feed")
        .expect("read_feed listed");
    assert_eq!(live_f273[fp - 1]["name"], "list_changes");
    assert_eq!(live_f273[fp]["_meta"]["rcfd/access"], "read");
    assert_eq!(live_f273[fp]["inputSchema"]["required"], json!([]));
    assert_eq!(live_f273[fp]["inputSchema"]["additionalProperties"], false);
    // the eight tools of J4 gain exactly the property reason, the same in each; the F-272 list is the rest
    let mut reason: Option<Value> = None;
    let mut f272: Vec<Value> = Vec::new();
    for t in live_f273.iter().filter(|t| t["name"] != "read_feed") {
        let mut t = t.clone();
        let n = t["name"].as_str().unwrap().to_string();
        // (a session tool, design_ledger_append, has a reason property of its own: the byte comparison below covers it)
        if REASON_TOOLS.contains(&n.as_str()) {
            let r = t["inputSchema"]["properties"]
                .as_object_mut()
                .unwrap()
                .remove("reason")
                .unwrap_or_else(|| panic!("{n} has no reason"));
            assert_eq!(r["type"], "string", "{n}");
            assert_eq!(r["maxLength"], 500, "{n}");
            assert!(reason.as_ref().is_none_or(|x| *x == r), "{n}: another reason property");
            reason = Some(r);
        }
        f272.push(t);
    }
    let live = &f272;
    assert_eq!(live.len(), 55, "the F-272 list: 33 session tools and 22 project tools");
    let name = |t: &Value| t["name"].as_str().unwrap().to_string();
    // new_case: right after write_note, writes files
    let at = live
        .iter()
        .position(|t| t["name"] == "new_case")
        .expect("new_case listed");
    assert_eq!(live[at - 1]["name"], "write_note");
    assert_eq!(live[at]["_meta"]["rcfd/access"], "write_files");
    assert_eq!(live[at]["inputSchema"]["required"], json!(["name"]));
    // ADR 0187: the four mesh tools right after new_case
    for (k, (n, access, required)) in MESH_TOOLS.iter().enumerate() {
        let t = &live[at + 1 + k];
        assert_eq!(t["name"], *n);
        assert_eq!(t["_meta"]["rcfd/access"], *access, "{n}");
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{n}");
        assert_eq!(t["inputSchema"]["required"], json!(required), "{n}");
    }
    let mesh = |t: &Value| MESH_TOOLS.iter().any(|m| t["name"] == m.0);
    // every other tool in the golden's order, byte-identical but start_run
    let rest: Vec<&Value> = live.iter().filter(|t| t["name"] != "new_case" && !mesh(t)).collect();
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
    for k in 0..MESH_TOOLS.len() {
        want.insert(wn + 2 + k, live[at + 1 + k].clone());
    }
    assert!(
        serde_json::to_string(&json!({"tools": want})).unwrap()
            == serde_json::to_string(&json!({"tools": live})).unwrap(),
        "tools/list with --project, without the F-273 changes, differs from the golden with the registered changes"
    );
}
