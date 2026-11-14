//! J2 plasticity with isotropic and kinematic hardening. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0325 (F-422).

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0325 (F-422)");

/// The declared state per integration point (ADR 0440 D3): the plastic strain (six), the back stress (six) and the
/// equivalent plastic strain.
pub const STATE_SIZE: usize = 13;
