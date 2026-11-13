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
//! solver history. Between two steps `replace_mesh` installs a mesh of another topology (a layer event, F-257, ADR 0213);
//! `step_sliding` takes a step onto a mesh with the same cells and other faces (a sliding interface, F-250, ADR 0216).

use super::{PimpleSolver, StepInfo};
use crate::convection::face_flux;
use crate::ddt::{History, TimeScheme};
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
    /// Cell volumes at the old-old time of a three-level (BDF2) moving step (F-345, ADR 0379 D2); `None` on a two-level
    /// step, which takes [`fix_ddt`](Self::fix_ddt).
    pub vol_older: Option<Vec<f64>>,
}

/// What the next BDF2 moving step needs of the last one (F-345, ADR 0379 D5).
#[derive(Debug, Clone)]
pub(crate) struct AleHistory {
    /// Cell volumes at the start of the last step.
    vol: Vec<f64>,
    /// Mesh flux `W_f/dt` of the last step (as `move_mesh` gave it, empty faces zero).
    mesh_flux: Vec<f64>,
    /// Rhie-Chow defect `phi - (u)_f . S` of the internal faces at the start of the last step, on its geometry.
    defect: Vec<f64>,
    /// Size of the last step.
    dt: f64,
    /// Step count after the last step (the history is valid only for the step right after it).
    step_no: usize,
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

    /// The three-level counterpart of [`fix_ddt`](Self::fix_ddt) (F-345, ADR 0379 D2): `add_ddt(Bdf2)` used the new volume
    /// for both old levels; put the old and the old-old volume back,
    /// `b += [(V0 - V1) c1 u0 - (V00 - V1) c2 u00] / dt`. Without `vol_older` it is `fix_ddt`.
    #[allow(clippy::too_many_arguments)]
    pub fn fix_ddt_bdf2(
        &self,
        m: &mut FvMatrix,
        vol_new: &[f64],
        u_old: &[f64],
        u_older: &[f64],
        c1: f64,
        c2: f64,
        dt: f64,
    ) {
        let Some(vol_older) = &self.vol_older else {
            return self.fix_ddt(m, vol_new, u_old, dt);
        };
        for c in 0..u_old.len() {
            let old_part = (self.vol_old[c] - vol_new[c]) * (c1 * u_old[c]);
            let older_part = (vol_older[c] - vol_new[c]) * (c2 * u_older[c]);
            m.b[c] += (old_part - older_part) / dt;
        }
    }
}

impl PimpleSolver {
    /// Choose the time scheme of the moving-mesh step (F-345, ADR 0379 D1). `TimeScheme::Bdf2` sets `settings.time_scheme`
    /// and opts in to the three-level moving step of `step_ale` / `step_ale_with_walls`: the mesh flux
    /// `(c0 W^{n+1} - c2 W^n) / dt` of the swept volumes of the last two steps (the geometric conservation law of BDF2), the
    /// old and the old-old volume in the transient term and the three-level Rhie-Chow defect. A moving step without the
    /// history of the moving step just before it (the first one, the first after a step at rest, a `restore` or a
    /// `reinitialise`) is the implicit Euler moving step. `TimeScheme::ImplicitEuler` sets the scheme and returns to the
    /// solver that did not opt in (the history is dropped). Refused, leaving the solver unchanged: every other scheme.
    /// `step_sliding` and `replace_mesh` keep refusing BDF2. Pair it with `ale::set_follow_conditions_bdf2`.
    pub fn set_moving_mesh_time_scheme(&mut self, scheme: TimeScheme) -> Result<(), String> {
        match scheme {
            TimeScheme::Bdf2 => self.ale_bdf2_opt_in = true,
            TimeScheme::ImplicitEuler => {
                self.ale_bdf2_opt_in = false;
                self.ale_bdf2 = None;
            }
            _ => return Err("a moving mesh supports the implicit Euler or the BDF2 time scheme only".into()),
        }
        self.settings.time_scheme = scheme;
        Ok(())
    }

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
        // ADR 0379 D1: BDF2 only with the opt-in
        let bdf2_on = self.ale_bdf2_opt_in && self.settings.time_scheme == TimeScheme::Bdf2;
        if self.settings.time_scheme != TimeScheme::ImplicitEuler && !bdf2_on {
            return Err("a moving mesh supports the implicit Euler time scheme only".into());
        }
        if self.immersed.is_some() {
            return Err(
                "moving-mesh step: the immersed-solid forcing (ADR 0218) is not defined on a moving mesh".into(),
            );
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
        // ADR 0379 D3: the raw defect of this step, kept for the next one (BDF2 only)
        let mut raw_defect: Vec<f64> = Vec::new();
        let mut defect = if self.settings.ddt_correction {
            let zero_b = vec![[0.0; 3]; old.mesh.n_boundary_faces()];
            let uold: Vec<Vec3> = self.u.values.clone();
            let fo = face_flux(&old, &uold, &zero_b);
            if bdf2_on {
                raw_defect = (0..n_int).map(|f| self.phi[f] - fo[f]).collect();
            }
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
        if !bdf2_on {
            self.ale_bdf2 = None;
            self.geom = new_geom;
            self.ale = Some(AleStep {
                mesh_flux,
                vol_old,
                defect,
                wall_dpdn,
                vol_older: None,
            });
            let info = self.step(dt);
            self.ale = None;
            return Ok(info);
        }
        // ADR 0379 D2, D3, D5: the three-level data, when the step just before was a moving step with the opt-in
        let hist = self.ale_bdf2.take().filter(|h| {
            h.step_no == self.step_no && h.vol.len() == vol_old.len() && h.mesh_flux.len() == mesh_flux.len()
        });
        let step_flux = mesh_flux.clone();
        let mut vol_older = None;
        if let Some(h) = hist {
            let r = dt / h.dt;
            let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
            for (m1, m0) in mesh_flux.iter_mut().zip(&h.mesh_flux) {
                *m1 = c0 * *m1 - c2 / r * m0;
            }
            if self.settings.ddt_correction && h.defect.len() == n_int {
                let (own, nei) = (new_geom.mesh.owner(), new_geom.mesh.neighbour());
                for f in 0..n_int {
                    let (o, nn) = (own[f], nei[f]);
                    let w = new_geom.weights[f];
                    let r00 = w * h.vol[o] / vol_new[o] + (1.0 - w) * h.vol[nn] / vol_new[nn];
                    let mut d = c1 * defect[f];
                    d -= c2 * (r00 * h.defect[f]);
                    defect[f] = d;
                }
            }
            vol_older = Some(h.vol);
        }
        let three_level = vol_older.is_some();
        self.geom = new_geom;
        self.ale = Some(AleStep {
            mesh_flux,
            vol_old,
            defect,
            wall_dpdn,
            vol_older,
        });
        // without a history the step is the implicit Euler moving step (ADR 0379 D5)
        if !three_level {
            self.settings.time_scheme = TimeScheme::ImplicitEuler;
        }
        let info = self.step(dt);
        self.settings.time_scheme = TimeScheme::Bdf2;
        let a = self.ale.take().expect("the moving-mesh data of this step");
        self.ale_bdf2 = Some(AleHistory {
            vol: a.vol_old,
            mesh_flux: step_flux,
            defect: raw_defect,
            dt,
            step_no: self.step_no,
        });
        Ok(info)
    }

    /// Advance one Euler step of size `dt` across a sliding interface (F-250, ADR 0216 S3): `step` (from
    /// `sliding::sliding_step`) holds the new joined mesh, which has the cells of the current one but other faces, its mesh
    /// flux, the old volumes, the old-level face flux remapped onto the new faces and the old-level Rhie-Chow defect. The
    /// remapped flux becomes the solver's flux (and the start of its flux history, as `replace_mesh`), the defect is scaled
    /// by `w V0_o/V1_o + (1 - w) V0_n/V1_n` as in `step_ale_with_walls`, `wall_dpdn` is the fixed-flux gradient of the moving
    /// walls (from `sliding::rotating_wall_conditions`; empty = none), and the step is taken.
    ///
    /// Fails, leaving the solver unchanged, for a time scheme other than implicit Euler, another number of cells, data that do
    /// not fit the step's mesh, a patch list that differs in number, names, kinds or sizes, and a set face viscosity, body
    /// force, source force, sink or face jump (their data are sized to the current mesh).
    pub fn step_sliding(
        &mut self,
        dt: f64,
        step: crate::sliding::SlidingStep,
        wall_dpdn: Vec<f64>,
    ) -> Result<StepInfo, String> {
        if self.settings.time_scheme != TimeScheme::ImplicitEuler {
            return Err("step_sliding: a moving mesh supports the implicit Euler time scheme only".into());
        }
        let g = step.geom.clone();
        if g.n_cells() != self.geom.n_cells() {
            return Err(format!(
                "step_sliding: the step's mesh has {} cells, the solver's {}",
                g.n_cells(),
                self.geom.n_cells()
            ));
        }
        let (nf, ni, nb) = (g.mesh.n_faces(), g.n_internal, g.mesh.n_boundary_faces());
        if step.mesh_flux.len() != nf
            || step.phi_old.len() != nf
            || step.vol_old.len() != g.n_cells()
            || step.old_defect.len() != ni
            || !(wall_dpdn.is_empty() || wall_dpdn.len() == nb)
        {
            return Err("step_sliding: the step's data do not fit its mesh".into());
        }
        let same_patches = {
            let (a, b) = (self.geom.mesh.patches(), g.mesh.patches());
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(x, y)| x.name == y.name && x.kind == y.kind && x.size == y.size)
        };
        if !same_patches {
            return Err("step_sliding: the step's mesh must have the patches of the current one".into());
        }
        if self.nu_face.is_some()
            || self.body_force.is_some()
            || self.source_force.is_some()
            || self.sink.is_some()
            || self.face_jump.is_some()
        {
            return Err(
                "step_sliding: a face viscosity, body force, source force, sink or face jump is set (sized to the current mesh)"
                    .into(),
            );
        }
        let mut mesh_flux = step.mesh_flux;
        for (f, m) in mesh_flux.iter_mut().enumerate() {
            if g.empty[f] {
                *m = 0.0;
            }
        }
        let vol_new = g.mesh.cell_volumes();
        let vol_old = step.vol_old;
        let defect = if self.settings.ddt_correction {
            let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
            (0..ni)
                .map(|f| {
                    let (o, n) = (own[f], nei[f]);
                    let w = g.weights[f];
                    let r = w * vol_old[o] / vol_new[o] + (1.0 - w) * vol_old[n] / vol_new[n];
                    r * step.old_defect[f]
                })
                .collect()
        } else {
            vec![0.0; ni]
        };
        self.phi_hist = History::new(step.phi_old.clone());
        self.phi = step.phi_old;
        self.fs_q = Vec::new();
        self.geom = g;
        self.ale = Some(AleStep {
            mesh_flux,
            vol_old,
            defect,
            wall_dpdn,
            vol_older: None,
        });
        // ADR 0352 D2: the step-consistent face coefficient is not defined across a sliding interface; this step keeps the
        // default one
        let rc = std::mem::replace(&mut self.settings.rhie_chow_step_consistent, false);
        let info = self.step(dt);
        self.settings.rhie_chow_step_consistent = rc;
        self.ale = None;
        Ok(info)
    }

    /// Install a geometry of another topology between two steps, with one velocity and pressure per cell and one face
    /// flux per face of it (a layer event, F-257, ADR 0213 S2; the remapped state of `rcfd_mesh::layering`). The old time
    /// level of the next step becomes this state; the time, the step count, the history, the settings and the boundary
    /// conditions are kept. A condition with per-face data must be on a patch whose face count does not change.
    ///
    /// Fails, leaving the solver unchanged, for a time scheme other than implicit Euler, a patch list that differs in
    /// number, names or kinds, sizes that do not fit the new mesh, and a set face viscosity, body force, source force,
    /// sink, face jump or immersed-solid forcing (their data are sized to the current mesh).
    pub fn replace_mesh(
        &mut self,
        geom: Arc<FvGeometry>,
        u: Vec<Vec3>,
        p: Vec<f64>,
        phi: Vec<f64>,
    ) -> Result<(), String> {
        if self.settings.time_scheme != TimeScheme::ImplicitEuler {
            return Err("replace_mesh: a moving mesh supports the implicit Euler time scheme only".into());
        }
        let same_patches = {
            let (a, b) = (self.geom.mesh.patches(), geom.mesh.patches());
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.name == y.name && x.kind == y.kind)
        };
        if !same_patches {
            return Err("replace_mesh: the new mesh must have the patches of the current one".into());
        }
        if u.len() != geom.n_cells() || p.len() != geom.n_cells() || phi.len() != geom.mesh.n_faces() {
            return Err("replace_mesh: one velocity and pressure per cell and one flux per face are required".into());
        }
        if self.nu_face.is_some()
            || self.body_force.is_some()
            || self.source_force.is_some()
            || self.sink.is_some()
            || self.face_jump.is_some()
        {
            return Err(
                "replace_mesh: a face viscosity, body force, source force, sink or face jump is set (sized to the current mesh)"
                    .into(),
            );
        }
        if self.immersed.is_some() {
            return Err(
                "replace_mesh: the immersed-solid forcing (ADR 0218) is set (sized to the current mesh)".into(),
            );
        }
        self.hist_u = [0, 1, 2].map(|k| History::new(u.iter().map(|v| v[k]).collect()));
        self.phi_hist = History::new(phi.clone());
        self.geom = geom;
        self.u.values = u;
        self.p.values = p;
        self.phi = phi;
        self.fs_q = Vec::new();
        Ok(())
    }
}
