//! STL surface files, ASCII and binary (F-119, ADR 0011 section 2.1).
//!
//! Written from the public STL description: ASCII `solid name` / `facet normal` / `outer loop` / three `vertex`
//! lines / `endloop` / `endfacet` / `endsolid`; binary = 80 byte header, little-endian `u32` triangle count, then per
//! triangle 12 `f32` (normal, three vertices) and a `u16` attribute. ASCII output uses the shortest representation
//! that reads back to the same `f64`, so an ASCII round trip is bit exact; binary stores `f32`.

use crate::guard::{check_count, check_file_size, finite};
use rcfd_core::vec3::{cross, dot, scale, sub, unit, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::collections::HashMap;
use std::fmt::Write as _;

/// One named surface made of triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct StlSolid {
    /// Solid name (ASCII `solid` line, or the printable part of the binary header).
    pub name: String,
    /// Triangles as three vertices each, counter-clockwise seen from outside.
    pub triangles: Vec<[Vec3; 3]>,
    /// Facet normals as stored in the file (zero vectors are allowed and mean "not given").
    pub stored_normals: Vec<Vec3>,
}

impl StlSolid {
    /// Normal computed from the vertex order (unit length, zero for a degenerate triangle).
    pub fn computed_normal(&self, i: usize) -> Vec3 {
        let [a, b, c] = self.triangles[i];
        unit(cross(sub(b, a), sub(c, a)))
    }

    /// Total triangle area.
    pub fn area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|[a, b, c]| 0.5 * rcfd_core::vec3::norm(cross(sub(*b, *a), sub(*c, *a))))
            .sum()
    }
}

/// Triangulate the boundary faces of the named patches (fan from the first vertex; faces must be convex).
/// The stored normal is the unit area vector of the source face.
pub fn solid_from_patches(mesh: &Mesh, name: &str, patches: &[&str]) -> Result<StlSolid> {
    let mut s = StlSolid {
        name: name.to_string(),
        triangles: Vec::new(),
        stored_normals: Vec::new(),
    };
    let pts = mesh.points();
    for pn in patches {
        let p = mesh
            .patch(pn)
            .ok_or_else(|| Error::Mesh(format!("unknown patch '{pn}'")))?;
        for f in p.faces() {
            let ids = mesh.face_points(f);
            let n = unit(mesh.face_area_vectors()[f]);
            for k in 1..ids.len() - 1 {
                s.triangles.push([pts[ids[0]], pts[ids[k]], pts[ids[k + 1]]]);
                s.stored_normals.push(n);
            }
        }
    }
    if s.triangles.is_empty() {
        return Err(Error::Mesh("the selected patches have no faces".into()));
    }
    Ok(s)
}

/// Names of the boundary patches that are surfaces (all but empty/cyclic ones).
pub fn surface_patch_names(mesh: &Mesh) -> Vec<String> {
    mesh.patches()
        .iter()
        .filter(|p| !matches!(p.kind, rcfd_mesh::PatchKind::Empty | rcfd_mesh::PatchKind::Cyclic) && p.size > 0)
        .map(|p| p.name.clone())
        .collect()
}

/// ASCII STL text. Fails on non-finite coordinates.
pub fn write_ascii(solid: &StlSolid) -> Result<String> {
    check_sizes(solid)?;
    let name: String = solid
        .name
        .chars()
        .map(|c| if c.is_whitespace() || c.is_control() { '_' } else { c })
        .collect();
    let mut o = String::new();
    let _ = writeln!(o, "solid {name}");
    for (i, t) in solid.triangles.iter().enumerate() {
        let n = solid.stored_normals[i];
        for (k, v) in t.iter().chain(std::iter::once(&n)).enumerate() {
            for c in v {
                if !c.is_finite() {
                    return Err(Error::Io(format!(
                        "facet {} {}: non-finite number, STL cannot represent it",
                        i + 1,
                        if k == 3 { "normal" } else { "vertex" }
                    )));
                }
            }
        }
        let _ = writeln!(o, "  facet normal {:?} {:?} {:?}", n[0], n[1], n[2]);
        let _ = writeln!(o, "    outer loop");
        for v in t {
            let _ = writeln!(o, "      vertex {:?} {:?} {:?}", v[0], v[1], v[2]);
        }
        let _ = writeln!(o, "    endloop");
        let _ = writeln!(o, "  endfacet");
    }
    let _ = writeln!(o, "endsolid {name}");
    Ok(o)
}

fn check_sizes(solid: &StlSolid) -> Result<()> {
    if solid.stored_normals.len() != solid.triangles.len() {
        return Err(Error::Io(format!(
            "{} normals for {} triangles",
            solid.stored_normals.len(),
            solid.triangles.len()
        )));
    }
    Ok(())
}

/// Binary STL bytes (`f32`, attribute 0). The header never starts with `solid`.
pub fn write_binary(solid: &StlSolid) -> Result<Vec<u8>> {
    check_sizes(solid)?;
    if solid.triangles.len() > u32::MAX as usize {
        return Err(Error::Io("too many triangles for binary STL".into()));
    }
    let mut b = vec![0u8; 80];
    let tag = format!("rcfd binary STL {}", solid.name);
    for (d, s) in b
        .iter_mut()
        .zip(tag.bytes().filter(|c| c.is_ascii_graphic() || *c == b' '))
    {
        *d = s;
    }
    b.extend_from_slice(&(solid.triangles.len() as u32).to_le_bytes());
    for (i, t) in solid.triangles.iter().enumerate() {
        let n = solid.stored_normals[i];
        for v in std::iter::once(&n).chain(t.iter()) {
            for c in v {
                if !c.is_finite() {
                    return Err(Error::Io(format!(
                        "facet {}: non-finite number, STL cannot represent it",
                        i + 1
                    )));
                }
                b.extend_from_slice(&(*c as f32).to_le_bytes());
            }
        }
        b.extend_from_slice(&[0, 0]);
    }
    Ok(b)
}

/// Read an STL file (binary or ASCII, detected as described in ADR 0011 S6). A binary file gives one solid; an ASCII
/// file one per `solid` block.
pub fn read_bytes(bytes: &[u8]) -> Result<Vec<StlSolid>> {
    check_file_size(bytes.len(), "STL")?;
    if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if n.checked_mul(50).and_then(|x| x.checked_add(84)) == Some(bytes.len()) {
            return read_binary(bytes, n).map(|s| vec![s]);
        }
    }
    let head = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    if !bytes[head..].starts_with(b"solid") {
        if bytes.len() >= 84 {
            let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
            return Err(Error::Io(format!(
                "STL: byte offset 80: binary header declares {n} triangles (needs {} bytes) but the file has {} bytes",
                n.saturating_mul(50).saturating_add(84),
                bytes.len()
            )));
        }
        return Err(Error::Io(format!(
            "STL: byte offset 0: neither a binary file (only {} bytes) nor an ASCII file starting with 'solid'",
            bytes.len()
        )));
    }
    read_ascii(bytes)
}

fn read_binary(bytes: &[u8], n: usize) -> Result<StlSolid> {
    check_count(n, 50, bytes.len().saturating_sub(84), "STL")?;
    let header = &bytes[..80];
    let name: String = header
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| {
            if c.is_ascii_graphic() || c == b' ' {
                c as char
            } else {
                '?'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    let f = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) as f64;
    let mut s = StlSolid {
        name,
        triangles: Vec::with_capacity(n),
        stored_normals: Vec::with_capacity(n),
    };
    for i in 0..n {
        let o = 84 + 50 * i;
        let v3 = |k: usize| -> Result<Vec3> {
            let mut r = [0.0; 3];
            for (c, slot) in r.iter_mut().enumerate() {
                let off = o + 4 * (3 * k + c);
                *slot = finite(f(off), || format!("STL: byte offset {off}: facet {}", i + 1))?;
            }
            Ok(r)
        };
        s.stored_normals.push(v3(0)?);
        s.triangles.push([v3(1)?, v3(2)?, v3(3)?]);
    }
    Ok(s)
}

fn read_ascii(bytes: &[u8]) -> Result<Vec<StlSolid>> {
    let text = String::from_utf8_lossy(bytes);
    let err = |line: usize, msg: String| Error::Io(format!("STL: line {line}: {msg}"));
    let mut solids: Vec<StlSolid> = Vec::new();
    #[derive(PartialEq)]
    enum St {
        Outside,
        Solid,
        Facet,
        Loop,
        AfterLoop,
    }
    let mut st = St::Outside;
    let mut normal = [0.0; 3];
    let mut verts: Vec<Vec3> = Vec::new();
    for (ln, raw) in text.lines().enumerate() {
        let line = ln + 1;
        let mut tok = raw.split_ascii_whitespace();
        let Some(kw) = tok.next() else { continue };
        let kw = kw.to_ascii_lowercase();
        let rest: Vec<&str> = tok.collect();
        let num = |t: &str| -> Result<f64> {
            let v: f64 = t.parse().map_err(|_| err(line, format!("'{t}' is not a number")))?;
            finite(v, || format!("STL: line {line}"))
        };
        let vec3 = |r: &[&str]| -> Result<Vec3> {
            if r.len() != 3 {
                return Err(err(line, format!("expected 3 numbers, found {}", r.len())));
            }
            Ok([num(r[0])?, num(r[1])?, num(r[2])?])
        };
        match (kw.as_str(), &st) {
            ("solid", St::Outside) => {
                solids.push(StlSolid {
                    name: rest.join(" "),
                    triangles: Vec::new(),
                    stored_normals: Vec::new(),
                });
                st = St::Solid;
            }
            ("facet", St::Solid) => {
                if rest.first().map(|s| s.to_ascii_lowercase()).as_deref() != Some("normal") {
                    return Err(err(line, "'facet' must be followed by 'normal nx ny nz'".into()));
                }
                normal = vec3(&rest[1..])?;
                verts.clear();
                st = St::Facet;
            }
            ("outer", St::Facet) => {
                if rest.first().map(|s| s.to_ascii_lowercase()).as_deref() != Some("loop") {
                    return Err(err(line, "'outer' must be followed by 'loop'".into()));
                }
                st = St::Loop;
            }
            ("vertex", St::Loop) => {
                if verts.len() == 3 {
                    return Err(err(line, "a facet has more than 3 vertices".into()));
                }
                verts.push(vec3(&rest)?);
            }
            ("endloop", St::Loop) => {
                if verts.len() != 3 {
                    return Err(err(line, format!("a facet needs 3 vertices, found {}", verts.len())));
                }
                st = St::AfterLoop;
            }
            ("endfacet", St::AfterLoop) => {
                let s = solids.last_mut().expect("inside a solid");
                s.triangles.push([verts[0], verts[1], verts[2]]);
                s.stored_normals.push(normal);
                if s.triangles.len() > crate::guard::MAX_ITEMS {
                    return Err(err(line, "too many triangles".into()));
                }
                st = St::Solid;
            }
            ("endsolid", St::Solid) => st = St::Outside,
            (k, _) => {
                return Err(err(
                    line,
                    format!(
                        "unexpected '{k}' {}",
                        match st {
                            St::Outside => "outside a solid",
                            St::Solid => "(expected 'facet' or 'endsolid')",
                            St::Facet => "(expected 'outer loop')",
                            St::Loop => "(expected 'vertex' or 'endloop')",
                            St::AfterLoop => "(expected 'endfacet')",
                        }
                    ),
                ))
            }
        }
    }
    if st != St::Outside {
        return Err(err(
            text.lines().count() + 1,
            "end of file inside a solid (missing endsolid/endfacet/endloop)".into(),
        ));
    }
    if solids.is_empty() {
        return Err(err(1, "no solid found".into()));
    }
    Ok(solids)
}

/// Read an STL file from disk.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Vec<StlSolid>> {
    let path = path.as_ref();
    let meta = std::fs::metadata(path)?;
    check_file_size(meta.len() as usize, "STL")?;
    let b = std::fs::read(path)?;
    read_bytes(&b).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}

/// Triangle soup welded by exact (bitwise) vertex equality.
#[derive(Debug, Clone)]
pub struct Welded {
    /// Distinct vertices.
    pub points: Vec<Vec3>,
    /// Triangles as vertex indices.
    pub triangles: Vec<[usize; 3]>,
}

/// Weld equal vertices (`-0.0` equals `0.0`).
pub fn weld(solid: &StlSolid) -> Welded {
    let mut map: HashMap<[u64; 3], usize> = HashMap::new();
    let mut points = Vec::new();
    let mut triangles = Vec::with_capacity(solid.triangles.len());
    for t in &solid.triangles {
        let mut ids = [0usize; 3];
        for (k, v) in t.iter().enumerate() {
            let key = [(v[0] + 0.0).to_bits(), (v[1] + 0.0).to_bits(), (v[2] + 0.0).to_bits()];
            ids[k] = *map.entry(key).or_insert_with(|| {
                points.push(*v);
                points.len() - 1
            });
        }
        triangles.push(ids);
    }
    Welded { points, triangles }
}

/// Edge statistics of a welded surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeStats {
    /// Distinct undirected edges.
    pub edges: usize,
    /// Edges used by exactly one triangle.
    pub boundary: usize,
    /// Edges used by more than two triangles.
    pub non_manifold: usize,
    /// Edges that two triangles traverse in the same direction (inconsistent orientation).
    pub misoriented: usize,
}

impl Welded {
    /// Edge statistics.
    pub fn edge_stats(&self) -> EdgeStats {
        let mut m: HashMap<(usize, usize), (u32, u32)> = HashMap::new();
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let e = m.entry((a.min(b), a.max(b))).or_default();
                if a < b {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
        }
        let mut s = EdgeStats {
            edges: m.len(),
            boundary: 0,
            non_manifold: 0,
            misoriented: 0,
        };
        for (f, r) in m.values() {
            match f + r {
                1 => s.boundary += 1,
                2 => {
                    if *f != 1 {
                        s.misoriented += 1;
                    }
                }
                _ => s.non_manifold += 1,
            }
        }
        s
    }

    /// Volume enclosed by a closed, outward-oriented surface (divergence theorem).
    pub fn enclosed_volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| dot(self.points[t[0]], cross(self.points[t[1]], self.points[t[2]])) / 6.0)
            .sum()
    }

    /// Euler characteristic V - E + F.
    pub fn euler(&self) -> i64 {
        self.points.len() as i64 - self.edge_stats().edges as i64 + self.triangles.len() as i64
    }
}

/// Scale all vertices (unit conversion helper for callers).
pub fn scaled(solid: &StlSolid, s: f64) -> StlSolid {
    StlSolid {
        name: solid.name.clone(),
        triangles: solid
            .triangles
            .iter()
            .map(|t| [scale(t[0], s), scale(t[1], s), scale(t[2], s)])
            .collect(),
        stored_normals: solid.stored_normals.clone(),
    }
}
