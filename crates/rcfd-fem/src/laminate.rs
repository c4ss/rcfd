//! Laminated shell: layup stiffness, ply stresses, failure indices. Declared by ADR 0440 (F-408, the scaffold of ADR
//! 0311); filled by ADR 0331 (F-428).
//!
//! [`section_stiffness`] is the laminate's side of the section-stiffness hook of the shell
//! ([`crate::shell::section_of`]): it replaces the thickness integral of a homogeneous shell without editing the element.
//! Until the package lands it refuses, and every solve refuses a `Layup` block before reaching it.

use crate::error::{FemError, Result};
use crate::mesh::Ply;
use crate::shell::SectionStiffness;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0331 (F-428)");

/// The section stiffness of a layup (membrane, coupling, bending and transverse shear). Refused until ADR 0331 lands.
pub fn section_stiffness(plies: &[Ply]) -> Result<SectionStiffness> {
    let _ = plies;
    Err(FemError::NotYetSupported {
        item: "shell property",
        variant: "Layup",
        location: "the section stiffness".to_string(),
        package: PENDING.unwrap_or("ADR 0331 (F-428)"),
    })
}
