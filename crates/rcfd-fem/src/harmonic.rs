//! Harmonic response by mode superposition: modal damping, static correction, effective mass, force harmonics. Declared
//! by ADR 0291; filled by ADR 0391 (the registration of F-399; design stub ADR 0302).
//!
//! The steady response `x(t) = Re(X e^{i omega t})` to a load `Re(F e^{i omega t})` of the system reduced by the model's zero
//! prescribed displacements, from M-orthonormal modes `phi_r` ([`ModalBasis`]) with modal damping ([`ModalDamping`]; ADR 0391
//! D1):
//!
//! `X = sum_r phi_r H_r(omega) phi_r^T F + (K^{-1} F - sum_r phi_r phi_r^T F / lambda_r)`,
//! `H_r = 1 / (lambda_r - omega^2 + i c_r omega)`, `c_r = 2 zeta_r omega_r`,
//!
//! the bracket being the static correction of the truncated modes (residual flexibility, from the spectral expansion
//! `K^{-1} = sum_{all r} phi_r phi_r^T / lambda_r`; ADR 0391 D2), optional and on by default. At equation `j` the amplitude
//! is `|X_j|` and the phase `arg X_j` (`x_j(t) = |X_j| cos(omega t + arg X_j)`).
//!
//! Also: participation factors `Gamma_r = phi_r^T M r_d` and effective masses `Gamma_r^2` of a rigid translation `r_d`
//! ([`participation`], D4), and the Fourier harmonics of a uniformly sampled record of a whole number of periods
//! ([`force_harmonics`], D5), whose [`ForceHarmonic::complex`] amplitude scales a load pattern ([`HarmonicLoad::from_harmonic`]).
//!
//! ADR 0400 (F-491) extends the static correction: for a free body (free rigid motions counted by
//! [`crate::free_motions::free_rigid_motions`]) by inertia relief, the elastic residual flexibility `P^T K_c^{-1} P F` with
//! `P = I - M R R^T` and `K_c` the stiffness on a statically determinate support (D1); on the transformation path by the
//! reduced solve `T K_r^{-1} T^T F` (D2); and with a pre-stressed basis by the stiffness `K + K_sigma` the modes were
//! computed with ([`HarmonicOptions::prestress`], D3). Models without these features take the code of ADR 0391 unchanged.

use crate::assemble::stiffness_with;
use crate::constraint::{prescribed_equations, reduce};
use crate::error::{FemError, Result};
use crate::free_motions::free_rigid_motions;
use crate::geometric::{add_on_pattern, geometric_stiffness, Prestress};
use crate::loads::nodal_load_vector;
use crate::mass::{consistent_with, rigid_motions};
use crate::modal::ModalResult;
use crate::model::{Constraint, DofMap, SolidModel};
use crate::mpc::Transformation;
use crate::recover::IntegrationPointField;
use crate::solve::PIVOT_TOLERANCE;
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::generalized_eigen_dense;
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// The largest number of free unknowns of [`ModalBasis::all_modes`] (a dense eigensolution).
pub const MAX_DENSE_UNKNOWNS: usize = 600;

/// An undamped mode with `|lambda_r - omega^2| <= RESONANCE_TOLERANCE lambda_r` is a resonance without damping (refused).
pub const RESONANCE_TOLERANCE: f64 = 1e-12;

/// Inertia relief (ADR 0400 D1): a basis mode with rigid weight `|R^T M phi|^2 > 1 - RIGID_SEPARATION` is a rigid mode, one
/// below `RIGID_SEPARATION` an elastic mode; a mode in between is refused.
pub const RIGID_SEPARATION: f64 = 1e-6;

/// A record spans a whole number `P` of periods when `|N dt / T - P| <= WHOLE_PERIOD_TOLERANCE max(1, P)`.
pub const WHOLE_PERIOD_TOLERANCE: f64 = 1e-9;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// The zero prescribed equations of a model not on the transformation path; `key` names the refusal.
fn zero_prescribed(model: &SolidModel, key: &'static str, what: &str) -> Result<Vec<(usize, f64)>> {
    model.validate()?;
    model.check_supported()?;
    if crate::mpc::uses_transformation(model) {
        return Err(invalid(
            key,
            format!(
                "linear constraints, rigid sets, cyclic constraints and elastic supports are not taken by {what} (ADR 0391: \
                 the transformation path is not included)"
            ),
        ));
    }
    let fixed = prescribed_equations(model, &[])?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = model.dof_map().node_component(eq).expect("existing equation");
        return Err(invalid(
            key,
            format!("prescribed value {v:e} at node {node} component {component}: {what} takes zero values only"),
        ));
    }
    Ok(fixed)
}

// ----------------------------------------------------------------------------------------------------------- basis

/// M-orthonormal modes over a model's equations (0 at prescribed equations) with their eigenvalues `lambda = omega^2`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalBasis {
    eigenvalues: Vec<f64>,
    modes: Vec<Vec<f64>>,
    dofs: DofMap,
}

impl ModalBasis {
    /// A basis from its parts: as many modes as eigenvalues, each of `dofs.n_equations()` finite values, finite eigenvalues.
    /// The M-orthonormality is the caller's.
    pub fn new(eigenvalues: Vec<f64>, modes: Vec<Vec<f64>>, dofs: DofMap) -> Result<ModalBasis> {
        if modes.len() != eigenvalues.len() {
            return Err(FemError::FieldLength {
                key: "modal basis",
                expected: eigenvalues.len(),
                found: modes.len(),
            });
        }
        if eigenvalues.is_empty() {
            return Err(invalid("modal basis", "no mode".to_string()));
        }
        let n = dofs.n_equations();
        for (r, (l, phi)) in eigenvalues.iter().zip(&modes).enumerate() {
            if !l.is_finite() {
                return Err(invalid("modal basis", format!("eigenvalue {r} is {l:e}")));
            }
            if phi.len() != n {
                return Err(FemError::FieldLength {
                    key: "modal basis",
                    expected: n,
                    found: phi.len(),
                });
            }
            if let Some(i) = phi.iter().position(|x| !x.is_finite()) {
                return Err(invalid(
                    "modal basis",
                    format!("mode {r} is not finite at equation {i}"),
                ));
            }
        }
        Ok(ModalBasis {
            eigenvalues,
            modes,
            dofs,
        })
    }

    /// The modes of a modal analysis (also of [`crate::mpc::modal_analysis_mpc`] and of the cyclic expansion when it is a
    /// [`ModalResult`]).
    pub fn from_result(result: &ModalResult) -> Result<ModalBasis> {
        ModalBasis::new(result.eigenvalues.clone(), result.modes.clone(), result.dofs().clone())
    }

    /// Every mode of a small model (at most [`MAX_DENSE_UNKNOWNS`] free unknowns) by the dense eigensolution of the reduced
    /// `K` and `M` (Cholesky of `M` and Jacobi, `rcfd_linalg::eigen::generalized_eigen_dense`); a free body is taken (its
    /// rigid modes are in the basis). Zero prescribed values only; the transformation path is refused.
    pub fn all_modes(model: &SolidModel, threads: Option<usize>) -> Result<ModalBasis> {
        let fixed = zero_prescribed(model, "constraint", "the dense modal basis")?;
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        let free_count = n - {
            let mut is_fixed = vec![false; n];
            for &(e, _) in &fixed {
                is_fixed[e] = true;
            }
            is_fixed.iter().filter(|&&b| b).count()
        };
        if free_count == 0 || free_count > MAX_DENSE_UNKNOWNS {
            return Err(invalid(
                "modes",
                format!(
                    "every mode of a model with {free_count} free unknowns is asked (dense: 1 to {MAX_DENSE_UNKNOWNS})"
                ),
            ));
        }
        let k = stiffness_with(model, threads)?;
        let m = consistent_with(model, threads)?;
        let zero = vec![0.0; n];
        let kr = reduce(&k, &zero, &fixed)?;
        let mr = reduce(&m, &zero, &fixed)?;
        let free = kr.free().to_vec();
        let nf = free.len();
        let dense = |a: &CsrMatrix| {
            let mut d = vec![0.0; nf * nf];
            for r in 0..a.n {
                for p in a.row_ptr[r]..a.row_ptr[r + 1] {
                    d[r * nf + a.col[p]] = a.val[p];
                }
            }
            d
        };
        let eig =
            generalized_eigen_dense(&dense(&kr.matrix), &dense(&mr.matrix), nf).map_err(|e| FemError::Solver {
                solver: "generalized_eigen_dense",
                reason: e.to_string(),
            })?;
        let modes = eig
            .vectors
            .iter()
            .map(|v| {
                let mut full = vec![0.0; n];
                for (&eq, &x) in free.iter().zip(v) {
                    full[eq] = x;
                }
                full
            })
            .collect();
        ModalBasis::new(eig.values, modes, dofs)
    }

    /// The eigenvalues `lambda_r = omega_r^2`.
    pub fn eigenvalues(&self) -> &[f64] {
        &self.eigenvalues
    }

    /// The circular frequencies `omega_r = sqrt(max(lambda_r, 0))`, rad/s.
    pub fn omegas(&self) -> Vec<f64> {
        self.eigenvalues.iter().map(|&l| l.max(0.0).sqrt()).collect()
    }

    /// The modes, one value per equation of [`ModalBasis::dofs`].
    pub fn modes(&self) -> &[Vec<f64>] {
        &self.modes
    }

    /// The unknown numbering of the modes.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }

    /// The number of modes.
    pub fn len(&self) -> usize {
        self.modes.len()
    }

    /// Whether the basis has no mode (never, by construction).
    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    fn check_model(&self, model: &SolidModel) -> Result<()> {
        let dofs = model.dof_map();
        if dofs.n_equations() != self.dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "modal basis",
                expected: dofs.n_equations(),
                found: self.dofs.n_equations(),
            });
        }
        if dofs != self.dofs {
            return Err(invalid(
                "modal basis",
                "the basis numbers the unknowns differently from the model".to_string(),
            ));
        }
        Ok(())
    }
}

// --------------------------------------------------------------------------------------------------------- damping

/// Modal damping (ADR 0391 D1): the term `c_r = 2 zeta_r omega_r` of each mode.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ModalDamping {
    /// The same ratio `zeta` for every mode.
    Ratio(f64),
    /// One ratio per mode of the basis.
    PerMode(Vec<f64>),
    /// Rayleigh damping `C = mass M + stiffness K`: `2 zeta_r omega_r = mass + stiffness lambda_r`.
    Rayleigh {
        /// The mass-proportional coefficient `alpha`, 1/s.
        mass: f64,
        /// The stiffness-proportional coefficient `beta`, s.
        stiffness: f64,
    },
}

impl Default for ModalDamping {
    fn default() -> Self {
        ModalDamping::Ratio(0.0)
    }
}

fn check_ratio(what: &str, z: f64) -> Result<()> {
    if !(z.is_finite() && z >= 0.0) {
        return Err(invalid("damping", format!("{what} {z:e} (must be finite and >= 0)")));
    }
    Ok(())
}

impl ModalDamping {
    /// The terms `c_r = 2 zeta_r omega_r` of the modes with these eigenvalues.
    pub fn coefficients(&self, eigenvalues: &[f64]) -> Result<Vec<f64>> {
        match self {
            ModalDamping::Ratio(z) => {
                check_ratio("damping ratio", *z)?;
                Ok(eigenvalues.iter().map(|&l| 2.0 * z * l.max(0.0).sqrt()).collect())
            }
            ModalDamping::PerMode(zs) => {
                if zs.len() != eigenvalues.len() {
                    return Err(invalid(
                        "damping",
                        format!("{} ratios for {} modes", zs.len(), eigenvalues.len()),
                    ));
                }
                for (r, &z) in zs.iter().enumerate() {
                    check_ratio(&format!("damping ratio of mode {r}"), z)?;
                }
                Ok(zs
                    .iter()
                    .zip(eigenvalues)
                    .map(|(&z, &l)| 2.0 * z * l.max(0.0).sqrt())
                    .collect())
            }
            ModalDamping::Rayleigh { mass, stiffness } => {
                check_ratio("Rayleigh mass coefficient", *mass)?;
                check_ratio("Rayleigh stiffness coefficient", *stiffness)?;
                Ok(eigenvalues.iter().map(|&l| mass + stiffness * l.max(0.0)).collect())
            }
        }
    }
}

// ----------------------------------------------------------------------------------------------------------- loads

/// A complex load vector `F = re + i im` (one value per equation; the load is `Re(F e^{i omega t})`).
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicLoad {
    re: Vec<f64>,
    im: Vec<f64>,
}

impl HarmonicLoad {
    /// A load from its real and imaginary parts (equal lengths, finite).
    pub fn new(re: Vec<f64>, im: Vec<f64>) -> Result<HarmonicLoad> {
        if re.len() != im.len() {
            return Err(FemError::FieldLength {
                key: "harmonic load",
                expected: re.len(),
                found: im.len(),
            });
        }
        for (part, v) in [("real", &re), ("imaginary", &im)] {
            if let Some(i) = v.iter().position(|x| !x.is_finite()) {
                return Err(invalid(
                    "harmonic load",
                    format!("{part} part {:e} at equation {i} is not finite", v[i]),
                ));
            }
        }
        Ok(HarmonicLoad { re, im })
    }

    /// A real load (in phase with `cos omega t`).
    pub fn real(re: Vec<f64>) -> Result<HarmonicLoad> {
        let im = vec![0.0; re.len()];
        HarmonicLoad::new(re, im)
    }

    /// The model's own loads ([`crate::loads::nodal_load_vector`]) as a real load.
    pub fn from_model(model: &SolidModel) -> Result<HarmonicLoad> {
        HarmonicLoad::real(nodal_load_vector(model, &[])?)
    }

    /// A pattern scaled by a force harmonic: `pattern A e^{i phi}`.
    pub fn from_harmonic(pattern: &[f64], harmonic: &ForceHarmonic) -> Result<HarmonicLoad> {
        let (c, s) = harmonic.complex();
        HarmonicLoad::new(
            pattern.iter().map(|p| p * c).collect(),
            pattern.iter().map(|p| p * s).collect(),
        )
    }

    /// The real part.
    pub fn re(&self) -> &[f64] {
        &self.re
    }

    /// The imaginary part.
    pub fn im(&self) -> &[f64] {
        &self.im
    }
}

// -------------------------------------------------------------------------------------------------------- response

/// The steady response at one frequency: `x(t) = Re((re + i im) e^{i omega t})`, one value per equation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicResponse {
    /// The circular frequency, rad/s.
    pub omega: f64,
    /// The real part (the response in phase with `cos omega t`).
    pub re: Vec<f64>,
    /// The imaginary part.
    pub im: Vec<f64>,
}

impl HarmonicResponse {
    /// The amplitude `|X_j|` at equation `j`.
    pub fn amplitude(&self, j: usize) -> f64 {
        self.re[j].hypot(self.im[j])
    }

    /// The phase `arg X_j` at equation `j`, in `(-pi, pi]` (a lag behind a real load is `-phase`).
    pub fn phase(&self, j: usize) -> f64 {
        self.im[j].atan2(self.re[j])
    }

    /// The stress of the real and imaginary parts at the integration points of [`IntegrationPointField`], each
    /// `sigma(x_part) - sigma(0)` (the stress is affine in the displacement; a thermal strain of the model is removed).
    pub fn stress(&self, model: &SolidModel) -> Result<HarmonicStress> {
        let zero = IntegrationPointField::compute(model, &vec![0.0; self.re.len()])?;
        let fr = IntegrationPointField::compute(model, &self.re)?;
        let fi = IntegrationPointField::compute(model, &self.im)?;
        let diff = |a: &[[f64; 6]], b: &[[f64; 6]]| -> Vec<[f64; 6]> {
            a.iter()
                .zip(b)
                .map(|(x, y)| std::array::from_fn(|c| x[c] - y[c]))
                .collect()
        };
        Ok(HarmonicStress {
            re: diff(fr.stress(), zero.stress()),
            im: diff(fi.stress(), zero.stress()),
            positions: fr.positions().to_vec(),
        })
    }
}

/// The harmonic stress at the integration points: `sigma(t) = Re((re + i im) e^{i omega t})` per component.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicStress {
    /// The real part, six components per point (xx, yy, zz, xy, yz, zx).
    pub re: Vec<[f64; 6]>,
    /// The imaginary part.
    pub im: Vec<[f64; 6]>,
    /// The positions of the points.
    pub positions: Vec<Vec3>,
}

impl HarmonicStress {
    /// The amplitude of component `c` at point `p`.
    pub fn amplitude(&self, p: usize, c: usize) -> f64 {
        self.re[p][c].hypot(self.im[p][c])
    }

    /// The phase of component `c` at point `p`.
    pub fn phase(&self, p: usize, c: usize) -> f64 {
        self.im[p][c].atan2(self.re[p][c])
    }
}

/// Options of a harmonic response.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicOptions {
    /// The modal damping (default: none).
    pub damping: ModalDamping,
    /// Whether the static correction of the truncated modes is added (default true).
    pub static_correction: bool,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
    /// The pre-stress the basis was computed with ([`crate::geometric::prestressed_modes`]); the static correction then
    /// uses `K + K_sigma` (ADR 0400 D3). Default `None` (the elastic `K`). Not used without the static correction.
    pub prestress: Option<Prestress>,
}

impl Default for HarmonicOptions {
    fn default() -> Self {
        HarmonicOptions {
            damping: ModalDamping::default(),
            static_correction: true,
            threads: None,
            prestress: None,
        }
    }
}

impl HarmonicOptions {
    /// The default options with a damping.
    pub fn new(damping: ModalDamping) -> HarmonicOptions {
        HarmonicOptions {
            damping,
            ..HarmonicOptions::default()
        }
    }
    /// The same options with or without the static correction.
    pub fn static_correction(mut self, on: bool) -> HarmonicOptions {
        self.static_correction = on;
        self
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> HarmonicOptions {
        self.threads = Some(threads);
        self
    }
    /// The same options with the pre-stress of the basis for the static correction (ADR 0400 D3).
    pub fn prestress(mut self, prestress: Prestress) -> HarmonicOptions {
        self.prestress = Some(prestress);
        self
    }
}

#[derive(Debug, Clone)]
enum StaticCorrection {
    /// ADR 0391 D2: the reduced `K` (prescribed equations removed).
    Plain { ldlt: SkylineLdlt, free: Vec<usize> },
    /// ADR 0400 D1: inertia relief of a free body.
    Relief {
        /// The factor of `K_c` (prescribed and support equations removed).
        ldlt: SkylineLdlt,
        /// The equations of `K_c`.
        kept: Vec<usize>,
        /// `R`, M-orthonormal, over every equation (0 at prescribed ones).
        rigid: Vec<Vec<f64>>,
        /// `M R`.
        m_rigid: Vec<Vec<f64>>,
        /// Whether each basis mode is elastic (subtracted) or rigid (skipped).
        elastic: Vec<bool>,
    },
    /// ADR 0400 D2: the reduced `K_r = T^T K_ext T`.
    Transformation { ldlt: SkylineLdlt, tr: Transformation },
}

/// A load prepared for a sweep: its modal forces and its static correction.
struct Prepared {
    modal_re: Vec<f64>,
    modal_im: Vec<f64>,
    residual: Option<(Vec<f64>, Vec<f64>)>,
}

/// Mode superposition of a model's steady harmonic response (ADR 0391 D1 to D3).
#[derive(Debug, Clone)]
pub struct HarmonicSolver {
    basis: ModalBasis,
    coefficients: Vec<f64>,
    correction: Option<StaticCorrection>,
}

impl HarmonicSolver {
    /// The solver of a model from a basis of its modes. With the static correction the reduced `K` is factored (refused for
    /// a non-zero prescribed value and for a basis with an eigenvalue `<= 0`; key `"static correction"`). Since ADR 0400 a
    /// free body takes inertia relief (its rigid modes must be in the basis), a model on the transformation path the
    /// reduced `T^T K_ext T` (a free rigid motion and cyclic constraints refused there), and
    /// [`HarmonicOptions::prestress`] adds `K_sigma` to the stiffness of the correction.
    pub fn new(model: &SolidModel, basis: ModalBasis, options: &HarmonicOptions) -> Result<HarmonicSolver> {
        basis.check_model(model)?;
        let coefficients = options.damping.coefficients(&basis.eigenvalues)?;
        let correction = if options.static_correction {
            if crate::mpc::uses_transformation(model) {
                Some(transformation_correction(model, &basis, options)?)
            } else {
                let fixed = zero_prescribed(model, "static correction", "the static correction")?;
                let free_motions = free_rigid_motions(model, &[]).map_or(0, |f| f.count);
                if free_motions > 0 {
                    Some(relief_correction(model, &basis, options, &fixed)?)
                } else {
                    if let Some((r, l)) = basis.eigenvalues.iter().enumerate().find(|(_, &l)| l <= 0.0) {
                        return Err(invalid(
                            "static correction",
                            format!("mode {r} has the eigenvalue {l:e} (a rigid motion: K^-1 does not exist)"),
                        ));
                    }
                    let k = stiffness_with(model, options.threads)?;
                    let k = match &options.prestress {
                        Some(p) => add_on_pattern(&k, &geometric_stiffness(model, p, options.threads)?),
                        None => k,
                    };
                    let kr = reduce(&k, &vec![0.0; k.n], &fixed)?;
                    let free = kr.free().to_vec();
                    let dofs = model.dof_map();
                    let ldlt = factor(&kr.matrix, &free, &dofs)?;
                    Some(StaticCorrection::Plain { ldlt, free })
                }
            }
        } else {
            None
        };
        Ok(HarmonicSolver {
            basis,
            coefficients,
            correction,
        })
    }

    /// The basis.
    pub fn basis(&self) -> &ModalBasis {
        &self.basis
    }

    /// The damping terms `c_r = 2 zeta_r omega_r`.
    pub fn damping_coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// Whether the static correction is added.
    pub fn has_static_correction(&self) -> bool {
        self.correction.is_some()
    }

    fn prepare(&self, load: &HarmonicLoad) -> Result<Prepared> {
        let n = self.basis.dofs.n_equations();
        if load.re.len() != n {
            return Err(FemError::FieldLength {
                key: "harmonic load",
                expected: n,
                found: load.re.len(),
            });
        }
        let modal_re: Vec<f64> = self.basis.modes.iter().map(|phi| dot(phi, &load.re)).collect();
        let modal_im: Vec<f64> = self.basis.modes.iter().map(|phi| dot(phi, &load.im)).collect();
        let residual = self.correction.as_ref().map(|sc| {
            let part = |f: &[f64], modal: &[f64]| -> Vec<f64> {
                let (mut full, elastic) = match sc {
                    StaticCorrection::Plain { ldlt, free } => {
                        let rhs: Vec<f64> = free.iter().map(|&e| f[e]).collect();
                        let x = ldlt.solve(&rhs);
                        let mut full = vec![0.0; n];
                        for (&e, &v) in free.iter().zip(&x) {
                            full[e] = v;
                        }
                        (full, None)
                    }
                    StaticCorrection::Relief {
                        ldlt,
                        kept,
                        rigid,
                        m_rigid,
                        elastic,
                    } => (relief_part(ldlt, kept, rigid, m_rigid, f, n), Some(elastic)),
                    StaticCorrection::Transformation { ldlt, tr } => {
                        let mut f_ext = f.to_vec();
                        f_ext.resize(tr.n_unknowns(), 0.0);
                        let q = ldlt.solve(&tr.transpose_times(&f_ext));
                        let mut full = tr.expand(&q);
                        full.truncate(n);
                        (full, None)
                    }
                };
                for (r, ((phi, &l), &q)) in self
                    .basis
                    .modes
                    .iter()
                    .zip(&self.basis.eigenvalues)
                    .zip(modal)
                    .enumerate()
                {
                    if elastic.is_some_and(|el| !el[r]) {
                        continue;
                    }
                    let s = q / l;
                    for (xi, pi) in full.iter_mut().zip(phi) {
                        *xi -= pi * s;
                    }
                }
                full
            };
            (part(&load.re, &modal_re), part(&load.im, &modal_im))
        });
        Ok(Prepared {
            modal_re,
            modal_im,
            residual,
        })
    }

    /// The factors `H_r(omega)` as (re, im) pairs.
    fn transfer(&self, omega: f64) -> Result<Vec<(f64, f64)>> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(invalid("frequency", format!("{omega:e} (must be finite and >= 0)")));
        }
        let w2 = omega * omega;
        let mut h = Vec::with_capacity(self.basis.len());
        for (r, (&l, &c)) in self.basis.eigenvalues.iter().zip(&self.coefficients).enumerate() {
            let a = l - w2;
            let b = c * omega;
            if b == 0.0 && a.abs() <= RESONANCE_TOLERANCE * l.abs() {
                return Err(invalid(
                    "frequency",
                    format!(
                        "omega {omega:e} is the undamped resonance of mode {r} (lambda {l:e}): the steady response does \
                         not exist"
                    ),
                ));
            }
            let d = a * a + b * b;
            h.push((a / d, -b / d));
        }
        Ok(h)
    }

    fn evaluate(&self, p: &Prepared, omega: f64) -> Result<HarmonicResponse> {
        let h = self.transfer(omega)?;
        let n = self.basis.dofs.n_equations();
        let (mut re, mut im) = match &p.residual {
            Some((r, i)) => (r.clone(), i.clone()),
            None => (vec![0.0; n], vec![0.0; n]),
        };
        for (r, phi) in self.basis.modes.iter().enumerate() {
            let (hr, hi) = h[r];
            let (fr, fi) = (p.modal_re[r], p.modal_im[r]);
            let qr = hr * fr - hi * fi;
            let qi = hr * fi + hi * fr;
            for ((x, y), &v) in re.iter_mut().zip(im.iter_mut()).zip(phi) {
                *x += v * qr;
                *y += v * qi;
            }
        }
        Ok(HarmonicResponse { omega, re, im })
    }

    /// The modal amplitudes `Q_r = H_r(omega) phi_r^T F` as (re, im) pairs.
    pub fn modal_amplitudes(&self, load: &HarmonicLoad, omega: f64) -> Result<Vec<(f64, f64)>> {
        let p = self.prepare(load)?;
        let h = self.transfer(omega)?;
        Ok(h.iter()
            .enumerate()
            .map(|(r, &(hr, hi))| {
                let (fr, fi) = (p.modal_re[r], p.modal_im[r]);
                (hr * fr - hi * fi, hr * fi + hi * fr)
            })
            .collect())
    }

    /// The steady response at `omega` (finite, `>= 0`).
    pub fn response(&self, load: &HarmonicLoad, omega: f64) -> Result<HarmonicResponse> {
        let p = self.prepare(load)?;
        self.evaluate(&p, omega)
    }

    /// The responses at several frequencies (the static correction of the load computed once); each has the bits of
    /// [`HarmonicSolver::response`] at its frequency.
    pub fn sweep(&self, load: &HarmonicLoad, omegas: &[f64]) -> Result<Vec<HarmonicResponse>> {
        let p = self.prepare(load)?;
        omegas.iter().map(|&w| self.evaluate(&p, w)).collect()
    }

    /// The modal forces `phi_r^T f` of a real load (one value per equation) and, with the static correction, its residual
    /// `x_s`: the response is `sum_r phi_r H_r(omega) phi_r^T f + x_s` (ADR 0477 D7, for the random response of
    /// [`crate::random`]).
    pub fn modal_decomposition(&self, load: &[f64]) -> Result<(Vec<f64>, Option<Vec<f64>>)> {
        let p = self.prepare(&HarmonicLoad::real(load.to_vec())?)?;
        Ok((p.modal_re, p.residual.map(|(re, _)| re)))
    }
}

/// The factor of the reduced stiffness with the pivot rule of the static solve.
fn factor(a: &CsrMatrix, free: &[usize], dofs: &DofMap) -> Result<SkylineLdlt> {
    factor_named(a, |row: usize| match dofs.node_component(free[row]) {
        Some((node, component)) => format!("reduced equation {row} (node {node} component {component})"),
        None => format!("equation {row}"),
    })
}

/// [`factor`] with the rows named by `at`.
fn factor_named(a: &CsrMatrix, at: impl Fn(usize) -> String) -> Result<SkylineLdlt> {
    let diag = a.diag();
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => invalid(
            "static correction",
            format!(
                "the stiffness is singular or not positive definite at {}: pivot {pivot:e}, diagonal {:e} (a free \
                 rigid motion?)",
                at(row),
                diag[row]
            ),
        ),
        other => invalid("static correction", other.to_string()),
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d <= PIVOT_TOLERANCE * diag[row] {
            return Err(invalid(
                "static correction",
                format!(
                    "the stiffness is singular at {}: pivot {d:e} at most {PIVOT_TOLERANCE:e} of the diagonal {:e} (a \
                     free rigid motion?)",
                    at(row),
                    diag[row]
                ),
            ));
        }
    }
    Ok(ldlt)
}

/// The inertia-relief residual flexibility of one load part (ADR 0400 D1), before the subtraction of the elastic basis
/// modes: `P^T K_c^{-1} P f`, `P = I - M R R^T`.
fn relief_part(
    ldlt: &SkylineLdlt,
    kept: &[usize],
    rigid: &[Vec<f64>],
    m_rigid: &[Vec<f64>],
    f: &[f64],
    n: usize,
) -> Vec<f64> {
    let mut pf = f.to_vec();
    for (r, mr) in rigid.iter().zip(m_rigid) {
        let a = dot(r, f);
        for (p, v) in pf.iter_mut().zip(mr) {
            *p -= v * a;
        }
    }
    let rhs: Vec<f64> = kept.iter().map(|&e| pf[e]).collect();
    let x = ldlt.solve(&rhs);
    let mut y = vec![0.0; n];
    for (&e, &v) in kept.iter().zip(&x) {
        y[e] = v;
    }
    let b: Vec<f64> = m_rigid.iter().map(|mr| dot(mr, &y)).collect();
    for (r, &bj) in rigid.iter().zip(&b) {
        for (yi, v) in y.iter_mut().zip(r) {
            *yi -= v * bj;
        }
    }
    y
}

/// The inertia relief of a free body on the plain path (ADR 0400 D1).
fn relief_correction(
    model: &SolidModel,
    basis: &ModalBasis,
    options: &HarmonicOptions,
    fixed: &[(usize, f64)],
) -> Result<StaticCorrection> {
    let key = "static correction";
    if options.prestress.is_some() {
        return Err(invalid(
            key,
            "a free body with a pre-stress: the null space of K + K_sigma is not the rigid motions (inertia relief of ADR \
             0400 takes the elastic K only)"
                .to_string(),
        ));
    }
    let frm = free_rigid_motions(model, &[])?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut is_fixed = vec![false; n];
    for &(e, _) in fixed {
        is_fixed[e] = true;
    }
    let m = consistent_with(model, options.threads)?;
    // R: the free rigid fields, 0 at the prescribed equations, M-orthonormal (modified Gram-Schmidt, two passes).
    let mut rigid: Vec<Vec<f64>> = Vec::with_capacity(frm.count);
    let mut m_rigid: Vec<Vec<f64>> = Vec::with_capacity(frm.count);
    for mut v in frm.fields(model) {
        for (x, &fx) in v.iter_mut().zip(&is_fixed) {
            if fx {
                *x = 0.0;
            }
        }
        for _ in 0..2 {
            for (r, mr) in rigid.iter().zip(&m_rigid) {
                let c = dot(mr, &v);
                for (x, ri) in v.iter_mut().zip(r) {
                    *x -= c * ri;
                }
            }
        }
        let mut mv = matvec(&m, &v);
        let norm = dot(&v, &mv).sqrt();
        if !(norm.is_finite() && norm > 0.0) {
            return Err(invalid(key, format!("a free rigid motion has the mass norm {norm:e}")));
        }
        for x in v.iter_mut().chain(mv.iter_mut()) {
            *x /= norm;
        }
        rigid.push(v);
        m_rigid.push(mv);
    }
    let n_r = rigid.len();
    // The basis: rigid and elastic modes.
    let mut elastic = Vec::with_capacity(basis.len());
    let mut rigid_modes = 0;
    for (r, (phi, &l)) in basis.modes.iter().zip(&basis.eigenvalues).enumerate() {
        let p: f64 = m_rigid.iter().map(|mr| dot(mr, phi).powi(2)).sum();
        if p > 1.0 - RIGID_SEPARATION {
            rigid_modes += 1;
            elastic.push(false);
        } else if p < RIGID_SEPARATION {
            if l <= 0.0 {
                return Err(invalid(
                    key,
                    format!("elastic mode {r} has the eigenvalue {l:e} (inertia relief needs lambda > 0)"),
                ));
            }
            elastic.push(true);
        } else {
            return Err(invalid(
                key,
                format!(
                    "mode {r} mixes rigid and elastic motion (rigid weight {p:e}, between {RIGID_SEPARATION:e} and 1 - \
                     {RIGID_SEPARATION:e})"
                ),
            ));
        }
    }
    if rigid_modes != n_r {
        return Err(invalid(
            key,
            format!(
                "the basis holds {rigid_modes} rigid modes of the {n_r} free rigid motions of the model (inertia relief \
                 needs every rigid mode in the basis)"
            ),
        ));
    }
    // A statically determinate support: pivoted rows of R over the free equations.
    let free: Vec<usize> = (0..n).filter(|&e| !is_fixed[e]).collect();
    let mut w: Vec<Vec<f64>> = free.iter().map(|&e| rigid.iter().map(|r| r[e]).collect()).collect();
    let mut support = Vec::with_capacity(n_r);
    for _ in 0..n_r {
        let (mut best, mut best_norm) = (usize::MAX, 0.0f64);
        for (i, wi) in w.iter().enumerate() {
            let s = dot(wi, wi).sqrt();
            if s > best_norm {
                best = i;
                best_norm = s;
            }
        }
        if best == usize::MAX {
            return Err(invalid(
                key,
                "no statically determinate support of the rigid motions".to_string(),
            ));
        }
        let u: Vec<f64> = w[best].iter().map(|x| x / best_norm).collect();
        for wi in w.iter_mut() {
            let c = dot(wi, &u);
            for (x, ui) in wi.iter_mut().zip(&u) {
                *x -= c * ui;
            }
        }
        w[best].iter_mut().for_each(|x| *x = 0.0);
        support.push(free[best]);
    }
    let mut removed: Vec<(usize, f64)> = fixed.to_vec();
    removed.extend(support.iter().map(|&e| (e, 0.0)));
    removed.sort_by_key(|&(e, _)| e);
    let k = stiffness_with(model, options.threads)?;
    let kc = reduce(&k, &vec![0.0; k.n], &removed)?;
    let kept = kc.free().to_vec();
    let ldlt = factor(&kc.matrix, &kept, &dofs)?;
    Ok(StaticCorrection::Relief {
        ldlt,
        kept,
        rigid,
        m_rigid,
        elastic,
    })
}

/// The static correction on the transformation path (ADR 0400 D2): `K_r = T^T (K_ext + K_sigma) T` factored.
fn transformation_correction(
    model: &SolidModel,
    basis: &ModalBasis,
    options: &HarmonicOptions,
) -> Result<StaticCorrection> {
    let key = "static correction";
    model.validate()?;
    model.check_supported()?;
    if let Some(i) = model
        .constraints()
        .iter()
        .position(|c| matches!(c, Constraint::Cyclic { .. }))
    {
        return Err(invalid(
            key,
            format!("constraint {i} (Cyclic): the static correction does not take cyclic constraints (ADR 0400)"),
        ));
    }
    crate::mpc::refuse_nonzero_values(model, "the static correction").map_err(|e| match e {
        FemError::InvalidValue { reason, .. } => invalid(key, reason),
        other => other,
    })?;
    if let Ok(f) = free_rigid_motions(model, &[]) {
        if f.count > 0 {
            return Err(invalid(
                key,
                format!(
                    "{} free rigid motions on the transformation path (inertia relief is taken on the path of prescribed \
                     displacements only; ADR 0400)",
                    f.count
                ),
            ));
        }
    }
    if let Some((r, l)) = basis.eigenvalues.iter().enumerate().find(|(_, &l)| l <= 0.0) {
        return Err(invalid(
            key,
            format!("mode {r} has the eigenvalue {l:e} (a rigid motion: K^-1 does not exist)"),
        ));
    }
    let tr = crate::mpc::transformation(model, &[])?;
    let mut k = stiffness_with(model, options.threads)?;
    crate::mpc::add_supports(&mut k, model)?;
    let k = match &options.prestress {
        Some(p) => add_on_pattern(&k, &geometric_stiffness(model, p, options.threads)?),
        None => k,
    };
    let (kr, _) = tr.reduce(&k, &vec![0.0; tr.n_unknowns()])?;
    let ldlt = factor_named(&kr, |row| {
        format!(
            "reduced unknown {row} ({})",
            crate::mpc::unknown_name(&tr, model, tr.independent()[row])
        )
    })?;
    Ok(StaticCorrection::Transformation { ldlt, tr })
}

// --------------------------------------------------------------------------------------------------- participation

/// Participation factors and effective masses of a rigid translation (ADR 0391 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Participation {
    /// The unit direction `d`.
    pub direction: Vec3,
    /// `Gamma_r = phi_r^T M r_d` (kg^(1/2) for mass-normalised modes).
    pub factors: Vec<f64>,
    /// `Gamma_r^2`, kg.
    pub effective_masses: Vec<f64>,
    /// `r_d^T M r_d`: the mass of the whole model (no constraint), kg.
    pub total_mass: f64,
}

impl Participation {
    /// The effective masses divided by the total mass.
    pub fn fractions(&self) -> Vec<f64> {
        self.effective_masses.iter().map(|m| m / self.total_mass).collect()
    }
}

/// The participation factors and effective masses of the modes of a basis for a translation along `direction` (any
/// non-zero finite vector; normalised), with the model's full consistent mass (the support rows included).
pub fn participation(
    model: &SolidModel,
    basis: &ModalBasis,
    direction: Vec3,
    threads: Option<usize>,
) -> Result<Participation> {
    basis.check_model(model)?;
    let norm = dot(&direction, &direction).sqrt();
    if !(norm.is_finite() && norm > 0.0) {
        return Err(invalid(
            "direction",
            format!("{direction:?} (must be finite and non-zero)"),
        ));
    }
    let d = [direction[0] / norm, direction[1] / norm, direction[2] / norm];
    let m = consistent_with(model, threads)?;
    let rigid = rigid_motions(model, [0.0; 3]);
    let r: Vec<f64> = (0..m.n)
        .map(|i| d[0] * rigid[0][i] + d[1] * rigid[1][i] + d[2] * rigid[2][i])
        .collect();
    let mr = matvec(&m, &r);
    let total_mass = dot(&r, &mr);
    let factors: Vec<f64> = basis.modes.iter().map(|phi| dot(phi, &mr)).collect();
    Ok(Participation {
        direction: d,
        effective_masses: factors.iter().map(|g| g * g).collect(),
        factors,
        total_mass,
    })
}

// -------------------------------------------------------------------------------------------------- force harmonics

/// One harmonic of a periodic record: `A cos(order Omega t + phi)`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ForceHarmonic {
    /// The order `h` (1 the fundamental).
    pub order: usize,
    /// The circular frequency `h Omega = 2 pi h / T`, rad/s.
    pub omega: f64,
    /// The amplitude `A >= 0`.
    pub amplitude: f64,
    /// The phase `phi` in `(-pi, pi]`, relative to `t = 0`.
    pub phase: f64,
}

impl ForceHarmonic {
    /// The complex amplitude `A e^{i phi}` as (re, im): `Re(A e^{i phi} e^{i omega t}) = A cos(omega t + phi)`.
    pub fn complex(&self) -> (f64, f64) {
        (self.amplitude * self.phase.cos(), self.amplitude * self.phase.sin())
    }
}

/// The mean and harmonics of a periodic record.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ForceSpectrum {
    /// The mean `a_0`.
    pub mean: f64,
    /// The number of whole periods the record spans.
    pub periods: usize,
    /// Orders 1 to the number asked.
    pub harmonics: Vec<ForceHarmonic>,
}

/// An angle wrapped to `(-pi, pi]`.
pub fn wrap_phase(x: f64) -> f64 {
    let w = x.rem_euclid(2.0 * PI);
    if w > PI {
        w - 2.0 * PI
    } else {
        w
    }
}

/// The mean and the harmonics of orders 1 to `orders` of the record `samples[j] = f(t0 + j dt)` of a signal of period
/// `period` (ADR 0391 D5). The record must span a whole number `P` of periods (`N dt = P T` within
/// [`WHOLE_PERIOD_TOLERANCE`]) and each order must lie below the Nyquist bin (`2 h P < N`); refused with key `"force
/// record"` otherwise.
pub fn force_harmonics(samples: &[f64], t0: f64, dt: f64, period: f64, orders: usize) -> Result<ForceSpectrum> {
    let key = "force record";
    let n = samples.len();
    if n < 2 {
        return Err(invalid(key, format!("{n} samples (at least 2)")));
    }
    if let Some(j) = samples.iter().position(|x| !x.is_finite()) {
        return Err(invalid(key, format!("sample {j} is {:e}", samples[j])));
    }
    if !t0.is_finite() {
        return Err(invalid(key, format!("start time {t0:e} is not finite")));
    }
    for (what, v) in [("sample interval", dt), ("period", period)] {
        if !(v.is_finite() && v > 0.0) {
            return Err(invalid(key, format!("{what} {v:e} (must be finite and > 0)")));
        }
    }
    if orders == 0 {
        return Err(invalid(key, "no harmonic order asked".to_string()));
    }
    let p = n as f64 * dt / period;
    let pr = p.round();
    if pr < 1.0 || (p - pr).abs() > WHOLE_PERIOD_TOLERANCE * p.max(1.0) {
        return Err(invalid(
            key,
            format!(
                "the record of {n} samples at {dt:e} spans {p} periods of {period:e}, not a whole number (within \
                 {WHOLE_PERIOD_TOLERANCE:e})"
            ),
        ));
    }
    let periods = pr as usize;
    let omega = 2.0 * PI / period;
    let mean = samples.iter().sum::<f64>() / n as f64;
    let mut harmonics = Vec::with_capacity(orders);
    for h in 1..=orders {
        let k = h * periods;
        if 2 * k >= n {
            return Err(invalid(
                key,
                format!("order {h} is DFT bin {k} of {n} samples, not below the Nyquist bin (2 h P < N)"),
            ));
        }
        let (mut cr, mut ci) = (0.0, 0.0);
        for (j, &f) in samples.iter().enumerate() {
            let theta = 2.0 * PI * ((k * j) % n) as f64 / n as f64;
            cr += f * theta.cos();
            ci -= f * theta.sin();
        }
        cr *= 2.0 / n as f64;
        ci *= 2.0 / n as f64;
        harmonics.push(ForceHarmonic {
            order: h,
            omega: h as f64 * omega,
            amplitude: cr.hypot(ci),
            phase: wrap_phase(ci.atan2(cr) - h as f64 * omega * t0),
        });
    }
    Ok(ForceSpectrum {
        mean,
        periods,
        harmonics,
    })
}
