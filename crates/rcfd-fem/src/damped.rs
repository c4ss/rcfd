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

use crate::assemble::{assemble_with, stiffness_with};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::harmonic_direct::{Dashpot, LossFactor};
use crate::mass::consistent_with;
use crate::modal::{modal_analysis, ModalOptions};
use crate::model::{DofMap, SolidModel};
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
        }
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

/// The damped modes of a model with Rayleigh damping, dashpots and constant loss factors, on its lowest undamped modes.
pub fn damped_modes(model: &SolidModel, options: &DampedOptions) -> Result<DampedModes> {
    model.validate()?;
    model.check_supported()?;
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
    let hr = if eta.iter().any(|e| e.is_some()) {
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
    let q = quadratic_eigen_with(&mr, &cr, &kr, hr.as_deref(), m, &GeneralEigenOptions::default()).map_err(|e| {
        FemError::Solver {
            solver: "quadratic_eigen",
            reason: e.to_string(),
        }
    })?;

    let n = k.n;
    let mut modes = Vec::new();
    for (idx, &lam) in q.values.iter().enumerate() {
        let r = lam.abs();
        let oscillating = lam.im > REAL_TOLERANCE * r;
        let real = lam.im.abs() <= REAL_TOLERANCE * r;
        if !oscillating && !real {
            continue;
        }
        let eigenvalue = if oscillating { lam } else { C64::from_real(lam.re) };
        let x = &q.vectors[idx];
        let mut shape = vec![C64::ZERO; n];
        for (j, p) in phi.iter().enumerate() {
            for (s, &v) in shape.iter_mut().zip(p) {
                *s += x[j].scale(v);
            }
        }
        let ra = eigenvalue.abs();
        let residual_share = if rvs.is_empty() {
            0.0
        } else {
            coordinate_share(&mr, m, rvs.len(), x)
        };
        modes.push(DampedMode {
            eigenvalue,
            decay_rate: -eigenvalue.re,
            damped_omega: eigenvalue.im,
            frequency: eigenvalue.im / (2.0 * std::f64::consts::PI),
            damping_ratio: if ra > 0.0 { -eigenvalue.re / ra } else { 0.0 },
            oscillating,
            shape,
            residual: q.residuals[idx],
            residual_share,
            pseudo: residual_share > PSEUDO_SHARE,
        });
    }
    modes.sort_by(|p, q| {
        p.damped_omega
            .total_cmp(&q.damped_omega)
            .then(p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()))
    });
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
        dofs,
    })
}
