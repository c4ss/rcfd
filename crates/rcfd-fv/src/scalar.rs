//! Scalar transport `div(phi s) - div(D grad s) = S` (steady iterations or transient steps) shared by the temperature equation
//! (F-050), passive scalars and multicomponent species (F-053).
//!
//! [`sweep`] is the one implicit sweep (convection and diffusion operators of momentum, optional
//! volumetric source, under-relaxation, linear solve); [`EnergySolver`](crate::energy::EnergySolver)
//! and [`ScalarTransport`] both call it. [`boundary_flux`] returns the discrete flux through the
//! boundary faces with the same coefficients the matrix uses, so for a conservative scheme the net
//! boundary outflow equals the integrated source up to the linear-solver residual.
//!
//! A [`SpeciesSet`] transports `N - 1` mass fractions and closes the last one by difference
//! (`Y_N = 1 - sum Y_i`), the usual treatment of the balance species: the sum is one to round-off by
//! construction, and every transported species is conserved separately.

use crate::bc::FaceCoeffs;
use crate::convection::{convection, ConvScheme};
use crate::ddt::{add_ddt, History, TimeScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::ScalarField;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::normalised_residual;
use rcfd_core::vec3;
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Inputs of one scalar sweep.
pub struct SweepInputs<'a> {
    /// Face flux (volumetric, positive out of the owner).
    pub phi: &'a [f64],
    /// Diffusivity at every face.
    pub gamma: &'a [f64],
    /// Convection scheme.
    pub scheme: ConvScheme,
    /// Implicit under-relaxation factor (1 = none).
    pub relax: f64,
    /// Volumetric source per cell (scalar units per second).
    pub source: Option<&'a [f64]>,
    /// Linear solver controls.
    pub solver: &'a SolveControl,
    /// Time derivative with its scheme, step size and history (transient run); `None` is steady.
    pub ddt: Option<(TimeScheme, f64, &'a History)>,
}

/// Optional additions of [`sweep_ext`] (zone sources, ADR 0019 D2): an implicit linear sink and fixed-value cells.
#[derive(Default, Clone, Copy)]
pub struct SweepExtras<'a> {
    /// Linear sink coefficient `k` per cell (1/s): `k * V` is added to the diagonal (`S = -k phi + ...`).
    pub sink: Option<&'a [f64]>,
    /// Cells held at a value; their equation is replaced by `phi_c = value` and their column is moved to the
    /// right-hand sides of the neighbours, so a symmetric system stays symmetric.
    pub fixed: Option<&'a [Option<f64>]>,
}

/// One implicit sweep of the transport equation for `field`; returns the normalised residual
/// before the solve.
pub fn sweep(geom: &FvGeometry, field: &mut ScalarField, inp: &SweepInputs) -> f64 {
    sweep_ext(geom, field, inp, &SweepExtras::default())
}

/// [`sweep`] with an implicit linear sink and fixed-value cells.
pub fn sweep_ext(geom: &FvGeometry, field: &mut ScalarField, inp: &SweepInputs, ext: &SweepExtras) -> f64 {
    if let Some(fx) = ext.fixed {
        for (v, f) in field.values.iter_mut().zip(fx) {
            if let Some(x) = f {
                *v = *x;
            }
        }
    }
    let cells = field.values.clone();
    let bc = field.boundary_coeffs(geom);
    let bvals = field.boundary_values(geom);
    let grad = inp.scheme.needs_gradient().then(|| least_squares(geom, &cells, &bvals));
    let mut m = convection(geom, inp.phi, &cells, &bc, inp.scheme, grad.as_deref());
    m.add(&laplacian(geom, inp.gamma, &bc, grad.as_deref()));
    if let Some(s) = inp.source {
        let vol = geom.mesh.cell_volumes();
        for c in 0..cells.len() {
            m.b[c] += s[c] * vol[c];
        }
    }
    if let Some(k) = ext.sink {
        let vol = geom.mesh.cell_volumes();
        for c in 0..cells.len() {
            m.a.diag[c] += k[c] * vol[c];
        }
    }
    if let Some((scheme, dt, hist)) = inp.ddt {
        m = add_ddt(scheme, m, geom.mesh.cell_volumes(), dt, hist);
    }
    if let Some(fx) = ext.fixed {
        impose_fixed(&mut m, fx);
    }
    let res = normalised_residual(&m, &cells);
    m.relax(inp.relax, &cells);
    let mut x = cells;
    m.solve(&mut x, inp.solver);
    if let Some(fx) = ext.fixed {
        for (v, f) in x.iter_mut().zip(fx) {
            if let Some(val) = f {
                *v = *val;
            }
        }
    }
    field.values = x;
    res
}

/// Replace the equations of the fixed cells by `phi_c = value`, moving their columns to the right-hand sides.
fn impose_fixed(m: &mut FvMatrix, fixed: &[Option<f64>]) {
    let addr = m.a.addr.clone();
    for f in 0..addr.lower.len() {
        let (a, b) = (addr.lower[f], addr.upper[f]);
        match (fixed[a], fixed[b]) {
            (None, None) => {}
            (Some(va), None) => {
                // column a of row b moves to the right-hand side
                m.b[b] -= m.a.lower[f] * va;
                m.a.lower[f] = 0.0;
                m.a.upper[f] = 0.0;
            }
            (None, Some(vb)) => {
                m.b[a] -= m.a.upper[f] * vb;
                m.a.upper[f] = 0.0;
                m.a.lower[f] = 0.0;
            }
            (Some(_), Some(_)) => {
                m.a.upper[f] = 0.0;
                m.a.lower[f] = 0.0;
            }
        }
    }
    for (c, fx) in fixed.iter().enumerate() {
        if let Some(v) = fx {
            m.a.diag[c] = 1.0;
            m.b[c] = *v;
        }
    }
}

/// Discrete flux of a scalar out of the domain through one boundary patch.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchFlux {
    /// Patch name.
    pub patch: String,
    /// Convective outflow `sum phi_f s_f`.
    pub convective: f64,
    /// Diffusive outflow `-sum D_f |S_f| ds/dn`.
    pub diffusive: f64,
    /// Volume flow out through the patch `sum phi_f`.
    pub volume_flow: f64,
}

impl PatchFlux {
    /// Total outflow of the scalar.
    pub fn total(&self) -> f64 {
        self.convective + self.diffusive
    }
}

/// Boundary fluxes per patch, evaluated with the boundary coefficients that [`sweep`] assembles.
/// Empty (2-D) patches are skipped.
pub fn boundary_flux(geom: &FvGeometry, phi: &[f64], gamma: &[f64], field: &ScalarField) -> Vec<PatchFlux> {
    let bc: Vec<FaceCoeffs> = field.boundary_coeffs(geom);
    let own = geom.mesh.owner();
    let sf = geom.mesh.face_area_vectors();
    geom.mesh
        .patches()
        .iter()
        .filter(|p| p.kind != rcfd_mesh::PatchKind::Empty)
        .map(|p| {
            let (mut conv, mut diff, mut vol) = (0.0, 0.0, 0.0);
            for f in p.faces() {
                if geom.empty[f] {
                    continue;
                }
                let co = &bc[f - geom.n_internal];
                let s = field.values[own[f]];
                conv += phi[f] * co.value(s);
                diff -= gamma[f] * vec3::norm(sf[f]) * co.gradient(s);
                vol += phi[f];
            }
            PatchFlux {
                patch: p.name.clone(),
                convective: conv,
                diffusive: diff,
                volume_flow: vol,
            }
        })
        .collect()
}

/// A transported scalar with constant or per-face diffusivity.
pub struct ScalarTransport {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// The scalar with its boundary conditions.
    pub field: ScalarField,
    /// Diffusivity (m^2/s).
    pub diffusivity: f64,
    /// Diffusivity at every face; `None` uses `diffusivity`.
    pub diffusivity_face: Option<Vec<f64>>,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Implicit under-relaxation factor (1 for the linear frozen-flux problem).
    pub relax: f64,
    /// Volumetric source (scalar units per second) per cell.
    pub source: Option<Vec<f64>>,
    /// Linear solver controls.
    pub solver: SolveControl,
    /// Implicit linear sink coefficient per cell (1/s), see [`SweepExtras`].
    pub sink: Option<Vec<f64>>,
    /// Cells held at a value, see [`SweepExtras`].
    pub fixed: Option<Vec<Option<f64>>>,
    /// Old time levels for transient runs ([`ScalarTransport::advance`]).
    pub history: History,
}

impl ScalarTransport {
    /// New transport with the tight default solver tolerance used for conservation checks.
    pub fn new(geom: Arc<FvGeometry>, field: ScalarField, diffusivity: f64) -> Self {
        let history = History::new(field.values.clone());
        ScalarTransport {
            geom,
            field,
            diffusivity,
            diffusivity_face: None,
            convection: ConvScheme::LinearUpwind,
            relax: 1.0,
            source: None,
            solver: SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-13,
                max_iter: 2000,
            },
            sink: None,
            fixed: None,
            history,
        }
    }

    fn gamma(&self) -> Vec<f64> {
        match &self.diffusivity_face {
            Some(g) => g.clone(),
            None => uniform_gamma(&self.geom, self.diffusivity),
        }
    }

    /// One sweep on the flux `phi`; returns the normalised residual before the solve.
    pub fn iterate(&mut self, phi: &[f64]) -> f64 {
        let gamma = self.gamma();
        let geom = self.geom.clone();
        sweep_ext(
            &geom,
            &mut self.field,
            &SweepInputs {
                phi,
                gamma: &gamma,
                scheme: self.convection,
                relax: self.relax,
                source: self.source.as_deref(),
                solver: &self.solver,
                ddt: None,
            },
            &SweepExtras {
                sink: self.sink.as_deref(),
                fixed: self.fixed.as_deref(),
            },
        )
    }

    /// One transient step of size `dt` on the flux `phi`: no relaxation, one linear solve; the new level is pushed
    /// into [`ScalarTransport::history`]. Returns the normalised residual before the solve.
    pub fn advance(&mut self, phi: &[f64], dt: f64, scheme: TimeScheme) -> f64 {
        let gamma = self.gamma();
        let geom = self.geom.clone();
        let res = sweep_ext(
            &geom,
            &mut self.field,
            &SweepInputs {
                phi,
                gamma: &gamma,
                scheme: self.convection,
                relax: 1.0,
                source: self.source.as_deref(),
                solver: &self.solver,
                ddt: Some((scheme, dt, &self.history)),
            },
            &SweepExtras {
                sink: self.sink.as_deref(),
                fixed: self.fixed.as_deref(),
            },
        );
        self.history.push(self.field.values.clone(), dt);
        res
    }

    /// Boundary fluxes per patch for the current field.
    pub fn boundary_flux(&self, phi: &[f64]) -> Vec<PatchFlux> {
        boundary_flux(&self.geom, phi, &self.gamma(), &self.field)
    }

    /// Net outflow through all boundaries minus the integrated source: zero for a converged,
    /// conservative solution.
    pub fn imbalance(&self, phi: &[f64]) -> f64 {
        let out: f64 = self.boundary_flux(phi).iter().map(PatchFlux::total).sum();
        let src: f64 = self.source.as_ref().map_or(0.0, |s| {
            s.iter().zip(self.geom.mesh.cell_volumes()).map(|(a, v)| a * v).sum()
        });
        out - src
    }

    /// Sum of the absolute boundary fluxes, the natural scale of the imbalance.
    pub fn flux_scale(&self, phi: &[f64]) -> f64 {
        self.boundary_flux(phi)
            .iter()
            .map(|p| p.convective.abs() + p.diffusive.abs())
            .sum()
    }
}

/// Multicomponent mass fractions: `N - 1` transported species and one balance species closed by difference.
pub struct SpeciesSet {
    /// Transported species (all but the balance species).
    pub transported: Vec<ScalarTransport>,
    /// Name of every species; the balance species is the last entry.
    pub names: Vec<String>,
}

impl SpeciesSet {
    /// Sweep every transported species; returns the residuals.
    pub fn iterate(&mut self, phi: &[f64]) -> Vec<f64> {
        self.transported.iter_mut().map(|s| s.iterate(phi)).collect()
    }

    /// Mass fraction of the balance species in every cell, `1 - sum Y_i`.
    pub fn balance_fractions(&self) -> Vec<f64> {
        let n = self.transported.first().map_or(0, |s| s.field.values.len());
        let mut y = vec![1.0; n];
        for s in &self.transported {
            for (a, b) in y.iter_mut().zip(&s.field.values) {
                *a -= b;
            }
        }
        y
    }

    /// Net boundary outflow of the balance species: the volume outflow minus the other species'
    /// net outflows (its flux follows from the closure `sum Y = 1` and `sum J_i = 0`).
    pub fn balance_outflow(&self, phi: &[f64]) -> f64 {
        let Some(first) = self.transported.first() else {
            return 0.0;
        };
        let g = &first.geom;
        let vol: f64 = (g.n_internal..g.mesh.n_faces())
            .filter(|&f| !g.empty[f])
            .map(|f| phi[f])
            .sum();
        let others: f64 = self
            .transported
            .iter()
            .map(|s| s.boundary_flux(phi).iter().map(PatchFlux::total).sum::<f64>())
            .sum();
        vol - others
    }
}
