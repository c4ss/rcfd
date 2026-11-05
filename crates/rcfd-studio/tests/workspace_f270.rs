//! ADR 0185 A3 and A4 through the Studio: the case editor's save goes through the project workspace's write rule
//! (`rcfd_workspace::save_case`), so it waits for the file's edit lock, compares the base hash under it, and leaves one
//! `.rcfd/edits.jsonl` line per accepted write.
//!
//! A3: another writer holds the edit lock, replaces the file and releases the lock; the Studio's save sent meanwhile ends
//! in the conflict state with that writer's bytes, and writes nothing. Control: the route before F-270 (`save_if_base`'s
//! compare-then-write without the lock, copied here) under the same schedule is written and then lost.
//! A4: the MCP server's `edit_case` (the workspace's, which the tool calls) and a Studio save from the same base, in both
//! orders: one write, one conflict naming the first writer's hash, one journal line naming the first writer.
//! The watcher is off in every test, so a conflict comes from the save alone.

mod common;

use common::{b1_project, temp, CAVITY};
use rcfd_project::records::hash_bytes;
use rcfd_studio::case_edit::EditorState;
use rcfd_studio::Studio;
use rcfd_workspace::{Answer, Workspace, CONFLICT};
use serde_json::{json, Value};
use std::path::Path;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(30);
const REL: &str = "cases/a.toml";

fn open(root: &Path) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    st.open_case(REL);
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, "mesh.nx", "10").unwrap();
    assert_eq!(st.editor_state(), Some(EditorState::Dirty));
    st
}

fn journal(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// The bytes another writer puts in the file.
fn other() -> String {
    CAVITY.replace("nx = 16", "nx = 24")
}

/// Hold the edit lock of `REL` as a cooperating writer does while it writes.
fn hold_lock(root: &Path) -> std::path::PathBuf {
    let lock = rcfd_project::atomic::edit_lock_path(root, REL);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, format!("{} f270-a3 test", std::process::id())).unwrap();
    lock
}

#[test]
fn a3_a_save_sent_while_another_writer_holds_the_lock_is_a_conflict() {
    let root = temp("f270-a3");
    b1_project(&root);
    let mut st = open(&root);
    let lock = hold_lock(&root);
    let t0 = Instant::now();
    st.save_case().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    // the other writer, still holding the lock, replaces the file, then lets go
    rcfd_project::atomic::atomic_write(&root.join(REL), other().as_bytes()).unwrap();
    std::fs::remove_file(&lock).unwrap();
    let released = t0.elapsed();
    assert!(
        st.drain_until(WAIT, |s| !s.editor.as_ref().unwrap().saving),
        "the save did not end"
    );
    let state = st.editor_state().unwrap();
    eprintln!(
        "A3: lock released after {:.3} s, the save ended after {:.3} s in conflict: {}; last message {:?}",
        released.as_secs_f64(),
        t0.elapsed().as_secs_f64(),
        matches!(state, EditorState::Conflict { .. }),
        st.last_message()
    );
    let EditorState::Conflict { disk: Some(disk) } = state else {
        panic!("not in conflict: {state:?}, {:?}", st.last_message())
    };
    assert_eq!(
        disk,
        other().as_bytes(),
        "the conflict does not hold the other writer's bytes"
    );
    assert_eq!(
        std::fs::read(root.join(REL)).unwrap(),
        other().as_bytes(),
        "the save wrote over the other writer"
    );
    assert!(
        journal(&root).is_empty(),
        "a save that wrote nothing left a journal line"
    );
}

/// The Studio's save before F-270 (`rcfd_studio::case_edit::save_if_base`, removed by ADR 0185 E4), copied: compare the
/// hash, then replace, without the edit lock. True when it wrote.
fn save_if_base_before(path: &Path, base: &str, bytes: &[u8]) -> bool {
    let disk = std::fs::read(path).ok();
    match &disk {
        Some(d) if hash_bytes(d) == base => {}
        None if base.is_empty() => {}
        _ => return false,
    }
    rcfd_project::atomic::atomic_write(path, bytes).unwrap();
    true
}

#[test]
fn a3_control_the_route_before_writes_under_the_lock_and_is_lost() {
    let root = temp("f270-a3c");
    b1_project(&root);
    let mut st = open(&root);
    let base = st.editor.as_ref().unwrap().base_hash.clone();
    let mine = st.editor.as_mut().unwrap().to_bytes().unwrap();
    let path = root.join(REL);
    let lock = hold_lock(&root);
    let wrote = save_if_base_before(&path, &base, &mine);
    let after_save = std::fs::read(&path).unwrap();
    rcfd_project::atomic::atomic_write(&path, other().as_bytes()).unwrap();
    std::fs::remove_file(&lock).unwrap();
    let end = std::fs::read(&path).unwrap();
    eprintln!(
        "A3 control: the route before wrote under the held lock: {wrote}; the file then {} the save's bytes; at the end \
         the other writer's: {}",
        if after_save == mine { "held" } else { "did not hold" },
        end == other().as_bytes()
    );
    assert!(wrote, "the route before did not write while the lock was held");
    assert_eq!(after_save, mine);
    assert_eq!(end, other().as_bytes());
    assert_ne!(end, mine, "the save's bytes survived");
}

fn edit_case(ws: &Workspace, base: &str) -> Result<Value, rcfd_workspace::RpcError> {
    let args = json!({"path": REL, "base_hash": base, "set": {"physics.nu": 0.02}});
    match ws.call("edit_case", &args).expect("edit_case is a command") {
        Ok(Answer::Value(v)) => Ok(v),
        Ok(other) => panic!("edit_case answered {other:?}"),
        Err(e) => Err(e),
    }
}

#[test]
fn a4_edit_case_then_the_studio_save_from_the_same_base() {
    let root = temp("f270-a4e");
    b1_project(&root);
    let mut st = open(&root);
    let ws = Workspace::open(&root).unwrap();
    let base = st.editor.as_ref().unwrap().base_hash.clone();
    let r = edit_case(&ws, &base).unwrap();
    let h1 = r["new_hash"].as_str().unwrap().to_string();
    let edited = std::fs::read(root.join(REL)).unwrap();
    st.save_case().unwrap();
    assert!(st.drain_until(WAIT, |s| !s.editor.as_ref().unwrap().saving));
    let state = st.editor_state().unwrap();
    eprintln!(
        "A4 edit_case first: written {h1}; the Studio's save: conflict {}",
        matches!(state, EditorState::Conflict { .. })
    );
    let EditorState::Conflict { disk: Some(disk) } = state else {
        panic!("not in conflict: {state:?}, {:?}", st.last_message())
    };
    assert_eq!(hash_bytes(&disk), h1);
    assert_eq!(std::fs::read(root.join(REL)).unwrap(), edited);
    let j = journal(&root);
    assert_eq!(j.len(), 1, "{j:?}");
    assert_eq!(j[0]["who"], "edit_case");
}

#[test]
fn a4_the_studio_save_then_edit_case_from_the_same_base() {
    let root = temp("f270-a4s");
    b1_project(&root);
    let mut st = open(&root);
    let ws = Workspace::open(&root).unwrap();
    let base = st.editor.as_ref().unwrap().base_hash.clone();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor_state() == Some(EditorState::Clean)),
        "{:?}",
        st.last_message()
    );
    let saved = st.editor.as_ref().unwrap().base_hash.clone();
    assert_eq!(hash_bytes(&std::fs::read(root.join(REL)).unwrap()), saved);
    let e = edit_case(&ws, &base).unwrap_err();
    eprintln!(
        "A4 the Studio first: saved {saved}; edit_case: {} {}",
        e.code, e.message
    );
    assert_eq!(e.code, CONFLICT);
    assert_eq!(e.data, Some(json!({"current_hash": saved})));
    assert_eq!(hash_bytes(&std::fs::read(root.join(REL)).unwrap()), saved);
    let j = journal(&root);
    assert_eq!(j.len(), 1, "{j:?}");
    assert_eq!(
        (
            j[0]["who"].as_str(),
            j[0]["path"].as_str(),
            j[0]["old_hash"].as_str(),
            j[0]["new_hash"].as_str()
        ),
        (Some("save_case"), Some(REL), Some(base.as_str()), Some(saved.as_str()))
    );
}
