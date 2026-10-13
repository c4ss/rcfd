//! Energy (temperature) transport and Boussinesq buoyancy for steady
//! incompressible flow (F-050).
//!
//! The temperature equation `div(phi T) - div(alpha grad T) = S` is discretised
//! with the same convection and diffusion operators as momentum. Buoyancy uses
//! the Boussinesq approximation: after removing the hydrostatic part the
//! momentum equation receives the body acceleration
//! `a = -beta (T - T_ref) g`, applied with the balanced-force treatment of
//! [`BodyForce`] so that a fluid at rest in a stratified field produces no
//! spurious flow from the pressure/buoyancy imbalance.

use crate::convection::{convection, ConvScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::ScalarField;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use crate::simple::{normalised_residual, BodyForce, IterationInfo, SimpleSolver};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Steady temperature transport on a given flux field.
pub struct EnergySolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Temperature with its boundary conditions.
    pub t: ScalarField,
    /// Thermal diffusivity `k / (rho cp)` (m^2/s).
    pub alpha: f64,
    /// Thermal diffusivity at every face (laminar plus turbulent); `None` uses `alpha`.
    pub alpha_face: Option<Vec<f64>>,
    /// Convection scheme.
    pub convection: ConvScheme,
    /// Implicit under-relaxation factor.
    pub relax: f64,
    /// Volumetric heat source per unit `rho cp` (K/s) per cell.
    pub source: Option<Vec<f64>>,
    /// Linear solver controls.
    pub solver: SolveControl,
}

impl EnergySolver {
    /// New solver with an initial uniform temperature field supplied by the caller.
    pub fn new(geom: Arc<FvGeometry>, t: ScalarField, alpha: f64) -> Self {
        EnergySolver {
            geom,
            t,
            alpha,
            alpha_face: None,
            convection: ConvScheme::LinearUpwind,
            relax: 0.9,
            source: None,
            solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-4,
                max_iter: 500,
            },
        }
    }

    /// One implicit sweep of the temperature equation with face flux `phi`;
    /// returns the normalised residual before the solve.
    pub fn iterate(&mut self, phi: &[f64]) -> f64 {
        let g = self.geom.clone();
        let cells = self.t.values.clone();
        let bc = self.t.boundary_coeffs(&g);
        let bvals = self.t.boundary_values(&g);
        let grad = if self.convection.needs_gradient() {
            Some(least_squares(&g, &cells, &bvals))
        } else {
            None
        };
        let mut m = convection(&g, phi, &cells, &bc, self.convection, grad.as_deref());
        let gamma = match &self.alpha_face {
            Some(a) => a.clone(),
            None => uniform_gamma(&g, self.alpha),
        };
        m.add(&laplacian(&g, &gamma, &bc, grad.as_deref()));
        if let Some(s) = &self.source {
            let vol = g.mesh.cell_volumes();
            for c in 0..cells.len() {
                m.b[c] += s[c] * vol[c];
            }
        }
        let res = normalised_residual(&m, &cells);
        m.relax(self.relax, &cells);
        let mut x = cells;
        m.solve(&mut x, &self.solver);
        self.t.values = x;
        res
    }

    /// Outward-normal temperature gradient at the faces of `patch` (one value per patch face).
    pub fn wall_gradient(&self, patch: &str) -> Vec<f64> {
        let g = &self.geom;
        let p = g.mesh.patch(patch).expect("known patch");
        let coeffs = self.t.boundary_coeffs(g);
        let own = g.mesh.owner();
        p.faces()
            .map(|f| coeffs[f - g.n_internal].gradient(self.t.values[own[f]]))
            .collect()
    }

    /// Conductive heat flow out of the domain through `patch` for conductivity `k`:
    /// `-k * sum(dT/dn * |S|)`.
    pub fn heat_flow_out(&self, patch: &str, k: f64) -> f64 {
        let g = &self.geom;
        let p = g.mesh.patch(patch).expect("known patch");
        let grad = self.wall_gradient(patch);
        -k * p
            .faces()
            .zip(&grad)
            .map(|(f, gr)| gr * vec3::norm(g.mesh.face_area_vectors()[f]))
            .sum::<f64>()
    }
}

/// Boussinesq body acceleration `-beta (T - t_ref) g` of a temperature field, with the
/// balanced-force face values (see [`BodyForce`]).
pub fn boussinesq_force(geom: &FvGeometry, t: &ScalarField, beta: f64, gravity: Vec3, t_ref: f64) -> BodyForce {
    let dt: Vec<f64> = t.values.iter().map(|v| v - t_ref).collect();
    let bt: Vec<f64> = t.boundary_values(geom).iter().map(|v| v - t_ref).collect();
    BodyForce::from_scalar(geom, &dt, &bt, vec3::scale(gravity, -beta))
}

/// Buoyancy-coupled steady flow: SIMPLE/SIMPLEC momentum with Boussinesq body
/// force plus the energy equation, iterated together.
pub struct BuoyantSimple {
    /// Flow solver.
    pub flow: SimpleSolver,
    /// Temperature solver.
    pub energy: EnergySolver,
    /// Volumetric thermal expansion coefficient (1/K).
    pub beta: f64,
    /// Gravity vector (m/s^2).
    pub gravity: Vec3,
    /// Reference temperature of the Boussinesq model (K).
    pub t_ref: f64,
}

/// Diagnostics of one coupled iteration.
#[derive(Debug, Clone, Copy)]
pub struct BuoyantInfo {
    /// Flow diagnostics.
    pub flow: IterationInfo,
    /// Normalised temperature residual.
    pub res_t: f64,
}

impl BuoyantSimple {
    /// Boussinesq body acceleration for the current temperature field.
    pub fn buoyancy(&self) -> BodyForce {
        boussinesq_force(&self.flow.geom, &self.energy.t, self.beta, self.gravity, self.t_ref)
    }

    /// One coupled iteration: buoyancy update, momentum/pressure, temperature.
    pub fn iterate(&mut self) -> BuoyantInfo {
        self.flow.body_force = Some(self.buoyancy());
        let flow = self.flow.iterate();
        let res_t = self.energy.iterate(&self.flow.phi);
        BuoyantInfo { flow, res_t }
    }
}
