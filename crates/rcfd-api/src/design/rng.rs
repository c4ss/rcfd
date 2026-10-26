//! The seeded generator of the design loop (ADR 0067). It lives in `rcfd_core::rng` since ADR 0118, so that the mesh crate
//! (fan tolerance ensembles) uses the same stream; this path re-exports it unchanged.

pub use rcfd_core::rng::Rng;
