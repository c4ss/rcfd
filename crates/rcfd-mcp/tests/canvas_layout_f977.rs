//! ADR 0817 (F-977): the layout of a run chosen by the category of its case, through the `rcfd-mcp` binary. B1: with one
//! `rcfd-mcp --project --canvas` process (the Studio's reading server), `show_view` of `{version: 1, run}` from a second
//! server shows the heat-transfer layout of the heated cavity's run, and the canvas's `read_view` gives that layout and
//! how it was chosen. B2: a view with panels gets the reply and `read_view` of F-976, key for key. B3: an unknown run,
//! and a run with panels, are refused (1003) at their key and change nothing.

mod common;

use common::Mcp;
use rcfd_workspace::view::CURRENT;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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

/// A copy of example-projects/quick-start in a fresh folder.
fn quick_start(tag: &str) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = common::scratch(&format!("f977-{tag}")).join("quick-start");
    copy_dir(&repo.join("example-projects/quick-start"), &dir);
    dir
}

fn d(dir: &Path) -> String {
    dir.display().to_string()
}

fn canvas(dir: &Path) -> Mcp {
    let mut c = Mcp::start(&["--project", &d(dir), "--client", "studio", "--read-only", "--canvas"]);
    c.request("tools/list", json!({}));
    c
}

fn journal(dir: &Path) -> Vec<u8> {
    std::fs::read(dir.join(".rcfd/edits.jsonl")).unwrap_or_default()
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// The six-kind view of `canvas_view_f976`.
fn panel_view() -> Value {
    json!({"version": 1, "title": "Cavity", "layout": {"columns": 2}, "panels": [
        {"kind": "case", "case": "cases/cavity-re100.toml"},
        {"kind": "run", "run": RUN},
        {"kind": "field", "run": RUN, "field": "p"},
        {"kind": "plot", "run": RUN},
        {"kind": "monitor", "run": RUN, "monitor": "nu_hot"},
        {"kind": "table", "runs": [RUN, OTHER]}
    ]})
}

/// ADR 0817 B1, B2, B3.
#[test]
fn b1_b2_b3_a_view_of_a_run_takes_the_layout_of_its_category() {
    let dir = quick_start("b");
    let mut studio = canvas(&dir);
    let mut c = Mcp::start(&["--project", &d(&dir)]);
    // B1
    let r = c.call(
        "show_view",
        json!({"view": {"version": 1, "run": RUN}, "reason": "F-977 B1"}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let shown = r["structuredContent"].clone();
    eprintln!("B1 show_view: {shown}");
    assert_eq!(shown["studios"], 1);
    assert_eq!(shown["category"], "heat_transfer");
    assert_eq!(shown["rule"], "energy.enabled = true");
    assert_eq!(shown["panels"], 6);
    let read = studio.call("read_view", json!({}))["structuredContent"].clone();
    eprintln!("B1 read_view: {read}");
    let f = |n: &str| json!({"kind": "field", "title": n, "run": RUN, "field": n});
    let m = |n: &str| json!({"kind": "monitor", "title": n, "run": RUN, "monitor": n});
    let want = json!([
        f("T"),
        f("ux"),
        m("q_hot"),
        m("nu_hot"),
        {"kind": "plot", "title": "Residuals", "run": RUN},
        {"kind": "table", "title": "Run", "runs": [RUN]}
    ]);
    assert_eq!(
        read["view"]["panels"], want,
        "the canvas does not read the heat-transfer layout"
    );
    assert_eq!(read["id"], shown["id"]);
    assert_eq!(
        read["chosen"],
        json!({"run": RUN, "case": "cases/heated-cavity-ra1e4.toml", "category": "heat_transfer",
            "rule": "energy.enabled = true", "from": format!("runs/{RUN}/case.snapshot")})
    );
    // B2: a view with panels, as F-976 gives it
    let r2 = c.call("show_view", json!({"view": panel_view()}));
    assert_eq!(r2["isError"], false, "{r2}");
    let k2 = keys(&r2["structuredContent"]);
    let read2 = studio.call("read_view", json!({}))["structuredContent"].clone();
    let kr = keys(&read2);
    eprintln!("B2 reply keys {k2:?}, read_view keys {kr:?}");
    assert_eq!(k2, ["hash", "id", "panels", "path", "shown", "studios"]);
    assert_eq!(kr, ["actor", "exists", "hash", "id", "path", "reason", "shown", "view"]);
    assert_eq!(read2["view"], panel_view());
    // B3: refused at their key, nothing changed
    let file = std::fs::read(dir.join(CURRENT)).unwrap();
    let before = journal(&dir);
    let bad = [
        (json!({"version": 1, "run": "r-20991231T000000-0000"}), "run"),
        (
            json!({"version": 1, "run": RUN, "panels": [{"kind": "plot", "run": RUN}]}),
            "panels",
        ),
    ];
    for (v, key) in bad {
        let r = c.call("show_view", json!({"view": v}));
        assert_eq!(r["isError"], true, "{r}");
        let e = r["structuredContent"]["error"].clone();
        eprintln!("B3 {key}: {e}");
        assert_eq!(e["code"], 1003);
        assert_eq!(e["data"]["errors"][0]["key"], key);
    }
    assert_eq!(
        std::fs::read(dir.join(CURRENT)).unwrap(),
        file,
        "the shown view changed"
    );
    assert_eq!(journal(&dir), before, "the journal changed");
}
