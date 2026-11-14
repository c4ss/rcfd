//! Power-law creep with time hardening. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0314 (F-411).

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0314 (F-411)");

/// The declared state per integration point (ADR 0440 D3): the creep strain (six values) and the accumulated equivalent
/// creep strain.
pub const STATE_SIZE: usize = 7;
