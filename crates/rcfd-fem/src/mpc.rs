//! Linear multi-point constraints by the transformation `u = T q + g`, rigid sets with a reference point, elastic supports
//! ([`crate::model::Constraint::Linear`], [`crate::model::Constraint::RigidSet`], [`crate::model::ElasticSupport`]).
//! Declared by ADR 0291; filled by ADR 0296.

/// The work package that adds linear constraints, rigid sets and elastic supports; `None` once it has landed.
pub const PENDING: Option<&'static str> = Some("ADR 0296");
