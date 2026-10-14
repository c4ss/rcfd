//! Structural validation of a GLB (glTF 2.0 binary) byte buffer against the container and accessor rules of the
//! glTF 2.0 specification that matter for loading (F-117, ADR 0011 section 4, rules G1-G8).
//!
//! Every failure is a [`GlbError`] with a JSON pointer into the document (`/accessors/3/count`) or a byte offset
//! (`@12`). The checker never panics and never allocates from counts declared in the file: all offsets use checked
//! arithmetic and all data reads are bounds checked against the BIN chunk. It is a self-check, not a replacement
//! for the Khronos validator, which is run separately (see docs/reference-data.md).

use serde_json::Value;
use std::fmt;

/// Largest GLB this checker reads.
pub const MAX_GLB_BYTES: usize = 1 << 30;

/// A validation failure with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbError {
    /// JSON pointer (`/accessors/3/count`) or byte offset (`@16`) of the problem.
    pub pointer: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for GlbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.pointer, self.message)
    }
}

type R<T> = std::result::Result<T, GlbError>;

fn err<T>(pointer: impl Into<String>, message: impl Into<String>) -> R<T> {
    Err(GlbError {
        pointer: pointer.into(),
        message: message.into(),
    })
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    let s = b.get(o..o.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Optional unsigned integer member; a present member of another type is an error.
fn opt_u64(v: &Value, key: &str, ptr: &str) -> R<Option<u64>> {
    match v.get(key) {
        None => Ok(None),
        Some(x) => x.as_u64().map(Some).ok_or_else(|| GlbError {
            pointer: format!("{ptr}/{key}"),
            message: "must be a non-negative integer".into(),
        }),
    }
}

fn req_u64(v: &Value, key: &str, ptr: &str) -> R<u64> {
    opt_u64(v, key, ptr)?.map_or_else(|| err(format!("{ptr}/{key}"), "required member is missing"), Ok)
}

/// `true` when the (checked) value is missing (overflow) or larger than the limit.
fn exceeds(x: Option<usize>, limit: usize) -> bool {
    match x {
        None => true,
        Some(v) => v > limit,
    }
}

fn comp_size(c: u64) -> Option<usize> {
    match c {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

fn n_comp(t: &str) -> Option<usize> {
    match t {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" => Some(4),
        _ => None,
    }
}

struct View {
    offset: usize,
    len: usize,
    stride: Option<usize>,
    target: Option<u64>,
}

struct Acc {
    view: usize,
    offset: usize,
    ctype: u64,
    comps: usize,
    count: usize,
    normalized: bool,
    stride: usize,
}

/// Read element `e`, component `k` of an accessor as `f64` (the caller has checked the extent).
fn read_comp(bin: &[u8], views: &[View], a: &Acc, e: usize, k: usize) -> Option<f64> {
    let cs = comp_size(a.ctype)?;
    let o = views[a.view]
        .offset
        .checked_add(a.offset)?
        .checked_add(e.checked_mul(a.stride)?)?
        .checked_add(k.checked_mul(cs)?)?;
    let s = bin.get(o..o.checked_add(cs)?)?;
    Some(match a.ctype {
        5126 => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
        5125 => u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
        5123 => u16::from_le_bytes([s[0], s[1]]) as f64,
        5122 => i16::from_le_bytes([s[0], s[1]]) as f64,
        5121 => s[0] as f64,
        5120 => s[0] as i8 as f64,
        _ => return None,
    })
}

/// Validate a GLB. Returns the parsed JSON document when every rule holds.
pub fn validate_glb(bytes: &[u8]) -> R<Value> {
    if bytes.len() > MAX_GLB_BYTES {
        return err(
            "@0",
            format!("file of {} bytes exceeds the {MAX_GLB_BYTES} byte limit", bytes.len()),
        );
    }
    // ---- G1 container
    if bytes.get(0..4) != Some(b"glTF") {
        return err("@0", "bad magic (expected 'glTF')");
    }
    match u32_at(bytes, 4) {
        Some(2) => {}
        Some(v) => return err("@4", format!("version {v}, only 2 is supported")),
        None => return err("@4", "truncated header"),
    }
    match u32_at(bytes, 8) {
        Some(l) if l as usize == bytes.len() => {}
        Some(l) => {
            return err(
                "@8",
                format!("header length {l} differs from file length {}", bytes.len()),
            )
        }
        None => return err("@8", "truncated header"),
    }
    let mut pos = 12usize;
    let mut json: Option<&[u8]> = None;
    let mut bin: Option<&[u8]> = None;
    let mut idx = 0;
    while pos < bytes.len() {
        let Some(len) = u32_at(bytes, pos) else {
            return err(format!("@{pos}"), "truncated chunk header");
        };
        let len = len as usize;
        let Some(ty) = bytes.get(pos + 4..pos + 8) else {
            return err(format!("@{pos}"), "truncated chunk header");
        };
        let start = pos + 8;
        let Some(end) = start.checked_add(len).filter(|&e| e <= bytes.len()) else {
            return err(
                format!("@{pos}"),
                format!("chunk of {len} bytes runs past the end of the file"),
            );
        };
        if len % 4 != 0 {
            return err(format!("@{pos}"), format!("chunk length {len} is not a multiple of 4"));
        }
        match (idx, ty) {
            (0, b"JSON") => json = Some(&bytes[start..end]),
            (0, _) => return err(format!("@{}", pos + 4), "the first chunk must be JSON"),
            (_, b"BIN\0") if bin.is_none() => bin = Some(&bytes[start..end]),
            (_, b"JSON") => return err(format!("@{}", pos + 4), "second JSON chunk"),
            (_, b"BIN\0") => return err(format!("@{}", pos + 4), "second BIN chunk"),
            _ => {} // unknown chunk types are ignored by the specification
        }
        pos = end;
        idx += 1;
    }
    let Some(json) = json else {
        return err("@12", "no JSON chunk");
    };
    let doc: Value = match serde_json::from_slice(json) {
        Ok(d) => d,
        Err(e) => return err("@20", format!("JSON: {e}")),
    };
    if !doc.is_object() {
        return err("", "the glTF document must be a JSON object");
    }
    if doc["asset"]["version"].as_str() != Some("2.0") {
        return err("/asset/version", "must be the string \"2.0\"");
    }
    if let Some(req) = doc.get("extensionsRequired") {
        match req.as_array() {
            Some(a) if a.is_empty() => {}
            Some(a) => {
                return err(
                    "/extensionsRequired",
                    format!("requires extension(s) this checker does not know: {a:?}"),
                )
            }
            None => return err("/extensionsRequired", "must be an array"),
        }
    }
    let arr = |key: &str| -> R<&[Value]> {
        match doc.get(key) {
            None => Ok(&[]),
            Some(Value::Array(a)) => Ok(a.as_slice()),
            Some(_) => err(format!("/{key}"), "must be an array"),
        }
    };
    // ---- buffers
    let buffers = arr("buffers")?;
    let mut buf_len: Vec<usize> = Vec::new();
    for (i, b) in buffers.iter().enumerate() {
        let p = format!("/buffers/{i}");
        let l = req_u64(b, "byteLength", &p)? as usize;
        if l < 1 {
            return err(format!("{p}/byteLength"), "must be at least 1");
        }
        if i == 0 {
            if b.get("uri").is_some() {
                return err(format!("{p}/uri"), "the GLB-stored buffer must not have a uri");
            }
            let Some(bin) = bin else {
                return err(p, "buffer 0 needs a BIN chunk");
            };
            if bin.len() < l || bin.len() - l > 3 {
                return err(
                    format!("{p}/byteLength"),
                    format!(
                        "{l} does not match the BIN chunk of {} bytes (padding of at most 3 allowed)",
                        bin.len()
                    ),
                );
            }
        } else if b.get("uri").is_some() {
            return err(format!("{p}/uri"), "external buffers are not supported by this checker");
        }
        buf_len.push(l);
    }
    let bin = bin.unwrap_or(&[]);
    // ---- bufferViews
    let mut views: Vec<View> = Vec::new();
    for (i, v) in arr("bufferViews")?.iter().enumerate() {
        let p = format!("/bufferViews/{i}");
        let b = req_u64(v, "buffer", &p)? as usize;
        let Some(&bl) = buf_len.get(b) else {
            return err(format!("{p}/buffer"), format!("buffer {b} does not exist"));
        };
        if b != 0 {
            return err(format!("{p}/buffer"), "only the GLB-stored buffer 0 can be read");
        }
        let offset = opt_u64(v, "byteOffset", &p)?.unwrap_or(0) as usize;
        let len = req_u64(v, "byteLength", &p)? as usize;
        if len < 1 {
            return err(format!("{p}/byteLength"), "must be at least 1");
        }
        if exceeds(offset.checked_add(len), bl) {
            return err(
                p,
                format!("byteOffset {offset} + byteLength {len} exceeds buffer {b} of {bl} bytes"),
            );
        }
        let stride = opt_u64(v, "byteStride", &p)?.map(|s| s as usize);
        if let Some(s) = stride {
            if !(4..=252).contains(&s) || s % 4 != 0 {
                return err(
                    format!("{p}/byteStride"),
                    format!("{s} must be a multiple of 4 in 4..=252"),
                );
            }
        }
        let target = opt_u64(v, "target", &p)?;
        if let Some(t) = target {
            if t != 34962 && t != 34963 {
                return err(format!("{p}/target"), format!("{t} is neither 34962 nor 34963"));
            }
        }
        views.push(View {
            offset,
            len,
            stride,
            target,
        });
    }
    // ---- accessors
    let mut accs: Vec<Acc> = Vec::new();
    for (i, a) in arr("accessors")?.iter().enumerate() {
        let p = format!("/accessors/{i}");
        if a.get("sparse").is_some() {
            return err(
                format!("{p}/sparse"),
                "sparse accessors are not supported by this checker",
            );
        }
        let Some(view) = opt_u64(a, "bufferView", &p)? else {
            return err(
                format!("{p}/bufferView"),
                "accessors without a bufferView are not supported by this checker",
            );
        };
        let view = view as usize;
        let Some(v) = views.get(view) else {
            return err(format!("{p}/bufferView"), format!("bufferView {view} does not exist"));
        };
        let ctype = req_u64(a, "componentType", &p)?;
        let Some(cs) = comp_size(ctype) else {
            return err(
                format!("{p}/componentType"),
                format!("{ctype} is not a valid component type"),
            );
        };
        let ty = a.get("type").and_then(Value::as_str).unwrap_or("");
        let Some(comps) = n_comp(ty) else {
            return err(
                format!("{p}/type"),
                format!("'{ty}' is not SCALAR, VEC2, VEC3 or VEC4 (matrices are not used)"),
            );
        };
        let count = req_u64(a, "count", &p)? as usize;
        if count < 1 {
            return err(format!("{p}/count"), "must be at least 1");
        }
        let normalized = match a.get("normalized") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return err(format!("{p}/normalized"), "must be a boolean"),
        };
        if normalized && (ctype == 5126 || ctype == 5125) {
            return err(
                format!("{p}/normalized"),
                "only 8 and 16 bit integer accessors can be normalized",
            );
        }
        let offset = opt_u64(a, "byteOffset", &p)?.unwrap_or(0) as usize;
        if offset % cs != 0 {
            return err(
                format!("{p}/byteOffset"),
                format!("{offset} is not a multiple of the component size {cs}"),
            );
        }
        if (v.offset.saturating_add(offset)) % cs != 0 {
            return err(
                format!("{p}/byteOffset"),
                format!(
                    "bufferView byteOffset + accessor byteOffset = {} is not a multiple of {cs}",
                    v.offset + offset
                ),
            );
        }
        let elem = comps * cs;
        let stride = v.stride.unwrap_or(elem);
        if stride < elem {
            return err(
                format!("/bufferViews/{view}/byteStride"),
                format!("{stride} is smaller than the element size {elem} of {p}"),
            );
        }
        let extent = (count - 1)
            .checked_mul(stride)
            .and_then(|x| x.checked_add(elem))
            .and_then(|x| x.checked_add(offset));
        if exceeds(extent, v.len) {
            return err(
                p,
                format!(
                    "{count} elements at stride {stride} from offset {offset} exceed bufferView {view} of {} bytes",
                    v.len
                ),
            );
        }
        accs.push(Acc {
            view,
            offset,
            ctype,
            comps,
            count,
            normalized,
            stride,
        });
    }
    // ---- G3/G4 min, max and finite data
    for (i, a) in arr("accessors")?.iter().enumerate() {
        let p = format!("/accessors/{i}");
        let acc = &accs[i];
        let is_float = acc.ctype == 5126;
        if is_float {
            for e in 0..acc.count {
                for k in 0..acc.comps {
                    match read_comp(bin, &views, acc, e, k) {
                        Some(x) if x.is_finite() => {}
                        Some(_) => return err(p, format!("non-finite value at element {e} component {k}")),
                        None => return err(p, "data read outside the buffer"),
                    }
                }
            }
        }
        let (mn, mx) = (a.get("min"), a.get("max"));
        if mn.is_some() != mx.is_some() {
            return err(format!("{p}/min"), "min and max must be given together");
        }
        if let (Some(mn), Some(mx)) = (mn, mx) {
            let (Some(mn), Some(mx)) = (mn.as_array(), mx.as_array()) else {
                return err(format!("{p}/min"), "min and max must be arrays");
            };
            if mn.len() != acc.comps || mx.len() != acc.comps {
                return err(format!("{p}/min"), format!("min/max need {} entries", acc.comps));
            }
            for k in 0..acc.comps {
                let (Some(lo_j), Some(hi_j)) = (mn[k].as_f64(), mx[k].as_f64()) else {
                    return err(format!("{p}/min/{k}"), "min and max entries must be numbers");
                };
                if lo_j > hi_j {
                    return err(format!("{p}/min/{k}"), format!("min {lo_j} exceeds max {hi_j}"));
                }
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for e in 0..acc.count {
                    let Some(x) = read_comp(bin, &views, acc, e, k) else {
                        return err(p, "data read outside the buffer");
                    };
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
                if lo != lo_j {
                    return err(
                        format!("{p}/min/{k}"),
                        format!("{lo_j} differs from the data minimum {lo}"),
                    );
                }
                if hi != hi_j {
                    return err(
                        format!("{p}/max/{k}"),
                        format!("{hi_j} differs from the data maximum {hi}"),
                    );
                }
            }
        }
    }
    // ---- meshes and primitives
    let materials = arr("materials")?.len();
    let meshes = arr("meshes")?;
    for (mi, m) in meshes.iter().enumerate() {
        let prims = match m.get("primitives") {
            Some(Value::Array(p)) if !p.is_empty() => p,
            _ => return err(format!("/meshes/{mi}/primitives"), "needs a non-empty array"),
        };
        for (pi, pr) in prims.iter().enumerate() {
            let pp = format!("/meshes/{mi}/primitives/{pi}");
            let Some(attrs) = pr.get("attributes").and_then(Value::as_object) else {
                return err(format!("{pp}/attributes"), "required object is missing");
            };
            let mode = opt_u64(pr, "mode", &pp)?.unwrap_or(4);
            if mode > 6 {
                return err(format!("{pp}/mode"), format!("{mode} is not in 0..=6"));
            }
            let Some(pos_i) = attrs.get("POSITION") else {
                return err(format!("{pp}/attributes/POSITION"), "required attribute is missing");
            };
            let acc_of = |v: &Value, ptr: String| -> R<&Acc> {
                let i = v.as_u64().ok_or_else(|| GlbError {
                    pointer: ptr.clone(),
                    message: "must be an accessor index".into(),
                })? as usize;
                accs.get(i).ok_or_else(|| GlbError {
                    pointer: ptr,
                    message: format!("accessor {i} does not exist"),
                })
            };
            let pos = acc_of(pos_i, format!("{pp}/attributes/POSITION"))?;
            let pos_idx = pos_i.as_u64().unwrap_or(0) as usize;
            if pos.ctype != 5126 || pos.comps != 3 {
                return err(format!("{pp}/attributes/POSITION"), "POSITION must be float VEC3");
            }
            if arr("accessors")?[pos_idx].get("min").is_none() {
                return err(
                    format!("/accessors/{pos_idx}/min"),
                    "POSITION accessors need min and max",
                );
            }
            for (name, ai) in attrs {
                let ptr = format!("{pp}/attributes/{name}");
                let a = acc_of(ai, ptr.clone())?;
                if a.count != pos.count {
                    return err(
                        ptr,
                        format!("count {} differs from POSITION count {}", a.count, pos.count),
                    );
                }
                if views[a.view].target == Some(34963) {
                    return err(ptr, "a vertex attribute must not use an ELEMENT_ARRAY_BUFFER view");
                }
                match name.as_str() {
                    "NORMAL" => {
                        if a.ctype != 5126 || a.comps != 3 {
                            return err(ptr, "NORMAL must be float VEC3");
                        }
                        for e in 0..a.count {
                            let (x, y, z) = (
                                read_comp(bin, &views, a, e, 0),
                                read_comp(bin, &views, a, e, 1),
                                read_comp(bin, &views, a, e, 2),
                            );
                            let (Some(x), Some(y), Some(z)) = (x, y, z) else {
                                return err(ptr, "data read outside the buffer");
                            };
                            let l = (x * x + y * y + z * z).sqrt();
                            if (l - 1.0).abs() > 1e-3 {
                                return err(ptr, format!("normal {e} has length {l}, expected 1 within 1e-3"));
                            }
                        }
                    }
                    "COLOR_0" => {
                        let ok_type = a.comps == 3 || a.comps == 4;
                        let ok_comp = a.ctype == 5126 || ((a.ctype == 5121 || a.ctype == 5123) && a.normalized);
                        if !ok_type || !ok_comp {
                            return err(
                                ptr,
                                "COLOR_0 must be VEC3/VEC4 of float or normalized unsigned byte/short",
                            );
                        }
                    }
                    _ => {}
                }
            }
            // indices
            let n_ind = match pr.get("indices") {
                Some(ii) => {
                    let ptr = format!("{pp}/indices");
                    let ia = acc_of(ii, ptr.clone())?;
                    if ia.comps != 1 || !matches!(ia.ctype, 5121 | 5123 | 5125) {
                        return err(ptr, "indices must be SCALAR of unsigned byte, short or int");
                    }
                    if views[ia.view].target == Some(34962) {
                        return err(ptr, "indices must not use an ARRAY_BUFFER view");
                    }
                    let restart = match ia.ctype {
                        5121 => 255.0,
                        5123 => 65535.0,
                        _ => 4294967295.0,
                    };
                    for e in 0..ia.count {
                        let Some(v) = read_comp(bin, &views, ia, e, 0) else {
                            return err(ptr, "data read outside the buffer");
                        };
                        if v >= pos.count as f64 {
                            return err(
                                ptr,
                                format!("index {e} = {v} is not below the vertex count {}", pos.count),
                            );
                        }
                        if v == restart {
                            return err(ptr, format!("index {e} equals the primitive restart value {restart}"));
                        }
                    }
                    ia.count
                }
                None => pos.count,
            };
            let ok = match mode {
                0 => n_ind >= 1,
                1 => n_ind % 2 == 0,
                2 | 3 => n_ind >= 2,
                4 => n_ind % 3 == 0,
                _ => n_ind >= 3,
            };
            if !ok {
                return err(
                    format!("{pp}/indices"),
                    format!("{n_ind} indices do not suit primitive mode {mode}"),
                );
            }
            if let Some(mat) = opt_u64(pr, "material", &pp)? {
                if mat as usize >= materials {
                    return err(format!("{pp}/material"), format!("material {mat} does not exist"));
                }
            }
        }
    }
    // ---- G7 nodes and scenes
    let nodes = arr("nodes")?;
    let mut parent: Vec<Option<usize>> = vec![None; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        let p = format!("/nodes/{i}");
        if let Some(m) = opt_u64(n, "mesh", &p)? {
            if m as usize >= meshes.len() {
                return err(format!("{p}/mesh"), format!("mesh {m} does not exist"));
            }
        }
        if let Some(ch) = n.get("children") {
            let Some(ch) = ch.as_array() else {
                return err(format!("{p}/children"), "must be an array");
            };
            for (k, c) in ch.iter().enumerate() {
                let cp = format!("{p}/children/{k}");
                let Some(c) = c.as_u64().map(|c| c as usize).filter(|&c| c < nodes.len()) else {
                    return err(cp, "must be an existing node index");
                };
                if c == i {
                    return err(cp, "a node cannot be its own child");
                }
                if parent[c].is_some() {
                    return err(cp, format!("node {c} has more than one parent"));
                }
                parent[c] = Some(i);
            }
        }
    }
    for start in 0..nodes.len() {
        let (mut cur, mut steps) = (start, 0);
        while let Some(p) = parent[cur] {
            cur = p;
            steps += 1;
            if steps > nodes.len() {
                return err(format!("/nodes/{start}"), "node hierarchy contains a cycle");
            }
        }
    }
    let scenes = arr("scenes")?;
    for (i, s) in scenes.iter().enumerate() {
        if let Some(list) = s.get("nodes") {
            let Some(list) = list.as_array() else {
                return err(format!("/scenes/{i}/nodes"), "must be an array");
            };
            for (k, n) in list.iter().enumerate() {
                match n.as_u64() {
                    Some(n) if (n as usize) < nodes.len() => {
                        if parent[n as usize].is_some() {
                            return err(format!("/scenes/{i}/nodes/{k}"), "scene roots must not have a parent");
                        }
                    }
                    _ => return err(format!("/scenes/{i}/nodes/{k}"), "must be an existing node index"),
                }
            }
        }
    }
    if let Some(s) = opt_u64(&doc, "scene", "")? {
        if s as usize >= scenes.len() {
            return err("/scene", format!("scene {s} does not exist"));
        }
    }
    Ok(doc)
}
