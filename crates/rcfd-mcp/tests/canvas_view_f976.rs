//! ADR 0805 (F-976): Claude's canvas through the `rcfd-mcp` binary. S0: with no canvas process (and a registry file of a
//! process that is gone) `show_view` says no Studio has the project open, writes nothing, and removes the stale file.
//! S1: one `rcfd-mcp --project --canvas` process (as the Studio starts its reading server): `show_view` from a second
//! server shows the view (studios 1), writes one `view_shown` journal line that the canvas process's `read_feed` holds
//! within 4 polls of 250 ms (ADR 0188 C3), and its `read_view` returns the view as given. W1: the workspace methods give
//! the same. S2: two canvas processes, studios 2. S3: invalid views are refused (1003) at their key and change nothing.
//! L1 and L4: the two tools last in the project list; `--canvas` without `--project` exits 2, and a server without
//! `--canvas` registers nothing.

mod common;

use common::Mcp;
use rcfd_workspace::view::{registry_dir, CURRENT, VIEW_SHOWN};
use rcfd_workspace::Workspace;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// ADR 0188 C3: the reader's poll period and the limit in polls.
const PERIOD: Duration = Duration::from_millis(250);
const POLLS: usize = 4;
const RUN: &str = "r-20261028T152641-3900";
const OTHER: &str = "r-20261028T152641-5cc1";

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

/// A copy of example-projects/quick-start in a fresh folder, and its canonical root.
fn quick_start(tag: &str) -> (PathBuf, PathBuf) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = common::scratch(&format!("f976-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    let root = Workspace::open(&dir).unwrap().root().to_path_buf();
    (dir, root)
}

/// The view of the binary checks: six kinds (the mesh kind is checked by the unit tests of `view.rs`).
fn view() -> Value {
    json!({"version": 1, "title": "Cavity", "layout": {"columns": 2}, "panels": [
        {"kind": "case", "case": "cases/cavity-re100.toml"},
        {"kind": "run", "run": RUN},
        {"kind": "field", "run": RUN, "field": "p"},
        {"kind": "plot", "run": RUN},
        {"kind": "monitor", "run": RUN, "monitor": "nu_hot"},
        {"kind": "table", "runs": [RUN, OTHER]}
    ]})
}

fn d(dir: &Path) -> String {
    dir.display().to_string()
}

/// A server of the project that is a canvas (the Studio's reading server), ready once it has answered.
fn canvas(dir: &Path) -> Mcp {
    let mut c = Mcp::start(&["--project", &d(dir), "--client", "studio", "--read-only", "--canvas"]);
    c.request("tools/list", json!({}));
    c
}

fn client(dir: &Path) -> Mcp {
    Mcp::start(&["--project", &d(dir)])
}

fn journal(dir: &Path) -> Vec<u8> {
    std::fs::read(dir.join(".rcfd/edits.jsonl")).unwrap_or_default()
}

fn error_of(r: &Value) -> Value {
    assert_eq!(r["isError"], true, "{r}");
    r["structuredContent"]["error"].clone()
}

/// ADR 0805 S0: no Studio open.
#[test]
fn s0_with_no_studio_open_the_tool_says_so_and_writes_nothing() {
    let (dir, root) = quick_start("s0");
    // a registry file of a process that has ended
    let mut gone = std::process::Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = gone.id();
    gone.wait().unwrap();
    let reg = registry_dir(&root);
    std::fs::create_dir_all(&reg).unwrap();
    let stale = reg.join(format!("{pid}.json"));
    std::fs::write(&stale, json!({"pid": pid}).to_string()).unwrap();
    let before = journal(&dir);
    let mut c = client(&dir);
    let r = c.call("show_view", json!({"view": view()}));
    let e = error_of(&r);
    eprintln!("S0: {e}");
    assert_eq!(e["code"], 1002);
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("no Studio has this project open"),
        "{e}"
    );
    assert!(!dir.join(".rcfd/views").exists(), "a view was written");
    assert_eq!(journal(&dir), before, "the journal changed");
    assert!(!stale.exists(), "the registry file of the ended process is still there");
}

/// ADR 0805 S1, W1, S2, S3: one canvas, the workspace methods, two canvases, invalid views.
#[test]
fn s1_s2_s3_a_view_reaches_every_open_studio_and_an_invalid_one_changes_nothing() {
    let (dir, root) = quick_start("s1");
    let mut studio = canvas(&dir);
    let cursor = studio.call("read_feed", json!({}))["structuredContent"]["cursor"]
        .as_u64()
        .unwrap();
    let mut c = client(&dir);
    // S1
    let r = c.call("show_view", json!({"view": view(), "reason": "the cavity at Re 100"}));
    assert_eq!(r["isError"], false, "{r}");
    let shown = r["structuredContent"].clone();
    eprintln!("S1 show_view: {shown}");
    assert_eq!(shown["shown"], true);
    assert_eq!(shown["studios"], 1);
    assert_eq!(shown["panels"], 6);
    let lines: Vec<Value> = String::from_utf8(journal(&dir))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|l: &Value| l["kind"] == VIEW_SHOWN)
        .collect();
    assert_eq!(lines.len(), 1, "one view_shown line");
    assert_eq!(lines[0]["path"], CURRENT);
    assert_eq!(lines[0]["hash"], shown["hash"]);
    let t0 = Instant::now();
    let mut polls = 0;
    let seen = loop {
        polls += 1;
        let f = studio.call("read_feed", json!({"since": cursor}))["structuredContent"].clone();
        let hit = f["entries"].as_array().unwrap().iter().any(|e| e["kind"] == VIEW_SHOWN);
        if hit || polls >= POLLS {
            break hit;
        }
        std::thread::sleep(PERIOD);
    };
    eprintln!(
        "S1: view_shown in the canvas's feed after {polls} poll(s), {:?} (limit {POLLS})",
        t0.elapsed()
    );
    assert!(seen, "the canvas's feed has no view_shown entry after {POLLS} polls");
    let read = studio.call("read_view", json!({}))["structuredContent"].clone();
    assert_eq!(read["exists"], true);
    assert_eq!(read["view"], view(), "read_view does not return the view as given");
    assert_eq!(read["id"], shown["id"]);
    assert_eq!(read["reason"], "the cavity at Re 100");
    // W1: the workspace methods (the service method of ADR 0805 finding 5)
    let ws = Workspace::open(&dir).unwrap();
    let mut w_read = ws.read_view().unwrap();
    assert_eq!(w_read, read, "Workspace::read_view differs from the tool");
    let w_shown = ws.show_view(&json!({"view": view()})).unwrap();
    assert_eq!(w_shown["studios"], 1);
    w_read = ws.read_view().unwrap();
    assert_eq!(w_read["view"], view());
    assert_eq!(w_read["id"], w_shown["id"]);
    // S2
    let second = canvas(&dir);
    let r2 = c.call("show_view", json!({"view": view()}))["structuredContent"].clone();
    eprintln!("S2: {r2}");
    assert_eq!(r2["studios"], 2);
    drop(second);
    // S3: invalid views change nothing
    let file = std::fs::read(root.join(CURRENT)).unwrap();
    let before = journal(&dir);
    let mut bad = Vec::new();
    let mut kind = view();
    kind["panels"][0] = json!({"kind": "chart"});
    bad.push((kind, "panels[0].kind"));
    let mut colour = view();
    colour["panels"][1]["colour"] = json!("red");
    bad.push((colour, "panels[1].colour"));
    let mut run = view();
    run["panels"][3]["run"] = json!("r-20991231T000000-0000");
    bad.push((run, "panels[3].run"));
    for (v, key) in bad {
        let e = error_of(&c.call("show_view", json!({"view": v})));
        eprintln!("S3 {key}: {e}");
        assert_eq!(e["code"], 1003);
        assert_eq!(e["data"]["errors"][0]["key"], key);
        assert!(e["data"]["errors"][0]["reason"].as_str().is_some_and(|r| !r.is_empty()));
    }
    assert_eq!(
        std::fs::read(root.join(CURRENT)).unwrap(),
        file,
        "the shown view changed"
    );
    assert_eq!(journal(&dir), before, "the journal changed");
}

/// ADR 0805 L1: the two tools are the last of the project list.
#[test]
fn l1_show_view_and_read_view_come_last_in_the_project_list() {
    let (dir, _) = quick_start("l1");
    let mut c = client(&dir);
    let list = c.request("tools/list", json!({}))["result"]["tools"].clone();
    let tools = list.as_array().unwrap();
    let n = tools.len();
    eprintln!("L1: {n} tools with --project");
    let (s, r) = (&tools[n - 2], &tools[n - 1]);
    assert_eq!(tools[n - 3]["name"], "empty_trash");
    assert_eq!(s["name"], "show_view");
    assert_eq!(s["_meta"]["rcfd/access"], "write_files");
    assert_eq!(s["inputSchema"]["required"], json!(["view"]));
    assert_eq!(s["inputSchema"]["additionalProperties"], false);
    assert_eq!(r["name"], "read_view");
    assert_eq!(r["_meta"]["rcfd/access"], "read");
    assert_eq!(r["inputSchema"]["required"], json!([]));
    assert_eq!(r["inputSchema"]["additionalProperties"], false);
    let mut plain = Mcp::start(&[]);
    let names: Vec<Value> = plain.request("tools/list", json!({}))["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].clone())
        .collect();
    assert!(!names.contains(&json!("show_view")) && !names.contains(&json!("read_view")));
}

/// ADR 0805 L4: `--canvas` needs `--project`; a server without it registers nothing.
#[test]
fn l4_canvas_is_opt_in() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
        .arg("--canvas")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let (dir, root) = quick_start("l4");
    let mut c = client(&dir);
    c.request("tools/list", json!({}));
    let mine = registry_dir(&root).join(format!("{}.json", c.pid()));
    assert!(!mine.exists(), "a server without --canvas registered itself");
    assert!(
        !registry_dir(&root).exists(),
        "the registry folder was made without --canvas"
    );
}
