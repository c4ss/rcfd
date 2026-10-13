//! Steady scalar transport `div(phi s) - div(D grad s) = S` shared by the temperature equation
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

/// One implicit sweep of the transport equation for `field`; returns the normalised residual
/// before the solve.
pub fn sweep(geom: &FvGeometry, field: &mut ScalarField, inp: &SweepInputs) -> f64 {
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
    if let Some((scheme, dt, hist)) = inp.ddt {
        m = add_ddt(scheme, m, geom.mesh.cell_volumes(), dt, hist);
    }
    let res = normalised_residual(&m, &cells);
    m.relax(inp.relax, &cells);
    let mut x = cells;
    m.solve(&mut x, inp.solver);
    field.values = x;
    res
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
}

impl ScalarTransport {
    /// New transport with the tight default solver tolerance used for conservation checks.
    pub fn new(geom: Arc<FvGeometry>, field: ScalarField, diffusivity: f64) -> Self {
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
        sweep(
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
        )
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
