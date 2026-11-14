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

use crate::assemble::{assemble_with, stiffness_with};
use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::harmonic_direct::{Dashpot, LossFactor};
use crate::mass::consistent_with;
use crate::modal::{modal_analysis, ModalOptions};
use crate::model::{DofMap, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_general::{quadratic_eigen_with, Defect, GeneralEigenOptions};
use rcfd_linalg::CsrMatrix;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// `|Im lambda| <= REAL_TOLERANCE |lambda|` is a real (non-oscillating) root (ADR 0450 D3.4).
pub const REAL_TOLERANCE: f64 = 1e-6;

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
    /// The basis size `m`.
    pub basis: usize,
    /// Reported oscillating roots.
    pub oscillating: usize,
    /// Reported real (non-oscillating) roots.
    pub non_oscillating: usize,
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

    let modal = modal_analysis(model, &options.modal)?;
    let phi = &modal.modes;
    let m = phi.len();
    let threads = options.modal.threads;
    let k = stiffness_with(model, threads)?;
    let mass = consistent_with(model, threads)?;
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
        modes.push(DampedMode {
            eigenvalue,
            decay_rate: -eigenvalue.re,
            damped_omega: eigenvalue.im,
            frequency: eigenvalue.im / (2.0 * std::f64::consts::PI),
            damping_ratio: if ra > 0.0 { -eigenvalue.re / ra } else { 0.0 },
            oscillating,
            shape,
            residual: q.residuals[idx],
        });
    }
    modes.sort_by(|p, q| {
        p.damped_omega
            .total_cmp(&q.damped_omega)
            .then(p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()))
    });
    let oscillating = modes.iter().filter(|d| d.oscillating).count();
    let non_oscillating = modes.len() - oscillating;
    let mut notes = Vec::new();
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
    Ok(DampedModes {
        modes,
        roots: q.values,
        omegas: modal.eigenvalues.iter().map(|l| l.max(0.0).sqrt()).collect(),
        basis: m,
        oscillating,
        non_oscillating,
        defects: q.defects,
        notes,
        dofs,
    })
}
