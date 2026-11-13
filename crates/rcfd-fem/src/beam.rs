//! Two-node shear-flexible beam (Beam2): sections, stiffness, mass and section forces. Declared by ADR 0291; filled by
//! ADR 0294.
//!
//! Until ADR 0294 lands, the kind-specific data that [`crate::mesh::ElementKind`] and [`crate::element::element_for`]
//! delegate to this module are empty and [`element`] is a placeholder whose every fallible entry point refuses (ADR 0291
//! D2, D4). The landing package fills these functions and sets [`PENDING`] to `None` in this file only.

use crate::element::{Element, Pending};
use crate::mesh::ElementKind;

/// The work package that adds Beam2; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0294");

static ELEMENT: Pending = Pending::new(ElementKind::Beam2, "ADR 0294");

/// The element of the kind (the placeholder until ADR 0294).
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (empty until ADR 0294).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces, corners first (empty until ADR 0294).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (empty until ADR 0294).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}
