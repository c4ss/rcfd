//! Tecplot ASCII data files (F-119, ADR 0011 section 2.2).
//!
//! Written from the public description of the Tecplot ASCII data format: `TITLE`, `VARIABLES`, then `ZONE` records
//! with `ZONETYPE` ORDERED / FETRIANGLE / FEQUADRILATERAL / FETETRAHEDRON / FEBRICK, `NODES`, `ELEMENTS` (`I`, `J`,
//! `K` for ordered zones), `DATAPACKING` BLOCK or POINT and `VARLOCATION` cell-centred lists, followed by the data
//! and, for finite-element zones, the 1-based connectivity. Numbers are written with the shortest text that reads
//! back to the same `f64`, so a round trip of mesh and field is bit exact.
//!
//! Wedges and pyramids are written as degenerate bricks (repeated nodes), the standard Tecplot convention, and are
//! recognised again when a mesh is rebuilt. Polyhedra are refused.

use crate::common::{build_mesh, cell_face_template, Shape};
use crate::guard::{check_count, check_file_size, MAX_BLOCKS};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::collections::HashMap;
use std::fmt::Write as _;

/// Zone type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneKind {
    /// Structured `I x J x K` points (no connectivity).
    Ordered {
        /// Points along I.
        i: usize,
        /// Points along J.
        j: usize,
        /// Points along K.
        k: usize,
    },
    /// Triangles (3 nodes per element).
    Triangle,
    /// Quadrilaterals (4 nodes).
    Quadrilateral,
    /// Tetrahedra (4 nodes).
    Tetrahedron,
    /// Bricks (8 nodes, possibly degenerate).
    Brick,
}

impl ZoneKind {
    /// Nodes per element (0 for ordered zones).
    pub fn nodes_per_element(&self) -> usize {
        match self {
            ZoneKind::Ordered { .. } => 0,
            ZoneKind::Triangle => 3,
            ZoneKind::Quadrilateral | ZoneKind::Tetrahedron => 4,
            ZoneKind::Brick => 8,
        }
    }

    fn keyword(&self) -> &'static str {
        match self {
            ZoneKind::Ordered { .. } => "ORDERED",
            ZoneKind::Triangle => "FETRIANGLE",
            ZoneKind::Quadrilateral => "FEQUADRILATERAL",
            ZoneKind::Tetrahedron => "FETETRAHEDRON",
            ZoneKind::Brick => "FEBRICK",
        }
    }
}

/// Where a variable lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// One value per node.
    Nodal,
    /// One value per element (cell).
    CellCentered,
}

/// Data layout in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    /// All values of variable 1, then variable 2, ...
    Block,
    /// All variables of node 1, then node 2, ... (nodal variables only)
    Point,
}

/// One zone.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Zone title.
    pub title: String,
    /// Zone type.
    pub kind: ZoneKind,
    /// Number of nodes (points).
    pub n_nodes: usize,
    /// Number of elements (cells); for ordered zones `(I-1) * max(J-1,1) * max(K-1,1)`.
    pub n_elems: usize,
    /// Location of each variable.
    pub locations: Vec<Location>,
    /// Values per variable (`n_nodes` or `n_elems` each).
    pub data: Vec<Vec<f64>>,
    /// Zero-based connectivity, `nodes_per_element` entries per element (empty for ordered zones).
    pub connectivity: Vec<usize>,
}

/// A Tecplot data set.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    /// Title.
    pub title: String,
    /// Variable names.
    pub variables: Vec<String>,
    /// Zones.
    pub zones: Vec<Zone>,
}

fn quote(s: &str) -> String {
    s.chars()
        .map(|c| if c == '"' || c.is_control() { '_' } else { c })
        .collect()
}

/// Write ASCII Tecplot text. `packing = Point` requires all variables nodal.
pub fn write(ds: &Dataset, packing: Packing) -> Result<String> {
    let mut o = String::new();
    let _ = writeln!(o, "TITLE = \"{}\"", quote(&ds.title));
    let names: Vec<String> = ds.variables.iter().map(|v| format!("\"{}\"", quote(v))).collect();
    let _ = writeln!(o, "VARIABLES = {}", names.join(", "));
    for (zi, z) in ds.zones.iter().enumerate() {
        let bad = |m: String| Error::Io(format!("Tecplot write: zone {}: {m}", zi + 1));
        if z.locations.len() != ds.variables.len() || z.data.len() != ds.variables.len() {
            return Err(bad("locations/data do not match the variable list".into()));
        }
        for (v, d) in z.data.iter().enumerate() {
            let want = if z.locations[v] == Location::Nodal {
                z.n_nodes
            } else {
                z.n_elems
            };
            if d.len() != want {
                return Err(bad(format!(
                    "variable {} has {} values, expected {want}",
                    v + 1,
                    d.len()
                )));
            }
            if let Some(i) = d.iter().position(|x| !x.is_finite()) {
                return Err(bad(format!("variable {} value {} is not finite", v + 1, i + 1)));
            }
        }
        if packing == Packing::Point && z.locations.contains(&Location::CellCentered) {
            return Err(bad("POINT packing cannot hold cell-centred variables".into()));
        }
        let cc: Vec<String> = z
            .locations
            .iter()
            .enumerate()
            .filter(|(_, l)| **l == Location::CellCentered)
            .map(|(i, _)| (i + 1).to_string())
            .collect();
        let dp = if packing == Packing::Block { "BLOCK" } else { "POINT" };
        match z.kind {
            ZoneKind::Ordered { i, j, k } => {
                let _ = write!(
                    o,
                    "ZONE T=\"{}\", I={i}, J={j}, K={k}, ZONETYPE=ORDERED, DATAPACKING={dp}",
                    quote(&z.title)
                );
            }
            kind => {
                let np = kind.nodes_per_element();
                if z.connectivity.len() != z.n_elems * np {
                    return Err(bad("connectivity length does not match the element count".into()));
                }
                if let Some(p) = z.connectivity.iter().position(|&n| n >= z.n_nodes) {
                    return Err(bad(format!(
                        "connectivity entry {} refers to a node beyond {}",
                        p + 1,
                        z.n_nodes
                    )));
                }
                let _ = write!(
                    o,
                    "ZONE T=\"{}\", NODES={}, ELEMENTS={}, ZONETYPE={}, DATAPACKING={dp}",
                    quote(&z.title),
                    z.n_nodes,
                    z.n_elems,
                    kind.keyword()
                );
            }
        }
        if !cc.is_empty() {
            let _ = write!(o, ", VARLOCATION=([{}]=CELLCENTERED)", cc.join(","));
        }
        o.push('\n');
        match packing {
            Packing::Block => {
                for d in &z.data {
                    for (i, x) in d.iter().enumerate() {
                        let _ = write!(o, "{x:?}{}", if (i + 1) % 5 == 0 { '\n' } else { ' ' });
                    }
                    if d.len() % 5 != 0 {
                        o.push('\n');
                    }
                }
            }
            Packing::Point => {
                for n in 0..z.n_nodes {
                    let row: Vec<String> = z.data.iter().map(|d| format!("{:?}", d[n])).collect();
                    let _ = writeln!(o, "{}", row.join(" "));
                }
            }
        }
        let np = z.kind.nodes_per_element();
        for e in 0..z.n_elems {
            if np == 0 {
                break;
            }
            let row: Vec<String> = z.connectivity[e * np..(e + 1) * np]
                .iter()
                .map(|n| (n + 1).to_string())
                .collect();
            let _ = writeln!(o, "{}", row.join(" "));
        }
    }
    Ok(o)
}

fn outward_loop(mesh: &Mesh, f: usize, c: usize) -> Vec<usize> {
    let mut ids = mesh.face_points(f).to_vec();
    if f < mesh.n_internal_faces() && mesh.neighbour()[f] == c {
        ids.reverse();
    }
    ids
}

/// Node list of cell `c` in the (Gmsh/VTK/Tecplot) order of its shape, derived from the faces.
pub fn ordered_cell_nodes(mesh: &Mesh, c: usize) -> Result<(Shape, Vec<usize>)> {
    let loops: Vec<Vec<usize>> = mesh.cell_faces(c).iter().map(|&f| outward_loop(mesh, f, c)).collect();
    let mut nodes: Vec<usize> = loops.iter().flatten().copied().collect();
    nodes.sort_unstable();
    nodes.dedup();
    let tris = loops.iter().filter(|l| l.len() == 3).count();
    let quads = loops.iter().filter(|l| l.len() == 4).count();
    let nf = loops.len();
    let fail = || {
        Error::Io(format!(
            "cell {c} ({nf} faces, {} nodes) is not a tetrahedron, hexahedron, prism or pyramid; Tecplot FE zones cannot hold it",
            nodes.len()
        ))
    };
    let reversed = |l: &Vec<usize>| -> Vec<usize> { l.iter().rev().copied().collect() };
    // neighbours of a node along face edges
    let adj = |n: usize| -> Vec<usize> {
        let mut a = Vec::new();
        for l in &loops {
            if let Some(p) = l.iter().position(|&x| x == n) {
                for q in [l[(p + 1) % l.len()], l[(p + l.len() - 1) % l.len()]] {
                    if !a.contains(&q) {
                        a.push(q);
                    }
                }
            }
        }
        a
    };
    let lift = |base: &Vec<usize>| -> Option<Vec<usize>> {
        let mut top = Vec::new();
        for &b in base {
            let up: Vec<usize> = adj(b).into_iter().filter(|x| !base.contains(x)).collect();
            if up.len() != 1 {
                return None;
            }
            top.push(up[0]);
        }
        Some(top)
    };
    match (nf, tris, quads, nodes.len()) {
        (4, 4, 0, 4) => {
            let l = &loops[0];
            let apex = *nodes.iter().find(|n| !l.contains(n)).ok_or_else(fail)?;
            let mut v = reversed(l);
            v.push(apex);
            Ok((Shape::Tet, v))
        }
        (6, 0, 6, 8) => {
            let base = reversed(&loops[0]);
            let top = lift(&base).ok_or_else(fail)?;
            let mut v = base;
            v.extend(top);
            Ok((Shape::Hex, v))
        }
        (5, 2, 3, 6) => {
            let l = loops.iter().find(|l| l.len() == 3).ok_or_else(fail)?;
            let base = reversed(l);
            let top = lift(&base).ok_or_else(fail)?;
            let mut v = base;
            v.extend(top);
            Ok((Shape::Prism, v))
        }
        (5, 4, 1, 5) => {
            let l = loops.iter().find(|l| l.len() == 4).ok_or_else(fail)?;
            let apex = *nodes.iter().find(|n| !l.contains(n)).ok_or_else(fail)?;
            let mut v = reversed(l);
            v.push(apex);
            Ok((Shape::Pyramid, v))
        }
        _ => Err(fail()),
    }
}

/// Data set with one volume zone of the mesh: nodal X, Y, Z followed by cell-centred scalar fields.
/// All-tetrahedral meshes use FETETRAHEDRON, everything else FEBRICK (degenerate for wedges and pyramids).
pub fn dataset_from_mesh(mesh: &Mesh, title: &str, fields: &[(&str, &[f64])]) -> Result<Dataset> {
    for (n, f) in fields {
        if f.len() != mesh.n_cells() {
            return Err(Error::Io(format!(
                "field '{n}' has {} values, the mesh has {} cells",
                f.len(),
                mesh.n_cells()
            )));
        }
    }
    let mut cells = Vec::with_capacity(mesh.n_cells());
    for c in 0..mesh.n_cells() {
        cells.push(ordered_cell_nodes(mesh, c)?);
    }
    let all_tet = cells.iter().all(|(s, _)| *s == Shape::Tet);
    let kind = if all_tet {
        ZoneKind::Tetrahedron
    } else {
        ZoneKind::Brick
    };
    let mut conn = Vec::new();
    for (s, v) in &cells {
        match (kind, s) {
            (ZoneKind::Tetrahedron, _) | (_, Shape::Hex) => conn.extend_from_slice(v),
            (_, Shape::Tet) => conn.extend_from_slice(&[v[0], v[1], v[2], v[2], v[3], v[3], v[3], v[3]]),
            (_, Shape::Prism) => conn.extend_from_slice(&[v[0], v[1], v[2], v[2], v[3], v[4], v[5], v[5]]),
            (_, Shape::Pyramid) => conn.extend_from_slice(&[v[0], v[1], v[2], v[3], v[4], v[4], v[4], v[4]]),
        }
    }
    let pts = mesh.points();
    let mut variables = vec!["X".to_string(), "Y".to_string(), "Z".to_string()];
    let mut data: Vec<Vec<f64>> = (0..3).map(|k| pts.iter().map(|p| p[k]).collect()).collect();
    let mut locations = vec![Location::Nodal; 3];
    for (n, f) in fields {
        variables.push((*n).to_string());
        data.push(f.to_vec());
        locations.push(Location::CellCentered);
    }
    Ok(Dataset {
        title: title.to_string(),
        variables,
        zones: vec![Zone {
            title: "volume".into(),
            kind,
            n_nodes: pts.len(),
            n_elems: mesh.n_cells(),
            locations,
            data,
            connectivity: conn,
        }],
    })
}

/// Data set with the boundary faces of the named patches as one FETRIANGLE / FEQUADRILATERAL zone (faces with more
/// than 4 nodes are refused). Face values are written cell-centred.
pub fn dataset_from_patches(
    mesh: &Mesh,
    title: &str,
    patches: &[&str],
    face_fields: &[(&str, &[f64])],
) -> Result<Dataset> {
    let mut faces: Vec<usize> = Vec::new();
    for pn in patches {
        let p = mesh
            .patch(pn)
            .ok_or_else(|| Error::Mesh(format!("unknown patch '{pn}'")))?;
        faces.extend(p.faces());
    }
    for (n, f) in face_fields {
        if f.len() != mesh.n_faces() {
            return Err(Error::Io(format!(
                "face field '{n}' has {} values, the mesh has {} faces",
                f.len(),
                mesh.n_faces()
            )));
        }
    }
    if faces.is_empty() {
        return Err(Error::Mesh("the selected patches have no faces".into()));
    }
    let max = faces.iter().map(|&f| mesh.face_points(f).len()).max().unwrap_or(0);
    if max > 4 {
        return Err(Error::Io(format!(
            "a boundary face has {max} nodes; Tecplot FE surface zones hold triangles or quadrilaterals"
        )));
    }
    let kind = if max == 3 {
        ZoneKind::Triangle
    } else {
        ZoneKind::Quadrilateral
    };
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut pts: Vec<Vec3> = Vec::new();
    let mut conn = Vec::new();
    for &f in &faces {
        let ids = mesh.face_points(f);
        let mut row: Vec<usize> = ids
            .iter()
            .map(|&p| {
                *remap.entry(p).or_insert_with(|| {
                    pts.push(mesh.points()[p]);
                    pts.len() - 1
                })
            })
            .collect();
        if kind == ZoneKind::Quadrilateral && row.len() == 3 {
            row.push(row[2]);
        }
        conn.extend(row);
    }
    let mut variables = vec!["X".to_string(), "Y".to_string(), "Z".to_string()];
    let mut data: Vec<Vec<f64>> = (0..3).map(|k| pts.iter().map(|p| p[k]).collect()).collect();
    let mut locations = vec![Location::Nodal; 3];
    for (n, f) in face_fields {
        variables.push((*n).to_string());
        data.push(faces.iter().map(|&i| f[i]).collect());
        locations.push(Location::CellCentered);
    }
    Ok(Dataset {
        title: title.to_string(),
        variables,
        zones: vec![Zone {
            title: patches.join("+"),
            kind,
            n_nodes: pts.len(),
            n_elems: faces.len(),
            locations,
            data,
            connectivity: conn,
        }],
    })
}

/// Rebuild a mesh from a tetrahedron or brick zone whose first three variables are X, Y, Z (nodal). Degenerate
/// bricks of the standard wedge and pyramid patterns are recognised; other degenerate bricks are errors.
pub fn mesh_from_zone(ds: &Dataset, zone: usize) -> Result<Mesh> {
    let z = ds
        .zones
        .get(zone)
        .ok_or_else(|| Error::Io(format!("Tecplot: no zone {}", zone + 1)))?;
    if ds.variables.len() < 3 || z.locations.iter().take(3).any(|l| *l != Location::Nodal) {
        return Err(Error::Io(
            "Tecplot: the first three variables must be nodal X, Y, Z".into(),
        ));
    }
    let np = z.kind.nodes_per_element();
    if !matches!(z.kind, ZoneKind::Tetrahedron | ZoneKind::Brick) {
        return Err(Error::Io(
            "Tecplot: only FETETRAHEDRON and FEBRICK zones become volume meshes".into(),
        ));
    }
    let points: Vec<Vec3> = (0..z.n_nodes)
        .map(|i| [z.data[0][i], z.data[1][i], z.data[2][i]])
        .collect();
    let mut cells = Vec::with_capacity(z.n_elems);
    for e in 0..z.n_elems {
        let v = &z.connectivity[e * np..(e + 1) * np];
        let bad = || {
            Error::Io(format!(
                "Tecplot: element {} is a degenerate brick of no supported shape",
                e + 1
            ))
        };
        let (shape, nodes): (Shape, Vec<usize>) = if np == 4 {
            (Shape::Tet, v.to_vec())
        } else if v[2] == v[3] && v[4] == v[5] && v[5] == v[6] && v[6] == v[7] && v[1] != v[2] {
            // tetrahedron (a b c c d d d d)
            (Shape::Tet, vec![v[0], v[1], v[2], v[4]])
        } else if v[4] == v[5] && v[5] == v[6] && v[6] == v[7] {
            // pyramid (a b c d e e e e)
            (Shape::Pyramid, vec![v[0], v[1], v[2], v[3], v[4]])
        } else if v[2] == v[3] && v[6] == v[7] {
            (Shape::Prism, vec![v[0], v[1], v[2], v[4], v[5], v[6]])
        } else {
            let mut d = v.to_vec();
            d.sort_unstable();
            d.dedup();
            if d.len() != 8 {
                return Err(bad());
            }
            (Shape::Hex, v.to_vec())
        };
        let tpl = cell_face_template(nodes.len(), shape).ok_or_else(bad)?;
        cells.push(
            tpl.iter()
                .map(|f| f.iter().map(|&i| nodes[i]).collect::<Vec<usize>>())
                .collect(),
        );
    }
    build_mesh(points, cells, Vec::new(), Vec::new())
}

// ---------------------------------------------------------------------------------------------------------------------
// reader

struct Attr {
    key: String,
    value: String,
}

/// `KEY=VALUE` pairs of a header text (values may be quoted or parenthesised).
fn attributes(text: &str, line: usize) -> Result<Vec<Attr>> {
    let c: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let skip = |i: &mut usize| {
        while *i < c.len() && (c[*i].is_whitespace() || c[*i] == ',') {
            *i += 1;
        }
    };
    loop {
        skip(&mut i);
        if i >= c.len() {
            break;
        }
        let s = i;
        while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
            i += 1;
        }
        if i == s {
            return Err(Error::Io(format!(
                "Tecplot: line {line}: unexpected '{}' in a header",
                c[i]
            )));
        }
        let key: String = c[s..i].iter().collect::<String>().to_ascii_uppercase();
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        if i >= c.len() || c[i] != '=' {
            return Err(Error::Io(format!(
                "Tecplot: line {line}: '{key}' is not followed by '='"
            )));
        }
        i += 1;
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        let value: String;
        if i < c.len() && c[i] == '"' {
            let s = i + 1;
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += 1;
            }
            if i >= c.len() {
                return Err(Error::Io(format!(
                    "Tecplot: line {line}: unterminated quote in '{key}'"
                )));
            }
            value = c[s..i].iter().collect();
            i += 1;
        } else if i < c.len() && c[i] == '(' {
            let s = i;
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '(' {
                    depth += 1;
                } else if c[i] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            if i >= c.len() {
                return Err(Error::Io(format!(
                    "Tecplot: line {line}: unbalanced parenthesis in '{key}'"
                )));
            }
            i += 1;
            value = c[s..i].iter().collect();
        } else {
            let s = i;
            while i < c.len() && !c[i].is_whitespace() && c[i] != ',' {
                i += 1;
            }
            value = c[s..i].iter().collect();
        }
        out.push(Attr { key, value });
    }
    Ok(out)
}

fn parse_usize(a: &Attr, line: usize) -> Result<usize> {
    a.value
        .trim()
        .parse::<usize>()
        .map_err(|_| Error::Io(format!("Tecplot: line {line}: {}={} is not a count", a.key, a.value)))
}

/// Indices (1-based) listed in `([4,5-6]=CELLCENTERED)`.
fn cell_centered_list(v: &str, line: usize) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    let t = v.trim().trim_start_matches('(').trim_end_matches(')');
    // groups: [a,b-c]=NODAL|CELLCENTERED separated by commas outside brackets
    let mut rest = t;
    while let Some(open) = rest.find('[') {
        let close = rest[open..]
            .find(']')
            .ok_or_else(|| Error::Io(format!("Tecplot: line {line}: VARLOCATION: missing ']'")))?
            + open;
        let list = &rest[open + 1..close];
        let after = rest[close + 1..].trim_start();
        let after = after
            .strip_prefix('=')
            .ok_or_else(|| Error::Io(format!("Tecplot: line {line}: VARLOCATION: missing '='")))?
            .trim_start();
        let end = after.find([',', ')']).unwrap_or(after.len());
        let loc = after[..end].trim().to_ascii_uppercase();
        rest = &after[end..];
        let cc = match loc.as_str() {
            "CELLCENTERED" => true,
            "NODAL" => false,
            o => {
                return Err(Error::Io(format!(
                    "Tecplot: line {line}: VARLOCATION: unknown location '{o}'"
                )))
            }
        };
        if cc {
            for part in list.split(',') {
                let part = part.trim();
                let (a, b) = match part.split_once('-') {
                    Some((a, b)) => (a, b),
                    None => (part, part),
                };
                let pa: usize = a
                    .trim()
                    .parse()
                    .map_err(|_| Error::Io(format!("Tecplot: line {line}: VARLOCATION: bad index '{part}'")))?;
                let pb: usize = b
                    .trim()
                    .parse()
                    .map_err(|_| Error::Io(format!("Tecplot: line {line}: VARLOCATION: bad index '{part}'")))?;
                if pa == 0 || pb < pa || pb > 4096 {
                    return Err(Error::Io(format!(
                        "Tecplot: line {line}: VARLOCATION: bad range '{part}'"
                    )));
                }
                out.extend(pa..=pb);
            }
        }
    }
    Ok(out)
}

fn parse_number(t: &str, line: usize) -> Result<f64> {
    let v: f64 = match t.parse() {
        Ok(v) => v,
        Err(_) => t
            .replace(['d', 'D'], "e")
            .parse()
            .map_err(|_| Error::Io(format!("Tecplot: line {line}: '{t}' is not a number")))?,
    };
    if !v.is_finite() {
        return Err(Error::Io(format!("Tecplot: line {line}: non-finite number '{t}'")));
    }
    Ok(v)
}

fn is_data_line(l: &str) -> bool {
    matches!(l.trim_start().chars().next(), Some(c) if c.is_ascii_digit() || c == '-' || c == '+' || c == '.')
}

/// Read a Tecplot ASCII file.
pub fn read(bytes: &[u8]) -> Result<Dataset> {
    check_file_size(bytes.len(), "Tecplot")?;
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let mut ds = Dataset {
        title: String::new(),
        variables: Vec::new(),
        zones: Vec::new(),
    };
    let mut ln = 0;
    let err = |line: usize, m: String| Error::Io(format!("Tecplot: line {line}: {m}"));
    while ln < lines.len() {
        let l = lines[ln].trim();
        if l.is_empty() || l.starts_with('#') {
            ln += 1;
            continue;
        }
        let upper = l.to_ascii_uppercase();
        if upper.starts_with("TITLE") {
            for a in attributes(l, ln + 1)? {
                if a.key == "TITLE" {
                    ds.title = a.value;
                }
            }
            ln += 1;
        } else if upper.starts_with("VARIABLES") {
            let mut h = l.to_string();
            let start = ln + 1;
            ln += 1;
            // variable names may continue on following lines, each quoted
            while ln < lines.len() && lines[ln].trim_start().starts_with('"') {
                h.push(' ');
                h.push_str(lines[ln]);
                ln += 1;
            }
            let body = h.split_once('=').map(|x| x.1).unwrap_or("");
            let mut names = Vec::new();
            let mut in_q = false;
            let mut cur = String::new();
            for ch in body.chars() {
                if ch == '"' {
                    if in_q {
                        names.push(std::mem::take(&mut cur));
                    }
                    in_q = !in_q;
                } else if in_q {
                    cur.push(ch);
                }
            }
            if in_q {
                return Err(err(start, "unterminated quote in VARIABLES".into()));
            }
            if names.is_empty() {
                // unquoted names
                names = body
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
            }
            if names.is_empty() || names.len() > 4096 {
                return Err(err(start, "VARIABLES needs between 1 and 4096 names".into()));
            }
            ds.variables = names;
        } else if upper.starts_with("ZONE") {
            if ds.variables.is_empty() {
                return Err(err(ln + 1, "ZONE before VARIABLES".into()));
            }
            if ds.zones.len() >= MAX_BLOCKS {
                return Err(err(ln + 1, format!("more than {MAX_BLOCKS} zones")));
            }
            let zline = ln + 1;
            let mut h = l[4..].to_string();
            ln += 1;
            while ln < lines.len() && !lines[ln].trim().is_empty() && !is_data_line(lines[ln]) {
                let t = lines[ln].trim_start().to_ascii_uppercase();
                if t.starts_with("ZONE") {
                    break;
                }
                h.push(' ');
                h.push_str(lines[ln]);
                ln += 1;
            }
            let attrs = attributes(&h, zline)?;
            let (mut title, mut zt, mut packing) = (String::new(), String::from("ORDERED"), String::from("BLOCK"));
            let (mut nodes, mut elems, mut ii, mut jj, mut kk) = (None, None, 1usize, 1usize, 1usize);
            let mut cc: Vec<usize> = Vec::new();
            let mut old_et: Option<String> = None;
            for a in &attrs {
                match a.key.as_str() {
                    "T" => title = a.value.clone(),
                    "ZONETYPE" => zt = a.value.to_ascii_uppercase(),
                    "DATAPACKING" => packing = a.value.to_ascii_uppercase(),
                    "NODES" | "N" => nodes = Some(parse_usize(a, zline)?),
                    "ELEMENTS" | "E" => elems = Some(parse_usize(a, zline)?),
                    "I" => ii = parse_usize(a, zline)?,
                    "J" => jj = parse_usize(a, zline)?,
                    "K" => kk = parse_usize(a, zline)?,
                    "VARLOCATION" => cc = cell_centered_list(&a.value, zline)?,
                    "F" => {
                        let v = a.value.to_ascii_uppercase();
                        packing = match v.as_str() {
                            "FEPOINT" | "POINT" => "POINT".into(),
                            "FEBLOCK" | "BLOCK" => "BLOCK".into(),
                            o => return Err(err(zline, format!("unsupported F={o}"))),
                        };
                        if v.starts_with("FE") {
                            zt = "FE".into();
                        }
                    }
                    "ET" => old_et = Some(a.value.to_ascii_uppercase()),
                    "DT" | "STRANDID" | "SOLUTIONTIME" | "AUXDATA" | "PARENTZONE" => {}
                    "VARSHARELIST"
                    | "CONNECTIVITYSHAREZONE"
                    | "PASSIVEVARLIST"
                    | "FACENEIGHBORMODE"
                    | "TOTALNUMFACENODES"
                    | "NUMCONNECTEDBOUNDARYFACES"
                    | "TOTALNUMBOUNDARYCONNECTIONS" => {
                        return Err(err(zline, format!("{} is not supported", a.key)));
                    }
                    _ => {}
                }
            }
            if zt == "FE" {
                zt = match old_et.as_deref() {
                    Some("TRIANGLE") => "FETRIANGLE",
                    Some("QUADRILATERAL") => "FEQUADRILATERAL",
                    Some("TETRAHEDRON") => "FETETRAHEDRON",
                    Some("BRICK") => "FEBRICK",
                    _ => return Err(err(zline, "old-style FE zone needs ET=".into())),
                }
                .into();
            }
            let kind = match zt.as_str() {
                "ORDERED" => ZoneKind::Ordered { i: ii, j: jj, k: kk },
                "FETRIANGLE" => ZoneKind::Triangle,
                "FEQUADRILATERAL" => ZoneKind::Quadrilateral,
                "FETETRAHEDRON" => ZoneKind::Tetrahedron,
                "FEBRICK" => ZoneKind::Brick,
                o => return Err(err(zline, format!("unsupported ZONETYPE={o}"))),
            };
            let (n_nodes, n_elems) = match kind {
                ZoneKind::Ordered { i, j, k } => {
                    if i == 0 || j == 0 || k == 0 {
                        return Err(err(zline, "I, J and K must be at least 1".into()));
                    }
                    let n = i
                        .checked_mul(j)
                        .and_then(|x| x.checked_mul(k))
                        .ok_or_else(|| err(zline, "I*J*K overflows".into()))?;
                    let e = (i - 1).max(1) * (j - 1).max(1) * (k - 1).max(1);
                    (n, if i * j * k == 1 { 0 } else { e })
                }
                _ => (
                    nodes.ok_or_else(|| err(zline, "NODES missing".into()))?,
                    elems.ok_or_else(|| err(zline, "ELEMENTS missing".into()))?,
                ),
            };
            let nv = ds.variables.len();
            let mut locations = vec![Location::Nodal; nv];
            for &i in &cc {
                if i > nv {
                    return Err(err(zline, format!("VARLOCATION names variable {i}, only {nv} exist")));
                }
                locations[i - 1] = Location::CellCentered;
            }
            if packing == "POINT" && !cc.is_empty() {
                return Err(err(zline, "POINT packing cannot hold cell-centred variables".into()));
            }
            if packing != "POINT" && packing != "BLOCK" {
                return Err(err(zline, format!("unknown DATAPACKING={packing}")));
            }
            let np = kind.nodes_per_element();
            let per_var: Vec<usize> = locations
                .iter()
                .map(|l| if *l == Location::Nodal { n_nodes } else { n_elems })
                .collect();
            let n_values: usize = per_var.iter().sum();
            let n_conn = n_elems
                .checked_mul(np)
                .ok_or_else(|| err(zline, "ELEMENTS overflows".into()))?;
            let remaining = bytes.len();
            check_count(n_values, 2, remaining, &format!("Tecplot: line {zline}: zone data"))?;
            check_count(n_conn, 2, remaining, &format!("Tecplot: line {zline}: connectivity"))?;
            // data tokens
            let mut vals: Vec<f64> = Vec::with_capacity(n_values);
            let mut conn: Vec<usize> = Vec::with_capacity(n_conn);
            'outer: while ln < lines.len() && (vals.len() < n_values || conn.len() < n_conn) {
                let line = ln + 1;
                for t in lines[ln].split_ascii_whitespace() {
                    if vals.len() < n_values {
                        vals.push(parse_number(t, line)?);
                    } else if conn.len() < n_conn {
                        let n: usize = t
                            .parse()
                            .map_err(|_| err(line, format!("'{t}' is not a node number")))?;
                        if n == 0 || n > n_nodes {
                            return Err(err(line, format!("node number {n} outside 1..={n_nodes}")));
                        }
                        conn.push(n - 1);
                    } else {
                        return Err(err(line, "more data than the zone header declares".into()));
                    }
                }
                ln += 1;
                if vals.len() >= n_values && conn.len() >= n_conn {
                    break 'outer;
                }
            }
            if vals.len() < n_values || conn.len() < n_conn {
                return Err(err(
                    lines.len().max(1),
                    format!(
                        "zone starting at line {zline} is short: expected {n_values} values and {n_conn} connectivity entries, found {} and {}",
                        vals.len(),
                        conn.len()
                    ),
                ));
            }
            let mut data = Vec::with_capacity(nv);
            if packing == "BLOCK" {
                let mut o = 0;
                for &n in &per_var {
                    data.push(vals[o..o + n].to_vec());
                    o += n;
                }
            } else {
                for v in 0..nv {
                    data.push((0..n_nodes).map(|n| vals[n * nv + v]).collect());
                }
            }
            ds.zones.push(Zone {
                title,
                kind,
                n_nodes,
                n_elems,
                locations,
                data,
                connectivity: conn,
            });
        } else if upper.starts_with("DATASETAUXDATA") || upper.starts_with("TEXT") || upper.starts_with("GEOMETRY") {
            ln += 1;
        } else {
            return Err(err(
                ln + 1,
                format!("unexpected text '{}'", l.chars().take(40).collect::<String>()),
            ));
        }
    }
    if ds.variables.is_empty() {
        return Err(err(1, "no VARIABLES record".into()));
    }
    Ok(ds)
}

/// Read a Tecplot ASCII file from disk.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Dataset> {
    let path = path.as_ref();
    check_file_size(std::fs::metadata(path)?.len() as usize, "Tecplot")?;
    read(&std::fs::read(path)?).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}
