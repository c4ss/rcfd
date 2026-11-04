//! ADR 0176 (job 49M) M1: the axial-fan example project's case previews as a 3D mesh in the Studio, headless and without a
//! GPU. The other example projects hold one-cell-thick 2D cases, which the preview draws face-on (`two_d`); the FAN-01
//! blade passage is not flat, so its preview is 3D (the orbitable view, "3D" in the Viewport caption).
//!
//! The project is opened from the tree whose runs were built on this platform (ADR 0150), as `example_projects_h.rs`
//! does: `example-projects-linux/` on x86_64 Linux, `example-projects/` elsewhere.

mod pv;

use rcfd_studio::preview::{self, Built, Limits};
use rcfd_tsstore::Reader;
use serde_json::Value;
use std::path::{Path, PathBuf};

const FAN_CASE: &str = "cases/fan01-passage-upwind.toml";

fn tree() -> PathBuf {
    let tree = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "example-projects-linux"
    } else {
        "example-projects"
    };
    pv::repo_root().join(tree)
}

fn build_ok(root: &Path, rel: &str) -> Built {
    match preview::build(root, rel, &Limits::default(), false, &|| false).result {
        Ok(b) => b,
        Err(e) => panic!("{}/{rel}: {e:?}", root.display()),
    }
}

/// M1: the fan case previews: a 3D mesh (`two_d` false) of the F3 cell count 4 920, a non-empty drawn surface whose
/// triangles and parts include the blade, a bounding box that spans more than 0.05 m along each axis, and the same mesh
/// (content hash) as the store of the committed run of the case. Control: a 2D example case of the same tree previews
/// with `two_d` true.
#[test]
fn m1_fan_example_previews_in_3d() {
    let root = rcfd_project::paths::canonicalize(&tree().join("axial-fan")).unwrap();
    let b = build_ok(&root, FAN_CASE);
    let (lo, hi) = b.bounds;
    let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let n_faces = b.surface.face_offsets.len().saturating_sub(1);
    println!(
        "M1 {}/{FAN_CASE}: 2D {}, {} cells, {} faces, {} points; surface {n_faces} faces, {} triangles, parts {:?}; extent {ext:?} m; mesh hash {}; built in {:.2} s",
        root.display(),
        b.two_d,
        b.cells,
        b.faces,
        b.points,
        b.triangles,
        b.surface.parts,
        b.mesh_hash,
        b.build_s
    );
    assert!(!b.two_d, "the fan passage is drawn in 3D");
    assert_eq!(b.cells, 4920, "the F3 mesh of ADR 0086");
    assert!(n_faces > 0 && b.triangles > 0, "empty surface");
    assert!(
        b.surface.parts.iter().any(|p| p == "blade"),
        "no blade part: {:?}",
        b.surface.parts
    );
    assert!(ext.iter().all(|e| *e > 0.05), "extent {ext:?}");
    // the committed run's store holds the same mesh as the preview
    let m: Value = serde_json::from_str(&std::fs::read_to_string(tree().join("manifest.json")).unwrap()).unwrap();
    let id = m["projects"]["axial-fan"]["runs"][0]["run"]
        .as_str()
        .unwrap()
        .to_string();
    let stored = Reader::open(&root.join("runs").join(&id).join("fields"))
        .unwrap()
        .mesh_hash();
    println!("M1 run {id}: store mesh hash {stored}");
    assert_eq!(stored, b.mesh_hash, "the run's stored mesh and the preview's differ");
    // control: a one-cell-thick example case of the same tree is drawn face-on
    let qs = rcfd_project::paths::canonicalize(&tree().join("quick-start")).unwrap();
    let c = build_ok(&qs, "cases/cavity-re100.toml");
    println!("M1 control quick-start/cases/cavity-re100.toml: 2D {}", c.two_d);
    assert!(c.two_d, "control: the cavity is 2D");
}
