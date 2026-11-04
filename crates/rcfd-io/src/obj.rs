//! Wavefront OBJ surface reader (F-274, ADR 0184 D5).
//!
//! Written from the public description of the OBJ format (polygonal subset): `v x y z [w]` vertices, `f` faces whose
//! references are `v`, `v/vt`, `v//vn` or `v/vt/vn`, 1-based, or negative to count back from the last vertex defined so
//! far; `o` names an object and `g` a group; `#` starts a comment and a trailing `\` continues a line.
//!
//! * Each `o` object is one patch; faces before the first `o` go to the patch `default`; a repeated name adds to the same
//!   patch. With [`ObjPatches::Groups`] the first name of each `g` line is the patch instead.
//! * Faces with more than three vertices are triangulated by ear clipping in the plane of the Newell normal, so a planar
//!   non-convex polygon is covered exactly; a polygon with no ear (self-intersecting or degenerate) becomes a fan from its
//!   first vertex, with a note.
//! * Texture, normal, smoothing, material, line, point and curve statements are ignored with a note; free-form surfaces
//!   (`surf`) are refused. Vertex colours after `x y z` are accepted and ignored.
//! * The length unit is required (ADR 0184 D3); the result is welded as described in [`crate::common::finish_import`].

use crate::common::{finish_import, length_factor, SurfaceImport};
use crate::guard::{check_file_size, finite, MAX_ITEMS};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashMap};

/// Which OBJ statement makes a patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjPatches {
    /// One patch per `o` object (the default, ADR 0184 D5).
    #[default]
    Objects,
    /// One patch per `g` group (first name of the `g` line), for exporters that write only groups.
    Groups,
}

/// Reader options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjOptions {
    /// Patch source.
    pub patches: ObjPatches,
}

/// Largest non-convex polygon that is ear-clipped (the work grows with the cube of the vertex count in the worst case).
pub const MAX_EAR_CLIP_VERTICES: usize = 1024;

const IGNORED: &[&str] = &[
    "vt",
    "vn",
    "vp",
    "s",
    "mtllib",
    "usemtl",
    "l",
    "p",
    "curv",
    "curv2",
    "cstype",
    "deg",
    "bmat",
    "step",
    "parm",
    "trim",
    "hole",
    "scrv",
    "sp",
    "end",
    "con",
    "mg",
    "lod",
    "maplib",
    "usemap",
    "bevel",
    "c_interp",
    "d_interp",
    "shadow_obj",
    "trace_obj",
    "ctech",
    "stech",
    "call",
    "csh",
];

/// Read an OBJ file from memory. `unit` is the length unit of the coordinates in the file.
pub fn read_bytes(bytes: &[u8], unit: &str, opt: &ObjOptions) -> Result<SurfaceImport> {
    check_file_size(bytes.len(), "OBJ")?;
    let factor = length_factor(unit)?;
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let err = |line: usize, msg: String| Error::Io(format!("OBJ: line {line}: {msg}"));

    let mut points: Vec<Vec3> = Vec::new();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut tags: Vec<u32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut name_index: HashMap<String, u32> = HashMap::new();
    let mut current: Option<u32> = None;
    let mut ignored: BTreeMap<String, usize> = BTreeMap::new();
    let mut unknown: BTreeMap<String, usize> = BTreeMap::new();
    let (mut polygons, mut fans, mut first_fan, mut extra_values) = (0usize, 0usize, 0usize, 0usize);
    let mut poly: Vec<usize> = Vec::new();

    let mut select = |name: &str, names: &mut Vec<String>| -> u32 {
        let name = if name.is_empty() { "default" } else { name };
        *name_index.entry(name.to_string()).or_insert_with(|| {
            names.push(name.to_string());
            (names.len() - 1) as u32
        })
    };

    let mut lines = text.split('\n').enumerate().peekable();
    while let Some((ln, raw)) = lines.next() {
        let line = ln + 1;
        // comment, then continuation lines (`\` at the end)
        let strip = |s: &str| -> String {
            let s = s.strip_suffix('\r').unwrap_or(s);
            match s.find('#') {
                Some(i) => s[..i].to_string(),
                None => s.to_string(),
            }
        };
        let mut logical = strip(raw);
        while logical.trim_end().ends_with('\\') {
            let t = logical.trim_end();
            logical = t[..t.len() - 1].to_string();
            match lines.next() {
                Some((_, next)) => {
                    logical.push(' ');
                    logical.push_str(&strip(next));
                }
                None => break,
            }
        }
        let mut tok = logical.split_ascii_whitespace();
        let Some(kw) = tok.next() else { continue };
        let rest: Vec<&str> = tok.collect();
        match kw {
            "v" => {
                if rest.len() < 3 {
                    return Err(err(line, format!("'v' needs x y z, found {} values", rest.len())));
                }
                if rest.len() > 7 {
                    return Err(err(
                        line,
                        format!("'v' has {} values, at most 7 are allowed", rest.len()),
                    ));
                }
                let mut p = [0.0; 3];
                for (k, t) in rest.iter().enumerate() {
                    let v: f64 = t.parse().map_err(|_| err(line, format!("'{t}' is not a number")))?;
                    let v = finite(v, || format!("OBJ: line {line}"))?;
                    if k < 3 {
                        p[k] = v * factor;
                    }
                }
                if rest.len() > 3 {
                    extra_values += 1;
                }
                if points.len() >= MAX_ITEMS {
                    return Err(err(line, "too many vertices".into()));
                }
                points.push(p);
            }
            "f" | "fo" => {
                if rest.len() < 3 {
                    return Err(err(
                        line,
                        format!("a face needs at least 3 vertices, found {}", rest.len()),
                    ));
                }
                poly.clear();
                for t in &rest {
                    let parts: Vec<&str> = t.split('/').collect();
                    if parts.len() > 3 {
                        return Err(err(line, format!("'{t}' is not a v, v/vt, v//vn or v/vt/vn reference")));
                    }
                    for (k, s) in parts.iter().enumerate() {
                        if k > 0 && s.is_empty() {
                            continue;
                        }
                        let r: i64 = s
                            .parse()
                            .map_err(|_| err(line, format!("'{t}': '{s}' is not an integer index")))?;
                        if r == 0 {
                            return Err(err(line, format!("'{t}': index 0 is not valid (indices start at 1)")));
                        }
                        if k == 0 {
                            let n = points.len() as i64;
                            let i = if r > 0 { r - 1 } else { n + r };
                            if i < 0 || i >= n {
                                return Err(err(
                                    line,
                                    format!("'{t}': vertex {r} does not exist ({n} vertices defined so far)"),
                                ));
                            }
                            poly.push(i as usize);
                        }
                    }
                }
                let tag = match current {
                    Some(t) => t,
                    None => {
                        let t = select("default", &mut names);
                        current = Some(t);
                        t
                    }
                };
                let before = tris.len();
                if poly.len() > 3 {
                    polygons += 1;
                }
                if triangulate(&poly, &points, &mut tris).map_err(|m| err(line, m))? {
                    fans += 1;
                    if first_fan == 0 {
                        first_fan = line;
                    }
                }
                if tris.len() > MAX_ITEMS {
                    return Err(err(line, "too many triangles".into()));
                }
                tags.resize(tags.len() + tris.len() - before, tag);
            }
            "o" => {
                if opt.patches == ObjPatches::Objects {
                    current = Some(select(&rest.join(" "), &mut names));
                } else {
                    *ignored.entry("o".into()).or_default() += 1;
                }
            }
            "g" => {
                if opt.patches == ObjPatches::Groups {
                    current = Some(select(rest.first().copied().unwrap_or(""), &mut names));
                } else {
                    *ignored.entry("g".into()).or_default() += 1;
                }
            }
            "surf" => {
                return Err(err(
                    line,
                    "free-form surfaces ('surf') cannot be read; export the model as polygons".into(),
                ))
            }
            k if IGNORED.contains(&k) => *ignored.entry(k.to_string()).or_default() += 1,
            k => {
                if unknown.len() < 16 || unknown.contains_key(k) {
                    let k: String = k.chars().take(24).collect();
                    *unknown.entry(k).or_default() += 1;
                }
            }
        }
    }
    let mut notes = Vec::new();
    if !ignored.is_empty() {
        let list: Vec<String> = ignored.iter().map(|(k, n)| format!("{n} {k}")).collect();
        notes.push(format!("statements ignored: {}", list.join(", ")));
    }
    if !unknown.is_empty() {
        let list: Vec<String> = unknown.iter().map(|(k, n)| format!("{n} '{k}'")).collect();
        notes.push(format!("unknown statements ignored: {}", list.join(", ")));
    }
    if extra_values > 0 {
        notes.push(format!(
            "{extra_values} vertices carry values after x y z (w or colour), ignored"
        ));
    }
    if polygons > 0 {
        notes.push(format!("{polygons} polygons with more than 3 vertices triangulated"));
    }
    if fans > 0 {
        notes.push(format!(
            "{fans} polygons have no ear (self-intersecting or degenerate) and were triangulated as fans, first at line {first_fan}"
        ));
    }
    finish_import(points, tris, tags, names, notes, "OBJ")
}

/// Read an OBJ file from disk.
pub fn read_file(path: impl AsRef<std::path::Path>, unit: &str, opt: &ObjOptions) -> Result<SurfaceImport> {
    let path = path.as_ref();
    let meta = std::fs::metadata(path)?;
    check_file_size(meta.len() as usize, "OBJ")?;
    let b = std::fs::read(path)?;
    read_bytes(&b, unit, opt).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}

/// Append the triangles of polygon `poly` (vertex ids, counter-clockwise seen from outside) to `out`, keeping its
/// orientation. Returns `true` when the polygon had no ear and a fan from its first vertex was used.
fn triangulate(poly: &[usize], pts: &[Vec3], out: &mut Vec<[usize; 3]>) -> std::result::Result<bool, String> {
    let n = poly.len();
    if n == 3 {
        out.push([poly[0], poly[1], poly[2]]);
        return Ok(false);
    }
    let fan = |out: &mut Vec<[usize; 3]>, ids: &[usize]| {
        for k in 1..ids.len() - 1 {
            out.push([ids[0], ids[k], ids[k + 1]]);
        }
    };
    // Newell normal: twice the vector area, right-handed with the vertex order
    let mut nrm = [0.0f64; 3];
    for i in 0..n {
        let (p, q) = (pts[poly[i]], pts[poly[(i + 1) % n]]);
        nrm[0] += (p[1] - q[1]) * (p[2] + q[2]);
        nrm[1] += (p[2] - q[2]) * (p[0] + q[0]);
        nrm[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    let k = (0..3)
        .max_by(|&a, &b| nrm[a].abs().total_cmp(&nrm[b].abs()))
        .unwrap_or(2);
    if nrm[k] == 0.0 || !nrm[k].is_finite() {
        fan(out, poly);
        return Ok(true);
    }
    // project by dropping axis k; the cyclic order (k+1, k+2) keeps a counter-clockwise loop counter-clockwise when
    // nrm[k] > 0, so `s` turns every orientation test into "positive = convex"
    let (a0, a1) = ((k + 1) % 3, (k + 2) % 3);
    let s = nrm[k].signum();
    let uv: Vec<[f64; 2]> = poly.iter().map(|&i| [pts[i][a0], pts[i][a1]]).collect();
    let cross =
        |o: [f64; 2], a: [f64; 2], b: [f64; 2]| s * ((a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]));
    let convex = (0..n).all(|i| cross(uv[(i + n - 1) % n], uv[i], uv[(i + 1) % n]) > 0.0);
    if convex {
        fan(out, poly);
        return Ok(false);
    }
    if n > MAX_EAR_CLIP_VERTICES {
        return Err(format!(
            "a non-convex polygon of {n} vertices exceeds the ear-clipping limit of {MAX_EAR_CLIP_VERTICES}"
        ));
    }
    let mut rem: Vec<usize> = (0..n).collect();
    while rem.len() > 3 {
        let m = rem.len();
        let mut ear = None;
        for step in 0..m {
            let i = (1 + step) % m;
            let (a, b, c) = (rem[(i + m - 1) % m], rem[i], rem[(i + 1) % m]);
            if cross(uv[a], uv[b], uv[c]) <= 0.0 {
                continue;
            }
            let blocked = rem.iter().any(|&p| {
                p != a
                    && p != b
                    && p != c
                    && cross(uv[a], uv[b], uv[p]) >= 0.0
                    && cross(uv[b], uv[c], uv[p]) >= 0.0
                    && cross(uv[c], uv[a], uv[p]) >= 0.0
            });
            if !blocked {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        match ear {
            Some((i, [a, b, c])) => {
                out.push([poly[a], poly[b], poly[c]]);
                rem.remove(i);
            }
            None => {
                let ids: Vec<usize> = rem.iter().map(|&r| poly[r]).collect();
                fan(out, &ids);
                return Ok(true);
            }
        }
    }
    out.push([poly[rem[0]], poly[rem[1]], poly[rem[2]]]);
    Ok(false)
}
