//! ADR 0131 B5: the case editor. Every schema leaf reachable and its default parses (port of the frozen GUI's test),
//! out-of-range and garbage input refused with a message naming the path, save round trip through the worker with the
//! base-hash rule; negative control: an edit the service refuses leaves the file's hash unchanged.

mod common;

use common::{b1_project, temp, CAVITY};
use rcfd_api::case::Case;
use rcfd_project::records::hash_bytes;
use rcfd_studio::case_edit::{schema_tree, CaseEditor, EditorState};
use rcfd_studio::schema_model::{format_value, parse_input, value_at, Kind};
use rcfd_studio::Studio;
use serde_json::json;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

fn editor() -> (Studio, CaseEditor) {
    let st = Studio::in_memory();
    let ed = CaseEditor::from_bytes(st.svc.clone(), "cases/a.toml", CAVITY.as_bytes()).unwrap();
    (st, ed)
}

#[test]
fn every_schema_leaf_is_reachable_and_parses_its_default() {
    let st = Studio::in_memory();
    let schema = schema_tree(&st.svc);
    let leaves = schema.leaf_paths();
    assert!(
        leaves.len() > 30,
        "schema should expose many settings, got {}",
        leaves.len()
    );
    let mut parsed = 0;
    for p in &leaves {
        let n = schema.find(p).unwrap_or_else(|| panic!("find {p}"));
        assert!(n.is_leaf());
        if matches!(n.kind, Kind::Text | Kind::NumberList) {
            continue;
        }
        let text = format_value(&n.default);
        if text.is_empty() || n.default.is_null() {
            continue;
        }
        parse_input(&n.kind, &text).unwrap_or_else(|e| panic!("default of {p} ({text}) does not parse: {e}"));
        parsed += 1;
    }
    eprintln!("b5: {} leaves reachable, {parsed} defaults parsed", leaves.len());
}

#[test]
fn parse_input_rejects_out_of_range_and_garbage() {
    let k = Kind::Int {
        min: Some(1.0),
        max: Some(10.0),
    };
    assert_eq!(parse_input(&k, "5").unwrap(), json!(5));
    assert!(parse_input(&k, "0").is_err());
    assert!(parse_input(&k, "abc").is_err());
    assert!(parse_input(&Kind::Bool, "maybe").is_err());
}

#[test]
fn b5_refusals_name_the_path_and_leave_the_value() {
    let (st, mut ed) = editor();
    let schema = &st.schema;
    let e = ed.edit_text(schema, "mesh.nx", "abc").unwrap_err();
    eprintln!("b5 garbage: {e}");
    assert!(e.starts_with("mesh.nx: "), "{e}");
    let e = ed.edit_text(schema, "mesh.nx", "-4").unwrap_err();
    eprintln!("b5 out of range: {e}");
    assert!(e.starts_with("mesh.nx: "), "{e}");
    assert_eq!(ed.field_errors.get("mesh.nx"), Some(&e));
    assert_eq!(value_at(&ed.settings, "mesh.nx"), Some(&json!(16)));
    assert_eq!(ed.state(), EditorState::Clean);
    let e = ed.edit_text(schema, "physics.no_such_thing", "1").unwrap_err();
    assert!(e.starts_with("physics.no_such_thing: "), "{e}");
    ed.edit_text(schema, "mesh.nx", "12").unwrap();
    assert!(!ed.field_errors.contains_key("mesh.nx"));
    assert_eq!(value_at(&ed.settings, "mesh.nx"), Some(&json!(12)));
    assert_eq!(ed.state(), EditorState::Dirty);
    // named entries (port of `named_entries_can_be_added_and_removed`)
    ed.add_named(schema, "reports", "m1").unwrap();
    assert!(value_at(&ed.settings, "reports.m1").is_some());
    ed.remove_named("reports.m1").unwrap();
    assert!(value_at(&ed.settings, "reports.m1").is_none());
    let e = ed.add_named(schema, "reports", "").unwrap_err();
    assert!(e.starts_with("reports: "), "{e}");
}

#[test]
fn b5_save_round_trip_through_the_worker() {
    let root = temp("b5");
    b1_project(&root);
    let path = root.join("cases/a.toml");
    let mut st = Studio::in_memory();
    st.open_project(&root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    st.open_case("cases/a.toml");
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    let schema = st.schema.clone();
    let ed = st.editor.as_mut().unwrap();
    assert_eq!(ed.base_hash, hash_bytes(&std::fs::read(&path).unwrap()));
    ed.edit_text(&schema, "mesh.nx", "10").unwrap();
    ed.edit_text(&schema, "solver.max_iterations", "123").unwrap();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor_state() == Some(EditorState::Clean)),
        "{:?}",
        st.last_message()
    );
    let bytes = std::fs::read(&path).unwrap();
    let ed = st.editor.as_ref().unwrap();
    assert_eq!(ed.base_hash, hash_bytes(&bytes));
    let back = Case::parse_toml(std::str::from_utf8(&bytes).unwrap()).expect("saved file parses");
    let again = Case::from_json(&ed.settings).expect("settings convert");
    assert_eq!(back.tree.content_hash(), again.tree.content_hash());
    // reopen in a fresh editor: the values are the edited ones
    let ed2 = CaseEditor::from_bytes(st.svc.clone(), "cases/a.toml", &bytes).unwrap();
    assert_eq!(value_at(&ed2.settings, "mesh.nx"), Some(&json!(10)));
    assert_eq!(value_at(&ed2.settings, "solver.max_iterations"), Some(&json!(123)));
    assert_eq!(ed2.settings, ed.settings);
    eprintln!("b5 round trip: {} bytes saved, hash {}", bytes.len(), ed.base_hash);
}

#[test]
fn b5_control_an_edit_the_service_refuses_leaves_the_file_hash_unchanged() {
    let root = temp("b5c");
    b1_project(&root);
    let path = root.join("cases/a.toml");
    let before = hash_bytes(&std::fs::read(&path).unwrap());
    let mut st = Studio::in_memory();
    st.open_project(&root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    st.open_case("cases/a.toml");
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    let ed = st.editor.as_mut().unwrap();
    // parses locally as JSON, refused by the service (a string for an integer)
    let e = ed.set_value("mesh.nx", json!("abc")).unwrap_err();
    eprintln!("b5 control: service refusal: {e}");
    assert!(e.starts_with("mesh.nx: "), "{e}");
    assert_eq!(ed.state(), EditorState::Clean);
    let fresh = CaseEditor::from_bytes(st.svc.clone(), "x.toml", CAVITY.as_bytes()).unwrap();
    assert_eq!(st.editor.as_ref().unwrap().settings, fresh.settings);
    // nothing to save: the model refuses, the worker writes nothing
    let r = st.save_case();
    assert!(!st.drain_until(Duration::from_millis(1500), |s| s.editor.as_ref().unwrap().saving));
    let after = hash_bytes(&std::fs::read(&path).unwrap());
    eprintln!("b5 control: save result {r:?}, hash before {before}, after {after}");
    assert!(r.is_err());
    assert_eq!(after, before, "the refused edit reached the file");
}
