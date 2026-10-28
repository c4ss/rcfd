//! ADR 0131 B6: a case file replaced atomically by another process while the project is open. The model reports it within
//! 2.0 s (4 poll periods of 500 ms); a clean editor reloads; a dirty editor enters the conflict state. Negative control: with
//! the watcher off, no report after 5 s. The other process is this test binary run again with `RCFD_STUDIO_CHILD` set
//! (the pattern of `rcfd-project`'s `tests/process.rs`); `child_entry` does nothing without it.

mod common;

use common::{b1_project, temp, CAVITY};
use rcfd_studio::case_edit::EditorState;
use rcfd_studio::schema_model::value_at;
use rcfd_studio::Studio;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const LIMIT: Duration = Duration::from_millis(2000);
const WAIT: Duration = Duration::from_secs(30);

#[test]
fn child_entry() {
    let Ok(target) = std::env::var("RCFD_STUDIO_CHILD") else {
        return;
    };
    let text = CAVITY.replace("nx = 16", "nx = 24");
    rcfd_project::atomic::atomic_write(Path::new(&target), text.as_bytes()).unwrap();
}

/// Replace `target` from another process; returns when that process has exited.
fn replace_from_child(target: &Path) -> Instant {
    let st = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_entry", "--nocapture", "--test-threads", "1"])
        .env("RCFD_STUDIO_CHILD", target)
        .status()
        .unwrap();
    let t = Instant::now();
    assert!(st.success(), "child failed: {st:?}");
    t
}

fn open(root: &Path, poll: Option<Duration>) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = poll;
    st.open_project(root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    st.open_case("cases/a.toml");
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    st
}

fn reported(st: &Studio) -> bool {
    st.project
        .as_ref()
        .unwrap()
        .changes
        .iter()
        .any(|(_, c)| c == "cases/a.toml")
}

#[test]
fn b6_clean_editor_reloads_within_the_limit() {
    let root = temp("b6clean");
    b1_project(&root);
    let mut st = open(&root, Some(rcfd_studio::watch::POLL));
    // let the watcher take its first scans before the change
    std::thread::sleep(Duration::from_millis(1200));
    st.drain();
    assert!(!reported(&st));
    let t0 = replace_from_child(&root.join("cases/a.toml"));
    let ok = st.drain_until(Duration::from_secs(10), reported);
    let dt = t0.elapsed();
    assert!(ok, "no report within 10 s");
    // the editor's reaction is applied in the same drain as the report
    assert!(st.drain_until(Duration::from_secs(2), |s| value_at(
        &s.editor.as_ref().unwrap().settings,
        "mesh.nx"
    ) == Some(&json!(24))));
    eprintln!(
        "b6 clean: reported after {:.3} s, editor {:?}, nx {:?}",
        dt.as_secs_f64(),
        st.editor_state(),
        value_at(&st.editor.as_ref().unwrap().settings, "mesh.nx")
    );
    assert!(dt <= LIMIT, "reported after {dt:?}");
    assert_eq!(st.editor_state(), Some(EditorState::Clean));
}

#[test]
fn b6_dirty_editor_enters_the_conflict_state_within_the_limit() {
    let root = temp("b6dirty");
    b1_project(&root);
    let mut st = open(&root, Some(rcfd_studio::watch::POLL));
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, "mesh.nx", "10").unwrap();
    assert_eq!(st.editor_state(), Some(EditorState::Dirty));
    std::thread::sleep(Duration::from_millis(1200));
    st.drain();
    let t0 = replace_from_child(&root.join("cases/a.toml"));
    let ok = st.drain_until(Duration::from_secs(10), reported);
    let dt = t0.elapsed();
    assert!(ok, "no report within 10 s");
    let state = st.editor_state().unwrap();
    eprintln!(
        "b6 dirty: reported after {:.3} s, editor in conflict: {}",
        dt.as_secs_f64(),
        matches!(state, EditorState::Conflict { .. })
    );
    assert!(dt <= LIMIT, "reported after {dt:?}");
    let EditorState::Conflict { disk: Some(disk) } = state else {
        panic!("not in conflict: {state:?}")
    };
    assert!(String::from_utf8(disk).unwrap().contains("nx = 24"));
    // the edit is kept
    assert_eq!(
        value_at(&st.editor.as_ref().unwrap().settings, "mesh.nx"),
        Some(&json!(10))
    );
    // a save is refused until the conflict is resolved; keeping the edits then saves over the disk version
    assert!(st.save_case().is_err());
    st.editor.as_mut().unwrap().keep_mine();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor_state() == Some(EditorState::Clean)),
        "{:?}",
        st.last_message()
    );
    let text = std::fs::read_to_string(root.join("cases/a.toml")).unwrap();
    assert!(text.contains("nx = 10"), "{text}");
}

#[test]
fn b6_control_watcher_off_reports_nothing_after_5_s() {
    let root = temp("b6off");
    b1_project(&root);
    let mut st = open(&root, None);
    let t0 = replace_from_child(&root.join("cases/a.toml"));
    let ok = st.drain_until(Duration::from_secs(5), reported);
    eprintln!(
        "b6 control: watcher off, report after {:.1} s: {ok}",
        t0.elapsed().as_secs_f64()
    );
    assert!(!ok);
    assert_eq!(
        value_at(&st.editor.as_ref().unwrap().settings, "mesh.nx"),
        Some(&json!(16))
    );
}

#[test]
fn b6_a_save_over_a_changed_file_is_a_conflict_and_writes_nothing() {
    let root = temp("b6save");
    b1_project(&root);
    let mut st = open(&root, None);
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, "mesh.nx", "10").unwrap();
    replace_from_child(&root.join("cases/a.toml"));
    let disk = std::fs::read(root.join("cases/a.toml")).unwrap();
    st.save_case().unwrap();
    assert!(st.drain_until(WAIT, |s| matches!(s.editor_state(), Some(EditorState::Conflict { .. }))));
    assert_eq!(std::fs::read(root.join("cases/a.toml")).unwrap(), disk, "written over");
}
