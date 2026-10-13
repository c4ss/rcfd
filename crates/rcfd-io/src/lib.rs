//! Mesh readers/writers and the native versioned rcfd case container.
//!
//! * [`gmsh`] - Gmsh `.msh` v2.x and v4.1, ASCII and binary (F-013)
//! * [`openfoam`] - OpenFOAM `polyMesh` reader and writer (F-014)
//! * [`fluent`] - Fluent `.msh` ASCII reader (F-014)
//! * [`container`] - native case/data container (F-015)
//!
//! All formats are implemented from their public documentation.

pub mod cases;
pub mod common;
pub mod container;
pub mod fluent;
pub mod gmsh;
pub mod openfoam;
pub mod vtk;
