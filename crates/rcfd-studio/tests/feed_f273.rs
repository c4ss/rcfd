//! ADR 0188 C3-St (F-273, ADR 0173 R3): the Studio, headless, with its watcher at 500 ms, reacts to the change feed. A
//! workspace in this test process with the actor `mcp` stands in for the MCP server (the Studio reads the same files the
//! server's journal lines go to; the two-process check is `crates/rcfd-mcp/tests/feed_f273.rs`).
//!
//! (1) an `edit_case` of the open case with a reason: within 4 Studio poll periods the editor holds the new value and a
//! message names the actor, the setting, its old and new values and the reason; (2) a `new_case`: the browser lists it
//! and a message names it; (3) control: the Studio's own save is journaled with its actor and the changed setting, and
//! adds no message naming the Studio; (4) control: an edit by a workspace without an actor reloads the editor and adds
//! no feed message; a case made with the case starter is journaled with `who` `case_starter` and the Studio's actor.

mod common;

use common::{b1_project, temp};
use rcfd_project::records::hash_bytes;
use rcfd_studio::case_edit::EditorState;
use rcfd_studio::schema_model::value_at;
use rcfd_studio::watch::POLL;
use rcfd_studio::Studio;
use rcfd_workspace::journal::{Actor, MCP};
use rcfd_workspace::{Answer, Workspace};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const REL: &str = "cases/a.toml";
const WAIT: Duration = Duration::from_secs(30);
/// ADR 0188 C3-St: 4 periods of the Studio's watcher.
const LIMIT: Duration = Duration::from_millis(4 * 500);

fn open(root: &Path) -> (Studio, PathBuf) {
    assert_eq!(POLL, Duration::from_millis(500));
    let mut st = Studio::in_memory();
    st.poll = Some(POLL);
    st.run_poll = None;
    st.open_project(root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    let canonical = st.browser().unwrap().root.clone();
    st.open_case(REL);
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    // let the watcher take its first scans before the change
    std::thread::sleep(Duration::from_millis(1200));
    st.drain();
    (st, canonical)
}

fn call(ws: &Workspace, name: &str, args: Value) -> Value {
    match ws.call(name, &args).expect("a workspace command") {
        Ok(Answer::Value(v)) => v,
        other => panic!("{name}: {other:?}"),
    }
}

fn said(st: &Studio, needle: &str) -> bool {
    st.messages.iter().any(|m| m.contains(needle))
}

fn journal(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn disk_hash(root: &Path) -> String {
    hash_bytes(&std::fs::read(root.join(REL)).unwrap())
}

#[test]
fn st1_st2_a_change_by_another_actor_updates_the_views_and_says_who_what_and_why() {
    let root = temp("f273-st1");
    b1_project(&root);
    let (mut st, root) = open(&root);
    let ws = Workspace::open(&root).unwrap().with_actor(Actor::this_process(MCP));
    let why = "to reach Re 50";
    call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": disk_hash(&root), "set": {"physics.nu": 0.02}, "reason": why}),
    );
    let t0 = Instant::now();
    let shown = st.drain_until(Duration::from_secs(10), |s| {
        said(s, "edit_case by mcp")
            && value_at(&s.editor.as_ref().unwrap().settings, "physics.nu") == Some(&json!(0.02))
    });
    let dt = t0.elapsed();
    let msg = st
        .messages
        .iter()
        .find(|m| m.contains("edit_case by mcp"))
        .cloned()
        .unwrap_or_default();
    eprintln!("C3-St (1): shown after {:.3} s: {msg}", dt.as_secs_f64());
    assert!(shown, "not shown within 10 s; messages {:?}", st.messages);
    assert!(dt <= LIMIT, "shown after {dt:?}");
    let pid = std::process::id();
    assert!(msg.contains(&format!("mcp (pid {pid})")), "{msg}");
    assert!(msg.contains("physics.nu 0.01 -> 0.02"), "{msg}");
    assert!(msg.contains(why), "{msg}");
    assert_eq!(st.editor_state(), Some(EditorState::Clean));
    // (2) a new case: the browser lists it and a message names it
    call(
        &ws,
        "new_case",
        json!({"name": "fresh", "template": "lid_driven_cavity", "reason": "a second case"}),
    );
    let t0 = Instant::now();
    let listed = st.drain_until(Duration::from_secs(10), |s| {
        s.browser().unwrap().cases.iter().any(|c| c == "cases/fresh.toml") && said(s, "created cases/fresh.toml")
    });
    let dt = t0.elapsed();
    eprintln!(
        "C3-St (2): listed and said after {:.3} s: {:?}",
        dt.as_secs_f64(),
        st.messages.iter().find(|m| m.contains("cases/fresh.toml"))
    );
    assert!(listed, "messages {:?}", st.messages);
    assert!(dt <= LIMIT, "after {dt:?}");
    assert!(said(&st, "a second case"));
}

#[test]
fn st3_the_studios_own_save_names_its_actor_and_is_not_announced() {
    let root = temp("f273-st3");
    b1_project(&root);
    let (mut st, root) = open(&root);
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, "mesh.nx", "10").unwrap();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor_state() == Some(EditorState::Clean)),
        "{:?}",
        st.last_message()
    );
    // the feed's passes after the save
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        st.drain();
        std::thread::sleep(Duration::from_millis(20));
    }
    let j = journal(&root);
    eprintln!("C3-St (3): {j:?}; messages {:?}", st.messages);
    assert_eq!(j.len(), 1);
    assert_eq!(
        (j[0]["kind"].as_str(), j[0]["who"].as_str()),
        (Some("edit"), Some("save_case"))
    );
    assert_eq!(j[0]["actor"]["client"], "studio");
    assert_eq!(j[0]["actor"]["instance"]["pid"], std::process::id());
    assert_eq!(j[0]["changes"], json!([{"path": "mesh.nx", "old": 16, "new": 10}]));
    assert!(
        !said(&st, "studio (pid"),
        "the Studio announced its own save: {:?}",
        st.messages
    );
}

#[test]
fn st4_an_edit_without_an_actor_reloads_and_adds_no_feed_message_and_the_case_starter_is_journaled() {
    let root = temp("f273-st4");
    b1_project(&root);
    let (mut st, root) = open(&root);
    let plain = Workspace::open(&root).unwrap();
    call(
        &plain,
        "edit_case",
        json!({"path": REL, "base_hash": disk_hash(&root), "set": {"physics.nu": 0.03}}),
    );
    assert!(st.drain_until(Duration::from_secs(10), |s| value_at(
        &s.editor.as_ref().unwrap().settings,
        "physics.nu"
    ) == Some(&json!(0.03))));
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        st.drain();
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("C3-St (4): messages {:?}", st.messages);
    assert!(!said(&st, "edit_case"), "{:?}", st.messages);
    assert!(journal(&root)[0].get("actor").is_none());
    // a case from the case starter
    let sod = rcfd_studio::case_starter::EXAMPLES
        .iter()
        .find(|e| e.source == "sod/sod.toml")
        .unwrap();
    let rel = st.create_case_from_example(sod, "sod").unwrap();
    assert!(st.drain_until(WAIT, |s| s.browser().unwrap().cases.contains(&rel)));
    let j = journal(&root);
    let c = j
        .iter()
        .find(|l| l["kind"] == "case_created")
        .expect("a case_created line");
    eprintln!("C3-St (4): the case starter's line {c}");
    assert_eq!(
        (c["who"].as_str(), c["path"].as_str()),
        (Some("case_starter"), Some(rel.as_str()))
    );
    assert_eq!(c["actor"]["client"], "studio");
    assert_eq!(c["hash"], hash_bytes(sod.bytes()));
}
