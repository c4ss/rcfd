//! Three-node shell (Shell3: flat facet, assumed shear strain) for triangulated surfaces. Declared by ADR 0440 (F-408, the
//! scaffold of ADR 0311); filled by ADR 0322 (F-419).
//!
//! Until the package lands the kind is refused by every solve ([`crate::SolidModel::check_elements_supported`] through
//! [`crate::ElementKind::pending`]) and [`element`] is a placeholder whose every fallible entry refuses naming it.

use crate::element::{Element, Pending};
use crate::mesh::ElementKind;

/// The package named while Shell3 has not landed.
const PACKAGE: &str = "ADR 0322 (F-419)";

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some(PACKAGE);

static ELEMENT: Pending = Pending::new(ElementKind::Shell3, PACKAGE);

/// The element of the kind for the solid-element routines: a placeholder that refuses.
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (none: a shell has no isoparametric volume map).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces (none).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (none).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}
