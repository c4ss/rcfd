//! Mesh preview from a case (ADR 0138, wave 48G1; live from the case editor since ADR 0144): the viewport's Mesh side
//! shows the boundary surface of the mesh the solver would build for the open case, without running the solver.
//!
//! * The source is the case file on disk while the case editor holds no unsaved edit of it, else the editor's text: the
//!   bytes a save would write now, with the text of the field being typed put in its place (ADR 0144, the maintainer's
//!   answer to ADR 0138 question 2). Either is parsed as `rcfd run <snapshot> --case-dir <the case's folder>` parses it
//!   (the runner's rule: JSON when the text starts with `{`, else TOML; relative paths against the case's folder), the
//!   mesh is built by `rcfd_api::sim::build_mesh` and its surface by [`crate::viewport::surface_of`], the run viewport's
//!   rule (a 2D case is its +z `Empty` faces).
//! * An edit or a keystroke in the editor asks for a build [`DEBOUNCE`] after the last one; opening a case, a change on
//!   disk, "Rebuild" and "anyway" ask at once.
//! * Builds run on a thread per request ([`build`] and [`build_source`] are the code it runs, callable from tests); at
//!   most [`MAX_BUILDS`] are alive at once, a request beyond waits with its source. Each request has a generation
//!   number: a newer request makes older builds stop at their next phase boundary and their results are discarded (a
//!   running `build_mesh` call cannot be stopped; "cancelled" means "discarded", ADR 0138). Nothing is joined on the UI
//!   thread.
//! * Size limits ([`Limits`]): a cell estimate before building (recipe meshes), the file size of a mesh file, and the
//!   surface triangles before the upload, which is the one GPU step on the UI thread. Over a limit the preview says so
//!   and offers to go on anyway. The limits hold for the editor's text as for the file.
//! * A build that fails or is over a limit leaves the last mesh that built for the case on screen, marked stale
//!   ([`Preview::stale`]), with the located reason (ADR 0144).
//! * Drawing reuses the run viewport ([`crate::viewport::Viewport::show_surface`]): the same `ViewGpu`, texture, camera
//!   mapping and pick pass, every face at value 0 and one solid colour per patch (`PartStyle::Solid`). A 2D case's
//!   boundary patches are edges in its face-on view; they are drawn as coloured segments over the image by
//!   [`crate::preview_ui`], with the cell edges of the front when there are at most [`Limits::overlay_edges`].

use crate::schema_model::{parse_input, Kind};
use crate::studio::Studio;
use crate::viewport::surface_of;
use crate::watch::Wake;
use rcfd_api::case::{Case, Diagnostic};
use rcfd_core::vec3::Vec3;
use rcfd_mesh::{Mesh, PatchKind};
use rcfd_render::camera::{mat_vec, Camera};
use rcfd_render::{PartStyle, SurfaceMesh};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default cell limit of the estimate made before building a recipe mesh (ADR 0138 G-limits).
pub const MAX_CELLS: u64 = 2_000_000;
/// Default size limit of a mesh file (`mesh.source = "file"`), bytes.
pub const MAX_FILE_BYTES: u64 = 256 << 20;
/// Default limit of surface triangles uploaded on the UI thread (ADR 0138: from ADR 0125 T1, 146-205 ms per 1M).
pub const MAX_TRIANGLES: usize = 250_000;
/// Default limit of the 2D cell edges drawn over the image.
pub const MAX_OVERLAY_EDGES: usize = 20_000;
/// Build threads alive at once; a request beyond waits for one to end.
pub const MAX_BUILDS: usize = 2;
/// Time after the last edit or keystroke in the case editor before the preview builds the editor's text (ADR 0144; the
/// maintainer's range was 400-800 ms).
pub const DEBOUNCE: Duration = Duration::from_millis(500);
/// Patch colours, by mesh patch index (repeating).
pub const PALETTE: [[u8; 3]; 10] = [
    [31, 119, 180],
    [255, 127, 14],
    [44, 160, 44],
    [214, 39, 40],
    [148, 103, 189],
    [140, 86, 75],
    [227, 119, 194],
    [127, 127, 127],
    [188, 189, 34],
    [23, 190, 207],
];
/// Colour of every face in "single colour" mode.
pub const SINGLE: [u8; 3] = [200, 200, 200];
/// Colour of the drawn front of a 2D case.
pub const FRONT_2D: [u8; 3] = [222, 222, 222];

/// Size limits of the preview (the app uses [`Limits::default`]; tests lower them to reach the "anyway" paths).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Estimated cells before building a recipe mesh.
    pub cells: u64,
    /// Bytes of a mesh file.
    pub file_bytes: u64,
    /// Surface triangles before the upload.
    pub triangles: usize,
    /// 2D cell edges kept for the overlay.
    pub overlay_edges: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            cells: MAX_CELLS,
            file_bytes: MAX_FILE_BYTES,
            triangles: MAX_TRIANGLES,
            overlay_edges: MAX_OVERLAY_EDGES,
        }
    }
}

/// One boundary patch of the previewed mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchInfo {
    /// Patch name.
    pub name: String,
    /// Patch kind.
    pub kind: PatchKind,
    /// Boundary faces of the patch in the volume mesh.
    pub faces: usize,
    /// Its colour (faces in 3D, outline in 2D).
    pub colour: [u8; 3],
    /// True when its faces are part of the drawn surface (every patch in 3D, the front `Empty` patches in 2D).
    pub drawn: bool,
}

/// A built preview.
#[derive(Debug, Clone)]
pub struct Built {
    /// Project-relative case file.
    pub rel: String,
    /// The case file (read, or the one the editor's text is of).
    pub file: PathBuf,
    /// `hash_bytes` of the text built: the case file as read, or the editor's text.
    pub case_hash: String,
    /// True when built from the case editor's unsaved text (ADR 0144), false from the file on disk.
    pub from_editor: bool,
    /// `Mesh::content_hash` of the built mesh.
    pub mesh_hash: u64,
    /// Cells of the volume mesh.
    pub cells: usize,
    /// Faces of the volume mesh.
    pub faces: usize,
    /// Points of the volume mesh.
    pub points: usize,
    /// The drawn surface ([`surface_of`]).
    pub surface: Arc<SurfaceMesh>,
    /// True for a 2D case.
    pub two_d: bool,
    /// Every mesh patch, in mesh order.
    pub patches: Vec<PatchInfo>,
    /// Mesh patch index of each surface part.
    pub part_patch: Vec<usize>,
    /// Fan triangles of the surface (what the upload draws).
    pub triangles: usize,
    /// 2D: unique edges of the drawn faces (empty when 3D or when more than [`Limits::overlay_edges`]).
    pub edges: Arc<Vec<[Vec3; 2]>>,
    /// 2D: number of unique edges of the drawn faces (also when not kept).
    pub n_edges: usize,
    /// 2D: `(patch, segment)`, the front edge of each face of every patch that is not drawn (the boundaries).
    pub outlines: Arc<Vec<(usize, [Vec3; 2])>>,
    /// The mesh file when `mesh.source = "file"` (absolute).
    pub mesh_file: Option<PathBuf>,
    /// Wall time of the build, seconds.
    pub build_s: f64,
}

/// Why a build gave no preview.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// Over the cell or file-size limit before building (located message; "build anyway" goes on).
    TooLarge(String),
    /// A located error.
    Failed(String),
    /// A newer request came first.
    Cancelled,
}

/// What a build returns: the hash of the bytes read (when the file was read) and the result.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// `hash_bytes` of the case file, when it was read from disk (`None` for the editor's text).
    pub read_hash: Option<String>,
    /// True when the build called `build_mesh` (a refusal before building, or a parse failure, did not).
    pub called_build_mesh: bool,
    /// True when the source was the case editor's text (ADR 0144).
    pub from_editor: bool,
    /// The preview or why there is none.
    pub result: Result<Built, Refusal>,
}

/// Size of a mesh as known before building it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimate {
    /// Cells, from the recipe's counts.
    Cells(u64),
    /// Bytes of the mesh file.
    FileBytes(u64),
    /// Not known before building (this generator's counts are not a closed form here).
    Unknown,
}

/// The text of a case-editor field as it is being typed, not committed yet (ADR 0144).
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    /// Settings path (`mesh.nx`).
    pub path: String,
    /// The field's kind in the settings schema (`None`: not a setting).
    pub kind: Option<Kind>,
    /// The text as typed.
    pub text: String,
}

/// The case editor's unsaved state of a case (ADR 0144).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorText {
    /// The editor's settings tree (`CaseEditor::settings`).
    pub settings: Value,
    /// The field being typed in, put in place of its value.
    pub draft: Option<Draft>,
    /// The case file is JSON (a save writes JSON), else TOML.
    pub json: bool,
}

/// What a build reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// The case file on disk.
    Disk,
    /// The case editor's unsaved text (ADR 0144).
    Editor(Arc<EditorText>),
}

fn int(case: &Case, path: &str) -> u64 {
    case.get(path).ok().and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64
}

fn text(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// The mesh file of a case with `mesh.source = "file"`, resolved as `build_mesh` resolves it.
pub fn mesh_file_of(case: &Case) -> Option<PathBuf> {
    if text(case, "mesh.source") != "file" {
        return None;
    }
    let f = text(case, "mesh.file");
    Some(if Path::new(&f).is_absolute() {
        PathBuf::from(f)
    } else {
        case.base_dir.join(f)
    })
}

/// The size of the case's mesh before it is built: the product of the recipe's cell counts for the generators whose
/// count is that product (checked against the built mesh by a test, ADR 0138), the file size for a mesh file.
pub fn estimate(case: &Case) -> Estimate {
    if let Some(f) = mesh_file_of(case) {
        return match std::fs::metadata(&f) {
            Ok(m) if m.is_file() => Estimate::FileBytes(m.len()),
            _ => Estimate::Unknown,
        };
    }
    let (nx, ny, nz) = (int(case, "mesh.nx"), int(case, "mesh.ny"), int(case, "mesh.nz").max(1));
    let right = |key: &str, same: u64| match int(case, key) {
        0 => same,
        n => n,
    };
    let (nxr, nyr, nzr) = (
        right("mesh.nx_right", nx),
        right("mesh.ny_right", ny),
        right("mesh.nz_right", nz),
    );
    let m = |a: u64, b: u64, c: u64| a.saturating_mul(b).saturating_mul(c);
    match text(case, "mesh.generator").as_str() {
        "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" => Estimate::Cells(m(nx, ny, 1)),
        "channel_2d_interface" => Estimate::Cells(m(nx, ny, 1).saturating_add(m(nxr, nyr, 1))),
        "annulus_3d" | "annulus_3d_sector" => Estimate::Cells(m(nx, ny, nz)),
        "annulus_3d_interface" => Estimate::Cells(m(nx, ny, nz).saturating_add(m(nx, nyr, nzr))),
        "annulus_3d_stage" | "annulus_3d_cylindrical_interface" => {
            Estimate::Cells(m(nx, ny, nz).saturating_add(m(nxr, nyr, nzr)))
        }
        _ => Estimate::Unknown,
    }
}

/// Diagnostics of `rel`, one per line, each prefixed with the file (`cases/a.toml: line 3: mesh.nx: ...`).
pub fn located(rel: &str, diags: &[Diagnostic]) -> String {
    if diags.is_empty() {
        return format!("{rel}: refused without a diagnostic");
    }
    diags
        .iter()
        .map(|d| format!("{rel}: {d}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse case text as `rcfd run <file> --case-dir <dir>` does: JSON when it starts with `{`, else TOML, relative paths
/// against `dir`.
pub fn parse_case(text: &str, dir: &Path) -> Result<Case, Vec<Diagnostic>> {
    if text.trim_start().starts_with('{') {
        Case::parse_json_with(text, dir, None)
    } else {
        Case::parse_toml_with(text, dir, None)
    }
}

/// A benchmark descriptor (a `cases/<id>/case.json` of the repository, read by `rcfd cases`) is not a case file:
/// the short message for it (ADR 0144, the maintainer's answer to ADR 0138 question 7), else `None`. The rule is the
/// content, asked only after the case parse failed: a JSON object whose top level has a `reference` table and an `id`
/// text or a `roadmap` list, keys no case file has.
pub fn descriptor_message(rel: &str, text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let o = v.as_object()?;
    if !o.get("reference").is_some_and(Value::is_object) {
        return None;
    }
    let id = o.get("id").and_then(Value::as_str);
    if id.is_none() && !o.get("roadmap").is_some_and(Value::is_array) {
        return None;
    }
    Some(format!(
        "{rel}: not a case file: this is a benchmark descriptor{} (the reference data and tolerances of a validation \
         case, read by rcfd cases), which rcfd run refuses too; Case > New case from example... makes a case file",
        id.map_or(String::new(), |i| format!(" (id \"{i}\")"))
    ))
}

/// Put `value` at the dotted settings `path` of a settings tree (tables made on the way).
fn set_at(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let segs: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    let Some((last, parents)) = segs.split_last() else {
        return Err("an empty settings path".into());
    };
    let mut cur = root;
    for s in parents {
        let Some(m) = cur.as_object_mut() else {
            return Err(format!("{path}: not a table"));
        };
        cur = m
            .entry((*s).to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    match cur.as_object_mut() {
        Some(m) => {
            m.insert((*last).to_string(), value);
            Ok(())
        }
        None => Err(format!("{path}: not a table")),
    }
}

/// The bytes a save of the case editor would write now, with the field being typed put in place of its value: the
/// settings checked and resolved against the case's folder `case_dir` (`Case::from_json_in`), written as TOML (JSON for
/// a `.json` case, as `CaseEditor::to_bytes`). A located message when the typed text or the settings are refused.
pub fn editor_bytes(rel: &str, case_dir: &Path, e: &EditorText) -> Result<Vec<u8>, String> {
    let mut tree = e.settings.clone();
    if let Some(d) = &e.draft {
        let Some(kind) = &d.kind else {
            return Err(format!("{rel}: {}: unknown setting", d.path));
        };
        let v = parse_input(kind, &d.text).map_err(|m| format!("{rel}: {}: {m}", d.path))?;
        set_at(&mut tree, &d.path, v).map_err(|m| format!("{rel}: {m}"))?;
    }
    let case = Case::from_json_in(&tree, case_dir).map_err(|d| located(rel, &d))?;
    Ok(if e.json { case.to_json_string() } else { case.to_toml() }.into_bytes())
}

/// A project-relative path with plain components only (no root, drive, `.` or `..`).
fn plain(rel: &str) -> bool {
    !rel.is_empty() && Path::new(rel).components().all(|c| matches!(c, Component::Normal(_)))
}

fn with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Every patch of the mesh with its colour and whether the drawn surface holds it.
pub fn patch_list(mesh: &Mesh, surface: &SurfaceMesh) -> (Vec<PatchInfo>, Vec<usize>) {
    let patches: Vec<PatchInfo> = mesh
        .patches()
        .iter()
        .enumerate()
        .map(|(k, p)| PatchInfo {
            name: p.name.clone(),
            kind: p.kind.clone(),
            faces: p.size,
            colour: PALETTE[k % PALETTE.len()],
            drawn: surface.parts.contains(&p.name),
        })
        .collect();
    let part_patch = surface
        .parts
        .iter()
        .map(|n| patches.iter().position(|p| p.name == *n).unwrap_or(0))
        .collect();
    (patches, part_patch)
}

/// 2D overlay geometry: the unique edges of the drawn faces (kept when at most `max_edges`, always counted) and the
/// front edge of every face of the patches that are not drawn. `None` when cancelled.
#[allow(clippy::type_complexity)]
pub fn overlay_2d(
    mesh: &Mesh,
    surface: &SurfaceMesh,
    patches: &[PatchInfo],
    max_edges: usize,
    cancelled: &dyn Fn() -> bool,
) -> Option<(Vec<[Vec3; 2]>, usize, Vec<(usize, [Vec3; 2])>)> {
    let mut keys: Vec<u64> = Vec::with_capacity(surface.face_points.len());
    for f in 0..surface.n_faces() {
        let l = surface.face(f);
        for k in 0..l.len() {
            let (a, b) = (l[k], l[(k + 1) % l.len()]);
            let (a, b) = (a.min(b) as u64, a.max(b) as u64);
            keys.push((a << 32) | b);
        }
    }
    if cancelled() {
        return None;
    }
    keys.sort_unstable();
    keys.dedup();
    let n_edges = keys.len();
    let edges: Vec<[Vec3; 2]> = if n_edges <= max_edges {
        keys.iter()
            .map(|k| {
                let (a, b) = ((k >> 32) as usize, (k & 0xffff_ffff) as usize);
                [surface.points[a], surface.points[b]]
            })
            .collect()
    } else {
        Vec::new()
    };
    if cancelled() {
        return None;
    }
    let mut front = vec![false; mesh.n_points()];
    for &p in &surface.point_id {
        front[p] = true;
    }
    let pts = mesh.points();
    let mut outlines = Vec::new();
    for (k, p) in mesh.patches().iter().enumerate() {
        if patches[k].drawn {
            continue;
        }
        for f in p.faces() {
            let l = mesh.face_points(f);
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if front[a] && front[b] {
                    outlines.push((k, [pts[a], pts[b]]));
                }
            }
        }
    }
    Some((edges, n_edges, outlines))
}

/// Build the preview of the case file `root/rel` from disk (the code a build thread runs for [`Source::Disk`]).
/// `cancelled` is polled between phases; `force` skips the cell and file-size limits.
pub fn build(root: &Path, rel: &str, limits: &Limits, force: bool, cancelled: &dyn Fn() -> bool) -> Outcome {
    if !plain(rel) {
        return refused(rel, false);
    }
    let file = root.join(rel);
    match std::fs::read(&file) {
        Ok(bytes) => build_text(root, rel, &bytes, false, limits, force, cancelled),
        Err(e) => Outcome {
            read_hash: None,
            called_build_mesh: false,
            from_editor: false,
            result: Err(Refusal::Failed(format!("{rel}: {}: {e}", file.display()))),
        },
    }
}

/// Build the preview of case `rel` from `source` (the code a build thread runs): the file on disk ([`build`]) or the
/// case editor's text ([`editor_bytes`], against the case's folder), the same way from the text on.
pub fn build_source(
    root: &Path,
    rel: &str,
    source: &Source,
    limits: &Limits,
    force: bool,
    cancelled: &dyn Fn() -> bool,
) -> Outcome {
    let e = match source {
        Source::Disk => return build(root, rel, limits, force, cancelled),
        Source::Editor(e) => e,
    };
    if !plain(rel) {
        return refused(rel, true);
    }
    let file = root.join(rel);
    let dir = file.parent().map_or_else(|| root.to_path_buf(), Path::to_path_buf);
    match editor_bytes(rel, &dir, e) {
        Ok(bytes) => build_text(root, rel, &bytes, true, limits, force, cancelled),
        Err(m) => Outcome {
            read_hash: None,
            called_build_mesh: false,
            from_editor: true,
            result: Err(Refusal::Failed(m)),
        },
    }
}

fn refused(rel: &str, from_editor: bool) -> Outcome {
    Outcome {
        read_hash: None,
        called_build_mesh: false,
        from_editor,
        result: Err(Refusal::Failed(format!(
            "{rel}: refused: a case is named by a plain project-relative path"
        ))),
    }
}

/// The build from case text `bytes` of `root/rel` (read from disk, or the editor's text when `from_editor`).
fn build_text(
    root: &Path,
    rel: &str,
    bytes: &[u8],
    from_editor: bool,
    limits: &Limits,
    force: bool,
    cancelled: &dyn Fn() -> bool,
) -> Outcome {
    let t0 = Instant::now();
    let case_hash = rcfd_project::records::hash_bytes(bytes);
    let read_hash = (!from_editor).then(|| case_hash.clone());
    let out = |called_build_mesh: bool, result: Result<Built, Refusal>| Outcome {
        read_hash: read_hash.clone(),
        called_build_mesh,
        from_editor,
        result,
    };
    let file = root.join(rel);
    let src = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => return out(false, Err(Refusal::Failed(format!("{rel}: not UTF-8 text: {e}")))),
    };
    let dir = file.parent().map_or_else(|| root.to_path_buf(), Path::to_path_buf);
    let case = match parse_case(src, &dir) {
        Ok(c) => c,
        Err(mut d) => {
            if from_editor {
                // lines of the written text are not lines of anything the user sees
                d.iter_mut().for_each(|x| x.line = None);
            }
            let m = descriptor_message(rel, src).unwrap_or_else(|| located(rel, &d));
            return out(false, Err(Refusal::Failed(m)));
        }
    };
    if cancelled() {
        return out(false, Err(Refusal::Cancelled));
    }
    if !force {
        match estimate(&case) {
            Estimate::Cells(n) if n > limits.cells => {
                return out(false, Err(Refusal::TooLarge(format!(
                    "{rel}: about {} cells (from the mesh.n* counts of generator \"{}\"), more than the preview limit of {}",
                    with_commas(n),
                    text(&case, "mesh.generator"),
                    with_commas(limits.cells)
                ))))
            }
            Estimate::FileBytes(b) if b > limits.file_bytes => {
                return out(false, Err(Refusal::TooLarge(format!(
                    "{rel}: mesh.file {} is {} bytes, more than the preview limit of {}",
                    mesh_file_of(&case).unwrap_or_default().display(),
                    with_commas(b),
                    with_commas(limits.file_bytes)
                ))))
            }
            _ => {}
        }
    }
    let mesh = match rcfd_api::sim::build_mesh(&case) {
        Ok(m) => m,
        Err(d) => return out(true, Err(Refusal::Failed(located(rel, &d)))),
    };
    if cancelled() {
        return out(true, Err(Refusal::Cancelled));
    }
    let (surface, two_d) = match surface_of(&mesh) {
        Ok(s) => s,
        Err(e) => return out(true, Err(Refusal::Failed(format!("{rel}: surface: {e}")))),
    };
    if surface.n_faces() == 0 {
        return out(
            true,
            Err(Refusal::Failed(format!(
                "{rel}: the mesh has no boundary surface to draw"
            ))),
        );
    }
    let (patches, part_patch) = patch_list(&mesh, &surface);
    let (edges, n_edges, outlines) = if two_d {
        match overlay_2d(&mesh, &surface, &patches, limits.overlay_edges, cancelled) {
            Some(o) => o,
            None => return out(true, Err(Refusal::Cancelled)),
        }
    } else {
        (Vec::new(), 0, Vec::new())
    };
    let triangles = surface.triangles().count();
    out(
        true,
        Ok(Built {
            rel: rel.to_string(),
            file,
            case_hash,
            from_editor,
            mesh_hash: mesh.content_hash(),
            cells: mesh.n_cells(),
            faces: mesh.n_faces(),
            points: mesh.n_points(),
            surface: Arc::new(surface),
            two_d,
            patches,
            part_patch,
            triangles,
            edges: Arc::new(edges),
            n_edges,
            outlines: Arc::new(outlines),
            mesh_file: mesh_file_of(&case),
            build_s: t0.elapsed().as_secs_f64(),
        }),
    )
}

// ---------------------------------------------------------------- picking and overlay geometry

/// The patch of a drawn surface face (3D picking: the pick pass gives the face).
pub fn patch_of_face(b: &Built, face: usize) -> Option<(usize, &PatchInfo)> {
    let part = *b.surface.face_part.get(face)?;
    let k = *b.part_patch.get(part)?;
    b.patches.get(k).map(|p| (k, p))
}

/// Texture-pixel position of world points under `cam` for a `size` texture (`None` behind the eye), with the
/// world-to-clip matrix made once (the formula of `Camera::project`).
pub fn project_all(cam: &Camera, size: [u32; 2], pts: impl Iterator<Item = Vec3>) -> Vec<Option<[f64; 2]>> {
    let (w, h) = (size[0].max(1) as f64, size[1].max(1) as f64);
    let m = cam.view_proj(w / h);
    pts.map(|p| {
        let c = mat_vec(&m, [p[0], p[1], p[2], 1.0]);
        (c[3] > 0.0).then(|| [0.5 * (c[0] / c[3] + 1.0) * w, 0.5 * (1.0 - c[1] / c[3]) * h])
    })
    .collect()
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    (x * x + y * y).sqrt()
}

/// 2D picking of a boundary patch: the outline segment nearest to texture pixel `px` within `tol` pixels.
pub fn outline_at(b: &Built, cam: &Camera, size: [u32; 2], px: [f64; 2], tol: f64) -> Option<usize> {
    let ends = project_all(cam, size, b.outlines.iter().flat_map(|(_, s)| [s[0], s[1]]));
    let mut best: Option<(f64, usize)> = None;
    for (i, e) in ends.chunks(2).enumerate() {
        if let (Some(a), Some(c)) = (e[0], e[1]) {
            let d = seg_dist(px, a, c);
            if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
    }
    best.map(|(_, i)| i)
}

/// The hover readout: a 2D boundary outline under the pointer first, else the picked surface face (its patch in 3D,
/// its cell in 2D). `face` is the pick's `(face, owner cell)`.
pub fn readout(b: &Built, face: Option<(usize, Option<usize>)>, outline: Option<usize>) -> Option<String> {
    if let Some(i) = outline {
        let (k, _) = b.outlines.get(i)?;
        let p = b.patches.get(*k)?;
        return Some(format!("patch {} ({}), boundary", p.name, kind_name(&p.kind)));
    }
    let (f, cell) = face?;
    let (_, p) = patch_of_face(b, f)?;
    let cell = cell.map_or("-".to_string(), |c| c.to_string());
    Some(if b.two_d {
        format!("cell {cell} (front patch {}, face {f})", p.name)
    } else {
        format!("patch {} ({}), face {f}, cell {cell}", p.name, kind_name(&p.kind))
    })
}

/// Lower-case name of a patch kind.
pub fn kind_name(k: &PatchKind) -> &'static str {
    match k {
        PatchKind::Generic => "patch",
        PatchKind::Wall => "wall",
        PatchKind::Inlet => "inlet",
        PatchKind::Outlet => "outlet",
        PatchKind::Symmetry => "symmetry",
        PatchKind::Empty => "empty",
        PatchKind::Cyclic => "cyclic",
    }
}

// ---------------------------------------------------------------- model

/// How the surface is coloured (3D).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colouring {
    /// One colour per patch ([`PALETTE`]).
    Patches,
    /// Every face [`SINGLE`].
    Single,
}

/// The state of the latest request.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No case asked for, or the project is not open yet.
    Idle,
    /// A build is running (or waiting for a free build thread).
    Building {
        /// When it was asked for.
        since: Instant,
    },
    /// Built and shown.
    Ready,
    /// Over a limit: the located message; `rebuild` when going on means building (cells, file), else only showing
    /// the built surface (triangles).
    TooLarge {
        /// Located message.
        message: String,
        /// True: "build anyway"; false: "show anyway".
        rebuild: bool,
    },
    /// A located error.
    Failed(String),
}

/// Negative-control switches (tests only; all off in the app).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// Build on the calling (UI) thread instead of a build thread.
    pub build_on_ui_thread: bool,
    /// No generation check: builds never stop early and every result is applied, also of an older request.
    pub ignore_generation: bool,
    /// ADR 0138's rule: the preview is built from the case file on disk only (unsaved editor edits are not shown).
    pub disk_only: bool,
    /// ADR 0138's rule: a build that fails or is over a limit takes the shown mesh away (no last good mesh).
    pub drop_on_failure: bool,
    /// The frame asked for when typing pauses is not lengthened by egui's predicted frame time, so it comes one frame
    /// before the debounce ends (the code of ADR 0144 J3b's known miss, amendment 2).
    pub uncompensated_repaint: bool,
}

/// Counters for the tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Builds asked for.
    pub requests: u64,
    /// Builds asked for of the case editor's text (ADR 0144).
    pub editor_requests: u64,
    /// Results dropped because a newer request came first.
    pub discarded: u64,
    /// Surfaces handed to the viewport.
    pub installs: u64,
    /// Results received (current or discarded) whose build called `build_mesh` (ADR 0138 amendment 2).
    pub meshes_built: u64,
    /// Most build threads alive at once, seen by [`Preview::drain`] and [`Preview::request`].
    pub most_running: usize,
}

struct Msg {
    gen: u64,
    rel: String,
    outcome: Outcome,
}

/// The mesh preview of the open case.
pub struct Preview {
    root: Option<PathBuf>,
    /// The case asked for (project-relative), set by [`Studio::open_case`].
    pub case: Option<String>,
    /// The state of the latest request.
    pub status: Status,
    /// The result of the latest successful build of [`Preview::case`] (also when over the triangle limit, until "show
    /// anyway"; none after a failed or refused build).
    pub built: Option<Arc<Built>>,
    /// What the viewport shows: the last build of [`Preview::case`] within the limits (or shown anyway). A later build
    /// that fails or is over a limit leaves it, stale (ADR 0144).
    pub shown: Option<Arc<Built>>,
    /// Hash of the case file the last disk outcome read.
    pub read_hash: Option<String>,
    /// Colouring (3D).
    pub colouring: Colouring,
    /// Draw the cell edges of a 2D case.
    pub cell_edges: bool,
    /// Mesh patch indices not drawn (3D).
    pub hidden: BTreeSet<usize>,
    /// Size limits.
    pub limits: Limits,
    /// Time from the last edit or keystroke in the case editor to its build ([`DEBOUNCE`]; tests may change it).
    pub debounce: Duration,
    /// Negative-control switches.
    pub switches: Switches,
    /// Counters.
    pub stats: Stats,
    want: bool,
    force_cells: bool,
    force_triangles: bool,
    queued: Option<Source>,
    due: Option<Instant>,
    gen: Arc<AtomicU64>,
    running: Arc<AtomicUsize>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    changes_seen: Option<Instant>,
    editor_seen: Option<(String, String)>,
    editor_rev: Option<(String, u64)>,
}

impl Default for Preview {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Preview {
            root: None,
            case: None,
            status: Status::Idle,
            built: None,
            shown: None,
            read_hash: None,
            colouring: Colouring::Patches,
            cell_edges: true,
            hidden: BTreeSet::new(),
            limits: Limits::default(),
            debounce: DEBOUNCE,
            switches: Switches::default(),
            stats: Stats::default(),
            want: false,
            force_cells: false,
            force_triangles: false,
            queued: None,
            due: None,
            gen: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicUsize::new(0)),
            tx,
            rx,
            changes_seen: None,
            editor_seen: None,
            editor_rev: None,
        }
    }
}

impl Preview {
    /// Show the mesh of case `rel` (project-relative) from now on; the same case again keeps what is built.
    pub fn select(&mut self, rel: &str) {
        if self.case.as_deref() == Some(rel) {
            return;
        }
        self.forget();
        self.case = Some(rel.to_string());
        self.want = true;
    }

    fn forget(&mut self) {
        self.gen.fetch_add(1, Ordering::SeqCst); // results still on their way are stale now
        self.case = None;
        self.status = Status::Idle;
        self.built = None;
        self.shown = None;
        self.read_hash = None;
        self.hidden.clear();
        self.want = false;
        self.queued = None;
        self.due = None;
        self.force_cells = false;
        self.force_triangles = false;
        self.editor_seen = None;
        self.editor_rev = None;
    }

    /// Forget the case and the project (builds still running end on their own; their results are dropped).
    pub fn close(&mut self) {
        self.forget();
        self.root = None;
        self.changes_seen = None;
    }

    /// Build threads alive.
    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }

    /// True while a build of the current request is running or waiting.
    pub fn building(&self) -> bool {
        matches!(self.status, Status::Building { .. })
    }

    /// Time until the build of the case editor's last change is asked for (`None`: none waits). The UI asks for a
    /// frame then, so the build starts without further input (ADR 0144).
    pub fn due_in(&self) -> Option<Duration> {
        self.due.map(|d| d.saturating_duration_since(Instant::now()))
    }

    /// True while the shown mesh is not the one of the current text: an edit waits for its build, a build runs, or the
    /// latest build failed or is over a limit (the last good mesh stays shown, ADR 0144).
    pub fn stale(&self) -> bool {
        self.shown.is_some() && (self.due.is_some() || self.status != Status::Ready)
    }

    /// Why the shown mesh is stale ([`Preview::stale`]), in words; `None` when it is not.
    pub fn stale_reason(&self) -> Option<&'static str> {
        if !self.stale() {
            return None;
        }
        Some(match &self.status {
            _ if self.due.is_some() => "the edits since are built when typing pauses",
            Status::Building { .. } => "the current text is being built",
            Status::Failed(_) => "the current text does not build (above)",
            Status::TooLarge { rebuild: true, .. } => "the current text is over a size limit (above)",
            Status::TooLarge { rebuild: false, .. } => "the new surface is over the triangle limit (above)",
            Status::Idle | Status::Ready => "the shown mesh is older than the current text",
        })
    }

    /// Go on past a size limit: build anyway (cells, file size) or show the built surface anyway (triangles).
    pub fn anyway(&mut self) {
        match &self.status {
            Status::TooLarge { rebuild: true, .. } => {
                self.force_cells = true;
                self.want = true;
            }
            Status::TooLarge { rebuild: false, .. } => {
                self.force_triangles = true;
                if self.built.is_some() {
                    self.shown = self.built.clone();
                    self.status = Status::Ready;
                }
            }
            _ => {}
        }
    }

    /// Ask for a rebuild of the current case (a change on disk, the Rebuild button).
    pub fn rebuild(&mut self) {
        if self.case.is_some() {
            self.want = true;
        }
    }

    fn note_running(&mut self) {
        self.stats.most_running = self.stats.most_running.max(self.running());
    }

    fn request(&mut self, wake: &Wake, source: Source) {
        let (Some(root), Some(rel)) = (self.root.clone(), self.case.clone()) else {
            return;
        };
        let gen = self.gen.fetch_add(1, Ordering::SeqCst) + 1;
        self.stats.requests += 1;
        if matches!(source, Source::Editor(_)) {
            self.stats.editor_requests += 1;
        }
        self.status = Status::Building { since: Instant::now() };
        if self.switches.build_on_ui_thread {
            let outcome = build_source(&root, &rel, &source, &self.limits, self.force_cells, &|| false);
            let _ = self.tx.send(Msg { gen, rel, outcome });
            return;
        }
        if self.running() >= MAX_BUILDS {
            self.queued = Some(source);
            return;
        }
        self.queued = None;
        self.spawn(root, rel, gen, source, wake.clone());
        self.note_running();
    }

    fn spawn(&self, root: PathBuf, rel: String, gen: u64, source: Source, wake: Wake) {
        let (latest, running, tx) = (self.gen.clone(), self.running.clone(), self.tx.clone());
        let (limits, force) = (self.limits, self.force_cells);
        let checked = !self.switches.ignore_generation;
        let from_editor = matches!(source, Source::Editor(_));
        running.fetch_add(1, Ordering::SeqCst);
        let spawned = std::thread::Builder::new()
            .name("rcfd-studio-preview".into())
            .spawn(move || {
                let outcome = build_source(&root, &rel, &source, &limits, force, &|| {
                    checked && latest.load(Ordering::SeqCst) != gen
                });
                let _ = tx.send(Msg { gen, rel, outcome });
                running.fetch_sub(1, Ordering::SeqCst);
                wake();
            });
        if let Err(e) = spawned {
            self.running.fetch_sub(1, Ordering::SeqCst);
            let _ = self.tx.send(Msg {
                gen,
                rel: self.case.clone().unwrap_or_default(),
                outcome: Outcome {
                    read_hash: None,
                    called_build_mesh: false,
                    from_editor,
                    result: Err(Refusal::Failed(format!("preview: no build thread: {e}"))),
                },
            });
        }
    }

    /// Apply finished builds; start a waiting one when a thread is free. The number of results applied or dropped.
    pub fn drain(&mut self, wake: &Wake) -> usize {
        self.note_running();
        let msgs: Vec<Msg> = self.rx.try_iter().collect();
        let n = msgs.len();
        for m in msgs {
            self.stats.meshes_built += u64::from(m.outcome.called_build_mesh);
            let current = m.gen == self.gen.load(Ordering::SeqCst) && self.case.as_deref() == Some(m.rel.as_str());
            if !current && !self.switches.ignore_generation {
                self.stats.discarded += 1;
                continue;
            }
            let Outcome {
                read_hash,
                result,
                from_editor,
                ..
            } = m.outcome;
            if !from_editor && !matches!(result, Err(Refusal::Cancelled)) {
                self.read_hash = read_hash;
            }
            match result {
                Err(Refusal::Cancelled) => {}
                Ok(mut b) => {
                    // the same mesh again (a rebuild, a typo undone, the saved text) keeps its surface: no new upload,
                    // the camera stays; the rest (source, hash, time) is the new build's
                    let same = [&self.built, &self.shown]
                        .into_iter()
                        .flatten()
                        .find(|o| o.rel == b.rel && o.mesh_hash == b.mesh_hash && o.patches == b.patches)
                        .cloned();
                    if let Some(o) = same {
                        b.surface = o.surface.clone();
                    }
                    let over = b.triangles > self.limits.triangles && !self.force_triangles;
                    let message = format!(
                        "{}: the surface has {} triangles, more than the preview limit of {} (uploading it pauses the window)",
                        b.rel,
                        with_commas(b.triangles as u64),
                        with_commas(self.limits.triangles as u64)
                    );
                    self.built = Some(Arc::new(b));
                    if over {
                        self.status = Status::TooLarge {
                            message,
                            rebuild: false,
                        };
                    } else {
                        self.shown = self.built.clone();
                        self.status = Status::Ready;
                    }
                }
                Err(Refusal::TooLarge(message)) => {
                    self.built = None;
                    self.status = Status::TooLarge { message, rebuild: true };
                }
                Err(Refusal::Failed(message)) => {
                    self.built = None;
                    self.status = Status::Failed(message);
                }
            }
        }
        if self.running() < MAX_BUILDS {
            if let (Some(source), Some(root), Some(rel)) = (self.queued.take(), self.root.clone(), self.case.clone()) {
                let gen = self.gen.load(Ordering::SeqCst);
                self.spawn(root, rel, gen, source, wake.clone());
                self.note_running();
            }
        }
        n
    }

    /// The part styles of the shown surface: one solid colour per patch (or [`SINGLE`]), hidden patches hidden; a 2D
    /// front in [`FRONT_2D`].
    pub fn part_styles(&self) -> Vec<PartStyle> {
        let Some(b) = self.displayed() else { return Vec::new() };
        b.part_patch
            .iter()
            .map(|&k| {
                if b.two_d {
                    PartStyle::Solid(FRONT_2D)
                } else if self.hidden.contains(&k) {
                    PartStyle::Hidden
                } else {
                    match self.colouring {
                        Colouring::Patches => PartStyle::Solid(b.patches[k].colour),
                        Colouring::Single => PartStyle::Solid(SINGLE),
                    }
                }
            })
            .collect()
    }

    /// The mesh the viewport shows: [`Preview::shown`]; with [`Switches::drop_on_failure`], ADR 0138's rule (the built
    /// mesh while it is ready or being rebuilt).
    pub fn displayed(&self) -> Option<&Arc<Built>> {
        if self.switches.drop_on_failure {
            return match (&self.built, &self.status) {
                (Some(b), Status::Ready | Status::Building { .. }) => Some(b),
                _ => None,
            };
        }
        self.shown.as_ref()
    }
}

/// The source of the next build of the open case: the case editor's text while it holds an unsaved edit or a field
/// being typed in of that case, else the file on disk ([`Switches::disk_only`]: always the file).
pub fn current_source(st: &Studio) -> Source {
    let p = &st.preview;
    let Some(case) = p.case.as_deref() else {
        return Source::Disk;
    };
    if p.switches.disk_only {
        return Source::Disk;
    }
    let Some(ed) = st.editor.as_ref().filter(|e| e.rel == case) else {
        return Source::Disk;
    };
    if ed.draft.is_none() && !ed.is_dirty() {
        return Source::Disk;
    }
    let draft = ed.draft.as_ref().map(|(path, text)| Draft {
        path: path.clone(),
        kind: st.schema.find(path).filter(|n| n.is_leaf()).map(|n| n.kind.clone()),
        text: text.clone(),
    });
    let json = Path::new(case)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    Source::Editor(Arc::new(EditorText {
        settings: ed.settings.clone(),
        draft,
        json,
    }))
}

/// Follow the Studio (called in [`Studio::drain`]): the open project, the asked case, changes of its file on disk and
/// edits in the case editor; apply finished builds; hand the shown surface to the viewport on the Mesh side. Never
/// blocks (with [`Switches::build_on_ui_thread`] off). Returns the number of build results seen.
pub fn follow(st: &mut Studio, wake: &Wake) -> usize {
    let root = st.browser().map(|b| b.root.clone());
    if st.preview.root != root {
        // a case asked for while the project was opening is kept; another project starts with nothing
        let keep = if st.preview.root.is_none() {
            st.preview.case.clone()
        } else {
            None
        };
        st.preview.close();
        st.preview.root = root.clone();
        if let (Some(_), Some(c)) = (&root, keep) {
            st.preview.select(&c);
        }
    }
    if root.is_none() {
        return 0;
    }
    let now = Instant::now();
    let p = &mut st.preview;
    if let Some(case) = p.case.clone() {
        if let Some(ed) = st.editor.as_ref().filter(|ed| ed.rel == case) {
            // the editor loaded, saved or reloaded other bytes of the file: rebuild once per change of its base hash
            // (not on inequality, which a conflict keeps forever)
            let changed = match &p.editor_seen {
                Some((r, old)) if *r == case => *old != ed.base_hash,
                _ => p.read_hash.as_ref().is_some_and(|h| *h != ed.base_hash),
            };
            if changed {
                p.want = true;
            }
            p.editor_seen = Some((case.clone(), ed.base_hash.clone()));
            // ADR 0144: an edit or a keystroke in the editor asks for a build DEBOUNCE after the last one
            let rev = ed.revision();
            if !p.switches.disk_only && matches!(&p.editor_rev, Some((r, old)) if *r == case && *old != rev) {
                p.due = Some(now + p.debounce);
            }
            p.editor_rev = Some((case.clone(), rev));
        }
        // the watcher saw the case file or a mesh file of it change (since the last frame)
        if let Some(op) = &st.project {
            let root = root.as_deref().unwrap_or(Path::new(""));
            let mesh_rels: Vec<String> = [&p.built, &p.shown]
                .into_iter()
                .flatten()
                .filter_map(|b| b.mesh_file.as_ref())
                .filter_map(|f| f.strip_prefix(root).ok())
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .collect();
            for (t, path) in &op.changes {
                if p.changes_seen.is_some_and(|s| *t <= s) {
                    continue;
                }
                if *path == case || mesh_rels.iter().any(|m| m == path) {
                    p.want = true;
                }
            }
        }
    }
    // changes seen while no case was asked for do not count later
    if let Some(op) = &st.project {
        p.changes_seen = op.changes.last().map(|(t, _)| *t).or(p.changes_seen);
    }
    if p.due.is_some_and(|d| now >= d) {
        p.want = true;
    }
    if p.want {
        // a newer request supersedes a running build (its result is dropped); it builds the current text, so an edit
        // still waiting for its debounce is built by it too
        p.want = false;
        p.due = None;
        let source = current_source(st);
        st.preview.request(wake, source);
    }
    let n = st.preview.drain(wake);
    install(st);
    n
}

/// Hand the shown surface to the viewport (only on the Mesh side: no run shown), or take it away.
fn install(st: &mut Studio) {
    if st.viewport.run.is_some() {
        return;
    }
    // `shown` belongs to the asked case only (a new case forgets it), so a rebuild keeps showing the last surface
    let shown = st.preview.displayed().cloned();
    match shown {
        Some(b) => {
            let styles = st.preview.part_styles();
            let same = st.viewport.surface.as_ref().is_some_and(|s| Arc::ptr_eq(s, &b.surface));
            if same {
                st.viewport.set_part_styles(styles);
            } else {
                st.viewport.show_surface(b.surface.clone(), b.two_d, styles);
                st.preview.stats.installs += 1;
            }
        }
        None => {
            if st.viewport.surface.is_some() {
                st.viewport.clear_surface();
            }
        }
    }
}
