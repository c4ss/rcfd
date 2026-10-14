//! Mesh readers/writers and the native versioned rcfd case container.
//!
//! * [`gmsh`] - Gmsh `.msh` v2.x and v4.1, ASCII and binary (F-013)
//! * [`openfoam`] - OpenFOAM `polyMesh` reader and writer (F-014)
//! * [`fluent`] - Fluent `.msh` / `.cas` reader, ASCII and binary sections, 2D and 3D (F-014, F-118)
//! * [`stl`], [`tecplot`], [`plot3d`], [`xdmf`] - interchange formats (F-119, ADR 0011)
//! * [`container`] - native case/data container (F-015)
//!
//! All formats are implemented from their public documentation.

pub mod cases;
pub mod common;
pub mod container;
pub mod datapack;
pub mod fluent;
pub mod gmsh;
pub mod guard;
pub mod openfoam;
pub mod plot3d;
pub mod stl;
pub mod tecplot;
pub mod vtk;
pub mod xdmf;

/// Read a `.msh` file of either family: Fluent (starts with a parenthesis) or Gmsh (`$MeshFormat`).
/// The second value lists non-fatal notes, for example Fluent sections that were skipped.
pub fn read_msh_with_notes(path: impl AsRef<std::path::Path>) -> rcfd_core::Result<(rcfd_mesh::Mesh, Vec<String>)> {
    let path = path.as_ref();
    let bytes = std::fs::read(path)?;
    let first = bytes.iter().copied().find(|b| !b.is_ascii_whitespace());
    if first == Some(b'(') || bytes.starts_with(&[0x1f, 0x8b]) || bytes.starts_with(b"\x89HDF") {
        let (mesh, rep) = fluent::read_bytes(&bytes, &fluent::Options::default())?;
        let mut notes = Vec::new();
        if rep.dimension == 2 {
            notes.push("Fluent 2D mesh extruded to one layer of depth 1 (frontAndBack empty patch)".to_string());
        }
        if !rep.ignored_sections.is_empty() {
            let list: Vec<String> = rep.ignored_sections.iter().map(|(i, n)| format!("{i} x{n}")).collect();
            notes.push(format!("Fluent sections not read: {}", list.join(", ")));
        }
        Ok((mesh, notes))
    } else {
        gmsh::read_file(path).map(|m| (m, Vec::new()))
    }
}

/// Read a `.msh` file of either family (see [`read_msh_with_notes`]).
pub fn read_msh_file(path: impl AsRef<std::path::Path>) -> rcfd_core::Result<rcfd_mesh::Mesh> {
    read_msh_with_notes(path).map(|(m, _)| m)
}
