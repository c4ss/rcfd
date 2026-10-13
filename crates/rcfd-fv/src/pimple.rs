//! Transient incompressible laminar flow: PISO and PIMPLE.
//!
//! Per time step (kinematic pressure, constant viscosity):
//!
//! * **Momentum predictor** – assemble `ddt(U) + div(phi U) - div(nu grad U)`
//!   with the flux of the previous (outer) iterate and solve
//!   `A U = b - V grad p`.
//! * **PISO correctors** (Issa, 1986) – with the frozen momentum matrix, form
//!   `HbyA = H(U) / a_P`, the face flux `phiHbyA = HbyA_f . S` plus the
//!   transient Rhie-Chow (Choi, 1999) term that removes the time-step
//!   dependence of the collocated interpolation, solve
//!   `div(rAU grad p) = div(phiHbyA)`, then `phi = phiHbyA - rAU_f snGrad(p) |S|`
//!   and `U = HbyA - rAU grad p`.
//! * **PIMPLE** – wraps the above in `n_outer` outer iterations per step with
//!   implicit under-relaxation (`alpha_u`, `alpha_p`) on all but the last
//!   outer iteration, which allows Courant numbers above one. With
//!   `n_outer = 1` the algorithm is plain PISO.

use crate::convection::{convection, face_flux, ConvScheme};
use crate::ddt::{add_ddt, max_courant, CflController, History, TimeScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, face_interpolate, pressure_bc, solve_pressure,
    PressureControls,
};
use crate::simple::normalised_residual;
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Settings of the transient solver.
#[derive(Debug, Clone)]
pub struct PimpleSettings {
    /// Kinematic viscosity.
    pub nu: f64,
    /// Temporal scheme (`ImplicitEuler` or `Bdf2`).
    pub time_scheme: TimeScheme,
    /// Convection scheme for momentum.
    pub convection: ConvScheme,
    /// Outer iterations per step (1 = PISO).
    pub n_outer: usize,
    /// Pressure correctors per outer iteration (PISO uses 2).
    pub n_correctors: usize,
    /// Extra pressure solves with non-orthogonal correction per corrector.
    pub non_orth_correctors: usize,
    /// Velocity under-relaxation on non-final outer iterations.
    pub alpha_u: f64,
    /// Pressure under-relaxation on non-final outer iterations.
    pub alpha_p: f64,
    /// Apply the transient Rhie-Chow correction.
    pub ddt_correction: bool,
    /// Reference cell and value for a pressure level that is otherwise undetermined.
    pub p_ref: (usize, f64),
    /// Use AMG-preconditioned CG for the pressure equation.
    pub amg_pressure: bool,
    /// Linear solver controls for momentum.
    pub momentum_solver: SolveControl,
    /// Linear solver controls for pressure.
    pub pressure_solver: SolveControl,
}

impl PimpleSettings {
    /// PISO defaults: Euler time scheme, two correctors, one outer iteration.
    pub fn piso(nu: f64) -> Self {
        PimpleSettings {
            nu,
            time_scheme: TimeScheme::ImplicitEuler,
            convection: ConvScheme::Upwind,
            n_outer: 1,
            n_correctors: 2,
            non_orth_correctors: 0,
            alpha_u: 1.0,
            alpha_p: 1.0,
            ddt_correction: true,
            p_ref: (0, 0.0),
            amg_pressure: false,
            momentum_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-8,
                max_iter: 500,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-7,
                max_iter: 3000,
            },
        }
    }

    /// PIMPLE defaults: several outer iterations with under-relaxation.
    pub fn pimple(nu: f64, n_outer: usize) -> Self {
        PimpleSettings {
            n_outer,
            n_correctors: 1,
            alpha_u: 0.8,
            alpha_p: 0.5,
            ..Self::piso(nu)
        }
    }
}

/// Diagnostics of one time step.
#[derive(Debug, Clone, Copy)]
pub struct StepInfo {
    /// Step number (1-based).
    pub step: usize,
    /// Time after the step.
    pub time: f64,
    /// Step size used.
    pub dt: f64,
    /// Maximum Courant number of the new flux.
    pub courant: f64,
    /// Largest cell imbalance of the corrected flux, normalised by the face-area scale.
    pub continuity: f64,
    /// Normalised initial momentum residual of the first outer iteration.
    pub res_u: f64,
    /// Normalised initial pressure residual of the first corrector of the last outer iteration.
    pub res_p: f64,
    /// Linear-solver iterations spent on pressure.
    pub p_iters: usize,
}

/// Transient PISO/PIMPLE solver state.
pub struct PimpleSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: PimpleSettings,
    /// Velocity.
    pub u: VectorField,
    /// Kinematic pressure.
    pub p: ScalarField,
    /// Face flux (positive out of the owner).
    pub phi: Vec<f64>,
    /// Current time.
    pub time: f64,
    /// Diagnostics of every step.
    pub history: Vec<StepInfo>,
    hist_u: [History; 3],
    phi_hist: History,
    step_no: usize,
    /// Kinematic viscosity at every face; `None` uses `settings.nu`.
    pub nu_face: Option<Vec<f64>>,
}

impl PimpleSolver {
    /// Create a solver at `t = 0`; the initial flux is interpolated from `u`.
    pub fn new(geom: Arc<FvGeometry>, u: VectorField, p: ScalarField, settings: PimpleSettings) -> Self {
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        let hist_u = [0, 1, 2].map(|k| History::new(u.values.iter().map(|v| v[k]).collect()));
        let phi_hist = History::new(phi.clone());
        PimpleSolver {
            geom,
            settings,
            u,
            p,
            phi,
            time: 0.0,
            history: Vec::new(),
            hist_u,
            phi_hist,
            step_no: 0,
            nu_face: None,
        }
    }

    /// Advance one time step of size `dt`.
    pub fn step(&mut self, dt: f64) -> StepInfo {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        self.step_no += 1;

        let gamma_nu = match &self.nu_face {
            Some(v) => v.clone(),
            None => uniform_gamma(&g, s.nu),
        };
        let n_active = g.active.iter().filter(|&&a| a).count() as f64;
        let mut res_u: f64 = 0.0;
        let mut res_p = 0.0;
        let mut p_iters = 0;

        // transient Rhie-Chow data: (c1, c2) weights of the old levels, sized for the scheme
        let (c1, c2) = ddt_history_weights(&s.time_scheme, &self.hist_u[0], dt);
        // old-level flux defects `phi_old - (U_old)_f . S`
        let ddt_defect: Vec<f64> = if s.ddt_correction {
            let zero_b = vec![[0.0; 3]; mesh.n_boundary_faces()];
            let uold: Vec<Vec3> = (0..n).map(|c| [0, 1, 2].map(|k| self.hist_u[k].old[c])).collect();
            let fo = face_flux(&g, &uold, &zero_b);
            let mut d: Vec<f64> = (0..n_int).map(|f| c1 * (self.phi_hist.old[f] - fo[f])).collect();
            if c2 != 0.0 {
                let older: Vec<Vec3> = (0..n)
                    .map(|c| [0, 1, 2].map(|k| self.hist_u[k].older.as_ref().map_or(self.hist_u[k].old[c], |o| o[c])))
                    .collect();
                let fol = face_flux(&g, &older, &zero_b);
                let phi_older = self.phi_hist.older.as_ref().unwrap_or(&self.phi_hist.old);
                for f in 0..n_int {
                    d[f] -= c2 * (phi_older[f] - fol[f]);
                }
            }
            d
        } else {
            vec![0.0; n_int]
        };

        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let pbc = pressure_bc(&g, &self.u, &self.p);
            let ub = self.u.boundary_values(&g);
            let bvals_p = boundary_values_pbc(&g, &pbc, &self.p.values);
            let grad_p = green_gauss(&g, &self.p.values, &bvals_p, 0);

            // ---- momentum predictor -----------------------------------------
            let mut mats: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            let u_iter: Vec<Vec3> = self.u.values.clone();
            // second-order linearisation: on the first outer iteration of a BDF2 step the
            // convecting flux and the deferred-correction state are extrapolated in time
            let extrap = if outer == 0 && c2 != 0.0 { Some(c1 - 1.0) } else { None };
            let phi_conv: Vec<f64> = match (extrap, &self.phi_hist.older) {
                (Some(r), Some(older)) => (0..self.phi.len())
                    .map(|f| (1.0 + r) * self.phi_hist.old[f] - r * older[f])
                    .collect(),
                _ => self.phi.clone(),
            };
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let bc = self.u.boundary_coeffs(&g, k);
                let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                let conv_cells: Vec<f64> = match (extrap, &self.hist_u[k].older) {
                    (Some(r), Some(older)) => (0..n)
                        .map(|c| (1.0 + r) * self.hist_u[k].old[c] - r * older[c])
                        .collect(),
                    _ => cells.clone(),
                };
                let grad = if s.convection.needs_gradient() || s.non_orth_correctors > 0 {
                    Some(least_squares(&g, &conv_cells, &bv))
                } else {
                    None
                };
                let mut m = convection(&g, &phi_conv, &conv_cells, &bc, s.convection, grad.as_deref());
                m.add(&laplacian(&g, &gamma_nu, &bc, grad.as_deref()));
                let mut m = add_ddt(s.time_scheme, m, &vol, dt, &self.hist_u[k]);
                if alpha_u < 1.0 {
                    m.relax(alpha_u, &cells);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad_p[c][k];
                }
                if outer == 0 {
                    res_u = res_u.max(normalised_residual(&sys, &cells));
                }
                let mut x = cells;
                sys.solve(&mut x, &s.momentum_solver);
                for c in 0..n {
                    self.u.values[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                mats[k] = Some(m);
            }
            for a in a_p.iter_mut() {
                *a /= n_active;
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            let gamma_p = face_interpolate(&g, &r_au);

            // ---- pressure correctors ---------------------------------------
            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let h = m.a.h_operator(&cells, &m.b);
                        for c in 0..n {
                            hby_a[c][k] = h[c] / a_p[c];
                        }
                    }
                }
                let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a);
                let mut phi_hby_a = face_flux(&g, &hby_a, &hb);
                if s.ddt_correction {
                    for f in 0..n_int {
                        phi_hby_a[f] += gamma_p[f] / dt * ddt_defect[f];
                    }
                }
                let controls = PressureControls {
                    p_ref: s.p_ref,
                    non_orth_correctors: s.non_orth_correctors,
                    solver: s.pressure_solver,
                    amg: s.amg_pressure,
                };
                let p_old = self.p.values.clone();
                let mut p_new = p_old.clone();
                let (r0, it) = solve_pressure(&g, &gamma_p, &pbc, &phi_hby_a, &mut p_new, &controls);
                p_iters += it;
                if corr == 0 {
                    res_p = r0;
                }
                self.phi = corrected_flux(&g, &gamma_p, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0);
                let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
                let grad_new = green_gauss(&g, &p_new, &bv_new, 0);
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            self.u.values[c][k] = hby_a[c][k] - r_au[c] * grad_new[c][k];
                        }
                    }
                }
                let last_corr = corr + 1 == s.n_correctors;
                let a = if last_corr { alpha_p } else { 1.0 };
                for c in 0..n {
                    self.p.values[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
        }

        // ---- finish the step ------------------------------------------------
        for k in 0..3 {
            let new: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
            self.hist_u[k].push(new, dt);
        }
        self.phi_hist.push(self.phi.clone(), dt);
        self.time += dt;
        let cont = divergence(&g, &self.phi).iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        let area_scale = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
        let info = StepInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant: max_courant(&g, &self.phi, dt),
            continuity: cont / area_scale.max(1e-300),
            res_u,
            res_p,
            p_iters,
        };
        self.history.push(info);
        info
    }

    /// Advance to `t_end` with a fixed step `dt` (the last step is shortened to land on `t_end`).
    pub fn run_fixed(&mut self, t_end: f64, dt: f64) -> Vec<StepInfo> {
        let mut out = Vec::new();
        while self.time < t_end - 1e-12 * dt.max(1e-300) {
            let d = dt.min(t_end - self.time);
            out.push(self.step(d));
        }
        out
    }

    /// Advance to `t_end` with a Courant-controlled step starting from `dt0`.
    pub fn run_adaptive(&mut self, t_end: f64, dt0: f64, ctl: &CflController) -> Vec<StepInfo> {
        let mut out = Vec::new();
        let mut dt = dt0;
        while self.time < t_end - 1e-12 {
            let d = dt.min(t_end - self.time);
            let info = self.step(d);
            dt = ctl.next_dt(&self.geom, &self.phi, d);
            out.push(info);
        }
        out
    }

    /// Volume-integrated kinetic energy per unit mass.
    pub fn kinetic_energy(&self) -> f64 {
        let vol = self.geom.mesh.cell_volumes();
        self.u
            .values
            .iter()
            .zip(vol)
            .map(|(u, v)| 0.5 * vec3::dot(*u, *u) * v)
            .sum()
    }
}

/// Weights `(c1, c2)` of the old levels in the time derivative
/// `(c0 u - c1 u^n + c2 u^{n-1}) / dt` for the active scheme and history.
fn ddt_history_weights(scheme: &TimeScheme, hist: &History, dt: f64) -> (f64, f64) {
    match (scheme, &hist.older, hist.dt_old) {
        (TimeScheme::Bdf2, Some(_), Some(dt_old)) => {
            let r = dt / dt_old;
            (1.0 + r, r * r / (1.0 + r))
        }
        _ => (1.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::box_2d;
    use std::f64::consts::PI;

    /// Taylor-Green vortex on a doubly periodic square.
    pub(crate) fn taylor_green(
        n: usize,
        nu: f64,
        settings: impl FnOnce(PimpleSettings) -> PimpleSettings,
    ) -> PimpleSolver {
        let l = 2.0 * PI;
        let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
        let geom = Arc::new(FvGeometry::new(mesh).unwrap());
        let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
        // initial pressure p = +(cos 2x + cos 2y)/4 for u = (sin x cos y, -cos x sin y)
        // (Taylor-Green; the earlier minus sign belongs to the u = (-cos x sin y, sin x cos y) form)
        let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
        PimpleSolver::new(geom, u, p, settings(PimpleSettings::piso(nu)))
    }

    /// Relative amplitude error of the velocity field against the analytic decay.
    fn amplitude_error(s: &PimpleSolver, nu: f64) -> f64 {
        let decay = (-2.0 * nu * s.time).exp();
        let c = s.geom.mesh.cell_centres();
        let (mut num, mut den) = (0.0, 0.0);
        for (i, u) in s.u.values.iter().enumerate() {
            let ex = [
                c[i][0].sin() * c[i][1].cos() * decay,
                -c[i][0].cos() * c[i][1].sin() * decay,
            ];
            num += (u[0] - ex[0]).powi(2) + (u[1] - ex[1]).powi(2);
            den += ex[0] * ex[0] + ex[1] * ex[1];
        }
        (num / den).sqrt()
    }

    #[test]
    fn taylor_green_decay_below_one_percent() {
        let nu = 0.05;
        let mut s = taylor_green(64, nu, |mut st| {
            st.convection = ConvScheme::Central;
            st
        });
        let ke0 = 0.25 * s.geom.mesh.total_volume(); // mean of 0.5|u|^2 is 1/4
        s.run_fixed(2.0, 0.02);
        let ke_exact = ke0 * (-4.0 * nu * 2.0f64).exp();
        let ke_err = (s.kinetic_energy() - ke_exact).abs() / ke_exact;
        assert!(ke_err < 0.01, "kinetic energy error {ke_err}");
        let amp = amplitude_error(&s, nu);
        assert!(amp < 0.01, "field error {amp}");
        assert!(s.history.last().unwrap().continuity < 1e-6);
    }

    #[test]
    fn taylor_green_error_is_second_order_in_space() {
        let nu = 0.1;
        let err = |n| {
            let mut s = taylor_green(n, nu, |mut st| {
                st.convection = ConvScheme::Central;
                st
            });
            s.run_fixed(0.5, 0.01);
            amplitude_error(&s, nu)
        };
        let (e1, e2) = (err(16), err(32));
        assert!(e1 / e2 > 3.0, "ratio {}", e1 / e2);
    }

    #[test]
    fn pimple_outer_iterations_and_bdf2_reduce_the_temporal_error() {
        let nu = 0.1;
        let run = |scheme, dt, outer| {
            let mut s = taylor_green(32, nu, |mut st| {
                st.convection = ConvScheme::Central;
                st.time_scheme = scheme;
                st.n_outer = outer;
                st.alpha_u = 1.0;
                st.alpha_p = 1.0;
                st
            });
            s.run_fixed(1.0, dt);
            s
        };
        let reference = run(TimeScheme::Bdf2, 0.0125, 3);
        let diff = |a: &PimpleSolver| {
            a.u.values
                .iter()
                .zip(&reference.u.values)
                .map(|(x, y)| (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2))
                .sum::<f64>()
                .sqrt()
        };
        let piso = diff(&run(TimeScheme::ImplicitEuler, 0.1, 1));
        let pimple = diff(&run(TimeScheme::ImplicitEuler, 0.1, 3));
        let bdf2 = diff(&run(TimeScheme::Bdf2, 0.1, 3));
        assert!(pimple < 0.5 * piso, "piso {piso} pimple {pimple}");
        assert!(bdf2 < pimple, "euler {pimple} bdf2 {bdf2}");
    }

    #[test]
    fn pimple_allows_courant_above_one() {
        let nu = 0.1;
        let mut s = taylor_green(16, nu, |mut st| {
            st.convection = ConvScheme::Central;
            st.n_outer = 4;
            st.n_correctors = 2;
            st.alpha_u = 0.9;
            st.alpha_p = 0.9;
            st
        });
        // dx = 0.39, |u| <= 1 -> Co ~ dt / dx; dt = 0.5 gives Co ~ 1.3
        let infos = s.run_fixed(1.0, 0.5);
        assert!(infos.iter().any(|i| i.courant > 1.0), "{infos:?}");
        let err = amplitude_error(&s, nu);
        assert!(err.is_finite() && err < 0.15, "err {err}");
    }

    #[test]
    fn piso_mean_pressure_and_momentum_are_conserved() {
        let mut s = taylor_green(16, 0.05, |st| st);
        s.run_fixed(0.2, 0.02);
        let vol = s.geom.mesh.cell_volumes();
        let tot = vol.iter().sum::<f64>();
        let mom: f64 = s.u.values.iter().zip(vol).map(|(u, v)| u[0] * v).sum::<f64>() / tot;
        assert!(mom.abs() < 1e-8, "{mom}");
    }
}
