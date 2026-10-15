//! Unstructured polyhedral mesh data structures, quality metrics, simple
//! block mesh generation and partitioning.
//!
//! Conventions (owner/neighbour face-addressing, as in standard FV texts):
//! * faces `0..n_internal_faces` are internal; each has an owner cell and a
//!   neighbour cell with `owner < neighbour`;
//! * the face normal (area vector) points from owner to neighbour;
//! * faces `n_internal_faces..n_faces` are boundary faces, grouped by patch,
//!   with the normal pointing out of the domain.

pub mod edit;
pub mod gen;
pub mod inflate;
pub mod layers;
pub mod measures;
pub mod mesh;
pub mod non_conformal;
pub mod partition;
pub mod predicates;
pub mod quality;
pub mod surface;
pub mod tetmesh;

pub use mesh::{BoundarySpec, Mesh, Patch, PatchKind, PeriodicRotation, Zone};
