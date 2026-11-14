//! Damped and complex modes. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0450 (F-414, design stub
//! ADR 0317).
//!
//! The roots `lambda` of `(lambda^2 M + lambda C + K + i H) x = 0` projected on `m` undamped modes `Phi` of
//! [`crate::modal::modal_analysis`] (ADR 0450 D3): `K_r = Phi^T K Phi`, `M_r = Phi^T M Phi` (computed, not assumed diagonal),
//! `C_r = a M_r + b K_r` (Rayleigh) plus the projected [`Dashpot`]s, `H_r = sum eta_s Phi^T K_s Phi` of constant loss factors
//! per element set; the `2 m` roots by [`rcfd_linalg::eigen_general::quadratic_eigen_with`]. Reporting: a root is oscillating
//! when `Im lambda > REAL_TOLERANCE |lambda|`; each oscillating pair is reported once by its root with `Im lambda > 0`
//! (with a loss stiffness, whose spectrum is not conjugate-symmetric, the roots with `Im lambda > 0` are the decaying ones of
//! positive frequency); each real root is reported once, non-oscillating, with `Im` set to 0. Damping ratio `-Re lambda /
//! |lambda|`. The elimination path of the modal analysis only (no linear constraints, rigid sets or supports). The assembly
//! takes the thread count; the dense algebra is sequential, so the result is bit-identical for any.
//!
//! ADR 0464 (F-532): with [`DampedOptions::residual_vectors`] the basis gains one static residual vector per dashpot,
//! `K^-1 f_i` of its influence vector `f_i`, M-orthogonalised (classical Gram-Schmidt, twice) against the undamped modes and
//! the vectors kept before it, and M-normalised; a vector whose M-norm falls to `RESIDUAL_DROP` of its own is dropped. Off
//! (the default) the computation is that of ADR 0450, operation for operation.
//!
//! ADR 0467 (F-548): each mode reports `residual_share`, the M-weighted share of its shape in the residual-vector coordinates
//! (0 without residual vectors), and `pseudo` when it exceeds `PSEUDO_SHARE`; the flagged roots are those of the strongly
//! damped coordinates the vectors add, not of the continuum.
//!
//! ADR 0491 (F-577): the viscoelastic material, whose complex modulus depends on the frequency, is refused (through
//! [`SolidModel::check_supported`], naming [`crate::mat_visco::FOLLOW_UP`]); the direct harmonic solve takes it.
//!
//! ADR 0495 (F-582): with [`DampedOptions::viscoelastic_reference`] (a circular frequency `omega_ref`, and the temperature of
//! a shifted material) the viscoelastic material is taken at that one frequency. The basis is the undamped modes of the model
//! with each viscoelastic material replaced by the isotropic elastic material of its storage shear and bulk moduli at
//! `omega_ref`; its loss stiffness `int B^T D'' B`, `D'' = 2 G'' I_dev + K'' 1 (x) 1` of the loss moduli, is projected on that
//! basis and added to the loss stiffness `H` of the equation. Every root carries the moduli at `omega_ref`. Without the option
//! the material is refused as before; the option on a model without the material is refused.
//!
//! ADR 0520 (F-586): with [`DampedOptions::viscoelastic_own_frequency`] as well, each of the lowest reported roots of that
//! frozen solve is iterated to its own frequency: on the same basis (the storage modes at `omega_ref`) the projected problem
//! at `w` takes `K_r(w) = K_rest + sum (G'(w) P^G + K'(w) P^K)` and `H_r(w) = H_eta + sum (G''(w) P^G + K''(w) P^K)`, where
//! `P^G`, `P^K` are the projections of the stiffness of each viscoelastic material for unit shear and bulk moduli, `K_rest`
//! that of the other materials and `H_eta` their constant loss factors; `C_r` (Rayleigh on the storage stiffness at
//! `omega_ref`, dashpots) is kept. Start `w = omega_ref` (the frozen root), update `w <- Im lambda` of the root nearest the
//! previous one, converged when `|dw| <= tolerance w`; not converged within the maximum count is refused. The reported
//! roots are the converged ones, with [`DampedModes::own_frequency`] in the same order.
//! ADR 0529 (F-609): with `OwnFrequency::skip_pseudo(true)` the roots flagged `pseudo` (ADR 0467) in the frozen solve are not
//! iterated: they are reported as the frozen solve gave them, at `omega_ref`, with `OwnFrequencyRoot::skipped` set.

use crate::assemble::{assemble_with, stiffness_with};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::harmonic_direct::{Dashpot, LossFactor};
use crate::mass::consistent_with;
use crate::material::Matrix6;
use crate::modal::{modal_analysis, ModalOptions};
use crate::model::{DofMap, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_general::{quadratic_eigen_with, Defect, GeneralEigenOptions};
use rcfd_linalg::sparse_chol::SparseLdlt;
use rcfd_linalg::CsrMatrix;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// `|Im lambda| <= REAL_TOLERANCE |lambda|` is a real (non-oscillating) root (ADR 0450 D3.4).
pub const REAL_TOLERANCE: f64 = 1e-6;

/// A root whose shape carries a share above `PSEUDO_SHARE` in the residual-vector coordinates is flagged as a pseudo-mode of
/// the truncation (ADR 0467).
pub const PSEUDO_SHARE: f64 = 0.1;

/// A residual vector whose M-norm after the orthogonalisation is at most `RESIDUAL_DROP` times its own is dropped (ADR 0464).
pub const RESIDUAL_DROP: f64 = 1e-8;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

/// The fixed point of each root of a viscoelastic material on its own frequency (ADR 0520 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct OwnFrequency {
    /// The number of the lowest reported roots of the frozen solve that are iterated (and reported).
    pub roots: usize,
    /// Converged when `|w_new - w| <= tolerance * w_new` (relative change of the frequency).
    pub tolerance: f64,
    /// The most solves after the frozen one; not converged by then is refused.
    pub max_iterations: usize,
    /// Leave the roots flagged `pseudo` (ADR 0467) in the frozen solve at the reference frequency, not iterated (ADR 0529).
    pub skip_pseudo: bool,
}

impl OwnFrequency {
    /// The lowest `roots` roots, tolerance `1e-12`, at most 50 iterations.
    pub fn new(roots: usize) -> OwnFrequency {
        OwnFrequency {
            roots,
            tolerance: 1e-12,
            max_iterations: 50,
            skip_pseudo: false,
        }
    }
    /// With the roots flagged as pseudo-modes of the residual vectors left at the reference frequency (ADR 0529).
    pub fn skip_pseudo(mut self, skip: bool) -> OwnFrequency {
        self.skip_pseudo = skip;
        self
    }
    /// With a relative tolerance on the frequency.
    pub fn tolerance(mut self, tolerance: f64) -> OwnFrequency {
        self.tolerance = tolerance;
        self
    }
    /// With a maximum iteration count.
    pub fn max_iterations(mut self, max_iterations: usize) -> OwnFrequency {
        self.max_iterations = max_iterations;
        self
    }
}

/// The record of one root iterated to its own frequency (ADR 0520).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct OwnFrequencyRoot {
    /// The root of the frozen solve at the reference frequency it started from.
    pub frozen: C64,
    /// The circular frequency, rad/s, at which the reported root was solved.
    pub omega: f64,
    /// The solves after the frozen one.
    pub iterations: usize,
    /// The last relative change of the frequency, `|Im lambda - omega| / Im lambda` (0 when equal).
    pub change: f64,
    /// The root was flagged `pseudo` and left at the reference frequency, not iterated (ADR 0529).
    pub skipped: bool,
}

/// Options of [`damped_modes`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DampedOptions {
    /// The undamped modal analysis that gives the basis (its `modes` is the basis size `m`).
    pub modal: ModalOptions,
    /// Rayleigh damping `C = a M + b K`.
    pub rayleigh: Option<(f64, f64)>,
    /// Viscous dashpots.
    pub dashpots: Vec<Dashpot>,
    /// Constant loss factors per element set (the set of a material of the model).
    pub loss_factors: Vec<(String, LossFactor)>,
    /// Static residual vectors of the dashpots in the basis (ADR 0464; default off).
    pub residual_vectors: bool,
    /// The circular frequency, rad/s, at which a viscoelastic material is taken (ADR 0495; default none: refused).
    pub reference_omega: Option<f64>,
    /// The temperature, K, of a viscoelastic material with a time-temperature shift (ADR 0495; default none).
    pub reference_temperature: Option<f64>,
    /// Each root iterated to its own frequency (ADR 0520; default none: every root at the reference frequency).
    pub own_frequency: Option<OwnFrequency>,
}

impl DampedOptions {
    /// A basis of the lowest `modes` undamped modes, no damping.
    pub fn new(modes: usize) -> DampedOptions {
        DampedOptions::with_modal(ModalOptions::new(modes))
    }
    /// A basis from the given modal options, no damping.
    pub fn with_modal(modal: ModalOptions) -> DampedOptions {
        DampedOptions {
            modal,
            rayleigh: None,
            dashpots: Vec::new(),
            loss_factors: Vec::new(),
            residual_vectors: false,
            reference_omega: None,
            reference_temperature: None,
            own_frequency: None,
        }
    }
    /// With the viscoelastic material of the model taken at the circular frequency `omega` (rad/s) and, for a shifted
    /// material, the temperature (ADR 0495): the basis from its storage moduli there, its loss moduli as loss stiffness.
    pub fn viscoelastic_reference(mut self, omega: f64, temperature: Option<f64>) -> DampedOptions {
        self.reference_omega = Some(omega);
        self.reference_temperature = temperature;
        self
    }
    /// With each of the lowest roots of a viscoelastic material iterated to its own frequency, on the basis of the storage
    /// moduli at the reference frequency (ADR 0520; needs [`DampedOptions::viscoelastic_reference`]).
    pub fn viscoelastic_own_frequency(mut self, own: OwnFrequency) -> DampedOptions {
        self.own_frequency = Some(own);
        self
    }
    /// With Rayleigh damping `a M + b K`.
    pub fn rayleigh(mut self, a: f64, b: f64) -> DampedOptions {
        self.rayleigh = Some((a, b));
        self
    }
    /// With a dashpot.
    pub fn dashpot(mut self, d: Dashpot) -> DampedOptions {
        self.dashpots.push(d);
        self
    }
    /// With a constant loss factor on the material of an element set (a [`LossFactor::Table`] is refused).
    pub fn loss_factor(mut self, set: &str, loss: LossFactor) -> DampedOptions {
        self.loss_factors.push((set.to_string(), loss));
        self
    }
    /// With (or without) the static residual vectors `K^-1 f_i` of the dashpots' influence vectors, M-orthogonalised
    /// against the undamped modes, in the basis (ADR 0464).
    pub fn residual_vectors(mut self, on: bool) -> DampedOptions {
        self.residual_vectors = on;
        self
    }
    /// With a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> DampedOptions {
        self.modal = self.modal.threads(threads);
        self
    }
}

/// One reported root `u = Re(shape e^{lambda t})`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DampedMode {
    /// `lambda` (`Im >= 0`; exactly 0 for a real root).
    pub eigenvalue: C64,
    /// `-Re lambda`, 1/s.
    pub decay_rate: f64,
    /// `Im lambda`, rad/s.
    pub damped_omega: f64,
    /// `Im lambda / (2 pi)`, Hz.
    pub frequency: f64,
    /// `-Re lambda / |lambda|` (0 for `lambda = 0`).
    pub damping_ratio: f64,
    /// Whether the root oscillates (`Im lambda > REAL_TOLERANCE |lambda|`).
    pub oscillating: bool,
    /// The complex shape on the model's `DofMap`, `x^H M x = 1`, the entry of largest modulus real and positive.
    pub shape: Vec<C64>,
    /// The backward residual of the projected quadratic problem.
    pub residual: f64,
    /// The M-weighted share of the shape carried by the residual-vector coordinates of the basis, in `[0, 1]`
    /// (exactly 0 without residual vectors; ADR 0467).
    pub residual_share: f64,
    /// Whether `residual_share > PSEUDO_SHARE`: a pseudo-mode of the truncation, not a root of the continuum (ADR 0467).
    pub pseudo: bool,
}

/// The damped modes of a model (ADR 0450 D3).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DampedModes {
    /// The reported roots, ordered by `(Im lambda, |lambda|)`.
    pub modes: Vec<DampedMode>,
    /// All `2 m` roots of the projected problem.
    pub roots: Vec<C64>,
    /// The undamped circular frequencies of the basis, rad/s.
    pub omegas: Vec<f64>,
    /// The basis size: `m` modes plus the residual vectors.
    pub basis: usize,
    /// The static residual vectors in the basis (M-orthonormal, on the model's `DofMap`; empty without the option).
    pub residual_vectors: Vec<Vec<f64>>,
    /// Reported oscillating roots.
    pub oscillating: usize,
    /// Reported real (non-oscillating) roots.
    pub non_oscillating: usize,
    /// Reported roots flagged as pseudo-modes (ADR 0467).
    pub pseudo_modes: usize,
    /// Defective groups of `roots` (indices into `roots`).
    pub defects: Vec<Defect>,
    /// Plain statements on the result, for example the non-oscillating roots.
    pub notes: Vec<String>,
    /// With [`DampedOptions::viscoelastic_own_frequency`], one record per reported mode in the same order (ADR 0520); `roots`
    /// and `defects` are then those of the frozen solve at the reference frequency. Empty otherwise.
    pub own_frequency: Vec<OwnFrequencyRoot>,
    dofs: DofMap,
}

impl DampedModes {
    /// The unknown numbering of the shapes.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }
}

fn finite_nonneg(key: &'static str, what: &str, v: f64) -> Result<()> {
    if !v.is_finite() || v < 0.0 {
        return Err(invalid(
            key,
            format!("{what} must be finite and non-negative, got {v:e}"),
        ));
    }
    Ok(())
}

/// `Phi^T A Phi` (dense `m x m`, row-major).
fn project(a: &CsrMatrix, phi: &[Vec<f64>]) -> Vec<f64> {
    let m = phi.len();
    let mut out = vec![0.0; m * m];
    let mut y = vec![0.0; a.n];
    for j in 0..m {
        a.matvec(&phi[j], &mut y);
        for i in 0..m {
            out[i * m + j] = phi[i].iter().zip(&y).map(|(p, q)| p * q).sum();
        }
    }
    out
}

/// `sqrt(x^T M x)`, with `y` as work space.
fn m_norm(mass: &CsrMatrix, x: &[f64], y: &mut [f64]) -> f64 {
    mass.matvec(x, y);
    x.iter().zip(y.iter()).map(|(a, b)| a * b).sum::<f64>().sqrt()
}

/// The M-weighted share of the Ritz vector `x` (coordinates on the basis, projected mass `mr` of order `m`) carried by the
/// last `r` coordinates: `x_R^H (M_r)_RR x_R / x^H M_r x` (ADR 0467).
fn coordinate_share(mr: &[f64], m: usize, r: usize, x: &[C64]) -> f64 {
    let quad = |lo: usize| -> f64 {
        let mut s = 0.0;
        for i in lo..m {
            for j in lo..m {
                let (a, b) = (x[i], x[j]);
                s += mr[i * m + j] * (a.re * b.re + a.im * b.im);
            }
        }
        s
    };
    let whole = quad(0);
    if whole > 0.0 {
        quad(m - r) / whole
    } else {
        0.0
    }
}

/// The static residual vectors of the dashpots (ADR 0464 D1): `r_i = K^-1 f_i`, M-orthogonalised twice against the modes
/// and the vectors kept, M-normalised; the count of dropped vectors.
fn residual_vectors(
    k: &CsrMatrix,
    mass: &CsrMatrix,
    fixed: &[(usize, f64)],
    phi: &[Vec<f64>],
    pots: &[(usize, Option<usize>, f64)],
) -> Result<(Vec<Vec<f64>>, usize)> {
    let n = k.n;
    let zero = vec![0.0; n];
    let kr = reduce(k, &zero, fixed)?;
    let ldlt = SparseLdlt::factor(&kr.matrix).map_err(|e| FemError::Solver {
        solver: "sparse_ldlt",
        reason: e.to_string(),
    })?;
    let free = kr.free();
    let mut kept: Vec<Vec<f64>> = Vec::new();
    let mut dropped = 0;
    let mut y = vec![0.0; n];
    for &(ea, eb, _) in pots {
        let mut f = vec![0.0; free.len()];
        if let Ok(i) = free.binary_search(&ea) {
            f[i] += 1.0;
        }
        if let Some(Ok(j)) = eb.map(|e| free.binary_search(&e)) {
            f[j] -= 1.0;
        }
        let rf = ldlt.solve(&f);
        let mut r = vec![0.0; n];
        for (&eq, &v) in free.iter().zip(&rf) {
            r[eq] = v;
        }
        let basis: Vec<&[f64]> = phi.iter().chain(kept.iter()).map(Vec::as_slice).collect();
        if m_orthonormalise(mass, &mut r, &basis, &mut y) {
            kept.push(r);
        } else {
            dropped += 1;
        }
    }
    Ok((kept, dropped))
}

/// `r` M-orthogonalised against `basis` (classical Gram-Schmidt in the inner product `x^T M y`, made twice) and
/// M-normalised (ADR 0464 D1 step 3, shared with the residual vectors of ADR 0474). `false` (the vector dropped) when its
/// M-norm before is zero or not finite, or after is at most [`RESIDUAL_DROP`] times before. `y` is work space of `r`'s size.
pub(crate) fn m_orthonormalise(mass: &CsrMatrix, r: &mut [f64], basis: &[&[f64]], y: &mut [f64]) -> bool {
    let own = m_norm(mass, r, y);
    if own == 0.0 || !own.is_finite() {
        return false;
    }
    for _pass in 0..2 {
        mass.matvec(r, y);
        let coefs: Vec<f64> = basis
            .iter()
            .map(|p| p.iter().zip(y.iter()).map(|(a, b)| a * b).sum())
            .collect();
        for (p, a) in basis.iter().zip(coefs) {
            for (ri, pi) in r.iter_mut().zip(p.iter()) {
                *ri -= a * pi;
            }
        }
    }
    let after = m_norm(mass, r, y);
    if after <= RESIDUAL_DROP * own {
        return false;
    }
    for v in r.iter_mut() {
        *v /= after;
    }
    true
}

/// The reported mode of the root `lam` with Ritz vector `x` (ADR 0450 D3.4), `None` for a root that is not reported.
#[allow(clippy::too_many_arguments)]
fn report(
    lam: C64,
    x: &[C64],
    residual: f64,
    phi: &[Vec<f64>],
    n: usize,
    mr: &[f64],
    m: usize,
    rvs: usize,
) -> Option<DampedMode> {
    let r = lam.abs();
    let oscillating = lam.im > REAL_TOLERANCE * r;
    let real = lam.im.abs() <= REAL_TOLERANCE * r;
    if !oscillating && !real {
        return None;
    }
    let eigenvalue = if oscillating { lam } else { C64::from_real(lam.re) };
    let mut shape = vec![C64::ZERO; n];
    for (j, p) in phi.iter().enumerate() {
        for (s, &v) in shape.iter_mut().zip(p) {
            *s += x[j].scale(v);
        }
    }
    let ra = eigenvalue.abs();
    let residual_share = if rvs == 0 { 0.0 } else { coordinate_share(mr, m, rvs, x) };
    Some(DampedMode {
        eigenvalue,
        decay_rate: -eigenvalue.re,
        damped_omega: eigenvalue.im,
        frequency: eigenvalue.im / (2.0 * std::f64::consts::PI),
        damping_ratio: if ra > 0.0 { -eigenvalue.re / ra } else { 0.0 },
        oscillating,
        shape,
        residual,
        residual_share,
        pseudo: residual_share > PSEUDO_SHARE,
    })
}

fn by_frequency(p: &DampedMode, q: &DampedMode) -> std::cmp::Ordering {
    p.damped_omega
        .total_cmp(&q.damped_omega)
        .then(p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()))
}

/// The own-frequency options checked (ADR 0520 D1).
fn check_own_frequency(options: &DampedOptions) -> Result<()> {
    let Some(own) = &options.own_frequency else {
        return Ok(());
    };
    let bad = |reason: String| Err(invalid("own frequency", reason));
    if options.reference_omega.is_none() {
        return bad(
            "the own-frequency iteration needs a reference frequency (DampedOptions::viscoelastic_reference: the \
             frequency of the storage basis and the start, ADR 0520)"
                .to_string(),
        );
    }
    if own.roots == 0 {
        return bad("at least one root must be iterated".to_string());
    }
    if !(own.tolerance.is_finite() && own.tolerance > 0.0) {
        return bad(format!("tolerance {:e} (must be finite and > 0)", own.tolerance));
    }
    if own.max_iterations == 0 {
        return bad("at least one iteration must be allowed".to_string());
    }
    Ok(())
}

/// `2 G I_dev` in Voigt form with engineering shear strain (the shear part of an isotropic stiffness of shear modulus `g`).
fn shear_matrix(g: f64) -> Matrix6 {
    let mut d = [[0.0; 6]; 6];
    for (i, row) in d.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = 2.0 * g * (if i == j { 1.0 } else { 0.0 } - 1.0 / 3.0);
        }
    }
    for (k, row) in d.iter_mut().enumerate().skip(3) {
        row[k] = g;
    }
    d
}

/// `K 1 (x) 1` in Voigt form.
fn bulk_matrix(k: f64) -> Matrix6 {
    let mut d = [[0.0; 6]; 6];
    for row in d.iter_mut().take(3) {
        for v in row.iter_mut().take(3) {
            *v = k;
        }
    }
    d
}

/// The viscoelastic materials taken at the reference frequency (ADR 0495 D1): the model with each replaced by the elastic
/// material of its storage moduli, and per material index the loss matrix `D''`.
struct ViscoReference {
    storage: SolidModel,
    loss: Vec<Option<Matrix6>>,
}

fn viscoelastic_reference(model: &SolidModel, options: &DampedOptions) -> Result<Option<ViscoReference>> {
    let has = model
        .materials()
        .iter()
        .any(|(_, m)| matches!(m, Material::Viscoelastic { .. }));
    let Some(omega) = options.reference_omega else {
        return Ok(None);
    };
    if !(omega.is_finite() && omega >= 0.0) {
        return Err(invalid(
            "reference frequency",
            format!("{omega:e} rad/s (must be finite and >= 0)"),
        ));
    }
    if !has {
        return Err(invalid(
            "reference frequency",
            "a reference frequency is given but the model has no viscoelastic material (ADR 0495)".to_string(),
        ));
    }
    let mut materials = Vec::with_capacity(model.materials().len());
    let mut loss = Vec::with_capacity(model.materials().len());
    for (set, material) in model.materials() {
        if let Material::Viscoelastic { base, .. } = material {
            let c = crate::mat_visco::complex_moduli(material, omega, options.reference_temperature)?;
            let (gs, ks) = (c.shear.re, c.bulk.re);
            let e = 9.0 * ks * gs / (3.0 * ks + gs);
            let nu = (3.0 * ks - 2.0 * gs) / (2.0 * (3.0 * ks + gs));
            let Material::IsotropicElastic { density, expansion, .. } = base.as_ref() else {
                return Err(FemError::NotYetSupported {
                    item: "material",
                    variant: material.name(),
                    location: "an orthotropic base".to_string(),
                    package: crate::mat_visco::FOLLOW_UP,
                });
            };
            materials.push((set.clone(), Material::isotropic_elastic(e, nu, *density, *expansion)?));
            let (ds, db) = (shear_matrix(c.shear.im), bulk_matrix(c.bulk.im));
            let mut d = [[0.0; 6]; 6];
            for i in 0..6 {
                for j in 0..6 {
                    d[i][j] = ds[i][j] + db[i][j];
                }
            }
            loss.push(Some(d));
        } else {
            materials.push((set.clone(), material.clone()));
            loss.push(None);
        }
    }
    Ok(Some(ViscoReference {
        storage: model.with_materials(materials),
        loss,
    }))
}

/// The damped modes of a model with Rayleigh damping, dashpots and constant loss factors, on its lowest undamped modes. A
/// viscoelastic material is taken at the reference frequency of the options (ADR 0495).
pub fn damped_modes(model: &SolidModel, options: &DampedOptions) -> Result<DampedModes> {
    model.validate()?;
    check_own_frequency(options)?;
    let visco = viscoelastic_reference(model, options)?;
    let original = model;
    let model = match &visco {
        Some(v) => &v.storage,
        None => model,
    };
    model.check_supported()?;
    // ADR 0493 D5: the damped modes do not take the Hex8 formulations.
    crate::hex8x::refuse_formulations(model, "the damped modes")?;
    if crate::mpc::uses_transformation(model) {
        return Err(invalid(
            "constraint",
            "linear constraints, rigid sets, cyclic constraints and elastic supports are not taken by the damped modes \
             (ADR 0450: the elimination path of the modal analysis only)"
                .to_string(),
        ));
    }
    let (a, b) = options.rayleigh.unwrap_or((0.0, 0.0));
    finite_nonneg("damping", "Rayleigh mass coefficient", a)?;
    finite_nonneg("damping", "Rayleigh stiffness coefficient", b)?;
    let materials = model.materials();
    let mut eta = vec![None::<f64>; materials.len()];
    for (set, loss) in &options.loss_factors {
        let mi = materials.iter().position(|(s, _)| s == set).ok_or_else(|| {
            invalid(
                "loss factor",
                format!("element set '{set}' carries no material of the model"),
            )
        })?;
        if eta[mi].is_some() {
            return Err(invalid("loss factor", format!("element set '{set}' given twice")));
        }
        if visco.as_ref().is_some_and(|v| v.loss[mi].is_some()) {
            return Err(invalid(
                "loss factor",
                format!(
                    "element set '{set}' carries a viscoelastic material, whose damping is its loss modulus (ADR 0495)"
                ),
            ));
        }
        match loss {
            LossFactor::Constant(e) => {
                finite_nonneg("loss factor", &format!("loss factor of set '{set}'"), *e)?;
                eta[mi] = Some(*e);
            }
            LossFactor::Table(_) => {
                return Err(invalid(
                    "loss factor",
                    format!(
                        "a modulus table on set '{set}' is refused by the damped modes: a frequency-dependent modulus \
                         makes the eigenproblem nonlinear (ADR 0450); give a constant loss factor"
                    ),
                ))
            }
        }
    }
    let dofs = model.dof_map();
    let n_nodes = model.mesh().nodes().len();
    let mut pots = Vec::with_capacity(options.dashpots.len());
    for (i, d) in options.dashpots.iter().enumerate() {
        let (node, other, component, c) = d.parts();
        finite_nonneg("dashpot", &format!("dashpot {i}: coefficient"), c)?;
        let eq = |node: usize| -> Result<usize> {
            if node >= n_nodes {
                return Err(invalid(
                    "dashpot",
                    format!("dashpot {i}: node {node} does not exist ({n_nodes} nodes)"),
                ));
            }
            dofs.equation(node, component).ok_or_else(|| {
                invalid(
                    "dashpot",
                    format!("dashpot {i}: node {node} has no component {component}"),
                )
            })
        };
        let ea = eq(node)?;
        let eb = match other {
            Some(o) => Some(eq(o)?),
            None => None,
        };
        pots.push((ea, eb, c));
    }
    // ADR 0464: the residual vectors need a nonsingular stiffness.
    let residual = options.residual_vectors && !pots.is_empty();
    let fixed = if residual {
        let fixed = prescribed_equations(model, &[])?;
        match check_rigid_body_motions(model, &fixed) {
            Ok(()) => {}
            Err(FemError::RigidBodyMotionFree { .. }) => {
                return Err(invalid(
                    "residual vectors",
                    "the static residual vectors need a model without free rigid-body motion (K^-1 of the dashpot \
                     influence vectors, ADR 0464)"
                        .to_string(),
                ))
            }
            Err(e) => return Err(e),
        }
        fixed
    } else {
        Vec::new()
    };

    let modal = modal_analysis(model, &options.modal)?;
    let threads = options.modal.threads;
    let k = stiffness_with(model, threads)?;
    let mass = consistent_with(model, threads)?;
    let (rvs, dropped) = if residual {
        residual_vectors(&k, &mass, &fixed, &modal.modes, &pots)?
    } else {
        (Vec::new(), 0)
    };
    let augmented: Vec<Vec<f64>>;
    let phi: &[Vec<f64>] = if rvs.is_empty() {
        &modal.modes
    } else {
        augmented = modal.modes.iter().chain(rvs.iter()).cloned().collect();
        &augmented
    };
    let m = phi.len();
    let kr = project(&k, phi);
    let mr = project(&mass, phi);
    let mut cr = vec![0.0; m * m];
    for i in 0..m * m {
        cr[i] = a * mr[i] + b * kr[i];
    }
    for &(ea, eb, c) in &pots {
        let rel: Vec<f64> = phi.iter().map(|p| p[ea] - eb.map_or(0.0, |e| p[e])).collect();
        for i in 0..m {
            for j in 0..m {
                cr[i * m + j] += c * rel[i] * rel[j];
            }
        }
    }
    let mut hr = if eta.iter().any(|e| e.is_some()) {
        let mut h = vec![0.0; m * m];
        if materials.len() == 1 {
            let e = eta[0].unwrap_or(0.0);
            for i in 0..m * m {
                h[i] = e * kr[i];
            }
        } else {
            let which = model.material_indices()?;
            for (mi, e) in eta.iter().enumerate() {
                let Some(e) = *e else { continue };
                let km = assemble_with(model, threads, |ctx| {
                    if which[ctx.element] != mi {
                        let size: usize = ctx.nodes.iter().map(|&a| dofs.components(a)).sum();
                        return Ok(crate::element::ElementMatrix::zeros(size));
                    }
                    let d = ctx.material.elasticity_matrix()?;
                    element_for(ctx.kind).stiffness(ctx.coords, &d)
                })?;
                let p = project(&km, phi);
                for i in 0..m * m {
                    h[i] += e * p[i];
                }
            }
        }
        Some(h)
    } else {
        None
    };
    let own = match (&visco, &options.own_frequency) {
        (Some(v), Some(own)) => Some(OwnSetup::new(original, model, v, phi, threads, hr.clone(), *own)?),
        _ => None,
    };
    if let Some(v) = &visco {
        let mut h = hr.unwrap_or_else(|| vec![0.0; m * m]);
        let which = model.material_indices()?;
        for (mi, d) in v.loss.iter().enumerate() {
            let Some(d) = *d else { continue };
            let km = assemble_with(model, threads, |ctx| {
                if which[ctx.element] != mi {
                    let size: usize = ctx.nodes.iter().map(|&a| dofs.components(a)).sum();
                    return Ok(crate::element::ElementMatrix::zeros(size));
                }
                element_for(ctx.kind).stiffness(ctx.coords, &d)
            })?;
            let p = project(&km, phi);
            for i in 0..m * m {
                h[i] += p[i];
            }
        }
        hr = Some(h);
    }
    let q = quadratic_eigen_with(&mr, &cr, &kr, hr.as_deref(), m, &GeneralEigenOptions::default()).map_err(|e| {
        FemError::Solver {
            solver: "quadratic_eigen",
            reason: e.to_string(),
        }
    })?;

    let n = k.n;
    let mut modes = Vec::new();
    for (idx, &lam) in q.values.iter().enumerate() {
        if let Some(mode) = report(lam, &q.vectors[idx], q.residuals[idx], phi, n, &mr, m, rvs.len()) {
            modes.push(mode);
        }
    }
    modes.sort_by(by_frequency);
    let mut own_records = Vec::new();
    let mut own_notes = Vec::new();
    if let (Some(setup), Some(omega_ref)) = (&own, options.reference_omega) {
        let solve_at = |w: f64| -> Result<Vec<DampedMode>> {
            let (kw, hw) = setup.at(w, options.reference_temperature)?;
            let q =
                quadratic_eigen_with(&mr, &cr, &kw, Some(&hw), m, &GeneralEigenOptions::default()).map_err(|e| {
                    FemError::Solver {
                        solver: "quadratic_eigen",
                        reason: e.to_string(),
                    }
                })?;
            Ok(q.values
                .iter()
                .enumerate()
                .filter_map(|(idx, &lam)| report(lam, &q.vectors[idx], q.residuals[idx], phi, n, &mr, m, rvs.len()))
                .collect())
        };
        let count = setup.own.roots.min(modes.len());
        if count < setup.own.roots {
            own_notes.push(format!(
                "{} roots asked for the own-frequency iteration, {count} reported by the frozen solve",
                setup.own.roots
            ));
        }
        let mut done = Vec::with_capacity(count);
        let mut skipped = 0;
        for (j, start) in modes.iter().take(count).enumerate() {
            let mut cur = start.clone();
            let mut w = omega_ref;
            let mut k = 0;
            if setup.own.skip_pseudo && start.pseudo {
                skipped += 1;
                done.push((
                    cur,
                    OwnFrequencyRoot {
                        frozen: start.eigenvalue,
                        omega: omega_ref,
                        iterations: 0,
                        change: 0.0,
                        skipped: true,
                    },
                ));
                continue;
            }
            loop {
                let wn = cur.eigenvalue.im.max(0.0);
                let change = if wn == w {
                    0.0
                } else if wn > 0.0 {
                    (wn - w).abs() / wn
                } else {
                    f64::INFINITY
                };
                if wn == w || (wn - w).abs() <= setup.own.tolerance * wn {
                    done.push((
                        cur,
                        OwnFrequencyRoot {
                            frozen: start.eigenvalue,
                            omega: w,
                            iterations: k,
                            change,
                            skipped: false,
                        },
                    ));
                    break;
                }
                if k == setup.own.max_iterations {
                    return Err(FemError::Solver {
                        solver: "viscoelastic own frequency",
                        reason: format!(
                            "root {j} (frozen {:e} {:+e} i) not converged in {k} iterations: last relative change of the \
                             frequency {change:e} above the tolerance {:e} (ADR 0520)",
                            start.eigenvalue.re, start.eigenvalue.im, setup.own.tolerance
                        ),
                    });
                }
                let prev = cur.eigenvalue;
                let Some(next) = solve_at(wn)?
                    .into_iter()
                    .min_by(|a, b| (a.eigenvalue - prev).abs().total_cmp(&(b.eigenvalue - prev).abs()))
                else {
                    return Err(FemError::Solver {
                        solver: "viscoelastic own frequency",
                        reason: format!("root {j}: no reported root at {wn:e} rad/s (ADR 0520)"),
                    });
                };
                cur = next;
                w = wn;
                k += 1;
            }
        }
        done.sort_by(|(p, _), (q, _)| by_frequency(p, q));
        let most = done.iter().map(|(_, r)| r.iterations).max().unwrap_or(0);
        own_notes.push(format!(
            "{} roots iterated to their own frequency on the storage basis at {omega_ref:e} rad/s, at most {most} \
             iterations (ADR 0520)",
            count - skipped
        ));
        if setup.own.skip_pseudo {
            own_notes.push(if skipped > 0 {
                format!(
                    "{skipped} roots flagged pseudo (ADR 0467) left at the reference frequency {omega_ref:e} rad/s, not \
                     iterated (ADR 0529)"
                )
            } else {
                format!("no root flagged pseudo among the {count} iterated (ADR 0529)")
            });
        }
        for a in 0..done.len() {
            for b in a + 1..done.len() {
                let (za, zb) = (done[a].0.eigenvalue, done[b].0.eigenvalue);
                if (za - zb).abs() <= 1e-9 * za.abs().max(zb.abs()) {
                    own_notes.push(format!("roots {a} and {b} converged to the same root (ADR 0520)"));
                }
            }
        }
        let (m2, r2): (Vec<DampedMode>, Vec<OwnFrequencyRoot>) = done.into_iter().unzip();
        modes = m2;
        own_records = r2;
    }
    let oscillating = modes.iter().filter(|d| d.oscillating).count();
    let non_oscillating = modes.len() - oscillating;
    let pseudo_modes = modes.iter().filter(|d| d.pseudo).count();
    let mut notes = Vec::new();
    if !rvs.is_empty() {
        notes.push(format!(
            "{} static residual vectors added to {} modes (ADR 0464)",
            rvs.len(),
            modal.modes.len()
        ));
    }
    if dropped > 0 {
        notes.push(format!(
            "{dropped} residual vectors dropped: already in the span of the basis (M-norm after the orthogonalisation at most \
             {RESIDUAL_DROP:e} of its own)"
        ));
    }
    if non_oscillating > 0 {
        notes.push(format!(
            "{non_oscillating} real roots: non-oscillating (overdamped) motion, damping ratio 1 for a negative real root"
        ));
    }
    if oscillating == 0 {
        notes.push("no oscillating root".to_string());
    }
    if !q.defects.is_empty() {
        notes.push(format!(
            "{} defective groups of roots (numerically parallel vectors, a Jordan block or a near one)",
            q.defects.len()
        ));
    }
    if pseudo_modes > 0 {
        notes.push(format!(
            "{pseudo_modes} roots flagged as pseudo-modes of the residual vectors (share above {PSEUDO_SHARE})"
        ));
    }
    notes.extend(own_notes);
    Ok(DampedModes {
        modes,
        roots: q.values,
        omegas: modal.eigenvalues.iter().map(|l| l.max(0.0).sqrt()).collect(),
        basis: m,
        residual_vectors: rvs,
        oscillating,
        non_oscillating,
        pseudo_modes,
        defects: q.defects,
        notes,
        own_frequency: own_records,
        dofs,
    })
}

/// A viscoelastic material of the model with the projections `P^G`, `P^K` of its stiffness for unit shear and bulk moduli.
type ViscoPart<'a> = (&'a Material, Vec<f64>, Vec<f64>);

/// The projected pieces of the own-frequency iteration (ADR 0520 D1): the viscoelastic materials; `K_rest` of the other
/// materials; `H_eta` of their constant loss factors.
struct OwnSetup<'a> {
    visco: Vec<ViscoPart<'a>>,
    k_rest: Option<Vec<f64>>,
    h_eta: Option<Vec<f64>>,
    m: usize,
    own: OwnFrequency,
}

impl<'a> OwnSetup<'a> {
    fn new(
        original: &'a SolidModel,
        storage: &SolidModel,
        v: &ViscoReference,
        phi: &[Vec<f64>],
        threads: Option<usize>,
        h_eta: Option<Vec<f64>>,
        own: OwnFrequency,
    ) -> Result<OwnSetup<'a>> {
        let which = storage.material_indices()?;
        let dofs = storage.dof_map();
        let part = |keep: &(dyn Fn(usize) -> bool + Sync), d: Option<Matrix6>| -> Result<Vec<f64>> {
            let km = assemble_with(storage, threads, |ctx| {
                if !keep(which[ctx.element]) {
                    let size: usize = ctx.nodes.iter().map(|&a| dofs.components(a)).sum();
                    return Ok(crate::element::ElementMatrix::zeros(size));
                }
                let d = match d {
                    Some(d) => d,
                    None => ctx.material.elasticity_matrix()?,
                };
                element_for(ctx.kind).stiffness(ctx.coords, &d)
            })?;
            Ok(project(&km, phi))
        };
        let mut visco = Vec::new();
        for (mi, loss) in v.loss.iter().enumerate() {
            if loss.is_some() {
                let pg = part(&|i| i == mi, Some(shear_matrix(1.0)))?;
                let pk = part(&|i| i == mi, Some(bulk_matrix(1.0)))?;
                visco.push((&original.materials()[mi].1, pg, pk));
            }
        }
        let k_rest = if v.loss.iter().any(|l| l.is_none()) {
            Some(part(&|i| v.loss[i].is_none(), None)?)
        } else {
            None
        };
        Ok(OwnSetup {
            visco,
            k_rest,
            h_eta,
            m: phi.len(),
            own,
        })
    }

    /// `K_r(w)` and `H_r(w)`.
    fn at(&self, w: f64, temperature: Option<f64>) -> Result<(Vec<f64>, Vec<f64>)> {
        let mm = self.m * self.m;
        let mut kw = self.k_rest.clone().unwrap_or_else(|| vec![0.0; mm]);
        let mut hw = self.h_eta.clone().unwrap_or_else(|| vec![0.0; mm]);
        for (material, pg, pk) in &self.visco {
            let c = crate::mat_visco::complex_moduli(material, w, temperature)?;
            for i in 0..mm {
                kw[i] += c.shear.re * pg[i] + c.bulk.re * pk[i];
                hw[i] += c.shear.im * pg[i] + c.bulk.im * pk[i];
            }
        }
        Ok((kw, hw))
    }
}
