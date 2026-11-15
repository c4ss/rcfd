//! Mean-dilatation element for near incompressibility. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR
//! 0541 (the registration of ADR 0328, F-425).
//!
//! The mean-dilatation element is the F-bar Hex8 of ADR 0506 (a solid block of the `BBar` formulation in the nonlinear
//! driver, [`crate::nonlinear`]) with a hyperelastic energy of the split form `W = W_iso(Cbar) + U(J)` of
//! [`crate::mat_hyper`]: `F_bar = (J_bar / J)^(1/3) F` has `det F_bar = J_bar`, the element mean of `det F`, and the isochoric
//! part of `F`, so the element energy is `sum_q w_q W_iso(Cbar_q) + V U(J_bar)`, the three-field element of Simo, Taylor and
//! Pister with a constant volume ratio and pressure per element, condensed (ADR 0541 arithmetic 2). No arithmetic lives here.

use crate::mesh::{BlockProperty, SolidFormulation};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The block property of a solid block that takes the mean-dilatation element (`BlockProperty::solid(SolidFormulation::BBar)`).
pub fn block_property() -> BlockProperty {
    BlockProperty::solid(SolidFormulation::BBar)
}
