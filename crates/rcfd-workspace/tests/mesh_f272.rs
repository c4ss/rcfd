//! ADR 0187 (F-272, ADR 0173 R2, W4, C8): the workspace's mesh commands. C8-I `import_mesh` of the Gmsh samples of
//! `rcfd-io`, a Fluent file written here, the public Fluent elbow when it is fetched and an OpenFOAM folder, with its
//! refusals; C8-E `edit_case` with a mesh setting; C8-P `preview_mesh` against the CPU renderer; C8-L `list_meshes`; and
//! the confinement of a case's mesh file (M2). The parity of `check_case_mesh` with `rcfd check-mesh` and the session
//! `check_mesh`, and the MCP tools, are `crates/rcfd-mcp/tests/mesh_setup_f272.rs`.

use rcfd_api::case::Case;
use rcfd_api::service::base64;
use rcfd_mesh::quality::{check_mesh, compute_metrics};
use rcfd_mesh::Mesh;
use rcfd_post::render::{render_scalar, RenderOptions};
use rcfd_project::Project;
use rcfd_workspace::{Answer, RpcError, Workspace, TEXT_BUDGET};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const CAVITY: &str = r#"title = "f272 cavity" # a comment edit_case keeps
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01   # viscosity
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 30
"#;

const WEDGE: &str = r#"title = "f272 wedge"
[mesh]
generator = "wedge_2d"
nx = 30
ny = 10
lx = 1.5
ly = 1.0
ramp_start = 0.2
ramp_angle = 20.0
"#;

const ANNULUS: &str = r#"title = "f272 annulus"
[mesh]
generator = "annulus_3d"
nx = 16
ny = 4
nz = 4
r_in = 0.5
r_out = 1.0
lz = 0.2
"#;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sample(name: &str) -> PathBuf {
    repo().join("crates/rcfd-io/tests/data").join(name)
}

fn project(name: &str) -> (PathBuf, Workspace) {
    let dir = std::env::temp_dir().join(format!("rcfd-ws-f272-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Project::create(&dir, name, "test").unwrap();
    let ws = Workspace::open(&dir).unwrap();
    let root = ws.root().to_path_buf();
    (root, ws)
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn answer(ws: &Workspace, name: &str, args: Value) -> Result<Answer, RpcError> {
    ws.call(name, &args).expect("a workspace command")
}

fn call(ws: &Workspace, name: &str, args: Value) -> Result<Value, RpcError> {
    match answer(ws, name, args)? {
        Answer::Value(v) => Ok(v),
        other => panic!("{name}: not a value: {other:?}"),
    }
}

fn image(ws: &Workspace, args: Value) -> (String, Value) {
    match answer(ws, "preview_mesh", args).unwrap() {
        Answer::Image { png_base64, meta } => (png_base64, meta),
        other => panic!("preview_mesh: not an image: {other:?}"),
    }
}

/// Sorted relative paths of every file and folder under `dir` (missing: none).
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            out.push(p.strip_prefix(dir).unwrap().to_string_lossy().into_owned());
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

fn diag_paths(e: &RpcError) -> Vec<String> {
    e.data
        .as_ref()
        .and_then(|d| d["diagnostics"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|d| d["path"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// The reader's counts, patches and zones of `mesh`, as `import_mesh` reports them.
fn reader_view(mesh: &Mesh) -> Value {
    let patches: Vec<Value> = mesh
        .patches()
        .iter()
        .map(|p| json!({"name": p.name, "faces": p.size, "kind": format!("{:?}", p.kind)}))
        .collect();
    let zones: Vec<Value> = mesh
        .cell_zones()
        .iter()
        .map(|z| json!({"name": z.name, "kind": "cell", "size": z.ids.len()}))
        .chain(
            mesh.face_zones()
                .iter()
                .map(|z| json!({"name": z.name, "kind": "face", "size": z.ids.len()})),
        )
        .collect();
    json!({"cells": mesh.n_cells(), "faces": mesh.n_faces(), "internal_faces": mesh.n_internal_faces(),
        "points": mesh.n_points(), "patches": patches, "zones": zones})
}

fn import_view(r: &Value) -> Value {
    json!({"cells": r["cells"], "faces": r["faces"], "internal_faces": r["internal_faces"], "points": r["points"],
        "patches": r["patches"], "zones": r["zones"]})
}

/// ASCII Fluent mesh of an `nx` x `ny` quad channel, `lx` x `ly`, sheared by `x += shear * y` (as
/// `crates/rcfd-cli/tests/fluent.rs` writes it, with the shear added).
fn channel_msh(nx: usize, ny: usize, lx: f64, ly: f64, shear: f64) -> String {
    let node = |i: usize, j: usize| 1 + i + (nx + 1) * j;
    let cell = |i: usize, j: usize| 1 + i + nx * j;
    let nn = (nx + 1) * (ny + 1);
    let mut s = format!("(0 \"channel\")\n(1 \"rcfd test\")\n(2 2)\n(10 (0 1 {nn:x} 0))\n(10 (1 1 {nn:x} 1 2)(\n");
    for j in 0..=ny {
        for i in 0..=nx {
            let y = ly * j as f64 / ny as f64;
            s += &format!("{} {}\n", lx * i as f64 / nx as f64 + shear * y, y);
        }
    }
    s += &format!("))\n(12 (0 1 {:x} 0))\n(12 (9 1 {:x} 1 3))\n", nx * ny, nx * ny);
    let mut zones: Vec<(usize, usize, Vec<String>)> = Vec::new();
    let mut interior = Vec::new();
    for j in 0..ny {
        for i in 1..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i, j + 1),
                cell(i - 1, j),
                cell(i, j)
            ));
        }
    }
    for j in 1..ny {
        for i in 0..nx {
            interior.push(format!(
                "{:x} {:x} {:x} {:x}",
                node(i, j),
                node(i + 1, j),
                cell(i, j - 1),
                cell(i, j)
            ));
        }
    }
    zones.push((3, 2, interior));
    let mut walls = Vec::new();
    for i in 0..nx {
        walls.push(format!("{:x} {:x} {:x} 0", node(i, 0), node(i + 1, 0), cell(i, 0)));
        walls.push(format!(
            "{:x} {:x} {:x} 0",
            node(i, ny),
            node(i + 1, ny),
            cell(i, ny - 1)
        ));
    }
    zones.push((4, 3, walls));
    zones.push((
        5,
        10,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(0, j), node(0, j + 1), cell(0, j)))
            .collect(),
    ));
    zones.push((
        6,
        5,
        (0..ny)
            .map(|j| format!("{:x} {:x} {:x} 0", node(nx, j), node(nx, j + 1), cell(nx - 1, j)))
            .collect(),
    ));
    let total: usize = zones.iter().map(|z| z.2.len()).sum();
    s += &format!("(13 (0 1 {total:x} 0))\n");
    let mut first = 1;
    for (id, bc, faces) in &zones {
        let last = first + faces.len() - 1;
        s += &format!("(13 ({id:x} {first:x} {last:x} {bc:x} 2)(\n{}\n))\n", faces.join("\n"));
        first = last + 1;
    }
    s += "(45 (3 interior interior-3)())\n(45 (4 wall walls)())\n(45 (5 velocity-inlet inlet)())\n(45 (6 pressure-outlet outlet)())\n(45 (9 fluid fluid)())\n";
    s
}

/// An OpenFOAM `polyMesh` folder of a 6 x 3 `box_2d` mesh at `dir`.
fn write_foam(dir: &Path) -> Mesh {
    let m = rcfd_mesh::gen::box_2d(6, 3, 1.5, 1.0, false, false).unwrap();
    rcfd_io::openfoam::write_dir(&m, dir).unwrap();
    rcfd_io::openfoam::read_dir(dir).unwrap()
}

/// C8-I: the 12 Gmsh samples of `rcfd-io` imported, each reporting what the reader gives.
#[test]
fn c8i_gmsh_samples() {
    let (root, ws) = project("gmsh");
    let mut n = 0;
    for (kind, cells) in [("tet", 1139usize), ("hex", 64), ("prism", 336)] {
        for ver in ["2.2", "4.1"] {
            for fmt in ["ascii", "bin"] {
                let file = format!("{kind}_v{ver}_{fmt}.msh");
                let bytes = std::fs::read(sample(&file)).unwrap();
                write(&root, &format!("incoming/{file}"), &bytes);
                let r = call(&ws, "import_mesh", json!({"path": format!("incoming/{file}")})).unwrap();
                let mesh = rcfd_io::read_msh_file(sample(&file)).unwrap();
                assert_eq!(r["format"], "gmsh", "{file}");
                assert_eq!(r["cells"], cells, "{file}");
                assert_eq!(import_view(&r), reader_view(&mesh), "{file}");
                let mut names: Vec<&str> = r["patches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p["name"].as_str().unwrap())
                    .collect();
                names.sort_unstable();
                assert_eq!(names, ["inlet", "outlet", "walls"], "{file}");
                assert_eq!(r["path"], format!("meshes/{file}"));
                assert_eq!(r["copied"], true);
                assert_eq!(r["mesh_file"], format!("../meshes/{file}"));
                assert_eq!(std::fs::read(root.join("meshes").join(&file)).unwrap(), bytes, "{file}");
                assert_eq!(r["files"][0]["hash"], rcfd_project::records::hash_bytes(&bytes));
                let (lo, hi) = (&r["bounds"]["min"], &r["bounds"]["max"]);
                for (k, want) in [2.0, 1.0, 1.0].into_iter().enumerate() {
                    assert!(lo[k].as_f64().unwrap().abs() < 1e-9, "{file} {lo}");
                    assert!((hi[k].as_f64().unwrap() - want).abs() < 1e-9, "{file} {hi}");
                }
                n += 1;
            }
        }
    }
    assert_eq!(n, 12);
    eprintln!("C8-I gmsh: 12 samples imported, counts, patches and zones as the reader's");
}

/// C8-I: a Fluent file written here, an OpenFOAM folder (as the folder holding `polyMesh` and as `polyMesh`), the same
/// file again (not copied), and the refusals, each leaving `meshes/` unchanged.
#[test]
fn c8i_fluent_openfoam_and_refusals() {
    let (root, ws) = project("fluent");
    // Fluent, 8 x 4 quads
    let text = channel_msh(8, 4, 2.0, 1.0, 0.0);
    write(&root, "incoming/chan.msh", &text);
    let r = call(&ws, "import_mesh", json!({"path": "incoming/chan.msh"})).unwrap();
    let mesh = rcfd_io::read_msh_file(root.join("incoming/chan.msh")).unwrap();
    assert_eq!(r["format"], "fluent");
    assert_eq!(r["cells"], 32);
    assert_eq!(import_view(&r), reader_view(&mesh));
    let mut names: Vec<&str> = r["patches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["frontAndBack", "inlet", "outlet", "walls"]);
    assert!(
        r["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("extruded")),
        "{r}"
    );
    assert_eq!(std::fs::read_to_string(root.join("meshes/chan.msh")).unwrap(), text);
    // the same file again: nothing written
    let before = tree(&root.join("meshes"));
    let again = call(&ws, "import_mesh", json!({"path": "incoming/chan.msh"})).unwrap();
    assert_eq!(again["copied"], false);
    assert_eq!(again["files"][0]["written"], false);
    assert_eq!(tree(&root.join("meshes")), before);

    // OpenFOAM: the folder holding polyMesh, then the polyMesh folder under another name
    let foam = write_foam(&root.join("incoming/foam/polyMesh"));
    let r = call(&ws, "import_mesh", json!({"path": "incoming/foam"})).unwrap();
    assert_eq!(r["format"], "openfoam");
    assert_eq!(r["path"], "meshes/foam");
    assert_eq!(r["mesh_file"], "../meshes/foam");
    assert_eq!(import_view(&r), reader_view(&foam));
    let files: Vec<&str> = r["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        assert!(files.contains(&format!("meshes/foam/{f}").as_str()), "{files:?}");
    }
    for f in &files {
        let name = f.rsplit('/').next().unwrap();
        assert_eq!(
            std::fs::read(root.join(f)).unwrap(),
            std::fs::read(root.join("incoming/foam/polyMesh").join(name)).unwrap(),
            "{f}"
        );
    }
    let r2 = call(
        &ws,
        "import_mesh",
        json!({"path": "incoming/foam/polyMesh", "name": "foam2"}),
    )
    .unwrap();
    assert_eq!(r2["path"], "meshes/foam2");
    assert_eq!(r2["copied"], true);
    assert_eq!(import_view(&r2), reader_view(&foam));
    // the polyMesh folder without a name is the folder above it: there already, same bytes
    let r3 = call(&ws, "import_mesh", json!({"path": "incoming/foam/polyMesh"})).unwrap();
    assert_eq!(
        (r3["path"].as_str(), r3["copied"].as_bool()),
        (Some("meshes/foam"), Some(false))
    );

    // refusals: nothing under meshes/ changes
    let outside = root
        .parent()
        .unwrap()
        .join(format!("f272-outside-{}.msh", std::process::id()));
    std::fs::write(&outside, &text).unwrap();
    write(&root, "incoming/notamesh.msh", "hello, not a mesh\n");
    write(
        &root,
        "incoming/tet.msh",
        std::fs::read(sample("tet_v4.1_ascii.msh")).unwrap(),
    );
    let before = tree(&root.join("meshes"));
    let cases: Vec<(Value, i64)> = vec![
        (json!({"path": "incoming/notamesh.msh"}), 1003),
        (json!({"path": "incoming/none.msh"}), 1202),
        (json!({"path": "../outside.msh"}), 1004),
        (json!({"path": outside.to_string_lossy()}), 1004),
        (json!({"path": "incoming/tet.msh", "name": "a/b.msh"}), 1004),
        (json!({"path": "incoming/tet.msh", "name": "tet.cas"}), 1004),
        (json!({"path": "incoming/tet.msh", "name": "CHAN.msh"}), 1004),
        (json!({"path": "incoming/tet.msh", "name": "chan.msh"}), 1004),
        (json!({"path": "incoming/tet.msh", "name": "con.msh"}), 1004),
    ];
    for (args, code) in &cases {
        let e = call(&ws, "import_mesh", args.clone()).unwrap_err();
        assert_eq!(e.code, *code, "{args}: {e:?}");
        assert_eq!(tree(&root.join("meshes")), before, "{args}");
    }
    let _ = std::fs::remove_file(&outside);
    // control against over-refusal: the same file under a free name
    let ok = call(
        &ws,
        "import_mesh",
        json!({"path": "incoming/tet.msh", "name": "tet-1.msh"}),
    )
    .unwrap();
    assert_eq!(ok["cells"], 1139);
    eprintln!(
        "C8-I fluent/openfoam: 32 cells, foam {} cells, {} refusals with meshes/ unchanged",
        foam.n_cells(),
        cases.len()
    );
}

/// C8-I: the public Fluent elbow of `fluent_public.rs`, when it has been fetched (data-gated as that test).
#[test]
fn c8i_fluent_public_elbow_when_fetched() {
    let path = repo().join("validation-data/data/fluent-elbow-openfoam/elbow.msh");
    if !path.exists() {
        eprintln!(
            "skipped: {} not fetched (python validation-data/fetch.py fetch fluent-elbow-openfoam)",
            path.display()
        );
        return;
    }
    let (root, ws) = project("elbow");
    write(&root, "incoming/elbow.msh", std::fs::read(&path).unwrap());
    let r = call(&ws, "import_mesh", json!({"path": "incoming/elbow.msh"})).unwrap();
    assert_eq!(r["format"], "fluent");
    assert_eq!(r["cells"], 0x396);
    let mesh = rcfd_io::read_msh_file(&path).unwrap();
    assert_eq!(import_view(&r), reader_view(&mesh));
    let names: Vec<&str> = r["patches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    for want in [
        "wall-4",
        "velocity-inlet-5",
        "velocity-inlet-6",
        "pressure-outlet-7",
        "wall-8",
        "frontAndBack",
    ] {
        assert!(names.contains(&want), "{want} missing in {names:?}");
    }
    eprintln!("C8-I elbow: {} cells", r["cells"]);
}

/// M2 for `check_case_mesh`: a mesh file outside the project (relative and absolute) is refused with 1004 located at
/// `mesh.file`; a missing one is 1003 at `mesh.file`; an absolute path inside the project is accepted.
#[test]
fn c8c_mesh_file_confinement() {
    let (root, ws) = project("confine");
    write(
        &root,
        "meshes/a.msh",
        std::fs::read(sample("hex_v4.1_ascii.msh")).unwrap(),
    );
    let outside = root
        .parent()
        .unwrap()
        .join(format!("f272-out-{}.msh", std::process::id()));
    std::fs::write(&outside, std::fs::read(sample("hex_v4.1_ascii.msh")).unwrap()).unwrap();
    let case = |file: &str| {
        format!(
            "[mesh]\nsource = \"file\"\nfile = {}\n",
            serde_json::to_string(file).unwrap()
        )
    };
    write(&root, "cases/rel_out.toml", case("../../outside.msh"));
    write(&root, "cases/abs_out.toml", case(&outside.to_string_lossy()));
    write(&root, "cases/missing.toml", case("../meshes/missing.msh"));
    write(&root, "cases/ok.toml", case("../meshes/a.msh"));
    let inside = root.join("meshes/a.msh");
    write(&root, "cases/abs_in.toml", case(&inside.to_string_lossy()));
    for (c, code, at) in [
        ("cases/rel_out.toml", 1004, "mesh.file"),
        ("cases/abs_out.toml", 1004, "mesh.file"),
        ("cases/missing.toml", 1003, "mesh.file"),
    ] {
        let e = call(&ws, "check_case_mesh", json!({"case": c})).unwrap_err();
        assert_eq!(e.code, code, "{c}: {e:?}");
        assert_eq!(diag_paths(&e), [at], "{c}: {e:?}");
        let e = answer(&ws, "preview_mesh", json!({"case": c})).unwrap_err();
        assert_eq!(e.code, code, "preview {c}: {e:?}");
    }
    for c in ["cases/ok.toml", "cases/abs_in.toml"] {
        let r = call(&ws, "check_case_mesh", json!({"case": c})).unwrap();
        assert_eq!(r["ok"], true, "{c}");
        assert_eq!(r["mesh"], json!({"source": "file", "file": "meshes/a.msh"}), "{c}");
        assert_eq!(r["report"]["n_cells"], 64);
        assert_eq!(r["patches_total"], 3);
    }
    let _ = std::fs::remove_file(&outside);
    eprintln!("C8-C confinement: 2 escapes 1004, missing 1003, relative and absolute inside accepted");
}

/// C8-E: `edit_case` with mesh settings builds and checks the edited mesh before writing.
#[test]
fn c8e_edit_case_with_mesh_settings() {
    let (root, ws) = project("edit");
    write(&root, "cases/cavity.toml", CAVITY);
    let hash = |root: &Path| rcfd_project::records::hash_bytes(&std::fs::read(root.join("cases/cavity.toml")).unwrap());
    let journal = |root: &Path| std::fs::read(root.join(".rcfd/edits.jsonl")).unwrap_or_default();
    // control: a non-mesh edit has exactly the keys of before F-272
    let h = hash(&root);
    let r = call(
        &ws,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"physics.nu": 0.02}}),
    )
    .unwrap();
    let mut keys: Vec<&String> = r.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        ["form", "new_hash", "old_hash", "path", "valid", "values", "written"]
    );
    // a mesh setting: written, with the check
    let h = hash(&root);
    let r = call(
        &ws,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"mesh.nx": 10}}),
    )
    .unwrap();
    assert_eq!(r["written"], true);
    assert_eq!(r["mesh"]["ok"], true, "{r}");
    assert_eq!(r["mesh"]["report"]["n_cells"], 120);
    assert!(std::fs::read_to_string(root.join("cases/cavity.toml"))
        .unwrap()
        .contains("# a comment edit_case keeps"));
    // dry run with a mesh key: the check, nothing written
    let h = hash(&root);
    let j = journal(&root);
    let r = call(
        &ws,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h, "set": {"mesh.ny": 6}, "dry_run": true}),
    )
    .unwrap();
    assert_eq!(r["written"], false);
    assert_eq!(r["mesh"]["report"]["n_cells"], 60);
    assert_eq!((hash(&root), journal(&root)), (h.clone(), j));
    // refusals: nothing written, no journal line
    let outside = root
        .parent()
        .unwrap()
        .join(format!("f272-edit-out-{}.msh", std::process::id()));
    std::fs::write(&outside, std::fs::read(sample("hex_v4.1_ascii.msh")).unwrap()).unwrap();
    let refusals = [
        (json!({"mesh.grading": 2.0}), 1003, "mesh.grading"),
        (
            json!({"mesh.source": "file", "mesh.file": "../meshes/missing.msh"}),
            1003,
            "mesh.file",
        ),
        (
            json!({"mesh.source": "file", "mesh.file": "../../outside.msh"}),
            1004,
            "mesh.file",
        ),
        (
            json!({"mesh.source": "file", "mesh.file": outside.to_string_lossy()}),
            1004,
            "mesh.file",
        ),
    ];
    let bytes = std::fs::read(root.join("cases/cavity.toml")).unwrap();
    let j = journal(&root);
    for (set, code, at) in &refusals {
        let e = call(
            &ws,
            "edit_case",
            json!({"path": "cases/cavity.toml", "base_hash": h, "set": set}),
        )
        .unwrap_err();
        assert_eq!(e.code, *code, "{set}: {e:?}");
        assert!(diag_paths(&e).iter().any(|p| p == at), "{set}: {e:?}");
        assert_eq!(std::fs::read(root.join("cases/cavity.toml")).unwrap(), bytes, "{set}");
        assert_eq!(journal(&root), j, "{set}");
    }
    let _ = std::fs::remove_file(&outside);
    // Q2 (a): a mesh that builds but fails the check is written, with mesh.ok false
    write(&root, "meshes/sheared.msh", channel_msh(8, 4, 2.0, 1.0, 20.0));
    let r = call(
        &ws,
        "edit_case",
        json!({"path": "cases/cavity.toml", "base_hash": h,
            "set": {"mesh.source": "file", "mesh.file": "../meshes/sheared.msh"}}),
    )
    .unwrap();
    assert_eq!(r["written"], true);
    assert_eq!(r["mesh"]["ok"], false, "{r}");
    assert!(!r["mesh"]["report"]["errors"].as_array().unwrap().is_empty());
    let maxno = r["mesh"]["report"]["max_non_orthogonality"].as_f64().unwrap();
    assert!(maxno > 85.0, "{maxno}");
    eprintln!(
        "C8-E: nx 10 -> 120 cells; {} refusals with nothing written; sheared mesh written with ok false (max non-orthogonality {maxno:.1} deg)",
        refusals.len()
    );
}

fn built(root: &Path, rel: &str) -> Mesh {
    let path = root.join(rel);
    let case = Case::load(&path).unwrap();
    rcfd_api::sim::build_mesh(&case).unwrap()
}

/// The per-cell value of M4, computed here from `compute_metrics`.
fn per_cell(mesh: &Mesh, measure: &str) -> Vec<f64> {
    let m = compute_metrics(mesh);
    match measure {
        "aspect_ratio" => m.aspect_ratio,
        _ => {
            let face = if measure == "skewness" {
                &m.skewness
            } else {
                &m.non_orthogonality
            };
            (0..mesh.n_cells())
                .map(|c| {
                    let mut x = 0.0f64;
                    for &f in mesh.cell_faces(c) {
                        if face[f] > x {
                            x = face[f];
                        }
                    }
                    x
                })
                .collect()
        }
    }
}

fn reference(mesh: &Mesh, values: &[f64], width: usize, plane: Option<(usize, f64)>, range: (f64, f64)) -> String {
    let opt = RenderOptions {
        width,
        axis: plane.map(|p| p.0),
        slice: plane.map(|p| p.1),
        range: Some(range),
        ..Default::default()
    };
    base64(&render_scalar(mesh, values, &opt).unwrap().image.to_png().unwrap())
}

/// C8-P: the preview against the CPU renderer on the same mesh, values, plane and range.
#[test]
fn c8p_preview_matches_the_cpu_reference() {
    let (root, ws) = project("preview");
    write(&root, "cases/wedge.toml", WEDGE);
    write(&root, "cases/cavity.toml", CAVITY);
    write(&root, "cases/annulus.toml", ANNULUS);
    // the wedge, default measure
    let mesh = built(&root, "cases/wedge.toml");
    let values = per_cell(&mesh, "non_orthogonality");
    let (lo, hi) = values
        .iter()
        .fold((f64::INFINITY, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
    let (png, meta) = image(&ws, json!({"case": "cases/wedge.toml"}));
    let q = check_mesh(&mesh);
    assert_eq!(meta["measure"], "non_orthogonality");
    assert_eq!(meta["max"].as_f64().unwrap(), q.max_non_orthogonality);
    assert_eq!(meta["max"].as_f64().unwrap(), hi);
    assert_eq!(meta["min"].as_f64().unwrap(), lo);
    assert!(hi > 1.0, "the wedge is skewed: {hi}");
    assert_eq!(meta["range"], json!([0.0, hi]));
    assert_eq!(meta["cells"], mesh.n_cells());
    assert_eq!(meta["cells_drawn"], mesh.n_cells());
    assert_eq!(meta["axis"], "z");
    let plane = (2, meta["slice"].as_f64().unwrap());
    assert_eq!(png, reference(&mesh, &values, 640, Some(plane), (0.0, hi)));
    assert_eq!(
        png,
        reference(&mesh, &values, 640, None, (0.0, hi)),
        "the renderer's own plane rule"
    );
    assert_eq!(meta["width"], 640);
    assert!(meta.get("reduced_from").is_none());
    // control: another measure draws another image
    let (png_ar, meta_ar) = image(&ws, json!({"case": "cases/wedge.toml", "measure": "aspect_ratio"}));
    assert_ne!(png_ar, png);
    assert_eq!(meta_ar["range"][0], 1.0);
    // the uniform cavity: one colour, range [0, 1]
    let (_, meta_c) = image(&ws, json!({"case": "cases/cavity.toml"}));
    assert!(meta_c["max"].as_f64().unwrap().abs() < 1e-9, "{meta_c}");
    assert!(meta_c["min"].as_f64().unwrap().abs() < 1e-9, "{meta_c}");
    assert_eq!(meta_c["range"], json!([0.0, 1.0]));
    // a 3D annulus: a slice across z
    let mesh3 = built(&root, "cases/annulus.toml");
    let (png3, meta3) = image(&ws, json!({"case": "cases/annulus.toml"}));
    let drawn = meta3["cells_drawn"].as_u64().unwrap() as usize;
    assert!(drawn > 0 && drawn < mesh3.n_cells(), "{meta3}");
    assert_eq!(meta3["axis"], "z");
    let v3 = per_cell(&mesh3, "non_orthogonality");
    let hi3 = v3.iter().copied().fold(0.0, f64::max);
    let range3 = if hi3 <= 1e-9 { (0.0, 1.0) } else { (0.0, hi3) };
    assert_eq!(
        png3,
        reference(&mesh3, &v3, 640, Some((2, meta3["slice"].as_f64().unwrap())), range3)
    );
    // width 4096: within the budget, narrower if needed
    let (png_w, meta_w) = image(&ws, json!({"case": "cases/wedge.toml", "width": 4096}));
    assert!(png_w.len() + meta_w.to_string().len() <= TEXT_BUDGET);
    match meta_w.get("reduced_from") {
        Some(r) => {
            assert_eq!(r, 4096);
            assert!(meta_w["width"].as_u64().unwrap() < 4096);
            eprintln!("C8-P width 4096: drawn at {} px", meta_w["width"]);
        }
        None => {
            assert_eq!(meta_w["width"], 4096);
            eprintln!("C8-P width 4096: fits at 4096 px");
        }
    }
    // a plane outside the mesh
    let e = answer(&ws, "preview_mesh", json!({"case": "cases/wedge.toml", "slice": 10.0})).unwrap_err();
    assert_eq!(e.code, 1003, "{e:?}");
    eprintln!(
        "C8-P: wedge max non-orthogonality {hi:.3} deg = the report's; png {} base64 bytes equal the reference; annulus {drawn} of {} cells drawn",
        png.len(),
        mesh3.n_cells()
    );
}

/// C8-L: `meshes/` with the cases that use each mesh, references elsewhere, unread cases, paging.
#[test]
fn c8l_list_meshes() {
    let (root, ws) = project("list");
    write(
        &root,
        "meshes/a.msh",
        std::fs::read(sample("hex_v2.2_ascii.msh")).unwrap(),
    );
    write(
        &root,
        "meshes/b.msh",
        std::fs::read(sample("prism_v2.2_ascii.msh")).unwrap(),
    );
    write_foam(&root.join("meshes/foam"));
    let file_case = |f: &str| format!("[mesh]\nsource = \"file\"\nfile = \"{f}\"\n");
    write(&root, "cases/x.toml", file_case("../meshes/a.msh"));
    write(&root, "cases/sub/y.toml", file_case("../../meshes/a.msh"));
    write(
        &root,
        "cases/z.json",
        r#"{"mesh": {"source": "file", "file": "../meshes/foam"}}"#,
    );
    write(
        &root,
        "cases/local.msh",
        std::fs::read(sample("hex_v2.2_ascii.msh")).unwrap(),
    );
    write(&root, "cases/w.toml", file_case("local.msh"));
    write(&root, "cases/v.toml", file_case("../../outside.msh"));
    write(&root, "cases/u.toml", "this is = = not toml\n");
    write(&root, "cases/cavity.toml", CAVITY);
    let r = call(&ws, "list_meshes", json!({})).unwrap();
    let rows: Vec<(String, String, Vec<String>)> = r["meshes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["path"].as_str().unwrap().to_string(),
                m["kind"].as_str().unwrap().to_string(),
                m["used_by"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| c.as_str().unwrap().to_string())
                    .collect(),
            )
        })
        .collect();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        rows,
        vec![
            (
                "meshes/a.msh".into(),
                "file".into(),
                s(&["cases/sub/y.toml", "cases/x.toml"])
            ),
            ("meshes/b.msh".into(), "file".into(), vec![]),
            ("meshes/foam".into(), "openfoam".into(), s(&["cases/z.json"])),
        ],
        "{r}"
    );
    assert!(r["meshes"][2]["files"].as_u64().unwrap() >= 5);
    assert_eq!(r["total"], 3);
    assert_eq!(r["next_offset"], Value::Null);
    let out = r["outside"].as_array().unwrap();
    assert_eq!(out.len(), 2, "{r}");
    assert_eq!(out[0]["case"], "cases/v.toml");
    assert_eq!(out[0]["path"], Value::Null);
    assert!(out[0]["refused"]
        .as_str()
        .unwrap()
        .contains("leaves the project folder"));
    assert_eq!(out[1]["case"], "cases/w.toml");
    assert_eq!(out[1]["path"], "cases/local.msh");
    assert_eq!(out[1]["exists"], true);
    assert_eq!(out[1]["refused"], Value::Null);
    let unread = r["unread"].as_array().unwrap();
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0]["path"], "cases/u.toml");
    // paging
    let p = call(&ws, "list_meshes", json!({"limit": 1})).unwrap();
    assert_eq!(p["meshes"].as_array().unwrap().len(), 1);
    assert_eq!(p["meshes"][0]["path"], "meshes/a.msh");
    assert_eq!(p["next_offset"], 1);
    let p = call(&ws, "list_meshes", json!({"offset": 1, "limit": 5})).unwrap();
    assert_eq!(p["meshes"][0]["path"], "meshes/b.msh");
    eprintln!("C8-L: 3 meshes (one OpenFOAM folder), 2 references elsewhere, 1 unread case");
}
