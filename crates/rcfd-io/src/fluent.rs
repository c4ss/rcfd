//! Fluent `.msh` / `.cas` mesh reader (F-118), from the publicly documented section layout.
//!
//! Source: written from memory of a public description of the section layout. No specification document is named in
//! the record, so the source is UNVERIFIED (docs/provenance.md). The ASCII 2D layout and the boundary type ids 2, 3, 5
//! and 10 were checked only as a black box against the public `elbow.msh` sample (an OpenFOAM tutorial file, GPL,
//! fetched and never committed; `tests/fluent_public.rs`).
//!
//! Handled sections: comment (0), header (1), dimension (2), nodes (10, binary 2010/3010),
//! cells (12, binary 2012/3012), faces (13, binary 2013/3013) and zone names (45 / 39). The
//! solver-setup sections of a `.cas` file are skipped and listed in [`Report::ignored_sections`].
//! Cell shapes are irrelevant: cells are rebuilt from their faces, so polyhedral meshes work.
//!
//! * 3D meshes are read as they are.
//! * 2D meshes (dimension 2, two-node faces) are extruded by [`Options::thickness`] into one
//!   layer of prisms; front and back form the `frontAndBack` empty patch, as the rcfd 2D
//!   generators do.
//! * Binary sections are read as little-endian, 32-bit integers for connectivity and 32-bit
//!   (2010) or 64-bit (3010) floats for coordinates. This layout and the `End of Binary Section`
//!   terminator are written from memory of the public description and are NOT verified against a
//!   real binary file (see docs/reference-data.md).
//! * Not supported and rejected with an explicit error: periodic zones (bc types 8 and 12,
//!   section 18), hanging-node parent faces (bc type 31), axisymmetric axis zones (bc type 37),
//!   multi-region files whose zones are not connected through faces, gzip-compressed input and
//!   HDF5 (`.cas.h5`) input.

use crate::common::{build_mesh, SurfaceGroup};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("fluent: {}", msg.into())))
}

/// Reader options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Depth of the extruded layer for 2D meshes (Fluent's planar default is 1).
    pub thickness: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { thickness: 1.0 }
    }
}

/// One face zone of the file and how it was mapped.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneInfo {
    /// Zone id.
    pub id: usize,
    /// Zone name (from the name section, or `zone_<id>`).
    pub name: String,
    /// Fluent boundary-condition type id of the zone.
    pub bc_type: usize,
    /// Boundary-condition type name when the file gives one.
    pub type_name: Option<String>,
    /// Number of faces.
    pub faces: usize,
    /// rcfd patch kind (None for interior zones, which become no patch).
    pub kind: Option<PatchKind>,
}

/// What the reader saw in the file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// 2 or 3.
    pub dimension: usize,
    /// At least one binary section was read.
    pub binary: bool,
    /// Face zones in file order.
    pub zones: Vec<ZoneInfo>,
    /// Sections that were skipped: (section index, how many times). Nothing is dropped silently.
    pub ignored_sections: Vec<(usize, usize)>,
}

#[derive(Debug)]
enum Node<'a> {
    Atom(&'a str),
    List(Vec<Node<'a>>),
}

fn parse_sexpr(src: &str) -> Result<Vec<Node<'_>>> {
    let b = src.as_bytes();
    let mut stack: Vec<Vec<Node>> = vec![vec![]];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                stack.push(vec![]);
                i += 1;
            }
            b')' => {
                if stack.len() < 2 {
                    return bad("unbalanced ')'");
                }
                let l = stack.pop().expect("non-empty");
                stack.last_mut().expect("parent").push(Node::List(l));
                i += 1;
            }
            b'"' => {
                let s = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                stack.last_mut().expect("top").push(Node::Atom(&src[s..i.min(b.len())]));
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let s = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'(' && b[i] != b')' && b[i] != b'"' {
                    i += 1;
                }
                stack.last_mut().expect("top").push(Node::Atom(&src[s..i]));
            }
        }
    }
    if stack.len() != 1 {
        return bad("unbalanced '('");
    }
    Ok(stack.pop().expect("root"))
}

fn atom<'a>(n: &Node<'a>) -> Option<&'a str> {
    match n {
        Node::Atom(a) => Some(a),
        Node::List(_) => None,
    }
}

fn hex(s: &str) -> Result<usize> {
    usize::from_str_radix(s, 16).or_else(|_| bad(format!("bad hex number '{s}'")))
}

struct FaceRec {
    nodes: Vec<usize>,
    c0: usize,
    c1: usize,
}

struct FaceZone {
    id: usize,
    bc_type: usize,
    faces: Vec<FaceRec>,
}

const END_MARK: &[u8] = b"End of Binary Section";

enum Raw {
    /// A parenthesised text section.
    Text(String),
    /// A binary-capable section (index >= 2000): header atoms and the raw payload, if any.
    Binary {
        index: usize,
        header: Vec<String>,
        payload: Option<Vec<u8>>,
    },
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// End (exclusive) of the balanced parenthesis group starting at `i` (b[i] == '(').
fn balanced_end(b: &[u8], mut i: usize) -> Result<usize> {
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i + 1);
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bad("unbalanced '(' (section is not closed)")
}

fn split_sections(b: &[u8]) -> Result<Vec<Raw>> {
    let mut out = Vec::new();
    let mut i = skip_ws(b, 0);
    while i < b.len() {
        if b[i] != b'(' {
            return bad(format!("unexpected data outside a section at byte {i}"));
        }
        let mut j = skip_ws(b, i + 1);
        let ds = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let index: Option<usize> = std::str::from_utf8(&b[ds..j]).ok().and_then(|s| s.parse().ok());
        match index {
            Some(index) if index >= 2000 => {
                let hs = skip_ws(b, j);
                if b.get(hs) != Some(&b'(') {
                    return bad(format!("section {index} has no header"));
                }
                let he = balanced_end(b, hs)?;
                let header: Vec<String> = String::from_utf8_lossy(&b[hs + 1..he - 1])
                    .split_whitespace()
                    .map(str::to_string)
                    .collect();
                let after = skip_ws(b, he);
                match b.get(after) {
                    Some(b')') => {
                        out.push(Raw::Binary {
                            index,
                            header,
                            payload: None,
                        });
                        i = skip_ws(b, after + 1);
                    }
                    Some(b'(') => {
                        let start = after + 1;
                        let Some(m) = b[start..].windows(END_MARK.len()).position(|w| w == END_MARK) else {
                            return bad(format!(
                                "binary section {index} is not terminated by 'End of Binary Section'"
                            ));
                        };
                        let mut e = start + m + END_MARK.len();
                        while e < b.len() && b[e] != b')' {
                            e += 1;
                        }
                        if e >= b.len() {
                            return bad(format!("binary section {index}: missing closing ')'"));
                        }
                        e += 1;
                        // the data parenthesis and the section parenthesis may be closed separately
                        let nxt = skip_ws(b, e);
                        if b.get(nxt) == Some(&b')') {
                            e = nxt + 1;
                        }
                        out.push(Raw::Binary {
                            index,
                            header,
                            payload: Some(b[start..start + m].to_vec()),
                        });
                        i = skip_ws(b, e);
                    }
                    _ => return bad(format!("binary section {index}: malformed")),
                }
            }
            _ => {
                let e = balanced_end(b, i)?;
                out.push(Raw::Text(String::from_utf8_lossy(&b[i..e]).into_owned()));
                i = skip_ws(b, e);
            }
        }
    }
    Ok(out)
}

struct State {
    dim: usize,
    binary: bool,
    points: Vec<Vec3>,
    zone_names: HashMap<usize, String>,
    zone_types: HashMap<usize, String>,
    cell_zones: Vec<(usize, usize, usize)>,
    face_zones: Vec<FaceZone>,
    ignored: BTreeMap<usize, usize>,
}

fn set_dim(st: &mut State, d: usize) -> Result<()> {
    if d != 2 && d != 3 {
        return bad(format!("unsupported dimension {d}"));
    }
    if st.dim != 0 && st.dim != d {
        return bad(format!("dimension {d} contradicts the earlier dimension {}", st.dim));
    }
    st.dim = d;
    Ok(())
}

fn store_nodes(st: &mut State, f: usize, l: usize, nd: usize, coords: &[f64]) -> Result<()> {
    set_dim(st, nd)?;
    if f == 0 || l < f {
        return bad(format!("bad node range {f:x}..{l:x}"));
    }
    if coords.len() != nd * (l + 1 - f) {
        return bad(format!(
            "node section: expected {} coordinates, found {}",
            nd * (l + 1 - f),
            coords.len()
        ));
    }
    if st.points.len() < l {
        st.points.resize(l, [0.0; 3]);
    }
    for (k, c) in coords.chunks(nd).enumerate() {
        let mut p = [0.0; 3];
        p[..nd].copy_from_slice(c);
        st.points[f - 1 + k] = p;
    }
    Ok(())
}

fn text_section(st: &mut State, items: &[Node]) -> Result<()> {
    let Some(first) = items.first().and_then(atom) else {
        return Ok(());
    };
    let Ok(index) = first.parse::<usize>() else {
        return Ok(());
    };
    match index {
        0 | 1 => {}
        2 => match items.get(1).and_then(atom).and_then(|s| s.parse::<usize>().ok()) {
            Some(d) => set_dim(st, d)?,
            None => return bad("malformed dimension section"),
        },
        10 => {
            let Some(Node::List(h)) = items.get(1) else {
                return bad("malformed node section");
            };
            let zone = hex(atom(&h[0]).unwrap_or("0"))?;
            if zone == 0 {
                return Ok(());
            }
            let (f, l) = (hex(atom(&h[1]).unwrap_or("0"))?, hex(atom(&h[2]).unwrap_or("0"))?);
            let nd = h
                .get(4)
                .and_then(atom)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(if st.dim == 2 { 2 } else { 3 });
            let Some(Node::List(data)) = items.get(2) else {
                return bad("node section without data");
            };
            let mut coords = Vec::with_capacity(data.len());
            for n in data {
                let s = atom(n).unwrap_or("");
                coords.push(s.parse::<f64>().or_else(|_| bad(format!("bad coordinate '{s}'")))?);
            }
            store_nodes(st, f, l, nd, &coords)?;
        }
        12 => {
            let Some(Node::List(h)) = items.get(1) else {
                return bad("malformed cell section");
            };
            let zone = hex(atom(&h[0]).unwrap_or("0"))?;
            if zone == 0 {
                return Ok(());
            }
            let (f, l) = (hex(atom(&h[1]).unwrap_or("0"))?, hex(atom(&h[2]).unwrap_or("0"))?);
            let ty = h.get(3).and_then(atom).map(hex).transpose()?.unwrap_or(1);
            if ty != 0 {
                st.cell_zones.push((zone, f, l));
            }
        }
        13 => {
            let Some(Node::List(h)) = items.get(1) else {
                return bad("malformed face section");
            };
            let zone = hex(atom(&h[0]).unwrap_or("0"))?;
            if zone == 0 {
                return Ok(());
            }
            let bc_type = hex(atom(&h[3]).unwrap_or("0"))?;
            let ftype = hex(atom(&h[4]).unwrap_or("0"))?;
            let Some(Node::List(data)) = items.get(2) else {
                return bad("face section without data");
            };
            let toks: Vec<&str> = data.iter().filter_map(atom).collect();
            let mut faces = Vec::new();
            let mut i = 0;
            while i < toks.len() {
                let n = if ftype == 0 || ftype == 5 {
                    let n = hex(toks[i])?;
                    i += 1;
                    n
                } else {
                    ftype
                };
                if i + n + 2 > toks.len() {
                    return bad("truncated face record");
                }
                let nodes: Result<Vec<usize>> = toks[i..i + n]
                    .iter()
                    .map(|t| {
                        hex(t).and_then(|v| v.checked_sub(1).ok_or_else(|| Error::Io("fluent: node index 0".into())))
                    })
                    .collect();
                faces.push(FaceRec {
                    nodes: nodes?,
                    c0: hex(toks[i + n])?,
                    c1: hex(toks[i + n + 1])?,
                });
                i += n + 2;
            }
            st.face_zones.push(FaceZone {
                id: zone,
                bc_type,
                faces,
            });
        }
        45 | 39 => {
            if let Some(Node::List(h)) = items.get(1) {
                let id = h.first().and_then(atom).and_then(|s| s.parse::<usize>().ok());
                let ty = h.get(1).and_then(atom);
                let name = h.get(2).and_then(atom);
                if let (Some(id), Some(name)) = (id, name) {
                    st.zone_names.insert(id, name.to_string());
                    if let Some(ty) = ty {
                        st.zone_types.insert(id, ty.to_string());
                    }
                }
            }
        }
        18 => return bad("periodic shadow faces (section 18) are not supported"),
        other => *st.ignored.entry(other).or_default() += 1,
    }
    Ok(())
}

fn le_i32(b: &[u8], i: usize) -> i64 {
    i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as i64
}

/// Parse `nfaces` binary face records from `region`; leading newline bytes may precede the data.
fn binary_faces(region: &[u8], nfaces: usize, ftype: usize) -> Result<Vec<FaceRec>> {
    'offset: for k in 0..=2usize.min(region.len()) {
        if region[..k].iter().any(|c| *c != b'\n' && *c != b'\r') {
            break;
        }
        let r = &region[k..];
        let mut faces = Vec::with_capacity(nfaces);
        let mut i = 0;
        for _ in 0..nfaces {
            let n = if ftype == 0 || ftype == 5 {
                if i + 4 > r.len() {
                    continue 'offset;
                }
                let n = le_i32(r, i);
                i += 4;
                if !(2..=4096).contains(&n) {
                    continue 'offset;
                }
                n as usize
            } else {
                ftype
            };
            if i + 4 * (n + 2) > r.len() {
                continue 'offset;
            }
            let mut nodes = Vec::with_capacity(n);
            for q in 0..n {
                let v = le_i32(r, i + 4 * q);
                if v < 1 {
                    continue 'offset;
                }
                nodes.push(v as usize - 1);
            }
            let (c0, c1) = (le_i32(r, i + 4 * n), le_i32(r, i + 4 * n + 4));
            if c0 < 0 || c1 < 0 {
                continue 'offset;
            }
            faces.push(FaceRec {
                nodes,
                c0: c0 as usize,
                c1: c1 as usize,
            });
            i += 4 * (n + 2);
        }
        if r[i..].iter().all(|c| c.is_ascii_whitespace()) {
            return Ok(faces);
        }
    }
    bad("binary face section: data size does not match the header (endianness or layout differs?)")
}

fn binary_section(st: &mut State, index: usize, header: &[String], payload: Option<&[u8]>) -> Result<()> {
    match index {
        2010 | 3010 | 2013 | 3013 | 2012 | 3012 => {}
        2018 | 3018 => return bad(format!("periodic shadow faces (section {index}) are not supported")),
        other => {
            *st.ignored.entry(other).or_default() += 1;
            return Ok(());
        }
    }
    if header.len() < 3 {
        return bad(format!("section {index}: short header"));
    }
    let zone = hex(&header[0])?;
    if zone == 0 {
        return Ok(());
    }
    let (f, l) = (hex(&header[1])?, hex(&header[2])?);
    st.binary = true;
    match index {
        2012 | 3012 => {
            let ty = header.get(3).map(|s| hex(s)).transpose()?.unwrap_or(1);
            if ty != 0 {
                st.cell_zones.push((zone, f, l));
            }
        }
        2010 | 3010 => {
            let nd = header
                .get(4)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(if st.dim == 2 { 2 } else { 3 });
            let size = if index == 2010 { 4 } else { 8 };
            let Some(region) = payload else {
                return bad(format!("section {index} has no data"));
            };
            if l < f {
                return bad(format!("section {index}: bad node range"));
            }
            let expected = (l + 1 - f) * nd * size;
            let mut k = 0;
            while region.len() - k > expected && k < 2 && (region[k] == b'\n' || region[k] == b'\r') {
                k += 1;
            }
            let data = &region[k..];
            if data.len() < expected || !data[expected..].iter().all(|c| c.is_ascii_whitespace()) {
                return bad(format!(
                    "section {index}: expected {expected} data bytes, found {}",
                    data.len()
                ));
            }
            let coords: Vec<f64> = (0..(l + 1 - f) * nd)
                .map(|q| {
                    if size == 4 {
                        f32::from_le_bytes([data[4 * q], data[4 * q + 1], data[4 * q + 2], data[4 * q + 3]]) as f64
                    } else {
                        let mut a = [0u8; 8];
                        a.copy_from_slice(&data[8 * q..8 * q + 8]);
                        f64::from_le_bytes(a)
                    }
                })
                .collect();
            store_nodes(st, f, l, nd, &coords)?;
        }
        _ => {
            let bc_type = hex(header.get(3).map_or("0", String::as_str))?;
            let ftype = hex(header.get(4).map_or("0", String::as_str))?;
            let Some(region) = payload else {
                return bad(format!("section {index} has no data"));
            };
            let faces = binary_faces(region, l + 1 - f, ftype)?;
            st.face_zones.push(FaceZone {
                id: zone,
                bc_type,
                faces,
            });
        }
    }
    Ok(())
}

fn kind_of(id: usize, bc_type: usize, type_name: Option<&str>) -> Result<Option<Option<PatchKind>>> {
    // Ok(None) = interior (no patch); Ok(Some(None)) = a patch whose kind is guessed from its name
    let t = type_name.map(str::to_ascii_lowercase);
    if let Some(t) = t.as_deref() {
        return Ok(match t {
            "interior" => None,
            "wall" => Some(Some(PatchKind::Wall)),
            "pressure-inlet" | "velocity-inlet" | "mass-flow-inlet" | "inlet-vent" | "intake-fan" => {
                Some(Some(PatchKind::Inlet))
            }
            "pressure-outlet" | "outflow" | "outlet-vent" | "exhaust-fan" => Some(Some(PatchKind::Outlet)),
            "symmetry" => Some(Some(PatchKind::Symmetry)),
            "periodic" | "periodic-shadow" | "axis" | "parent" => {
                return bad(format!("zone {id}: boundary type '{t}' is not supported"))
            }
            _ => Some(None),
        });
    }
    Ok(match bc_type {
        2 => None,
        3 => Some(Some(PatchKind::Wall)),
        4 | 10 | 20 => Some(Some(PatchKind::Inlet)),
        5 | 36 => Some(Some(PatchKind::Outlet)),
        7 => Some(Some(PatchKind::Symmetry)),
        8 | 12 | 31 | 37 => {
            return bad(format!(
                "zone {id}: boundary type id {bc_type} (periodic, hanging-node parent or axis) is not supported"
            ))
        }
        _ => Some(None),
    })
}

fn polygon_loop(edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    if edges.len() < 3 {
        return None;
    }
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(a, b) in edges {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    if adj.values().any(|v| v.len() != 2) {
        return None;
    }
    let start = edges[0].0;
    let mut out = vec![start];
    let mut prev = start;
    let mut cur = adj[&start][0];
    while cur != start {
        out.push(cur);
        let nb = &adj[&cur];
        let next = if nb[0] != prev { nb[0] } else { nb[1] };
        prev = cur;
        cur = next;
        if out.len() > edges.len() {
            return None;
        }
    }
    (out.len() == edges.len()).then_some(out)
}

/// Read a Fluent mesh file (ASCII or binary sections).
pub fn read_file(path: impl AsRef<Path>) -> Result<Mesh> {
    read_file_with(path, &Options::default()).map(|(m, _)| m)
}

/// Read a Fluent mesh file and return what was found.
pub fn read_file_with(path: impl AsRef<Path>, opt: &Options) -> Result<(Mesh, Report)> {
    let src = std::fs::read(path.as_ref())?;
    read_bytes(&src, opt)
}

/// Parse Fluent mesh text.
pub fn read_str(src: &str) -> Result<Mesh> {
    read_bytes(src.as_bytes(), &Options::default()).map(|(m, _)| m)
}

/// Parse a Fluent mesh from bytes.
pub fn read_bytes(src: &[u8], opt: &Options) -> Result<(Mesh, Report)> {
    if src.starts_with(&[0x1f, 0x8b]) {
        return bad("gzip-compressed file: decompress it first (gunzip)");
    }
    if src.starts_with(b"\x89HDF") {
        return bad("HDF5 (.cas.h5) files are not supported yet; export an ASCII or binary .msh/.cas file");
    }
    let mut st = State {
        dim: 0,
        binary: false,
        points: Vec::new(),
        zone_names: HashMap::new(),
        zone_types: HashMap::new(),
        cell_zones: Vec::new(),
        face_zones: Vec::new(),
        ignored: BTreeMap::new(),
    };
    for raw in split_sections(src)? {
        match raw {
            Raw::Text(t) => {
                let top = parse_sexpr(&t)?;
                if let Some(Node::List(items)) = top.first() {
                    text_section(&mut st, items)?;
                }
            }
            Raw::Binary { index, header, payload } => binary_section(&mut st, index, &header, payload.as_deref())?,
        }
    }
    if st.points.is_empty() || st.face_zones.is_empty() {
        return bad("no nodes or faces found");
    }
    let dim = if st.dim == 0 { 3 } else { st.dim };
    let n_cells = st
        .face_zones
        .iter()
        .flat_map(|z| z.faces.iter())
        .map(|f| f.c0.max(f.c1))
        .max()
        .unwrap_or(0);
    let npts = st.points.len();
    for z in &st.face_zones {
        for f in &z.faces {
            if f.nodes.iter().any(|&p| p >= npts) {
                return bad("face references a node out of range");
            }
            let want = if dim == 2 { 2 } else { 3 };
            if f.nodes.len() < want || (dim == 2 && f.nodes.len() != 2) {
                return bad(format!("a {}-node face in a {dim}D mesh", f.nodes.len()));
            }
        }
    }

    // map every zone first: unsupported types are errors, interior zones become no patch
    let mut report = Report {
        dimension: dim,
        binary: st.binary,
        zones: Vec::new(),
        ignored_sections: st.ignored.iter().map(|(k, v)| (*k, *v)).collect(),
    };
    let mut kinds: Vec<Option<Option<PatchKind>>> = Vec::new();
    for z in &st.face_zones {
        let k = kind_of(z.id, z.bc_type, st.zone_types.get(&z.id).map(String::as_str))?;
        kinds.push(k.clone());
        report.zones.push(ZoneInfo {
            id: z.id,
            name: st
                .zone_names
                .get(&z.id)
                .cloned()
                .unwrap_or_else(|| format!("zone_{}", z.id)),
            bc_type: z.bc_type,
            type_name: st.zone_types.get(&z.id).cloned(),
            faces: z.faces.len(),
            kind: k.flatten(),
        });
    }

    let groups_of = |cells_n: usize| -> Vec<(String, Vec<usize>)> {
        st.cell_zones
            .iter()
            .map(|&(id, f, l)| {
                (
                    st.zone_names.get(&id).cloned().unwrap_or_else(|| format!("zone_{id}")),
                    (f.saturating_sub(1)..l.min(cells_n)).collect(),
                )
            })
            .collect()
    };

    let mesh = if dim == 3 {
        let mut cells: Vec<Vec<Vec<usize>>> = vec![vec![]; n_cells];
        for z in &st.face_zones {
            for f in &z.faces {
                for c in [f.c0, f.c1] {
                    if c > 0 {
                        cells[c - 1].push(f.nodes.clone());
                    }
                }
            }
        }
        if cells.iter().any(|c| c.is_empty()) {
            return bad("some cells have no faces");
        }
        let mut surfaces = Vec::new();
        for ((z, k), info) in st.face_zones.iter().zip(&kinds).zip(&report.zones) {
            let Some(kind) = k else { continue };
            surfaces.push(SurfaceGroup {
                name: info.name.clone(),
                faces: z.faces.iter().map(|f| f.nodes.clone()).collect(),
                kind: kind.clone(),
            });
        }
        build_mesh(st.points.clone(), cells, surfaces, groups_of(n_cells))?
    } else {
        if opt.thickness.is_nan() || opt.thickness <= 0.0 {
            return bad("the 2D extrusion thickness must be positive");
        }
        let n = npts;
        let mut points = Vec::with_capacity(2 * n);
        points.extend(st.points.iter().map(|p| [p[0], p[1], 0.0]));
        points.extend(st.points.iter().map(|p| [p[0], p[1], opt.thickness]));
        let mut cell_edges: Vec<Vec<(usize, usize)>> = vec![vec![]; n_cells];
        for z in &st.face_zones {
            for f in &z.faces {
                for c in [f.c0, f.c1] {
                    if c > 0 {
                        cell_edges[c - 1].push((f.nodes[0], f.nodes[1]));
                    }
                }
            }
        }
        let quad = |a: usize, b: usize| vec![a, b, b + n, a + n];
        let mut cells = Vec::with_capacity(n_cells);
        let mut ends: Vec<Vec<usize>> = Vec::new();
        for (ci, edges) in cell_edges.iter().enumerate() {
            let Some(lp) = polygon_loop(edges) else {
                return bad(format!("cell {} is not a closed polygon of its edges", ci + 1));
            };
            let mut faces: Vec<Vec<usize>> = edges.iter().map(|&(a, b)| quad(a, b)).collect();
            let back: Vec<usize> = lp.iter().map(|&p| p + n).collect();
            ends.push(lp.clone());
            ends.push(back.clone());
            faces.push(lp);
            faces.push(back);
            cells.push(faces);
        }
        let mut surfaces = Vec::new();
        for ((z, k), info) in st.face_zones.iter().zip(&kinds).zip(&report.zones) {
            let Some(kind) = k else { continue };
            surfaces.push(SurfaceGroup {
                name: info.name.clone(),
                faces: z.faces.iter().map(|f| quad(f.nodes[0], f.nodes[1])).collect(),
                kind: kind.clone(),
            });
        }
        surfaces.push(SurfaceGroup {
            name: "frontAndBack".into(),
            faces: ends,
            kind: Some(PatchKind::Empty),
        });
        build_mesh(points, cells, surfaces, groups_of(n_cells))?
    };
    Ok((mesh, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two unit hexes along x, point id (1-based) = 1 + x + 3y + 6z.
    fn sample(mixed: bool) -> String {
        let id = |x: usize, y: usize, z: usize| 1 + x + 3 * y + 6 * z;
        let quad = |a: [usize; 3], b: [usize; 3], c: [usize; 3], d: [usize; 3], c0: usize, c1: usize| {
            let n = |p: [usize; 3]| format!("{:x}", id(p[0], p[1], p[2]));
            format!(
                "{}{} {} {} {} {} {}\n",
                if mixed { "4 " } else { "" },
                n(a),
                n(b),
                n(c),
                n(d),
                c0,
                c1
            )
        };
        let mut s = String::from(
            "(0 \"Sample (with parens) comment\")\n(1 \"rcfd test\")\n(2 3)\n(10 (0 1 c 0 3))\n(10 (1 1 c 1 3)(\n",
        );
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    s += &format!("{:e} {:e} {:e}\n", x as f64, y as f64, z as f64);
                }
            }
        }
        s += "))\n(12 (0 1 2 0))\n(12 (7 1 2 1 4))\n";
        s += &format!("(13 (0 1 b 0 0))\n(13 (2 1 1 2 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1], 1, 2);
        s += ")\n)\n";
        // inlet x=0 (zone 4, pressure-inlet) cell 1
        s += &format!("(13 (4 2 2 4 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0], 1, 0);
        s += "))\n";
        // outlet x=2 (zone 5) cell 2
        s += &format!("(13 (5 3 3 5 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([2, 0, 0], [2, 1, 0], [2, 1, 1], [2, 0, 1], 2, 0);
        s += "))\n";
        // walls (zone 3): remaining 8 faces
        s += &format!("(13 (3 4 b 3 {})(\n", if mixed { 0 } else { 4 });
        for (x0, c) in [(0, 1), (1, 2)] {
            s += &quad([x0, 0, 0], [x0 + 1, 0, 0], [x0 + 1, 0, 1], [x0, 0, 1], c, 0); // y=0
            s += &quad([x0, 1, 0], [x0, 1, 1], [x0 + 1, 1, 1], [x0 + 1, 1, 0], c, 0); // y=1
            s += &quad([x0, 0, 0], [x0, 1, 0], [x0 + 1, 1, 0], [x0 + 1, 0, 0], c, 0); // z=0
            s += &quad([x0, 0, 1], [x0 + 1, 0, 1], [x0 + 1, 1, 1], [x0, 1, 1], c, 0);
            // z=1
        }
        s += "))\n(45 (7 fluid fluid)())\n(45 (3 wall wall-3)())\n(45 (4 pressure-inlet in)())\n(45 (5 pressure-outlet out)())\n(45 (2 interior interior-7)())\n";
        s
    }

    #[test]
    fn reads_quad_and_mixed_face_sections() {
        let a = read_str(&sample(false)).unwrap();
        let b = read_str(&sample(true)).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
        for m in [&a, &b] {
            assert_eq!(m.n_cells(), 2);
            assert_eq!(m.n_points(), 12);
            assert_eq!(m.n_internal_faces(), 1);
            assert_eq!(m.n_faces(), 11);
            assert!((m.total_volume() - 2.0).abs() < 1e-12);
            let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
            assert_eq!(names, vec!["in", "out", "wall-3"]);
            assert_eq!(m.patch("in").unwrap().kind, PatchKind::Inlet);
            assert_eq!(m.patch("out").unwrap().kind, PatchKind::Outlet);
            assert_eq!(m.patch("wall-3").unwrap().kind, PatchKind::Wall);
            assert_eq!(m.patch("wall-3").unwrap().size, 8);
            assert_eq!(m.cell_zone("fluid").unwrap().ids, vec![0, 1]);
        }
    }

    #[test]
    fn rejects_unsupported_and_corrupt_input() {
        assert!(read_str("(2 2)\n").is_err());
        assert!(read_str("(2010 (1 1 2 3 4)(1 2 3))\n").is_err());
        assert!(read_str("(10 (1 1 2 1 3)(0 0 0))\n").is_err());
        assert!(read_str("((").is_err());
        assert!(read_str(&sample(false).replace("(13 (3 4 b 3 4)(", "(13 (3 4 b 3 4)(1")).is_err());
        assert!(read_str(&sample(false).replace("(45 (3 wall wall-3)", "(45 (3 periodic wall-3)")).is_err());
        assert!(read_bytes(&[0x1f, 0x8b, 8, 0], &Options::default()).is_err());
        assert!(read_bytes(b"\x89HDF\r\n\x1a\n", &Options::default()).is_err());
        assert!(read_str(&(sample(false) + "(18 (1 2 3 4 5 6)())\n")).is_err());
    }

    #[test]
    fn unknown_sections_are_reported_not_dropped() {
        let src = sample(false) + "(33 (1 2))\n(37 ())\n(33 (3))\n(4 (x))\n";
        let (m, rep) = read_bytes(src.as_bytes(), &Options::default()).unwrap();
        assert_eq!(m.n_cells(), 2);
        assert_eq!(rep.ignored_sections, vec![(4, 1), (33, 2), (37, 1)]);
        assert_eq!(rep.dimension, 3);
        let names: Vec<&str> = rep.zones.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(names, vec!["interior-7", "in", "out", "wall-3"]);
        assert_eq!(rep.zones[0].kind, None);
    }

    // Two unit squares side by side (x in [0,2], y in [0,1]); point id = 1 + x + 3 y.
    fn sample_2d() -> String {
        let id = |x: usize, y: usize| format!("{:x}", 1 + x + 3 * y);
        let mut s = String::from(
            "(2 2)
(10 (0 1 6 0))
(10 (1 1 6 1 2)(
",
        );
        for y in 0..2 {
            for x in 0..3 {
                s += &format!(
                    "{} {}
",
                    x as f64, y as f64
                );
            }
        }
        s += "))
(12 (0 1 2 0))
(12 (9 1 2 1 3))
";
        s += &format!(
            "(13 (3 1 1 2 2)(
{} {} 1 2
))
",
            id(1, 0),
            id(1, 1)
        );
        s += &format!(
            "(13 (4 2 3 3 2)(
{} {} 1 0
{} {} 2 0
))
",
            id(0, 0),
            id(1, 0),
            id(1, 0),
            id(2, 0)
        );
        s += &format!(
            "(13 (5 4 5 3 2)(
{} {} 1 0
{} {} 2 0
))
",
            id(0, 1),
            id(1, 1),
            id(1, 1),
            id(2, 1)
        );
        s += &format!(
            "(13 (6 6 6 a 2)(
{} {} 1 0
))
",
            id(0, 0),
            id(0, 1)
        );
        s += &format!(
            "(13 (7 7 7 5 2)(
{} {} 2 0
))
",
            id(2, 0),
            id(2, 1)
        );
        s += "(45 (3 interior int)())
(45 (4 wall bottom)())
(45 (5 wall top)())
(45 (6 velocity-inlet in)())
(45 (7 pressure-outlet out)())
(45 (9 fluid fluid)())
";
        s
    }

    #[test]
    fn two_d_mesh_is_extruded_with_an_empty_patch() {
        let (m, rep) = read_bytes(sample_2d().as_bytes(), &Options { thickness: 0.5 }).unwrap();
        assert_eq!(rep.dimension, 2);
        assert_eq!(m.n_cells(), 2);
        assert_eq!(m.n_points(), 12);
        assert!((m.total_volume() - 1.0).abs() < 1e-12, "{}", m.total_volume());
        let fb = m.patch("frontAndBack").unwrap();
        assert_eq!((fb.kind.clone(), fb.size), (PatchKind::Empty, 4));
        assert_eq!(m.patch("in").unwrap().kind, PatchKind::Inlet);
        assert_eq!(m.patch("out").unwrap().kind, PatchKind::Outlet);
        assert_eq!(m.patch("bottom").unwrap().size, 2);
        assert_eq!(m.n_internal_faces(), 1);
        assert_eq!(m.cell_zone("fluid").unwrap().ids, vec![0, 1]);
        assert!(read_bytes(sample_2d().as_bytes(), &Options { thickness: 0.0 }).is_err());
    }

    // the same 3D sample written with binary node and face sections (layout as documented in the
    // module header; not a real Fluent-written binary file)
    fn binary_sample(double: bool) -> Vec<u8> {
        let mut b: Vec<u8> = Vec::new();
        b.extend_from_slice(
            b"(2 3)
",
        );
        let (ni, fi) = if double { (3010, 3013) } else { (2010, 2013) };
        b.extend_from_slice(format!("({ni} (1 1 c 1 3)(").as_bytes());
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    for v in [x, y, z] {
                        if double {
                            b.extend_from_slice(&(v as f64).to_le_bytes());
                        } else {
                            b.extend_from_slice(&(v as f32).to_le_bytes());
                        }
                    }
                }
            }
        }
        b.extend_from_slice(
            format!(
                "End of Binary Section   {ni})
"
            )
            .as_bytes(),
        );
        b.extend_from_slice(
            b"(12 (7 1 2 1 4))
",
        );
        let id = |x: usize, y: usize, z: usize| (1 + x + 3 * y + 6 * z) as i32;
        let mut face_sec = |zone: usize, first: usize, last: usize, bc: usize, faces: &[([i32; 4], i32, i32)]| {
            b.extend_from_slice(format!("({fi} ({zone:x} {first:x} {last:x} {bc:x} 4)(").as_bytes());
            for (n, c0, c1) in faces {
                for v in n.iter().chain([c0, c1]) {
                    b.extend_from_slice(&v.to_le_bytes());
                }
            }
            b.extend_from_slice(
                format!(
                    "End of Binary Section   {fi})
"
                )
                .as_bytes(),
            );
        };
        let q = |a: (usize, usize, usize),
                 b_: (usize, usize, usize),
                 c: (usize, usize, usize),
                 d: (usize, usize, usize),
                 c0,
                 c1| {
            (
                [
                    id(a.0, a.1, a.2),
                    id(b_.0, b_.1, b_.2),
                    id(c.0, c.1, c.2),
                    id(d.0, d.1, d.2),
                ],
                c0,
                c1,
            )
        };
        face_sec(2, 1, 1, 2, &[q((1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1), 1, 2)]);
        face_sec(4, 2, 2, 4, &[q((0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0), 1, 0)]);
        face_sec(5, 3, 3, 5, &[q((2, 0, 0), (2, 1, 0), (2, 1, 1), (2, 0, 1), 2, 0)]);
        let mut walls = Vec::new();
        for (x0, c) in [(0, 1), (1, 2)] {
            walls.push(q((x0, 0, 0), (x0 + 1, 0, 0), (x0 + 1, 0, 1), (x0, 0, 1), c, 0));
            walls.push(q((x0, 1, 0), (x0, 1, 1), (x0 + 1, 1, 1), (x0 + 1, 1, 0), c, 0));
            walls.push(q((x0, 0, 0), (x0, 1, 0), (x0 + 1, 1, 0), (x0 + 1, 0, 0), c, 0));
            walls.push(q((x0, 0, 1), (x0 + 1, 0, 1), (x0 + 1, 1, 1), (x0, 1, 1), c, 0));
        }
        face_sec(3, 4, 11, 3, &walls);
        b.extend_from_slice(
            b"(45 (7 fluid fluid)())
(45 (3 wall wall-3)())
(45 (4 pressure-inlet in)())
(45 (5 pressure-outlet out)())
(45 (2 interior interior-7)())
",
        );
        b
    }

    #[test]
    fn binary_sections_match_the_ascii_mesh() {
        let ascii = read_str(&sample(false)).unwrap();
        for double in [false, true] {
            let (m, rep) = read_bytes(&binary_sample(double), &Options::default()).unwrap();
            assert!(rep.binary);
            assert_eq!(m.content_hash(), ascii.content_hash(), "double = {double}");
        }
        // truncated payload and a missing terminator are errors
        let b = binary_sample(true);
        assert!(read_bytes(&b[..b.len() - 120], &Options::default()).is_err());
        let cut = String::from_utf8_lossy(&b).replace("End of Binary Section   3013", "");
        assert!(read_bytes(cut.as_bytes(), &Options::default()).is_err());
    }
}
