//! Unstructured polyhedral mesh data structures, quality metrics, simple
//! block mesh generation and partitioning.
//!
//! Conventions (owner/neighbour face-addressing, as in standard FV texts):
//! * faces `0..n_internal_faces` are internal; each has an owner cell and a
//!   neighbour cell with `owner < neighbour`;
//! * the face normal (area vector) points from owner to neighbour;
//! * faces `n_internal_faces..n_faces` are boundary faces, grouped by patch,
//!   with the normal pointing out of the domain.

pub mod adapt;
pub mod edit;
pub mod fan;
pub mod fan_cgrid;
pub mod fan_design;
pub mod fan_mesh;
pub mod gen;
pub mod inflate;
pub mod layering;
pub mod layers;
pub mod measures;
pub mod mesh;
pub mod motion;
pub mod non_conformal;
pub mod octree;
pub mod partition;
pub mod polydual;
pub mod predicates;
pub mod quality;
pub mod surface;
pub mod surface_remesh;
pub mod sweep;
pub mod tetmesh;

pub use mesh::{BoundarySpec, Mesh, Patch, PatchKind, PeriodicRotation, Zone};
