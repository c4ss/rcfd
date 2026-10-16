//! Non-iterative fractional-step (projection) time advance for the transient solver (F-210, ADR 0037 D3).
//!
//! One momentum predictor and ONE pressure Poisson solve per step; no outer or corrector iterations.
//!
//! * `incremental = true` (pressure-correction form): the predictor carries `grad p^n`, the Poisson problem is for the
//!   increment `q` with homogeneous boundary conditions, `p^{n+1} = p^n + q`.
//! * `incremental = false` (Chorin form): the predictor carries no pressure, the Poisson problem is for `p^{n+1}` itself.
//! * `kim_moin = true`: the velocity condition of the intermediate field is `u_b + dt (grad q)_tangential` with the
//!   unknown `q` lagged by one step (incremental: the previous increment; Chorin: `p^n`), after Kim and Moin (1985).
//!
//! The face flux of the intermediate field is the collocated form `interp(u*).S + dt (interp(grad p^n).S - snGrad(p^n)|S|)`
//! (the pressure force carried by the cell velocity is replaced by the compact face gradient; incremental form only).
//! Child module of `pimple`: it reads the private history of the solver.

use super::{ddt_history_weights, PimpleSolver, StepInfo};
use crate::convection::{convection, face_flux};
use crate::ddt::{add_ddt, max_courant};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, pressure_bc, solve_pressure, PressureBc,
    PressureControls,
};
use crate::simple::normalised_residual;
use rcfd_core::vec3::{self, Vec3};

/// Options of the fractional-step advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FractionalSettings {
    /// Pressure-increment form (true) or Chorin's form with the full pressure in the projection (false).
    pub incremental: bool,
    /// Kim-Moin boundary treatment of the intermediate velocity (false: plain physical velocity).
    pub kim_moin: bool,
}

impl Default for FractionalSettings {
    fn default() -> Self {
        FractionalSettings {
            incremental: true,
            kim_moin: true,
        }
    }
}

/// Pressure conditions of the increment: the real ones with the explicit parts removed (homogeneous Dirichlet where the
/// pressure is fixed, zero gradient where the normal velocity is prescribed); coupled faces unchanged.
fn homogeneous(bc: &PressureBc) -> PressureBc {
    let mut out = bc.clone();
    for (c, cp) in out.coeffs.iter_mut().zip(&bc.coupled) {
        if cp.is_none() {
            c.b = 0.0;
            c.d = 0.0;
        }
    }
    out
}

impl PimpleSolver {
    /// One non-iterative fractional step of size `dt` (see the module documentation).
    pub(crate) fn step_fractional(&mut self, dt: f64) -> StepInfo {
        let fs = self.settings.fractional.unwrap_or_default();
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        self.step_no += 1;
        if self.fs_q.len() != n {
            self.fs_q = vec![0.0; n];
        }
        let gamma_nu = match &self.nu_face {
            Some(v) => v.clone(),
            None => uniform_gamma(&g, s.nu),
        };
        let (c1, c2) = ddt_history_weights(&s.time_scheme, &self.hist_u[0], dt);
        // the pressure acts over the effective step dt/c0 (c0 = 1 for Euler, 3/2 for constant-step BDF2)
        let dte = dt / (c1 - c2);

        // pressure at the old level
        let pbc = pressure_bc(&g, &self.u, &self.p);
        let pbc_h = homogeneous(&pbc);
        let bvals_p = boundary_values_pbc(&g, &pbc, &self.p.values);
        let grad_p = green_gauss(&g, &self.p.values, &bvals_p, 0);
        // gradient of the lagged unknown for the boundary treatment
        let grad_shift: Vec<Vec3> = if fs.incremental {
            let bq = boundary_values_pbc(&g, &pbc_h, &self.fs_q);
            green_gauss(&g, &self.fs_q, &bq, 0)
        } else {
            grad_p.clone()
        };
        let own = mesh.owner();

        // ---- momentum predictor -------------------------------------------
        let u_old: Vec<Vec3> = self.u.values.clone();
        let extrap = if c2 != 0.0 { Some(c1 - 1.0) } else { None };
        let phi_conv: Vec<f64> = match (extrap, &self.phi_hist.older) {
            (Some(r), Some(older)) => (0..self.phi.len())
                .map(|f| (1.0 + r) * self.phi_hist.old[f] - r * older[f])
                .collect(),
            _ => self.phi.clone(),
        };
        let ub_phys = self.u.boundary_values(&g);
        let mut res_u: f64 = 0.0;
        for k in 0..3 {
            if !g.active[k] {
                continue;
            }
            let cells: Vec<f64> = u_old.iter().map(|v| v[k]).collect();
            let mut bc = self.u.boundary_coeffs(&g, k);
            if fs.kim_moin {
                for (i, c) in bc.iter_mut().enumerate() {
                    let f = n_int + i;
                    if c.a.abs() < 1e-12 && pbc.coupled[i].is_none() && !g.empty[f] {
                        let nrm = g.normal(f);
                        let gr = grad_shift[own[f]];
                        let gn = vec3::dot(gr, nrm);
                        let tang = gr[k] - gn * nrm[k];
                        c.b += dte * tang;
                        c.d += dte * tang / g.dn[i];
                    }
                }
            }
            let bv: Vec<f64> = ub_phys.iter().map(|v| v[k]).collect();
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
            let m = add_ddt(s.time_scheme, m, &vol, dt, &self.hist_u[k]);
            let mut sys = m;
            if fs.incremental {
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad_p[c][k];
                }
            }
            res_u = res_u.max(normalised_residual(&sys, &cells));
            let mut x = cells;
            sys.solve(&mut x, &s.momentum_solver);
            for c in 0..n {
                self.u.values[c][k] = x[c];
            }
        }

        // ---- intermediate flux ---------------------------------------------
        let ustar = self.u.values.clone();
        let hb = boundary_hby_a(&g, &pbc, &ub_phys, &ustar);
        let mut phi_star = face_flux(&g, &ustar, &hb);
        if fs.incremental {
            let p = &self.p.values;
            let nei = mesh.neighbour();
            for f in 0..mesh.n_faces() {
                if g.empty[f] {
                    continue;
                }
                if f < n_int {
                    let w = g.weights[f];
                    let gf = [0, 1, 2].map(|i| w * grad_p[own[f]][i] + (1.0 - w) * grad_p[nei[f]][i]);
                    phi_star[f] += dte * (vec3::dot(gf, sf[f]) - g.lap_coeff[f] * (p[nei[f]] - p[own[f]]));
                } else {
                    let i = f - n_int;
                    if let Some(gp) = pbc.coupled[i] {
                        let a = pbc.coeffs[i].a;
                        let gf = [0, 1, 2].map(|k| a * grad_p[own[f]][k] + (1.0 - a) * grad_p[own[gp]][k]);
                        let span = g.dn[i] + g.dn[gp - n_int];
                        phi_star[f] +=
                            dte * (vec3::dot(gf, sf[f]) - vec3::norm(sf[f]) * (p[own[gp]] - p[own[f]]) / span);
                    } else if !pbc.fixed_u[i] {
                        let co = &pbc.coeffs[i];
                        phi_star[f] +=
                            dte * (vec3::dot(grad_p[own[f]], sf[f]) - vec3::norm(sf[f]) * co.gradient(p[own[f]]));
                    }
                }
            }
        }

        // ---- the one Poisson solve and the projection ------------------------
        let gamma_p = vec![dte; mesh.n_faces()];
        let (bc_q, p_ref) = if fs.incremental {
            (&pbc_h, (s.p_ref.0, 0.0))
        } else {
            (&pbc, s.p_ref)
        };
        let controls = PressureControls {
            p_ref,
            non_orth_correctors: s.non_orth_correctors,
            solver: s.pressure_solver,
            amg: s.amg_pressure,
            linear: s.linear,
        };
        let mut q = if fs.incremental {
            vec![0.0; n]
        } else {
            self.p.values.clone()
        };
        let (res_p, p_iters) = solve_pressure(&g, &gamma_p, bc_q, &phi_star, &mut q, &controls);
        self.phi = corrected_flux(&g, &gamma_p, bc_q, &phi_star, &q, &ub_phys, s.non_orth_correctors > 0);
        let bq = boundary_values_pbc(&g, bc_q, &q);
        let grad_q = green_gauss(&g, &q, &bq, 0);
        for c in 0..n {
            for k in 0..3 {
                if g.active[k] {
                    self.u.values[c][k] = ustar[c][k] - dte * grad_q[c][k];
                }
            }
        }
        if fs.incremental {
            for c in 0..n {
                self.p.values[c] += q[c];
            }
            self.fs_q = q;
        } else {
            self.p.values = q.clone();
            self.fs_q = q;
        }

        // ---- finish the step -------------------------------------------------
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
}
