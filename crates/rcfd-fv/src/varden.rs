//! Steady variable-density (ideal gas) SIMPLE/SIMPLEC on collocated meshes (ADR 0005).
//!
//! A separate solver next to the kinematic [`SimpleSolver`](crate::simple::SimpleSolver): absolute
//! pressure, dynamic viscosity, mass flux. The unknowns are the velocity, the pressure, the
//! temperature and the density `rho = P/(R T)`.
//!
//! * [`DensityMode::Compressible`]: `P = p` is the solved pressure; energy in total-enthalpy form;
//!   the pressure equation carries the upwinded `psi p` convective term (transonic formulation) so that
//!   it stays well posed up to and beyond Mach 1.
//! * [`DensityMode::LowMach`]: `P` is a spatially uniform thermodynamic pressure (fixed by the
//!   mass constraint in a closed domain), `p` is the dynamic pressure; energy in temperature form.
//!
//! Per iteration: properties and density from the equation of state, momentum predictor with the mass
//! flux (`div(m u) - Sp(div m) u - div(mu grad u) = -grad p + f`), pressure equation for the mass flux
//! (Rhie-Chow `HbyA` interpolation, density-weighted), velocity and flux correction, energy.

use crate::bc::{FaceCoeffs, FixedGradient, FixedValue, Profile};
use crate::convection::{convection, face_flux, ConvScheme};
use crate::diffusion::laplacian;
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{boundary_hby_a, boundary_values_pbc, divergence, face_interpolate, pressure_bc, PressureBc};
use crate::realgas::RealGas;
use crate::simple::{normalised_residual, BodyForce, Coupling};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Role of the pressure in the equation of state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityMode {
    /// `rho = p/(R T)` with the solved absolute pressure; total-enthalpy energy equation.
    Compressible,
    /// `rho = P_th/(R T)` with a uniform thermodynamic pressure; temperature energy equation.
    LowMach,
}

/// Interpolation of the density to the faces for the mass flux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityScheme {
    /// Upwind cell value.
    Upwind,
    /// Linear interpolation.
    Central,
    /// Upwind value extrapolated with the upwind-cell gradient.
    LinearUpwind,
}

/// A transport property as a function of temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Property {
    /// Constant value.
    Constant(f64),
    /// Sutherland's law `reference (T/t_ref)^1.5 (t_ref + s)/(T + s)`.
    Sutherland {
        /// Value at `t_ref`.
        reference: f64,
        /// Reference temperature (K).
        t_ref: f64,
        /// Sutherland constant (K).
        s: f64,
    },
}

impl Property {
    /// Value at temperature `t`.
    pub fn at(&self, t: f64) -> f64 {
        match *self {
            Property::Constant(v) => v,
            Property::Sutherland { reference, t_ref, s } => reference * (t / t_ref).powf(1.5) * (t_ref + s) / (t + s),
        }
    }
}

/// Calorically perfect gas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdealGas {
    /// Specific gas constant (J/kg/K).
    pub r: f64,
    /// Specific heat at constant pressure (J/kg/K).
    pub cp: f64,
}

impl IdealGas {
    /// Ratio of specific heats.
    pub fn gamma(&self) -> f64 {
        self.cp / (self.cp - self.r)
    }
    /// Density from pressure and temperature.
    pub fn rho(&self, p: f64, t: f64) -> f64 {
        p / (self.r * t)
    }
    /// Compressibility `d rho/d p = 1/(R T)`.
    pub fn psi(&self, t: f64) -> f64 {
        1.0 / (self.r * t)
    }
    /// Speed of sound.
    pub fn sound_speed(&self, t: f64) -> f64 {
        (self.gamma() * self.r * t).sqrt()
    }
}

/// A stagnation (total pressure and temperature) inlet on a patch: zero-gradient velocity, static
/// pressure and temperature from the isentropic relations at the owner-cell speed.
#[derive(Debug, Clone, PartialEq)]
pub struct StagnationInlet {
    /// Patch name.
    pub patch: String,
    /// Total pressure (Pa).
    pub p0: f64,
    /// Total temperature (K).
    pub t0: f64,
}

/// Solver settings.
#[derive(Debug, Clone)]
pub struct VdSettings {
    /// Pressure-velocity coupling.
    pub coupling: Coupling,
    /// Equation-of-state mode.
    pub mode: DensityMode,
    /// Gas.
    pub gas: IdealGas,
    /// Real-gas model (ADR 0007). When set it replaces the ideal-gas equation of state and enthalpy; `gas.cp` must
    /// equal its `cp0` (used for the energy diffusivity `k/cp`) and `gas.r` its `R_u/M`. Compressible mode only.
    pub real: Option<RealGas>,
    /// Dynamic viscosity (Pa s).
    pub mu: Property,
    /// Thermal conductivity (W/m/K).
    pub k: Property,
    /// Gravity (m/s^2).
    pub gravity: Vec3,
    /// Thermodynamic pressure of the low-Mach mode at the start (Pa); also fixes the mass of a closed domain.
    pub reference_pressure: f64,
    /// Velocity under-relaxation.
    pub alpha_u: f64,
    /// Pressure under-relaxation.
    pub alpha_p: f64,
    /// Density under-relaxation.
    pub alpha_rho: f64,
    /// Energy under-relaxation.
    pub alpha_h: f64,
    /// Momentum convection scheme.
    pub convection: ConvScheme,
    /// Energy convection scheme.
    pub energy_convection: ConvScheme,
    /// Face-density scheme of the mass flux.
    pub density_scheme: DensityScheme,
    /// Reference cell and value when the pressure level is undetermined.
    pub p_ref: (usize, f64),
    /// H operator from the solved predictor.
    pub h_from_solution: bool,
    /// Stagnation inlets.
    pub inlets: Vec<StagnationInlet>,
    /// Linear solver controls for momentum.
    pub momentum_solver: SolveControl,
    /// Linear solver controls for pressure.
    pub pressure_solver: SolveControl,
    /// Linear solver controls for energy.
    pub energy_solver: SolveControl,
}

impl VdSettings {
    /// Defaults for a gas and coupling.
    pub fn new(gas: IdealGas, mu: Property, k: Property, coupling: Coupling) -> Self {
        let (alpha_u, alpha_p) = match coupling {
            Coupling::Simple => (0.7, 0.3),
            Coupling::Simplec => (0.9, 1.0),
        };
        VdSettings {
            coupling,
            mode: DensityMode::Compressible,
            gas,
            real: None,
            mu,
            k,
            gravity: [0.0; 3],
            reference_pressure: 101_325.0,
            alpha_u,
            alpha_p,
            alpha_rho: 1.0,
            alpha_h: 0.9,
            convection: ConvScheme::LinearUpwind,
            energy_convection: ConvScheme::LinearUpwind,
            density_scheme: DensityScheme::LinearUpwind,
            p_ref: (0, 0.0),
            h_from_solution: true,
            inlets: Vec::new(),
            momentum_solver: SolveControl {
                tol_abs: 1e-12,
                tol_rel: 1e-3,
                max_iter: 300,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-3,
                max_iter: 3000,
            },
            energy_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-4,
                max_iter: 500,
            },
        }
    }
}

/// Diagnostics of one iteration.
#[derive(Debug, Clone, Copy)]
pub struct VdInfo {
    /// Iteration number (1-based).
    pub iteration: usize,
    /// Largest velocity change.
    pub max_du: f64,
    /// Largest cell mass imbalance divided by `max(rho) * max|S|`.
    pub continuity: f64,
    /// Linear iterations on pressure.
    pub p_iters: usize,
    /// Normalised initial momentum residual.
    pub res_u: f64,
    /// Normalised initial pressure residual.
    pub res_p: f64,
    /// Normalised initial energy residual.
    pub res_h: f64,
}

/// Steady variable-density solver.
pub struct VarDensitySolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: VdSettings,
    /// Velocity.
    pub u: VectorField,
    /// Pressure (absolute in compressible mode, dynamic in low-Mach mode).
    pub p: ScalarField,
    /// Temperature (carries the user's thermal boundary conditions).
    pub t: ScalarField,
    /// Cell density.
    pub rho: Vec<f64>,
    /// Mass flux (kg/s, positive out of the owner).
    pub phi: Vec<f64>,
    /// Thermodynamic pressure of the low-Mach mode.
    pub p_th: f64,
    /// Mass of the closed domain (kg per unit depth of the mesh).
    pub mass: f64,
    /// Reference density removed from the body force (low-Mach mode).
    pub rho_ref: f64,
    /// Iteration diagnostics.
    pub history: Vec<VdInfo>,
    h: ScalarField,
    iteration: usize,
    body_ratio: Vec<f64>,
    /// Volumetric flux of the previous iterate (deferred high-order density correction, transonic term).
    vflux: Vec<f64>,
    /// Inverse of `sum_f S S / |S|` per cell (face-to-cell vector reconstruction).
    rec_inv: Vec<[[f64; 3]; 3]>,
}

fn invert3(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |i: usize, j: usize| {
        let (i1, i2, j1, j2) = ((i + 1) % 3, (i + 2) % 3, (j + 1) % 3, (j + 2) % 3);
        m[i1][j1] * m[i2][j2] - m[i1][j2] * m[i2][j1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = c(j, i) / det;
        }
    }
    r
}

fn mismatch(path: &str, reason: impl Into<String>) -> Error {
    Error::InvalidValue {
        path: path.into(),
        reason: reason.into(),
    }
}

impl VarDensitySolver {
    /// Create the solver; the initial density, the mass of the domain and the initial flux follow from the
    /// initial `p`, `T` and `u`.
    pub fn new(
        geom: Arc<FvGeometry>,
        u: VectorField,
        p: ScalarField,
        t: ScalarField,
        settings: VdSettings,
    ) -> Result<Self> {
        let s = &settings;
        if !(s.gas.r > 0.0 && s.gas.cp > s.gas.r) {
            return Err(mismatch("gas", "needs R > 0 and cp > R"));
        }
        if let Some(rg) = &s.real {
            if s.mode != DensityMode::Compressible {
                return Err(mismatch("fluid.mode", "a real gas needs the compressible mode"));
            }
            if (rg.cp0 / s.gas.cp - 1.0).abs() > 1e-9 || (rg.r() / s.gas.r - 1.0).abs() > 1e-9 {
                return Err(mismatch(
                    "gas",
                    "the ideal-gas settings must carry cp0 and R_u/M of the real gas",
                ));
            }
        }
        if t.values.iter().any(|&x| x <= 0.0 || !x.is_finite()) {
            return Err(mismatch("temperature", "the temperature must be positive"));
        }
        let p_eos = |c: usize| match s.mode {
            DensityMode::Compressible => p.values[c],
            DensityMode::LowMach => s.reference_pressure,
        };
        if (0..geom.n_cells()).any(|c| p_eos(c) <= 0.0 || !p_eos(c).is_finite()) {
            return Err(mismatch("pressure", "the absolute pressure must be positive"));
        }
        let rho: Vec<f64> = (0..geom.n_cells())
            .map(|c| match &s.real {
                Some(rg) => rg.rho(p_eos(c), t.values[c]),
                None => s.gas.rho(p_eos(c), t.values[c]),
            })
            .collect();
        let vol = geom.mesh.cell_volumes();
        let mass: f64 = rho.iter().zip(vol).map(|(r, v)| r * v).sum();
        let rho_ref = mass / vol.iter().sum::<f64>();
        let h = ScalarField::uniform("h0", &geom, 0.0);
        let mut sol = VarDensitySolver {
            geom,
            p_th: settings.reference_pressure,
            settings,
            u,
            p,
            t,
            rho,
            phi: Vec::new(),
            mass,
            rho_ref,
            history: Vec::new(),
            h,
            iteration: 0,
            body_ratio: Vec::new(),
            vflux: Vec::new(),
            rec_inv: Vec::new(),
        };
        sol.rec_inv = sol.build_reconstruction();
        sol.h.values = (0..sol.geom.n_cells())
            .map(|c| sol.h_static(sol.p.values[c], sol.t.values[c]) + 0.5 * vec3::dot(sol.u.values[c], sol.u.values[c]))
            .collect();
        for inl in &sol.settings.inlets {
            if sol.geom.mesh.patch(&inl.patch).is_none() {
                return Err(Error::PathNotFound(format!("patch {}", inl.patch)));
            }
        }
        let pbc = sol.effective_pbc();
        let rho_b = sol.boundary_density(&pbc, &sol.p.values.clone());
        sol.phi = sol.fixed_or_zero_flux(&pbc, &rho_b);
        // initial flux of the initial velocity field on the faces that are not prescribed
        let ub = sol.u.boundary_values(&sol.geom);
        let vol_flux = face_flux(&sol.geom, &sol.u.values, &ub);
        let rho_lin = sol.face_values(&sol.rho, &rho_b);
        sol.vflux = vec![0.0; sol.phi.len()];
        for f in 0..sol.phi.len() {
            let fixed = f >= sol.geom.n_internal && pbc.fixed_u[f - sol.geom.n_internal];
            if !sol.geom.empty[f] && !fixed {
                sol.vflux[f] = vol_flux[f];
                sol.phi[f] = rho_lin[f] * vol_flux[f];
            }
        }
        Ok(sol)
    }

    fn build_reconstruction(&self) -> Vec<[[f64; 3]; 3]> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let sf = mesh.face_area_vectors();
        (0..g.n_cells())
            .map(|c| {
                let mut m = [[0.0; 3]; 3];
                for &f in mesh.cell_faces(c) {
                    if g.empty[f] {
                        continue;
                    }
                    let a = vec3::norm(sf[f]);
                    for i in 0..3 {
                        for j in 0..3 {
                            m[i][j] += sf[f][i] * sf[f][j] / a;
                        }
                    }
                }
                for k in 0..3 {
                    if !g.active[k] {
                        m[k][k] += 1.0;
                    }
                }
                invert3(m)
            })
            .collect()
    }

    /// Cell vector reconstructed from face quantities `ssf` (a force or velocity times area along `S`):
    /// `(sum S S/|S|)^-1 sum S/|S| ssf`. Exact for a uniform vector field and zero when every face quantity is zero.
    fn reconstruct(&self, ssf: &[f64]) -> Vec<Vec3> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let sf = mesh.face_area_vectors();
        (0..g.n_cells())
            .map(|c| {
                let mut r = [0.0; 3];
                for &f in mesh.cell_faces(c) {
                    if g.empty[f] {
                        continue;
                    }
                    let a = vec3::norm(sf[f]);
                    for k in 0..3 {
                        r[k] += sf[f][k] / a * ssf[f];
                    }
                }
                let m = &self.rec_inv[c];
                [0, 1, 2].map(|i| m[i][0] * r[0] + m[i][1] * r[1] + m[i][2] * r[2])
            })
            .collect()
    }

    /// Initialise the pressure (and with it the density) to the hydrostatic state of the current temperature
    /// field: Picard iterations of `div(rho (f - grad p)) = 0` with the wall pressure gradients balancing the
    /// body force, the mass of the domain kept (compressible mode: the level follows from the mass; low-Mach mode:
    /// the dynamic pressure is pinned at the reference cell). The velocity is not changed.
    pub fn initialise_hydrostatic(&mut self, sweeps: usize) {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let vol = mesh.cell_volumes();
        let n_int = g.n_internal;
        if vec3::norm(self.settings.gravity) == 0.0 {
            return;
        }
        let compressible = self.settings.mode == DensityMode::Compressible;
        for _ in 0..sweeps {
            let pbc = self.effective_pbc();
            let rho_b = self.boundary_density(&pbc, &self.p.values.clone());
            let shift = if compressible { 0.0 } else { self.rho_ref };
            let cells: Vec<f64> = self.rho.iter().map(|r| r - shift).collect();
            let bnd: Vec<f64> = rho_b.iter().map(|r| r - shift).collect();
            let bf = BodyForce::from_scalar(&g, &cells, &bnd, self.settings.gravity);
            let rho_lin = self.face_values(&self.rho, &rho_b);
            let mut flux = vec![0.0; mesh.n_faces()];
            for f in 0..mesh.n_faces() {
                if !g.empty[f] && !(f >= n_int && pbc.fixed_u[f - n_int]) {
                    flux[f] = rho_lin[f] * bf.face[f];
                }
            }
            let div = divergence(&g, &flux);
            let mut pm: FvMatrix = laplacian(&g, &rho_lin, &pbc.coeffs, None);
            for (b, d) in pm.b.iter_mut().zip(&div) {
                *b -= d;
            }
            if !pbc.has_dirichlet() {
                let (cell, value) = self.settings.p_ref;
                let d = pm.a.diag[cell];
                pm.a.diag[cell] += d;
                pm.b[cell] += d * value;
            }
            let mut p = self.p.values.clone();
            let ctl = SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-12,
                max_iter: 5000,
            };
            if pm.a.is_symmetric() && n > 2000 {
                pm.solve_amg(&mut p, &ctl);
            } else {
                pm.solve(&mut p, &ctl);
            }
            self.p.values = p;
            if compressible && !pbc.has_dirichlet() {
                let (mut num, mut den) = (0.0, 0.0);
                for c in 0..n {
                    let psi = self.psi_eos(self.p.values[c], self.t.values[c]);
                    num += vol[c] * psi * self.p.values[c];
                    den += vol[c] * psi;
                }
                let sh = (self.mass - num) / den;
                for pv in self.p.values.iter_mut() {
                    *pv += sh;
                }
            }
            if compressible {
                for c in 0..n {
                    self.rho[c] = self.rho_eos(self.p.values[c], self.t.values[c]);
                }
            }
        }
    }

    /// Density from the equation of state (ideal gas or the real-gas model).
    fn rho_eos(&self, p: f64, t: f64) -> f64 {
        match &self.settings.real {
            Some(rg) => rg.rho(p, t),
            None => self.settings.gas.rho(p, t),
        }
    }

    /// Compressibility `d rho/d p` at fixed temperature.
    fn psi_eos(&self, p: f64, t: f64) -> f64 {
        match &self.settings.real {
            Some(rg) => rg.psi(p, t),
            None => self.settings.gas.psi(t),
        }
    }

    /// Speed of sound.
    fn sound_speed_eos(&self, p: f64, t: f64) -> f64 {
        match &self.settings.real {
            Some(rg) => rg.sound_speed(p, t),
            None => self.settings.gas.sound_speed(t),
        }
    }

    /// Static specific enthalpy (J/kg): `cp T`, or `cp0 T` plus the departure for a real gas.
    fn h_static(&self, p: f64, t: f64) -> f64 {
        match &self.settings.real {
            Some(rg) => rg.h(p, t),
            None => self.settings.gas.cp * t,
        }
    }

    /// Temperature from the static enthalpy `h` at pressure `p` (`guess` for the real-gas Newton iteration).
    fn t_from_h_eos(&self, h: f64, p: f64, guess: f64) -> f64 {
        match &self.settings.real {
            Some(rg) => rg.t_from_h(h, p, guess).max(1.0),
            None => (h / self.settings.gas.cp).max(1.0),
        }
    }

    /// Whether no pressure boundary condition fixes the level.
    pub fn is_closed(&self) -> bool {
        !self.effective_pbc().has_dirichlet()
    }

    /// Mass flux through the faces of a patch (kg/s, positive out of the domain).
    pub fn mass_flow(&self, patch: &str) -> Result<f64> {
        let p = self
            .geom
            .mesh
            .patch(patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
        Ok(p.faces().map(|f| self.phi[f]).sum())
    }

    /// Conductive heat flow out of the domain through a patch (W per unit mesh depth): `-sum k(T_b) dT/dn |S|`
    /// with the boundary-face temperature `T_b`.
    pub fn heat_flow_out(&self, patch: &str) -> Result<f64> {
        let g = &self.geom;
        let p = g
            .mesh
            .patch(patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
        let coeffs = self.t.boundary_coeffs(g);
        let own = g.mesh.owner();
        let sf = g.mesh.face_area_vectors();
        Ok(-p
            .faces()
            .map(|f| {
                let c = &coeffs[f - g.n_internal];
                let tc = self.t.values[own[f]];
                self.settings.k.at(c.value(tc)) * c.gradient(tc) * vec3::norm(sf[f])
            })
            .sum::<f64>())
    }

    /// Mach number of every cell.
    pub fn mach(&self) -> Vec<f64> {
        (0..self.geom.n_cells())
            .map(|c| vec3::norm(self.u.values[c]) / self.sound_speed_eos(self.p.values[c], self.t.values[c]))
            .collect()
    }

    /// Total mass in the domain.
    pub fn total_mass(&self) -> f64 {
        self.rho
            .iter()
            .zip(self.geom.mesh.cell_volumes())
            .map(|(r, v)| r * v)
            .sum()
    }

    /// Absolute pressure of every cell (the thermodynamic pressure plus the dynamic one in low-Mach mode).
    pub fn absolute_pressure(&self) -> Vec<f64> {
        match self.settings.mode {
            DensityMode::Compressible => self.p.values.clone(),
            DensityMode::LowMach => self.p.values.iter().map(|d| self.p_th + d).collect(),
        }
    }

    /// Pressure conditions: the shared construction plus slip (symmetry) faces, which have a prescribed zero
    /// normal velocity also when the face is not axis-aligned.
    fn effective_pbc(&self) -> PressureBc {
        let g = &self.geom;
        let mut pbc = pressure_bc(g, &self.u, &self.p);
        for (pi, patch) in g.mesh.patches().iter().enumerate() {
            if self.u.bc(pi).type_name() == "symmetry" {
                for f in patch.faces() {
                    let i = f - g.n_internal;
                    pbc.fixed_u[i] = true;
                    pbc.coeffs[i] = FaceCoeffs {
                        a: 1.0,
                        b: 0.0,
                        c: 0.0,
                        d: 0.0,
                    };
                }
            }
        }
        pbc
    }

    /// Update the boundary conditions of stagnation inlets from the current owner-cell state.
    fn update_inlets(&mut self) {
        let g = self.geom.clone();
        let (cp, gamma) = (self.settings.gas.cp, self.settings.gas.gamma());
        let own = g.mesh.owner();
        for inl in self.settings.inlets.clone() {
            let patch = g.mesh.patch(&inl.patch).expect("checked in new");
            let (mut pv, mut tv) = (Vec::new(), Vec::new());
            for f in patch.faces() {
                let uc = self.u.values[own[f]];
                if let Some(rg) = &self.settings.real {
                    let (ps, ts) = rg.static_from_stagnation(inl.p0, inl.t0, 0.5 * vec3::dot(uc, uc));
                    tv.push([ts, 0.0, 0.0]);
                    pv.push([ps, 0.0, 0.0]);
                    continue;
                }
                let tb = (inl.t0 - 0.5 * vec3::dot(uc, uc) / cp).max(0.05 * inl.t0);
                tv.push([tb, 0.0, 0.0]);
                pv.push([inl.p0 * (tb / inl.t0).powf(gamma / (gamma - 1.0)), 0.0, 0.0]);
            }
            let _ = self
                .p
                .set_bc(&g, &inl.patch, Box::new(FixedValue(Profile::PerFace(pv))));
            let _ = self
                .t
                .set_bc(&g, &inl.patch, Box::new(FixedValue(Profile::PerFace(tv))));
        }
    }

    /// Boundary-face density from the boundary values of the pressure and temperature.
    fn boundary_density(&self, pbc: &PressureBc, p_cells: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let tb = self.t.boundary_values(g);
        match self.settings.mode {
            DensityMode::Compressible => {
                let pb = boundary_values_pbc(g, pbc, p_cells);
                (0..tb.len()).map(|i| self.rho_eos(pb[i], tb[i])).collect()
            }
            DensityMode::LowMach => tb.iter().map(|&t| self.settings.gas.rho(self.p_th, t)).collect(),
        }
    }

    /// Mass flux of the prescribed-velocity boundary faces (zero elsewhere).
    fn fixed_or_zero_flux(&self, pbc: &PressureBc, rho_b: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let sf = mesh.face_area_vectors();
        let ub = self.u.boundary_values(g);
        let vf = face_flux(g, &self.u.values, &ub);
        (0..mesh.n_faces())
            .map(|f| {
                if f < g.n_internal {
                    let w = g.weights[f];
                    let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
                    (w * self.rho[o] + (1.0 - w) * self.rho[n]) * vf[f]
                } else if g.empty[f] {
                    0.0
                } else {
                    let i = f - g.n_internal;
                    let _ = pbc;
                    rho_b[i] * vec3::dot(ub[i], sf[f])
                }
            })
            .collect()
    }

    /// Face values (linear interpolation, boundary values given) of a cell field.
    fn face_values(&self, cells: &[f64], bvals: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mut out = face_interpolate(g, cells);
        for (i, b) in bvals.iter().enumerate() {
            out[g.n_internal + i] = *b;
        }
        out
    }

    /// Density at the faces for the explicit mass flux, upwinded with the sign of `q`.
    fn density_faces(&self, q: &[f64], rho_b: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let grad =
            (self.settings.density_scheme == DensityScheme::LinearUpwind).then(|| least_squares(g, &self.rho, rho_b));
        (0..mesh.n_faces())
            .map(|f| {
                if f >= g.n_internal {
                    return rho_b[f - g.n_internal];
                }
                let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
                let up = if q[f] >= 0.0 { o } else { n };
                match self.settings.density_scheme {
                    DensityScheme::Upwind => self.rho[up],
                    DensityScheme::Central => {
                        let w = g.weights[f];
                        w * self.rho[o] + (1.0 - w) * self.rho[n]
                    }
                    DensityScheme::LinearUpwind => {
                        let gr = grad.as_ref().expect("gradient")[up];
                        self.rho[up] + vec3::dot(gr, vec3::sub(mesh.face_centres()[f], mesh.cell_centres()[up]))
                    }
                }
            })
            .collect()
    }

    /// Thermodynamic-pressure and density update after pressure and energy changed.
    fn update_density(&mut self, first_relaxation: bool) {
        let n = self.geom.n_cells();
        let vol = self.geom.mesh.cell_volumes();
        let gas = self.settings.gas;
        let a = if first_relaxation { 1.0 } else { self.settings.alpha_rho };
        if self.settings.mode == DensityMode::LowMach && self.is_closed() {
            let inv_t: f64 = (0..n).map(|c| vol[c] / self.t.values[c]).sum();
            self.p_th = self.mass * gas.r / inv_t;
        }
        for c in 0..n {
            let pe = match self.settings.mode {
                DensityMode::Compressible => self.p.values[c],
                DensityMode::LowMach => self.p_th,
            };
            let target = self.rho_eos(pe, self.t.values[c]);
            self.rho[c] += a * (target - self.rho[c]);
        }
    }

    /// Set the energy-field boundary conditions from the thermal conditions on `t`.
    fn update_energy_bcs(&mut self) {
        let g = self.geom.clone();
        let cp = self.settings.gas.cp;
        let tc = self.t.boundary_coeffs(&g);
        let ub = self.u.boundary_values(&g);
        let own = g.mesh.owner();
        let pbv = self.p.boundary_values(&g);
        for patch in g.mesh.patches() {
            if patch.kind == rcfd_mesh::PatchKind::Empty || patch.kind == rcfd_mesh::PatchKind::Cyclic {
                continue;
            }
            let first = patch.faces().next();
            let Some(f0) = first else { continue };
            let stagnation = self.settings.inlets.iter().find(|i| i.patch == patch.name);
            if tc[f0 - g.n_internal].a == 0.0 {
                let vals = patch
                    .faces()
                    .map(|f| {
                        let i = f - g.n_internal;
                        let kin = 0.5 * vec3::dot(ub[i], ub[i]);
                        let v = match stagnation {
                            Some(s) => self.h_static(s.p0, s.t0),
                            None => self.h_static(pbv[i], tc[i].value(self.t.values[own[f]])) + kin,
                        };
                        [v, 0.0, 0.0]
                    })
                    .collect();
                let _ = self
                    .h
                    .set_bc(&g, &patch.name, Box::new(FixedValue(Profile::PerFace(vals))));
            } else {
                let vals = patch
                    .faces()
                    .map(|f| [cp * tc[f - g.n_internal].gradient(self.t.values[own[f]]), 0.0, 0.0])
                    .collect();
                let _ = self
                    .h
                    .set_bc(&g, &patch.name, Box::new(FixedGradient(Profile::PerFace(vals))));
            }
        }
    }

    /// One energy sweep (total enthalpy or temperature); returns the normalised residual.
    fn solve_energy(&mut self) -> f64 {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let s = self.settings.clone();
        let compressible = s.mode == DensityMode::Compressible;
        if compressible {
            self.update_energy_bcs();
        }
        let (field_bc, cells): (Vec<FaceCoeffs>, Vec<f64>) = if compressible {
            (self.h.boundary_coeffs(&g), self.h.values.clone())
        } else {
            (self.t.boundary_coeffs(&g), self.t.values.clone())
        };
        let bvals: Vec<f64> = (0..field_bc.len())
            .map(|i| field_bc[i].value(cells[mesh.owner()[g.n_internal + i]]))
            .collect();
        let grad = s
            .energy_convection
            .needs_gradient()
            .then(|| least_squares(&g, &cells, &bvals));
        // diffusivity k/cp at faces
        let tb = self.t.boundary_values(&g);
        let tf = self.face_values(&self.t.values, &tb);
        let gamma: Vec<f64> = tf.iter().map(|&t| s.k.at(t) / s.gas.cp).collect();
        let mut m = convection(&g, &self.phi, &cells, &field_bc, s.energy_convection, grad.as_deref());
        m.add(&laplacian(&g, &gamma, &field_bc, grad.as_deref()));
        let div = divergence(&g, &self.phi);
        for c in 0..n {
            m.a.diag[c] -= div[c];
        }
        if compressible {
            // diffusion of the kinetic energy: -div(Gamma grad K) on the right-hand side
            let kin: Vec<f64> = self.u.values.iter().map(|v| 0.5 * vec3::dot(*v, *v)).collect();
            let ub = self.u.boundary_values(&g);
            let sf = mesh.face_area_vectors();
            for f in 0..mesh.n_faces() {
                if g.empty[f] {
                    continue;
                }
                if f < g.n_internal {
                    let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
                    let flux = gamma[f] * g.lap_coeff[f] * (kin[nb] - kin[o]);
                    m.b[o] -= flux;
                    m.b[nb] += flux;
                } else {
                    let i = f - g.n_internal;
                    let o = mesh.owner()[f];
                    let kb = 0.5 * vec3::dot(ub[i], ub[i]);
                    m.b[o] -= gamma[f] * vec3::norm(sf[f]) * (kb - kin[o]) / g.dn[i];
                }
            }
        }
        let res = normalised_residual(&m, &cells);
        m.relax(s.alpha_h, &cells);
        let mut x = cells;
        m.solve(&mut x, &s.energy_solver);
        if compressible {
            self.h.values = x;
            for c in 0..n {
                let kin = 0.5 * vec3::dot(self.u.values[c], self.u.values[c]);
                self.t.values[c] = self.t_from_h_eos(self.h.values[c] - kin, self.p.values[c], self.t.values[c]);
            }
        } else {
            self.t.values = x.iter().map(|&v| v.max(1.0)).collect();
        }
        res
    }

    /// Run one iteration.
    pub fn iterate(&mut self) -> VdInfo {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let vol = mesh.cell_volumes();
        let sf = mesh.face_area_vectors();
        let n_int = g.n_internal;
        let s = self.settings.clone();
        let compressible = s.mode == DensityMode::Compressible;
        self.iteration += 1;
        let u_old = self.u.values.clone();
        let p_old = self.p.values.clone();

        self.update_inlets();
        let mut pbc = self.effective_pbc();
        let rho_b = self.boundary_density(&pbc, &p_old);
        let tb = self.t.boundary_values(&g);
        let t_face = self.face_values(&self.t.values, &tb);
        let mu_f: Vec<f64> = t_face.iter().map(|&t| s.mu.at(t)).collect();

        // body force per unit volume
        let body: Option<BodyForce> = (vec3::norm(s.gravity) > 0.0).then(|| {
            let shift = if compressible { 0.0 } else { self.rho_ref };
            let cells: Vec<f64> = self.rho.iter().map(|r| r - shift).collect();
            let bnd: Vec<f64> = rho_b.iter().map(|r| r - shift).collect();
            BodyForce::from_scalar(&g, &cells, &bnd, s.gravity)
        });
        // matrix coefficients: prescribed-velocity faces carry a fixed flux, so no pressure-gradient flux there
        let pbc_mat = pbc.clone();
        if let Some(bf) = &body {
            for i in 0..pbc.coeffs.len() {
                if pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                    let f = n_int + i;
                    let ratio = self.body_ratio.get(mesh.owner()[f]).copied().unwrap_or(1.0);
                    let gradient = ratio * bf.face[f] / vec3::norm(sf[f]).max(1e-300);
                    pbc.coeffs[i] = FaceCoeffs {
                        a: 1.0,
                        b: g.dn[i] * gradient,
                        c: 0.0,
                        d: gradient,
                    };
                }
            }
        }

        // ---- momentum predictor ----------------------------------------------------------
        let p_bvals = boundary_values_pbc(&g, &pbc, &p_old);
        let grad_p = green_gauss(&g, &p_old, &p_bvals, 0);
        // With a body force the pressure gradient and the force enter the momentum source as one face-based
        // (balanced) quantity, reconstructed to the cell: a hydrostatic state then has exactly zero source.
        let balanced: Option<Vec<Vec3>> = body.as_ref().map(|bf| {
            let mut ssf = vec![0.0; mesh.n_faces()];
            for f in 0..mesh.n_faces() {
                if g.empty[f] {
                    continue;
                }
                ssf[f] = if f < n_int {
                    bf.face[f] - g.lap_coeff[f] * (p_old[mesh.neighbour()[f]] - p_old[mesh.owner()[f]])
                } else if pbc.fixed_u[f - n_int] {
                    0.0
                } else {
                    bf.face[f] - vec3::norm(sf[f]) * pbc.coeffs[f - n_int].gradient(p_old[mesh.owner()[f]])
                };
            }
            self.reconstruct(&ssf)
        });
        let div_phi = divergence(&g, &self.phi);
        let mut inlet_u: Vec<(usize, Vec3)> = Vec::new();
        for inl in &s.inlets {
            if let Some(patch) = mesh.patch(&inl.patch) {
                for f in patch.faces() {
                    let i = f - n_int;
                    let speed = self.phi[f] / (rho_b[i] * vec3::norm(sf[f])).max(1e-300);
                    let nhat = vec3::scale(sf[f], 1.0 / vec3::norm(sf[f]));
                    inlet_u.push((i, vec3::scale(nhat, speed)));
                }
            }
        }
        let mut a_p = vec![0.0; n];
        let mut h = vec![[0.0; 3]; n];
        let mut h1 = vec![0.0; n];
        let mut n_active = 0.0;
        let mut res_u: f64 = 0.0;
        let ub = self.u.boundary_values(&g);
        for k in 0..3 {
            if !g.active[k] {
                continue;
            }
            n_active += 1.0;
            let cells: Vec<f64> = u_old.iter().map(|v| v[k]).collect();
            let mut bc = self.u.boundary_coeffs(&g, k);
            let mut bvals: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            // stagnation inlets: the inflow velocity follows the mass flux (explicit, like OpenFOAM's
            // pressureInletVelocity); an implicit zero-gradient inflow would remove the convective diagonal
            for &(i, v) in &inlet_u {
                bc[i] = FaceCoeffs::from_value(0.0, v[k], g.dn[i]);
                bvals[i] = v[k];
            }
            let grad = s.convection.needs_gradient().then(|| least_squares(&g, &cells, &bvals));
            let mut m = convection(&g, &self.phi, &cells, &bc, s.convection, grad.as_deref());
            m.add(&laplacian(&g, &mu_f, &bc, grad.as_deref()));
            for c in 0..n {
                m.a.diag[c] -= div_phi[c];
            }
            m.relax(s.alpha_u, &cells);
            let mut hk = m.a.h_operator(&cells, &m.b);
            let off = m.a.off_diag_abs_sum();
            for c in 0..n {
                a_p[c] += m.a.diag[c];
                h1[c] += off[c];
            }
            let mut sys = m.clone();
            for c in 0..n {
                match &balanced {
                    Some(bal) => sys.b[c] += vol[c] * bal[c][k],
                    None => sys.b[c] -= vol[c] * grad_p[c][k],
                }
            }
            res_u = res_u.max(normalised_residual(&sys, &cells));
            let mut x = cells;
            sys.solve(&mut x, &s.momentum_solver);
            if s.h_from_solution {
                hk = m.a.h_operator(&x, &m.b);
            }
            for c in 0..n {
                h[c][k] = hk[c];
                self.u.values[c][k] = x[c];
            }
        }
        for c in 0..n {
            a_p[c] /= n_active;
            h1[c] /= n_active;
        }

        // ---- pressure equation -------------------------------------------------------------
        let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
        let r_atu: Vec<f64> = match s.coupling {
            Coupling::Simple => r_au.clone(),
            Coupling::Simplec => (0..n).map(|c| vol[c] / (a_p[c] - h1[c]).max(1e-3 * a_p[c])).collect(),
        };
        self.body_ratio = (0..n).map(|c| r_au[c] / r_atu[c]).collect();
        let hby_a0: Vec<Vec3> = (0..n)
            .map(|c| {
                let mut v = [0.0; 3];
                for k in 0..3 {
                    if g.active[k] {
                        v[k] = h[c][k] / a_p[c];
                    }
                }
                v
            })
            .collect();
        let mut hby_a = hby_a0.clone();
        if let Some(bf) = &body {
            for c in 0..n {
                for k in 0..3 {
                    if g.active[k] {
                        hby_a[c][k] += r_au[c] * bf.cell[c][k];
                    }
                }
            }
        }
        if s.coupling == Coupling::Simplec {
            for c in 0..n {
                for k in 0..3 {
                    hby_a[c][k] -= (r_au[c] - r_atu[c]) * grad_p[c][k];
                }
            }
        }
        // volumetric flux of HbyA (prescribed-velocity boundary faces carry their own flux)
        let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a0);
        let mut q = face_flux(&g, &hby_a0, &hb);
        if let Some(bf) = &body {
            let r_au_f = face_interpolate(&g, &r_au);
            for f in 0..mesh.n_faces() {
                if !g.empty[f] {
                    q[f] += r_au_f[f] * bf.face[f];
                }
            }
        }
        if s.coupling == Coupling::Simplec {
            for f in 0..n_int {
                let w = g.weights[f];
                let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
                let dr = w * (r_atu[o] - r_au[o]) + (1.0 - w) * (r_atu[nb] - r_au[nb]);
                q[f] += dr * g.lap_coeff[f] * (p_old[nb] - p_old[o]);
            }
        }
        // explicit mass flux and compressibility convection
        let rho_ho = self.density_faces(&q, &rho_b);
        let rho_lin = self.face_values(&self.rho, &rho_b);
        let mut m_exp = vec![0.0; mesh.n_faces()];
        let mut m_base = vec![0.0; mesh.n_faces()];
        let mut phid = vec![0.0; mesh.n_faces()];
        let mut p_up_old = vec![0.0; mesh.n_faces()];
        // volumetric flux of the previous iterate: the density change `psi (p' - p*)` multiplies the flux the
        // pressure correction itself produces, so the linearisation uses the current corrected flux, not `HbyA`
        let v_prev: Vec<f64> = self.vflux.clone();
        for f in 0..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            if f >= n_int && pbc.fixed_u[f - n_int] {
                m_exp[f] = rho_b[f - n_int] * vec3::dot(ub[f - n_int], sf[f]);
                m_base[f] = m_exp[f];
                continue;
            }
            // mass flux = rho_lin * (volumetric flux) + deferred (rho_ho - rho_lin) * (previous volumetric flux)
            m_exp[f] = rho_lin[f] * q[f] + (rho_ho[f] - rho_lin[f]) * v_prev[f];
            m_base[f] = m_exp[f];
            if compressible {
                if f < n_int {
                    let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
                    let up = if q[f] >= 0.0 { o } else { nb };
                    phid[f] = self.psi_eos(p_old[up], self.t.values[up]) * v_prev[f];
                    p_up_old[f] = p_old[up];
                } else {
                    let i = f - n_int;
                    phid[f] = self.psi_eos(p_bvals[i], tb[i]) * v_prev[f];
                    p_up_old[f] = p_bvals[i];
                }
                m_exp[f] -= phid[f] * p_up_old[f];
            }
        }
        let gamma_p: Vec<f64> = {
            let r_atu_f = face_interpolate(&g, &r_atu);
            (0..mesh.n_faces()).map(|f| rho_lin[f] * r_atu_f[f]).collect()
        };
        let div_exp = divergence(&g, &m_exp);
        let mut pm: FvMatrix = laplacian(&g, &gamma_p, &pbc_mat.coeffs, None);
        for (b, d) in pm.b.iter_mut().zip(&div_exp) {
            *b -= d;
        }
        if compressible {
            pm.add(&convection(
                &g,
                &phid,
                &p_old,
                &pbc_mat.coeffs,
                ConvScheme::Upwind,
                None,
            ));
        }
        let res_p = normalised_residual(&pm, &p_old);
        let closed = !pbc.has_dirichlet();
        if closed {
            // the level is pinned at the current value (a zero change); the level itself is restored below
            let cell = s.p_ref.0;
            let d = pm.a.diag[cell];
            pm.a.diag[cell] += d;
            pm.b[cell] += d * p_old[cell];
        }
        // Solve for the pressure change: the flux is then the (exactly differenced) old-pressure part plus a small
        // correction, which keeps round-off in the large absolute pressure out of the mass flux.
        let mut pm_d = pm.clone();
        pm_d.b = pm.residual(&p_old);
        let mut dp = vec![0.0; n];
        let stats = if pm_d.a.is_symmetric() && n > 2000 {
            pm_d.solve_amg(&mut dp, &s.pressure_solver)
        } else {
            pm_d.solve(&mut dp, &s.pressure_solver)
        };
        let p_new: Vec<f64> = (0..n).map(|c| p_old[c] + dp[c]).collect();
        let p_iters = stats.iterations;
        // corrected mass flux
        let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
        let own = mesh.owner();
        let nei = mesh.neighbour();
        let mut phi = vec![0.0; mesh.n_faces()];
        for f in 0..mesh.n_faces() {
            if g.empty[f] {
                continue;
            }
            if f < n_int {
                let up = if phid[f] >= 0.0 { own[f] } else { nei[f] };
                let mut flux = m_base[f] - gamma_p[f] * g.lap_coeff[f] * (p_old[nei[f]] - p_old[own[f]]);
                flux -= gamma_p[f] * g.lap_coeff[f] * (dp[nei[f]] - dp[own[f]]);
                if compressible {
                    flux += phid[f] * dp[up];
                }
                phi[f] = flux;
            } else {
                let i = f - n_int;
                if pbc.fixed_u[i] {
                    phi[f] = m_base[f];
                } else {
                    let co = &pbc.coeffs[i];
                    let g_old = co.gradient(p_old[own[f]]);
                    let mut flux = m_base[f] - gamma_p[f] * vec3::norm(sf[f]) * g_old;
                    flux -= gamma_p[f] * vec3::norm(sf[f]) * (co.gradient(p_new[own[f]]) - g_old);
                    if compressible {
                        flux += phid[f] * (bv_new[i] - p_up_old[f]);
                    }
                    phi[f] = flux;
                }
            }
        }
        for f in 0..mesh.n_faces() {
            self.vflux[f] = if g.empty[f] || (f >= n_int && pbc.fixed_u[f - n_int]) || rho_lin[f] <= 0.0 {
                0.0
            } else {
                (phi[f] - (rho_ho[f] - rho_lin[f]) * v_prev[f]) / rho_lin[f]
            };
        }
        self.phi = phi;

        // relax pressure, correct velocity
        for c in 0..n {
            self.p.values[c] = p_old[c] + s.alpha_p * (p_new[c] - p_old[c]);
        }
        if closed && !compressible {
            let shift = s.p_ref.1 - self.p.values[s.p_ref.0];
            for pv in self.p.values.iter_mut() {
                *pv += shift;
            }
        }
        let grad_corr = green_gauss(&g, &p_new, &bv_new, 0);
        // body force present: u = HbyA0 + Rec(rAU f.S - (rAU - rAtU) snGrad p_old |S| - rAtU snGrad p_new |S|)
        let face_corr: Option<Vec<Vec3>> = body.as_ref().map(|bf| {
            let r_au_f = face_interpolate(&g, &r_au);
            let r_atu_f = face_interpolate(&g, &r_atu);
            let mut ssf = vec![0.0; mesh.n_faces()];
            for f in 0..mesh.n_faces() {
                if g.empty[f] {
                    continue;
                }
                ssf[f] = if f < n_int {
                    let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
                    r_au_f[f] * bf.face[f]
                        - r_au_f[f] * g.lap_coeff[f] * (p_old[nb] - p_old[o])
                        - r_atu_f[f] * g.lap_coeff[f] * (dp[nb] - dp[o])
                } else if pbc.fixed_u[f - n_int] {
                    0.0
                } else {
                    let o = mesh.owner()[f];
                    r_au_f[f] * bf.face[f] - r_atu_f[f] * vec3::norm(sf[f]) * pbc.coeffs[f - n_int].gradient(p_new[o])
                };
            }
            self.reconstruct(&ssf)
        });
        let mut max_du: f64 = 0.0;
        for c in 0..n {
            for k in 0..3 {
                if g.active[k] {
                    let un = match &face_corr {
                        Some(fc) => hby_a0[c][k] + fc[c][k],
                        None => hby_a[c][k] - r_atu[c] * grad_corr[c][k],
                    };
                    self.u.values[c][k] = un;
                    max_du = max_du.max((un - u_old[c][k]).abs());
                }
            }
        }
        // ---- energy and density ------------------------------------------------------------
        let res_h = self.solve_energy();
        // closed compressible domain: the level comes from the mass, not from the pinned cell
        if compressible && closed {
            let (mut num, mut den) = (0.0, 0.0);
            for c in 0..n {
                let psi = self.psi_eos(self.p.values[c], self.t.values[c]);
                num += vol[c] * psi * self.p.values[c];
                den += vol[c] * psi;
            }
            let shift = (self.mass - num) / den;
            for pv in self.p.values.iter_mut() {
                *pv += shift;
            }
        }
        self.update_density(false);

        // continuity measure
        let mut cont: f64 = 0.0;
        for c in 0..n {
            let mut sum = 0.0;
            for &f in mesh.cell_faces(c) {
                if g.empty[f] {
                    continue;
                }
                sum += if f >= n_int || mesh.owner()[f] == c {
                    self.phi[f]
                } else {
                    -self.phi[f]
                };
            }
            cont = cont.max(sum.abs());
        }
        let area_scale = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
        let rho_scale = self.rho.iter().cloned().fold(0.0, f64::max);
        let info = VdInfo {
            iteration: self.iteration,
            max_du,
            continuity: cont / (area_scale * rho_scale).max(1e-300),
            p_iters,
            res_u,
            res_p,
            res_h,
        };
        self.history.push(info);
        info
    }

    /// Iterate until the normalised momentum, pressure and energy residuals are below `tol` or `max_iter` is hit.
    pub fn run_until(&mut self, max_iter: usize, tol: f64) -> VdInfo {
        let mut last = None;
        for _ in 0..max_iter {
            let i = self.iterate();
            last = Some(i);
            if i.res_u < tol && i.res_p < tol && i.res_h < tol {
                break;
            }
        }
        last.unwrap_or(VdInfo {
            iteration: 0,
            max_du: f64::INFINITY,
            continuity: f64::INFINITY,
            p_iters: 0,
            res_u: f64::INFINITY,
            res_p: f64::INFINITY,
            res_h: f64::INFINITY,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideal_gas_relations() {
        let gas = IdealGas {
            r: 287.0,
            cp: 3.5 * 287.0,
        };
        assert!((gas.gamma() - 1.4).abs() < 1e-12);
        assert!((gas.rho(101_325.0, 300.0) - 1.1769).abs() < 1e-3);
        assert!((gas.rho(2.0 * 101_325.0, 300.0) / gas.rho(101_325.0, 300.0) - 2.0).abs() < 1e-12);
        assert!((gas.psi(300.0) * 101_325.0 - gas.rho(101_325.0, 300.0)).abs() < 1e-12);
        assert!((gas.sound_speed(300.0) - 347.2).abs() < 0.1);
    }

    #[test]
    fn sutherland_law_returns_the_reference_value() {
        let mu = Property::Sutherland {
            reference: 1.68e-5,
            t_ref: 273.0,
            s: 110.5,
        };
        assert!((mu.at(273.0) - 1.68e-5).abs() < 1e-18);
        assert!(mu.at(600.0) > mu.at(300.0));
        assert_eq!(Property::Constant(2.0).at(1234.0), 2.0);
    }
}
