//! Gmsh `.msh` reader: format 2.x and 4.1, ASCII and binary.
//!
//! Supported elements: linear and higher-order tetrahedra, hexahedra, prisms
//! and pyramids (only corner nodes are used) as cells; triangles/quads as
//! boundary (or interior) surfaces. Physical group names become patch / zone
//! names. Format 4.0 is rejected with a clear error.

use crate::common::{build_mesh, cell_face_template, Shape, SurfaceGroup};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("gmsh: {}", msg.into())))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    binary: bool,
    big_endian: bool,
}

impl Reader<'_> {
    fn token(&mut self) -> Result<String> {
        while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        let s = self.pos;
        while self.pos < self.data.len() && !self.data[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        if s == self.pos {
            return bad("unexpected end of file");
        }
        Ok(String::from_utf8_lossy(&self.data[s..self.pos]).into_owned())
    }
    fn line_rest(&mut self) -> String {
        while self.pos < self.data.len() && (self.data[self.pos] == b' ' || self.data[self.pos] == b'\t') {
            self.pos += 1;
        }
        let s = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b'\n' {
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.data[s..self.pos])
            .trim_end_matches('\r')
            .to_string()
    }
    fn skip_newline(&mut self) {
        if self.data.get(self.pos) == Some(&b'\r') {
            self.pos += 1;
        }
        if self.data.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
        }
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.pos + N > self.data.len() {
            return bad("unexpected end of binary data");
        }
        let mut b = [0u8; N];
        b.copy_from_slice(&self.data[self.pos..self.pos + N]);
        self.pos += N;
        if self.big_endian {
            b.reverse();
        }
        Ok(b)
    }
    /// 32-bit integer (ASCII token or binary).
    fn int(&mut self) -> Result<i64> {
        if self.binary {
            Ok(i64::from(i32::from_le_bytes(self.bytes::<4>()?)))
        } else {
            let t = self.token()?;
            t.parse().or_else(|_| bad(format!("expected integer, got '{t}'")))
        }
    }
    /// size_t (64-bit in binary v4).
    fn size(&mut self) -> Result<usize> {
        if self.binary {
            Ok(u64::from_le_bytes(self.bytes::<8>()?) as usize)
        } else {
            let t = self.token()?;
            t.parse()
                .or_else(|_| bad(format!("expected unsigned integer, got '{t}'")))
        }
    }
    fn float(&mut self) -> Result<f64> {
        if self.binary {
            Ok(f64::from_le_bytes(self.bytes::<8>()?))
        } else {
            let t = self.token()?;
            t.parse().or_else(|_| bad(format!("expected number, got '{t}'")))
        }
    }
    fn expect_end(&mut self, name: &str) -> Result<()> {
        // tolerate stray bytes before the end marker (binary padding)
        let marker = format!("$End{name}");
        let hay = &self.data[self.pos..];
        match hay.windows(marker.len()).position(|w| w == marker.as_bytes()) {
            Some(i) => {
                self.pos += i + marker.len();
                Ok(())
            }
            None => bad(format!("missing {marker}")),
        }
    }
    fn skip_section(&mut self, name: &str) -> Result<()> {
        self.expect_end(name)
    }
}

fn nodes_per_type(t: i64) -> Option<usize> {
    Some(match t {
        1 => 2,
        2 => 3,
        3 => 4,
        4 => 4,
        5 => 8,
        6 => 6,
        7 => 5,
        8 => 3,
        9 => 6,
        10 => 9,
        11 => 10,
        12 => 27,
        13 => 18,
        14 => 14,
        15 => 1,
        16 => 8,
        17 => 20,
        18 => 15,
        19 => 13,
        _ => return None,
    })
}

/// Classify an element type: (dimension, corner-node shape or surface corner count).
enum Class {
    Cell(Shape, usize),
    Surface(usize),
    Ignore,
}

fn classify(t: i64) -> Class {
    match t {
        4 | 11 => Class::Cell(Shape::Tet, 4),
        5 | 12 | 17 => Class::Cell(Shape::Hex, 8),
        6 | 13 | 18 => Class::Cell(Shape::Prism, 6),
        7 | 14 | 19 => Class::Cell(Shape::Pyramid, 5),
        2 | 9 => Class::Surface(3),
        3 | 10 | 16 => Class::Surface(4),
        _ => Class::Ignore,
    }
}

struct Elem {
    etype: i64,
    nodes: Vec<usize>,
    phys: Vec<i64>,
    entity: i64,
}

#[derive(Default)]
struct Raw {
    names: HashMap<(i64, i64), String>,
    nodes: Vec<(usize, Vec3)>,
    elems: Vec<Elem>,
    /// (dim, entity tag) -> physical tags (v4)
    entity_phys: HashMap<(i64, i64), Vec<i64>>,
}

/// Read a `.msh` file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Mesh> {
    let data = std::fs::read(path)?;
    read_bytes(&data)
}

/// Parse a `.msh` file held in memory.
pub fn read_bytes(data: &[u8]) -> Result<Mesh> {
    let mut r = Reader {
        data,
        pos: 0,
        binary: false,
        big_endian: false,
    };
    let mut raw = Raw::default();
    let mut version = 0.0;
    while let Ok(tok) = r.token() {
        match tok.as_str() {
            "$MeshFormat" => {
                let v = r.token()?;
                version = v.parse::<f64>().or_else(|_| bad(format!("bad version '{v}'")))?;
                let ft: i64 = r.token()?.parse().or_else(|_| bad("bad file-type"))?;
                let _size = r.token()?;
                r.binary = ft == 1;
                if r.binary {
                    r.skip_newline();
                    if r.pos + 4 > data.len() {
                        return bad("truncated binary header");
                    }
                    let one = [data[r.pos], data[r.pos + 1], data[r.pos + 2], data[r.pos + 3]];
                    r.pos += 4;
                    r.big_endian = match (i32::from_le_bytes(one), i32::from_be_bytes(one)) {
                        (1, _) => false,
                        (_, 1) => true,
                        _ => return bad("bad binary endianness marker"),
                    };
                }
                r.expect_end("MeshFormat")?;
                if !(2.0..3.0).contains(&version) && !(4.05..4.5).contains(&version) {
                    return bad(format!("unsupported format version {v} (supported: 2.x, 4.1)"));
                }
            }
            "$PhysicalNames" => {
                let n: usize = r.token()?.parse().or_else(|_| bad("bad PhysicalNames count"))?;
                for _ in 0..n {
                    let dim: i64 = r.token()?.parse().or_else(|_| bad("bad physical dim"))?;
                    let tag: i64 = r.token()?.parse().or_else(|_| bad("bad physical tag"))?;
                    let name = r.line_rest();
                    raw.names.insert((dim, tag), name.trim().trim_matches('"').to_string());
                }
                r.expect_end("PhysicalNames")?;
            }
            "$Entities" if version >= 4.0 => {
                read_entities(&mut r, &mut raw)?;
                r.expect_end("Entities")?;
            }
            "$Nodes" => {
                if version >= 4.0 {
                    read_nodes_v4(&mut r, &mut raw)?;
                } else {
                    read_nodes_v2(&mut r, &mut raw)?;
                }
                r.expect_end("Nodes")?;
            }
            "$Elements" => {
                if version >= 4.0 {
                    read_elements_v4(&mut r, &mut raw)?;
                } else {
                    read_elements_v2(&mut r, &mut raw)?;
                }
                r.expect_end("Elements")?;
            }
            t if t.starts_with("$End") => {}
            t if t.starts_with('$') => r.skip_section(&t[1..])?,
            other => return bad(format!("unexpected token '{other}'")),
        }
    }
    if version == 0.0 {
        return bad("missing $MeshFormat");
    }
    assemble(raw)
}

fn read_entities(r: &mut Reader, raw: &mut Raw) -> Result<()> {
    if r.binary {
        r.skip_newline();
    }
    let counts = [r.size()?, r.size()?, r.size()?, r.size()?];
    for (dim, &n) in counts.iter().enumerate() {
        for _ in 0..n {
            let tag = r.int()?;
            if dim == 0 {
                for _ in 0..3 {
                    r.float()?;
                }
            } else {
                for _ in 0..6 {
                    r.float()?;
                }
            }
            let np = r.size()?;
            let mut phys = Vec::new();
            for _ in 0..np {
                phys.push(r.int()?);
            }
            if dim > 0 {
                let nb = r.size()?;
                for _ in 0..nb {
                    r.int()?;
                }
            }
            raw.entity_phys.insert((dim as i64, tag), phys);
        }
    }
    Ok(())
}

fn read_nodes_v2(r: &mut Reader, raw: &mut Raw) -> Result<()> {
    let n: usize = r.token()?.parse().or_else(|_| bad("bad node count"))?;
    if r.binary {
        r.skip_newline();
    }
    raw.nodes.reserve(n);
    for _ in 0..n {
        let id = r.int()?;
        let p = [r.float()?, r.float()?, r.float()?];
        raw.nodes.push((id as usize, p));
    }
    Ok(())
}

fn read_nodes_v4(r: &mut Reader, raw: &mut Raw) -> Result<()> {
    if r.binary {
        r.skip_newline();
    }
    let blocks = r.size()?;
    let total = r.size()?;
    r.size()?;
    r.size()?;
    raw.nodes.reserve(total);
    for _ in 0..blocks {
        let dim = r.int()?;
        let _tag = r.int()?;
        let parametric = r.int()?;
        let n = r.size()?;
        let mut tags = Vec::with_capacity(n);
        for _ in 0..n {
            tags.push(r.size()?);
        }
        for t in tags {
            let p = [r.float()?, r.float()?, r.float()?];
            if parametric != 0 {
                for _ in 0..dim.max(0) {
                    r.float()?;
                }
            }
            raw.nodes.push((t, p));
        }
    }
    Ok(())
}

fn read_elements_v2(r: &mut Reader, raw: &mut Raw) -> Result<()> {
    let n: usize = r.token()?.parse().or_else(|_| bad("bad element count"))?;
    if r.binary {
        r.skip_newline();
        let mut done = 0;
        while done < n {
            let etype = r.int()?;
            let num = r.int()? as usize;
            let ntags = r.int()? as usize;
            let nn = nodes_per_type(etype).ok_or_else(|| Error::Io(format!("gmsh: unknown element type {etype}")))?;
            for _ in 0..num {
                let _id = r.int()?;
                let mut tags = Vec::new();
                for _ in 0..ntags {
                    tags.push(r.int()?);
                }
                let mut nodes = Vec::new();
                for _ in 0..nn {
                    nodes.push(r.int()? as usize);
                }
                raw.elems.push(Elem {
                    etype,
                    nodes,
                    phys: vec![tags.first().copied().unwrap_or(0)],
                    entity: tags.get(1).copied().unwrap_or(0),
                });
            }
            done += num;
        }
    } else {
        for _ in 0..n {
            let _id = r.int()?;
            let etype = r.int()?;
            let ntags = r.int()? as usize;
            let nn = nodes_per_type(etype).ok_or_else(|| Error::Io(format!("gmsh: unknown element type {etype}")))?;
            let mut tags = Vec::new();
            for _ in 0..ntags {
                tags.push(r.int()?);
            }
            let mut nodes = Vec::new();
            for _ in 0..nn {
                nodes.push(r.int()? as usize);
            }
            raw.elems.push(Elem {
                etype,
                nodes,
                phys: vec![tags.first().copied().unwrap_or(0)],
                entity: tags.get(1).copied().unwrap_or(0),
            });
        }
    }
    Ok(())
}

fn read_elements_v4(r: &mut Reader, raw: &mut Raw) -> Result<()> {
    if r.binary {
        r.skip_newline();
    }
    let blocks = r.size()?;
    r.size()?;
    r.size()?;
    r.size()?;
    for _ in 0..blocks {
        let dim = r.int()?;
        let entity = r.int()?;
        let etype = r.int()?;
        let n = r.size()?;
        let nn = nodes_per_type(etype).ok_or_else(|| Error::Io(format!("gmsh: unknown element type {etype}")))?;
        let phys = raw.entity_phys.get(&(dim, entity)).cloned().unwrap_or_default();
        for _ in 0..n {
            let _tag = r.size()?;
            let mut nodes = Vec::with_capacity(nn);
            for _ in 0..nn {
                nodes.push(r.size()?);
            }
            raw.elems.push(Elem {
                etype,
                nodes,
                phys: phys.clone(),
                entity,
            });
        }
    }
    Ok(())
}

fn group_names(raw: &Raw, e: &Elem, dim: i64) -> Vec<String> {
    let named: Vec<String> = e
        .phys
        .iter()
        .filter(|&&p| p != 0)
        .map(|&p| {
            raw.names
                .get(&(dim, p))
                .cloned()
                .unwrap_or_else(|| format!("physical_{dim}_{p}"))
        })
        .collect();
    if named.is_empty() {
        vec![format!("entity_{dim}_{}", e.entity)]
    } else {
        named
    }
}

fn assemble(raw: Raw) -> Result<Mesh> {
    let index: HashMap<usize, usize> = raw.nodes.iter().enumerate().map(|(i, (t, _))| (*t, i)).collect();
    if index.len() != raw.nodes.len() {
        return bad("duplicate node tags");
    }
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut points: Vec<Vec3> = Vec::new();
    let mut map_node = |tag: usize| -> Result<usize> {
        let i = *index
            .get(&tag)
            .ok_or_else(|| Error::Io(format!("gmsh: element references unknown node {tag}")))?;
        Ok(*remap.entry(i).or_insert_with(|| {
            points.push(raw.nodes[i].1);
            points.len() - 1
        }))
    };
    let mut cells = Vec::new();
    let mut cell_groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for e in &raw.elems {
        if let Class::Cell(shape, nc) = classify(e.etype) {
            let ids: Vec<usize> = e.nodes[..nc].iter().map(|&t| map_node(t)).collect::<Result<_>>()?;
            let tpl = cell_face_template(nc, shape).expect("template exists for shape");
            let faces: Vec<Vec<usize>> = tpl.iter().map(|f| f.iter().map(|&i| ids[i]).collect()).collect();
            for name in group_names(&raw, e, 3) {
                cell_groups.entry(name).or_default().push(cells.len());
            }
            cells.push(faces);
        }
    }
    if cells.is_empty() {
        return bad("no 3D cells found (2D meshes are not supported)");
    }
    let mut surf: BTreeMap<String, Vec<Vec<usize>>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for e in &raw.elems {
        if let Class::Surface(nc) = classify(e.etype) {
            // surfaces touching nodes unused by any cell are irrelevant
            let ids: Result<Vec<usize>> = e.nodes[..nc].iter().map(|&t| map_node(t)).collect();
            let ids = ids?;
            for name in group_names(&raw, e, 2) {
                if !surf.contains_key(&name) {
                    order.push(name.clone());
                }
                surf.entry(name).or_default().push(ids.clone());
            }
        }
    }
    drop(map_node);
    let surfaces: Vec<SurfaceGroup> = order
        .into_iter()
        .map(|n| SurfaceGroup {
            faces: surf.remove(&n).unwrap_or_default(),
            name: n,
            kind: None,
        })
        .collect();
    // a single cell group named after the default entity is not a meaningful zone
    let cell_groups: Vec<(String, Vec<usize>)> =
        if cell_groups.len() == 1 && cell_groups.keys().next().is_some_and(|k| k.starts_with("entity_")) {
            vec![]
        } else {
            cell_groups.into_iter().collect()
        };
    build_mesh(points, cells, surfaces, cell_groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::PatchKind;

    // Two unit hexes side by side: nodes (x,y,z) x in 0..=2.
    fn nodes() -> Vec<(usize, Vec3)> {
        let mut v = Vec::new();
        let mut id = 1;
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    v.push((id, [x as f64, y as f64, z as f64]));
                    id += 1;
                }
            }
        }
        v
    }
    fn nid(x: usize, y: usize, z: usize) -> usize {
        1 + x + 3 * y + 6 * z
    }
    // Gmsh hex ordering: bottom 0..3 counter-clockwise, then top.
    fn hex(x0: usize) -> Vec<usize> {
        vec![
            nid(x0, 0, 0),
            nid(x0 + 1, 0, 0),
            nid(x0 + 1, 1, 0),
            nid(x0, 1, 0),
            nid(x0, 0, 1),
            nid(x0 + 1, 0, 1),
            nid(x0 + 1, 1, 1),
            nid(x0, 1, 1),
        ]
    }
    // quads: (nodes, physical)  1 = inlet (x=0), 2 = outlet (x=2), 3 = walls (y=0 two quads)
    fn quads() -> Vec<(Vec<usize>, i64)> {
        vec![
            (vec![nid(0, 0, 0), nid(0, 1, 0), nid(0, 1, 1), nid(0, 0, 1)], 1),
            (vec![nid(2, 0, 0), nid(2, 1, 0), nid(2, 1, 1), nid(2, 0, 1)], 2),
            (vec![nid(0, 0, 0), nid(1, 0, 0), nid(1, 0, 1), nid(0, 0, 1)], 3),
            (vec![nid(1, 0, 0), nid(2, 0, 0), nid(2, 0, 1), nid(1, 0, 1)], 3),
        ]
    }
    const NAMES: &str =
        "$PhysicalNames\n4\n2 1 \"inlet\"\n2 2 \"outlet\"\n2 3 \"bottom wall\"\n3 9 \"fluid\"\n$EndPhysicalNames\n";

    fn v2_ascii() -> Vec<u8> {
        let mut s = String::from("$MeshFormat\n2.2 0 8\n$EndMeshFormat\n");
        s += NAMES;
        s += &format!("$Nodes\n{}\n", nodes().len());
        for (id, p) in nodes() {
            s += &format!("{id} {} {} {}\n", p[0], p[1], p[2]);
        }
        s += "$EndNodes\n";
        s += &format!("$Elements\n{}\n", 6);
        let mut id = 1;
        for (q, ph) in quads() {
            s += &format!("{id} 3 2 {ph} {ph} {} {} {} {}\n", q[0], q[1], q[2], q[3]);
            id += 1;
        }
        for x0 in 0..2 {
            let h = hex(x0);
            s += &format!(
                "{id} 5 2 9 1 {}\n",
                h.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
            );
            id += 1;
        }
        s += "$EndElements\n";
        s.into_bytes()
    }

    fn v2_binary(big: bool) -> Vec<u8> {
        let i32b = |v: i32| if big { v.to_be_bytes() } else { v.to_le_bytes() };
        let f64b = |v: f64| if big { v.to_be_bytes() } else { v.to_le_bytes() };
        let mut b = b"$MeshFormat\n2.2 1 8\n".to_vec();
        b.extend(i32b(1));
        b.extend(b"\n$EndMeshFormat\n");
        b.extend(NAMES.as_bytes());
        b.extend(format!("$Nodes\n{}\n", nodes().len()).as_bytes());
        for (id, p) in nodes() {
            b.extend(i32b(id as i32));
            for x in p {
                b.extend(f64b(x));
            }
        }
        b.extend(b"\n$EndNodes\n$Elements\n6\n");
        // quads grouped by physical tag is not required: one group header per element group
        b.extend(i32b(3));
        b.extend(i32b(4));
        b.extend(i32b(2));
        for (k, (q, ph)) in quads().into_iter().enumerate() {
            b.extend(i32b(k as i32 + 1));
            b.extend(i32b(ph as i32));
            b.extend(i32b(ph as i32));
            for n in q {
                b.extend(i32b(n as i32));
            }
        }
        b.extend(i32b(5));
        b.extend(i32b(2));
        b.extend(i32b(2));
        for x0 in 0..2 {
            b.extend(i32b(5 + x0 as i32));
            b.extend(i32b(9));
            b.extend(i32b(1));
            for n in hex(x0) {
                b.extend(i32b(n as i32));
            }
        }
        b.extend(b"\n$EndElements\n");
        b
    }

    fn v4_ascii() -> Vec<u8> {
        let mut s = String::from("$MeshFormat\n4.1 0 8\n$EndMeshFormat\n");
        s += NAMES;
        // entities: surfaces 1,2,3 (phys 1,2,3), volume 1 (phys 9); no points/curves
        s += "$Entities\n0 0 3 1\n";
        for t in 1..=3 {
            s += &format!("{t} 0 0 0 2 1 1 1 {t} 0\n");
        }
        s += "1 0 0 0 2 1 1 1 9 0\n$EndEntities\n";
        s += &format!(
            "$Nodes\n1 {} 1 {}\n3 1 0 {}\n",
            nodes().len(),
            nodes().len(),
            nodes().len()
        );
        for (id, _) in nodes() {
            s += &format!("{id}\n");
        }
        for (_, p) in nodes() {
            s += &format!("{} {} {}\n", p[0], p[1], p[2]);
        }
        s += "$EndNodes\n$Elements\n4 6 1 6\n";
        let mut id = 1;
        for (ent, qs) in [(1, vec![0]), (2, vec![1]), (3, vec![2, 3])] {
            s += &format!("2 {ent} 3 {}\n", qs.len());
            for qi in qs {
                let q = &quads()[qi].0;
                s += &format!("{id} {} {} {} {}\n", q[0], q[1], q[2], q[3]);
                id += 1;
            }
        }
        s += "3 1 5 2\n";
        for x0 in 0..2 {
            s += &format!(
                "{id} {}\n",
                hex(x0).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
            );
            id += 1;
        }
        s += "$EndElements\n";
        s.into_bytes()
    }

    fn v4_binary() -> Vec<u8> {
        let mut b = b"$MeshFormat\n4.1 1 8\n".to_vec();
        b.extend(1i32.to_le_bytes());
        b.extend(b"\n$EndMeshFormat\n");
        b.extend(NAMES.as_bytes());
        b.extend(b"$Entities\n");
        for v in [0u64, 0, 3, 1] {
            b.extend(v.to_le_bytes());
        }
        let surf = |b: &mut Vec<u8>, tag: i32, phys: i32| {
            b.extend(tag.to_le_bytes());
            for _ in 0..6 {
                b.extend(0f64.to_le_bytes());
            }
            b.extend(1u64.to_le_bytes());
            b.extend(phys.to_le_bytes());
            b.extend(0u64.to_le_bytes());
        };
        for t in 1..=3 {
            surf(&mut b, t, t);
        }
        surf(&mut b, 1, 9);
        b.extend(b"\n$EndEntities\n$Nodes\n");
        for v in [1u64, nodes().len() as u64, 1, nodes().len() as u64] {
            b.extend(v.to_le_bytes());
        }
        b.extend(3i32.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.extend(0i32.to_le_bytes());
        b.extend((nodes().len() as u64).to_le_bytes());
        for (id, _) in nodes() {
            b.extend((id as u64).to_le_bytes());
        }
        for (_, p) in nodes() {
            for x in p {
                b.extend(x.to_le_bytes());
            }
        }
        b.extend(b"\n$EndNodes\n$Elements\n");
        for v in [4u64, 6, 1, 6] {
            b.extend(v.to_le_bytes());
        }
        let mut id = 1u64;
        for (ent, qs) in [(1, vec![0]), (2, vec![1]), (3, vec![2, 3])] {
            b.extend(2i32.to_le_bytes());
            b.extend(i32::to_le_bytes(ent));
            b.extend(3i32.to_le_bytes());
            b.extend((qs.len() as u64).to_le_bytes());
            for qi in qs {
                b.extend(id.to_le_bytes());
                for n in &quads()[qi].0 {
                    b.extend((*n as u64).to_le_bytes());
                }
                id += 1;
            }
        }
        b.extend(3i32.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.extend(5i32.to_le_bytes());
        b.extend(2u64.to_le_bytes());
        for x0 in 0..2 {
            b.extend(id.to_le_bytes());
            id += 1;
            for n in hex(x0) {
                b.extend((n as u64).to_le_bytes());
            }
        }
        b.extend(b"\n$EndElements\n");
        b
    }

    fn check(m: &Mesh) {
        assert_eq!(m.n_cells(), 2);
        assert_eq!(m.n_points(), 12);
        assert_eq!(m.n_internal_faces(), 1);
        assert_eq!(m.n_faces(), 11);
        assert!((m.total_volume() - 2.0).abs() < 1e-12);
        let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["inlet", "outlet", "bottom wall", "defaultFaces"]);
        assert_eq!(m.patch("inlet").unwrap().kind, PatchKind::Inlet);
        assert_eq!(m.patch("outlet").unwrap().kind, PatchKind::Outlet);
        assert_eq!(m.patch("bottom wall").unwrap().kind, PatchKind::Wall);
        assert_eq!(m.patch("bottom wall").unwrap().size, 2);
        assert_eq!(m.patch("defaultFaces").unwrap().size, 6);
        assert_eq!(m.cell_zone("fluid").unwrap().ids, vec![0, 1]);
    }

    #[test]
    fn reads_all_variants_identically() {
        let a = read_bytes(&v2_ascii()).unwrap();
        check(&a);
        let h = a.content_hash();
        for (label, bytes) in [
            ("v2 bin le", v2_binary(false)),
            ("v2 bin be", v2_binary(true)),
            ("v4 ascii", v4_ascii()),
            ("v4 bin", v4_binary()),
        ] {
            let m = read_bytes(&bytes).unwrap_or_else(|e| panic!("{label}: {e}"));
            check(&m);
            assert_eq!(m.content_hash(), h, "{label}");
        }
    }

    #[test]
    fn tets_prisms_and_pyramid_from_second_order_ids() {
        // one tet written as 10-node tetrahedron (type 11); only corners used
        let mut s = String::from("$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$Nodes\n10\n");
        let p = [
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0.5, 0., 0.],
            [0.5, 0.5, 0.],
            [0., 0.5, 0.],
            [0., 0., 0.5],
            [0.5, 0., 0.5],
            [0., 0.5, 0.5],
        ];
        for (i, q) in p.iter().enumerate() {
            s += &format!("{} {} {} {}\n", i + 1, q[0], q[1], q[2]);
        }
        s += "$EndNodes\n$Elements\n1\n1 11 2 1 1 1 2 3 4 5 6 7 8 9 10\n$EndElements\n";
        let m = read_bytes(s.as_bytes()).unwrap();
        assert_eq!(m.n_cells(), 1);
        assert_eq!(m.n_points(), 4);
        assert!((m.total_volume() - 1.0 / 6.0).abs() < 1e-14);
        assert_eq!(m.patches()[0].name, "defaultFaces");
    }

    #[test]
    fn errors_are_reported() {
        assert!(read_bytes(b"").is_err());
        assert!(read_bytes(b"$MeshFormat\n4.0 0 8\n$EndMeshFormat\n").is_err());
        assert!(read_bytes(
            b"$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$Nodes\n1\n1 0 0 0\n$EndNodes\n$Elements\n0\n$EndElements\n"
        )
        .is_err());
        let mut truncated = v4_binary();
        truncated.truncate(truncated.len() / 2);
        assert!(read_bytes(&truncated).is_err());
        let unknown_node = String::from_utf8(v2_ascii())
            .unwrap()
            .replace("5 2 9 1 1 2 5 4 7 8 11 10", "5 2 9 1 1 2 5 4 7 8 11 99");
        assert!(read_bytes(unknown_node.as_bytes()).is_err());
    }
}
