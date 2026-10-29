//! ADR 0142 H1: the finished example projects in the Studio model, headless and without a GPU. Each project opens,
//! lists its cases and runs, every committed run shows a field at once, and every case opens in the Case editor and
//! validates there. The two aerofoil cases (`mesh.file = "../meshes/naca0012-ogrid"`) failed there until ADR 0144 made
//! the editor resolve relative paths against the case's folder (ADR 0142 E1's predicted miss); this branch was the check
//! of that failure and is now positive.
//!
//! ADR 0150: the projects are opened from the tree whose runs were built on this platform: `example-projects-linux/` on
//! x86_64 Linux, `example-projects/` on Windows (and on platforms with no tree of their own). Each tree holds only its
//! own platform's runs, so the Runs panel lists each case's run once.

mod vp;

use rcfd_studio::viewport::Status;
use rcfd_studio::Studio;
use serde_json::Value;
use std::path::PathBuf;

/// The tree of this platform (ADR 0150).
fn examples() -> PathBuf {
    let tree = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "example-projects-linux"
    } else {
        "example-projects"
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(tree)
}

/// (run id, case) of the committed runs of `project`, from the tree's `manifest.json`.
fn committed(project: &str) -> Vec<(String, String)> {
    let m: Value = serde_json::from_str(&std::fs::read_to_string(examples().join("manifest.json")).unwrap()).unwrap();
    m["projects"][project]["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["run"].as_str().unwrap().to_string(),
                r["case"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn h1_example_projects_open_to_a_picture_in_the_studio_model() {
    // wind-tunnel: the six cases of ADR 0142 and the two rotated aerofoil cases of ADR 0155
    for (name, n_cases) in [("wind-tunnel", 8), ("quick-start", 4)] {
        let root = rcfd_project::paths::canonicalize(&examples().join(name)).unwrap();
        let runs = committed(name);
        let mut st = Studio::in_memory();
        st.poll = None;
        st.run_poll = None;
        st.open_project(&root);
        assert!(
            st.drain_until(vp::WAIT, |s| s.browser().is_some()),
            "{name}: did not open"
        );
        let b = st.browser().unwrap();
        let (cases, listed, skipped) = (b.cases.clone(), b.runs.len(), b.skipped.len());
        println!(
            "H1 {}: {} cases, {listed} runs, {skipped} skipped, errors {:?}",
            root.display(),
            cases.len(),
            b.errors
        );
        assert_eq!(cases.len(), n_cases, "{name}: cases {cases:?}");
        assert_eq!(listed, runs.len(), "{name}: runs");
        assert_eq!(skipped, 0, "{name}: skipped {:?}", b.skipped);
        for (id, case) in &runs {
            let idx: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join("runs").join(id).join("fields").join("index.json")).unwrap(),
            )
            .unwrap();
            let n = idx["snapshots"].as_array().unwrap().len();
            st.select_run(id);
            assert!(
                st.drain_until(vp::WAIT, |s| s.viewport.frame.is_some() && !s.viewport.pending()),
                "{name}/{id}: no frame ({:?})",
                st.viewport.status
            );
            let vp = &st.viewport;
            let f = vp.frame.as_ref().unwrap();
            println!(
                "H1 {name}/{id} {case}: {:?}, {} snapshots, fields {:?}, shown {} at snapshot {} (step {}), 2D {}, {} faces",
                vp.status,
                vp.snaps.len(),
                vp.fields,
                f.field,
                f.index,
                f.step,
                vp.two_d,
                f.faces.len()
            );
            assert_eq!(vp.status, Status::Ready, "{name}/{id}");
            assert_eq!(vp.snaps.len(), n, "{name}/{id}: snapshots");
            assert_eq!(f.field, "p", "{name}/{id}: default field");
            assert_eq!(f.index, n - 1, "{name}/{id}: the last snapshot is shown first");
            assert!(vp.two_d, "{name}/{id}: a 2D case is drawn face-on");
            assert!(vp.error.is_none(), "{name}/{id}: {:?}", vp.error);
        }
        for case in &cases {
            st.open_case(case);
            assert!(
                st.drain_until(vp::WAIT, |s| s.editor.as_ref().is_some_and(|e| &e.rel == case)),
                "{name}/{case}: not opened ({:?})",
                st.last_message()
            );
            let ed = st.editor.as_mut().unwrap();
            let ok = ed.validate();
            println!("H1 {name}/{case}: editor validate {ok} {:?}", ed.diagnostics);
            // ADR 0144: the aerofoil cases' `../meshes/naca0012-ogrid` resolves against their folder, as in a run
            assert!(ok, "{name}/{case}: {:?}", ed.diagnostics);
        }
    }
}
