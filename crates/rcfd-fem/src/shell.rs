//! Four-node shell (Shell4): flat facet with assumed transverse shear strain, resultants and surface stresses. Declared
//! by ADR 0291; filled by ADR 0295.
//!
//! Until ADR 0295 lands, the kind-specific data that [`crate::mesh::ElementKind`] and [`crate::element::element_for`]
//! delegate to this module are empty and [`element`] is a placeholder whose every fallible entry point refuses (ADR 0291
//! D2, D4). The landing package fills these functions and sets [`PENDING`] to `None` in this file only.

use crate::element::{Element, Pending};
use crate::mesh::ElementKind;

/// The work package that adds Shell4; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0295");

static ELEMENT: Pending = Pending::new(ElementKind::Shell4, "ADR 0295");

/// The element of the kind (the placeholder until ADR 0295).
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (empty until ADR 0295).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces, corners first (empty until ADR 0295).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (empty until ADR 0295).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}
