//! Constitutive matrices and the constitutive call of the materials (ADR 0273 D5). Declared by ADR 0271; filled by ADR
//! 0273.
//!
//! Voigt order xx, yy, zz, xy, yz, zx with engineering shear strains (ADR 0270 section 2.2), so the strain energy density
//! is `eps^T D eps / 2`.

use crate::error::{FemError, Result};
use crate::model::Material;

/// A 6 x 6 matrix in Voigt order (row-major array of rows).
pub type Matrix6 = [[f64; 6]; 6];

/// The isotropic elasticity matrix from Young's modulus and the Poisson ratio: `lambda = E nu / ((1 + nu)(1 - 2 nu))`,
/// `mu = E / (2 (1 + nu))`; normal block `lambda + 2 mu` on the diagonal and `lambda` off it, shear diagonal `mu`. No
/// validation: the values come from [`Material::isotropic_elastic`].
pub fn isotropic_elasticity(e: f64, nu: f64) -> Matrix6 {
    let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
    let mu = e / (2.0 * (1.0 + nu));
    let mut d = [[0.0; 6]; 6];
    for (i, row) in d.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = if i == j { lambda + 2.0 * mu } else { lambda };
        }
    }
    for (k, row) in d.iter_mut().enumerate().skip(3) {
        row[k] = mu;
    }
    d
}

/// `D v` for a Voigt vector.
pub fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    let mut s = [0.0; 6];
    for (si, row) in s.iter_mut().zip(d) {
        *si = row.iter().zip(v).map(|(a, b)| a * b).sum();
    }
    s
}

/// What the constitutive call knows about the point it is evaluated at. Non-exhaustive and empty in the basic tier:
/// ADR 0311 adds temperature, time increment and material axes, the default meaning "none, global axes".
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct PointContext {}

impl Material {
    /// The elasticity matrix of a linear elastic material.
    pub fn elasticity_matrix(&self) -> Result<Matrix6> {
        match self {
            Material::IsotropicElastic { e, nu, .. } => Ok(isotropic_elasticity(*e, *nu)),
        }
    }

    /// Number of state values per integration point (0 for elasticity).
    pub fn state_size(&self) -> usize {
        match self {
            Material::IsotropicElastic { .. } => 0,
        }
    }

    /// The constitutive call: stress and tangent for a strain (Voigt, engineering shear) at a point with its state
    /// (`state_size()` values). For linear elasticity the stress is `D eps` and the tangent `D`.
    pub fn stress(&self, strain: &[f64; 6], point: &PointContext, state: &[f64]) -> Result<([f64; 6], Matrix6)> {
        let _ = point;
        if state.len() != self.state_size() {
            return Err(FemError::FieldLength {
                key: "state",
                expected: self.state_size(),
                found: state.len(),
            });
        }
        let d = self.elasticity_matrix()?;
        Ok((apply(&d, strain), d))
    }
}
