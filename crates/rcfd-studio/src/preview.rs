//! Mesh preview from a case (ADR 0138, wave 48G1): when a case is open and no run is selected, the viewport shows the
//! boundary surface of the mesh the solver would build for that case, without running the solver.
//!
//! * The case file is read from disk and parsed as `rcfd run <snapshot> --case-dir <the case's folder>` parses it (the
//!   runner's rule: JSON when the text starts with `{`, else TOML; relative paths against the case's folder), the mesh
//!   is built by `rcfd_api::sim::build_mesh` and its surface by [`crate::viewport::surface_of`], the run viewport's rule
//!   (a 2D case is its +z `Empty` faces). Unsaved edits in the case editor are not shown: the preview is of the file a
//!   run would use.
//! * Builds run on a thread per request ([`build`] is the code it runs, callable from tests); at most [`MAX_BUILDS`]
//!   are alive at once. Each request has a generation number: a newer request makes older builds stop at their next
//!   phase boundary and their results are discarded (a running `build_mesh` call cannot be stopped; "cancelled" means
//!   "discarded", ADR 0138). Nothing is joined on the UI thread.
//! * Size limits ([`Limits`]): a cell estimate before building (recipe meshes), the file size of a mesh file, and the
//!   surface triangles before the upload, which is the one GPU step on the UI thread. Over a limit the preview says so
//!   and offers to go on anyway.
//! * Drawing reuses the run viewport ([`crate::viewport::Viewport::show_surface`]): the same `ViewGpu`, texture, camera
//!   mapping and pick pass, every face at value 0 and one solid colour per patch (`PartStyle::Solid`). A 2D case's
//!   boundary patches are edges in its face-on view; they are drawn as coloured segments over the image by
//!   [`crate::preview_ui`], with the cell edges of the front when there are at most [`Limits::overlay_edges`].

use crate::studio::Studio;
use crate::viewport::surface_of;
use crate::watch::Wake;
use rcfd_api::case::{Case, Diagnostic};
use rcfd_core::vec3::Vec3;
use rcfd_mesh::{Mesh, PatchKind};
use rcfd_render::camera::{mat_vec, Camera};
use rcfd_render::{PartStyle, SurfaceMesh};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

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

/// Number of builds that reached `build_mesh` (tests: a refusal before building leaves it unchanged).
pub static MESHES_BUILT: AtomicU64 = AtomicU64::new(0);

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
    /// The case file read.
    pub file: PathBuf,
    /// `hash_bytes` of the case file as read.
    pub case_hash: String,
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
    /// Wall time of [`build`], seconds.
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

/// What [`build`] returns: the hash of the bytes read (when the file was read) and the result.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// `hash_bytes` of the case file, when it was read.
    pub read_hash: Option<String>,
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

/// Build the preview of the case file `root/rel` (the code a build thread runs). `cancelled` is polled between phases;
/// `force` skips the cell and file-size limits.
pub fn build(root: &Path, rel: &str, limits: &Limits, force: bool, cancelled: &dyn Fn() -> bool) -> Outcome {
    let t0 = Instant::now();
    let fail = |m: String| Outcome {
        read_hash: None,
        result: Err(Refusal::Failed(m)),
    };
    if !plain(rel) {
        return fail(format!(
            "{rel}: refused: a case is named by a plain project-relative path"
        ));
    }
    let file = root.join(rel);
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(e) => return fail(format!("{rel}: {}: {e}", file.display())),
    };
    let read_hash = Some(rcfd_project::records::hash_bytes(&bytes));
    let out = |result: Result<Built, Refusal>| Outcome {
        read_hash: read_hash.clone(),
        result,
    };
    let src = match std::str::from_utf8(&bytes) {
        Ok(t) => t,
        Err(e) => return out(Err(Refusal::Failed(format!("{rel}: not UTF-8 text: {e}")))),
    };
    let dir = file.parent().map_or_else(|| root.to_path_buf(), Path::to_path_buf);
    let case = match parse_case(src, &dir) {
        Ok(c) => c,
        Err(d) => return out(Err(Refusal::Failed(located(rel, &d)))),
    };
    if cancelled() {
        return out(Err(Refusal::Cancelled));
    }
    if !force {
        match estimate(&case) {
            Estimate::Cells(n) if n > limits.cells => {
                return out(Err(Refusal::TooLarge(format!(
                    "{rel}: about {} cells (from the mesh.n* counts of generator \"{}\"), more than the preview limit of {}",
                    with_commas(n),
                    text(&case, "mesh.generator"),
                    with_commas(limits.cells)
                ))))
            }
            Estimate::FileBytes(b) if b > limits.file_bytes => {
                return out(Err(Refusal::TooLarge(format!(
                    "{rel}: mesh.file {} is {} bytes, more than the preview limit of {}",
                    mesh_file_of(&case).unwrap_or_default().display(),
                    with_commas(b),
                    with_commas(limits.file_bytes)
                ))))
            }
            _ => {}
        }
    }
    MESHES_BUILT.fetch_add(1, Ordering::Relaxed);
    let mesh = match rcfd_api::sim::build_mesh(&case) {
        Ok(m) => m,
        Err(d) => return out(Err(Refusal::Failed(located(rel, &d)))),
    };
    if cancelled() {
        return out(Err(Refusal::Cancelled));
    }
    let (surface, two_d) = match surface_of(&mesh) {
        Ok(s) => s,
        Err(e) => return out(Err(Refusal::Failed(format!("{rel}: surface: {e}")))),
    };
    if surface.n_faces() == 0 {
        return out(Err(Refusal::Failed(format!(
            "{rel}: the mesh has no boundary surface to draw"
        ))));
    }
    let (patches, part_patch) = patch_list(&mesh, &surface);
    let (edges, n_edges, outlines) = if two_d {
        match overlay_2d(&mesh, &surface, &patches, limits.overlay_edges, cancelled) {
            Some(o) => o,
            None => return out(Err(Refusal::Cancelled)),
        }
    } else {
        (Vec::new(), 0, Vec::new())
    };
    let triangles = surface.triangles().count();
    out(Ok(Built {
        rel: rel.to_string(),
        file,
        case_hash: read_hash.clone().unwrap_or_default(),
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
    }))
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

/// What the preview shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No case asked for, or the project is not open yet.
    Idle,
    /// A build is running (or waiting for a free build thread).
    Building {
        /// When it was asked for.
        since: Instant,
    },
    /// Built and shown (when no run is selected).
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
}

/// Counters for the tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Builds asked for.
    pub requests: u64,
    /// Results dropped because a newer request came first.
    pub discarded: u64,
    /// Surfaces handed to the viewport.
    pub installs: u64,
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
    /// What is shown.
    pub status: Status,
    /// The built preview of [`Preview::case`].
    pub built: Option<Arc<Built>>,
    /// Hash of the case file the last outcome was built from.
    pub read_hash: Option<String>,
    /// Colouring (3D).
    pub colouring: Colouring,
    /// Draw the cell edges of a 2D case.
    pub cell_edges: bool,
    /// Mesh patch indices not drawn (3D).
    pub hidden: BTreeSet<usize>,
    /// Size limits.
    pub limits: Limits,
    /// Negative-control switches.
    pub switches: Switches,
    /// Counters.
    pub stats: Stats,
    want: bool,
    force_cells: bool,
    force_triangles: bool,
    queued: bool,
    gen: Arc<AtomicU64>,
    running: Arc<AtomicUsize>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    changes_seen: Option<Instant>,
    editor_seen: Option<(String, String)>,
}

impl Default for Preview {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Preview {
            root: None,
            case: None,
            status: Status::Idle,
            built: None,
            read_hash: None,
            colouring: Colouring::Patches,
            cell_edges: true,
            hidden: BTreeSet::new(),
            limits: Limits::default(),
            switches: Switches::default(),
            stats: Stats::default(),
            want: false,
            force_cells: false,
            force_triangles: false,
            queued: false,
            gen: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicUsize::new(0)),
            tx,
            rx,
            changes_seen: None,
            editor_seen: None,
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
        self.read_hash = None;
        self.hidden.clear();
        self.want = false;
        self.queued = false;
        self.force_cells = false;
        self.force_triangles = false;
        self.editor_seen = None;
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
                    self.status = Status::Ready;
                }
            }
            _ => {}
        }
    }

    /// Ask for a rebuild of the current case (a change on disk).
    pub fn rebuild(&mut self) {
        if self.case.is_some() {
            self.want = true;
        }
    }

    fn request(&mut self, wake: &Wake) {
        let (Some(root), Some(rel)) = (self.root.clone(), self.case.clone()) else {
            return;
        };
        let gen = self.gen.fetch_add(1, Ordering::SeqCst) + 1;
        self.stats.requests += 1;
        if matches!(self.status, Status::TooLarge { .. } | Status::Failed(_)) {
            self.built = None; // only a shown surface stays on screen while it is rebuilt
        }
        self.status = Status::Building { since: Instant::now() };
        if self.switches.build_on_ui_thread {
            let outcome = build(&root, &rel, &self.limits, self.force_cells, &|| false);
            let _ = self.tx.send(Msg { gen, rel, outcome });
            return;
        }
        if self.running() >= MAX_BUILDS {
            self.queued = true;
            return;
        }
        self.queued = false;
        self.spawn(root, rel, gen, wake.clone());
    }

    fn spawn(&self, root: PathBuf, rel: String, gen: u64, wake: Wake) {
        let (latest, running, tx) = (self.gen.clone(), self.running.clone(), self.tx.clone());
        let (limits, force) = (self.limits, self.force_cells);
        let checked = !self.switches.ignore_generation;
        running.fetch_add(1, Ordering::SeqCst);
        let spawned = std::thread::Builder::new()
            .name("rcfd-studio-preview".into())
            .spawn(move || {
                let outcome = build(&root, &rel, &limits, force, &|| {
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
                    result: Err(Refusal::Failed(format!("preview: no build thread: {e}"))),
                },
            });
        }
    }

    /// Apply finished builds; start a waiting one when a thread is free. The number of results applied or dropped.
    pub fn drain(&mut self, wake: &Wake) -> usize {
        let msgs: Vec<Msg> = self.rx.try_iter().collect();
        let n = msgs.len();
        for m in msgs {
            let current = m.gen == self.gen.load(Ordering::SeqCst) && self.case.as_deref() == Some(m.rel.as_str());
            if !current && !self.switches.ignore_generation {
                self.stats.discarded += 1;
                continue;
            }
            let Outcome { read_hash, result } = m.outcome;
            match result {
                Err(Refusal::Cancelled) => {}
                Ok(b) => {
                    self.read_hash = read_hash;
                    let same = self
                        .built
                        .as_ref()
                        .is_some_and(|o| o.rel == b.rel && o.mesh_hash == b.mesh_hash && o.patches == b.patches);
                    let over = b.triangles > self.limits.triangles && !self.force_triangles;
                    let message = format!(
                        "{}: the surface has {} triangles, more than the preview limit of {} (uploading it pauses the window)",
                        b.rel,
                        with_commas(b.triangles as u64),
                        with_commas(self.limits.triangles as u64)
                    );
                    if !same {
                        self.built = Some(Arc::new(b));
                    }
                    self.status = if over {
                        Status::TooLarge {
                            message,
                            rebuild: false,
                        }
                    } else {
                        Status::Ready
                    };
                }
                Err(Refusal::TooLarge(message)) => {
                    self.read_hash = read_hash;
                    self.built = None;
                    self.status = Status::TooLarge { message, rebuild: true };
                }
                Err(Refusal::Failed(message)) => {
                    self.read_hash = read_hash;
                    self.built = None;
                    self.status = Status::Failed(message);
                }
            }
        }
        if self.queued && self.running() < MAX_BUILDS {
            if let (Some(root), Some(rel)) = (self.root.clone(), self.case.clone()) {
                self.queued = false;
                let gen = self.gen.load(Ordering::SeqCst);
                self.spawn(root, rel, gen, wake.clone());
            }
        }
        n
    }

    /// The part styles of the drawn surface: one solid colour per patch (or [`SINGLE`]), hidden patches hidden; a 2D
    /// front in [`FRONT_2D`].
    pub fn part_styles(&self) -> Vec<PartStyle> {
        let Some(b) = &self.built else { return Vec::new() };
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
}

/// Follow the Studio (called at the end of [`Studio::drain`]): the open project, the asked case and changes of its
/// file on disk; apply finished builds; hand the built surface to the viewport while no run is selected. Never
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
    let p = &mut st.preview;
    if let Some(case) = p.case.clone() {
        // the editor loaded, saved or reloaded other bytes of the file: rebuild once per change of its base hash (not
        // on inequality, which a conflict keeps forever)
        if let Some(ed) = st.editor.as_ref().filter(|ed| ed.rel == case) {
            let changed = match &p.editor_seen {
                Some((r, old)) if *r == case => *old != ed.base_hash,
                _ => p.read_hash.as_ref().is_some_and(|h| *h != ed.base_hash),
            };
            if changed {
                p.want = true;
            }
            p.editor_seen = Some((case.clone(), ed.base_hash.clone()));
        }
        // the watcher saw the case file or the mesh file change (since the last frame)
        if let Some(op) = &st.project {
            let mesh_rel = p
                .built
                .as_ref()
                .and_then(|b| b.mesh_file.as_ref())
                .and_then(|f| root.as_ref().and_then(|r| f.strip_prefix(r).ok()))
                .map(|r| r.to_string_lossy().replace('\\', "/"));
            for (t, path) in &op.changes {
                if p.changes_seen.is_some_and(|s| *t <= s) {
                    continue;
                }
                if *path == case || mesh_rel.as_deref() == Some(path.as_str()) {
                    p.want = true;
                }
            }
        }
    }
    // changes seen while no case was asked for do not count later
    if let Some(op) = &st.project {
        p.changes_seen = op.changes.last().map(|(t, _)| *t).or(p.changes_seen);
    }
    if p.want {
        // a newer request supersedes a running build (its result is dropped)
        p.want = false;
        p.request(wake);
    }
    let n = p.drain(wake);
    install(st);
    n
}

/// Hand the built surface to the viewport (only while no run is selected), or take it away.
fn install(st: &mut Studio) {
    if st.viewport.run.is_some() {
        return;
    }
    // `built` belongs to the asked case only (a new case forgets it), so a rebuild keeps showing the last surface
    let shown = match (&st.preview.built, &st.preview.status) {
        (Some(b), Status::Ready | Status::Building { .. }) => Some(b.clone()),
        _ => None,
    };
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
