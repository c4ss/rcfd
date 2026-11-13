//! Quadratic hexahedron (Hex20): shape functions, the 3 x 3 x 3 rule, its 8-node faces, stiffness, mass and the
//! quadratic upgrade of a hexahedron box. Declared by ADR 0291; filled by ADR 0292.
//!
//! Until ADR 0292 lands, the kind-specific data that [`crate::mesh::ElementKind`] and [`crate::element::element_for`]
//! delegate to this module are empty and [`element`] is a placeholder whose every fallible entry point refuses (ADR 0291
//! D2, D4). The landing package fills these functions and sets [`PENDING`] to `None` in this file only.

use crate::element::{Element, Pending};
use crate::mesh::ElementKind;

/// The work package that adds Hex20; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0292");

static ELEMENT: Pending = Pending::new(ElementKind::Hex20, "ADR 0292");

/// The element of the kind (the placeholder until ADR 0292).
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (empty until ADR 0292).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces, corners first (empty until ADR 0292).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (empty until ADR 0292).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}

/// The fitted polynomial space of the stress recovery at `xi` (empty until ADR 0292).
pub fn fit_basis(_xi: [f64; 3]) -> Vec<f64> {
    Vec::new()
}

/// The volume of an element from `det J` at reference points (NaN until ADR 0292).
pub fn volume(_jacobian_at: &dyn Fn([f64; 3]) -> f64) -> f64 {
    f64::NAN
}
