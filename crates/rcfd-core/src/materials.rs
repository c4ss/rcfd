//! Temperature-dependent property laws (F-053), the base of the material library (F-111).
//!
//! * `Constant(v)`
//! * `Polynomial(c)`: `p(T) = c0 + c1 T + c2 T^2 + ...` evaluated by Horner's rule (T in kelvin)
//! * `Sutherland`: `p(T) = p_ref (T / T_ref)^(3/2) (T_ref + S) / (T + S)`
//!
//! Sutherland's law is the kinetic-theory result of W. Sutherland, Phil. Mag. 36 (1893) 507-531, for the
//! viscosity of a gas of weakly attracting rigid spheres; the same functional form is used for the thermal
//! conductivity with its own constant. The constants are inputs: nothing here ships measured data.

use crate::error::{Error, Result};
use crate::tabular::{PiecewiseLinear, PiecewisePoly, Vogel};

/// A scalar property as a function of temperature.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyLaw {
    /// Independent of temperature.
    Constant(f64),
    /// Polynomial in T (coefficients in ascending powers).
    Polynomial(Vec<f64>),
    /// Sutherland's law.
    Sutherland {
        /// Value at the reference temperature.
        reference_value: f64,
        /// Reference temperature (K).
        reference_temperature: f64,
        /// Sutherland constant S (K).
        constant: f64,
    },
    /// Piecewise-linear table in T (F-220).
    PiecewiseLinear(PiecewiseLinear),
    /// Piecewise polynomial in T (F-220).
    PiecewisePolynomial(PiecewisePoly),
    /// `a exp(b / (T - c))` (F-220).
    Vogel(Vogel),
}

impl PropertyLaw {
    /// Value at temperature `t` (K).
    pub fn eval(&self, t: f64) -> f64 {
        match self {
            PropertyLaw::Constant(v) => *v,
            PropertyLaw::Polynomial(c) => c.iter().rev().fold(0.0, |acc, &k| acc * t + k),
            PropertyLaw::Sutherland {
                reference_value,
                reference_temperature,
                constant,
            } => {
                let r = t / reference_temperature;
                reference_value * r * r.sqrt() * (reference_temperature + constant) / (t + constant)
            }
            PropertyLaw::PiecewiseLinear(l) => l.eval(t),
            PropertyLaw::PiecewisePolynomial(l) => l.eval(t),
            PropertyLaw::Vogel(l) => l.eval(t),
        }
    }

    /// True when the value depends on temperature.
    pub fn depends_on_temperature(&self) -> bool {
        !matches!(self, PropertyLaw::Constant(_))
    }

    /// Check that the law is usable: finite coefficients, positive reference data.
    pub fn validate(&self) -> Result<()> {
        let bad = |reason: &str| {
            Err(Error::InvalidValue {
                path: "property law".into(),
                reason: reason.into(),
            })
        };
        match self {
            PropertyLaw::Constant(v) if !v.is_finite() => bad("the value must be finite"),
            PropertyLaw::Polynomial(c) if c.is_empty() || c.iter().any(|v| !v.is_finite()) => {
                bad("a polynomial needs at least one finite coefficient")
            }
            PropertyLaw::Sutherland {
                reference_value,
                reference_temperature,
                constant,
            } if !(*reference_value > 0.0 && *reference_temperature > 0.0 && *constant > -*reference_temperature) => {
                bad("Sutherland's law needs a positive reference value and temperature and S > -T_ref")
            }
            PropertyLaw::Vogel(v) => v.validate(),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_uses_ascending_powers() {
        let p = PropertyLaw::Polynomial(vec![1.0, 2.0, 3.0]);
        assert_eq!(p.eval(2.0), 1.0 + 4.0 + 12.0);
        assert_eq!(PropertyLaw::Polynomial(vec![5.0]).eval(1e3), 5.0);
    }

    #[test]
    fn sutherland_returns_the_reference_value_and_the_closed_form() {
        let s = PropertyLaw::Sutherland {
            reference_value: 1.716e-5,
            reference_temperature: 273.15,
            constant: 110.4,
        };
        assert!((s.eval(273.15) - 1.716e-5).abs() < 1e-18);
        // closed form at T = 2 T_ref
        let t = 2.0 * 273.15;
        let want = 1.716e-5 * 2f64.powf(1.5) * (273.15 + 110.4) / (t + 110.4);
        assert!((s.eval(t) - want).abs() < 1e-15 * want);
        assert!(s.depends_on_temperature());
        assert!(!PropertyLaw::Constant(1.0).depends_on_temperature());
    }

    #[test]
    fn invalid_laws_are_rejected() {
        assert!(PropertyLaw::Polynomial(vec![]).validate().is_err());
        assert!(PropertyLaw::Polynomial(vec![f64::NAN]).validate().is_err());
        assert!(PropertyLaw::Sutherland {
            reference_value: 0.0,
            reference_temperature: 300.0,
            constant: 100.0
        }
        .validate()
        .is_err());
    }
}
