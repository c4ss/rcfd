//! Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, selected per element block by
//! [`crate::mesh::SolidFormulation`]. Declared by ADR 0291; filled by ADR 0293.

/// The work package that adds the formulations `BBar` and `Incompatible`; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0293");
