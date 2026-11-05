//! glTF 2.0 surface reader, `.gltf` and `.glb` (F-280, ADR 0184 D6).
//!
//! Written from the public Khronos glTF 2.0 specification. Only what describes the surface is read:
//!
//! * buffers from base64 data URIs, from sidecar files (through a resolver, see [`read_file`]) or, in a `.glb`, from the
//!   `BIN` chunk (buffer 0 with no `uri`);
//! * buffer views and accessors with `byteOffset` and `byteStride`, every range checked before it is read; an accessor
//!   with no buffer view reads as zeros; sparse accessors are refused;
//! * mesh primitives of mode 4 (triangles), 5 (strip, `p_i = {v_i, v_{i+(1+i%2)}, v_{i+(2-i%2)}}`) and 6 (fan,
//!   `p_i = {v_{i+1}, v_{i+2}, v_0}`), indexed by unsigned 8, 16 or 32 bit integers or not indexed; modes 0 to 3 (points
//!   and lines) are skipped with a note; `POSITION` must be `VEC3` of `FLOAT`;
//! * the node hierarchy of the scene (`scene`, else scene 0, else every root node), with `matrix` (column-major) or
//!   `T * R * S`; when the determinant of a node's world transform is negative the triangle order is reversed, so the
//!   winding stays counter-clockwise seen from outside.
//!
//! Each node with a mesh is one patch, named by the node's `name`, else the mesh's `name`, else `mesh<i>`; equal names
//! merge. Materials, textures, images, cameras, animations, skins and morph targets are ignored with a note. Every entry
//! of `extensionsRequired` is refused. The length unit is required (ADR 0184 D3; the specification's metre is not
//! assumed), and the result is welded as described in [`crate::common::finish_import`]. Positions are `f32` in the file.

use crate::common::{finish_import, length_factor, SurfaceImport};
use crate::guard::{check_file_size, MAX_ITEMS};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const MAX_DEPTH: usize = 1024;

fn e(msg: String) -> Error {
    Error::Io(format!("glTF: {msg}"))
}

/// A resolver that refuses every external file (for documents whose buffers are all embedded).
pub fn no_external(uri: &str) -> Result<Vec<u8>> {
    Err(e(format!(
        "external file '{uri}' requested, but no resolver is available"
    )))
}

/// Read a glTF document from memory: a `.glb` container (detected by its magic) or `.gltf` JSON. `resolve` loads a
/// sidecar file given its relative path (already percent-decoded and checked to stay inside the document's folder).
pub fn read_bytes(bytes: &[u8], unit: &str, resolve: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<SurfaceImport> {
    if bytes.starts_with(GLB_MAGIC) {
        read_glb(bytes, unit, resolve)
    } else {
        read_gltf(bytes, unit, resolve)
    }
}

/// Read `.gltf` JSON.
pub fn read_gltf(json: &[u8], unit: &str, resolve: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<SurfaceImport> {
    check_file_size(json.len(), "glTF")?;
    let factor = length_factor(unit)?;
    let root: Value = serde_json::from_slice(json).map_err(|x| e(format!("JSON: {x}")))?;
    read_doc(&root, None, factor, resolve)
}

/// Read a `.glb` container.
pub fn read_glb(bytes: &[u8], unit: &str, resolve: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<SurfaceImport> {
    check_file_size(bytes.len(), "GLB")?;
    let factor = length_factor(unit)?;
    let ge = |off: usize, msg: String| Error::Io(format!("GLB: byte offset {off}: {msg}"));
    let u32_at = |off: usize| -> Option<u32> {
        bytes
            .get(off..off.checked_add(4)?)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if bytes.len() < 12 || &bytes[..4] != GLB_MAGIC {
        return Err(ge(
            0,
            "not a GLB file (magic 'glTF' and a 12-byte header expected)".into(),
        ));
    }
    let version = u32_at(4).unwrap_or(0);
    if version != 2 {
        return Err(ge(4, format!("GLB version {version}, only version 2 is read")));
    }
    let length = u32_at(8).unwrap_or(0) as usize;
    if length != bytes.len() {
        return Err(ge(
            8,
            format!("header declares {length} bytes, the file has {}", bytes.len()),
        ));
    }
    let mut off = 12;
    let mut json: Option<&[u8]> = None;
    let mut bin: Option<&[u8]> = None;
    while off < bytes.len() {
        let (Some(len), Some(kind)) = (u32_at(off), u32_at(off + 4)) else {
            return Err(ge(off, "truncated chunk header".into()));
        };
        let start = off + 8;
        let end = start
            .checked_add(len as usize)
            .filter(|&x| x <= bytes.len())
            .ok_or_else(|| ge(off, format!("chunk of {len} bytes runs past the end of the file")))?;
        let data = &bytes[start..end];
        if json.is_none() {
            if kind != CHUNK_JSON {
                return Err(ge(off + 4, format!("first chunk has type 0x{kind:08X}, JSON expected")));
            }
            json = Some(data);
        } else if kind == CHUNK_BIN && bin.is_none() {
            bin = Some(data);
        }
        off = end;
    }
    let json = json.ok_or_else(|| ge(12, "no JSON chunk".into()))?;
    let trimmed_len = json.iter().rposition(|&b| b != 0 && b != b' ').map_or(0, |p| p + 1);
    let root: Value = serde_json::from_slice(&json[..trimmed_len]).map_err(|x| ge(20, format!("JSON chunk: {x}")))?;
    read_doc(&root, bin, factor, resolve)
}

/// Read a `.gltf` or `.glb` file from disk. Sidecar files are read from the file's folder; absolute paths, `..`
/// segments and URI schemes other than `data:` are refused.
pub fn read_file(path: impl AsRef<std::path::Path>, unit: &str) -> Result<SurfaceImport> {
    let path = path.as_ref();
    let meta = std::fs::metadata(path)?;
    check_file_size(meta.len() as usize, "glTF")?;
    let bytes = std::fs::read(path)?;
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let resolve = |rel: &str| -> Result<Vec<u8>> {
        let p = dir.join(rel);
        let m = std::fs::metadata(&p).map_err(|x| e(format!("sidecar '{rel}': {x}")))?;
        check_file_size(m.len() as usize, "glTF sidecar")?;
        std::fs::read(&p).map_err(|x| e(format!("sidecar '{rel}': {x}")))
    };
    read_bytes(&bytes, unit, resolve).map_err(|x| Error::Io(format!("{}: {x}", path.display())))
}

// ---- JSON helpers ----------------------------------------------------------------------------------------------------

fn at(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn list<'a>(v: &'a Value, key: &str, path: &str) -> Result<&'a [Value]> {
    match v.get(key) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(e(format!("{}: not an array", at(path, key)))),
    }
}

fn opt_index(v: &Value, key: &str, path: &str) -> Result<Option<usize>> {
    match v.get(key) {
        None => Ok(None),
        Some(x) => x
            .as_u64()
            .filter(|&n| n <= usize::MAX as u64)
            .map(|n| Some(n as usize))
            .ok_or_else(|| e(format!("{}: not a non-negative integer", at(path, key)))),
    }
}

fn req_index(v: &Value, key: &str, path: &str) -> Result<usize> {
    opt_index(v, key, path)?.ok_or_else(|| e(format!("{}: missing", at(path, key))))
}

fn numbers<const N: usize>(v: &Value, key: &str, path: &str) -> Result<Option<[f64; N]>> {
    let Some(x) = v.get(key) else { return Ok(None) };
    let a = x
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| e(format!("{}: expected {N} numbers", at(path, key))))?;
    let mut out = [0.0; N];
    for (o, item) in out.iter_mut().zip(a) {
        *o = item
            .as_f64()
            .filter(|f| f.is_finite())
            .ok_or_else(|| e(format!("{}: expected {N} finite numbers", at(path, key))))?;
    }
    Ok(Some(out))
}

fn name_of(v: &Value) -> Option<String> {
    v.get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ---- buffers ---------------------------------------------------------------------------------------------------------

fn base64_decode(s: &str, path: &str) -> Result<Vec<u8>> {
    let body = s.trim_end_matches('=');
    if s.len() - body.len() > 2 {
        return Err(e(format!("{path}: more than two '=' padding characters")));
    }
    let mut out = Vec::with_capacity(body.len() / 4 * 3 + 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for (i, c) in body.bytes().enumerate() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => {
                return Err(e(format!(
                    "{path}: character {} of the base64 data is not in the base64 alphabet",
                    i + 1
                )))
            }
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if body.len() % 4 == 1 {
        return Err(e(format!("{path}: base64 data has an impossible length")));
    }
    Ok(out)
}

fn percent_decode(s: &str, path: &str) -> Result<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| e(format!("{path}: bad percent escape in the URI")))?;
            out.push(hex);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| e(format!("{path}: the URI is not UTF-8 after percent decoding")))
}

fn load_uri(uri: &str, path: &str, resolve: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Result<Vec<u8>> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (head, data) = rest
            .split_once(',')
            .ok_or_else(|| e(format!("{path}.uri: data URI without ','")))?;
        if !head.ends_with(";base64") {
            return Err(e(format!("{path}.uri: only base64 data URIs are read")));
        }
        return base64_decode(data, &format!("{path}.uri"));
    }
    if let Some(c) = uri.find(':') {
        if !uri[..c].contains('/') {
            return Err(e(format!(
                "{path}.uri: URI scheme '{}:' is not read (only data: URIs and relative paths)",
                &uri[..c]
            )));
        }
    }
    let rel = percent_decode(uri, path)?;
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.starts_with('\\')
        || rel.contains(':')
        || rel.split(['/', '\\']).any(|seg| seg == "..")
    {
        return Err(e(format!(
            "{path}.uri: '{uri}' must be a relative path inside the document's folder"
        )));
    }
    resolve(&rel).map_err(|x| e(format!("{path}.uri '{uri}': {x}")))
}

// ---- accessors -------------------------------------------------------------------------------------------------------

struct Doc<'a> {
    root: &'a Value,
    buffers: Vec<Option<Vec<u8>>>,
}

fn comp_size(ct: u64) -> Option<usize> {
    match ct {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

fn type_width(t: &str) -> Option<usize> {
    match t {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" => Some(4),
        "MAT2" => Some(4),
        "MAT3" => Some(9),
        "MAT4" => Some(16),
        _ => None,
    }
}

/// An accessor resolved to bytes: element `i` starts at `data[offset + i * stride]`, or is all zeros without data.
struct Elements<'b> {
    data: Option<&'b [u8]>,
    offset: usize,
    stride: usize,
    count: usize,
    comp: u64,
    ty: String,
}

impl<'a> Doc<'a> {
    fn accessor(&self, i: usize) -> Result<&'a Value> {
        list(self.root, "accessors", "")?
            .get(i)
            .ok_or_else(|| e(format!("accessors[{i}] does not exist")))
    }

    /// Buffers that the given accessors read, by index.
    fn buffers_of(&self, accessors: &BTreeSet<usize>) -> Result<BTreeSet<usize>> {
        let views = list(self.root, "bufferViews", "")?;
        let mut out = BTreeSet::new();
        for &a in accessors {
            let acc = self.accessor(a)?;
            if let Some(v) = opt_index(acc, "bufferView", &format!("accessors[{a}]"))? {
                let view = views
                    .get(v)
                    .ok_or_else(|| e(format!("accessors[{a}].bufferView: bufferViews[{v}] does not exist")))?;
                out.insert(req_index(view, "buffer", &format!("bufferViews[{v}]"))?);
            }
        }
        Ok(out)
    }

    fn load_buffers(
        &mut self,
        which: &BTreeSet<usize>,
        bin: Option<&[u8]>,
        resolve: &mut dyn FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<()> {
        let bufs = list(self.root, "buffers", "")?;
        self.buffers = vec![None; bufs.len()];
        for &b in which {
            let path = format!("buffers[{b}]");
            let buf = bufs.get(b).ok_or_else(|| e(format!("{path} does not exist")))?;
            let len = req_index(buf, "byteLength", &path)?;
            let data = match buf.get("uri") {
                Some(Value::String(uri)) => load_uri(uri, &path, resolve)?,
                Some(_) => return Err(e(format!("{path}.uri: not a string"))),
                None => match (b, bin) {
                    (0, Some(bin)) => bin.to_vec(),
                    _ => {
                        return Err(e(format!(
                            "{path}: no uri (only buffer 0 of a .glb with a BIN chunk may omit it)"
                        )))
                    }
                },
            };
            if data.len() < len {
                return Err(e(format!(
                    "{path}: byteLength is {len} but the data has {} bytes",
                    data.len()
                )));
            }
            let mut data = data;
            data.truncate(len);
            self.buffers[b] = Some(data);
        }
        Ok(())
    }

    fn elements(&self, a: usize) -> Result<Elements<'_>> {
        let path = format!("accessors[{a}]");
        let acc = self.accessor(a)?;
        if acc.get("sparse").is_some() {
            return Err(e(format!("{path}: sparse accessors are not supported")));
        }
        let comp = acc
            .get("componentType")
            .and_then(|c| c.as_u64())
            .ok_or_else(|| e(format!("{path}.componentType: missing or not an integer")))?;
        let cs = comp_size(comp).ok_or_else(|| e(format!("{path}.componentType: unknown value {comp}")))?;
        let ty = acc
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| e(format!("{path}.type: missing or not a string")))?
            .to_string();
        let width = type_width(&ty).ok_or_else(|| e(format!("{path}.type: unknown value '{ty}'")))?;
        let count = req_index(acc, "count", &path)?;
        if count == 0 || count > MAX_ITEMS {
            return Err(e(format!("{path}.count: {count} is outside 1..={MAX_ITEMS}")));
        }
        let elem = cs * width;
        let acc_off = opt_index(acc, "byteOffset", &path)?.unwrap_or(0);
        let Some(v) = opt_index(acc, "bufferView", &path)? else {
            return Ok(Elements {
                data: None,
                offset: 0,
                stride: elem,
                count,
                comp,
                ty,
            });
        };
        let vpath = format!("bufferViews[{v}]");
        let view = list(self.root, "bufferViews", "")?
            .get(v)
            .ok_or_else(|| e(format!("{path}.bufferView: {vpath} does not exist")))?;
        let b = req_index(view, "buffer", &vpath)?;
        let buf = self
            .buffers
            .get(b)
            .and_then(|x| x.as_deref())
            .ok_or_else(|| e(format!("{vpath}.buffer: buffers[{b}] is not loaded")))?;
        let v_off = opt_index(view, "byteOffset", &vpath)?.unwrap_or(0);
        let v_len = req_index(view, "byteLength", &vpath)?;
        let v_end = v_off.checked_add(v_len).filter(|&x| x <= buf.len()).ok_or_else(|| {
            e(format!(
                "{vpath}: bytes {v_off}..{v_off}+{v_len} lie outside buffers[{b}] ({} bytes)",
                buf.len()
            ))
        })?;
        let stride = match opt_index(view, "byteStride", &vpath)? {
            None => elem,
            Some(s) => {
                if s < elem || s % cs != 0 || s > 252 {
                    return Err(e(format!(
                        "{vpath}.byteStride: {s} does not fit {path} (element of {elem} bytes; a stride is a multiple of the component size, at most 252)"
                    )));
                }
                s
            }
        };
        let need = stride
            .checked_mul(count - 1)
            .and_then(|x| x.checked_add(elem))
            .and_then(|x| x.checked_add(acc_off))
            .ok_or_else(|| e(format!("{path}: size overflows")))?;
        if need > v_len {
            return Err(e(format!(
                "{path}: {count} elements from byte {acc_off} with stride {stride} need {need} bytes, {vpath} has {v_len}"
            )));
        }
        Ok(Elements {
            data: Some(&buf[v_off..v_end]),
            offset: acc_off,
            stride,
            count,
            comp,
            ty,
        })
    }

    fn positions(&self, a: usize) -> Result<Vec<Vec3>> {
        let el = self.elements(a)?;
        if el.ty != "VEC3" || el.comp != 5126 {
            return Err(e(format!(
                "accessors[{a}]: POSITION must be VEC3 of FLOAT (5126), found {} of {}",
                el.ty, el.comp
            )));
        }
        let mut out = Vec::with_capacity(el.count);
        for i in 0..el.count {
            let mut p = [0.0f64; 3];
            if let Some(d) = el.data {
                let o = el.offset + i * el.stride;
                for (k, c) in p.iter_mut().enumerate() {
                    let q = o + 4 * k;
                    let f = f32::from_le_bytes([d[q], d[q + 1], d[q + 2], d[q + 3]]);
                    if !f.is_finite() {
                        return Err(e(format!("accessors[{a}]: element {i} is not finite")));
                    }
                    *c = f as f64;
                }
            }
            out.push(p);
        }
        Ok(out)
    }

    fn indices(&self, a: usize, n_vertices: usize) -> Result<Vec<usize>> {
        let el = self.elements(a)?;
        let (cs, max) = match el.comp {
            5121 => (1, u8::MAX as u64),
            5123 => (2, u16::MAX as u64),
            5125 => (4, u32::MAX as u64),
            c => {
                return Err(e(format!(
                    "accessors[{a}]: indices must be unsigned byte, short or int (5121, 5123, 5125), found {c}"
                )))
            }
        };
        if el.ty != "SCALAR" {
            return Err(e(format!("accessors[{a}]: indices must be SCALAR, found {}", el.ty)));
        }
        let mut out = Vec::with_capacity(el.count);
        for i in 0..el.count {
            let v = match el.data {
                None => 0,
                Some(d) => {
                    let o = el.offset + i * el.stride;
                    match cs {
                        1 => d[o] as u64,
                        2 => u16::from_le_bytes([d[o], d[o + 1]]) as u64,
                        _ => u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as u64,
                    }
                }
            };
            if v == max {
                return Err(e(format!(
                    "accessors[{a}]: element {i} is {v}, the primitive-restart value, which glTF does not allow"
                )));
            }
            if v >= n_vertices as u64 {
                return Err(e(format!(
                    "accessors[{a}]: element {i} is {v}, the primitive has {n_vertices} vertices"
                )));
            }
            out.push(v as usize);
        }
        Ok(out)
    }
}

// ---- transforms ------------------------------------------------------------------------------------------------------

type Mat = [[f64; 4]; 4];

const IDENTITY: Mat = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let mut c = [[0.0; 4]; 4];
    for (r, row) in c.iter_mut().enumerate() {
        for (k, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|j| a[r][j] * b[j][k]).sum();
        }
    }
    c
}

fn det3(m: &Mat) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Local transform of a node, or `None` when it has none (identity).
fn local_matrix(node: &Value, path: &str) -> Result<Option<Mat>> {
    if let Some(a) = numbers::<16>(node, "matrix", path)? {
        let mut m = [[0.0; 4]; 4];
        for (c, col) in a.chunks(4).enumerate() {
            for (r, v) in col.iter().enumerate() {
                m[r][c] = *v;
            }
        }
        return Ok(if m == IDENTITY { None } else { Some(m) });
    }
    let t = numbers::<3>(node, "translation", path)?;
    let r = numbers::<4>(node, "rotation", path)?;
    let s = numbers::<3>(node, "scale", path)?;
    if t.is_none() && r.is_none() && s.is_none() {
        return Ok(None);
    }
    let [tx, ty, tz] = t.unwrap_or([0.0; 3]);
    let [qx, qy, qz, qw] = r.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let [sx, sy, sz] = s.unwrap_or([1.0; 3]);
    let qn = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
    if qn == 0.0 || !qn.is_finite() {
        return Err(e(format!("{path}.rotation: zero quaternion")));
    }
    let (x, y, z, w) = (qx / qn, qy / qn, qz / qn, qw / qn);
    let rot = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    let sc = [sx, sy, sz];
    let mut m = IDENTITY;
    for r in 0..3 {
        for c in 0..3 {
            m[r][c] = rot[r][c] * sc[c];
        }
    }
    m[0][3] = tx;
    m[1][3] = ty;
    m[2][3] = tz;
    Ok(Some(m))
}

fn apply(m: &Mat, p: Vec3) -> Vec3 {
    let mut q = [0.0; 3];
    for (r, out) in q.iter_mut().enumerate() {
        *out = m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3];
    }
    q
}

// ---- document --------------------------------------------------------------------------------------------------------

fn read_doc(
    root: &Value,
    bin: Option<&[u8]>,
    factor: f64,
    mut resolve: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<SurfaceImport> {
    if !root.is_object() {
        return Err(e("the JSON root is not an object".into()));
    }
    let version = root
        .get("asset")
        .and_then(|a| a.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| e("asset.version: missing".into()))?;
    if !version.starts_with("2.") {
        return Err(e(format!("asset.version: '{version}', only glTF 2.x is read")));
    }
    if let Some(min) = root.get("asset").and_then(|a| a.get("minVersion")) {
        if min.as_str() != Some("2.0") {
            return Err(e(format!("asset.minVersion: {min}, only 2.0 is read")));
        }
    }
    let required: Vec<String> = list(root, "extensionsRequired", "")?
        .iter()
        .map(|x| x.as_str().unwrap_or("?").to_string())
        .collect();
    if !required.is_empty() {
        return Err(e(format!(
            "extensionsRequired: {} not supported (export without compression or quantization)",
            required.join(", ")
        )));
    }
    let nodes = list(root, "nodes", "")?;
    let meshes = list(root, "meshes", "")?;
    let scenes = list(root, "scenes", "")?;

    // roots of the scene
    let roots: Vec<usize> = match (opt_index(root, "scene", "")?, scenes.is_empty()) {
        (Some(s), _) => {
            let sc = scenes
                .get(s)
                .ok_or_else(|| e(format!("scene: scenes[{s}] does not exist")))?;
            index_list(sc, "nodes", &format!("scenes[{s}]"))?
        }
        (None, false) => index_list(&scenes[0], "nodes", "scenes[0]")?,
        (None, true) => {
            let mut child = vec![false; nodes.len()];
            for (i, n) in nodes.iter().enumerate() {
                for c in index_list(n, "children", &format!("nodes[{i}]"))? {
                    if let Some(f) = child.get_mut(c) {
                        *f = true;
                    }
                }
            }
            (0..nodes.len()).filter(|&i| !child[i]).collect()
        }
    };

    // walk: (node, world transform) for every node with a mesh, in depth-first order
    let mut visited = vec![false; nodes.len()];
    let mut instances: Vec<(usize, usize, Option<Mat>)> = Vec::new();
    let mut stack: Vec<(usize, Option<Mat>, usize)> = roots.iter().rev().map(|&r| (r, None, 0)).collect();
    let (mut skins, mut cameras) = (0usize, 0usize);
    while let Some((n, parent, depth)) = stack.pop() {
        let path = format!("nodes[{n}]");
        let node = nodes.get(n).ok_or_else(|| e(format!("{path} does not exist")))?;
        if std::mem::replace(&mut visited[n], true) {
            return Err(e(format!(
                "{path} is reached twice (a cycle, or a node with two parents)"
            )));
        }
        if depth > MAX_DEPTH {
            return Err(e(format!("{path}: node hierarchy deeper than {MAX_DEPTH}")));
        }
        let world = match (parent, local_matrix(node, &path)?) {
            (None, l) => l,
            (Some(p), None) => Some(p),
            (Some(p), Some(l)) => Some(mat_mul(&p, &l)),
        };
        if node.get("skin").is_some() {
            skins += 1;
        }
        if node.get("camera").is_some() {
            cameras += 1;
        }
        if let Some(m) = opt_index(node, "mesh", &path)? {
            if m >= meshes.len() {
                return Err(e(format!("{path}.mesh: meshes[{m}] does not exist")));
            }
            instances.push((n, m, world));
        }
        for c in index_list(node, "children", &path)?.into_iter().rev() {
            stack.push((c, world, depth + 1));
        }
    }

    // accessors that will be read, then the buffers they need
    let mut needed = BTreeSet::new();
    for &(_, m, _) in &instances {
        for (p, prim) in list(&meshes[m], "primitives", &format!("meshes[{m}]"))?
            .iter()
            .enumerate()
        {
            let path = format!("meshes[{m}].primitives[{p}]");
            let mode = opt_index(prim, "mode", &path)?.unwrap_or(4);
            if mode < 4 {
                continue;
            }
            let attrs = prim
                .get("attributes")
                .ok_or_else(|| e(format!("{path}.attributes: missing")))?;
            needed.insert(req_index(attrs, "POSITION", &format!("{path}.attributes"))?);
            if let Some(i) = opt_index(prim, "indices", &path)? {
                needed.insert(i);
            }
        }
    }
    let mut doc = Doc {
        root,
        buffers: Vec::new(),
    };
    let which = doc.buffers_of(&needed)?;
    doc.load_buffers(&which, bin, &mut resolve)?;

    let mut points: Vec<Vec3> = Vec::new();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut tags: Vec<u32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut name_index: HashMap<String, u32> = HashMap::new();
    let mut skipped: BTreeMap<u64, usize> = BTreeMap::new();
    let (mut other_attrs, mut morphs, mut mirrored) = (0usize, 0usize, 0usize);
    let mut cache: HashMap<usize, Vec<Vec3>> = HashMap::new();
    for &(n, m, world) in &instances {
        let name = name_of(&nodes[n])
            .or_else(|| name_of(&meshes[m]))
            .unwrap_or_else(|| format!("mesh{m}"));
        let tag = *name_index.entry(name.clone()).or_insert_with(|| {
            names.push(name);
            (names.len() - 1) as u32
        });
        let flip = world.as_ref().is_some_and(|w| det3(w) < 0.0);
        if flip {
            mirrored += 1;
        }
        for (p, prim) in list(&meshes[m], "primitives", &format!("meshes[{m}]"))?
            .iter()
            .enumerate()
        {
            let path = format!("meshes[{m}].primitives[{p}]");
            let mode = opt_index(prim, "mode", &path)?.unwrap_or(4) as u64;
            if mode < 4 {
                *skipped.entry(mode).or_default() += 1;
                continue;
            }
            if mode > 6 {
                return Err(e(format!("{path}.mode: unknown value {mode}")));
            }
            let attrs = &prim["attributes"];
            let pa = req_index(attrs, "POSITION", &format!("{path}.attributes"))?;
            if attrs.as_object().is_some_and(|o| o.len() > 1) {
                other_attrs += 1;
            }
            if prim.get("targets").is_some() {
                morphs += 1;
            }
            let local = match cache.get(&pa) {
                Some(v) => v.clone(),
                None => {
                    let v = doc.positions(pa)?;
                    cache.insert(pa, v.clone());
                    v
                }
            };
            let nv = local.len();
            let idx: Vec<usize> = match opt_index(prim, "indices", &path)? {
                Some(i) => doc.indices(i, nv)?,
                None => (0..nv).collect(),
            };
            let k = idx.len();
            let mut local_tris: Vec<[usize; 3]> = Vec::new();
            match mode {
                4 => {
                    if k % 3 != 0 {
                        return Err(e(format!("{path}: {k} vertices for triangles, not a multiple of 3")));
                    }
                    local_tris.extend(idx.chunks(3).map(|c| [c[0], c[1], c[2]]));
                }
                5 => {
                    if k < 3 {
                        return Err(e(format!(
                            "{path}: a triangle strip needs at least 3 vertices, found {k}"
                        )));
                    }
                    for i in 0..k - 2 {
                        local_tris.push([idx[i], idx[i + 1 + i % 2], idx[i + 2 - i % 2]]);
                    }
                }
                _ => {
                    if k < 3 {
                        return Err(e(format!(
                            "{path}: a triangle fan needs at least 3 vertices, found {k}"
                        )));
                    }
                    for i in 0..k - 2 {
                        local_tris.push([idx[i + 1], idx[i + 2], idx[0]]);
                    }
                }
            }
            if points.len() + nv > MAX_ITEMS || tris.len() + local_tris.len() > MAX_ITEMS {
                return Err(e(format!(
                    "{path}: more than {MAX_ITEMS} vertices or triangles in total"
                )));
            }
            let base = points.len();
            for q in &local {
                let w = match &world {
                    Some(mtx) => apply(mtx, *q),
                    None => *q,
                };
                points.push([w[0] * factor, w[1] * factor, w[2] * factor]);
            }
            for t in local_tris {
                if flip {
                    tris.push([base + t[0], base + t[2], base + t[1]]);
                } else {
                    tris.push([base + t[0], base + t[1], base + t[2]]);
                }
                tags.push(tag);
            }
        }
    }
    if points.iter().any(|p| p.iter().any(|c| !c.is_finite())) {
        return Err(e("a transformed vertex is not finite".into()));
    }

    let mut notes = Vec::new();
    if !skipped.is_empty() {
        let list: Vec<String> = skipped.iter().map(|(m, n)| format!("{n} of mode {m}")).collect();
        notes.push(format!("point and line primitives skipped: {}", list.join(", ")));
    }
    if other_attrs > 0 {
        notes.push(format!(
            "{other_attrs} primitives carry attributes other than POSITION (normals, texture coordinates, ...), ignored"
        ));
    }
    if morphs > 0 {
        notes.push(format!(
            "{morphs} primitives have morph targets, ignored (base positions used)"
        ));
    }
    if skins > 0 {
        notes.push(format!("{skins} nodes have skins, ignored (bind pose used)"));
    }
    if cameras > 0 {
        notes.push(format!("{cameras} cameras ignored"));
    }
    if mirrored > 0 {
        notes.push(format!(
            "{mirrored} mesh instances have a mirroring transform; their triangle order was reversed"
        ));
    }
    for key in ["materials", "textures", "images", "animations"] {
        let n = list(root, key, "")?.len();
        if n > 0 {
            notes.push(format!("{n} {key} ignored"));
        }
    }
    let unread = meshes.len() - instances.iter().map(|i| i.1).collect::<BTreeSet<_>>().len();
    if unread > 0 {
        notes.push(format!(
            "{unread} meshes are not used by a node of the scene and were not read"
        ));
    }
    finish_import(points, tris, tags, names, notes, "glTF")
}

fn index_list(v: &Value, key: &str, path: &str) -> Result<Vec<usize>> {
    list(v, key, path)?
        .iter()
        .enumerate()
        .map(|(i, x)| {
            x.as_u64()
                .filter(|&n| n <= usize::MAX as u64)
                .map(|n| n as usize)
                .ok_or_else(|| e(format!("{}[{i}]: not a non-negative integer", at(path, key))))
        })
        .collect()
}
