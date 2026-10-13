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
