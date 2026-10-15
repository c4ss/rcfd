//! ASCII VTK XML PolyData output of particles (points and attributes), ADR 0028 D8.

use crate::tracker::Particle;
use std::fmt::Write as _;
use std::path::Path;

fn array(out: &mut String, name: &str, ty: &str, comps: usize, vals: impl Iterator<Item = String>) {
    let _ = writeln!(
        out,
        "        <DataArray type=\"{ty}\" Name=\"{name}\" NumberOfComponents=\"{comps}\" format=\"ascii\">"
    );
    let v: Vec<String> = vals.collect();
    let _ = writeln!(out, "          {}", v.join(" "));
    out.push_str("        </DataArray>\n");
}

/// VTP text for the given particles (all states; `state` is an attribute).
pub fn vtp_string(ps: &[Particle]) -> String {
    let n = ps.len();
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\"?>\n<VTKFile type=\"PolyData\" version=\"0.1\" byte_order=\"LittleEndian\">\n  <PolyData>\n");
    let _ = writeln!(
        s,
        "    <Piece NumberOfPoints=\"{n}\" NumberOfVerts=\"{n}\" NumberOfLines=\"0\" NumberOfStrips=\"0\" NumberOfPolys=\"0\">"
    );
    s.push_str("      <Points>\n");
    array(
        &mut s,
        "Points",
        "Float64",
        3,
        ps.iter().flat_map(|p| p.x.iter().map(|x| format!("{x}"))),
    );
    s.push_str("      </Points>\n      <Verts>\n");
    array(&mut s, "connectivity", "Int64", 1, (0..n).map(|i| i.to_string()));
    array(&mut s, "offsets", "Int64", 1, (1..=n).map(|i| i.to_string()));
    s.push_str("      </Verts>\n      <PointData>\n");
    array(&mut s, "id", "Int64", 1, ps.iter().map(|p| p.id.to_string()));
    array(&mut s, "diameter", "Float64", 1, ps.iter().map(|p| format!("{}", p.d)));
    array(
        &mut s,
        "velocity",
        "Float64",
        3,
        ps.iter().flat_map(|p| p.v.iter().map(|x| format!("{x}"))),
    );
    array(&mut s, "density", "Float64", 1, ps.iter().map(|p| format!("{}", p.rho)));
    array(
        &mut s,
        "parcel_count",
        "Float64",
        1,
        ps.iter().map(|p| format!("{}", p.parcel)),
    );
    array(&mut s, "cell", "Int64", 1, ps.iter().map(|p| p.cell.to_string()));
    array(&mut s, "age", "Float64", 1, ps.iter().map(|p| format!("{}", p.age)));
    array(
        &mut s,
        "state",
        "Int32",
        1,
        ps.iter().map(|p| (p.state as i32).to_string()),
    );
    s.push_str("      </PointData>\n    </Piece>\n  </PolyData>\n</VTKFile>\n");
    s
}

/// Write the VTP file.
pub fn write_vtp(path: impl AsRef<Path>, ps: &[Particle]) -> std::io::Result<()> {
    std::fs::write(path, vtp_string(ps))
}
