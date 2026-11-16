//! F-703 (ADR 0630): the ADR 0608 checks R-* with the stabilisation of rule L (the linear-stress coefficient, diagnostic); the body is
//! `f703/r_body.rs`, a copy of the R-* part of `si_eas_hg.rs` (existing tests are not edited).

use rcfd_fem::nl_incompatible::IncompatibleStabilisation;

const STAB: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;

include!("f703/r_body.rs");
