//! ADR 0185 A4 and A5 at the workspace: the MCP server's `edit_case` and the Studio's `save_case` on the same file from the
//! same base, in both orders (one write, one conflict naming the first writer's hash, one `.rcfd/edits.jsonl` line naming
//! the first writer); and the two forms of `save_case`: `Lines` keeps a comment line and the file's layout, `Whole` (the
//! Studio's form) writes the text it is given, and a removal in `Lines` is written whole (Q2 (a)).

use rcfd_api::case::Case;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_workspace::{SaveError, SaveForm, Workspace, CONFLICT};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const REL: &str = "cases/a5.toml";

/// A cavity case with a comment line and an end-of-line comment.
const CASE: &str = r#"# F-270 A5: a comment line a save keeps
title = "a5 cavity"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
[physics]
nu = 0.01   # viscosity
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
max_iterations = 5
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

fn project(tag: &str) -> (PathBuf, Workspace) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir()
        .join("rcfd-workspace-tests")
        .join(format!("{tag}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Project::create(&root, tag, "rcfd test").unwrap();
    std::fs::write(root.join(REL), CASE).unwrap();
    let ws = Workspace::open(&root).unwrap();
    (root, ws)
}

/// The text the Studio's editor saves for `CASE` with `edit` applied to its settings: the whole tree written by
/// `Case::to_toml` (`CaseEditor::to_bytes`).
fn studio_bytes(edit: impl FnOnce(&mut Case)) -> Vec<u8> {
    let mut c = Case::parse_toml(CASE).unwrap();
    edit(&mut c);
    Case::from_json(&c.tree.to_json()).unwrap().to_toml().into_bytes()
}

fn journal(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn disk(root: &Path) -> Vec<u8> {
    std::fs::read(root.join(REL)).unwrap()
}

fn settings(text: &[u8]) -> Value {
    Case::parse_toml(std::str::from_utf8(text).unwrap())
        .unwrap()
        .tree
        .to_json()
}

fn edit_case(ws: &Workspace, base: &str) -> Result<Value, rcfd_workspace::RpcError> {
    let args = json!({"path": REL, "base_hash": base, "set": {"physics.nu": 0.02}});
    match ws.call("edit_case", &args).expect("edit_case is a command") {
        Ok(rcfd_workspace::Answer::Value(v)) => Ok(v),
        Ok(other) => panic!("edit_case answered {other:?}"),
        Err(e) => Err(e),
    }
}

#[test]
fn a4_edit_case_then_a_studio_save_from_the_same_base() {
    let (root, ws) = project("a4-edit-first");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.set("mesh.nx", json!(10)).unwrap());
    let r = edit_case(&ws, &h0).unwrap();
    let h1 = r["new_hash"].as_str().unwrap().to_string();
    assert_eq!(r["written"], true);
    let e = ws.save_case(REL, &h0, &save, SaveForm::Whole).unwrap_err();
    eprintln!("A4 edit_case first: edit_case wrote {h1}; the save: {e}");
    assert_eq!(
        e,
        SaveError::Conflict {
            current: Some(h1.clone())
        }
    );
    assert_eq!(hash_bytes(&disk(&root)), h1, "the file is not edit_case's");
    let j = journal(&root);
    assert_eq!(j.len(), 1, "{j:?}");
    assert_eq!(
        (j[0]["who"].as_str(), j[0]["new_hash"].as_str()),
        (Some("edit_case"), Some(h1.as_str()))
    );
}

#[test]
fn a4_a_studio_save_then_edit_case_from_the_same_base() {
    let (root, ws) = project("a4-save-first");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.set("mesh.nx", json!(10)).unwrap());
    let s = ws.save_case(REL, &h0, &save, SaveForm::Whole).unwrap();
    assert_eq!((s.old_hash.as_deref(), s.form), (Some(h0.as_str()), SaveForm::Whole));
    assert_eq!(disk(&root), save, "the Whole form writes the text it is given");
    let e = edit_case(&ws, &h0).unwrap_err();
    eprintln!(
        "A4 save first: the save wrote {}; edit_case: {} {}",
        s.new_hash, e.code, e.message
    );
    assert_eq!(e.code, CONFLICT);
    assert_eq!(e.data, Some(json!({"current_hash": s.new_hash})));
    assert_eq!(hash_bytes(&disk(&root)), s.new_hash);
    let j = journal(&root);
    assert_eq!(j.len(), 1, "{j:?}");
    assert_eq!(
        (j[0]["who"].as_str(), j[0]["path"].as_str(), j[0]["old_hash"].as_str()),
        (Some("save_case"), Some(REL), Some(h0.as_str()))
    );
}

#[test]
fn a4_an_empty_base_creates_and_never_replaces() {
    let (root, ws) = project("a4-empty-base");
    let rel = "cases/new.toml";
    let s = ws.save_case(rel, "", CASE.as_bytes(), SaveForm::Whole).unwrap();
    assert_eq!(s.old_hash, None);
    let e = ws.save_case(rel, "", b"title = \"x\"\n", SaveForm::Whole).unwrap_err();
    assert_eq!(
        e,
        SaveError::Conflict {
            current: Some(s.new_hash)
        }
    );
    assert_eq!(std::fs::read(root.join(rel)).unwrap(), CASE.as_bytes());
}

#[test]
fn a5_lines_keeps_the_comment_line_and_every_other_line() {
    let (root, ws) = project("a5-lines");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.set("physics.nu", json!(0.02)).unwrap());
    let s = ws.save_case(REL, &h0, &save, SaveForm::Lines).unwrap();
    let text = String::from_utf8(disk(&root)).unwrap();
    eprintln!(
        "A5 lines: form {:?}, {} bytes (whole text {} bytes)",
        s.form,
        text.len(),
        save.len()
    );
    assert_eq!(s.form, SaveForm::Lines);
    assert_eq!(text, CASE.replace("nu = 0.01   # viscosity", "nu = 0.02   # viscosity"));
    assert!(text.starts_with("# F-270 A5: a comment line a save keeps\n"));
    assert_eq!(
        settings(text.as_bytes()),
        settings(&save),
        "the file does not hold the saved settings"
    );
    assert_eq!(journal(&root).len(), 1);
}

#[test]
fn a5_lines_adds_only_the_changed_setting_of_an_absent_optional_table() {
    let (root, ws) = project("a5-optional");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.set("mesh.transform.rotate_z_deg", json!(-4.0)).unwrap());
    let s = ws.save_case(REL, &h0, &save, SaveForm::Lines).unwrap();
    let text = String::from_utf8(disk(&root)).unwrap();
    eprintln!("A5 optional table: form {:?}\n{text}", s.form);
    assert_eq!(s.form, SaveForm::Lines);
    assert!(text.starts_with(CASE), "the file's own lines were not kept");
    assert_eq!(&text[CASE.len()..], "\n[mesh.transform]\nrotate_z_deg = -4.0\n");
    assert_eq!(settings(text.as_bytes()), settings(&save));
}

#[test]
fn a5_control_whole_loses_the_comment_line() {
    let (root, ws) = project("a5-whole");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.set("physics.nu", json!(0.02)).unwrap());
    let s = ws.save_case(REL, &h0, &save, SaveForm::Whole).unwrap();
    let text = String::from_utf8(disk(&root)).unwrap();
    assert_eq!(s.form, SaveForm::Whole);
    assert!(
        !text.contains("a comment line a save keeps"),
        "the whole text kept the comment:\n{text}"
    );
    assert!(!text.contains("# viscosity"));
    assert_eq!(text.as_bytes(), save.as_slice());
}

#[test]
fn a5_a_removed_report_in_lines_is_written_whole() {
    let (root, ws) = project("a5-removal");
    let h0 = hash_bytes(CASE.as_bytes());
    let save = studio_bytes(|c| c.tree.remove_named("reports", "lid_drag").unwrap());
    let s = ws.save_case(REL, &h0, &save, SaveForm::Lines).unwrap();
    assert_eq!(s.form, SaveForm::Whole);
    assert_eq!(disk(&root), save);
    assert!(settings(&save)["reports"].as_object().is_some_and(|r| r.is_empty()));
    assert_eq!(journal(&root).len(), 1);
}

#[test]
fn a5_lines_over_a_changed_file_is_a_conflict_and_writes_nothing() {
    let (root, ws) = project("a5-conflict");
    let h0 = hash_bytes(CASE.as_bytes());
    let other = CASE.replace("nx = 8", "nx = 9");
    std::fs::write(root.join(REL), &other).unwrap();
    let save = studio_bytes(|c| c.set("physics.nu", json!(0.02)).unwrap());
    let e = ws.save_case(REL, &h0, &save, SaveForm::Lines).unwrap_err();
    assert_eq!(
        e,
        SaveError::Conflict {
            current: Some(hash_bytes(other.as_bytes()))
        }
    );
    assert_eq!(disk(&root), other.as_bytes());
    assert!(journal(&root).is_empty());
}
