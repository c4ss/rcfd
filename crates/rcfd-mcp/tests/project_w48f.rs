//! ADR 0135 (wave 48F, server part): checks of the project tools that need no run service — F2 tool names, F3
//! protocol (access classes, read-only, schema), F4 path safety, F5 edit validation, F8 output size, F9 render,
//! F10 field summary, F12 plugin files. F3 result shapes, F11 and F13 are in `project_runs_w48f.rs`.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use rcfd_project::records::{hash_bytes, RunState, RunStatus};
use rcfd_project::Project;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// ADR 0135's tool table: name and access class.
const PROJECT_TOOLS: [(&str, &str); 17] = [
    ("project_info", "read"),
    ("list_runs", "read"),
    ("run_status", "read"),
    ("read_metrics", "read"),
    ("read_residuals", "read"),
    ("field_summary", "read"),
    ("render_field", "read"),
    ("compare_runs", "read"),
    ("list_changes", "read"),
    ("read_project_file", "read"),
    ("edit_case", "write_files"),
    ("write_note", "write_files"),
    ("start_run", "execute"),
    ("pause_run", "execute"),
    ("resume_run", "execute"),
    ("cancel_run", "execute"),
    ("wait_run", "read"),
];

fn names(list: &Value) -> Vec<String> {
    list["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

/// F2: every project tool's full name, in the plugin form and the Studio's app-owned form, passes
/// `AllowRule::Tool` validation and is at most 128 characters; plugin, server and app-owned server names have no `-`.
#[test]
fn f2_tool_names_pass_the_allow_list_rules() {
    use rcfd_claude::allow::{AllowRule, PINNED_SERVER, PROJECT_READ_TOOLS};
    let dir = project("f2");
    let s = server(&dir, Policy::default());
    let all = names(&s.tool_list());
    let project: Vec<&str> = PROJECT_TOOLS.iter().map(|t| t.0).collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients/claude-plugin");
    let plugin: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
    let mcp: Value = serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    let plugin_name = plugin["name"].as_str().unwrap();
    let servers: Vec<&String> = mcp["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(servers.len(), 1);
    let server_name = servers[0].as_str();
    for n in [plugin_name, server_name, PINNED_SERVER] {
        assert!(!n.contains('-'), "{n}");
    }
    let mut longest = 0;
    for t in &project {
        assert!(all.iter().any(|a| a == t), "{t} not listed");
        for full in [
            format!("mcp__plugin_{plugin_name}_{server_name}__{t}"),
            format!("mcp__{PINNED_SERVER}__{t}"),
        ] {
            AllowRule::Tool(full.clone())
                .validate()
                .unwrap_or_else(|e| panic!("{full}: {e}"));
            assert!(full.len() <= 128, "{full}");
            longest = longest.max(full.len());
        }
    }
    // negative control: a hyphen (as in the demoted rcfd-panel plugin) is refused
    assert!(AllowRule::Tool("mcp__plugin_rcfd-panel_project__list_runs".into())
        .validate()
        .is_err());
    // the bridge's first allow-list is exactly the read-class project tools
    let read: BTreeSet<&str> = PROJECT_TOOLS.iter().filter(|t| t.1 == "read").map(|t| t.0).collect();
    assert_eq!(read, PROJECT_READ_TOOLS.into_iter().collect::<BTreeSet<_>>());
    eprintln!(
        "F2: {} tools x 2 forms valid, longest {longest} characters; plugin '{plugin_name}', server '{server_name}', app-owned '{PINNED_SERVER}'",
        project.len()
    );
}

/// F3 (protocol part): access classes and `_meta` as ADR 0135's table; `--read-only` refuses every non-read tool
/// with 1100; an unknown argument is -32602 for every tool.
#[test]
fn f3_access_classes_read_only_and_schema() {
    let dir = project("f3p");
    let s = server(&dir, Policy::default());
    let list = s.tool_list();
    assert_eq!(names(&list).len(), 33 + 17);
    for (name, access) in PROJECT_TOOLS {
        let t = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert_eq!(t["_meta"]["rcfd/access"], access, "{name}");
        assert_eq!(t["_meta"]["rcfd/needsApproval"], access != "read", "{name}");
        assert_eq!(t["annotations"]["readOnlyHint"], access == "read", "{name}");
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
        // negative control of the schema check: an unknown argument
        let r = rpc_call(&s, name, json!({"no_such_argument": 1}));
        assert_eq!(r["error"]["code"], -32602, "{name}: {r}");
    }
    let ro = server(&dir, Policy::read_only());
    let before = tree(&dir);
    let args = |name: &str| match name {
        "edit_case" => json!({"path": "cases/cavity.toml", "base_hash": "fnv64:0", "set": {"physics.nu": 0.02}}),
        "write_note" => json!({"path": "notes/a.md", "text": "x"}),
        "start_run" => json!({"case": "cases/cavity.toml"}),
        _ => json!({"run": "r-20260101T000000-0000"}),
    };
    let mut refused = 0;
    for (name, access) in PROJECT_TOOLS.iter().filter(|t| t.1 != "read") {
        let r = call(&ro, name, args(name));
        assert_eq!(err_code(&r), 1100, "{name} ({access}): {r}");
        refused += 1;
    }
    assert_eq!(tree(&dir), before, "a refused call changed the project");
    // read tools still work read-only
    ok(&call(&ro, "project_info", json!({})));
    eprintln!("F3 protocol: 17 access classes as listed, 17 unknown-argument calls -32602, {refused} non-read tools refused read-only (1100)");
}

/// Sorted relative paths of every file and folder under `dir`.
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            out.push(p.strip_prefix(dir).unwrap().to_string_lossy().into_owned());
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

fn dir_link(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// F4: the 17 escape and refusal inputs of ADR 0122 P1, given to every path argument, are refused with 1004 and
/// create nothing; `notes/a.md` is accepted.
#[test]
fn f4_path_arguments_refuse_escapes() {
    let base = scratch("f4");
    let outside = base.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
    let dir = base.join("proj");
    Project::create(&dir, "f4", "test").unwrap();
    std::fs::write(dir.join("cases/cavity.toml"), cavity(12, 30)).unwrap();
    let root = Project::open(&dir).unwrap().root().to_path_buf();
    assert!(
        dir_link(&outside, &root.join("cases").join("out")),
        "set-up: no directory link"
    );
    let out_abs = outside.canonicalize().unwrap().join("secret.txt");
    // the inputs of crates/rcfd-project/tests/paths.rs (ADR 0122 P1)
    let mut inputs: Vec<String> = vec![
        "../outside/secret.txt".into(),
        "cases/../../outside/secret.txt".into(),
        "./cases/./../../outside/secret.txt".into(),
        "runs/r-20260101T000000-0000/../../../outside/secret.txt".into(),
        out_abs.to_string_lossy().into_owned(),
        "/outside/secret.txt".into(),
        "cases/out/secret.txt".into(),
        "cases/out".into(),
    ];
    if cfg!(windows) {
        let plain = out_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let drive = plain.chars().next().unwrap();
        inputs.push(r"cases\..\..\outside\secret.txt".into());
        inputs.push(format!("{drive}:secret.txt"));
        inputs.push(r"\\localhost\C$\Windows".into());
        inputs.push(plain.clone());
        inputs.push(format!(r"\\?\{plain}"));
    }
    inputs.extend(["", "  ", "cases/a.json:stream", "cases/\0x"].map(String::from));
    let s = server(&dir, Policy::default());
    let in_before = tree(&root);
    let out_before = tree(&outside);
    let mut n = 0;
    for x in &inputs {
        let calls = [
            ("read_project_file", json!({"path": x})),
            (
                "edit_case",
                json!({"path": x, "base_hash": "fnv64:0", "set": {"physics.nu": 0.02}}),
            ),
            ("write_note", json!({"path": x, "text": "x"})),
            ("start_run", json!({"case": x})),
        ];
        for (tool, args) in calls {
            let r = call(&s, tool, args);
            assert_eq!(err_code(&r), 1004, "{tool} {x:?}: {r}");
            n += 1;
        }
    }
    // run arguments: anything that is not a run id is refused the same way
    for x in ["../x", "r-1", "runs", r"..\..\x", "r-20260101T000000-000g"] {
        let r = call(&s, "run_status", json!({"run": x}));
        assert_eq!(err_code(&r), 1004, "run {x:?}: {r}");
    }
    assert_eq!(tree(&root), in_before, "the project changed");
    assert_eq!(tree(&outside), out_before, "the outside folder changed");
    // control against over-refusal
    let r = ok(&call(&s, "write_note", json!({"path": "notes/a.md", "text": "x"})));
    assert_eq!(r["path"], "notes/a.md");
    eprintln!(
        "F4: {} inputs x 4 path arguments = {n} calls refused with 1004, nothing created; notes/a.md accepted",
        inputs.len()
    );
    assert_eq!(inputs.len(), if cfg!(windows) { 17 } else { 12 });
}

/// F5: an invalid value is refused with the located message and the file unchanged; a valid edit is written and
/// re-parses to the edited value; a stale base hash is a conflict naming the current hash.
#[test]
fn f5_edit_case_validates_before_writing() {
    let dir = project("f5");
    let s = server(&dir, Policy::default());
    let path = dir.join("cases/cavity.toml");
    let f = ok(&call(&s, "read_project_file", json!({"path": "cases/cavity.toml"})));
    let h0 = f["hash"].as_str().unwrap().to_string();
    assert_eq!(h0, hash_bytes(&std::fs::read(&path).unwrap()));
    let r = call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h0, "set": {"physics.nu": -1}}),
    );
    assert_eq!(err_code(&r), 1003, "{r}");
    let msg = r["structuredContent"]["error"]["message"].as_str().unwrap().to_string();
    assert!(msg.contains("physics.nu") && msg.contains("cases/cavity.toml"), "{msg}");
    assert_eq!(
        hash_bytes(&std::fs::read(&path).unwrap()),
        h0,
        "the refused edit changed the file"
    );
    // dry run: valid, nothing written
    let d = ok(&call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h0, "set": {"physics.nu": 0.02}, "dry_run": true}),
    ));
    assert_eq!((d["written"].clone(), d["valid"].clone()), (json!(false), json!(true)));
    assert_eq!(hash_bytes(&std::fs::read(&path).unwrap()), h0);
    // valid edit
    let e = ok(&call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h0, "set": {"physics.nu": 0.02, "solver.max_iterations": 40}}),
    ));
    assert_eq!(e["written"], true);
    assert_eq!(e["form"], "minimal");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(e["new_hash"], hash_bytes(text.as_bytes()));
    assert!(
        text.contains("nu = 0.02   # viscosity") && text.contains("# a comment edit_case keeps"),
        "{text}"
    );
    let c = rcfd_api::case::Case::load(&path).unwrap();
    assert_eq!(c.get("physics.nu").unwrap(), json!(0.02));
    assert_eq!(c.get("solver.max_iterations").unwrap(), json!(40));
    // the old base is now a conflict naming the current hash
    let r = call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h0, "set": {"physics.nu": 0.03}}),
    );
    assert_eq!(err_code(&r), 1200);
    assert_eq!(r["structuredContent"]["error"]["data"]["current_hash"], e["new_hash"]);
    // a JSON case
    let json_case = rcfd_api::case::Case::parse_toml(&cavity(12, 30))
        .unwrap()
        .to_json_string();
    std::fs::write(dir.join("cases/c.json"), &json_case).unwrap();
    let hj = hash_bytes(json_case.as_bytes());
    let j = ok(&call(
        &s,
        "edit_case",
        json!({"path": "cases/c.json", "base_hash": hj, "set": {"physics.nu": 0.05}}),
    ));
    assert_eq!(
        rcfd_api::case::Case::load(dir.join("cases/c.json"))
            .unwrap()
            .get("physics.nu")
            .unwrap(),
        json!(0.05)
    );
    // the edit log names the tool and chains the hashes
    let log: Vec<Value> = std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(log.len(), 2);
    assert_eq!(
        (log[0]["who"].clone(), log[0]["old_hash"].clone()),
        (json!("edit_case"), json!(h0))
    );
    // negative control: a naive writer of the same invalid value changes the file
    std::fs::write(&path, text.replace("nu = 0.02", "nu = -1")).unwrap();
    assert_ne!(
        hash_bytes(&std::fs::read(&path).unwrap()),
        e["new_hash"].as_str().unwrap()
    );
    assert!(rcfd_api::case::Case::load(&path).is_err());
    eprintln!(
        "F5: physics.nu = -1 refused (1003: {msg}); valid edit written ({}), re-parsed 0.02 / 40; JSON form {}",
        e["form"], j["form"]
    );
}

/// ADR 0135 amendment 3 E1: an edit that cannot be placed in the file's own lines is refused, nothing written.
#[test]
fn e1_edit_case_refuses_a_whole_case_rewrite() {
    let dir = project("e1");
    let s = server(&dir, Policy::default());
    let path = dir.join("cases/cavity.toml");
    let text = cavity(12, 30).replace(
        "velocity_value = [1.0, 0.0, 0.0]",
        "velocity_value = [\n  1.0,\n  0.0,\n  0.0,\n]",
    );
    std::fs::write(&path, &text).unwrap();
    let h = hash_bytes(text.as_bytes());
    let r = call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"boundaries.lid.velocity_value": [2.0, 0.0, 0.0]}}),
    );
    assert_eq!(err_code(&r), 1003, "{r}");
    let msg = r["structuredContent"]["error"]["message"].as_str().unwrap().to_string();
    assert!(
        msg.contains("boundaries.lid.velocity_value") && msg.contains("edit the file by hand"),
        "{msg}"
    );
    assert_eq!(
        hash_bytes(&std::fs::read(&path).unwrap()),
        h,
        "the refused edit changed the file"
    );
    assert!(!dir.join(".rcfd/edits.jsonl").exists(), "an edit-log line was written");
    // control: a single-line setting of the same file is still edited in place
    let ok_r = ok(&call(
        &s,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"physics.nu": 0.02}}),
    ));
    assert_eq!(
        (ok_r["written"].clone(), ok_r["form"].clone()),
        (json!(true), json!("minimal"))
    );
    eprintln!("E1: refused with 1003 ({msg}); hash unchanged; single-line control written");
}

/// ADR 0135 amendment 3 M1 control: a `metrics.json` without `reports` (an older record) gives `list_runs` its numeric
/// top-level entries; with `reports`, the report values.
#[test]
fn m1_list_runs_metrics_fall_back_without_reports() {
    let dir = project("m1");
    let p = Project::open(&dir).unwrap();
    let old = p.create_run("cases/cavity.toml", None, "test", json!({})).unwrap().id;
    let new = p.create_run("cases/cavity.toml", None, "test", json!({})).unwrap().id;
    let summary = json!({"state": "finished", "reason": "r", "exit_code": 0, "steps": 30, "time": 1.5});
    rcfd_project::atomic::write_json(&dir.join("runs").join(&old).join("metrics.json"), &summary).unwrap();
    let mut with = summary.clone();
    with["reports"] = json!({"lid_drag": -8.5e-3});
    rcfd_project::atomic::write_json(&dir.join("runs").join(&new).join("metrics.json"), &with).unwrap();
    let s = server(&dir, Policy::default());
    let runs = ok(&call(&s, "list_runs", json!({})))["runs"].clone();
    let by = |id: &str| runs.as_array().unwrap().iter().find(|r| r["id"] == id).unwrap()["metrics"].clone();
    assert_eq!(by(&old), json!({"exit_code": 0.0, "steps": 30.0, "time": 1.5}));
    assert_eq!(by(&new), json!({"lid_drag": -8.5e-3}));
}

/// The F8 fixture: 500 runs (finished, metrics), one with a 100 000-step residual file and a 64 x 64 field store.
fn f8_fixture() -> (PathBuf, Vec<String>, String, String) {
    let dir = project("f8");
    let p = Project::open(&dir).unwrap();
    let mut ids = Vec::new();
    for _ in 0..500 {
        let rec = p
            .create_run(
                "cases/cavity.toml",
                None,
                "test",
                json!({"priority": 0, "threads": 1, "set": []}),
            )
            .unwrap();
        let mut st = RunStatus::new(RunState::Finished);
        st.step = 30;
        st.reason = Some("converged: test fixture".into());
        st.exit_code = Some(0);
        p.write_status(&rec.id, &st).unwrap();
        let m: serde_json::Map<String, Value> = (0..20)
            .map(|k| (format!("metric_{k:02}"), json!(k as f64 * 0.1 + 1e-3)))
            .collect();
        rcfd_project::atomic::write_json(&dir.join("runs").join(&rec.id).join("metrics.json"), &m).unwrap();
        ids.push(rec.id);
    }
    let big = ids[0].clone();
    let mut text = String::with_capacity(16 << 20);
    for i in 1..=100_000u64 {
        let x = (i as f64).powf(-0.7) * (1.0 + 0.3 * ((i * 7919) % 101) as f64 / 101.0);
        text.push_str(&format!(
            "{{\"step\":{i},\"time\":{i}.0,\"residuals\":{{\"u\":{x:e},\"p\":{:e},\"continuity\":{:e}}},\"monitors\":{{\"lid_drag\":{:e}}}}}\n",
            x * 2.0,
            x * 0.1,
            -8.5e-3 + x * 1e-3
        ));
    }
    std::fs::write(dir.join("runs").join(&big).join("residuals.jsonl"), &text).unwrap();
    let (_svc, _s, store) = service_store(&dir.parent().unwrap().join("svc"), 64, 20);
    let stored = run_with_store(&dir, &store);
    (dir, ids, big, stored)
}

/// F8: every read tool on the 500-run fixture, with its default arguments and its largest page or point count,
/// returns at most 40 000 bytes of text, or a path to a file.
#[test]
fn f8_read_tools_stay_under_the_size_limit() {
    let (dir, ids, big, stored) = f8_fixture();
    let s = server(&dir, Policy::default());
    std::fs::write(dir.join("notes/long.md"), "line \"quoted\"\n".repeat(20_000)).unwrap();
    let token = ok(&call(&s, "list_changes", json!({})))["token"].as_u64().unwrap();
    for k in 0..2000 {
        std::fs::write(dir.join(format!("notes/n{k:04}-with-a-longer-file-name.md")), "x").unwrap();
    }
    let ten: Vec<&String> = ids.iter().take(10).collect();
    let fifty: Vec<&String> = ids.iter().take(50).collect();
    let calls: Vec<(&str, Value)> = vec![
        ("project_info", json!({})),
        ("list_runs", json!({})),
        ("list_runs", json!({"limit": 100})),
        ("list_runs", json!({"limit": 100, "offset": 450})),
        ("run_status", json!({"run": big})),
        ("run_status", json!({"run": big, "events": 50})),
        ("read_metrics", json!({"runs": [ids[1]]})),
        ("read_metrics", json!({"runs": fifty})),
        ("read_residuals", json!({"run": big})),
        ("read_residuals", json!({"run": big, "max_points": 2000})),
        ("field_summary", json!({"run": stored, "field": "p"})),
        ("render_field", json!({"run": stored, "field": "p"})),
        ("render_field", json!({"run": stored, "field": "p", "width": 4096})),
        ("compare_runs", json!({"runs": [ids[1], ids[2]]})),
        ("compare_runs", json!({"runs": ten})),
        ("list_changes", json!({"since": token})),
        ("read_project_file", json!({"path": "notes/long.md"})),
        ("read_project_file", json!({"path": "notes/long.md", "limit": 32000})),
        (
            "read_project_file",
            json!({"path": format!("runs/{big}/residuals.jsonl"), "limit": 32000}),
        ),
        ("wait_run", json!({"run": big, "timeout_s": 0})),
    ];
    let mut worst = (0, String::new());
    for (tool, args) in &calls {
        let r = call(&s, tool, args.clone());
        let v = ok(&r);
        let n = result_bytes(&r);
        let path = v
            .get("path")
            .filter(|_| matches!(*tool, "read_residuals" | "render_field"));
        eprintln!(
            "F8 {tool} {}: {n} bytes{}",
            args.to_string().chars().take(60).collect::<String>(),
            path.map(|p| format!(", path {p}")).unwrap_or_default()
        );
        if let Some(p) = path.and_then(Value::as_str) {
            assert!(dir.join(p).is_file(), "{p}");
        }
        assert!(n <= 40_000, "{tool} {args}: {n} bytes");
        if n > worst.0 {
            worst = (n, format!("{tool} {args}"));
        }
    }
    // the residual set of 2000 points per series goes to a file; the paged list stops at the budget
    let r = ok(&call(&s, "read_residuals", json!({"run": big, "max_points": 2000})));
    assert!(r["path"].is_string(), "{r}");
    let l = ok(&call(&s, "list_changes", json!({"since": token})));
    assert_eq!(l["truncated"], true);
    // negative control: the residual series without decimation
    let bytes = std::fs::read(dir.join("runs").join(&big).join("residuals.jsonl")).unwrap();
    let (series, lines, _) = rcfd_mcp::project::residual_series(&bytes, 0, None);
    let full = serde_json::to_string(&series).unwrap().len();
    eprintln!(
        "F8: worst {} bytes ({}); control: {lines} lines without decimation {full} bytes",
        worst.0, worst.1
    );
    assert!(full > 40_000);
}

/// F9: `render_field` on a stored snapshot gives the PNG bytes of `render.png` on a service session holding the same
/// field values (amendment 1 item 8).
#[test]
fn f9_render_field_equals_render_png() {
    let dir = project("f9");
    let (svc, sess, store) = service_store(&dir.parent().unwrap().join("svc"), 12, 30);
    let id = run_with_store(&dir, &store);
    let want = svc
        .call("render.png", &json!({"session": sess, "field": "p", "width": 640}))
        .unwrap()["data_base64"]
        .as_str()
        .unwrap()
        .to_string();
    let s = server(&dir, Policy::default());
    let r = call(&s, "render_field", json!({"run": id, "field": "p", "width": 640}));
    let got = r["content"][0]["data"].as_str().unwrap().to_string();
    let meta = ok(&r);
    eprintln!(
        "F9: render.png {} base64 bytes, render_field {} (snapshot {})",
        want.len(),
        got.len(),
        meta["snapshot"]
    );
    assert_eq!(r["content"][0]["mimeType"], "image/png");
    assert!(got == want, "render_field differs from render.png");
    // negative control: the snapshot before the last
    let n = meta["snapshot"]["index"].as_u64().unwrap();
    let prev = rcfd_tsstore::Reader::open(&dir.join("runs").join(&id).join("fields"))
        .unwrap()
        .snapshots()[n as usize - 1]
        .step;
    let r2 = call(
        &s,
        "render_field",
        json!({"run": id, "field": "p", "width": 640, "step": prev}),
    );
    assert_ne!(r2["content"][0]["data"].as_str().unwrap(), want);
    // a point field or a 3D mesh is refused with a located message (2D cases only)
    let r3 = call(&s, "render_field", json!({"run": id, "field": "nope"}));
    assert_eq!(err_code(&r3), 1202);
}

fn neumaier(xs: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in xs {
        let t = s + x;
        if s.abs() >= x.abs() {
            c += (s - t) + x;
        } else {
            c += (x - t) + s;
        }
        s = t;
    }
    s + c
}

/// F10: min and max equal the stored field's exactly; the mean is within 1e-12 relative of a compensated sum.
#[test]
fn f10_field_summary_is_exact() {
    let dir = project("f10");
    let (_svc, _s, store) = service_store(&dir.parent().unwrap().join("svc"), 24, 30);
    let id = run_with_store(&dir, &store);
    let s = server(&dir, Policy::default());
    let mut r = rcfd_tsstore::Reader::open(&dir.join("runs").join(&id).join("fields")).unwrap();
    let vols = r.mesh().unwrap().cell_volumes().to_vec();
    let last = r.snapshots().len() - 1;
    let mut checked = 0;
    for field in ["p", "ux", "uy"] {
        let vals = r.read_field(last, field).unwrap();
        let fin: Vec<usize> = (0..vals.len()).filter(|&i| vals[i].is_finite()).collect();
        let min = fin.iter().map(|&i| vals[i]).fold(f64::INFINITY, f64::min);
        let max = fin.iter().map(|&i| vals[i]).fold(f64::NEG_INFINITY, f64::max);
        let mean = neumaier(fin.iter().map(|&i| vals[i] * vols[i])) / neumaier(fin.iter().map(|&i| vols[i]));
        let v = ok(&call(&s, "field_summary", json!({"run": id, "field": field})));
        assert_eq!(v["min"].as_f64().unwrap().to_bits(), min.to_bits(), "{field} min");
        assert_eq!(v["max"].as_f64().unwrap().to_bits(), max.to_bits(), "{field} max");
        let rel = (v["mean"].as_f64().unwrap() - mean).abs() / mean.abs().max(f64::MIN_POSITIVE);
        eprintln!(
            "F10 {field}: min {min:e}, max {max:e}, mean {mean:e}, relative difference {rel:e}, weighting {}",
            v["weighting"]
        );
        assert!(rel <= 1e-12, "{field}: {rel:e}");
        assert_eq!(v["weighting"], "cell_volume");
        checked += 1;
        // negative control: another snapshot's summary differs
        let o = ok(&call(
            &s,
            "field_summary",
            json!({"run": id, "field": field, "step": r.snapshots()[0].step}),
        ));
        assert!(
            o["min"] != v["min"] || o["max"] != v["max"] || o["mean"] != v["mean"],
            "{field}"
        );
    }
    // a time between two snapshots, interpolated, is flagged
    let (t0, t1) = (r.snapshots()[0].time, r.snapshots()[1].time);
    let i = ok(&call(
        &s,
        "field_summary",
        json!({"run": id, "field": "p", "time": 0.5 * (t0 + t1), "pick": "linear"}),
    ));
    assert_eq!(i["interpolated"], t0 != t1);
    assert_eq!(checked, 3);
}

/// Front matter of a SKILL.md: (key, value) pairs.
fn front_matter(text: &str) -> Result<Vec<(String, String)>, String> {
    let body = text.strip_prefix("---\n").ok_or("no front matter")?;
    let end = body.find("\n---\n").ok_or("front matter not closed")?;
    Ok(body[..end]
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect())
}

/// F12 checker of one skill: `name` and `description` present; `allowed-tools` names only existing project tools.
fn check_skill(text: &str, dir_name: &str, known: &BTreeSet<String>) -> Result<(), String> {
    let fm = front_matter(&text.replace("\r\n", "\n"))?;
    let get = |k: &str| fm.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
    let name = get("name").ok_or("no name")?;
    if name != dir_name {
        return Err(format!("name {name} is not the folder name {dir_name}"));
    }
    if get("description").is_none_or(|d| d.is_empty()) {
        return Err("no description".into());
    }
    let tools = get("allowed-tools").ok_or("no allowed-tools")?;
    for t in tools.split(',').map(str::trim) {
        if !known.contains(t) {
            return Err(format!("allowed-tools names {t}, which is not a project tool"));
        }
    }
    Ok(())
}

/// F12: plugin manifest, `.mcp.json`, skills and the in-repo marketplace.
#[test]
fn f12_plugin_files() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repo.join("clients/claude-plugin");
    let plugin: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
    assert_eq!(plugin["name"], "rcfd");
    assert_eq!(
        plugin["version"],
        env!("CARGO_PKG_VERSION"),
        "plugin version vs workspace version"
    );
    let mcp: Value = serde_json::from_str(&std::fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    let srv = &mcp["mcpServers"]["project"];
    assert_eq!(srv["command"], "rcfd-mcp");
    let args: Vec<&str> = srv["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    assert_eq!(
        args,
        ["--project", "${CLAUDE_PROJECT_DIR}", "--root", "${CLAUDE_PROJECT_DIR}"]
    );
    let known: BTreeSet<String> = PROJECT_TOOLS
        .iter()
        .map(|t| format!("mcp__plugin_rcfd_project__{}", t.0))
        .collect();
    let mut skills = Vec::new();
    for e in std::fs::read_dir(root.join("skills")).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(e.path().join("SKILL.md")).unwrap();
        check_skill(&text, &name, &known).unwrap_or_else(|m| panic!("{name}: {m}"));
        skills.push(name);
    }
    skills.sort();
    assert_eq!(skills, ["compare", "project-status", "start-and-watch"]);
    // negative control: a skill naming an unknown tool fails
    let bad = "---\nname: x\ndescription: d\nallowed-tools: mcp__plugin_rcfd_project__list_runs, mcp__plugin_rcfd_project__no_such_tool\n---\nbody\n";
    assert!(check_skill(bad, "x", &known).unwrap_err().contains("no_such_tool"));
    let market: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".claude-plugin/marketplace.json")).unwrap()).unwrap();
    let entry = &market["plugins"][0];
    assert_eq!(entry["name"], "rcfd");
    assert!(repo
        .join(entry["source"].as_str().unwrap())
        .join(".claude-plugin/plugin.json")
        .is_file());
    eprintln!(
        "F12: plugin rcfd {}, server project, skills {skills:?}",
        plugin["version"]
    );
}
