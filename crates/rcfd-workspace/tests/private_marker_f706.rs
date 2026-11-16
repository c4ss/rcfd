//! ADR 0647 (F-706) check B3w: the project-mode marker of `Ledger::private` is the manifest `rcfd-project.json` of a real
//! project folder.

use rcfd_api::design::ledger::{repository_marker, Ledger, PROJECT_MARKER};
use rcfd_project::Project;

#[test]
fn b3w_private_ledger_refused_in_a_project_folder() {
    assert_eq!(PROJECT_MARKER, rcfd_project::project::MANIFEST);
    let t = std::env::temp_dir().join(format!("rcfd_f706_b3w_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&t);
    std::fs::create_dir_all(&t).unwrap();
    assert_eq!(
        repository_marker(&t.join("probe.jsonl")),
        None,
        "N0: {} is inside a marker",
        t.display()
    );
    let root = t.join("proj");
    Project::create(&root, "f706", "test").unwrap();
    let p = root.join("designs").join("private.jsonl");
    let want = root.canonicalize().unwrap();
    assert_eq!(repository_marker(&p), Some((want.clone(), "rcfd-project.json")));
    let err = Ledger::private(&p).unwrap_err();
    println!("B3w: {err}");
    assert!(
        err.contains(&want.display().to_string()) && err.contains("(marker rcfd-project.json)"),
        "{err}"
    );
    assert!(!p.exists());
}
