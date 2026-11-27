//! ADR 0838 (F-987): show_view's description in the project-mode tool list names every panel kind the engine takes
//! (`KINDS`, `KINDS_F979`, `KINDS_F978` of `rcfd_workspace::view`), F-976's forms, the process panel of F-978, the three
//! forms of ADR 0807 D1 (a mesh of a case, a setting, a monitor of a case) and the run form of ADR 0817 D3. D1 the
//! phrases, D2 the rest of the entry as before, N1 the negative control: origin/main's text (476de79b) lacks the six new
//! phrases. No golden holds show_view (ADR 0838 finding 1); the goldens are not touched.

mod common;

use common::{project, Mcp};
use rcfd_workspace::view::{KINDS, KINDS_F978, KINDS_F979};
use serde_json::{json, Value};

/// show_view's description at 476de79b, before F-987.
const OLD: &str = "Show a view in the Studio that has this project open (Claude's canvas). view is a view specification: {version: 1, title?, layout?: {columns: 1-4}, panels: [1 to 8 panels]}, each panel {kind, title?} with, by kind: case {case: a file under cases/}, run {run}, field {run, field: a stored field, snapshot?: index, default the last}, mesh {mesh: an entry of list_meshes}, plot {run} (the residuals), monitor {run, monitor: a name of the run's monitors.<name>}, table {runs: 1 to 10 run ids}, process {case} (the case's steps from its creation to its last result, which read_view gives as process; each step names the view of its state). Checked against the project first: an invalid view is refused (error 1003) with every key and reason in data.errors, and nothing changes. With no Studio open, error 1002 says so and nothing is written. Otherwise the view is written to .rcfd/views/current.json with one journal line (view_shown), which every open Studio reads from the change feed and shows with its own views; returns its id and how many Studios have the project open.";

/// ADR 0838 D2: the registered phrases, in order: the kinds, F-976's forms, F-978's, ADR 0807 D1's, ADR 0817 D3's.
fn phrases() -> Vec<String> {
    let mut p: Vec<String> = KINDS
        .iter()
        .chain(KINDS_F979.iter())
        .chain(KINDS_F978.iter())
        .map(|k| format!("{k} {{"))
        .collect();
    for s in [
        // F-976 (ADR 0805)
        "case {case: a file under cases/}",
        "run {run}",
        "field {run, field",
        "mesh {mesh: an entry of list_meshes}",
        "plot {run}",
        "monitor {run, monitor",
        "table {runs",
        // F-978 (ADR 0826)
        "process {case}",
        // ADR 0807 D1
        "mesh {case}",
        "setting {case, setting",
        "monitor {case, monitor",
        // ADR 0817 D3
        "{version: 1, run, title?, layout?}",
        "layout of its case's category",
    ] {
        p.push(s.to_string());
    }
    p
}

/// The registered phrases `desc` does not contain, in order.
fn missing(desc: &str) -> Vec<String> {
    phrases().into_iter().filter(|p| !desc.contains(p.as_str())).collect()
}

/// show_view's entry of the live project-mode list.
fn show_view_entry(name: &str) -> Value {
    let dir = project(name);
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let list = m.request("tools/list", json!({}));
    list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "show_view")
        .expect("show_view listed")
        .clone()
}

/// D1: 0 of the 22 phrases missing.
#[test]
fn d1_show_view_description_names_every_form() {
    assert_eq!(phrases().len(), 22);
    let e = show_view_entry("f987d1");
    let desc = e["description"].as_str().unwrap();
    let miss = missing(desc);
    eprintln!("D1: {} of {} phrases missing {miss:?}", miss.len(), phrases().len());
    assert!(miss.is_empty(), "missing {miss:?}");
}

/// D2: the class, the required key and the properties as before F-987.
#[test]
fn d2_show_view_entry_otherwise_as_before() {
    let e = show_view_entry("f987d2");
    assert_eq!(e["_meta"]["rcfd/access"], "write_files");
    assert_eq!(e["inputSchema"]["required"], json!(["view"]));
    let props = e["inputSchema"]["properties"].as_object().unwrap();
    let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, ["reason", "view"]);
    assert_eq!(props["view"]["description"], "the view specification (version 1)");
    eprintln!("D2: write_files, required [view], properties {keys:?}");
}

/// N1: origin/main's text misses exactly the six new phrases.
#[test]
fn n1_old_description_misses_the_new_forms() {
    let miss = missing(OLD);
    eprintln!("N1: {} missing {miss:?}", miss.len());
    assert_eq!(
        miss,
        [
            "setting {",
            "mesh {case}",
            "setting {case, setting",
            "monitor {case, monitor",
            "{version: 1, run, title?, layout?}",
            "layout of its case's category",
        ]
    );
}
