//! Shared set-up of the ADR 0138 tests (wave 48G1, the mesh preview of a case): projects with case files, a Studio
//! showing a case, surface comparison.
#![allow(dead_code)]

use rcfd_project::Project;
use rcfd_render::SurfaceMesh;
use rcfd_studio::preview::{Built, Status};
use rcfd_studio::{Studio, StudioApp};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const WAIT: Duration = Duration::from_secs(60);

/// The 16 x 16 lid-driven cavity (2D; patches `lid`, `walls`, `frontAndBack`).
pub const CAVITY: &str = include_str!("../fixtures/cavity.toml");

/// A 4 x 4 x 4 hex box on [0, 2] x [0, 1] x [0, 1] in Gmsh 2.2 ASCII (copied from
/// `crates/rcfd-io/tests/data/hex_v2.2_ascii.msh`): patches `inlet` (x = 0), `outlet` (x = 2), `walls` (the other 4 sides).
pub const HEX_MSH: &[u8] = include_bytes!("../fixtures/hex_v2.2_ascii.msh");

/// A case on the hex box, by mesh file relative to the case's folder (`cases/`).
pub const HEX_CASE: &str = r#"title = "box from a Gmsh file (ADR 0138 fixture)"
[mesh]
source = "file"
file = "../meshes/hex.msh"
[physics]
nu = 0.01
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 5
"#;

/// A `box_2d` case of `nx` x `ny` cells on the unit square (patches `xmin`, `xmax`, `ymin`, `ymax`, `frontAndBack`).
pub fn box_case(nx: usize, ny: usize) -> String {
    format!(
        "title = \"box {nx} x {ny}\"\n[mesh]\ngenerator = \"box_2d\"\nnx = {nx}\nny = {ny}\nlx = 1.0\nly = 1.0\n\
         [physics]\nnu = 0.01\n[solver]\nalgorithm = \"simplec\"\n"
    )
}

/// A fresh, empty temporary folder unique to this process and call.
pub fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir()
        .join("rcfd-studio-tests")
        .join(format!("{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A project with `cases/cavity.toml`, `cases/hex.toml`, `meshes/hex.msh` and the extra files given (project-relative
/// path, content). Returns the canonical root.
pub fn project(name: &str, extra: &[(&str, &[u8])]) -> PathBuf {
    let root = temp(name).join("proj");
    Project::create(&root, name, "rcfd test").unwrap();
    std::fs::write(root.join("cases/cavity.toml"), CAVITY).unwrap();
    std::fs::write(root.join("cases/hex.toml"), HEX_CASE).unwrap();
    std::fs::write(root.join("meshes/hex.msh"), HEX_MSH).unwrap();
    for (rel, bytes) in extra {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    rcfd_project::paths::canonicalize(&root).unwrap()
}

/// The repository root (the shipped `cases/` folder is there).
pub fn repo_root() -> PathBuf {
    rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// A Studio (watcher and run worker off) with the project at `root` open.
pub fn studio(root: &Path) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.open_project(root);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some()),
        "the project did not open"
    );
    st
}

/// True when the preview is no longer building.
pub fn settled(st: &Studio) -> bool {
    !matches!(st.preview.status, Status::Building { .. } | Status::Idle)
}

/// Open case `rel` and drain until its preview settled; the status.
pub fn show_case(st: &mut Studio, rel: &str) -> Status {
    st.open_case(rel);
    assert!(
        st.drain_until(WAIT, |s| s.preview.case.as_deref() == Some(rel) && settled(s)),
        "the preview of {rel} did not settle: {:?}",
        st.preview.status
    );
    st.preview.status.clone()
}

/// The built preview (panics when there is none).
pub fn built(st: &Studio) -> std::sync::Arc<Built> {
    st.preview
        .built
        .clone()
        .unwrap_or_else(|| panic!("no preview built: {:?}", st.preview.status))
}

/// Field-by-field, bit-for-bit comparison of two surfaces; the first difference.
pub fn same_surface(a: &SurfaceMesh, b: &SurfaceMesh) -> Result<(), String> {
    let bits = |s: &SurfaceMesh| -> Vec<u64> { s.points.iter().flat_map(|p| p.map(f64::to_bits)).collect() };
    if bits(a) != bits(b) {
        return Err(format!("points differ ({} vs {})", a.points.len(), b.points.len()));
    }
    if a.face_offsets != b.face_offsets || a.face_points != b.face_points {
        return Err("face loops differ".into());
    }
    if a.face_part != b.face_part || a.parts != b.parts {
        return Err(format!("parts differ ({:?} vs {:?})", a.parts, b.parts));
    }
    if a.mesh_face != b.mesh_face || a.cell != b.cell || a.point_id != b.point_id {
        return Err("mesh face, owner cell or point ids differ".into());
    }
    Ok(())
}

/// The patch of volume-mesh face `f` from the patch ranges (independent of the surface's part numbering).
pub fn patch_by_range(mesh: &rcfd_mesh::Mesh, f: usize) -> Option<String> {
    mesh.patches()
        .iter()
        .find(|p| (p.start..p.start + p.size).contains(&f))
        .map(|p| p.name.clone())
}

/// Centroid of surface face `f`.
pub fn centroid(s: &SurfaceMesh, f: usize) -> [f64; 3] {
    let l = s.face(f);
    let mut c = [0.0; 3];
    for &p in l {
        for d in 0..3 {
            c[d] += s.points[p][d] / l.len() as f64;
        }
    }
    c
}

/// One headless frame of the app at 1280 x 800 points with extra events; the drawn text and the frame's wall time
/// (`run_ui` plus tessellation, ms).
pub fn timed_frame(app: &mut StudioApp, ctx: &egui::Context, events: Vec<egui::Event>) -> (Vec<String>, f64) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        events,
        ..Default::default()
    };
    let t = Instant::now();
    let out = ctx.run_ui(input, |ui| app.ui(ui));
    let shapes = out.shapes.clone();
    let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    (texts(&shapes), ms)
}

/// The text of every drawn text shape.
pub fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Median of a list (mean of the two middle values for an even count).
pub fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n.is_multiple_of(2) {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    } else {
        s[n / 2]
    }
}
