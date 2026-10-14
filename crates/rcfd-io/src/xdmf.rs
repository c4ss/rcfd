//! XDMF writer with raw binary heavy data (F-119, ADR 0011 section 2.4).
//!
//! The light data (`.xmf`, XML) describe a mixed-topology unstructured grid and cell attributes; the heavy data are
//! plain little-endian `Float64` / `Int64` files next to it, referenced with `Format="Binary"`. HDF5 heavy data are
//! not written (no HDF5 writer exists in this workspace). Written from the public XDMF description; whether a given
//! viewer opens the file is reported by the tests that actually ran a third-party reader, not assumed.
//!
//! Mixed-topology type numbers: triangle 4, quadrilateral 5, tetrahedron 6, pyramid 7, wedge 8, hexahedron 9.

use crate::common::Shape;
use crate::tecplot::ordered_cell_nodes;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Everything to write: the XML and the heavy-data files (name relative to the XML, bytes).
#[derive(Debug, Clone)]
pub struct XdmfFiles {
    /// XML text.
    pub xml: String,
    /// Heavy data files.
    pub blobs: Vec<(String, Vec<u8>)>,
}

fn safe_stem(stem: &str) -> Result<String> {
    if stem.is_empty()
        || stem.len() > 100
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        || stem.starts_with('.')
    {
        return Err(Error::Io(format!(
            "XDMF: file stem '{stem}' must be 1-100 characters of letters, digits, '_', '-' or '.' and not start with '.'"
        )));
    }
    Ok(stem.to_string())
}

fn xml_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            c if c.is_control() => "_".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Build the XDMF text and the heavy-data files of a mesh with cell scalar fields.
pub fn build(mesh: &Mesh, stem: &str, fields: &[(&str, &[f64])]) -> Result<XdmfFiles> {
    let stem = safe_stem(stem)?;
    for (n, f) in fields {
        if f.len() != mesh.n_cells() {
            return Err(Error::Io(format!(
                "field '{n}' has {} values, the mesh has {} cells",
                f.len(),
                mesh.n_cells()
            )));
        }
        if let Some(i) = f.iter().position(|v| !v.is_finite()) {
            return Err(Error::Io(format!("field '{n}': cell {i} is not finite")));
        }
    }
    let mut conn: Vec<i64> = Vec::new();
    for c in 0..mesh.n_cells() {
        let (shape, v) = ordered_cell_nodes(mesh, c)?;
        let ty = match shape {
            Shape::Tet => 6,
            Shape::Pyramid => 7,
            Shape::Prism => 8,
            Shape::Hex => 9,
        };
        conn.push(ty);
        // The VTK reader of XDMF reads the wedge with the opposite orientation of the Gmsh/Tecplot prism order
        // (negative cell volume on a Gmsh prism mesh), so both triangles are reversed for the wedge.
        let nodes: Vec<usize> = if shape == Shape::Prism {
            vec![v[0], v[2], v[1], v[3], v[5], v[4]]
        } else {
            v
        };
        conn.extend(nodes.iter().map(|&n| n as i64));
    }
    let mut pts = Vec::with_capacity(mesh.n_points() * 24);
    for p in mesh.points() {
        for c in p {
            pts.extend_from_slice(&c.to_le_bytes());
        }
    }
    let conn_bytes: Vec<u8> = conn.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut blobs = vec![
        (format!("{stem}.points.bin"), pts),
        (format!("{stem}.cells.bin"), conn_bytes),
    ];
    let mut o = String::new();
    o.push_str("<?xml version=\"1.0\" ?>\n<Xdmf Version=\"3.0\">\n  <Domain>\n");
    let _ = writeln!(o, "    <Grid Name=\"{}\" GridType=\"Uniform\">", xml_escape(&stem));
    let _ = writeln!(
        o,
        "      <Topology TopologyType=\"Mixed\" NumberOfElements=\"{}\">",
        mesh.n_cells()
    );
    let _ = writeln!(
        o,
        "        <DataItem Format=\"Binary\" NumberType=\"Int\" Precision=\"8\" Endian=\"Little\" Dimensions=\"{}\">{stem}.cells.bin</DataItem>",
        conn.len()
    );
    o.push_str("      </Topology>\n      <Geometry GeometryType=\"XYZ\">\n");
    let _ = writeln!(
        o,
        "        <DataItem Format=\"Binary\" NumberType=\"Float\" Precision=\"8\" Endian=\"Little\" Dimensions=\"{} 3\">{stem}.points.bin</DataItem>",
        mesh.n_points()
    );
    o.push_str("      </Geometry>\n");
    for (k, (name, f)) in fields.iter().enumerate() {
        let file = format!("{stem}.field{k}.bin");
        blobs.push((file.clone(), f.iter().flat_map(|v| v.to_le_bytes()).collect()));
        let _ = writeln!(
            o,
            "      <Attribute Name=\"{}\" AttributeType=\"Scalar\" Center=\"Cell\">",
            xml_escape(name)
        );
        let _ = writeln!(
            o,
            "        <DataItem Format=\"Binary\" NumberType=\"Float\" Precision=\"8\" Endian=\"Little\" Dimensions=\"{}\">{file}</DataItem>",
            f.len()
        );
        o.push_str("      </Attribute>\n");
    }
    o.push_str("    </Grid>\n  </Domain>\n</Xdmf>\n");
    Ok(XdmfFiles { xml: o, blobs })
}

/// Write `<dir>/<stem>.xmf` and the heavy-data files; returns the written paths (the XML first).
pub fn write(dir: impl AsRef<Path>, stem: &str, mesh: &Mesh, fields: &[(&str, &[f64])]) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let files = build(mesh, stem, fields)?;
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    let xmf = dir.join(format!("{}.xmf", safe_stem(stem)?));
    std::fs::write(&xmf, files.xml)?;
    out.push(xmf);
    for (name, bytes) in files.blobs {
        let p = dir.join(&name);
        std::fs::write(&p, bytes)?;
        out.push(p);
    }
    Ok(out)
}

/// Minimal well-formedness check used by the tests: balanced tags, quoted attributes, one root.
pub fn check_wellformed(xml: &str) -> std::result::Result<(), String> {
    let b: Vec<char> = xml.chars().collect();
    let mut i = 0;
    let mut stack: Vec<String> = Vec::new();
    let mut roots = 0;
    while i < b.len() {
        if b[i] != '<' {
            if stack.is_empty() && !b[i].is_whitespace() {
                return Err(format!("text outside the root at char {i}"));
            }
            i += 1;
            continue;
        }
        let end = (i..b.len()).find(|&j| b[j] == '>').ok_or("unterminated tag")?;
        let tag: String = b[i + 1..end].iter().collect();
        if tag.starts_with('?') || tag.starts_with('!') {
        } else if let Some(name) = tag.strip_prefix('/') {
            let open = stack.pop().ok_or("closing tag without opening tag")?;
            if open != name.trim() {
                return Err(format!("</{}> closes <{open}>", name.trim()));
            }
        } else {
            let self_close = tag.ends_with('/');
            let body = tag.trim_end_matches('/');
            let name = body.split_whitespace().next().ok_or("empty tag")?;
            if body.matches('"').count() % 2 != 0 {
                return Err(format!("odd number of quotes in <{name}>"));
            }
            if stack.is_empty() {
                roots += 1;
            }
            if !self_close {
                stack.push(name.to_string());
            }
        }
        i = end + 1;
    }
    if !stack.is_empty() {
        return Err(format!("unclosed <{}>", stack.last().expect("non-empty")));
    }
    if roots != 1 {
        return Err(format!("{roots} root elements"));
    }
    Ok(())
}
