//! ADR 0128 finding 13 (R9): `run.json` records the case as a normalised project-relative path, and
//! `latest_finished` matches older records that kept the path as given.

mod common;

use rcfd_project::{atomic, Project, RunState, RunStatus};

fn finish(p: &Project, id: &str) {
    p.write_status(
        id,
        &RunStatus {
            state: RunState::Finished,
            step: 1,
            time: 1.0,
            pid: None,
            updated: rcfd_project::time::now(),
        },
    )
    .unwrap();
}

#[test]
fn r9_case_path_is_recorded_normalised() {
    let dir = common::scratch("r9");
    let p = Project::create(&dir.join("proj"), "r9", "0").unwrap();
    std::fs::write(p.root().join("cases/a.json"), b"{\"case\": 1}").unwrap();
    std::fs::write(dir.join("outside.json"), b"{}").unwrap();
    let abs = p.root().join("cases").join("a.json").to_string_lossy().into_owned();
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    for given in ["./cases/a.json", "cases/a.json", abs.as_str(), "cases/./a.json"] {
        std::thread::sleep(std::time::Duration::from_millis(5)); // distinct `created` milliseconds
        let r = match p.create_run(given, None, "0", serde_json::json!({})) {
            Ok(r) => r,
            Err(e) => {
                bad.push(format!("{given}: {e}"));
                continue;
            }
        };
        finish(&p, &r.id);
        let found = p.latest_finished("cases/a.json").unwrap().map(|s| s.id);
        let found_dot = p.latest_finished("./cases/a.json").unwrap().map(|s| s.id);
        lines.push(format!(
            "{given} -> recorded {:?}, found {:?}",
            r.case,
            found.as_ref() == Some(&r.id)
        ));
        if r.case != "cases/a.json" || found.as_ref() != Some(&r.id) || found_dot.as_ref() != Some(&r.id) {
            bad.push(format!(
                "{given}: recorded {:?}, latest {found:?} / {found_dot:?}, want {}",
                r.case, r.id
            ));
        }
    }
    // an older run.json that kept the path as given is still matched
    for old in ["./cases/a.json", "cases\\a.json", "cases//a.json"] {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let r = p.create_run("cases/a.json", None, "0", serde_json::json!({})).unwrap();
        let run_json = p.run_dir(&r.id).unwrap().join("run.json");
        let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&run_json).unwrap()).unwrap();
        v["case"] = old.into();
        atomic::write_json(&run_json, &v).unwrap();
        finish(&p, &r.id);
        let found = p.latest_finished("cases/a.json").unwrap().map(|s| s.id);
        lines.push(format!("old record {old:?}: found {:?}", found.as_ref() == Some(&r.id)));
        if found.as_ref() != Some(&r.id) {
            bad.push(format!("old record {old:?} not matched"));
        }
    }
    // outside the project is still refused
    let outside = dir.join("outside.json").to_string_lossy().into_owned();
    let refused = p.create_run(&outside, None, "0", serde_json::json!({})).is_err();
    lines.push(format!("outside {outside}: refused {refused}"));
    if !refused {
        bad.push("an absolute path outside the project was accepted".into());
    }
    for l in &lines {
        eprintln!("R9 {l}");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
