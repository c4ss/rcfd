//! Constitutive matrices and the constitutive call of the materials (ADR 0273 D5). Declared by ADR 0271; filled by ADR
//! 0273.
//! ADR 0440 (F-408) adds the point context, the sub-data of the advanced-tier material variants and the declared state
//! layout.
//!
//! Voigt order xx, yy, zz, xy, yz, zx with engineering shear strains (ADR 0270 section 2.2), so the strain energy density
//! is `eps^T D eps / 2`.

use crate::error::{FemError, Result};
use crate::model::Material;
use rcfd_core::vec3::Vec3;

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

/// The tolerance of the orthonormality and handedness tests of a set of material axes (ADR 0440 D2): every entry of
/// `R R^T - I` and `det R - 1` within it.
pub const AXES_TOLERANCE: f64 = 1e-12;

/// Material axes at a point: the rows are the three axes in global components.
pub type Axes = [[f64; 3]; 3];

/// Refuses axes that are not finite, orthonormal and right-handed within [`AXES_TOLERANCE`] (key `"axes"`).
pub fn check_axes(axes: &Axes) -> Result<()> {
    let bad = |reason: String| FemError::InvalidValue { key: "axes", reason };
    if !axes.iter().flatten().all(|v| v.is_finite()) {
        return Err(bad(format!("{axes:?} must be finite")));
    }
    for i in 0..3 {
        for j in 0..3 {
            let g: f64 = (0..3).map(|k| axes[i][k] * axes[j][k]).sum();
            let target = if i == j { 1.0 } else { 0.0 };
            if (g - target).abs() > AXES_TOLERANCE {
                return Err(bad(format!("{axes:?} are not orthonormal: (R R^T)[{i}][{j}] = {g:e}")));
            }
        }
    }
    let [a, b, c] = axes;
    let det =
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
    if (det - 1.0).abs() > AXES_TOLERANCE {
        return Err(bad(format!("{axes:?} are not right-handed: det = {det:e}")));
    }
    Ok(())
}

/// What the constitutive call knows about the point it is evaluated at (ADR 0310 section 2.3, ADR 0440 D3): the
/// temperature, the time and time increment, and the material axes. The default (every field `None`) means no
/// temperature, no time increment and the global axes; strain and stress are always in global axes, the axes are for an
/// anisotropic material to rotate into. The isotropic elastic material does not read the context.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct PointContext {
    /// Temperature at the point, K.
    pub temperature: Option<f64>,
    /// Time at the end of the increment, s.
    pub time: Option<f64>,
    /// Time increment, s, non-negative.
    pub time_increment: Option<f64>,
    /// The material axes at the point (rows in global components); `None`: the global axes.
    pub axes: Option<Axes>,
}

impl PointContext {
    /// The default context.
    pub fn new() -> PointContext {
        PointContext::default()
    }
    /// The same context with a temperature; refuses a non-finite value.
    pub fn with_temperature(mut self, temperature: f64) -> Result<PointContext> {
        if !temperature.is_finite() {
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason: format!("{temperature} must be finite"),
            });
        }
        self.temperature = Some(temperature);
        Ok(self)
    }
    /// The same context with the time and the time increment; refuses a non-finite time and an increment that is negative
    /// or not finite.
    pub fn with_time(mut self, time: f64, increment: f64) -> Result<PointContext> {
        if !time.is_finite() {
            return Err(FemError::InvalidValue {
                key: "time",
                reason: format!("{time} must be finite"),
            });
        }
        if !(increment >= 0.0 && increment.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "time_increment",
                reason: format!("{increment} must be non-negative and finite"),
            });
        }
        self.time = Some(time);
        self.time_increment = Some(increment);
        Ok(self)
    }
    /// The same context with material axes; refuses axes that fail [`check_axes`].
    pub fn with_axes(mut self, axes: Axes) -> Result<PointContext> {
        check_axes(&axes)?;
        self.axes = Some(axes);
        Ok(self)
    }
    /// Whether every field is `None`.
    pub fn is_default(&self) -> bool {
        *self == PointContext::default()
    }
}

/// The rule that gives the material axes of an orthotropic material (ADR 0310 section 2.3). Plain data.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MaterialAxes {
    /// One triad for every point.
    Fixed(Axes),
    /// Cylindrical axes about a line: radial, tangential, axial.
    #[non_exhaustive]
    Cylindrical {
        /// A point of the axis.
        origin: Vec3,
        /// The direction of the axis (non-zero).
        axis: Vec3,
    },
    /// One triad per element of the material's element set, in the set's order.
    PerElement(Vec<Axes>),
}

impl MaterialAxes {
    /// One triad; refuses axes that fail [`check_axes`].
    pub fn fixed(axes: Axes) -> Result<MaterialAxes> {
        check_axes(&axes)?;
        Ok(MaterialAxes::Fixed(axes))
    }
    /// Cylindrical axes; refuses a non-finite origin and a zero or non-finite axis.
    pub fn cylindrical(origin: Vec3, axis: Vec3) -> Result<MaterialAxes> {
        if !origin.iter().chain(&axis).all(|v| v.is_finite()) || axis.iter().all(|&v| v == 0.0) {
            return Err(FemError::InvalidValue {
                key: "axes",
                reason: format!("origin {origin:?} and axis {axis:?} must be finite and the axis non-zero"),
            });
        }
        Ok(MaterialAxes::Cylindrical { origin, axis })
    }
    /// One triad per element; refuses an empty list and any triad that fails [`check_axes`].
    pub fn per_element(axes: Vec<Axes>) -> Result<MaterialAxes> {
        if axes.is_empty() {
            return Err(FemError::InvalidValue {
                key: "axes",
                reason: "a list of per-element axes needs at least one triad".to_string(),
            });
        }
        for a in &axes {
            check_axes(a)?;
        }
        Ok(MaterialAxes::PerElement(axes))
    }
}

/// One term of a Prony series (ADR 0313): relative shear and bulk weights in [0, 1] and the relaxation time, s.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PronyTerm {
    /// Relative weight of the shear modulus.
    pub shear: f64,
    /// Relative weight of the bulk modulus.
    pub bulk: f64,
    /// Relaxation time, s, positive.
    pub tau: f64,
}

impl PronyTerm {
    /// A term; refuses a weight outside [0, 1], both weights 0 (a branch without modulus, key `"shear"`, ADR 0485
    /// arithmetic 8) and a relaxation time that is not positive and finite.
    pub fn new(shear: f64, bulk: f64, tau: f64) -> Result<PronyTerm> {
        for (key, v) in [("shear", shear), ("bulk", bulk)] {
            if !(0.0..=1.0).contains(&v) {
                return Err(FemError::InvalidMaterial {
                    key,
                    value: v,
                    reason: "a Prony weight must lie in [0, 1]",
                });
            }
        }
        if shear == 0.0 && bulk == 0.0 {
            return Err(FemError::InvalidMaterial {
                key: "shear",
                value: shear,
                reason: "a Prony term needs a positive shear or bulk weight (a branch of zero modulus)",
            });
        }
        positive_material("tau", tau)?;
        Ok(PronyTerm { shear, bulk, tau })
    }
}

/// A time-temperature shift (ADR 0313). Plain data.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TimeTemperatureShift {
    /// Williams-Landel-Ferry: `log10 a_T = -c1 (T - T_ref) / (c2 + T - T_ref)`.
    #[non_exhaustive]
    Wlf {
        /// Reference temperature, K.
        reference: f64,
        /// `c1`.
        c1: f64,
        /// `c2`, K, positive.
        c2: f64,
    },
    /// Arrhenius: `ln a_T = (activation / R) (1/T - 1/T_ref)`.
    #[non_exhaustive]
    Arrhenius {
        /// Reference temperature, K.
        reference: f64,
        /// Activation energy, J/mol, positive.
        activation: f64,
    },
}

impl TimeTemperatureShift {
    /// The WLF shift; refuses non-finite values and `c2 <= 0`.
    pub fn wlf(reference: f64, c1: f64, c2: f64) -> Result<TimeTemperatureShift> {
        finite_material("reference", reference)?;
        finite_material("c1", c1)?;
        positive_material("c2", c2)?;
        Ok(TimeTemperatureShift::Wlf { reference, c1, c2 })
    }
    /// The Arrhenius shift; refuses a non-finite reference and an activation energy that is not positive and finite.
    pub fn arrhenius(reference: f64, activation: f64) -> Result<TimeTemperatureShift> {
        finite_material("reference", reference)?;
        positive_material("activation", activation)?;
        Ok(TimeTemperatureShift::Arrhenius { reference, activation })
    }
}

/// The strain energy of a hyperelastic material (ADR 0328). Plain data.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum HyperelasticEnergy {
    /// Neo-Hookean with shear modulus `mu` and bulk modulus `kappa`, Pa.
    #[non_exhaustive]
    NeoHookean {
        /// `mu`.
        mu: f64,
        /// `kappa`.
        kappa: f64,
    },
    /// Mooney-Rivlin with `c10`, `c01` and the bulk modulus `kappa`, Pa.
    #[non_exhaustive]
    MooneyRivlin {
        /// `c10`.
        c10: f64,
        /// `c01`.
        c01: f64,
        /// `kappa`.
        kappa: f64,
    },
}

impl HyperelasticEnergy {
    /// Neo-Hookean; refuses `mu` or `kappa` not positive and finite.
    pub fn neo_hookean(mu: f64, kappa: f64) -> Result<HyperelasticEnergy> {
        positive_material("mu", mu)?;
        positive_material("kappa", kappa)?;
        Ok(HyperelasticEnergy::NeoHookean { mu, kappa })
    }
    /// Mooney-Rivlin; refuses non-finite constants, `c10 + c01 <= 0` and `kappa` not positive and finite.
    pub fn mooney_rivlin(c10: f64, c01: f64, kappa: f64) -> Result<HyperelasticEnergy> {
        finite_material("c10", c10)?;
        finite_material("c01", c01)?;
        if c10 + c01 <= 0.0 {
            return Err(FemError::InvalidMaterial {
                key: "c10",
                value: c10,
                reason: "c10 + c01 must be positive",
            });
        }
        positive_material("kappa", kappa)?;
        Ok(HyperelasticEnergy::MooneyRivlin { c10, c01, kappa })
    }
}

pub(crate) fn finite_material(key: &'static str, v: f64) -> Result<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidMaterial {
            key,
            value: v,
            reason: "must be finite",
        })
    }
}

pub(crate) fn positive_material(key: &'static str, v: f64) -> Result<()> {
    if v > 0.0 && v.is_finite() {
        Ok(())
    } else {
        Err(FemError::InvalidMaterial {
            key,
            value: v,
            reason: "must be positive and finite",
        })
    }
}

impl Material {
    /// The elasticity matrix of a linear elastic material. A variant whose package has not landed is refused naming it. An
    /// orthotropic material with `Fixed` axes gives its global matrix (ADR 0452 D3); the other cases of `Orthotropic` and
    /// `Tabulated` vary by point and are refused naming [`crate::mat_ortho::FOLLOW_UP`].
    pub fn elasticity_matrix(&self) -> Result<Matrix6> {
        match self {
            Material::IsotropicElastic { e, nu, .. } => Ok(isotropic_elasticity(*e, *nu)),
            Material::Orthotropic {
                axes: MaterialAxes::Fixed(_),
                ..
            } => self.elasticity_at(&PointContext::default()),
            Material::Orthotropic { .. } | Material::Tabulated { .. } => Err(FemError::NotYetSupported {
                item: "material",
                variant: self.name(),
                location: "the element-constant elasticity matrix".to_string(),
                package: crate::mat_ortho::FOLLOW_UP,
            }),
            // ADR 0475 D2: J2 has no element-constant matrix; only the nonlinear static solve takes it.
            Material::J2 { .. } => Err(FemError::NotYetSupported {
                item: "material",
                variant: self.name(),
                location: "the element-constant elasticity matrix".to_string(),
                package: crate::mat_j2::FOLLOW_UP,
            }),
            // ADR 0485 D2: nor has the viscoelastic material; only the nonlinear driver at a point context takes it.
            Material::Viscoelastic { .. } => Err(FemError::NotYetSupported {
                item: "material",
                variant: self.name(),
                location: "the element-constant elasticity matrix".to_string(),
                package: crate::mat_visco::FOLLOW_UP,
            }),
            // ADR 0489 D2: nor has the creep material.
            Material::Creep { .. } => Err(FemError::NotYetSupported {
                item: "material",
                variant: self.name(),
                location: "the element-constant elasticity matrix".to_string(),
                package: crate::mat_creep::FOLLOW_UP,
            }),
            _ => Err(self.not_yet("the constitutive call")),
        }
    }

    /// The elasticity matrix at a point (ADR 0452 D3): isotropic `isotropic_elasticity(e, nu)`; orthotropic the material
    /// stiffness rotated to the point's axes (`point.axes`, else the `Fixed` triad; cylindrical and per-element axes without
    /// `point.axes` are refused, key `"axes"`); tabulated the base's at `point.temperature` (refused without one, key
    /// `"temperature"`, and outside the table). Other variants as [`Material::elasticity_matrix`].
    pub fn elasticity_at(&self, point: &PointContext) -> Result<Matrix6> {
        match self {
            Material::Orthotropic { e, nu, g, axes, .. } => {
                crate::mat_ortho::orthotropic_elasticity(*e, *nu, *g, axes, point)
            }
            Material::Tabulated { base, .. } => {
                let t = point.temperature.ok_or_else(crate::mat_ortho::needs_temperature)?;
                match crate::mat_table::constants_at(self, t)? {
                    crate::mat_table::Constants::Isotropic(e, nu) => Ok(isotropic_elasticity(e, nu)),
                    crate::mat_table::Constants::Orthotropic(e, nu, g) => match base.as_ref() {
                        Material::Orthotropic { axes, .. } => {
                            crate::mat_ortho::orthotropic_elasticity(e, nu, g, axes, point)
                        }
                        _ => unreachable!("orthotropic constants come from an orthotropic base"),
                    },
                }
            }
            _ => self.elasticity_matrix(),
        }
    }

    /// Number of state values per integration point (ADR 0440 D3): 0 for the elastic and hyperelastic variants, the
    /// base's for `Tabulated`, the declared layouts of [`crate::mat_visco`], [`crate::mat_creep`] and [`crate::mat_j2`].
    pub fn state_size(&self) -> usize {
        match self {
            Material::IsotropicElastic { .. } => 0,
            Material::Orthotropic { .. } | Material::Hyperelastic { .. } => 0,
            Material::Tabulated { base, .. } => base.state_size(),
            Material::Viscoelastic { terms, .. } => crate::mat_visco::state_size(terms.len()),
            Material::Creep { .. } => crate::mat_creep::STATE_SIZE,
            Material::J2 { .. } => crate::mat_j2::STATE_SIZE,
        }
    }

    /// The constitutive call: stress and tangent for a strain (Voigt, engineering shear) at a point with its state
    /// (`state_size()` values). For linear elasticity the stress is `D eps` and the tangent `D`; the isotropic material
    /// does not read the point context. For J2 the return map of [`crate::mat_j2::return_map`] from the committed state
    /// `state` (ADR 0475 D2); for the viscoelastic material the Prony update of [`crate::mat_visco::update`] from the
    /// committed state over the step of `point` (ADR 0485 D2); for the creep material the return of
    /// [`crate::mat_creep::update`] from the committed state over the step of `point` (ADR 0489 D2). A variant whose package
    /// has not landed is refused naming it.
    pub fn stress(&self, strain: &[f64; 6], point: &PointContext, state: &[f64]) -> Result<([f64; 6], Matrix6)> {
        let _ = point;
        if self.pending().is_some() {
            return Err(self.not_yet("the constitutive call"));
        }
        if state.len() != self.state_size() {
            return Err(FemError::FieldLength {
                key: "state",
                expected: self.state_size(),
                found: state.len(),
            });
        }
        if let Material::J2 { .. } = self {
            // ADR 0475 D2: the return map from the committed state (the state itself is not changed here).
            let u = crate::mat_j2::update(self, strain, state)?;
            return Ok((u.stress, u.tangent));
        }
        if let Material::Viscoelastic { .. } = self {
            // ADR 0485 D2: the Prony update from the committed state over the step of the point context.
            let u = crate::mat_visco::update(self, strain, state, point)?;
            return Ok((u.stress, u.tangent));
        }
        if let Material::Creep { .. } = self {
            // ADR 0489 D2: the creep return from the committed state over the step of the point context.
            let u = crate::mat_creep::update(self, strain, state, point)?;
            return Ok((u.stress, u.tangent));
        }
        let d = match self {
            // ADR 0452 D3: the two per-point variants read the point.
            Material::Orthotropic { .. } | Material::Tabulated { .. } => self.elasticity_at(point)?,
            _ => self.elasticity_matrix()?,
        };
        Ok((apply(&d, strain), d))
    }

    /// The refusal of a variant whose package has not landed, at `location`.
    fn not_yet(&self, location: &str) -> FemError {
        FemError::NotYetSupported {
            item: "material",
            variant: self.name(),
            location: location.to_string(),
            package: self.pending().unwrap_or("its package"),
        }
    }
}
