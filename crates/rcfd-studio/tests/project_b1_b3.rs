//! ADR 0131 B1 (browser lists equal the project library's), B2 (new project) and B3 (newer schema, v0 migration), driven
//! through the Studio model and its project worker.

mod common;

use common::{b1_project, temp, HALF_RUN};
use rcfd_project::Project;
use rcfd_studio::project_view::{self, Browser};
use rcfd_studio::Studio;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

fn opened(st: &mut Studio) -> Browser {
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() || s.project.is_none()),
        "the worker did not answer"
    );
    st.browser()
        .cloned()
        .unwrap_or_else(|| panic!("open failed: {:?}", st.last_message()))
}

#[test]
fn b1_browser_lists_equal_the_project_library() {
    let root = temp("b1");
    let p = b1_project(&root);
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    let b = opened(&mut st);

    let runs = p.runs().unwrap();
    assert_eq!(b.cases, p.cases().unwrap());
    assert_eq!(b.meshes, p.meshes().unwrap());
    assert_eq!(b.designs, p.designs().unwrap());
    assert_eq!(b.runs, runs.runs);
    assert_eq!(b.skipped, runs.skipped);
    assert!(b.errors.is_empty(), "{:?}", b.errors);

    assert_eq!(b.cases, ["cases/a.toml", "cases/b.toml", "cases/sub/c.toml"]);
    assert_eq!(b.meshes, ["meshes/m1.msh", "meshes/m2.msh"]);
    assert_eq!(b.runs.len(), 8);
    let mut per_state: BTreeMap<String, usize> = BTreeMap::new();
    for r in &b.runs {
        *per_state.entry(format!("{:?}", r.state)).or_default() += 1;
    }
    assert_eq!(
        per_state,
        BTreeMap::from([
            ("Failed".to_string(), 2),
            ("Finished".to_string(), 2),
            ("Queued".to_string(), 2),
            ("Running".to_string(), 2)
        ])
    );
    assert_eq!(b.skipped.len(), 1);
    assert!(b.skipped[0].path.ends_with(HALF_RUN), "{:?}", b.skipped[0].path);
    assert!(b.skipped[0].message.contains("run.json"), "{}", b.skipped[0].message);
    eprintln!(
        "b1: {} cases, {} meshes, {} runs {per_state:?}, skipped {:?}",
        b.cases.len(),
        b.meshes.len(),
        b.runs.len(),
        b.skipped
    );
}

#[test]
fn b1_control_run_ids_without_summaries_list_nine() {
    let root = temp("b1c");
    let p = b1_project(&root);
    let ids = p.run_ids().unwrap();
    eprintln!("b1 control: run_ids() lists {}", ids.len());
    assert_eq!(
        ids.len(),
        9,
        "a model built from run_ids() would list the half-created run"
    );
}

fn validate_and_manifest(root: &Path) -> (usize, String, String) {
    let p = Project::open(root).unwrap();
    let issues = p.validate().unwrap();
    for i in &issues {
        eprintln!("issue: {} {}", i.path.display(), i.message);
    }
    (
        issues.len(),
        p.manifest().name.clone(),
        p.manifest().solver_version_created.clone(),
    )
}

#[test]
fn b2_new_project_validates_with_the_name_and_version_entered() {
    let root = temp("b2").join("fan study");
    let version = project_view::solver_version();
    let mut st = Studio::in_memory();
    st.poll = None;
    st.create_project(&root, "Fan study", &version);
    let b = opened(&mut st);
    assert_eq!(b.name, "Fan study");
    assert_eq!(b.solver_version_created, version);
    let (n, name, ver) = validate_and_manifest(&root);
    eprintln!("b2: {n} issues, name {name:?}, solver version {ver:?}");
    assert_eq!(n, 0);
    assert_eq!(name, "Fan study");
    assert_eq!(ver, version);
    assert!(version.starts_with("rcfd "), "{version}");
}

#[test]
fn b2_control_create_into_an_existing_project_is_a_located_error() {
    let root = temp("b2c");
    Project::create(&root, "first", "rcfd test").unwrap();
    let mut st = Studio::in_memory();
    st.poll = None;
    st.create_project(&root, "second", "rcfd test");
    assert!(st.drain_until(WAIT, |s| s.project.is_none()), "create did not fail");
    let m = st.last_message().unwrap().to_string();
    eprintln!("b2 control: {m}");
    let manifest = root.join("rcfd-project.json");
    assert!(m.contains(&manifest.display().to_string()), "{m}");
    assert!(m.contains("exists"), "{m}");
    assert_eq!(validate_and_manifest(&root).1, "first");
}

/// Bytes of every file under `root`, by relative path.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

#[test]
fn b3_newer_schema_is_a_located_message_and_nothing_is_written() {
    let root = temp("b3n");
    b1_project(&root);
    let manifest = root.join("rcfd-project.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    v["schema_version"] = 2.into();
    std::fs::write(&manifest, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let before = snapshot(&root);
    let mut st = Studio::in_memory();
    st.open_project(&root);
    assert!(st.drain_until(WAIT, |s| s.project.is_none()), "open did not fail");
    let m = st.last_message().unwrap().to_string();
    eprintln!("b3 newer: {m}");
    assert!(m.contains(&manifest.display().to_string()), "{m}");
    assert!(m.contains("newer"), "{m}");
    let after = snapshot(&root);
    assert_eq!(before.len(), after.len());
    assert!(before == after, "files changed");
}

#[test]
fn b3_v0_project_is_migrated_in_memory_and_shown_as_unsaved() {
    let root = temp("b3v0");
    b1_project(&root);
    let manifest = root.join("rcfd-project.json");
    let v0 = br#"{"schema_version": 0, "title": "old project", "created_unix": 1700000000}"#;
    std::fs::write(&manifest, v0).unwrap();
    let before = snapshot(&root);
    let mut st = Studio::in_memory();
    st.open_project(&root);
    let b = opened(&mut st);
    assert_eq!(b.migrated_from, Some(0));
    assert_eq!(b.name, "old project");
    let notice = b.notice().unwrap();
    assert!(notice.contains("unsaved migration (schema 0)"), "{notice}");
    assert!(st.last_message().unwrap().contains("unsaved migration (schema 0)"));
    // The browser panel shows the notice.
    let mut app = rcfd_studio::StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let out = common::frame(&mut app, &ctx);
    let shown = format!("{:?}", out.shapes);
    assert!(shown.contains("unsaved migration"), "notice not drawn");
    drop(app);
    assert_eq!(std::fs::read(&manifest).unwrap(), v0, "manifest rewritten");
    assert!(before == snapshot(&root), "files changed");
    eprintln!("b3 v0: {notice}");
}
