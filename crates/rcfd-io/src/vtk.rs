//! VTK output (F-070): XML `.vtu` (volume, as polyhedral cells), `.vtp`
//! (boundary patches), `.pvtu` (partitioned), `.pvd` (time series) and legacy
//! `.vtk` files. Everything is ASCII with shortest round-trip floating-point
//! formatting, so values read back are bit-identical to the solver data.

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::fmt::Write as _;
use std::path::Path;

/// Values of one output field.
#[derive(Debug, Clone, Copy)]
pub enum VtkValues<'a> {
    /// One scalar per cell (or per patch face).
    Scalar(&'a [f64]),
    /// One 3-vector per cell (or per patch face).
    Vector(&'a [Vec3]),
}

impl VtkValues<'_> {
    fn len(&self) -> usize {
        match self {
            VtkValues::Scalar(v) => v.len(),
            VtkValues::Vector(v) => v.len(),
        }
    }

    fn components(&self) -> usize {
        match self {
            VtkValues::Scalar(_) => 1,
            VtkValues::Vector(_) => 3,
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            VtkValues::Scalar(v) => {
                for x in v.iter() {
                    let _ = write!(out, "{x:?} ");
                }
            }
            VtkValues::Vector(v) => {
                for x in v.iter() {
                    let _ = write!(out, "{:?} {:?} {:?} ", x[0], x[1], x[2]);
                }
            }
        }
        out.push('\n');
    }
}

/// A named output field.
#[derive(Debug, Clone, Copy)]
pub struct VtkField<'a> {
    /// Array name shown in ParaView.
    pub name: &'a str,
    /// Values.
    pub values: VtkValues<'a>,
}

fn check_fields(fields: &[VtkField], n: usize) -> Result<()> {
    for f in fields {
        if f.values.len() != n {
            return Err(Error::InvalidValue {
                path: f.name.to_string(),
                reason: format!("field has {} entries, expected {n}", f.values.len()),
            });
        }
    }
    Ok(())
}

fn data_array(out: &mut String, ty: &str, name: &str, comps: usize, body: impl FnOnce(&mut String)) {
    let _ = writeln!(
        out,
        "<DataArray type=\"{ty}\" Name=\"{name}\" NumberOfComponents=\"{comps}\" format=\"ascii\">"
    );
    body(out);
    out.push_str("</DataArray>\n");
}

/// Outward-oriented point loop of face `f` as seen from cell `c`.
fn oriented_face(mesh: &Mesh, f: usize, c: usize) -> Vec<usize> {
    let mut ids = mesh.face_points(f).to_vec();
    if f < mesh.n_internal_faces() && mesh.neighbour()[f] == c {
        ids.reverse();
    }
    ids
}

/// Distinct points of a cell, sorted.
fn cell_points(mesh: &Mesh, c: usize) -> Vec<usize> {
    let mut p: Vec<usize> = mesh
        .cell_faces(c)
        .iter()
        .flat_map(|&f| mesh.face_points(f).iter().copied())
        .collect();
    p.sort_unstable();
    p.dedup();
    p
}

/// VTK face stream of a polyhedral cell: `[n_faces, n_pts, ids..., ...]`.
fn face_stream(mesh: &Mesh, c: usize) -> Vec<usize> {
    let faces = mesh.cell_faces(c);
    let mut s = vec![faces.len()];
    for &f in faces {
        let ids = oriented_face(mesh, f, c);
        s.push(ids.len());
        s.extend(ids);
    }
    s
}

fn points_block(out: &mut String, mesh: &Mesh) {
    out.push_str("<Points>\n");
    data_array(out, "Float64", "Points", 3, |o| {
        for p in mesh.points() {
            let _ = write!(o, "{:?} {:?} {:?} ", p[0], p[1], p[2]);
        }
        o.push('\n');
    });
    out.push_str("</Points>\n");
}

fn cell_data_block(out: &mut String, fields: &[VtkField]) {
    if fields.is_empty() {
        return;
    }
    out.push_str("<CellData>\n");
    for f in fields {
        data_array(out, "Float64", f.name, f.values.components(), |o| f.values.write(o));
    }
    out.push_str("</CellData>\n");
}

/// XML unstructured grid (`.vtu`) of the whole mesh with cell fields.
/// Cells are written as `VTK_POLYHEDRON`, so any mesh topology is supported.
pub fn vtu_string(mesh: &Mesh, fields: &[VtkField]) -> Result<String> {
    check_fields(fields, mesh.n_cells())?;
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\"?>\n<VTKFile type=\"UnstructuredGrid\" version=\"1.0\" byte_order=\"LittleEndian\">\n<UnstructuredGrid>\n");
    let _ = writeln!(
        o,
        "<Piece NumberOfPoints=\"{}\" NumberOfCells=\"{}\">",
        mesh.n_points(),
        mesh.n_cells()
    );
    points_block(&mut o, mesh);
    o.push_str("<Cells>\n");
    let (mut conn, mut offsets, mut faces, mut face_offsets) =
        (String::new(), String::new(), String::new(), String::new());
    let (mut n_conn, mut n_faces) = (0usize, 0usize);
    for c in 0..mesh.n_cells() {
        let pts = cell_points(mesh, c);
        n_conn += pts.len();
        for p in &pts {
            let _ = write!(conn, "{p} ");
        }
        let _ = write!(offsets, "{n_conn} ");
        let stream = face_stream(mesh, c);
        n_faces += stream.len();
        for v in &stream {
            let _ = write!(faces, "{v} ");
        }
        let _ = write!(face_offsets, "{n_faces} ");
    }
    data_array(&mut o, "Int64", "connectivity", 1, |s| {
        s.push_str(&conn);
        s.push('\n');
    });
    data_array(&mut o, "Int64", "offsets", 1, |s| {
        s.push_str(&offsets);
        s.push('\n');
    });
    data_array(&mut o, "UInt8", "types", 1, |s| {
        for _ in 0..mesh.n_cells() {
            s.push_str("42 ");
        }
        s.push('\n');
    });
    data_array(&mut o, "Int64", "faces", 1, |s| {
        s.push_str(&faces);
        s.push('\n');
    });
    data_array(&mut o, "Int64", "faceoffsets", 1, |s| {
        s.push_str(&face_offsets);
        s.push('\n');
    });
    o.push_str("</Cells>\n");
    cell_data_block(&mut o, fields);
    o.push_str("</Piece>\n</UnstructuredGrid>\n</VTKFile>\n");
    Ok(o)
}

/// Write a `.vtu` file.
pub fn write_vtu(path: impl AsRef<Path>, mesh: &Mesh, fields: &[VtkField]) -> Result<()> {
    std::fs::write(path, vtu_string(mesh, fields)?).map_err(|e| Error::Io(e.to_string()))
}

/// XML polygonal data (`.vtp`) of one boundary patch; `fields` have one
/// value per face of the patch.
pub fn vtp_string(mesh: &Mesh, patch: &str, fields: &[VtkField]) -> Result<String> {
    let p = mesh
        .patch(patch)
        .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
    check_fields(fields, p.size)?;
    // compact point numbering
    let mut used: Vec<usize> = p.faces().flat_map(|f| mesh.face_points(f).iter().copied()).collect();
    used.sort_unstable();
    used.dedup();
    let local = |g: usize| used.binary_search(&g).expect("point of patch face");
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\"?>\n<VTKFile type=\"PolyData\" version=\"1.0\" byte_order=\"LittleEndian\">\n<PolyData>\n");
    let _ = writeln!(o, "<Piece NumberOfPoints=\"{}\" NumberOfVerts=\"0\" NumberOfLines=\"0\" NumberOfStrips=\"0\" NumberOfPolys=\"{}\">", used.len(), p.size);
    o.push_str("<Points>\n");
    data_array(&mut o, "Float64", "Points", 3, |s| {
        for &g in &used {
            let q = mesh.points()[g];
            let _ = write!(s, "{:?} {:?} {:?} ", q[0], q[1], q[2]);
        }
        s.push('\n');
    });
    o.push_str("</Points>\n<Polys>\n");
    let (mut conn, mut offs) = (String::new(), String::new());
    let mut total = 0;
    for f in p.faces() {
        for &g in mesh.face_points(f) {
            let _ = write!(conn, "{} ", local(g));
        }
        total += mesh.face_points(f).len();
        let _ = write!(offs, "{total} ");
    }
    data_array(&mut o, "Int64", "connectivity", 1, |s| {
        s.push_str(&conn);
        s.push('\n');
    });
    data_array(&mut o, "Int64", "offsets", 1, |s| {
        s.push_str(&offs);
        s.push('\n');
    });
    o.push_str("</Polys>\n");
    cell_data_block(&mut o, fields);
    o.push_str("</Piece>\n</PolyData>\n</VTKFile>\n");
    Ok(o)
}

/// Write a `.vtp` file for one patch.
pub fn write_vtp(path: impl AsRef<Path>, mesh: &Mesh, patch: &str, fields: &[VtkField]) -> Result<()> {
    std::fs::write(path, vtp_string(mesh, patch, fields)?).map_err(|e| Error::Io(e.to_string()))
}

/// Parallel `.pvtu` header referencing piece files (relative paths).
/// `arrays` lists `(name, components)` of the cell arrays present in every piece.
pub fn pvtu_string(pieces: &[String], arrays: &[(String, usize)]) -> String {
    let mut o = String::from("<?xml version=\"1.0\"?>\n<VTKFile type=\"PUnstructuredGrid\" version=\"1.0\" byte_order=\"LittleEndian\">\n<PUnstructuredGrid GhostLevel=\"0\">\n");
    o.push_str("<PPoints>\n<PDataArray type=\"Float64\" Name=\"Points\" NumberOfComponents=\"3\"/>\n</PPoints>\n");
    if !arrays.is_empty() {
        o.push_str("<PCellData>\n");
        for (n, c) in arrays {
            let _ = writeln!(
                o,
                "<PDataArray type=\"Float64\" Name=\"{n}\" NumberOfComponents=\"{c}\"/>"
            );
        }
        o.push_str("</PCellData>\n");
    }
    for p in pieces {
        let _ = writeln!(o, "<Piece Source=\"{p}\"/>");
    }
    o.push_str("</PUnstructuredGrid>\n</VTKFile>\n");
    o
}

/// ParaView time-series collection (`.pvd`).
#[derive(Debug, Clone, Default)]
pub struct PvdSeries {
    entries: Vec<(f64, String)>,
}

impl PvdSeries {
    /// Empty series.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a data file (path relative to the `.pvd`) at time `t`.
    pub fn add(&mut self, t: f64, file: &str) {
        self.entries.push((t, file.to_string()));
    }

    /// Render the collection.
    pub fn to_xml(&self) -> String {
        let mut o = String::from("<?xml version=\"1.0\"?>\n<VTKFile type=\"Collection\" version=\"0.1\" byte_order=\"LittleEndian\">\n<Collection>\n");
        for (t, f) in &self.entries {
            let _ = writeln!(o, "<DataSet timestep=\"{t:?}\" group=\"\" part=\"0\" file=\"{f}\"/>");
        }
        o.push_str("</Collection>\n</VTKFile>\n");
        o
    }

    /// Write to disk.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path, self.to_xml()).map_err(|e| Error::Io(e.to_string()))
    }
}

/// Legacy ASCII `.vtk` (version 4.2) unstructured grid with polyhedral cells.
pub fn legacy_vtk_string(mesh: &Mesh, fields: &[VtkField]) -> Result<String> {
    check_fields(fields, mesh.n_cells())?;
    let mut o = String::from("# vtk DataFile Version 4.2\nrcfd volume mesh\nASCII\nDATASET UNSTRUCTURED_GRID\n");
    let _ = writeln!(o, "POINTS {} double", mesh.n_points());
    for p in mesh.points() {
        let _ = writeln!(o, "{:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    let streams: Vec<Vec<usize>> = (0..mesh.n_cells()).map(|c| face_stream(mesh, c)).collect();
    let total: usize = streams.iter().map(|s| s.len() + 1).sum();
    let _ = writeln!(o, "CELLS {} {}", mesh.n_cells(), total);
    for s in &streams {
        let _ = write!(o, "{}", s.len());
        for v in s {
            let _ = write!(o, " {v}");
        }
        o.push('\n');
    }
    let _ = writeln!(o, "CELL_TYPES {}", mesh.n_cells());
    for _ in 0..mesh.n_cells() {
        o.push_str("42\n");
    }
    if !fields.is_empty() {
        let _ = writeln!(o, "CELL_DATA {}", mesh.n_cells());
        for f in fields {
            match f.values {
                VtkValues::Scalar(v) => {
                    let _ = writeln!(o, "SCALARS {} double 1\nLOOKUP_TABLE default", f.name);
                    for x in v {
                        let _ = writeln!(o, "{x:?}");
                    }
                }
                VtkValues::Vector(v) => {
                    let _ = writeln!(o, "VECTORS {} double", f.name);
                    for x in v {
                        let _ = writeln!(o, "{:?} {:?} {:?}", x[0], x[1], x[2]);
                    }
                }
            }
        }
    }
    Ok(o)
}

// ------------------------------------------------------------------------------------------- finite-element cells (F-377)

/// VTK cell type of the linear tetrahedron (4 nodes; the right-hand normal of the base `(0,1,2)` points to node 3).
pub const VTK_TETRA: u8 = 10;
/// VTK cell type of the linear hexahedron (8 nodes; base `(0,1,2,3)` with its right-hand normal towards `(4,5,6,7)`, edges
/// `(0,4), (1,5), (2,6), (3,7)`).
pub const VTK_HEXAHEDRON: u8 = 12;
/// VTK cell type of the quadratic tetrahedron (10 nodes; corners 0 to 3, mid-edge nodes 4 to 9 on `(0,1), (1,2), (2,0),
/// (0,3), (1,3), (2,3)`).
pub const VTK_QUADRATIC_TETRA: u8 = 24;

/// VTK cell type of the two-node line (beam elements, ADR 0291).
pub const VTK_LINE: u8 = 3;
/// VTK cell type of the four-node quadrilateral (shell elements, ADR 0291).
pub const VTK_QUAD: u8 = 9;
/// VTK cell type of the quadratic hexahedron (20 nodes; corners 0 to 7 as [`VTK_HEXAHEDRON`], mid-edge nodes 8 to 19 on
/// `(0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7)`; ADR 0291).
pub const VTK_QUADRATIC_HEXAHEDRON: u8 = 25;

/// Number of nodes of a supported finite-element cell type (`None` for any other type).
pub fn vtk_cell_nodes(cell_type: u8) -> Option<usize> {
    match cell_type {
        VTK_TETRA => Some(4),
        VTK_HEXAHEDRON => Some(8),
        VTK_QUADRATIC_TETRA => Some(10),
        VTK_LINE => Some(2),
        VTK_QUAD => Some(4),
        VTK_QUADRATIC_HEXAHEDRON => Some(20),
        _ => None,
    }
}

/// Cells of one type, their nodes flat in VTK's node order.
#[derive(Debug, Clone, Copy)]
pub struct FeCellBlock<'a> {
    /// [`VTK_TETRA`], [`VTK_HEXAHEDRON`] or [`VTK_QUADRATIC_TETRA`].
    pub cell_type: u8,
    /// Node indices, `vtk_cell_nodes(cell_type)` per cell.
    pub connectivity: &'a [usize],
}

/// A named array of point or cell data: `components` values per point or cell, flat and row-major.
#[derive(Debug, Clone, Copy)]
pub struct FeArray<'a> {
    /// Array name (no `<`, `>`, `&`, `"`, `'`).
    pub name: &'a str,
    /// Values per point or cell (at least 1).
    pub components: usize,
    /// The values, `components * count`.
    pub values: &'a [f64],
}

fn fe_invalid(path: &str, reason: String) -> Error {
    Error::InvalidValue {
        path: path.to_string(),
        reason,
    }
}

fn check_fe_arrays(section: &str, arrays: &[FeArray], n: usize) -> Result<()> {
    for (i, a) in arrays.iter().enumerate() {
        if a.name.is_empty() {
            return Err(fe_invalid("name", format!("{section} array {i} has an empty name")));
        }
        if a.name.chars().any(|c| matches!(c, '<' | '>' | '&' | '"' | '\'')) {
            return Err(fe_invalid(
                a.name,
                format!(
                    "{section} array name '{}' contains a character not allowed in XML",
                    a.name
                ),
            ));
        }
        if arrays[..i].iter().any(|b| b.name == a.name) {
            return Err(fe_invalid(
                a.name,
                format!("{section} array '{}' is given twice", a.name),
            ));
        }
        if a.components == 0 {
            return Err(fe_invalid(
                a.name,
                format!("{section} array '{}' has 0 components", a.name),
            ));
        }
        if a.values.len() != a.components * n {
            return Err(fe_invalid(
                a.name,
                format!(
                    "{section} array '{}' has {} values, expected {} ({} components x {n})",
                    a.name,
                    a.values.len(),
                    a.components * n,
                    a.components
                ),
            ));
        }
    }
    Ok(())
}

fn fe_data_block(out: &mut String, tag: &str, arrays: &[FeArray]) {
    if arrays.is_empty() {
        return;
    }
    let _ = writeln!(out, "<{tag}>");
    for a in arrays {
        data_array(out, "Float64", a.name, a.components, |o| {
            for x in a.values {
                let _ = write!(o, "{x:?} ");
            }
            o.push('\n');
        });
    }
    let _ = writeln!(out, "</{tag}>");
}

/// XML unstructured grid (`.vtu`) of finite-element cells from plain arrays: `points`, cell blocks in VTK node order,
/// point data (one row per point) and cell data (one row per cell, cells numbered block by block). ASCII with the shortest
/// round-trip formatting, as [`vtu_string`]. Refuses an unknown cell type, a connectivity that is not a whole number of
/// cells or names a missing point, and an array of the wrong length (named by the array), with no components, or with an
/// empty, repeated or non-XML name.
pub fn fe_vtu_string(
    points: &[Vec3],
    blocks: &[FeCellBlock],
    point_data: &[FeArray],
    cell_data: &[FeArray],
) -> Result<String> {
    let mut n_cells = 0usize;
    for (b, block) in blocks.iter().enumerate() {
        let npc = vtk_cell_nodes(block.cell_type).ok_or_else(|| {
            fe_invalid(
                "cell_type",
                format!("block {b}: cell type {} is not 3, 9, 10, 12, 24 or 25", block.cell_type),
            )
        })?;
        if block.connectivity.is_empty() || block.connectivity.len() % npc != 0 {
            return Err(fe_invalid(
                "connectivity",
                format!(
                    "block {b}: {} indices is not a positive multiple of {npc} nodes per cell of type {}",
                    block.connectivity.len(),
                    block.cell_type
                ),
            ));
        }
        for (k, &p) in block.connectivity.iter().enumerate() {
            if p >= points.len() {
                return Err(fe_invalid(
                    "connectivity",
                    format!(
                        "block {b}: cell {} names point {p}, the grid has {} points",
                        k / npc,
                        points.len()
                    ),
                ));
            }
        }
        n_cells += block.connectivity.len() / npc;
    }
    check_fe_arrays("point", point_data, points.len())?;
    check_fe_arrays("cell", cell_data, n_cells)?;
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\"?>\n<VTKFile type=\"UnstructuredGrid\" version=\"1.0\" byte_order=\"LittleEndian\">\n<UnstructuredGrid>\n");
    let _ = writeln!(
        o,
        "<Piece NumberOfPoints=\"{}\" NumberOfCells=\"{n_cells}\">",
        points.len()
    );
    fe_data_block(&mut o, "PointData", point_data);
    fe_data_block(&mut o, "CellData", cell_data);
    o.push_str("<Points>\n");
    data_array(&mut o, "Float64", "Points", 3, |s| {
        for p in points {
            let _ = write!(s, "{:?} {:?} {:?} ", p[0], p[1], p[2]);
        }
        s.push('\n');
    });
    o.push_str("</Points>\n<Cells>\n");
    data_array(&mut o, "Int64", "connectivity", 1, |s| {
        for block in blocks {
            for p in block.connectivity {
                let _ = write!(s, "{p} ");
            }
        }
        s.push('\n');
    });
    data_array(&mut o, "Int64", "offsets", 1, |s| {
        let mut end = 0usize;
        for block in blocks {
            let npc = vtk_cell_nodes(block.cell_type).expect("checked");
            for _ in 0..block.connectivity.len() / npc {
                end += npc;
                let _ = write!(s, "{end} ");
            }
        }
        s.push('\n');
    });
    data_array(&mut o, "UInt8", "types", 1, |s| {
        for block in blocks {
            let npc = vtk_cell_nodes(block.cell_type).expect("checked");
            for _ in 0..block.connectivity.len() / npc {
                let _ = write!(s, "{} ", block.cell_type);
            }
        }
        s.push('\n');
    });
    o.push_str("</Cells>\n</Piece>\n</UnstructuredGrid>\n</VTKFile>\n");
    Ok(o)
}

/// Write a finite-element `.vtu` file (see [`fe_vtu_string`]).
pub fn write_fe_vtu(
    path: impl AsRef<Path>,
    points: &[Vec3],
    blocks: &[FeCellBlock],
    point_data: &[FeArray],
    cell_data: &[FeArray],
) -> Result<()> {
    std::fs::write(path, fe_vtu_string(points, blocks, point_data, cell_data)?).map_err(|e| Error::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};

    fn array_text<'a>(xml: &'a str, name: &str) -> &'a str {
        let key = format!("Name=\"{name}\"");
        let start = xml.find(&key).unwrap_or_else(|| panic!("{name} missing"));
        let body = xml[start..].find('>').unwrap() + start + 1;
        let end = xml[body..].find("</DataArray>").unwrap() + body;
        xml[body..end].trim()
    }

    fn parse_f64(s: &str) -> Vec<f64> {
        s.split_whitespace().map(|v| v.parse().unwrap()).collect()
    }

    #[test]
    fn vtu_roundtrips_field_values_exactly() {
        let m = box_mesh([3, 2, 2], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let p: Vec<f64> = (0..m.n_cells()).map(|i| (i as f64).sin() * 1e-7 + 1.0 / 3.0).collect();
        let u: Vec<Vec3> = (0..m.n_cells())
            .map(|i| [i as f64 * 0.1, -(i as f64).sqrt(), 1e300])
            .collect();
        let xml = vtu_string(
            &m,
            &[
                VtkField {
                    name: "p",
                    values: VtkValues::Scalar(&p),
                },
                VtkField {
                    name: "U",
                    values: VtkValues::Vector(&u),
                },
            ],
        )
        .unwrap();
        assert!(xml.contains("NumberOfCells=\"12\""));
        let p2 = parse_f64(array_text(&xml, "p"));
        assert!(p.iter().zip(&p2).all(|(a, b)| a.to_bits() == b.to_bits()));
        let u2 = parse_f64(array_text(&xml, "U"));
        assert_eq!(u2.len(), 36);
        assert!(u.iter().flatten().zip(&u2).all(|(a, b)| a.to_bits() == b.to_bits()));
        // connectivity structure
        let offs: Vec<usize> = array_text(&xml, "offsets")
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(offs.len(), 12);
        assert_eq!(offs[0], 8); // hexahedron has 8 distinct points
        let fo: Vec<usize> = array_text(&xml, "faceoffsets")
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(fo[0], 1 + 6 * 5); // n_faces + 6 * (n_pts + 4 ids)
        assert!(array_text(&xml, "types").split_whitespace().all(|t| t == "42"));
    }

    #[test]
    fn polyhedron_faces_point_outward() {
        let m = box_mesh([2, 2, 2], [1.0; 3], [Grading::Uniform; 3]).unwrap();
        for c in 0..m.n_cells() {
            let s = face_stream(&m, c);
            let (nf, mut k) = (s[0], 1);
            let cc = m.cell_centres()[c];
            for _ in 0..nf {
                let n = s[k];
                let ids = &s[k + 1..k + 1 + n];
                k += 1 + n;
                let pts: Vec<Vec3> = ids.iter().map(|&i| m.points()[i]).collect();
                let a = rcfd_core::vec3::sub(pts[1], pts[0]);
                let b = rcfd_core::vec3::sub(pts[2], pts[0]);
                let normal = rcfd_core::vec3::cross(a, b);
                let out = rcfd_core::vec3::sub(pts[0], cc);
                assert!(rcfd_core::vec3::dot(normal, out) > 0.0, "cell {c}");
            }
        }
    }

    #[test]
    fn vtp_patch_pvd_pvtu_and_legacy() {
        let m = cavity_2d(4, 1.0).unwrap();
        let lid = m.patch("lid").unwrap();
        let shear: Vec<f64> = (0..lid.size).map(|i| i as f64 * 0.5).collect();
        let xml = vtp_string(
            &m,
            "lid",
            &[VtkField {
                name: "wallShear",
                values: VtkValues::Scalar(&shear),
            }],
        )
        .unwrap();
        assert!(xml.contains("NumberOfPolys=\"4\""));
        assert_eq!(parse_f64(array_text(&xml, "wallShear")), shear);
        assert!(vtp_string(&m, "nope", &[]).is_err());
        assert!(vtp_string(
            &m,
            "lid",
            &[VtkField {
                name: "bad",
                values: VtkValues::Scalar(&[1.0])
            }]
        )
        .is_err());

        let mut pvd = PvdSeries::new();
        pvd.add(0.0, "case_0.vtu");
        pvd.add(0.5, "case_1.vtu");
        let x = pvd.to_xml();
        assert!(x.contains("timestep=\"0.5\"") && x.contains("file=\"case_1.vtu\""));

        let px = pvtu_string(&["a.vtu".into(), "b.vtu".into()], &[("p".into(), 1), ("U".into(), 3)]);
        assert!(px.matches("<Piece Source").count() == 2 && px.contains("Name=\"U\" NumberOfComponents=\"3\""));

        let p: Vec<f64> = (0..m.n_cells()).map(|i| i as f64).collect();
        let legacy = legacy_vtk_string(
            &m,
            &[VtkField {
                name: "p",
                values: VtkValues::Scalar(&p),
            }],
        )
        .unwrap();
        assert!(legacy.starts_with("# vtk DataFile Version 4.2"));
        assert!(legacy.contains("CELL_TYPES 16") && legacy.contains("SCALARS p double 1"));
    }

    #[test]
    fn files_are_written() {
        let m = cavity_2d(2, 1.0).unwrap();
        let dir = std::env::temp_dir().join(format!("rcfd_vtk_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_vtu(dir.join("a.vtu"), &m, &[]).unwrap();
        write_vtp(dir.join("lid.vtp"), &m, "lid", &[]).unwrap();
        let mut s = PvdSeries::new();
        s.add(1.0, "a.vtu");
        s.write(dir.join("c.pvd")).unwrap();
        assert!(dir.join("a.vtu").exists() && dir.join("lid.vtp").exists() && dir.join("c.pvd").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
