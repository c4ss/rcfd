//! Moving-mesh (ALE) step of the transient solver (F-063, ADR 0033 D2/D3).
//!
//! The mesh moves between two steps; the solver then advances one Euler step on the new geometry with the
//! mesh flux `m_f = W_f/dt` of the swept volumes (see `rcfd_mesh::motion`):
//!
//! ```text
//! (V1 u1 - V0 u0)/dt + sum_f (phi_f - m_f) u_f - diffusion = -V1 grad p,     sum_f phi_f = 0
//! ```
//!
//! The convection operator is assembled in conservative form, so the relative flux `phi - m` is simply handed to it;
//! the right-hand side of the transient term carries the old volume. Child module of `pimple`: it reads the private
//! solver history.

use super::{PimpleSolver, StepInfo};
use crate::convection::face_flux;
use crate::ddt::TimeScheme;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use rcfd_core::vec3::Vec3;
use std::sync::Arc;

/// Data of one moving-mesh step, consumed by `PimpleSolver::step` while it is set.
#[derive(Debug, Clone)]
pub struct AleStep {
    /// Mesh flux `W_f/dt` of every face (positive out of the owner, zero on empty faces).
    pub mesh_flux: Vec<f64>,
    /// Cell volumes of the old mesh.
    pub vol_old: Vec<f64>,
    /// Old-level Rhie-Chow defect of the internal faces (computed with the old face areas, scaled by `V0/V1`).
    pub defect: Vec<f64>,
    /// Outward-normal pressure gradient prescribed on moving-wall faces (index `f - n_internal`, `NaN` = none): the
    /// fixed-flux condition `dp/dn = -n . a_wall`.
    pub wall_dpdn: Vec<f64>,
}

impl AleStep {
    /// Relative flux `phi - m`.
    pub fn relative_flux(&self, phi: &[f64]) -> Vec<f64> {
        phi.iter().zip(&self.mesh_flux).map(|(p, m)| p - m).collect()
    }

    /// Replace the pressure condition of the fixed-flux moving-wall faces by the prescribed gradient.
    pub fn apply_wall_gradients(&self, pbc: &mut crate::pressure::PressureBc, dn: &[f64]) {
        for (i, g) in self.wall_dpdn.iter().enumerate() {
            if g.is_finite() && pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                pbc.coeffs[i] = crate::bc::FaceCoeffs {
                    a: 1.0,
                    b: dn[i] * g,
                    c: 0.0,
                    d: *g,
                };
            }
        }
    }

    /// Balanced-force counterpart of [`apply_wall_gradients`](Self::apply_wall_gradients): the flux of a fixed-flux
    /// face is prescribed, so the Neumann term of the pressure equation is cancelled by adding it to `phiHbyA`
    /// (as for the body-force condition of the steady solver).
    pub fn add_wall_flux(
        &self,
        g: &FvGeometry,
        pbc: &crate::pressure::PressureBc,
        gamma_p: &[f64],
        phi_hby_a: &mut [f64],
    ) {
        let sf = g.mesh.face_area_vectors();
        for (i, d) in self.wall_dpdn.iter().enumerate() {
            if d.is_finite() && pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                let f = g.n_internal + i;
                phi_hby_a[f] += gamma_p[f] * rcfd_core::vec3::norm(sf[f]) * d;
            }
        }
    }

    /// `add_ddt` used the new volume for the old level too; put the old volume back:
    /// `b += (V0 - V1)/dt u0`.
    pub fn fix_ddt(&self, m: &mut FvMatrix, vol_new: &[f64], u_old: &[f64], dt: f64) {
        for c in 0..u_old.len() {
            m.b[c] += (self.vol_old[c] - vol_new[c]) / dt * u_old[c];
        }
    }
}

impl PimpleSolver {
    /// Advance one Euler step of size `dt` on the moved mesh described by `new_geom`, with the mesh flux `mesh_flux`
    /// (`W_f/dt`, face count of the mesh). The topology of `new_geom` must equal that of the current geometry.
    /// Fails (leaving the solver unchanged) for time schemes other than `ImplicitEuler` and for a size mismatch.
    pub fn step_ale(&mut self, dt: f64, new_geom: Arc<FvGeometry>, mesh_flux: Vec<f64>) -> Result<StepInfo, String> {
        self.step_ale_with_walls(dt, new_geom, mesh_flux, Vec::new())
    }

    /// [`step_ale`](Self::step_ale) with the fixed-flux pressure gradient of the moving walls (from
    /// `ale::set_follow_conditions`; empty = zero normal gradient everywhere, which is wrong for accelerating walls).
    pub fn step_ale_with_walls(
        &mut self,
        dt: f64,
        new_geom: Arc<FvGeometry>,
        mut mesh_flux: Vec<f64>,
        wall_dpdn: Vec<f64>,
    ) -> Result<StepInfo, String> {
        if self.settings.time_scheme != TimeScheme::ImplicitEuler {
            return Err("a moving mesh supports the implicit Euler time scheme only".into());
        }
        let old = self.geom.clone();
        if new_geom.n_cells() != old.n_cells()
            || new_geom.mesh.n_faces() != old.mesh.n_faces()
            || mesh_flux.len() != old.mesh.n_faces()
        {
            return Err("moving-mesh step: the new mesh does not have the topology of the current one".into());
        }
        for (f, m) in mesh_flux.iter_mut().enumerate() {
            if old.empty[f] {
                *m = 0.0;
            }
        }
        let vol_old = old.mesh.cell_volumes().to_vec();
        let vol_new = new_geom.mesh.cell_volumes();
        let n_int = old.n_internal;
        let defect = if self.settings.ddt_correction {
            let zero_b = vec![[0.0; 3]; old.mesh.n_boundary_faces()];
            let uold: Vec<Vec3> = self.u.values.clone();
            let fo = face_flux(&old, &uold, &zero_b);
            let (own, nei) = (new_geom.mesh.owner(), new_geom.mesh.neighbour());
            (0..n_int)
                .map(|f| {
                    let (o, n) = (own[f], nei[f]);
                    let w = new_geom.weights[f];
                    let r = w * vol_old[o] / vol_new[o] + (1.0 - w) * vol_old[n] / vol_new[n];
                    r * (self.phi[f] - fo[f])
                })
                .collect()
        } else {
            vec![0.0; n_int]
        };
        self.geom = new_geom;
        self.ale = Some(AleStep {
            mesh_flux,
            vol_old,
            defect,
            wall_dpdn,
        });
        let info = self.step(dt);
        self.ale = None;
        Ok(info)
    }
}
