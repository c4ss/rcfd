//! Linear viscoelasticity: Prony series, time-temperature shift, complex modulus. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0313 (F-410).

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0313 (F-410)");

/// The declared state per integration point (ADR 0440 D3): the internal strain (six values) of each Prony term.
pub fn state_size(terms: usize) -> usize {
    6 * terms
}
