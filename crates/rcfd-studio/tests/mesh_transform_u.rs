//! ADR 0155 (job 48U) S1-S3: `[mesh.transform]` in the Studio, headless and without a GPU. S1: the case editor lists the
//! table in the `mesh` group with its units and hover texts, and edits it (control: a unit string typed in the angle is
//! refused). S2: the mesh preview of the edited case, from the editor's text, a field being typed and the saved file,
//! is the mesh `rcfd_api::sim::build_mesh` builds, the mesh of the committed rotated example run; a case without the
//! table previews with the mesh hash recorded before this job (control: the bounds of the two differ). S3: a case made by
//! the case starter takes a rotation in the editor, validates and previews rotated.

mod common;

use common::temp;
use rcfd_api::case::Case;
use rcfd_mesh::Mesh;
use rcfd_project::Project;
use rcfd_studio::case_edit::{schema_tree, CaseEditor};
use rcfd_studio::case_starter;
use rcfd_studio::preview::{self, Built, Draft, EditorText, Limits, Outcome, Source};
use rcfd_studio::schema_model::Kind;
use rcfd_studio::Studio;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// R0's mesh hash of `example-projects/wind-tunnel/cases/naca0012-aoa4.toml` (`rcfd-api/tests/fixtures`).
const R0_AEROFOIL_MESH: u64 = 0x8516e4b6ee41c1d9;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn wind_tunnel() -> PathBuf {
    repo().join("example-projects").join("wind-tunnel")
}

/// A project holding a copy of the flow-rotated aerofoil case as `cases/a.toml` and its mesh folder.
fn aerofoil_project(tag: &str) -> PathBuf {
    let root = temp(tag);
    Project::create(&root, "48U aerofoil", "rcfd test").unwrap();
    std::fs::copy(
        wind_tunnel().join("cases").join("naca0012-aoa4.toml"),
        root.join("cases").join("a.toml"),
    )
    .unwrap();
    let mesh = root.join("meshes").join("naca0012-ogrid");
    std::fs::create_dir_all(&mesh).unwrap();
    for e in std::fs::read_dir(wind_tunnel().join("meshes").join("naca0012-ogrid")).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), mesh.join(e.file_name())).unwrap();
    }
    root
}

fn built(o: Outcome) -> Built {
    o.result.unwrap_or_else(|r| panic!("{r:?}"))
}

fn bounds(m: &Mesh) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in m.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn bounds_gap(a: ([f64; 3], [f64; 3]), b: ([f64; 3], [f64; 3])) -> f64 {
    (0..3)
        .map(|k| (a.0[k] - b.0[k]).abs().max((a.1[k] - b.1[k]).abs()))
        .fold(0.0, f64::max)
}

fn mesh_of(text: &str, dir: &Path) -> Mesh {
    rcfd_api::sim::build_mesh(&Case::parse_toml_in(text, dir).unwrap()).unwrap()
}

/// The store mesh hash of the committed run of `case` (ADR 0142 manifest).
fn run_mesh_hash(case: &str) -> String {
    let m: Value =
        serde_json::from_slice(&std::fs::read(wind_tunnel().join("..").join("manifest.json")).unwrap()).unwrap();
    let id = m["projects"]["wind-tunnel"]["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["case"] == case)
        .unwrap()["run"]
        .as_str()
        .unwrap()
        .to_string();
    let idx: Value = serde_json::from_slice(
        &std::fs::read(wind_tunnel().join("runs").join(id).join("fields").join("index.json")).unwrap(),
    )
    .unwrap();
    idx["mesh_hash"].as_str().unwrap().to_string()
}

/// S1: the editor's schema tree and an edit of the table.
#[test]
fn s1_the_editor_lists_the_table_with_units_and_edits_it() {
    let st = Studio::in_memory();
    let schema = schema_tree(&st.svc);
    let mesh = schema.find("mesh").unwrap();
    assert!(mesh.children.iter().any(|c| c.name == "transform"), "mesh group");
    let a = schema.find("mesh.transform.rotate_z_deg").unwrap();
    let o = schema.find("mesh.transform.rotate_origin").unwrap();
    println!(
        "S1 rotate_z_deg: {:?}, unit {:?}, hover {:?}\nS1 rotate_origin: {:?}, unit {:?}, hover {:?}",
        a.kind, a.unit, a.doc, o.kind, o.unit, o.doc
    );
    assert_eq!(
        a.kind,
        Kind::Float {
            min: Some(-360.0),
            max: Some(360.0),
            dim: None
        }
    );
    assert_eq!(a.unit.as_deref(), Some("deg"));
    assert!(
        a.doc.contains("counter-clockwise") && a.doc.contains("-alpha"),
        "{}",
        a.doc
    );
    assert_eq!(o.kind, Kind::NumberList);
    assert_eq!(o.unit.as_deref(), Some("m"));
    assert!(o.doc.contains("quarter chord"), "{}", o.doc);
    // no other leaf of the schema gained a unit (the display unit is new with this job)
    let with_unit: Vec<String> = schema
        .leaf_paths()
        .into_iter()
        .filter(|p| schema.find(p).is_some_and(|n| n.unit.is_some()))
        .collect();
    assert_eq!(
        with_unit,
        ["mesh.transform.rotate_origin", "mesh.transform.rotate_z_deg"]
    );
    // edit a copy of the aerofoil case
    let root = aerofoil_project("w48u_s1");
    let dir = root.join("cases");
    let bytes = std::fs::read(dir.join("a.toml")).unwrap();
    let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), "cases/a.toml", &dir, &bytes).unwrap();
    assert!(ed.settings["mesh"].get("transform").is_none(), "set-up: no table");
    let refused = ed
        .edit_text(&schema, "mesh.transform.rotate_z_deg", "-4 deg")
        .unwrap_err();
    println!("S1 control: {refused}");
    assert!(refused.starts_with("mesh.transform.rotate_z_deg"), "{refused}");
    assert!(
        ed.settings["mesh"].get("transform").is_none(),
        "a refused edit creates nothing"
    );
    ed.edit_text(&schema, "mesh.transform.rotate_z_deg", "-4").unwrap();
    ed.edit_text(&schema, "mesh.transform.rotate_origin", "[0.25, 0, 0]")
        .unwrap();
    assert_eq!(
        ed.settings["mesh"]["transform"],
        json!({"rotate_z_deg": -4.0, "rotate_origin": [0.25, 0.0, 0.0]})
    );
    assert!(ed.is_dirty());
    let text = String::from_utf8(ed.to_bytes().unwrap()).unwrap();
    assert!(
        text.contains("[mesh.transform]") && text.contains("rotate_z_deg = -4.0"),
        "{text}"
    );
    let back = Case::parse_toml_in(&text, &dir).unwrap();
    assert_eq!(back.tree.to_json(), ed.settings);
    assert!(ed.validate(), "{:?}", ed.diagnostics);
    println!("S1: edited, saved text parses back equal, valid");
}

/// S2: the preview of the edited case, three ways, against `build_mesh` and the committed rotated run.
#[test]
fn s2_the_preview_shows_the_rotated_mesh() {
    let st = Studio::in_memory();
    let schema = schema_tree(&st.svc);
    let root = aerofoil_project("w48u_s2");
    let dir = root.join("cases");
    let lim = Limits::default();
    let plain = built(preview::build(&root, "cases/a.toml", &lim, false, &|| false));
    println!("S2 without the table: mesh {:016x}", plain.mesh_hash);
    assert_eq!(
        plain.mesh_hash, R0_AEROFOIL_MESH,
        "the preview of a case without the table is as before"
    );
    let bytes = std::fs::read(dir.join("a.toml")).unwrap();
    let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), "cases/a.toml", &dir, &bytes).unwrap();
    ed.edit_text(&schema, "mesh.transform.rotate_z_deg", "-4").unwrap();
    ed.edit_text(&schema, "mesh.transform.rotate_origin", "[0.25, 0, 0]")
        .unwrap();
    let text = String::from_utf8(ed.to_bytes().unwrap()).unwrap();
    let want = mesh_of(&text, &dir);
    // the editor's unsaved text
    let src = Source::Editor(Arc::new(EditorText {
        settings: ed.settings.clone(),
        draft: None,
        json: false,
    }));
    let from_editor = built(preview::build_source(&root, "cases/a.toml", &src, &lim, false, &|| {
        false
    }));
    // a field being typed: -2 in the angle
    let draft = Draft {
        path: "mesh.transform.rotate_z_deg".into(),
        kind: Some(schema.find("mesh.transform.rotate_z_deg").unwrap().kind.clone()),
        text: "-2".into(),
    };
    let typing = Source::Editor(Arc::new(EditorText {
        settings: ed.settings.clone(),
        draft: Some(draft),
        json: false,
    }));
    let typed = built(preview::build_source(
        &root,
        "cases/a.toml",
        &typing,
        &lim,
        false,
        &|| false,
    ));
    let want2 = mesh_of(&text.replace("rotate_z_deg = -4.0", "rotate_z_deg = -2.0"), &dir);
    // the saved file
    std::fs::write(dir.join("a.toml"), text.as_bytes()).unwrap();
    let saved = built(preview::build(&root, "cases/a.toml", &lim, false, &|| false));
    let run = run_mesh_hash("cases/naca0012-aoa4-rotated.toml");
    println!(
        "S2 rotated -4: editor {:016x}, saved {:016x}, build_mesh {:016x}, committed run {run}; typed -2: {:016x} (build_mesh {:016x})",
        from_editor.mesh_hash,
        saved.mesh_hash,
        want.content_hash(),
        typed.mesh_hash,
        want2.content_hash()
    );
    assert_eq!(from_editor.mesh_hash, want.content_hash());
    assert_eq!(saved.mesh_hash, want.content_hash());
    // the committed run was made on Windows; the rotated coordinates come from the platform's sin and cos, whose last
    // bits may differ elsewhere (ADR 0137): compared on Windows, printed elsewhere
    if cfg!(windows) {
        assert_eq!(
            format!("{:016x}", saved.mesh_hash),
            run,
            "the mesh the rotated example run used"
        );
    } else {
        println!("S2: not Windows, the committed run's mesh hash {run} is not compared (ADR 0137)");
    }
    assert_eq!(typed.mesh_hash, want2.content_hash());
    for b in [&from_editor, &saved] {
        let gap = bounds_gap(b.bounds, bounds(&want));
        assert!(gap <= 1e-12, "bounds {gap:e}");
        assert_eq!(b.cells, 6144);
    }
    let control = bounds_gap(plain.bounds, saved.bounds);
    println!("S2 control: the unrotated preview's bounds differ by {control:.4}");
    assert!(control > 1e-3);
}

/// S3: a case made by the case starter (the wedge example) takes a rotation in the editor, validates, previews rotated.
#[test]
fn s3_a_starter_case_takes_a_rotation() {
    let st = Studio::in_memory();
    let schema = schema_tree(&st.svc);
    let root = temp("w48u_s3");
    let p = Project::create(&root, "48U starter", "rcfd test").unwrap();
    let ex = case_starter::EXAMPLES
        .iter()
        .find(|e| e.source == "wedge/wedge.toml")
        .expect("the wedge example");
    case_starter::create(&p, "cases/w.toml", ex.text.as_bytes()).unwrap();
    let dir = root.join("cases");
    let bytes = std::fs::read(dir.join("w.toml")).unwrap();
    let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), "cases/w.toml", &dir, &bytes).unwrap();
    ed.edit_text(&schema, "mesh.transform.rotate_z_deg", "10").unwrap();
    assert!(ed.validate(), "{:?}", ed.diagnostics);
    let text = String::from_utf8(ed.to_bytes().unwrap()).unwrap();
    std::fs::write(dir.join("w.toml"), text.as_bytes()).unwrap();
    let saved = built(preview::build(
        &root,
        "cases/w.toml",
        &Limits::default(),
        false,
        &|| false,
    ));
    let (plain, want) = (mesh_of(ex.text, &dir), mesh_of(&text, &dir));
    println!(
        "S3 wedge rotated 10: preview {:016x}, build_mesh {:016x}, unrotated {:016x}, {} cells, 2D {}",
        saved.mesh_hash,
        want.content_hash(),
        plain.content_hash(),
        saved.cells,
        saved.two_d
    );
    assert_eq!(saved.mesh_hash, want.content_hash());
    assert_ne!(saved.mesh_hash, plain.content_hash());
    assert_eq!(saved.cells, plain.n_cells());
    assert!(saved.two_d, "a 2D case stays 2D");
}
