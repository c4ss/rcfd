//! ADR 0807 (F-979): the live canvas through the `rcfd-mcp` binary. B1: a view with setting panels, shown from a client
//! to an `rcfd-mcp --project --canvas` process (the Studio's reading server), gives each setting's value in the canvas's
//! `read_view` (`settings`, D2). B2: the client's `edit_case` with a reason reaches the canvas's `read_feed` within 4
//! polls of 250 ms (ADR 0188 C3) with its actor, reason and changes, and the canvas's `read_view` then gives the new
//! value, the other case's value unchanged. B3: a setting that is not one is refused at its key and changes nothing. B4:
//! a view without a setting panel gets F-976's `read_view` reply, key for key.

mod common;

use common::Mcp;
use rcfd_workspace::view::CURRENT;
use rcfd_workspace::Workspace;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// ADR 0188 C3: the reader's poll period and the limit in polls.
const PERIOD: Duration = Duration::from_millis(250);
const POLLS: usize = 4;
const RUN: &str = "r-20261028T152641-3900";
const OTHER: &str = "r-20261028T152641-5cc1";
const RE100: &str = "cases/cavity-re100.toml";
const RE400: &str = "cases/cavity-re400.toml";

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
    let dir = common::scratch(&format!("f979-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    let root = Workspace::open(&dir).unwrap().root().to_path_buf();
    (dir, root)
}

/// The live view of B1: two setting panels, a mesh panel and a monitor panel of a case.
fn live() -> Value {
    json!({"version": 1, "panels": [
        {"kind": "setting", "case": RE100, "setting": "physics.nu"},
        {"kind": "setting", "case": RE400, "setting": "physics.nu"},
        {"kind": "mesh", "case": RE100},
        {"kind": "monitor", "case": RE100, "monitor": "lid_drag"}
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

fn ok(r: &Value) -> Value {
    assert_eq!(r["isError"], false, "{r}");
    r["structuredContent"].clone()
}

/// ADR 0807 B1, B2, B3.
#[test]
fn b1_b2_b3_a_setting_changed_by_a_client_reaches_the_canvas_with_its_actor_and_reason() {
    let (dir, root) = quick_start("b1");
    let mut studio = canvas(&dir);
    let mut c = client(&dir);
    // B1
    let shown = ok(&c.call("show_view", json!({"view": live(), "reason": "F-979 B1"})));
    eprintln!("B1 show_view: {shown}");
    assert_eq!(shown["studios"], 1);
    let read = ok(&studio.call("read_view", json!({})));
    eprintln!("B1 read_view settings: {}", read["settings"]);
    assert_eq!(
        read["settings"],
        json!([
            {"panel": 0, "case": RE100, "setting": "physics.nu", "value": 0.01},
            {"panel": 1, "case": RE400, "setting": "physics.nu", "value": 0.0025}
        ]),
        "B1: the settings of read_view"
    );
    // B2
    let cursor = ok(&studio.call("read_feed", json!({})))["cursor"].as_u64().unwrap();
    let hash = ok(&c.call("read_project_file", json!({"path": RE100})))["hash"].clone();
    let edit = ok(&c.call(
        "edit_case",
        json!({"path": RE100, "base_hash": hash, "set": {"physics.nu": 0.005}, "reason": "F-979 B2: Re 200"}),
    ));
    assert_eq!(edit["written"], true, "{edit}");
    let t0 = Instant::now();
    let mut polls = 0;
    let entry = loop {
        polls += 1;
        let f = ok(&studio.call("read_feed", json!({"since": cursor})));
        let hit = f["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == RE100 && e["source"] == "journal")
            .cloned();
        if hit.is_some() || polls >= POLLS {
            break hit;
        }
        std::thread::sleep(PERIOD);
    };
    eprintln!(
        "B2: the edit in the canvas's feed after {polls} poll(s), {:?} (limit {POLLS}): {entry:?}",
        t0.elapsed()
    );
    let entry = entry.expect("B2: the canvas's feed has no entry of the edit within 4 polls");
    assert_eq!(entry["actor"]["client"], "mcp");
    assert_eq!(entry["actor"]["instance"]["pid"], c.pid());
    assert_eq!(entry["reason"], "F-979 B2: Re 200");
    assert_eq!(
        entry["changes"],
        json!([{"path": "physics.nu", "old": 0.01, "new": 0.005}])
    );
    let after = ok(&studio.call("read_view", json!({})));
    eprintln!("B2 read_view settings: {}", after["settings"]);
    assert_eq!(after["settings"][0]["value"], 0.005, "B2: the new value");
    assert_eq!(after["settings"][1]["value"], 0.0025, "B2: the other case's value");
    // B3: not a setting; nothing changes
    let file = std::fs::read(root.join(CURRENT)).unwrap();
    let before = journal(&dir);
    let bad = json!({"version": 1, "panels": [{"kind": "setting", "case": RE100, "setting": "physics.nope"}]});
    let r = c.call("show_view", json!({"view": bad}));
    assert_eq!(r["isError"], true, "B3: physics.nope was shown: {r}");
    let e = r["structuredContent"]["error"].clone();
    eprintln!("B3: {e}");
    assert_eq!(e["code"], 1003);
    assert_eq!(e["data"]["errors"][0]["key"], "panels[0].setting");
    assert!(e["data"]["errors"][0]["reason"]
        .as_str()
        .is_some_and(|r| r.contains("not a setting")));
    assert_eq!(
        std::fs::read(root.join(CURRENT)).unwrap(),
        file,
        "B3: the shown view changed"
    );
    assert_eq!(journal(&dir), before, "B3: the journal changed");
}

/// ADR 0807 B4: a view without a setting panel gets the reply of F-976, key for key.
#[test]
fn b4_a_view_without_a_setting_panel_reads_as_before() {
    let (dir, _) = quick_start("b4");
    let mut studio = canvas(&dir);
    let mut c = client(&dir);
    let view = json!({"version": 1, "title": "Cavity", "layout": {"columns": 2}, "panels": [
        {"kind": "case", "case": RE100},
        {"kind": "run", "run": RUN},
        {"kind": "field", "run": RUN, "field": "p"},
        {"kind": "plot", "run": RUN},
        {"kind": "monitor", "run": RUN, "monitor": "nu_hot"},
        {"kind": "table", "runs": [RUN, OTHER]}
    ]});
    ok(&c.call("show_view", json!({"view": view})));
    let read = ok(&studio.call("read_view", json!({})));
    let mut keys: Vec<&String> = read.as_object().unwrap().keys().collect();
    keys.sort();
    eprintln!("B4: read_view keys {keys:?}");
    assert_eq!(
        keys,
        ["actor", "exists", "hash", "id", "path", "reason", "shown", "view"],
        "B4: the keys of read_view"
    );
}
