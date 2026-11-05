//! Mesh set-up commands (ADR 0173 R2 and W4, registered for F-272 by ADR 0187): `check_case_mesh` (the report of
//! `rcfd check-mesh` and the patch list of a project case's mesh), `preview_mesh` (a PNG of that mesh coloured by a
//! cell-quality measure, drawn by the CPU renderer), `import_mesh` (a Gmsh, Fluent or OpenFOAM mesh of the project read
//! with the solver's readers and copied into `meshes/` without replacing anything) and `list_meshes` (`meshes/` with the
//! cases that use each mesh). The mesh of a case is set up by its settings through `edit_case`, which builds and checks
//! the edited mesh when a `mesh.*` setting changes ([`is_mesh_key`], `crate::write`).
//!
//! A case's mesh is built by `rcfd_api::sim::build_mesh`, which reads `mesh.file` without a limit; every command here
//! first confines that file to the project folder ([`resolve_mesh_file`], ADR 0187 M2).
//!
//! No `.rcfd/edits.jsonl` line is written for an import (a mesh import line is ADR 0173 W6).

use crate::create::invalid;
use crate::{
    err, project_error as perr, str_arg, text_len, u64_arg, Answer, RpcError, Workspace, INTERNAL, NOT_FOUND, PARAMS,
    REFUSED, TEXT_BUDGET, VALIDATION,
};
use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::service::base64;
use rcfd_mesh::quality::{check_mesh, compute_metrics, QualityReport};
use rcfd_mesh::Mesh;
use rcfd_project::atomic;
use rcfd_project::records::hash_bytes;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

/// The cell measures `preview_mesh` colours by (ADR 0187 M4, Q5).
pub const MEASURES: [&str; 3] = ["non_orthogonality", "skewness", "aspect_ratio"];
/// The measure of a preview without `measure` (ADR 0187 Q5 (a)).
pub const DEFAULT_MEASURE: &str = "non_orthogonality";
/// Default width of a preview, pixels (as `render_field`).
pub const PREVIEW_WIDTH: usize = 640;
/// Narrowest preview; a preview over the size budget is drawn again at half the width down to this (ADR 0187 Q4 (a)).
pub const MIN_PREVIEW_WIDTH: usize = 64;
/// Widest preview.
pub const MAX_PREVIEW_WIDTH: usize = 4096;
/// A measure whose largest value is within this of its ideal value colours the mesh as ideal (range ideal to ideal + 1).
pub const FLAT: f64 = 1e-9;
/// Longest name of an imported mesh in `meshes/`.
pub const MAX_MESH_NAME: usize = 128;
/// The files of an OpenFOAM `polyMesh` folder that `rcfd_io::openfoam::read_dir` reads (all needed).
pub const OPENFOAM_FILES: [&str; 5] = ["points", "faces", "owner", "neighbour", "boundary"];
/// The optional files of such a folder that it reads when they are there.
pub const OPENFOAM_ZONE_FILES: [&str; 2] = ["cellZones", "faceZones"];
/// Axis names of a preview's plane.
const AXES: [&str; 3] = ["x", "y", "z"];

/// True for a setting path of a case's mesh (`mesh` or `mesh.<...>`): an `edit_case` that sets one builds and checks
/// the edited mesh before it writes (ADR 0187 M7).
pub fn is_mesh_key(k: &str) -> bool {
    k == "mesh" || k.starts_with("mesh.")
}

/// The ideal value of a measure (the low end of a preview's colour range).
pub fn ideal(measure: &str) -> f64 {
    if measure == "aspect_ratio" {
        1.0
    } else {
        0.0
    }
}

/// The value of `measure` for each cell (ADR 0187 M4): `non_orthogonality` and `skewness` the largest of the cell's
/// faces (`rcfd_mesh::quality::compute_metrics`, per face), `aspect_ratio` the cell's. `None` for another name.
pub fn cell_measure(mesh: &Mesh, measure: &str) -> Option<Vec<f64>> {
    let m = compute_metrics(mesh);
    let per_face = match measure {
        "aspect_ratio" => return Some(m.aspect_ratio),
        "non_orthogonality" => m.non_orthogonality,
        "skewness" => m.skewness,
        _ => return None,
    };
    Some(
        (0..mesh.n_cells())
            .map(|c| mesh.cell_faces(c).iter().map(|&f| per_face[f]).fold(0.0, f64::max))
            .collect(),
    )
}

/// The plane `rcfd_post::render::render_scalar` draws by default: the thinnest bounding-box direction (the first of
/// equal ones) and its middle. `(axis, slice)`.
pub fn default_plane(mesh: &Mesh) -> (usize, f64) {
    let (lo, hi) = bounds(mesh).unwrap_or(([0.0; 3], [0.0; 3]));
    let axis = (0..3)
        .min_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
        .unwrap_or(2);
    (axis, 0.5 * (lo[axis] + hi[axis]))
}

fn bounds(mesh: &Mesh) -> Option<([f64; 3], [f64; 3])> {
    let pts = mesh.points();
    if pts.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Some((lo, hi))
}

/// Where a case's mesh file is, confined to the project (ADR 0187 M2): `file` as the case writes it, joined to the
/// case's folder `base` when relative (as `build_mesh` joins it) and normalised (`.` dropped, `..` removing one
/// component, refused when it would leave the project root), then checked with `rcfd_project::paths::resolve_in` (no
/// links, no drive or stream syntax). An absolute name must lie under the root. Returns the path and its
/// project-relative form, or the reason of the refusal.
pub fn resolve_mesh_file(root: &Path, base: &Path, file: &str) -> Result<(PathBuf, String), String> {
    let outside = || format!("mesh.file = {file:?} is outside the project folder {}", root.display());
    let given = Path::new(file);
    let rel_parts: Vec<String> = if given.is_absolute() {
        let full = if given.exists() {
            rcfd_project::paths::canonicalize(given).map_err(|e| format!("mesh.file = {file:?}: {e}"))?
        } else {
            normalise(given).ok_or_else(outside)?
        };
        let rel = full.strip_prefix(root).map_err(|_| outside())?;
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    } else {
        let base_rel = base.strip_prefix(root).map_err(|_| outside())?;
        let mut parts: Vec<String> = Vec::new();
        for c in base_rel.components().chain(given.components()) {
            match c {
                Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if parts.pop().is_none() {
                        return Err(format!("mesh.file = {file:?} leaves the project folder"));
                    }
                }
                Component::Prefix(_) | Component::RootDir => return Err(outside()),
            }
        }
        parts
    };
    if rel_parts.is_empty() {
        return Err(format!("mesh.file = {file:?} names the project folder itself"));
    }
    let rel = rel_parts.join("/");
    let path = rcfd_project::paths::resolve_in(root, &rel).map_err(|e| format!("mesh.file = {file:?}: {e}"))?;
    Ok((path, rel))
}

/// `p` without `.` and `..` components (`None` when a `..` would go above the start).
fn normalise(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(s) => {
                out.push(s);
                depth += 1;
            }
        }
    }
    Some(out)
}

fn get_str(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn patch_json(mesh: &Mesh) -> Vec<Value> {
    mesh.patches()
        .iter()
        .map(|p| json!({"name": p.name, "faces": p.size, "kind": format!("{:?}", p.kind)}))
        .collect()
}

/// Drop entries from the end of `v[key]` (an array) until `v`'s text is at most `budget` bytes.
fn cut_to(v: &mut Value, key: &str, budget: usize) {
    while text_len(v) > budget {
        match v[key].as_array_mut() {
            Some(a) if !a.is_empty() => {
                a.pop();
            }
            _ => break,
        }
    }
}

/// The check of a built mesh as a result object of at most `budget` bytes of text (ADR 0187 M3): `ok`, `report` (the
/// `QualityReport` as `rcfd check-mesh --json` prints it), `patches` [{`name`, `faces`, `kind`}] and `patches_total`.
/// Over the budget, patches are dropped from the end, then the report's poor list is emptied (`poor_list_omitted`).
pub fn check_object(mesh: &Mesh, q: &QualityReport, budget: usize) -> Value {
    let patches = patch_json(mesh);
    let mut v = json!({"ok": q.is_ok(), "report": serde_json::to_value(q).unwrap_or(Value::Null),
        "patches_total": patches.len(), "patches": patches});
    cut_to(&mut v, "patches", budget);
    if text_len(&v) > budget {
        let n = v["report"]["poor_list"].as_array().map_or(0, Vec::len);
        v["report"]["poor_list"] = json!([]);
        v["poor_list_omitted"] = json!(n);
    }
    v
}

/// A refusal of a case's mesh file (1004), located at `mesh.file`.
fn refused_file(rel: &str, message: String) -> RpcError {
    let diags = vec![Diagnostic {
        path: "mesh.file".into(),
        line: None,
        message: message.clone(),
    }];
    RpcError {
        code: REFUSED,
        message: format!("{rel}: {message}"),
        data: Some(json!({"diagnostics": diags})),
    }
}

impl Workspace {
    /// Read the case file `given` as `rcfd check-mesh` reads it (by extension; its folder as the base, the project as the
    /// data-file sandbox). Returns its normalised path and the case.
    fn read_case_file(&self, what: &str, given: &str) -> Result<(String, Case), RpcError> {
        let path = self.project.resolve(given).map_err(perr)?;
        let norm = self.rel(&path);
        let is_json = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => false,
            Some("json") => true,
            _ => {
                return Err(err(
                    VALIDATION,
                    format!("{norm}: {what} reads .toml or .json case files"),
                ))
            }
        };
        if path.is_dir() {
            return Err(err(NOT_FOUND, format!("{norm} is a directory")));
        }
        let bytes = atomic::read(&path).map_err(perr)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{norm} is not UTF-8 text ({e})")))?;
        let root = self.project.root();
        let base = path.parent().unwrap_or(root);
        let case = if is_json {
            Case::parse_json_with(text, base, Some(root))
        } else {
            Case::parse_toml_with(text, base, Some(root))
        }
        .map_err(|d| invalid(&format!("{norm}: the case is not valid"), d))?;
        Ok((norm, case))
    }

    /// Build the mesh of `case` (the file `rel`) after confining its mesh file (ADR 0187 M2): the mesh and its source
    /// object (`{source: "generator", generator}` or `{source: "file", file}`). A refused mesh file is 1004, a mesh that
    /// cannot be built 1003 with `build_mesh`'s located diagnostics.
    pub(crate) fn build_case_mesh(&self, rel: &str, case: &Case) -> Result<(Mesh, Value), RpcError> {
        let source = if get_str(case, "mesh.source") == "file" {
            let file = get_str(case, "mesh.file");
            let (_, frel) =
                resolve_mesh_file(self.project.root(), &case.base_dir, &file).map_err(|m| refused_file(rel, m))?;
            json!({"source": "file", "file": frel})
        } else {
            json!({"source": "generator", "generator": get_str(case, "mesh.generator")})
        };
        let mesh =
            rcfd_api::sim::build_mesh(case).map_err(|d| invalid(&format!("{rel}: the mesh cannot be built"), d))?;
        Ok((mesh, source))
    }

    pub(crate) fn check_case_mesh(&self, args: &Value) -> Result<Value, RpcError> {
        let (rel, case) = self.read_case_file("check_case_mesh", str_arg(args, "case"))?;
        let (mesh, source) = self.build_case_mesh(&rel, &case)?;
        let q = check_mesh(&mesh);
        let mut v = json!({"case": rel, "mesh": source});
        let budget = TEXT_BUDGET.saturating_sub(text_len(&v) + 16);
        if let (Some(o), Value::Object(c)) = (v.as_object_mut(), check_object(&mesh, &q, budget)) {
            o.extend(c);
        }
        Ok(v)
    }

    pub(crate) fn preview_mesh(&self, args: &Value) -> Result<Answer, RpcError> {
        use rcfd_post::render::{render_scalar, RenderOptions};
        let (rel, case) = self.read_case_file("preview_mesh", str_arg(args, "case"))?;
        let measure = args.get("measure").and_then(Value::as_str).unwrap_or(DEFAULT_MEASURE);
        if !MEASURES.contains(&measure) {
            return Err(err(PARAMS, format!("measure: one of {MEASURES:?}, not {measure:?}")));
        }
        let (mesh, _) = self.build_case_mesh(&rel, &case)?;
        let values = cell_measure(&mesh, measure).expect("a known measure");
        let finite: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
        let (min, max) = finite
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| (a.0.min(x), a.1.max(x)));
        let lo = ideal(measure);
        let hi = if finite.is_empty() || max - lo <= FLAT {
            lo + 1.0
        } else {
            max
        };
        let (def_axis, def_slice) = default_plane(&mesh);
        let axis = match args.get("axis").and_then(Value::as_str) {
            None => def_axis,
            Some(a) => AXES
                .iter()
                .position(|x| *x == a)
                .ok_or_else(|| err(PARAMS, format!("axis: x, y or z, not {a:?}")))?,
        };
        let slice = match args.get("slice").and_then(Value::as_f64) {
            Some(s) => s,
            None if axis == def_axis => def_slice,
            None => {
                let (l, h) = bounds(&mesh).unwrap_or(([0.0; 3], [0.0; 3]));
                0.5 * (l[axis] + h[axis])
            }
        };
        let asked = u64_arg(args, "width")
            .unwrap_or(PREVIEW_WIDTH as u64)
            .clamp(MIN_PREVIEW_WIDTH as u64, MAX_PREVIEW_WIDTH as u64) as usize;
        let none = finite.is_empty();
        let mut width = asked;
        loop {
            let opt = RenderOptions {
                width,
                axis: Some(axis),
                slice: Some(slice),
                range: Some((lo, hi)),
                ..Default::default()
            };
            let r = render_scalar(&mesh, &values, &opt).map_err(|e| err(VALIDATION, e.to_string()))?;
            if r.cells_drawn == 0 {
                return Err(err(
                    VALIDATION,
                    format!("{rel}: the plane {} = {slice} cuts no cell of the mesh", AXES[axis]),
                ));
            }
            let png = r.image.to_png().map_err(|e| err(INTERNAL, e.to_string()))?;
            let b64 = base64(&png);
            let mut meta = json!({"case": rel, "measure": measure,
                "min": if none { Value::Null } else { json!(min) }, "max": if none { Value::Null } else { json!(max) },
                "non_finite": values.len() - finite.len(), "range": [lo, hi], "cells": mesh.n_cells(),
                "cells_drawn": r.cells_drawn, "axis": AXES[axis], "slice": slice, "width": r.image.width,
                "height": r.image.height, "png_bytes": png.len()});
            if width != asked {
                meta["reduced_from"] = json!(asked);
            }
            if b64.len() + text_len(&meta) <= TEXT_BUDGET {
                return Ok(Answer::Image { png_base64: b64, meta });
            }
            if width == MIN_PREVIEW_WIDTH {
                return Err(err(
                    VALIDATION,
                    format!("{rel}: the preview is over the size budget even {MIN_PREVIEW_WIDTH} pixels wide"),
                ));
            }
            width = (width / 2).max(MIN_PREVIEW_WIDTH);
        }
    }

    pub(crate) fn import_mesh(&self, args: &Value) -> Result<Value, RpcError> {
        let given = str_arg(args, "path");
        let src = self.project.resolve(given).map_err(perr)?;
        let src_rel = self.rel(&src);
        let meta = match std::fs::metadata(&src) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(err(
                    NOT_FOUND,
                    format!("import_mesh: {src_rel}: no such file or folder"),
                ))
            }
            Err(e) => return Err(err(INTERNAL, format!("import_mesh: {src_rel}: {e}"))),
        };
        // read first: nothing is written for a source no reader reads
        let (mesh, notes, format, files, folder) = if meta.is_dir() {
            let sub = format!("{src_rel}/polyMesh");
            let dir = match self.project.resolve(&sub) {
                Ok(p) if p.is_dir() => p,
                Ok(_) => src.clone(),
                Err(e) => return Err(perr(e)),
            };
            let mesh = rcfd_io::openfoam::read_dir(&dir).map_err(|e| {
                err(
                    VALIDATION,
                    format!("import_mesh: {src_rel} is not an OpenFOAM polyMesh folder: {e}"),
                )
            })?;
            let mut files = Vec::new();
            for name in OPENFOAM_FILES.iter().chain(&OPENFOAM_ZONE_FILES) {
                let p = dir.join(name);
                if p.is_file() {
                    files.push((name.to_string(), atomic::read(&p).map_err(perr)?));
                }
            }
            (mesh, Vec::new(), "openfoam", files, true)
        } else {
            let bytes = atomic::read(&src).map_err(perr)?;
            let (mesh, notes) = rcfd_io::read_msh_with_notes(&src).map_err(|e| {
                err(
                    VALIDATION,
                    format!("import_mesh: {src_rel}: no mesh reader reads it: {e}"),
                )
            })?;
            // the family rule of rcfd_io::read_msh_with_notes
            let first = bytes.iter().copied().find(|b| !b.is_ascii_whitespace());
            let fluent = first == Some(b'(') || bytes.starts_with(&[0x1f, 0x8b]) || bytes.starts_with(b"\x89HDF");
            let name = src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (
                mesh,
                notes,
                if fluent { "fluent" } else { "gmsh" },
                vec![(name, bytes)],
                false,
            )
        };
        let name = match args.get("name").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None if folder => Path::new(&src_rel)
                .components()
                .rev()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .find(|c| !c.eq_ignore_ascii_case("polyMesh") && !c.eq_ignore_ascii_case("constant"))
                .unwrap_or_default(),
            None => files[0].0.clone(),
        };
        let src_ext = if folder {
            None
        } else {
            Some(
                Path::new(&files[0].0)
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned()),
            )
        };
        check_mesh_name(&name, src_ext.as_ref().map(|e| e.as_deref()))
            .map_err(|m| err(REFUSED, format!("import_mesh: name '{name}': {m}")))?;
        let root = self.project.root();
        // names differing only in letter case are one file on Windows
        if let Ok(rd) = std::fs::read_dir(root.join("meshes")) {
            for e in rd.flatten() {
                let other = e.file_name().to_string_lossy().into_owned();
                if other != name && other.eq_ignore_ascii_case(&name) {
                    return Err(err(
                        REFUSED,
                        format!("import_mesh: meshes/{other} exists; meshes/{name} would be the same file on Windows"),
                    ));
                }
            }
        }
        let target_rel = format!("meshes/{name}");
        let target = self.project.resolve(&target_rel).map_err(perr)?;
        let written = if folder {
            write_folder(&target, &target_rel, &files)?
        } else {
            write_file(&target, &target_rel, &files[0].1)?
        };
        let file_rel = |f: &str| {
            if folder {
                format!("{target_rel}/{f}")
            } else {
                target_rel.clone()
            }
        };
        let files_json: Vec<Value> = files
            .iter()
            .map(|(f, b)| json!({"path": file_rel(f), "bytes": b.len(), "hash": hash_bytes(b), "written": written}))
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
        let patches = patch_json(&mesh);
        let mut v = json!({
            "source": src_rel, "path": target_rel, "copied": written, "format": format, "files": files_json,
            "cells": mesh.n_cells(), "faces": mesh.n_faces(), "internal_faces": mesh.n_internal_faces(),
            "points": mesh.n_points(), "patches_total": patches.len(), "patches": patches,
            "zones_total": zones.len(), "zones": zones,
            "bounds": bounds(&mesh).map(|(lo, hi)| json!({"min": lo, "max": hi})),
            "notes": notes, "mesh_file": format!("../{target_rel}"),
        });
        cut_to(&mut v, "zones", TEXT_BUDGET);
        cut_to(&mut v, "patches", TEXT_BUDGET);
        Ok(v)
    }

    pub(crate) fn list_meshes(&self, args: &Value) -> Result<Value, RpcError> {
        let root = self.project.root();
        let mut entries = mesh_entries(root);
        let mut outside = Vec::new();
        let mut unread = Vec::new();
        for c in self.project.cases().map_err(perr)? {
            let is_json = match c.rsplit_once('.').map(|(_, e)| e) {
                Some("toml") => false,
                Some("json") => true,
                _ => continue,
            };
            let path = root.join(&c);
            let text = match atomic::read(&path) {
                Ok(b) => String::from_utf8(b).map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            let parsed = text.and_then(|t| {
                if is_json {
                    serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())
                } else {
                    toml::from_str::<Value>(&t).map_err(|e| e.to_string())
                }
            });
            let v = match parsed {
                Ok(v) => v,
                Err(m) => {
                    unread.push(json!({"path": c, "message": m}));
                    continue;
                }
            };
            if v.pointer("/mesh/source").and_then(Value::as_str) != Some("file") {
                continue;
            }
            let file = v.pointer("/mesh/file").and_then(Value::as_str).unwrap_or_default();
            if file.is_empty() {
                continue;
            }
            let base = path.parent().unwrap_or(root);
            match resolve_mesh_file(root, base, file) {
                Ok((p, rel)) => {
                    let same = |e: &str| {
                        if cfg!(windows) {
                            e.eq_ignore_ascii_case(&rel)
                        } else {
                            e == rel
                        }
                    };
                    match entries.iter_mut().find(|e| same(&e.path)) {
                        Some(e) => e.used_by.push(c.clone()),
                        None => outside.push(json!({"case": c, "mesh_file": file, "path": rel,
                            "exists": p.exists(), "refused": null})),
                    }
                }
                Err(m) => outside.push(json!({"case": c, "mesh_file": file, "path": null, "exists": false,
                    "refused": m})),
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        let limit = u64_arg(args, "limit").unwrap_or(50) as usize;
        let (outside_total, unread_total) = (outside.len(), unread.len());
        outside.truncate(20);
        unread.truncate(20);
        let mut v = json!({"meshes": [], "total": entries.len(), "offset": offset, "next_offset": null,
            "outside": outside, "outside_total": outside_total, "unread": unread, "unread_total": unread_total});
        let mut size = text_len(&v);
        let mut out = Vec::new();
        for e in entries.iter().skip(offset).take(limit) {
            let row = json!({"path": e.path, "kind": e.kind, "bytes": e.bytes, "files": e.files,
                "used_by": e.used_by});
            let n = text_len(&row) + 1;
            if size + n > TEXT_BUDGET {
                break;
            }
            size += n;
            out.push(row);
        }
        let end = offset + out.len();
        v["meshes"] = json!(out);
        if end < entries.len() {
            v["next_offset"] = json!(end);
        }
        Ok(v)
    }
}

/// The name rule of an imported mesh (ADR 0187 M5): one path component of letters, digits, `-`, `_` and `.`, starting
/// with a letter or digit, not ending in `.`, at most [`MAX_MESH_NAME`] characters, not a Windows device name; a file
/// (`ext` is `Some(its source's extension)`) keeps its source's extension.
pub fn check_mesh_name(name: &str, ext: Option<Option<&str>>) -> Result<(), String> {
    if name.is_empty() {
        return Err("give a name for the mesh in meshes/".into());
    }
    let n = name.chars().count();
    if n > MAX_MESH_NAME {
        return Err(format!("at most {MAX_MESH_NAME} characters (this name has {n})"));
    }
    if name.contains(['/', '\\']) {
        return Err("no path separators: the mesh is imported directly in meshes/".into());
    }
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    {
        return Err(format!(
            "{c:?} is not allowed: use letters, digits, '-', '_' and '.' only"
        ));
    }
    if !name.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        return Err("start the name with a letter or a digit".into());
    }
    if name.ends_with('.') {
        return Err("a name cannot end in '.'".into());
    }
    let stem = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    if crate::cases::RESERVED.contains(&stem.as_str()) {
        return Err(format!("'{name}' is a reserved device name on Windows"));
    }
    if let Some(want) = ext {
        let have = Path::new(name).extension().map(|e| e.to_string_lossy().into_owned());
        let same = match (want, have.as_deref()) {
            (None, None) => true,
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        };
        if !same {
            return Err(format!(
                "a mesh file keeps its extension ({})",
                want.map_or("none".to_string(), |e| format!(".{e}"))
            ));
        }
    }
    Ok(())
}

/// Write an imported mesh file at `target` without replacing anything: `Ok(true)` written, `Ok(false)` already there
/// with the same bytes; another file there is 1004.
fn write_file(target: &Path, rel: &str, bytes: &[u8]) -> Result<bool, RpcError> {
    if target.is_dir() {
        return Err(err(REFUSED, format!("import_mesh: {rel} exists and is a folder")));
    }
    match std::fs::read(target) {
        Ok(old) if old == bytes => return Ok(false),
        Ok(_) => {
            return Err(err(
                REFUSED,
                format!("import_mesh: {rel} exists with other content; import the mesh under another name"),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(err(INTERNAL, format!("import_mesh: {rel}: {e}"))),
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err(INTERNAL, format!("import_mesh: {}: {e}", dir.display())))?;
    }
    atomic::write_new(target, bytes).map_err(|e| match e {
        rcfd_project::ProjectError::Exists { .. } => err(REFUSED, format!("import_mesh: {rel} exists")),
        e => err(INTERNAL, format!("import_mesh: {rel}: {e}")),
    })?;
    Ok(true)
}

/// Write an imported OpenFOAM mesh as the folder `target` holding `files` (name, bytes), never replacing anything:
/// `Ok(true)` written, `Ok(false)` when every file is there with the same bytes; anything else there is 1004. On a
/// failure the files written are removed again.
fn write_folder(target: &Path, rel: &str, files: &[(String, Vec<u8>)]) -> Result<bool, RpcError> {
    if target.exists() {
        if !target.is_dir() {
            return Err(err(REFUSED, format!("import_mesh: {rel} exists and is not a folder")));
        }
        let same = files
            .iter()
            .all(|(f, b)| std::fs::read(target.join(f)).is_ok_and(|old| old == *b));
        return if same {
            Ok(false)
        } else {
            Err(err(
                REFUSED,
                format!("import_mesh: {rel}/ exists with other content; import the mesh under another name"),
            ))
        };
    }
    std::fs::create_dir_all(target).map_err(|e| err(INTERNAL, format!("import_mesh: {rel}: {e}")))?;
    let mut done: Vec<PathBuf> = Vec::new();
    for (f, b) in files {
        let p = target.join(f);
        if let Err(e) = atomic::write_new(&p, b) {
            for w in &done {
                let _ = std::fs::remove_file(w);
            }
            let _ = std::fs::remove_dir(target);
            return Err(err(INTERNAL, format!("import_mesh: {rel}/{f}: {e}")));
        }
        done.push(p);
    }
    Ok(true)
}

/// One entry of `list_meshes`.
struct Entry {
    path: String,
    kind: &'static str,
    bytes: u64,
    files: usize,
    used_by: Vec<String>,
}

fn is_openfoam_dir(d: &Path) -> bool {
    OPENFOAM_FILES.iter().all(|f| d.join(f).is_file())
}

/// The meshes under `meshes/`: each file, except that a folder holding an OpenFOAM `polyMesh` (the files of
/// [`OPENFOAM_FILES`]) is one entry and its files are not listed.
fn mesh_entries(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("meshes")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if atomic::is_temp_name(&name) {
                continue;
            }
            let p = e.path();
            let Some(rel) = rcfd_project::paths::rel_string(root, &p) else {
                continue;
            };
            match e.file_type() {
                Ok(t) if t.is_dir() && is_openfoam_dir(&p) => {
                    let (mut bytes, mut files) = (0, 0);
                    for f in std::fs::read_dir(&p).into_iter().flatten().flatten() {
                        if f.file_type().is_ok_and(|t| t.is_file()) {
                            bytes += f.metadata().map_or(0, |m| m.len());
                            files += 1;
                        }
                    }
                    out.push(Entry {
                        path: rel,
                        kind: "openfoam",
                        bytes,
                        files,
                        used_by: Vec::new(),
                    });
                }
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() => out.push(Entry {
                    path: rel,
                    kind: "file",
                    bytes: e.metadata().map_or(0, |m| m.len()),
                    files: 1,
                    used_by: Vec::new(),
                }),
                _ => {}
            }
        }
    }
    out
}
