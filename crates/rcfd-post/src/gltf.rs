//! glTF 2.0 binary (GLB) export of boundary surfaces with scalar colouring (F-117).
//!
//! Written from the public Khronos glTF 2.0 specification. Each selected boundary face is
//! flat shaded with the colour of its value (cell value of the owner cell, or an explicit
//! per-face value), so what a viewer shows is the cell data. Vertices are not shared between
//! faces, hence the file size is linear in the face count. The raw value is also stored in the
//! application specific attribute `_SCALAR` (float). Coordinates are written as in the mesh
//! (no axis swap); glTF viewers assume +Y up, so a case with z up appears rotated.

use rcfd_core::vec3::{cross, dot, norm, sub, unit, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use serde_json::{json, Value};

use crate::render::colormap;

/// Where the scalar values come from.
#[derive(Debug, Clone, Copy)]
pub enum GltfScalar<'a> {
    /// One value per cell; a boundary face takes the value of its owner cell.
    Cell(&'a [f64]),
    /// One value per face over all faces of the mesh (internal faces first).
    Face(&'a [f64]),
}

/// Export options.
#[derive(Debug, Clone, Default)]
pub struct GltfOptions {
    /// Colour range; `None` uses the min and max over the exported faces.
    pub range: Option<(f64, f64)>,
    /// Name of the scalar, written into the mesh `extras` and the file `asset.extras`.
    pub scalar_name: String,
}

/// Summary of what was written (also embedded in the GLB JSON as `asset.extras`).
#[derive(Debug, Clone, PartialEq)]
pub struct GltfStats {
    /// Mesh faces exported.
    pub faces: usize,
    /// Triangles written.
    pub triangles: usize,
    /// Vertices written.
    pub vertices: usize,
    /// Colour range used.
    pub range: (f64, f64),
}

fn pad4(buf: &mut Vec<u8>, byte: u8) {
    while buf.len() % 4 != 0 {
        buf.push(byte);
    }
}

fn push_f32(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&(v as f32).to_le_bytes());
}

/// Export the listed boundary patches as one GLB mesh (one primitive per patch).
/// `scalar = None` writes uncoloured geometry.
pub fn glb_patches(
    mesh: &Mesh,
    patches: &[&str],
    scalar: Option<GltfScalar>,
    opt: &GltfOptions,
) -> Result<(Vec<u8>, GltfStats)> {
    if patches.is_empty() {
        return Err(Error::Other("glb export needs at least one patch".into()));
    }
    let mut ranges = Vec::new();
    for name in patches {
        let p = mesh
            .patches()
            .iter()
            .find(|p| p.name == *name)
            .ok_or_else(|| Error::Mesh(format!("unknown patch '{name}'")))?;
        ranges.push(p.faces());
    }
    let value_of = |f: usize| -> Result<f64> {
        match scalar {
            None => Ok(0.0),
            Some(GltfScalar::Cell(v)) => v.get(mesh.owner()[f]).copied().ok_or_else(|| {
                Error::Other(format!(
                    "cell field has {} values, mesh needs {}",
                    v.len(),
                    mesh.n_cells()
                ))
            }),
            Some(GltfScalar::Face(v)) => v.get(f).copied().ok_or_else(|| {
                Error::Other(format!(
                    "face field has {} values, mesh needs {}",
                    v.len(),
                    mesh.n_faces()
                ))
            }),
        }
    };
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    if scalar.is_some() {
        for r in &ranges {
            for f in r.clone() {
                let v = value_of(f)?;
                if !v.is_finite() {
                    return Err(Error::Numerical(format!("non-finite value on face {f}")));
                }
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    } else {
        lo = 0.0;
        hi = 1.0;
    }
    let (lo, hi) = match opt.range {
        Some((a, b)) => {
            if a.is_nan() || b.is_nan() || b <= a {
                return Err(Error::Other("colour range needs max > min".into()));
            }
            (a, b)
        }
        None => (lo, hi),
    };
    let span = if hi > lo { hi - lo } else { 1.0 };

    let pts = mesh.points();
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut prims = Vec::new();
    let mut stats = GltfStats {
        faces: 0,
        triangles: 0,
        vertices: 0,
        range: (lo, hi),
    };

    for (name, r) in patches.iter().zip(&ranges) {
        let mut pos: Vec<Vec3> = Vec::new();
        let mut nrm: Vec<Vec3> = Vec::new();
        let mut col: Vec<[u8; 4]> = Vec::new();
        let mut val: Vec<f32> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for f in r.clone() {
            let ids = mesh.face_points(f);
            if ids.len() < 3 {
                continue;
            }
            let n = unit(mesh.face_area_vectors()[f]);
            let v = value_of(f)?;
            let c = if scalar.is_some() {
                let rgb = colormap((v - lo) / span);
                [rgb[0], rgb[1], rgb[2], 255]
            } else {
                [200, 200, 200, 255]
            };
            let base = pos.len() as u32;
            for &i in ids {
                pos.push(pts[i]);
            }
            let centre = if ids.len() > 4 {
                pos.push(mesh.face_centres()[f]);
                true
            } else {
                false
            };
            let nv = pos.len() - base as usize;
            for _ in 0..nv {
                nrm.push(n);
                col.push(c);
                val.push(v as f32);
            }
            let m = ids.len() as u32;
            if centre {
                let cidx = base + m;
                for k in 0..m {
                    idx.extend_from_slice(&[cidx, base + k, base + (k + 1) % m]);
                }
                stats.triangles += m as usize;
            } else {
                for k in 1..m - 1 {
                    idx.extend_from_slice(&[base, base + k, base + k + 1]);
                }
                stats.triangles += (m - 2) as usize;
            }
            stats.faces += 1;
        }
        if pos.is_empty() {
            return Err(Error::Mesh(format!("patch '{name}' has no faces to export")));
        }
        stats.vertices += pos.len();

        // Section helper: append aligned data, add bufferView and accessor, return accessor index.
        let mut add = |bytes: &[u8],
                       target: u32,
                       comp: u32,
                       ty: &str,
                       count: usize,
                       norm_: bool,
                       minmax: Option<(Value, Value)>| {
            pad4(&mut bin, 0);
            let off = bin.len();
            bin.extend_from_slice(bytes);
            views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": bytes.len(), "target": target}));
            let mut a = json!({"bufferView": views.len() - 1, "componentType": comp, "count": count, "type": ty});
            if norm_ {
                a["normalized"] = json!(true);
            }
            if let Some((mn, mx)) = minmax {
                a["min"] = mn;
                a["max"] = mx;
            }
            accessors.push(a);
            accessors.len() - 1
        };
        let mut pb = Vec::with_capacity(pos.len() * 12);
        let (mut mn, mut mx) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in &pos {
            for k in 0..3 {
                // min/max must describe the stored f32 values
                let q = p[k] as f32 as f64;
                mn[k] = mn[k].min(q);
                mx[k] = mx[k].max(q);
                push_f32(&mut pb, p[k]);
            }
        }
        let mut nb = Vec::with_capacity(nrm.len() * 12);
        for n in &nrm {
            for k in 0..3 {
                push_f32(&mut nb, n[k]);
            }
        }
        let cb: Vec<u8> = col.iter().flat_map(|c| c.iter().copied()).collect();
        let mut vb = Vec::with_capacity(val.len() * 4);
        for v in &val {
            vb.extend_from_slice(&v.to_le_bytes());
        }
        let ib: Vec<u8> = idx.iter().flat_map(|i| i.to_le_bytes()).collect();
        let a_pos = add(&pb, 34962, 5126, "VEC3", pos.len(), false, Some((json!(mn), json!(mx))));
        let a_nrm = add(&nb, 34962, 5126, "VEC3", nrm.len(), false, None);
        let mut attrs = json!({"POSITION": a_pos, "NORMAL": a_nrm});
        if scalar.is_some() {
            attrs["COLOR_0"] = json!(add(&cb, 34962, 5121, "VEC4", col.len(), true, None));
            let (vmin, vmax) = val
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
            attrs["_SCALAR"] = json!(add(
                &vb,
                34962,
                5126,
                "SCALAR",
                val.len(),
                false,
                Some((json!([vmin]), json!([vmax])))
            ));
        }
        let a_idx = add(
            &ib,
            34963,
            5125,
            "SCALAR",
            idx.len(),
            false,
            Some((json!([0]), json!([pos.len() - 1]))),
        );
        let mut prim = json!({"attributes": attrs, "indices": a_idx, "mode": 4, "material": 0});
        prim["extras"] = json!({"patch": name});
        prims.push(prim);
    }
    pad4(&mut bin, 0);

    let doc = json!({
        "asset": {"version": "2.0", "generator": "rcfd", "extras": {
            "scalar": opt.scalar_name, "range": [lo, hi], "colormap": "viridis",
            "faces": stats.faces, "triangles": stats.triangles
        }},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "rcfd-surface"}],
        "meshes": [{"name": "boundary", "primitives": prims}],
        "materials": [{"name": "flat", "doubleSided": true,
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0}}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": views,
        "accessors": accessors,
    });
    let mut js = serde_json::to_vec(&doc).map_err(|e| Error::Io(e.to_string()))?;
    pad4(&mut js, b' ');
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    Ok((out, stats))
}

/// Write a GLB file (see [`glb_patches`]).
pub fn write_glb(
    path: impl AsRef<std::path::Path>,
    mesh: &Mesh,
    patches: &[&str],
    scalar: Option<GltfScalar>,
    opt: &GltfOptions,
) -> Result<GltfStats> {
    let (bytes, stats) = glb_patches(mesh, patches, scalar, opt)?;
    std::fs::write(path, bytes)?;
    Ok(stats)
}

/// Structural validation of a GLB byte buffer against the glTF 2.0 container and accessor rules that matter for
/// loading (ADR 0011 section 4; the rules and their pointers are in [`crate::glb_check`]). Returns the parsed JSON.
/// This is a self-check, not a replacement for the Khronos validator.
pub fn check_glb(bytes: &[u8]) -> std::result::Result<Value, String> {
    crate::glb_check::validate_glb(bytes).map_err(|e| e.to_string())
}

/// Geometric consistency used by the tests: the triangle normals of a primitive agree with the
/// outward face normals (counter-clockwise winding seen from outside). Returns the number of
/// triangles that disagree.
pub fn wrong_winding(bytes: &[u8]) -> usize {
    let doc = match check_glb(bytes) {
        Ok(d) => d,
        Err(_) => return usize::MAX,
    };
    let jl = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let bin = &bytes[28 + jl..];
    let acc = |i: usize| &doc["accessors"][i];
    let view = |a: &Value| &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let f32s = |a: &Value| -> Vec<f32> {
        let o = view(a)["byteOffset"].as_u64().unwrap() as usize;
        let n = a["count"].as_u64().unwrap() as usize * if a["type"] == "VEC3" { 3 } else { 1 };
        (0..n)
            .map(|k| f32::from_le_bytes(bin[o + 4 * k..o + 4 * k + 4].try_into().unwrap()))
            .collect()
    };
    let mut bad = 0;
    for p in doc["meshes"][0]["primitives"].as_array().unwrap() {
        let pos = f32s(acc(p["attributes"]["POSITION"].as_u64().unwrap() as usize));
        let nrm = f32s(acc(p["attributes"]["NORMAL"].as_u64().unwrap() as usize));
        let ia = acc(p["indices"].as_u64().unwrap() as usize);
        let o = view(ia)["byteOffset"].as_u64().unwrap() as usize;
        let n = ia["count"].as_u64().unwrap() as usize;
        let idx: Vec<usize> = (0..n)
            .map(|k| u32::from_le_bytes(bin[o + 4 * k..o + 4 * k + 4].try_into().unwrap()) as usize)
            .collect();
        let pt = |i: usize| -> Vec3 { [pos[3 * i] as f64, pos[3 * i + 1] as f64, pos[3 * i + 2] as f64] };
        for t in idx.chunks(3) {
            let n_t = cross(sub(pt(t[1]), pt(t[0])), sub(pt(t[2]), pt(t[0])));
            let n_f = [nrm[3 * t[0]] as f64, nrm[3 * t[0] + 1] as f64, nrm[3 * t[0] + 2] as f64];
            if norm(n_t) > 0.0 && dot(n_t, n_f) <= 0.0 {
                bad += 1;
            }
        }
    }
    bad
}
